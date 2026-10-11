//! AirPlay output through AVFoundation: an audio renderer on a shared audio output
//! context, a render synchronizer, a feeder thread, and the route picker popover.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, ProtocolObject};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_core_audio_types::{
    AudioStreamBasicDescription, kAudioFormatFlagIsFloat, kAudioFormatFlagIsPacked,
    kAudioFormatLinearPCM,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{
    CMAudioFormatDescriptionCreate, CMAudioSampleBufferCreateReadyWithPacketDescriptions,
    CMBlockBuffer, CMFormatDescription, CMSampleBuffer, CMTime, kCMBlockBufferAssureMemoryNowFlag,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNotificationName, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString,
};
use tauri::{AppHandle, Manager};

use crate::output::clock::{self, DriftMonitor};
use crate::output::controller::{PickerAnchor, RouteError, RoutePoll};
use crate::output::ring::SampleRing;

// SAFETY: The static is an `NSString` constant that AVFoundation exports.
#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification: &'static NSString;
}

// Linking AVKit makes `AVRoutePickerView` available to `AnyClass::get`.
// SAFETY: The block declares no items.
#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {}

/// Frames in one enqueued sample buffer.
const CHUNK_FRAMES: usize = 4_800;
const CHANNELS: usize = 2;
const CHUNK_SAMPLES: usize = CHUNK_FRAMES * CHANNELS;
/// Audio queued ahead before the synchronizer starts, in seconds.
const PRIME_S: f64 = 0.3;
const FEED_INTERVAL: Duration = Duration::from_millis(10);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
/// `AVQueuedSampleBufferRenderingStatusFailed`.
const RENDERER_FAILED: isize = 2;
/// `NSWindowBelow`.
const WINDOW_BELOW: isize = -1;

/// The AVFoundation objects of the route.
pub(super) struct Av {
    context: Retained<AnyObject>,
    context_id: Retained<NSString>,
    renderer: Retained<AnyObject>,
    synchronizer: Retained<AnyObject>,
}

// SAFETY: The `Retained` handles are never replaced after creation. The controller
// thread calls `status`, `outputDevices`, and `setRate:` while the feeder thread
// enqueues and flushes, so these calls are not serialized. The code relies on AVFoundation
// tolerating cross-thread calls on these objects: the probe read the renderer `status`
// on the main thread while it enqueued from a dispatch queue. Synchronizer rate changes
// are serialized by `Shared::rate_lock`. The picker view stays on the main thread (it is
// not in this type).
unsafe impl Send for Av {}
// SAFETY: See the `Send` impl. All shared access goes through `&self` methods that only
// send messages to the objects.
unsafe impl Sync for Av {}

impl Av {
    /// True when every class the route needs exists on this system.
    pub(super) fn classes_exist() -> bool {
        [
            c"CATapDescription",
            c"AVOutputContext",
            c"AVSampleBufferAudioRenderer",
            c"AVSampleBufferRenderSynchronizer",
            c"AVRoutePickerView",
        ]
        .iter()
        .all(|name| AnyClass::get(name).is_some())
    }

    /// Creates the output context, renderer, and synchronizer. Call on the main thread.
    pub(super) fn create(_main: MainThreadMarker) -> Option<Self> {
        let context_class = AnyClass::get(c"AVOutputContext")?;
        let renderer_class = AnyClass::get(c"AVSampleBufferAudioRenderer")?;
        let synchronizer_class = AnyClass::get(c"AVSampleBufferRenderSynchronizer")?;
        // SAFETY: `+addSharedAudioOutputContext` exists on `AVOutputContext` (the probe
        // used it). `new` returns a retained object for these classes. The renderer and
        // context selectors match the probe's call sequence.
        unsafe {
            let responds: bool =
                msg_send![context_class, respondsToSelector: sel!(addSharedAudioOutputContext)];
            if !responds {
                return None;
            }
            let context: Option<Retained<AnyObject>> =
                msg_send![context_class, addSharedAudioOutputContext];
            let context = context?;
            let context_id: Option<Retained<NSString>> = msg_send![&*context, contextID];
            let context_id = context_id?;
            let renderer: Retained<AnyObject> = msg_send![renderer_class, new];
            let _: () = msg_send![&*renderer, setOutputContext: &*context];
            let synchronizer: Retained<AnyObject> = msg_send![synchronizer_class, new];
            let _: () = msg_send![&*synchronizer, addRenderer: &*renderer];
            Some(Self {
                context,
                context_id,
                renderer,
                synchronizer,
            })
        }
    }

