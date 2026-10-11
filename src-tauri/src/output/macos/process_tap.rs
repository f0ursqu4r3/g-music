//! Core Audio process tap on the mpv process. The tap mutes mpv locally and
//! delivers its mixdown to an IOProc, which pushes the samples into the ring.

use std::ffi::{CStr, c_void};
use std::ptr::{self, NonNull};
use std::sync::Arc;

use objc2::msg_send;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2_core_audio::{
    AudioDeviceCreateIOProcID, AudioDeviceDestroyIOProcID, AudioDeviceIOProcID, AudioDeviceStart,
    AudioDeviceStop, AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap,
    AudioHardwareDestroyAggregateDevice, AudioHardwareDestroyProcessTap,
    AudioObjectGetPropertyData, AudioObjectID, AudioObjectPropertyAddress, CATapDescription,
    kAudioAggregateDeviceIsPrivateKey, kAudioAggregateDeviceNameKey,
    kAudioAggregateDeviceTapAutoStartKey, kAudioAggregateDeviceTapListKey,
    kAudioAggregateDeviceUIDKey, kAudioHardwarePropertyTranslatePIDToProcessObject,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
    kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey, kAudioTapPropertyFormat,
};
use objc2_core_audio_types::{
    AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp, kAudioFormatFlagIsFloat,
    kAudioFormatFlagIsNonInterleaved, kAudioFormatLinearPCM,
};
use objc2_core_foundation::CFDictionary;
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSUUID};

use crate::output::controller::RouteError;
use crate::output::ring::SampleRing;

/// `CATapMutedWhenTapped`: the tapped process is silent on the local device.
const MUTED_WHEN_TAPPED: isize = 2;
const TAP_NAME: &str = "G Music AirPlay";

pub(super) struct ProcessTap {
    tap_id: AudioObjectID,
    aggregate_id: AudioObjectID,
    proc_id: AudioDeviceIOProcID,
    started: bool,
    /// From `Arc::into_raw`. Released once in `Drop`, after the IOProc is gone.
    ring_ptr: *const SampleRing,
    sample_rate: f64,
}

// SAFETY: The handle holds only Core Audio object IDs, a function pointer, and a
// raw `Arc<SampleRing>` pointer. `SampleRing` is `Send + Sync`.
unsafe impl Send for ProcessTap {}

fn unavailable(message: impl Into<String>) -> RouteError {
    RouteError::Unavailable(message.into())
}

fn main_address(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn process_object_for(pid: u32) -> Result<AudioObjectID, RouteError> {
    let not_ready = || unavailable("mpv is not producing audio yet.");
    let pid = i32::try_from(pid).map_err(|_| not_ready())?;
    let address = main_address(kAudioHardwarePropertyTranslatePIDToProcessObject);
    let mut object: AudioObjectID = 0;
    let mut size = size_of::<AudioObjectID>() as u32;
    // SAFETY: Every pointer refers to a live local. The qualifier is a 4-byte pid and
    // the output buffer is a 4-byte object ID, as the property requires.
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject as AudioObjectID,
            NonNull::from(&address),
            size_of::<i32>() as u32,
            ptr::from_ref(&pid).cast(),
            NonNull::from(&mut size),
            NonNull::from(&mut object).cast(),
        )
    };
    if status != 0 || object == 0 {
        return Err(not_ready());
    }
    Ok(object)
}

fn tap_format(tap_id: AudioObjectID) -> Option<AudioStreamBasicDescription> {
    let address = main_address(kAudioTapPropertyFormat);
    let mut format = AudioStreamBasicDescription {
        mSampleRate: 0.0,
        mFormatID: 0,
        mFormatFlags: 0,
        mBytesPerPacket: 0,
        mFramesPerPacket: 0,
        mBytesPerFrame: 0,
        mChannelsPerFrame: 0,
        mBitsPerChannel: 0,
        mReserved: 0,
    };
    let mut size = size_of::<AudioStreamBasicDescription>() as u32;
    // SAFETY: The output buffer is a live `AudioStreamBasicDescription` and `size`
    // holds its byte length.
    let status = unsafe {
        AudioObjectGetPropertyData(
            tap_id,
            NonNull::from(&address),
            0,
            ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut format).cast(),
        )
    };
    (status == 0).then_some(format)
}

