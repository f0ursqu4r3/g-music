# Playback devices and AirPlay output

Status: Approved. Revised on 2026-10-11: the AirPlay audio path uses a Core Audio process tap instead of an mpv PCM FIFO.

## Outcome

Let the listener choose where G Music plays audio without leaving the app:

- Any local output that mpv detects: built-in speakers, displays, USB DACs, Bluetooth, and HDMI. macOS and Linux.
- AirPlay receivers, including HomePod and AirPlay 2 speakers. macOS only.

Keep mpv as the only decoder. Do not open a second YouTube stream. Do not save audio files.

## Out of scope

- Chromecast, DLNA, and other network protocols.
- Multi-room playback to several AirPlay receivers at the same time.
- Automatic reconnection to an AirPlay receiver after launch.
- A custom in-app list of AirPlay receivers. macOS does not permit it without an Apple-restricted entitlement.
- Exact track-change timing on AirPlay. See [Known limitations](#known-limitations).

## Evidence from the temporary probe

A throwaway Objective-C probe ran on macOS 26.6.2 (25G83) on 2026-10-10. The listener confirmed audio on a HomePod mini.

- `+[AVOutputContext addSharedAudioOutputContext]` returns an audio-type output context for an unentitled process.
- `AVRoutePickerView -setOutputContextID:` binds the system AirPlay popover to that context. The popover listed the Mac, two HomePods, and an Apple TV.
- `+[AVOutputContext outputContext]` returns a video-type context. Its popover hides audio-only receivers. Do not use it.
- `AVSampleBufferAudioRenderer -setOutputContext:` plays enqueued PCM to the selected receiver through `AVSampleBufferRenderSynchronizer`.
- Every route change posts `AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification`. Without a stop, flush, re-enqueue, and restart, the receiver stays silent.
- `AVOutputDeviceDiscoverySession` returns no devices, and `+sharedSystemAudioContext` returns nil. Both need the restricted `com.apple.avfoundation.allows-access-to-device-list` entitlement. Programmatic route selection fails for the same reason.
- `mpv --audio-device=help` lists CoreAudio and AVFoundation entries for each local device. It does not list AirPlay receivers until macOS routes system output to one.

These APIs are private. G Music ships with a Developer ID signature and notarization, not through the App Store. A future macOS release can still remove or change them. Resolve every private class and selector at runtime, and hide AirPlay when any check fails.

The [airlift](https://github.com/jgarms/airlift) project (MIT) uses the same context, picker, and renderer approach. It captures app audio with a Core Audio process tap.

### Rejected: mpv PCM output through a FIFO

A second probe on 2026-10-11 with mpv 0.41.0 tested `set_property ao pcm` during playback, with `ao-pcm-file` set to a FIFO and a reader paced at real time.

- The runtime switch works. `current-ao` changes to `pcm`, the FIFO receives float32 stereo 48 kHz PCM, and switching back works.
- `ao=pcm` writes from the mpv core thread. FIFO back-pressure blocks IPC: `set_property pause true` took 2.96 to 3.24 s, and `seek` plus the next `get_property` took about 1.7 s. `--audio-buffer=0.05` did not help.
- `time-pos` ran about 1.4 s ahead of the consumed audio.

The existing UI sends IPC on every pause and seek, so this path would freeze controls. Do not use it.

## User experience

### Output menu

Add one output button beside every volume control: Artwork window, mini player, mini window, queue window, and library playback footer. The button opens a Reka UI dropdown:

1. One item for each local output. A check mark shows the active output in Local mode.
2. A separator.
3. **AirPlay…** (macOS only, hidden when the private API checks fail). A check mark and the receiver names show the active route in AirPlay mode.

The button icon shows the mode: a speaker icon for Local mode and the AirPlay icon for AirPlay mode. Its accessible name includes the active output name.

**AirPlay…** opens the system AirPlay popover, anchored to the output button.

### Command palette

- **Output: \<device name\>** for each local output.
- **AirPlay…** on macOS. The palette closes, then G Music opens the popover anchored to the center of the focused window.

### Persistence

Save the selected local device ID in `SavedPlaybackState`. Restore it on launch. If the device is gone, use mpv's `auto` device and keep the saved ID, so the device is used again when it returns.

Every launch starts in Local mode. AirPlay selection does not persist.

### Disconnect

When the AirPlay route disappears unexpectedly or the renderer fails:

1. Pause playback.
2. Return to Local mode with the saved device.
3. Show the notice "AirPlay speaker disconnected".

## Architecture

### Mode rule

G Music state alone decides the mode: `Local { device_id }` or `AirPlay { receiver_names }`. The private context can stay routed to a receiver after G Music leaves AirPlay mode, because unentitled code cannot reset the route. G Music then stops sending audio, and the receiver goes idle.

### Rust units (`src-tauri/src/output/`)

| Unit | Purpose | Depends on |
|---|---|---|
| `mod.rs`: `OutputState`, `OutputController` | Owns the mode. Applies changes, emits `output-changed`, and saves the local device ID. | mpv IPC, `AirPlayRoute` |
| `airplay_clock.rs` | Pure math: position correction, queue depth, and the drift check. No platform code. | — |
| `devices.rs` | Parses mpv `audio-device-list` into `OutputDevice { id, name }`. Removes `auto` and the duplicate `avfoundation/*` entries on macOS. Observes the property for hot-plug changes. | mpv IPC |
| `airplay.rs` (macOS) | Private-API layer. Creates the shared audio context, renderer, and synchronizer. Opens the picker. Handles auto-flush notifications. Reports route changes. Implements `AirPlayRoute` together with `process_tap.rs`. | `objc2`, `objc2-av-foundation`, `objc2-av-kit`, `objc2-core-media` |
| `process_tap.rs` (macOS) | Creates a muted Core Audio process tap on the mpv PID, wraps it in a private aggregate device, and delivers float32 PCM from an IOProc to the renderer through a bounded ring buffer. Destroys the tap and aggregate device on stop, which restores local sound. | `objc2-core-audio` |
| `AirPlayRoute` trait | Interface between `OutputController` and the native layer. A fake implementation serves tests. A no-op implementation on Linux reports "unavailable". | — |

Use typed `objc2` bindings where they exist, as `media_session.rs` does. Use `msg_send!` only for the private selectors. Run all AppKit and AVKit calls on the main thread.

### Changes to existing code

- `playback/youtube.rs`: replace `--audio-device=auto` with the saved device. Add an IPC helper that sets `audio-device` at runtime. Expose the mpv PID and a restart notification, so the tap follows a new mpv process. In AirPlay mode, correct the reported position. See [Position](#position). mpv output options do not change between modes.
- `commands.rs`, `lib.rs`, `src/api.ts`: add `list_output_devices`, `select_output_device`, `open_airplay_picker`, and `get_output_state`, and the `output-changed` event.
- `persistence.rs`: add `output_device_id: Option<String>` to `SavedPlaybackState` with `#[serde(default)]`.
- Visualizer: in AirPlay mode, add the queued audio duration to the synchronization delay.
- `capabilities/*.json`: allow the new commands and event in every window that shows the output button.
- Bundle Info.plist: add `NSAudioCaptureUsageDescription` with the text "G Music captures its own audio to play it on AirPlay speakers."

### Frontend units

- `src/components/OutputMenu.vue`: the button and dropdown.
- `src/composables/useOutput.ts`: shared output state, updated from `output-changed`, with actions for selection and the picker.
- `src/lib/command-palette.ts`: the output entries.

## Data flow

### Select a local device in Local mode

1. The UI calls `select_output_device(id)`.
2. Rust sends `set_property audio-device <id>` to mpv. Playback continues without a restart.
3. Rust saves the ID and emits `output-changed`.

### Enter AirPlay mode

1. The UI calls `open_airplay_picker` with the window label and the button rectangle in CSS pixels.
2. On the main thread, Rust adds a hidden `AVRoutePickerView` to that window's content view and opens the popover at the rectangle. Create the shared context once per app run.
3. When the popover closes with a non-local route, Rust:
   1. creates the muted process tap on the mpv PID and starts its IOProc;
   2. starts the renderer, at rate 1 when playing and rate 0 when paused;
   3. emits `output-changed`.
4. When the popover closes with the local Mac route, G Music stays in or returns to Local mode with the saved device.

mpv keeps playing to the saved local device at real-time speed. The tap mutes that device for mpv only. Other apps still play locally.

### Leave AirPlay mode

When the listener selects a local device in the G Music menu:

1. Set the synchronizer rate to 0 and flush the renderer.
2. Destroy the process tap and its aggregate device. mpv sound returns to the local device.
3. Set `audio-device` on mpv when the selected device differs from the current one.
4. Save the device and emit `output-changed`.

### Events in AirPlay mode

| Event | Action |
|---|---|
| Pause or play | Set mpv `pause` and the synchronizer rate together. Queued audio is held, so resume is immediate. |
| Seek | Seek in mpv and flush the renderer. Discard tapped PCM until mpv sends `playback-restart`. Restart timestamps at the synchronizer's current time. |
| Clock drift | Every second, compare the queue depth with the depth measured 3 s after start. When the difference passes 250 ms, set rate 0, flush, and restart from the next tapped buffer. |
| mpv restarts | Destroy the tap, create a new tap on the new PID, flush, and restart the renderer. |
| Auto-flush notification | Set rate 0, flush, re-enqueue from the current time, and restore the previous rate. |
| Volume | The G Music slider sets mpv volume, as in Local mode. Changes reach the receiver after the AirPlay delay. The system popover slider sets receiver volume. |
| Route lost or renderer failed | Follow [Disconnect](#disconnect). |
| Quit | Stop the renderer and destroy the tap. |

### Position

In AirPlay mode, report `time-pos − queued`, where `queued` is the last enqueued timestamp minus the synchronizer time. Clamp the result at zero. mpv runs at real time in both modes, so this matches the audio at the receiver. The visualizer uses the same `queued` value as extra delay.

## Known limitations

- At a track change in AirPlay mode, title and artwork switch about 2 seconds before the new track reaches the receiver. mpv starts the next file while the end of the previous file is still queued. A later change can add a queue boundary marker.
- The listener selects the AirPlay receiver again after each launch.
- Volume changes on AirPlay take effect after the AirPlay delay.

## Error handling

- A private class or selector is missing: hide **AirPlay…**, log one diagnostic line, and keep local output working.
- macOS is older than 14.2: hide **AirPlay…**.
- The listener denies the audio capture permission, or tap creation fails: stay in Local mode and emit `playback-error` with code `airplay_permission_denied`. The message tells the listener to allow G Music under System Settings → Privacy & Security → Screen & System Audio Recording.
- The saved local device is missing at launch or disappears during playback: use `auto`, keep the saved ID, and update the menu.
- `set_property audio-device` fails: keep the previous device and emit `playback-error` with code `output_device_unavailable`.
- The renderer or the aggregate device cannot start: stay in Local mode and emit `playback-error` with code `airplay_unavailable`.
- Commands from a window without the capability are rejected by Tauri, as for other commands.

## Testing

### Automated

- `devices.rs`: parse real `audio-device-list` samples from macOS and Linux, filter `auto` and duplicates, and detect hot-plug changes.
- `OutputController` with the fake `AirPlayRoute` and the existing fake mpv socket: local selection, entering and leaving AirPlay, pause, seek, disconnect, permission denial, mpv restart, and persistence.
- `airplay_clock.rs`: queued duration, clamping, seek reset, and the drift threshold.
- `persistence.rs`: old saved state without `output_device_id` still loads.
- `OutputMenu.vue`, `useOutput.ts`, and command palette tests: rendering, check marks, AirPlay item visibility by platform, and event updates.
- `window-capabilities.test.ts`: new commands are allowed only where the button appears.

### Process tap evidence

A throwaway probe ran on 2026-10-11 against a local mpv 0.41.0 playing a 440 Hz tone. It used `initStereoMixdownOfProcesses:` with `CATapMutedWhenTapped` and `privateTap = YES`, and a private aggregate device with `kAudioAggregateDeviceTapAutoStartKey` and drift compensation.

- The tap format was 48 kHz, 2 channels, float32 (`flags=0x9`).
- The IOProc received about 48,000 frames per second with the expected peak level (0.042 measured, 0.043 expected).
- The listener heard the tone, then silence while tapped, then the tone again after the tap was destroyed.
- No permission prompt appeared, because macOS attributed the probe to an app that already had capture permission. Smoke test step 8 covers the G Music prompt.

### Manual native smoke test

Add these steps to `docs/release-readiness.md`:

1. Switch between two local outputs during playback.
2. Select an AirPlay receiver and confirm audio, pause, resume, seek, and position.
3. Change the receiver in the popover during playback and confirm audio continues.
4. Power off the receiver and confirm pause, the notice, and return to the local output.
5. Select a local output and confirm the receiver goes silent.
6. Confirm the visualizer stays in sync in AirPlay mode.
7. Play to AirPlay for 30 minutes and confirm no audible drift between the UI position and the receiver.
8. On a fresh user account, confirm the audio capture permission prompt and the denial message.

## References

- [AVRoutePickerView documentation](https://developer.apple.com/documentation/avkit/avroutepickerview)
- [airlift](https://github.com/jgarms/airlift)
- [Core Audio taps for capturing process audio](https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps)