    fn current_time(&self) -> CMTime {
        // SAFETY: `currentTime` returns a `CMTime` by value.
        unsafe { msg_send![&*self.synchronizer, currentTime] }
    }

    fn set_rate(&self, rate: f32) {
        // SAFETY: `setRate:` takes a float.
        unsafe {
            let _: () = msg_send![&*self.synchronizer, setRate: rate];
        }
    }

    fn set_rate_at(&self, rate: f32, time: CMTime) {
        // SAFETY: `setRate:time:` takes a float and a `CMTime`.
        unsafe {
            let _: () = msg_send![&*self.synchronizer, setRate: rate, time: time];
        }
    }

    fn flush(&self) {
        // SAFETY: `flush` takes no arguments.
        unsafe {
            let _: () = msg_send![&*self.renderer, flush];
        }
    }

    fn is_ready(&self) -> bool {
        // SAFETY: `isReadyForMoreMediaData` returns a BOOL.
        unsafe { msg_send![&*self.renderer, isReadyForMoreMediaData] }
    }

    fn enqueue(&self, buffer: &CMSampleBuffer) {
        // SAFETY: `enqueueSampleBuffer:` takes a `CMSampleBufferRef`, which `buffer` is.
        unsafe {
            let _: () = msg_send![&*self.renderer, enqueueSampleBuffer: buffer];
        }
    }

    fn renderer_failed(&self) -> bool {
        // SAFETY: `status` returns an NSInteger.
        let status: isize = unsafe { msg_send![&*self.renderer, status] };
        status == RENDERER_FAILED
    }

    fn receiver_names(&self) -> Vec<String> {
        // SAFETY: `outputDevices` returns an NSArray of `AVOutputDevice`, each with a
        // `deviceName` string. Index access stays below `count`.
        unsafe {
            let devices: Option<Retained<AnyObject>> = msg_send![&*self.context, outputDevices];
            let Some(devices) = devices else {
                return Vec::new();
            };
            let count: usize = msg_send![&*devices, count];
            let mut names = Vec::with_capacity(count);
            for index in 0..count {
                let device: Option<Retained<AnyObject>> =
                    msg_send![&*devices, objectAtIndex: index];
                let Some(device) = device else { continue };
                let name: Option<Retained<NSString>> = msg_send![&*device, deviceName];
                if let Some(name) = name {
                    names.push(name.to_string());
                }
            }
            names
        }
    }
}

type ObserverToken = Retained<ProtocolObject<dyn NSObjectProtocol>>;

/// Sets a flag when the renderer flushes itself (for example after a route change).
struct FlushObserver {
    center: Retained<NSNotificationCenter>,
    token: ObserverToken,
}

// SAFETY: `NSNotificationCenter` is thread-safe, and the token is only used to remove
// the observer.
unsafe impl Send for FlushObserver {}

impl FlushObserver {
    fn new(renderer: &AnyObject, shared: Arc<Shared>) -> Self {
        let center = NSNotificationCenter::defaultCenter();
        let block: RcBlock<dyn Fn(NonNull<NSNotification>)> = RcBlock::new(move |_| {
            shared.flush_requested.store(true, Ordering::Release);
        });
        // SAFETY: The notification name is a framework constant. The block does not use
        // its argument, and it only touches atomics.
        let token = unsafe {
            let name: &NSNotificationName =
                AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification;
            center.addObserverForName_object_queue_usingBlock(
                Some(name),
                Some(renderer),
                None,
                &block,
            )
        };
        Self { center, token }
    }
}

impl Drop for FlushObserver {
    fn drop(&mut self) {
        // SAFETY: `token` came from `addObserverForName...` on this center.
        unsafe { self.center.removeObserver(self.token.as_ref()) };
    }
}

/// State shared between the route, the flush observer, and the feeder thread.
struct Shared {
    flush_requested: AtomicBool,
    resync_requested: AtomicBool,
    playing: AtomicBool,
    /// True from start (and after each flush) until the synchronizer starts.
    waiting_for_prime: AtomicBool,
    /// `f64::to_bits` of the queue depth in milliseconds.
    queued_bits: AtomicU64,
    /// Serializes synchronizer rate changes between the feeder and `set_playing`.
    rate_lock: Mutex<()>,
}