fn is_supported_format(format: &AudioStreamBasicDescription) -> bool {
    let rate = format.mSampleRate;
    (rate == 48_000.0 || rate == 44_100.0)
        && format.mFormatID == kAudioFormatLinearPCM
        && format.mFormatFlags & kAudioFormatFlagIsFloat != 0
        && format.mFormatFlags & kAudioFormatFlagIsNonInterleaved == 0
        && format.mChannelsPerFrame == 2
        && format.mBitsPerChannel == 32
}

/// Creates the tap description. Returns the object and its UUID string.
fn describe_tap(
    process_object: AudioObjectID,
) -> Result<(Retained<AnyObject>, String), RouteError> {
    let class = AnyClass::get(c"CATapDescription")
        .ok_or_else(|| unavailable("Process taps need macOS 14.2 or later."))?;
    let processes = NSArray::from_retained_slice(&[NSNumber::new_u32(process_object)]);
    let name = NSString::from_str(TAP_NAME);
    // SAFETY: `CATapDescription` has these selectors since macOS 14.2. The argument
    // types match: an NSArray of NSNumber, an NSInteger, a BOOL, and an NSString.
    let (description, uuid) = unsafe {
        let allocated: Allocated<AnyObject> = msg_send![class, alloc];
        let description: Option<Retained<AnyObject>> =
            msg_send![allocated, initStereoMixdownOfProcesses: &*processes];
        let description =
            description.ok_or_else(|| unavailable("The audio tap could not be described."))?;
        let _: () = msg_send![&*description, setMuteBehavior: MUTED_WHEN_TAPPED];
        let _: () = msg_send![&*description, setPrivate: true];
        let _: () = msg_send![&*description, setName: &*name];
        let uuid: Option<Retained<NSUUID>> = msg_send![&*description, UUID];
        (description, uuid)
    };
    let uuid = uuid.ok_or_else(|| unavailable("The audio tap has no identifier."))?;
    Ok((description, uuid.UUIDString().to_string()))
}

fn bool_object(value: bool) -> Retained<AnyObject> {
    // SAFETY: `NSNumber` is an `NSObject`; the cast only widens the static type.
    unsafe { Retained::cast_unchecked(NSNumber::new_bool(value)) }
}

fn string_object(value: &str) -> Retained<AnyObject> {
    // SAFETY: `NSString` is an `NSObject`; the cast only widens the static type.
    unsafe { Retained::cast_unchecked(NSString::from_str(value)) }
}

fn key(constant: &CStr) -> Retained<NSString> {
    NSString::from_str(&constant.to_string_lossy())
}

fn aggregate_description(tap_uuid: &str) -> Retained<NSDictionary<NSString, AnyObject>> {
    let sub_tap_uid_key = key(kAudioSubTapUIDKey);
    let sub_tap_drift_key = key(kAudioSubTapDriftCompensationKey);
    let sub_tap = NSDictionary::from_retained_objects(
        &[&*sub_tap_uid_key, &*sub_tap_drift_key],
        &[string_object(tap_uuid), bool_object(true)],
    );
    // SAFETY: `NSDictionary` is an `NSObject`; the cast only widens the static type.
    let sub_tap: Retained<AnyObject> = unsafe { Retained::cast_unchecked(sub_tap) };
    // SAFETY: `NSArray` is an `NSObject`; the cast only widens the static type.
    let tap_list: Retained<AnyObject> =
        unsafe { Retained::cast_unchecked(NSArray::from_retained_slice(&[sub_tap])) };
    let uid_key = key(kAudioAggregateDeviceUIDKey);
    let name_key = key(kAudioAggregateDeviceNameKey);
    let private_key = key(kAudioAggregateDeviceIsPrivateKey);
    let auto_start_key = key(kAudioAggregateDeviceTapAutoStartKey);
    let tap_list_key = key(kAudioAggregateDeviceTapListKey);
    NSDictionary::from_retained_objects(
        &[
            &*uid_key,
            &*name_key,
            &*private_key,
            &*auto_start_key,
            &*tap_list_key,
        ],
        &[
            string_object(&NSUUID::new().UUIDString().to_string()),
            string_object(TAP_NAME),
            bool_object(true),
            bool_object(true),
            tap_list,
        ],
    )
}

