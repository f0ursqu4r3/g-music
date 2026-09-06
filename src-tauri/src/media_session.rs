#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeAction {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    Seek(u64),
    Stop,
    Sleep,
    Wake,
}

#[cfg(target_os = "macos")]
mod platform {
    use std::{
        sync::{
            OnceLock,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, Instant},
    };

    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition,
        PlatformConfig,
    };
    use tauri::AppHandle;

    use super::NativeAction;
    use crate::{
        commands,
        playback::{PlaybackStatus, PlaybackTransport},
    };

    const SYNC_INTERVAL: Duration = Duration::from_secs(1);
    const POSITION_REFRESH_INTERVAL: Duration = Duration::from_secs(15);
    const POSITION_DRIFT_TOLERANCE_MS: u64 = 2_000;

    static SESSION: OnceLock<mpsc::Sender<WorkerMessage>> = OnceLock::new();
    static STARTED: AtomicBool = AtomicBool::new(false);

    #[derive(Clone, Debug)]
    enum WorkerMessage {
        Remote(NativeAction),
        Sleep,
        Wake,
        Shutdown,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MetadataFingerprint {
        id: String,
        title: String,
        artist: String,
        album: Option<String>,
        cover_url: Option<String>,
        duration_ms: u64,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(super) struct TransportView {
        metadata: Option<MetadataFingerprint>,
        status: PlaybackStatus,
        position_ms: u64,
    }

    impl From<&PlaybackTransport> for TransportView {
        fn from(transport: &PlaybackTransport) -> Self {
            Self {
                metadata: transport
                    .current_item
                    .as_ref()
                    .map(|item| MetadataFingerprint {
                        id: item.id.clone(),
                        title: item.title.clone(),
                        artist: item.artist.clone(),
                        album: item.album.clone(),
                        cover_url: item.thumbnail_url.clone(),
                        duration_ms: item.duration_ms,
                    }),
                status: transport.status,
                position_ms: transport.position_ms,
            }
        }
    }

    #[cfg(test)]
    impl TransportView {
        pub(super) fn for_test(
            id: Option<&str>,
            status: PlaybackStatus,
            position_ms: u64,
            duration_ms: u64,
        ) -> Self {
            Self {
                metadata: id.map(|id| MetadataFingerprint {
                    id: id.into(),
                    title: "Title".into(),
                    artist: "Artist".into(),
                    album: None,
                    cover_url: None,
                    duration_ms,
                }),
                status,
                position_ms,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(super) struct SyncDecision {
        pub(super) metadata: bool,
        pub(super) playback: bool,
    }

    #[derive(Default)]
    pub(super) struct PublishedState {
        view: Option<TransportView>,
        published_at: Option<Instant>,
    }

    impl PublishedState {
        pub(super) fn decision(
            &self,
            current: &TransportView,
            now: Instant,
            force: bool,
        ) -> SyncDecision {
            let Some(previous) = self.view.as_ref() else {
                return SyncDecision {
                    metadata: true,
                    playback: true,
                };
            };

            let metadata = force || previous.metadata != current.metadata;
            let status_changed = previous.status != current.status;
            let elapsed = self
                .published_at
                .and_then(|published_at| now.checked_duration_since(published_at))
                .unwrap_or_default();
            let expected_position_ms = if previous.status == PlaybackStatus::Playing {
                previous
                    .position_ms
                    .saturating_add(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            } else {
                previous.position_ms
            };
            let position_changed = if current.status == PlaybackStatus::Playing {
                current.position_ms.abs_diff(expected_position_ms) > POSITION_DRIFT_TOLERANCE_MS
                    || elapsed >= POSITION_REFRESH_INTERVAL
            } else {
                current.position_ms != previous.position_ms
            };

            SyncDecision {
                metadata,
                playback: force || metadata || status_changed || position_changed,
            }
        }

        fn record(&mut self, view: TransportView, now: Instant) {
            self.view = Some(view);
            self.published_at = Some(now);
        }

        #[cfg(test)]
        pub(super) fn from_test(view: &TransportView, published_at: Instant) -> Self {
            Self {
                view: Some(view.clone()),
                published_at: Some(published_at),
            }
        }
    }

    pub(super) fn map_media_event(event: MediaControlEvent) -> Option<NativeAction> {
        match event {
            MediaControlEvent::Play => Some(NativeAction::Play),
            MediaControlEvent::Pause => Some(NativeAction::Pause),
            MediaControlEvent::Toggle => Some(NativeAction::Toggle),
            MediaControlEvent::Next => Some(NativeAction::Next),
            MediaControlEvent::Previous => Some(NativeAction::Previous),
            MediaControlEvent::Stop => Some(NativeAction::Stop),
            MediaControlEvent::SetPosition(MediaPosition(position)) => Some(NativeAction::Seek(
                u64::try_from(position.as_millis()).unwrap_or(u64::MAX),
            )),
            _ => None,
        }
    }

    pub(super) fn start(app: &AppHandle) -> Result<(), String> {
        if STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Ok(());
        }

        let result = start_once(app);
        if result.is_err() {
            STARTED.store(false, Ordering::Release);
        }
        result
    }

    fn start_once(app: &AppHandle) -> Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        let callback_sender = sender.clone();
        let mut controls = MediaControls::new(PlatformConfig {
            dbus_name: "com.gmusic.player",
            display_name: "G Music",
            hwnd: None,
        })
        .map_err(|error| format!("could not create macOS media controls: {error}"))?;
        controls
            .attach(move |event| {
                if let Some(action) = map_media_event(event) {
                    let _ = callback_sender.send(WorkerMessage::Remote(action));
                }
            })
            .map_err(|error| format!("could not attach macOS media controls: {error}"))?;

        power::install(app.clone(), sender.clone())?;
        let worker_app = app.clone();
        if let Err(error) = thread::Builder::new()
            .name("native-media-session".into())
            .spawn(move || run_worker(worker_app, controls, receiver))
        {
            power::uninstall();
            return Err(format!("could not start native media worker: {error}"));
        }

        SESSION
            .set(sender)
            .map_err(|_| "native media session was already started".to_string())
    }

    pub(super) fn stop(_app: &AppHandle) {
        power::uninstall();
        if let Some(sender) = SESSION.get() {
            let _ = sender.send(WorkerMessage::Shutdown);
        }
    }

    fn run_worker(
        app: AppHandle,
        mut controls: MediaControls,
        receiver: mpsc::Receiver<WorkerMessage>,
    ) {
        let mut published = PublishedState::default();
        let mut force_sync = true;

        loop {
            if let Err(error) = sync_native(&app, &mut controls, &mut published, force_sync) {
                tracing::error!(%error, "could not synchronize native media state");
            }
            force_sync = false;

            match receiver.recv_timeout(SYNC_INTERVAL) {
                Ok(WorkerMessage::Remote(action)) => {
                    if let Err(error) = commands::native_transport(&app, action) {
                        tracing::error!(%error.message, "native media command failed");
                    }
                }
                Ok(WorkerMessage::Sleep) => {
                    if let Err(error) = commands::native_transport(&app, NativeAction::Sleep) {
                        tracing::error!(%error.message, "could not pause playback before sleep");
                    }
                }
                Ok(WorkerMessage::Wake) => {
                    if let Err(error) = commands::native_transport(&app, NativeAction::Wake) {
                        tracing::error!(%error.message, "could not keep playback paused after wake");
                    }
                    force_sync = true;
                }
                Ok(WorkerMessage::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn sync_native(
        app: &AppHandle,
        controls: &mut MediaControls,
        published: &mut PublishedState,
        force: bool,
    ) -> Result<(), String> {
        let transport = commands::cached_transport(app).map_err(|error| error.message)?;
        let view = TransportView::from(&transport);
        let now = Instant::now();
        let decision = published.decision(&view, now, force);

        if decision.metadata {
            let item = transport.current_item.as_ref();
            controls
                .set_metadata(MediaMetadata {
                    title: item.map(|item| item.title.as_str()),
                    artist: item.map(|item| item.artist.as_str()),
                    album: item.and_then(|item| item.album.as_deref()),
                    cover_url: item.and_then(|item| item.thumbnail_url.as_deref()),
                    duration: item.map(|item| Duration::from_millis(item.duration_ms)),
                })
                .map_err(|error| format!("could not set native media metadata: {error}"))?;
        }

        if decision.playback {
            let progress = Some(MediaPosition(Duration::from_millis(transport.position_ms)));
            let playback = match transport.current_item {
                None => MediaPlayback::Stopped,
                Some(_) if transport.status == PlaybackStatus::Playing => {
                    MediaPlayback::Playing { progress }
                }
                Some(_) => MediaPlayback::Paused { progress },
            };
            controls
                .set_playback(playback)
                .map_err(|error| format!("could not set native playback state: {error}"))?;
        }

        if decision.metadata || decision.playback {
            published.record(view, now);
        }
        Ok(())
    }

    mod power {
        use std::{cell::RefCell, ptr::NonNull, sync::mpsc};

        use block2::RcBlock;
        use objc2::{MainThreadMarker, rc::Retained, runtime::ProtocolObject};
        use objc2_app_kit::{
            NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
        };
        use objc2_foundation::{
            NSNotification, NSNotificationCenter, NSObjectProtocol, NSOperationQueue,
        };

        use super::WorkerMessage;
        use crate::commands;
        use tauri::AppHandle;

        type ObserverToken = Retained<ProtocolObject<dyn NSObjectProtocol>>;

        thread_local! {
            static OBSERVERS: RefCell<Option<PowerObservers>> = const { RefCell::new(None) };
        }

        struct PowerObservers {
            center: Retained<NSNotificationCenter>,
            tokens: Vec<ObserverToken>,
        }

        impl Drop for PowerObservers {
            fn drop(&mut self) {
                for token in &self.tokens {
                    unsafe { self.center.removeObserver(token.as_ref()) };
                }
            }
        }

        pub(super) fn install(
            app: AppHandle,
            sender: mpsc::Sender<WorkerMessage>,
        ) -> Result<(), String> {
            MainThreadMarker::new().ok_or_else(|| {
                "macOS power observers must be installed on the main thread".to_string()
            })?;

            let workspace = NSWorkspace::sharedWorkspace();
            let center = workspace.notificationCenter();
            let queue = NSOperationQueue::mainQueue();
            let mut tokens = Vec::with_capacity(2);

            tokens.push(add_observer(
                &center,
                &workspace,
                &queue,
                unsafe { NSWorkspaceWillSleepNotification },
                app.clone(),
                sender.clone(),
                WorkerMessage::Sleep,
            ));
            tokens.push(add_observer(
                &center,
                &workspace,
                &queue,
                unsafe { NSWorkspaceDidWakeNotification },
                app,
                sender,
                WorkerMessage::Wake,
            ));

            OBSERVERS.with_borrow_mut(|observers| {
                *observers = Some(PowerObservers { center, tokens });
            });
            Ok(())
        }

        fn add_observer(
            center: &NSNotificationCenter,
            workspace: &NSWorkspace,
            queue: &NSOperationQueue,
            name: &objc2_foundation::NSNotificationName,
            app: AppHandle,
            sender: mpsc::Sender<WorkerMessage>,
            message: WorkerMessage,
        ) -> ObserverToken {
            let block: RcBlock<dyn Fn(NonNull<NSNotification>)> = RcBlock::new(move |_| {
                commands::native_pause_hint(&app);
                let _ = sender.send(message.clone());
            });
            unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(name),
                    Some(workspace.as_ref()),
                    Some(queue),
                    &block,
                )
            }
        }

        pub(super) fn uninstall() {
            OBSERVERS.with_borrow_mut(|observers| {
                observers.take();
            });
        }
    }
}

#[cfg(target_os = "macos")]
pub fn start(app: &tauri::AppHandle) -> Result<(), String> {
    platform::start(app)
}

#[cfg(target_os = "macos")]
pub fn stop(app: &tauri::AppHandle) {
    platform::stop(app);
}

#[cfg(not(target_os = "macos"))]
pub fn start(_app: &tauri::AppHandle) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn stop(_app: &tauri::AppHandle) {}

#[cfg(all(test, target_os = "macos"))]
use platform::{PublishedState, SyncDecision, TransportView, map_media_event};

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use std::time::{Duration, Instant};

    use souvlaki::{MediaControlEvent, MediaPosition};

    use super::{NativeAction, PublishedState, SyncDecision, TransportView, map_media_event};
    use crate::playback::PlaybackStatus;

    #[test]
    fn maps_native_transport_events_to_authoritative_actions() {
        let cases = [
            (MediaControlEvent::Play, Some(NativeAction::Play)),
            (MediaControlEvent::Pause, Some(NativeAction::Pause)),
            (MediaControlEvent::Toggle, Some(NativeAction::Toggle)),
            (MediaControlEvent::Next, Some(NativeAction::Next)),
            (MediaControlEvent::Previous, Some(NativeAction::Previous)),
            (MediaControlEvent::Stop, Some(NativeAction::Stop)),
            (
                MediaControlEvent::SetPosition(MediaPosition(Duration::from_millis(4_250))),
                Some(NativeAction::Seek(4_250)),
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(map_media_event(event), expected);
        }
    }

    #[test]
    fn ignores_native_events_that_this_player_does_not_support() {
        assert_eq!(map_media_event(MediaControlEvent::SetVolume(0.5)), None);
    }

    #[test]
    fn first_transport_state_publishes_metadata_and_playback() {
        let now = Instant::now();
        let state = PublishedState::default();
        let view =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Playing, 10_000, 180_000);

        assert_eq!(
            state.decision(&view, now, false),
            SyncDecision {
                metadata: true,
                playback: true,
            }
        );
    }

    #[test]
    fn normal_playing_progress_does_not_publish_each_poll() {
        let published_at = Instant::now();
        let view =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Playing, 10_000, 180_000);
        let state = PublishedState::from_test(&view, published_at);
        let next =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Playing, 11_000, 180_000);

        assert_eq!(
            state.decision(&next, published_at + Duration::from_secs(1), false),
            SyncDecision {
                metadata: false,
                playback: false,
            }
        );
    }

    #[test]
    fn seek_or_track_change_publishes_the_changed_native_state() {
        let published_at = Instant::now();
        let original =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Playing, 10_000, 180_000);
        let state = PublishedState::from_test(&original, published_at);
        let seeked =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Playing, 45_000, 180_000);
        let changed_track =
            TransportView::for_test(Some("track-2"), PlaybackStatus::Playing, 0, 240_000);

        assert_eq!(
            state.decision(&seeked, published_at + Duration::from_secs(1), false),
            SyncDecision {
                metadata: false,
                playback: true,
            }
        );
        assert_eq!(
            state.decision(&changed_track, published_at + Duration::from_secs(1), false),
            SyncDecision {
                metadata: true,
                playback: true,
            }
        );
    }

    #[test]
    fn wake_forces_a_complete_native_refresh() {
        let published_at = Instant::now();
        let view =
            TransportView::for_test(Some("track-1"), PlaybackStatus::Paused, 10_000, 180_000);
        let state = PublishedState::from_test(&view, published_at);

        assert_eq!(
            state.decision(&view, published_at, true),
            SyncDecision {
                metadata: true,
                playback: true,
            }
        );
    }
}