struct Feeder {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub(super) struct Engine {
    av: Arc<Av>,
    shared: Arc<Shared>,
    _observer: FlushObserver,
    feeder: Option<Feeder>,
}

impl Engine {
    pub(super) fn new(av: Av) -> Self {
        let shared = Arc::new(Shared {
            flush_requested: AtomicBool::new(false),
            resync_requested: AtomicBool::new(false),
            playing: AtomicBool::new(false),
            waiting_for_prime: AtomicBool::new(true),
            queued_bits: AtomicU64::new(0f64.to_bits()),
            rate_lock: Mutex::new(()),
        });
        let observer = FlushObserver::new(&av.renderer, shared.clone());
        Self {
            av: Arc::new(av),
            shared,
            _observer: observer,
            feeder: None,
        }
    }

    pub(super) fn av(&self) -> &Arc<Av> {
        &self.av
    }

    /// Starts the feeder for a new tap. Fails before any thread starts when the audio
    /// format cannot be created or the thread cannot spawn.
    pub(super) fn start(
        &mut self,
        ring: Arc<SampleRing>,
        sample_rate: f64,
        playing: bool,
    ) -> Result<(), RouteError> {
        self.stop_feeder();
        let format = prepare_format(sample_rate)?;
        self.shared.playing.store(playing, Ordering::Release);
        self.shared.waiting_for_prime.store(true, Ordering::Release);
        self.shared.flush_requested.store(false, Ordering::Release);
        self.shared.resync_requested.store(false, Ordering::Release);
        let stop = Arc::new(AtomicBool::new(false));
        let av = self.av.clone();
        let shared = self.shared.clone();
        let stop_flag = stop.clone();
        let handle = thread::Builder::new()
            .name("gmusic-airplay-feeder".into())
            .spawn(move || run_feeder(&av, &shared, &ring, &stop_flag, &format, sample_rate))
            .map_err(|error| {
                RouteError::Unavailable(format!("The AirPlay feeder did not start: {error}"))
            })?;
        self.feeder = Some(Feeder { stop, handle });
        Ok(())
    }

    fn stop_feeder(&mut self) {
        if let Some(feeder) = self.feeder.take() {
            feeder.stop.store(true, Ordering::Release);
            let _ = feeder.handle.join();
        }
    }

    pub(super) fn stop(&mut self) {
        self.stop_feeder();
        let _guard = self
            .shared
            .rate_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.shared.waiting_for_prime.store(true, Ordering::Release);
        self.av.set_rate(0.0);
        self.av.flush();
        self.shared
            .queued_bits
            .store(0f64.to_bits(), Ordering::Release);
    }

    pub(super) fn set_playing(&self, playing: bool) {
        self.shared.playing.store(playing, Ordering::Release);
        let _guard = self
            .shared
            .rate_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !self.shared.waiting_for_prime.load(Ordering::Acquire) {
            self.av.set_rate(if playing { 1.0 } else { 0.0 });
        }
    }

    /// Asks the feeder to drop queued audio and restart at the synchronizer's time.
    pub(super) fn request_resync(&self) {
        self.shared.resync_requested.store(true, Ordering::Release);
    }

    pub(super) fn queued_ms(&self) -> f64 {
        f64::from_bits(self.shared.queued_bits.load(Ordering::Acquire))
    }