/// Real-time callback. It must not allocate, lock (beyond `try_lock` in `push`), or log.
///
/// # Safety
///
/// Core Audio calls this with valid pointers. `client_data` is null or the ring pointer
/// that `ProcessTap::start` registered.
unsafe extern "C-unwind" fn io_proc(
    _device: AudioObjectID,
    _now: NonNull<AudioTimeStamp>,
    input: NonNull<AudioBufferList>,
    _input_time: NonNull<AudioTimeStamp>,
    _output: NonNull<AudioBufferList>,
    _output_time: NonNull<AudioTimeStamp>,
    client_data: *mut c_void,
) -> i32 {
    if client_data.is_null() {
        return 0;
    }
    // SAFETY: Core Audio passes a valid list for the duration of the callback.
    let list = unsafe { input.as_ref() };
    if list.mNumberBuffers == 0 {
        return 0;
    }
    let buffer = list.mBuffers[0];
    if buffer.mData.is_null() {
        return 0;
    }
    let count = buffer.mDataByteSize as usize / size_of::<f32>();
    // SAFETY: The buffer holds `mDataByteSize` bytes of interleaved f32 samples (the
    // format was checked at tap creation) and stays valid during the callback. `mData`
    // is aligned for float data.
    let samples = unsafe { std::slice::from_raw_parts(buffer.mData.cast::<f32>(), count) };
    // SAFETY: `client_data` is the `Arc::into_raw` pointer owned by the `ProcessTap`.
    // `Drop` destroys the IOProc, which waits for running callbacks, before it
    // releases the `Arc`.
    let ring = unsafe { &*client_data.cast::<SampleRing>() };
    ring.push(samples);
    0
}

impl ProcessTap {
    pub(super) fn start(mpv_pid: u32, ring: Arc<SampleRing>) -> Result<Self, RouteError> {
        let process_object = process_object_for(mpv_pid)?;
        let (description, tap_uuid) = describe_tap(process_object)?;

        // From here, `Drop` cleans up whatever this value holds, on any error path.
        let mut tap = Self {
            tap_id: 0,
            aggregate_id: 0,
            proc_id: None,
            started: false,
            ring_ptr: Arc::into_raw(ring),
            sample_rate: 0.0,
        };

        let mut tap_id: AudioObjectID = 0;
        // SAFETY: `description` is a live `CATapDescription` (created from that class).
        // The cast changes only the static type. `tap_id` is a live local.
        let status = unsafe {
            let typed = &*Retained::as_ptr(&description).cast::<CATapDescription>();
            AudioHardwareCreateProcessTap(Some(typed), &mut tap_id)
        };
        if status != 0 {
            return Err(RouteError::PermissionDenied(format!("tap status {status}")));
        }
        tap.tap_id = tap_id;

        let format = tap_format(tap_id)
            .filter(is_supported_format)
            .ok_or_else(|| unavailable("The audio tap uses an unsupported format."))?;
        tap.sample_rate = format.mSampleRate;

        let aggregate = aggregate_description(&tap_uuid);
        let mut aggregate_id: AudioObjectID = 0;
        // SAFETY: `NSDictionary` is toll-free bridged to `CFDictionary`. `aggregate_id`
        // is a live local.
        let status = unsafe {
            let dictionary = &*Retained::as_ptr(&aggregate).cast::<CFDictionary>();
            AudioHardwareCreateAggregateDevice(dictionary, NonNull::from(&mut aggregate_id))
        };
        if status != 0 || aggregate_id == 0 {
            return Err(unavailable(format!("aggregate device status {status}")));
        }
        tap.aggregate_id = aggregate_id;

        let mut proc_id: AudioDeviceIOProcID = None;
        // SAFETY: `io_proc` has the `AudioDeviceIOProc` signature. The client data is
        // the ring pointer, which outlives the IOProc (see `Drop`).
        let status = unsafe {
            AudioDeviceCreateIOProcID(
                aggregate_id,
                Some(io_proc),
                tap.ring_ptr.cast_mut().cast(),
                NonNull::from(&mut proc_id),
            )
        };
        if status != 0 || proc_id.is_none() {
            return Err(unavailable(format!("IOProc status {status}")));
        }
        tap.proc_id = proc_id;

        // SAFETY: The device and IOProc ID come from the calls above.
        let status = unsafe { AudioDeviceStart(aggregate_id, proc_id) };
        if status != 0 {
            return Err(unavailable(format!("start status {status}")));
        }
        tap.started = true;
        Ok(tap)
    }

    pub(super) fn sample_rate(&self) -> f64 {
        self.sample_rate
    }
}

impl Drop for ProcessTap {
    fn drop(&mut self) {
        // Stop, destroy the IOProc, destroy the aggregate, destroy the tap. Destroying
        // the tap restores local sound.
        // SAFETY: Each ID was created by this value and is destroyed once. Failures
        // leave nothing further to clean up, so statuses are ignored. `ring_ptr` came
        // from `Arc::into_raw` in `start` and is released once, after the IOProc is gone.
        unsafe {
            if self.started {
                AudioDeviceStop(self.aggregate_id, self.proc_id);
            }
            if self.proc_id.is_some() {
                AudioDeviceDestroyIOProcID(self.aggregate_id, self.proc_id);
            }
            if self.aggregate_id != 0 {
                AudioHardwareDestroyAggregateDevice(self.aggregate_id);
            }
            if self.tap_id != 0 {
                AudioHardwareDestroyProcessTap(self.tap_id);
            }
            drop(Arc::from_raw(self.ring_ptr));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(rate: f64, flags: u32, channels: u32, bits: u32) -> AudioStreamBasicDescription {
        AudioStreamBasicDescription {
            mSampleRate: rate,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: flags,
            mBytesPerPacket: channels * bits / 8,
            mFramesPerPacket: 1,
            mBytesPerFrame: channels * bits / 8,
            mChannelsPerFrame: channels,
            mBitsPerChannel: bits,
            mReserved: 0,
        }
    }

    #[test]
    fn accepts_only_interleaved_float32_stereo_at_48_or_44_1_khz() {
        let float = kAudioFormatFlagIsFloat;
        assert!(is_supported_format(&format(48_000.0, float, 2, 32)));
        assert!(is_supported_format(&format(44_100.0, float, 2, 32)));
        assert!(!is_supported_format(&format(96_000.0, float, 2, 32)));
        assert!(!is_supported_format(&format(48_000.0, 0, 2, 32)));
        assert!(!is_supported_format(&format(48_000.0, float, 1, 32)));
        assert!(!is_supported_format(&format(48_000.0, float, 2, 16)));
        let planar = float | kAudioFormatFlagIsNonInterleaved;
        assert!(!is_supported_format(&format(48_000.0, planar, 2, 32)));
    }

    #[test]
    fn a_process_without_audio_is_not_ready() {
        let error = process_object_for(u32::MAX - 1).unwrap_err();
        assert_eq!(
            error,
            RouteError::Unavailable("mpv is not producing audio yet.".into())
        );
    }

    #[test]
    fn tap_description_has_an_identifier_and_builds_the_aggregate_dictionary() {
        // The class is missing before macOS 14.2.
        let Ok((_description, uuid)) = describe_tap(1) else {
            return;
        };
        assert!(!uuid.is_empty());
        assert_eq!(aggregate_description(&uuid).count(), 5);
    }
}