    pub(super) fn poll(&self) -> RoutePoll {
        RoutePoll {
            receivers: self.av.receiver_names(),
            failed: self.av.renderer_failed(),
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn frames_at(time: CMTime, sample_rate: f64) -> i64 {
    // SAFETY: `CMTimeGetSeconds` takes a `CMTime` by value.
    let seconds = unsafe { time.seconds() };
    if seconds.is_finite() {
        (seconds * sample_rate).round() as i64
    } else {
        0
    }
}

fn time_of(frames: i64, sample_rate: f64) -> CMTime {
    // SAFETY: `CMTimeMake` takes plain values.
    unsafe { CMTime::new(frames, sample_rate as i32) }
}

fn seconds_of(frames: i64, sample_rate: f64) -> f64 {
    frames as f64 / sample_rate
}

fn make_format(sample_rate: f64) -> Option<CFRetained<CMFormatDescription>> {
    let description = AudioStreamBasicDescription {
        mSampleRate: sample_rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8,
        mFramesPerPacket: 1,
        mBytesPerFrame: 8,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    };
    let mut out: *const CMFormatDescription = ptr::null();
    // SAFETY: `description` and `out` are live locals. Layout, cookie, and extensions
    // are null with zero sizes, as the API allows for linear PCM.
    let status = unsafe {
        CMAudioFormatDescriptionCreate(
            None,
            NonNull::from(&description),
            0,
            ptr::null(),
            0,
            ptr::null(),
            None,
            NonNull::from(&mut out),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: On success `out` holds a +1 reference that this value now owns.
    NonNull::new(out.cast_mut()).map(|pointer| unsafe { CFRetained::from_raw(pointer) })
}

/// The format description handed to the feeder thread.
struct SendFormat(CFRetained<CMFormatDescription>);

// SAFETY: A `CMFormatDescription` is an immutable Core Foundation object. Core Media
// documents such objects as safe to use from any thread, and this one is only read.
unsafe impl Send for SendFormat {}

fn prepare_format(sample_rate: f64) -> Result<SendFormat, RouteError> {
    make_format(sample_rate).map(SendFormat).ok_or_else(|| {
        RouteError::Unavailable("The AirPlay audio format could not be created.".into())
    })
}

fn make_sample_buffer(
    format: &CMFormatDescription,
    samples: &[f32],
    pts: CMTime,
) -> Option<CFRetained<CMSampleBuffer>> {
    let bytes = std::mem::size_of_val(samples);
    let frames = (samples.len() / CHANNELS) as isize;
    let mut block: *mut CMBlockBuffer = ptr::null_mut();
    // SAFETY: A null memory block with `AssureMemoryNow` makes Core Media allocate
    // `bytes` bytes. `block` is a live local.
    let status = unsafe {
        CMBlockBuffer::create_with_memory_block(
            None,
            ptr::null_mut(),
            bytes,
            None,
            ptr::null(),
            0,
            bytes,
            kCMBlockBufferAssureMemoryNowFlag,
            NonNull::from(&mut block),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: On success `block` holds a +1 reference that this value now owns.
    let block = unsafe { CFRetained::from_raw(NonNull::new(block)?) };
    // SAFETY: `samples` is `bytes` readable bytes. The block holds `bytes` bytes.
    let status = unsafe {
        CMBlockBuffer::replace_data_bytes(NonNull::from(samples).cast(), &block, 0, bytes)
    };
    if status != 0 {
        return None;
    }
    let mut buffer: *mut CMSampleBuffer = ptr::null_mut();
    // SAFETY: The block, format, and timestamp are valid. There are no packet
    // descriptions for constant-size PCM packets. `buffer` is a live local.
    let status = unsafe {
        CMAudioSampleBufferCreateReadyWithPacketDescriptions(
            None,
            &block,
            format,
            frames,
            pts,
            ptr::null(),
            NonNull::from(&mut buffer),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: On success `buffer` holds a +1 reference that this value now owns.
    NonNull::new(buffer).map(|pointer| unsafe { CFRetained::from_raw(pointer) })
}

struct FeederState {
    next_pts: i64,
    prime_start: i64,
    drift: DriftMonitor,
}

fn run_feeder(
    av: &Av,
    shared: &Shared,
    ring: &SampleRing,
    stop: &AtomicBool,
    format: &SendFormat,
    rate: f64,
) {
    let format = &*format.0;
    let epoch = Instant::now();
    let now_s = || epoch.elapsed().as_secs_f64();
    let start_pts = frames_at(av.current_time(), rate);
    let mut state = FeederState {
        next_pts: start_pts,
        prime_start: start_pts,
        drift: DriftMonitor::new(now_s()),
    };
    // Reserved once so `pop_into` never reallocates while it holds the ring lock.
    let mut chunk: Vec<f32> = Vec::with_capacity(CHUNK_SAMPLES);
    let mut last_stats = Instant::now();

    while !stop.load(Ordering::Acquire) {
        thread::sleep(FEED_INTERVAL);

        let flush = shared.flush_requested.swap(false, Ordering::AcqRel);
        let resync = shared.resync_requested.swap(false, Ordering::AcqRel);
        if flush || resync {
            let _guard = shared.rate_lock.lock().unwrap_or_else(|e| e.into_inner());
            shared.waiting_for_prime.store(true, Ordering::Release);
            av.set_rate(0.0);
            av.flush();
            ring.clear();
            state.next_pts = frames_at(av.current_time(), rate);
            state.prime_start = state.next_pts;
            state.drift.reset(now_s());
        }

        while av.is_ready() && ring.len() >= CHUNK_SAMPLES {
            chunk.clear();
            ring.pop_into(&mut chunk, CHUNK_SAMPLES);
            if chunk.len() < CHUNK_SAMPLES {
                break;
            }
            if let Some(buffer) = make_sample_buffer(format, &chunk, time_of(state.next_pts, rate))
            {
                av.enqueue(&buffer);
            }
            state.next_pts += CHUNK_FRAMES as i64;
        }

        if shared.waiting_for_prime.load(Ordering::Acquire)
            && seconds_of(state.next_pts - state.prime_start, rate) >= PRIME_S
        {
            let _guard = shared.rate_lock.lock().unwrap_or_else(|e| e.into_inner());
            // Clear the flag before reading `playing`, so a concurrent `set_playing`
            // either sees the flag clear (and sets the rate) or is read here.
            shared.waiting_for_prime.store(false, Ordering::Release);
            let playing = shared.playing.load(Ordering::Acquire);
            av.set_rate_at(
                if playing { 1.0 } else { 0.0 },
                time_of(state.prime_start, rate),
            );
            state.drift.reset(now_s());
        }

        if last_stats.elapsed() >= STATS_INTERVAL {
            last_stats = Instant::now();
            let queued = clock::queued_ms(
                seconds_of(state.next_pts, rate),
                seconds_of(frames_at(av.current_time(), rate), rate),
            );
            shared
                .queued_bits
                .store(queued.to_bits(), Ordering::Release);
            let primed = !shared.waiting_for_prime.load(Ordering::Acquire);
            if primed && state.drift.observe(now_s(), queued) {
                shared.resync_requested.store(true, Ordering::Release);
            }
        }
    }
}

thread_local! {
    /// One route picker per window label. Main thread only.
    static PICKERS: RefCell<HashMap<String, Retained<AnyObject>>> = RefCell::new(HashMap::new());
}

/// Dispatches the picker to the main thread and returns without waiting for it.
pub(super) fn open_picker(
    app: &AppHandle,
    av: &Arc<Av>,
    anchor: &PickerAnchor,
) -> Result<(), RouteError> {
    let unavailable = |message: String| RouteError::Unavailable(message);
    let window = app
        .get_webview_window(&anchor.window_label)
        .ok_or_else(|| unavailable("The window for the AirPlay picker was not found.".into()))?;
    // `ns_window()` waits on the main thread, so it runs inside the dispatched closure.
    let av = av.clone();
    let anchor = anchor.clone();
    app.run_on_main_thread(move || {
        if let Err(error) = show_picker(&window, &av, &anchor) {
            tracing::warn!(?error, "AirPlay picker did not open");
        }
    })
    .map_err(|error| {
        unavailable(format!(
            "The AirPlay picker could not be scheduled: {error}"
        ))
    })
}

fn show_picker(
    window: &tauri::WebviewWindow,
    av: &Av,
    anchor: &PickerAnchor,
) -> Result<(), RouteError> {
    let unavailable = |message: &str| RouteError::Unavailable(message.to_string());
    MainThreadMarker::new().ok_or_else(|| unavailable("The picker needs the main thread."))?;
    let ns_window = window
        .ns_window()
        .map_err(|_| unavailable("The window has no native handle."))?;
    // SAFETY: Tauri returns a valid `NSWindow` pointer for a live window. This runs on
    // the main thread, where AppKit objects may be used, and `window` is alive.
    let ns_window = unsafe { &*ns_window.cast::<AnyObject>() };
    // SAFETY: `contentView` returns the window's content view or nil.
    let content: Option<Retained<AnyObject>> = unsafe { msg_send![ns_window, contentView] };
    let content = content.ok_or_else(|| unavailable("The window has no content view."))?;
    // SAFETY: `frame` returns an `NSRect` by value.
    let content_frame: NSRect = unsafe { msg_send![&*content, frame] };
    let flipped_y = content_frame.size.height - (anchor.y + anchor.height);
    let frame = NSRect::new(
        NSPoint::new(anchor.x, flipped_y),
        NSSize::new(anchor.width, anchor.height),
    );

    let picker = PICKERS.with_borrow_mut(|pickers| -> Result<Retained<AnyObject>, RouteError> {
        if let Some(existing) = pickers.get(&anchor.window_label) {
            // SAFETY: `superview` returns a view or nil. The pointer is only compared.
            let superview: Option<Retained<AnyObject>> =
                unsafe { msg_send![&**existing, superview] };
            if superview.is_some_and(|view| ptr::eq(&*view, &*content)) {
                return Ok(existing.clone());
            }
            // The window changed under the same label. Drop the stale picker.
            // SAFETY: `removeFromSuperview` takes no arguments.
            unsafe {
                let _: () = msg_send![&**existing, removeFromSuperview];
            }
            pickers.remove(&anchor.window_label);
        }
        let class = AnyClass::get(c"AVRoutePickerView")
            .ok_or_else(|| unavailable("The AirPlay picker is unavailable."))?;
        // SAFETY: `AVRoutePickerView` supports these selectors (the probe used them).
        // `alphaValue` is a CGFloat. `NSWindowBelow` keeps the invisible picker under
        // the web view, so it never receives clicks meant for the page.
        let picker = unsafe {
            let allocated: Allocated<AnyObject> = msg_send![class, alloc];
            let picker: Retained<AnyObject> = msg_send![allocated, initWithFrame: frame];
            let _: () = msg_send![&*picker, setOutputContextID: &*av.context_id];
            let _: () = msg_send![&*picker, setAlphaValue: 0.0f64];
            let _: () = msg_send![
                &*content,
                addSubview: &*picker,
                positioned: WINDOW_BELOW,
                relativeTo: None::<&AnyObject>
            ];
            picker
        };
        pickers.insert(anchor.window_label.clone(), picker.clone());
        Ok(picker)
    })?;

    // SAFETY: `setFrame:` takes an `NSRect`.
    unsafe {
        let _: () = msg_send![&*picker, setFrame: frame];
    }
    // SAFETY: `respondsToSelector:` takes a selector and returns a BOOL.
    let can_present: bool = unsafe {
        msg_send![
            &*picker,
            respondsToSelector: sel!(showRoutePickingControlsForOutputContext:relativeToRect:ofView:)
        ]
    };
    if can_present {
        // SAFETY: The picker responds to this selector. The arguments are an output
        // context, a rect, and a view.
        unsafe {
            let bounds: NSRect = msg_send![&*picker, bounds];
            let _: () = msg_send![
                &*picker,
                showRoutePickingControlsForOutputContext: &*av.context,
                relativeToRect: bounds,
                ofView: &*picker
            ];
        }
        return Ok(());
    }
    let button =
        find_button(&picker).ok_or_else(|| unavailable("The AirPlay picker has no button."))?;
    // SAFETY: `performClick:` takes a sender object or nil.
    unsafe {
        let _: () = msg_send![&*button, performClick: None::<&AnyObject>];
    }
    Ok(())
}

/// Finds the first `NSButton` in a view's subview tree.
fn find_button(view: &AnyObject) -> Option<Retained<AnyObject>> {
    let button_class = AnyClass::get(c"NSButton")?;
    // SAFETY: `subviews` returns an NSArray of views. Index access stays below `count`.
    // `isKindOfClass:` takes a class.
    unsafe {
        let subviews: Option<Retained<AnyObject>> = msg_send![view, subviews];
        let subviews = subviews?;
        let count: usize = msg_send![&*subviews, count];
        let mut children = Vec::with_capacity(count);
        for index in 0..count {
            let child: Option<Retained<AnyObject>> = msg_send![&*subviews, objectAtIndex: index];
            let Some(child) = child else { continue };
            let is_button: bool = msg_send![&*child, isKindOfClass: button_class];
            if is_button {
                return Some(child);
            }
            children.push(child);
        }
        children.iter().find_map(|child| find_button(child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_format_accepts_the_tap_rates() {
        assert!(prepare_format(48_000.0).is_ok());
        assert!(prepare_format(44_100.0).is_ok());
    }

    #[test]
    fn prepare_format_failure_is_a_route_error() {
        // Core Media rejects a zero sample rate. `Engine::start` returns this error
        // before it starts a thread, and `MacAirPlayRoute::start` then drops the tap.
        // Running those two needs a live tap, so they are not covered here.
        let error = prepare_format(0.0).err();
        assert!(matches!(error, Some(RouteError::Unavailable(_))));
    }
}
