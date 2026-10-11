# Playback Devices and AirPlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the listener pick a local audio output or an AirPlay receiver from inside G Music.

**Architecture:** Local outputs come from mpv's `audio-device-list` and switch with `set_property audio-device`. AirPlay uses a private audio-type `AVOutputContext`, the system `AVRoutePickerView` popover, and an `AVSampleBufferAudioRenderer`. A muted Core Audio process tap on the mpv PID supplies the PCM. A process-global `output` module (same pattern as `crate::visualizer`) receives hooks from `MpvPlayer` and owns the AirPlay state machine. The frontend adds one self-contained `OutputMenu.vue`.

**Tech Stack:** Rust 2024, Tauri 2, `objc2` 0.6, `objc2-foundation` 0.3, `objc2-core-audio` 0.3, `objc2-core-media` 0.3, `block2` 0.6, Vue 3, Reka UI dropdown (shadcn-vue wrappers in `src/components/ui/dropdown-menu`), Vitest, Bun.

**Spec:** `docs/superpowers/specs/2026-10-10-playback-devices-design.md`

## Global Constraints

- mpv stays the only decoder. No second YouTube stream. No audio files on disk.
- AirPlay is macOS only. Hide it when `CATapDescription` or any private class or selector is missing (macOS older than 14.2 has no `CATapDescription`).
- Use `+[AVOutputContext addSharedAudioOutputContext]`. Never use `+outputContext` (video type; hides HomePods).
- Never call `AVOutputDeviceDiscoverySession`, `+sharedSystemAudioContext`, or `setOutputDevice:`. They need an Apple-only entitlement.
- mpv output options (`ao`, `audio-format`, sample rate) never change between modes.
- Every launch starts in Local mode. Only the local device ID persists (`SavedPlaybackState.output_device_id`).
- Error codes: `output_device_unavailable`, `airplay_permission_denied`, `airplay_unavailable`, and `airplay_disconnected` (the disconnect notice, sent on the existing `playback-error` event).
- Disconnect notice text: `AirPlay speaker disconnected`.
- Permission text (Info.plist `NSAudioCaptureUsageDescription`): `G Music captures its own audio to play it on AirPlay speakers.`
- Drift limit: 250 ms, measured against the queue depth 3 s after start.
- Output commands take `state.transport` before `state.playback`, like `set_volume`.
- No Claude attribution in commits or code comments. Commit messages use Conventional Commits.
- Verification commands: `bun run test`, `bun run lint`, `bun run build`, `bun run format`, and from `src-tauri/`: `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`.

## Review Focus

1. **Track change during a transport call drops the saved device.** `prepare_transport` builds a new provider; if it does not copy `output_device_id`, `commit_transport` saves `None`. Test in Task 2.
2. **Saved device unplugged at launch.** mpv must start on `auto` and switch only when the saved ID is in the list. Test in Task 2.
3. **Picking "This Mac" in the popover is not a disconnect.** No pause, no notice. Test in Task 3.
4. **Pause in AirPlay mode must not overwrite queued audio with silence.** The tap must discard input while paused. Test in Task 3 (`pause_gate_protects_queued_audio`); Task 7 wires `set_playing` to `ring.set_open`.
5. **mpv restarts while in AirPlay mode** (session change, load retry). The tap must move to the new PID. Test in Task 3.

---

## File Structure

| Path | Responsibility |
|---|---|
| `src-tauri/src/output/mod.rs` (create) | Process-global facade: `install`, mpv hooks, state access, effect application (events, pause). |
| `src-tauri/src/output/devices.rs` (create) | Parse mpv `audio-device-list`; resolve the startup device. Pure. |
| `src-tauri/src/output/clock.rs` (create) | Queue depth, position correction, drift monitor. Pure. |
| `src-tauri/src/output/ring.rs` (create) | Bounded f32 sample ring with a playing gate. Pure, thread-safe. |
| `src-tauri/src/output/controller.rs` (create) | `OutputState`, `AirPlayRoute` trait, `OutputController` state machine, `OutputEffect`. Platform-free. |
| `src-tauri/src/output/macos/mod.rs` (create) | `MacAirPlayRoute`: implements `AirPlayRoute` with the two files below. |
| `src-tauri/src/output/macos/process_tap.rs` (create) | Core Audio process tap + private aggregate device + IOProc into `SampleRing`. |
| `src-tauri/src/output/macos/airplay.rs` (create) | Private AVFoundation context, renderer, synchronizer, feeder thread, picker. |
| `src-tauri/src/persistence.rs` (modify) | `output_device_id` field. |
| `src-tauri/src/playback/youtube.rs` (modify) | Provider device methods; `MpvPlayer` preferred device and output hooks. |
| `src-tauri/src/playback/mod.rs` (modify) | Re-export nothing new; no change unless the compiler needs it. |
| `src-tauri/src/visualizer/worker.rs` (modify) | Subtract AirPlay queue depth from `audio-pts`. |
| `src-tauri/src/commands.rs` (modify) | `get_output_state`, `select_output_device`, `open_airplay_picker`. |
| `src-tauri/src/lib.rs` (modify) | `mod output;`, register commands, call `output::install`. |
| `src-tauri/Cargo.toml` (modify) | macOS crates and features. |
| `src-tauri/Info.plist` (create) | `NSAudioCaptureUsageDescription`. |
| `src/api.ts` (modify) | Types and `outputApi`. |
| `src/composables/useOutput.ts` (create) | Output state, actions, `output-changed` subscription. |
| `src/components/OutputMenu.vue` (create) | Button and dropdown. |
| `src/components/ArtworkWindow.vue`, `MiniPlayer.vue`, `QueueWindow.vue`, `library/LibraryPlaybackFooter.vue` (modify) | Place `OutputMenu`. |
| `src/lib/command-palette.ts`, `src/components/LibraryWindow.vue` (modify) | Output palette entries. |
| `README.md`, `docs/release-readiness.md` (modify) | Feature docs and smoke steps. |

Note on spec names: the spec lists `list_output_devices` and `get_output_state`. This plan merges them into `get_output_state`, which refreshes the device list and returns the whole state. The spec's hot-plug requirement is met by refreshing when the menu opens.

---

### Task 1: Device list parsing

**Files:**
- Create: `src-tauri/src/output/mod.rs` (module declarations only in this task)
- Create: `src-tauri/src/output/devices.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod output;` after `mod media_session;`)

**Interfaces:**
- Produces: `crate::output::devices::OutputDevice { id: String, name: String }` (Serialize, camelCase), `parse_device_list(&serde_json::Value) -> Vec<OutputDevice>`, `resolve_startup_device(saved: Option<&str>, devices: &[OutputDevice]) -> Option<String>`.

- [ ] **Step 1: Create the module skeleton**

`src-tauri/src/output/mod.rs`:

```rust
//! Audio output selection: local devices through mpv and AirPlay on macOS.

pub(crate) mod devices;
```

Add `mod output;` to `src-tauri/src/lib.rs` after `mod media_session;`.

- [ ] **Step 2: Write the failing tests**

`src-tauri/src/output/devices.rs`:

```rust
use serde::Serialize;
use serde_json::Value;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_macos_list_without_auto_or_avfoundation_duplicates() {
        let list = json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "coreaudio/BuiltInSpeakerDevice", "description": "MacBook Pro Speakers"},
            {"name": "coreaudio/3669D23D", "description": "MPG321UX OLED"},
            {"name": "avfoundation/BuiltInSpeakerDevice", "description": "MacBook Pro Speakers"}
        ]);
        assert_eq!(
            parse_device_list(&list),
            vec![
                OutputDevice { id: "coreaudio/BuiltInSpeakerDevice".into(), name: "MacBook Pro Speakers".into() },
                OutputDevice { id: "coreaudio/3669D23D".into(), name: "MPG321UX OLED".into() },
            ]
        );
    }

    #[test]
    fn parses_linux_list_and_falls_back_to_id_for_blank_names() {
        let list = json!([
            {"name": "pipewire", "description": "Default (pipewire)"},
            {"name": "pulse/alsa_output.usb-DAC", "description": "  "},
            {"name": "pulse/alsa_output.usb-DAC", "description": "Duplicate"}
        ]);
        assert_eq!(
            parse_device_list(&list),
            vec![
                OutputDevice { id: "pipewire".into(), name: "Default (pipewire)".into() },
                OutputDevice { id: "pulse/alsa_output.usb-DAC".into(), name: "pulse/alsa_output.usb-DAC".into() },
            ]
        );
    }

    #[test]
    fn ignores_malformed_values() {
        assert!(parse_device_list(&json!(true)).is_empty());
        assert!(parse_device_list(&json!([{"description": "No id"}, 4])).is_empty());
    }

    #[test]
    fn resolves_saved_device_only_when_present() {
        let devices = vec![OutputDevice { id: "coreaudio/A".into(), name: "A".into() }];
        assert_eq!(resolve_startup_device(Some("coreaudio/A"), &devices), Some("coreaudio/A".into()));
        assert_eq!(resolve_startup_device(Some("coreaudio/Gone"), &devices), None);
        assert_eq!(resolve_startup_device(None, &devices), None);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run from `src-tauri/`: `cargo test output::devices`
Expected: compile errors for missing `OutputDevice`, `parse_device_list`, `resolve_startup_device`.

- [ ] **Step 4: Implement**

Insert above the test module in `devices.rs`:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutputDevice {
    pub(crate) id: String,
    pub(crate) name: String,
}

/// mpv lists every macOS device twice (CoreAudio and AVFoundation). Keep CoreAudio only.
pub(crate) fn parse_device_list(value: &Value) -> Vec<OutputDevice> {
    let mut devices: Vec<OutputDevice> = Vec::new();
    for entry in value.as_array().into_iter().flatten() {
        let Some(id) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        if id == "auto" || id.starts_with("avfoundation/") || devices.iter().any(|d| d.id == id) {
            continue;
        }
        let name = entry
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(id);
        devices.push(OutputDevice { id: id.into(), name: name.into() });
    }
    devices
}

pub(crate) fn resolve_startup_device(saved: Option<&str>, devices: &[OutputDevice]) -> Option<String> {
    saved
        .filter(|id| devices.iter().any(|device| device.id == *id))
        .map(str::to_owned)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test output::devices`
Expected: 4 passed.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/output src-tauri/src/lib.rs
git commit -m "feat: parse mpv audio output devices"
```

---

### Task 2: Local device selection, persistence, and commands

**Files:**
- Modify: `src-tauri/src/persistence.rs` (struct `SavedPlaybackState`, line ~19)
- Modify: `src-tauri/src/playback/youtube.rs` (`MpvPlayer` ~2566, `ensure_started` ~2724, `send` test branch ~2806, `mpv_arguments` unchanged, provider struct ~191, `with_library` ~365, `prepare_transport` ~942, `saved_playback_state` ~1636, `restore_playback_state` ~1657)
- Modify: `src-tauri/src/output/mod.rs` (temporary state holder; replaced in Task 5)
- Modify: `src-tauri/src/commands.rs` (after `set_volume`, ~1675)
- Modify: `src-tauri/src/lib.rs` (register commands)

**Interfaces:**
- Consumes: `OutputDevice`, `parse_device_list`, `resolve_startup_device` (Task 1).
- Produces:
  - `SavedPlaybackState.output_device_id: Option<String>`.
  - `YouTubePlaybackProvider::output_devices(&mut self) -> Result<(Vec<OutputDevice>, Option<String>), YouTubePlaybackError>` — returns devices and the saved ID.
  - `YouTubePlaybackProvider::select_output_device(&mut self, id: &str) -> Result<(), YouTubePlaybackError>`.
  - Tauri commands `get_output_state() -> OutputState` and `select_output_device(id: String) -> OutputState`.
  - `crate::output::controller::OutputState` is defined in Task 3. In this task, define it in `output/mod.rs` as shown, and Task 3 moves it into `controller.rs` with the same fields.

- [ ] **Step 1: Write the failing persistence test**

Append to the existing `#[cfg(test)]` module in `src-tauri/src/persistence.rs` (create `mod output_device_tests` at the end of the file if there is no suitable module):

```rust
#[cfg(test)]
mod output_device_tests {
    use super::SavedPlaybackState;

    #[test]
    fn old_saved_state_without_output_device_still_loads() {
        let state: SavedPlaybackState = serde_json::from_str(r#"{"volumePercent":40}"#).unwrap();
        assert_eq!(state.output_device_id, None);
        assert_eq!(state.volume_percent, 40);
    }

    #[test]
    fn output_device_round_trips() {
        let state: SavedPlaybackState =
            serde_json::from_str(r#"{"outputDeviceId":"coreaudio/A"}"#).unwrap();
        assert_eq!(state.output_device_id.as_deref(), Some("coreaudio/A"));
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains(r#""outputDeviceId":"coreaudio/A""#));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test output_device_tests`
Expected: compile error, no field `output_device_id`.

- [ ] **Step 3: Add the field**

In `SavedPlaybackState`, after `shuffle_order`:

```rust
    #[serde(default)]
    pub(crate) output_device_id: Option<String>,
```

Fix every struct literal of `SavedPlaybackState` that the compiler reports (for example `saved_playback_state` in `youtube.rs`; Step 7 sets the real value there, use `output_device_id: None` now).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test output_device_tests`
Expected: 2 passed.

- [ ] **Step 5: Write the failing provider tests**

Add a scripted-response hook to `MpvPlayer` for tests. In the struct, after `test_failure`:

```rust
    #[cfg(test)]
    test_responses: std::collections::HashMap<String, Value>,
```

In `MpvPlayer::new()`, after `test_failure: None,`:

```rust
            #[cfg(test)]
            test_responses: std::collections::HashMap::new(),
```

In `send()`, in the `#[cfg(test)]` branch, directly after the `test_failure` check:

```rust
            if let Some(response) = self.test_responses.get(&command.to_string()) {
                return Ok(response.clone());
            }
```

Add these tests to the existing `#[cfg(test)] mod tests` in `youtube.rs`:

```rust
    fn device_list_fixture() -> Value {
        json!([
            {"name": "auto", "description": "Autoselect device"},
            {"name": "coreaudio/BuiltInSpeakerDevice", "description": "MacBook Pro Speakers"},
            {"name": "coreaudio/DAC", "description": "USB DAC"}
        ])
    }

    fn provider_with_device_list() -> (YouTubePlaybackProvider, std::sync::Arc<std::sync::Mutex<Vec<Value>>>) {
        let mut provider = YouTubePlaybackProvider::with_entries(Vec::new(), None);
        let commands = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        provider.player.test_commands = Some(commands.clone());
        provider.player.test_responses.insert(
            json!(["get_property", "audio-device-list"]).to_string(),
            device_list_fixture(),
        );
        (provider, commands)
    }

    #[test]
    fn lists_output_devices_with_saved_selection() {
        let (mut provider, _) = provider_with_device_list();
        provider.output_device_id = Some("coreaudio/DAC".into());
        let (devices, selected) = provider.output_devices().unwrap();
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[1].name, "USB DAC");
        assert_eq!(selected.as_deref(), Some("coreaudio/DAC"));
    }

    #[test]
    fn selecting_a_device_sets_mpv_and_saved_state() {
        let (mut provider, commands) = provider_with_device_list();
        provider.select_output_device("coreaudio/DAC").unwrap();
        assert!(commands
            .lock()
            .unwrap()
            .contains(&json!(["set_property", "audio-device", "coreaudio/DAC"])));
        assert_eq!(provider.saved_playback_state().output_device_id.as_deref(), Some("coreaudio/DAC"));
    }

    #[test]
    fn selecting_a_missing_device_fails_and_keeps_previous() {
        let (mut provider, commands) = provider_with_device_list();
        provider.output_device_id = Some("coreaudio/BuiltInSpeakerDevice".into());
        assert!(provider.select_output_device("coreaudio/Gone").is_err());
        assert_eq!(provider.output_device_id.as_deref(), Some("coreaudio/BuiltInSpeakerDevice"));
        assert!(!commands
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.get(1) == Some(&json!("audio-device"))));
    }

    #[test]
    fn prepared_transport_keeps_the_saved_output_device() {
        let (mut provider, _) = provider_with_device_list();
        provider.output_device_id = Some("coreaudio/DAC".into());
        let prepared = provider.prepare_transport();
        assert_eq!(prepared.saved_playback_state().output_device_id.as_deref(), Some("coreaudio/DAC"));
    }

    #[test]
    fn restored_state_sets_preferred_mpv_device() {
        let mut provider = YouTubePlaybackProvider::with_entries(Vec::new(), None);
        provider.restore_playback_state(crate::persistence::SavedPlaybackState {
            current_item_id: None,
            position_ms: 0,
            volume_percent: 50,
            shuffle_enabled: false,
            repeat_mode: RepeatMode::Off,
            queue_ids: Vec::new(),
            shuffle_order: Vec::new(),
            output_device_id: Some("coreaudio/DAC".into()),
        });
        assert_eq!(provider.player.preferred_audio_device.as_deref(), Some("coreaudio/DAC"));
    }

    #[test]
    fn preferred_device_applies_only_when_present() {
        let (mut provider, commands) = provider_with_device_list();
        provider.player.preferred_audio_device = Some("coreaudio/Gone".into());
        provider.player.apply_preferred_audio_device();
        assert!(!commands.lock().unwrap().iter().any(|c| c.get(1) == Some(&json!("audio-device"))));

        provider.player.preferred_audio_device = Some("coreaudio/DAC".into());
        provider.player.apply_preferred_audio_device();
        assert!(commands
            .lock()
            .unwrap()
            .contains(&json!(["set_property", "audio-device", "coreaudio/DAC"])));
    }
```

If `with_entries` has a different signature, use the constructor the neighboring tests use.

- [ ] **Step 6: Run to verify they fail**

Run: `cargo test output_device`
Expected: compile errors for `output_device_id`, `output_devices`, `select_output_device`, `preferred_audio_device`, `apply_preferred_audio_device`.

- [ ] **Step 7: Implement provider and player changes**

1. `MpvPlayer` struct: add `preferred_audio_device: Option<String>,` and initialize it to `None` in `new()`.
2. Add to `impl MpvPlayer`:

```rust
    /// mpv starts on `auto`. A saved device that is gone stays saved and is not applied.
    fn apply_preferred_audio_device(&mut self) {
        let Some(preferred) = self.preferred_audio_device.clone() else {
            return;
        };
        let result = (|| {
            let list = self.send(json!(["get_property", "audio-device-list"]))?;
            let devices = crate::output::devices::parse_device_list(&list);
            if let Some(id) =
                crate::output::devices::resolve_startup_device(Some(&preferred), &devices)
            {
                self.send(json!(["set_property", "audio-device", id]))?;
            }
            Ok::<(), YouTubePlaybackError>(())
        })();
        if let Err(error) = result {
            tracing::warn!(%error, "could not apply the saved audio output");
        }
    }
```

3. In `ensure_started`, inside the `if self.socket_path.exists()` block, after `visualizer.start(...)` and before `return Ok(());`, add `self.apply_preferred_audio_device();`.
4. Provider struct: add `output_device_id: Option<String>,`; in `with_library` add `output_device_id: None,`.
5. In `prepare_transport`, after `prepared.shuffle_order = ...`, add:

```rust
        prepared.output_device_id = self.output_device_id.clone();
```

Also, in `prepare_transport`, the moved `player` keeps its `preferred_audio_device`, so no change there.
6. In `saved_playback_state`, set `output_device_id: self.output_device_id.clone(),`.
7. In `restore_playback_state`, at the end:

```rust
        self.player.preferred_audio_device = state.output_device_id.clone();
        self.output_device_id = state.output_device_id;
```

8. Add provider methods next to `set_volume`:

```rust
    pub(crate) fn output_devices(
        &mut self,
    ) -> Result<(Vec<crate::output::devices::OutputDevice>, Option<String>), YouTubePlaybackError> {
        self.player.ensure_started(self.snapshot.volume_percent)?;
        let list = self.player.send(json!(["get_property", "audio-device-list"]))?;
        Ok((
            crate::output::devices::parse_device_list(&list),
            self.output_device_id.clone(),
        ))
    }

    pub(crate) fn select_output_device(&mut self, id: &str) -> Result<(), YouTubePlaybackError> {
        let (devices, _) = self.output_devices()?;
        if !devices.iter().any(|device| device.id == id) {
            return Err(YouTubePlaybackError::Player(format!(
                "the audio output is unavailable: {id}"
            )));
        }
        self.player.send(json!(["set_property", "audio-device", id]))?;
        self.player.preferred_audio_device = Some(id.into());
        self.output_device_id = Some(id.into());
        self.persist_playback_state()?;
        tracing::info!(device = id, "audio output selected");
        Ok(())
    }
```

`persist_playback_state` returns `Ok(())` when there is no library path, so tests without a path pass. If it does not, guard the call with `if self.library_path.is_some()`.

- [ ] **Step 8: Run to verify they pass**

Run: `cargo test output_device`
Expected: all new tests pass. Then run `cargo test` and confirm the existing suite still passes.

- [ ] **Step 9: Add the state type and commands**

Add to `src-tauri/src/output/mod.rs`:

```rust
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutputState {
    pub(crate) devices: Vec<devices::OutputDevice>,
    pub(crate) selected_device_id: Option<String>,
    pub(crate) airplay_available: bool,
    /// `Some` means AirPlay mode. The names come from the active route.
    pub(crate) airplay_receivers: Option<Vec<String>>,
}
```

Add to `src-tauri/src/commands.rs` after `set_volume`:

```rust
fn output_state_from(
    state: &AppState,
) -> Result<crate::output::OutputState, CommandError> {
    let (devices, selected_device_id) =
        with_playback(state, "list_output_devices", |playback| playback.output_devices())?;
    Ok(crate::output::local_state(devices, selected_device_id))
}

#[tauri::command]
pub async fn get_output_state(app: AppHandle) -> Result<crate::output::OutputState, CommandError> {
    blocking(app, move |state, _app| {
        let _serial = state.transport.lock().map_err(|_| unavailable())?;
        output_state_from(state)
    })
    .await
}

#[tauri::command]
pub async fn select_output_device(
    app: AppHandle,
    id: String,
) -> Result<crate::output::OutputState, CommandError> {
    blocking(app, move |state, app| {
        let _serial = state.transport.lock().map_err(|_| unavailable())?;
        if state.shutting_down.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        crate::output::leave_airplay();
        with_playback(state, "select_output_device", |playback| {
            playback.select_output_device(&id)
        })
        .map_err(|error| CommandError {
            code: "output_device_unavailable",
            message: error.message,
        })?;
        let output = output_state_from(state)?;
        let _ = app.emit("output-changed", &output);
        Ok(output)
    })
    .await
}
```

Add temporary functions to `output/mod.rs` (Task 5 replaces their bodies; keep the signatures):

```rust
/// Merges the latest local device list into the shared output state.
pub(crate) fn local_state(
    devices: Vec<devices::OutputDevice>,
    selected_device_id: Option<String>,
) -> OutputState {
    OutputState { devices, selected_device_id, ..OutputState::default() }
}

/// Returns to Local mode when AirPlay is active. No-op until Task 5.
pub(crate) fn leave_airplay() {}
```

Register `commands::get_output_state` and `commands::select_output_device` in `tauri::generate_handler!` in `src-tauri/src/lib.rs`, after `commands::set_volume`.

- [ ] **Step 10: Build and lint**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: pass, no warnings.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src
git commit -m "feat: select and persist the local audio output"
```

---

### Task 3: AirPlay clock math, sample ring, and controller state machine

**Files:**
- Create: `src-tauri/src/output/clock.rs`
- Create: `src-tauri/src/output/ring.rs`
- Create: `src-tauri/src/output/controller.rs`
- Modify: `src-tauri/src/output/mod.rs` (declare modules; move `OutputState` into `controller.rs` and `pub(crate) use controller::OutputState;`)

**Interfaces:**
- Consumes: `OutputDevice` (Task 1), `OutputState` fields (Task 2).
- Produces:
  - `clock::queued_ms(enqueued_end_s: f64, synchronizer_s: f64) -> f64`
  - `clock::corrected_position_ms(raw_ms: u64, queued_ms: f64) -> u64`
  - `clock::DriftMonitor::{new(start_s: f64) -> Self, reset(&mut self, now_s: f64), observe(&mut self, now_s: f64, queued_ms: f64) -> bool}`; consts `SETTLE_S = 3.0`, `LIMIT_MS = 250.0`.
  - `ring::SampleRing::{new(capacity_samples: usize) -> Self, set_open(&self, bool), push(&self, &[f32]) -> usize, pop_into(&self, &mut Vec<f32>, max: usize) -> usize, clear(&self), len(&self) -> usize}`
  - `controller::{OutputState, PickerAnchor, RouteError, RoutePoll, AirPlayRoute, OutputEffect, OutputController}` exactly as in Step 7.

- [ ] **Step 1: Write failing clock tests**

`src-tauri/src/output/clock.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_is_enqueued_minus_played_and_never_negative() {
        assert_eq!(queued_ms(12.5, 10.0), 2500.0);
        assert_eq!(queued_ms(10.0, 10.2), 0.0);
    }

    #[test]
    fn corrected_position_subtracts_queue_and_clamps() {
        assert_eq!(corrected_position_ms(60_000, 2_000.4), 58_000);
        assert_eq!(corrected_position_ms(1_000, 2_000.0), 0);
    }

    #[test]
    fn drift_monitor_waits_for_settle_then_flags_large_changes() {
        let mut monitor = DriftMonitor::new(100.0);
        assert!(!monitor.observe(101.0, 2_000.0)); // still settling
        assert!(!monitor.observe(103.0, 2_000.0)); // baseline taken at 3 s
        assert!(!monitor.observe(200.0, 2_249.0));
        assert!(monitor.observe(300.0, 2_251.0));
        assert!(monitor.observe(301.0, 1_749.0));
    }

    #[test]
    fn reset_restarts_the_settle_period() {
        let mut monitor = DriftMonitor::new(0.0);
        monitor.observe(3.0, 2_000.0);
        monitor.reset(10.0);
        assert!(!monitor.observe(11.0, 5_000.0));
        assert!(!monitor.observe(13.0, 1_000.0));
        assert!(!monitor.observe(14.0, 1_100.0));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test output::clock` (after adding `pub(crate) mod clock;` to `output/mod.rs`)
Expected: compile errors for missing items.

- [ ] **Step 3: Implement clock**

Above the tests in `clock.rs`:

```rust
//! Timing math for AirPlay output. All times are seconds unless the name says `_ms`.

pub(crate) fn queued_ms(enqueued_end_s: f64, synchronizer_s: f64) -> f64 {
    ((enqueued_end_s - synchronizer_s) * 1000.0).max(0.0)
}

pub(crate) fn corrected_position_ms(raw_ms: u64, queued_ms: f64) -> u64 {
    raw_ms.saturating_sub(queued_ms.round() as u64)
}

/// The tap follows the local device clock; the renderer follows the AirPlay clock.
/// A queue that grows or shrinks past the limit means the clocks drifted apart.
pub(crate) struct DriftMonitor {
    started_s: f64,
    baseline_ms: Option<f64>,
}

impl DriftMonitor {
    pub(crate) const SETTLE_S: f64 = 3.0;
    pub(crate) const LIMIT_MS: f64 = 250.0;

    pub(crate) fn new(start_s: f64) -> Self {
        Self { started_s: start_s, baseline_ms: None }
    }

    pub(crate) fn reset(&mut self, now_s: f64) {
        self.started_s = now_s;
        self.baseline_ms = None;
    }

    pub(crate) fn observe(&mut self, now_s: f64, queued_ms: f64) -> bool {
        match self.baseline_ms {
            None if now_s - self.started_s >= Self::SETTLE_S => {
                self.baseline_ms = Some(queued_ms);
                false
            }
            None => false,
            Some(baseline) => (queued_ms - baseline).abs() > Self::LIMIT_MS,
        }
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test output::clock`
Expected: 4 passed.

- [ ] **Step 5: Write failing ring tests, then implement**

`src-tauri/src/output/ring.rs`:

```rust
//! Sample buffer between the Core Audio IOProc thread and the renderer feeder.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct SampleRing {
    samples: Mutex<VecDeque<f32>>,
    capacity: usize,
    open: AtomicBool,
}

impl SampleRing {
    pub(crate) fn new(capacity_samples: usize) -> Self {
        Self {
            samples: Mutex::new(VecDeque::with_capacity(capacity_samples)),
            capacity: capacity_samples,
            open: AtomicBool::new(false),
        }
    }

    /// A closed ring drops input. Close it while paused, so silence cannot
    /// push queued music out.
    pub(crate) fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::Release);
    }

    /// Called from the real-time thread: never blocks. Returns samples kept.
    pub(crate) fn push(&self, input: &[f32]) -> usize {
        if !self.open.load(Ordering::Acquire) {
            return 0;
        }
        let Ok(mut samples) = self.samples.try_lock() else {
            return 0;
        };
        let overflow = (samples.len() + input.len()).saturating_sub(self.capacity);
        samples.drain(..overflow.min(samples.len()));
        let kept = input.len().min(self.capacity);
        samples.extend(&input[input.len() - kept..]);
        kept
    }

    pub(crate) fn pop_into(&self, output: &mut Vec<f32>, max: usize) -> usize {
        let mut samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        let count = max.min(samples.len());
        output.extend(samples.drain(..count));
        count
    }

    pub(crate) fn clear(&self) {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    pub(crate) fn len(&self) -> usize {
        self.samples.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_ring_drops_input() {
        let ring = SampleRing::new(8);
        assert_eq!(ring.push(&[1.0, 2.0]), 0);
        assert_eq!(ring.len(), 0);
    }

    #[test]
    fn open_ring_keeps_newest_samples_when_full() {
        let ring = SampleRing::new(4);
        ring.set_open(true);
        ring.push(&[1.0, 2.0, 3.0]);
        ring.push(&[4.0, 5.0]);
        let mut out = Vec::new();
        assert_eq!(ring.pop_into(&mut out, 10), 4);
        assert_eq!(out, [2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn oversized_input_keeps_its_tail() {
        let ring = SampleRing::new(2);
        ring.set_open(true);
        assert_eq!(ring.push(&[1.0, 2.0, 3.0]), 2);
        let mut out = Vec::new();
        ring.pop_into(&mut out, 2);
        assert_eq!(out, [2.0, 3.0]);
    }

    #[test]
    fn pause_gate_protects_queued_audio() {
        let ring = SampleRing::new(4);
        ring.set_open(true);
        ring.push(&[0.5, 0.5]);
        ring.set_open(false);
        ring.push(&[0.0, 0.0, 0.0, 0.0]);
        let mut out = Vec::new();
        ring.pop_into(&mut out, 4);
        assert_eq!(out, [0.5, 0.5]);
    }
}
```

Run: `cargo test output::ring` (after `pub(crate) mod ring;`)
Expected: 4 passed. (Write the tests first, run, see the compile failure, then add the implementation; the full file is shown together here for reference.)

- [ ] **Step 6: Write failing controller tests**

`src-tauri/src/output/controller.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[derive(Default)]
    struct FakeLog {
        calls: Vec<String>,
        receivers: Vec<String>,
        failed: bool,
        start_error: Option<RouteError>,
        queued_ms: f64,
    }

    struct FakeRoute(Arc<Mutex<FakeLog>>);

    impl AirPlayRoute for FakeRoute {
        fn available(&self) -> bool { true }
        fn open_picker(&mut self, anchor: &PickerAnchor) -> Result<(), RouteError> {
            self.0.lock().unwrap().calls.push(format!("open_picker:{}", anchor.window_label));
            Ok(())
        }
        fn start(&mut self, mpv_pid: u32, playing: bool) -> Result<(), RouteError> {
            let mut log = self.0.lock().unwrap();
            log.calls.push(format!("start:{mpv_pid}:{playing}"));
            match log.start_error.take() { Some(e) => Err(e), None => Ok(()) }
        }
        fn stop(&mut self) { self.0.lock().unwrap().calls.push("stop".into()); }
        fn set_playing(&mut self, playing: bool) {
            self.0.lock().unwrap().calls.push(format!("playing:{playing}"));
        }
        fn flush(&mut self) { self.0.lock().unwrap().calls.push("flush".into()); }
        fn queued_ms(&self) -> f64 { self.0.lock().unwrap().queued_ms }
        fn poll(&mut self) -> RoutePoll {
            let log = self.0.lock().unwrap();
            RoutePoll { receivers: log.receivers.clone(), failed: log.failed }
        }
    }

    fn controller() -> (OutputController, Arc<Mutex<FakeLog>>) {
        let log = Arc::new(Mutex::new(FakeLog::default()));
        (OutputController::new(Box::new(FakeRoute(log.clone()))), log)
    }

    fn anchor() -> PickerAnchor {
        PickerAnchor { window_label: "artwork".into(), x: 10.0, y: 20.0, width: 28.0, height: 28.0 }
    }

    fn state_change(effects: &[OutputEffect]) -> Option<&OutputState> {
        effects.iter().find_map(|e| match e { OutputEffect::StateChanged(s) => Some(s), _ => None })
    }

    #[test]
    fn picking_a_receiver_enters_airplay() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.set_playing(true);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        let effects = c.tick(Instant::now());
        assert_eq!(state_change(&effects).unwrap().airplay_receivers, Some(vec!["Master Bedroom".into()]));
        assert!(log.lock().unwrap().calls.contains(&"start:42:true".into()));
    }

    #[test]
    fn leftover_route_without_picker_does_not_enter_airplay() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        assert!(c.tick(Instant::now()).is_empty());
        assert_eq!(c.state().airplay_receivers, None);
    }

    #[test]
    fn receiver_picked_before_mpv_runs_starts_when_mpv_starts() {
        let (mut c, log) = controller();
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Family Room".into()];
        assert!(c.tick(Instant::now()).is_empty());
        let effects = c.mpv_started(7);
        assert!(state_change(&effects).unwrap().airplay_receivers.is_some());
        assert!(log.lock().unwrap().calls.contains(&"start:7:false".into()));
    }

    #[test]
    fn permission_denial_stays_local_and_reports_code() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        log.lock().unwrap().start_error = Some(RouteError::PermissionDenied("denied".into()));
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        let effects = c.tick(Instant::now());
        assert!(effects.iter().any(|e| matches!(e, OutputEffect::Error { code: "airplay_permission_denied", .. })));
        assert_eq!(c.state().airplay_receivers, None);
    }

    #[test]
    fn choosing_this_mac_in_the_popover_is_not_a_disconnect() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers.clear();
        let effects = c.tick(Instant::now());
        assert!(!effects.iter().any(|e| matches!(e, OutputEffect::PausePlayback | OutputEffect::Notice(_))));
        assert_eq!(c.state().airplay_receivers, None);
        assert!(log.lock().unwrap().calls.contains(&"stop".into()));
    }

    #[test]
    fn unexpected_route_loss_pauses_and_notifies() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        log.lock().unwrap().receivers.clear();
        let effects = c.tick(Instant::now() + Duration::from_secs(1));
        assert!(effects.iter().any(|e| matches!(e, OutputEffect::PausePlayback)));
        assert!(effects.iter().any(|e| matches!(e, OutputEffect::Notice(m) if m == "AirPlay speaker disconnected")));
        assert_eq!(c.state().airplay_receivers, None);
    }

    #[test]
    fn renderer_failure_is_a_disconnect() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        log.lock().unwrap().failed = true;
        let effects = c.tick(Instant::now());
        assert!(effects.iter().any(|e| matches!(e, OutputEffect::PausePlayback)));
    }

    #[test]
    fn mpv_restart_moves_the_tap_to_the_new_pid() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        c.mpv_stopped();
        c.mpv_started(43);
        let calls = log.lock().unwrap().calls.clone();
        assert!(calls.contains(&"start:43:false".into()));
        assert_eq!(c.state().airplay_receivers, Some(vec!["Master Bedroom".into()]));
    }

    #[test]
    fn pause_seek_and_position_reach_the_route_only_in_airplay() {
        let (mut c, log) = controller();
        c.set_playing(true);
        c.seeked();
        assert!(log.lock().unwrap().calls.is_empty());
        assert_eq!(c.corrected_position_ms(5_000), 5_000);

        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        log.lock().unwrap().queued_ms = 2_000.0;
        c.set_playing(false);
        c.seeked();
        let calls = log.lock().unwrap().calls.clone();
        assert!(calls.contains(&"playing:false".into()));
        assert!(calls.contains(&"flush".into()));
        assert_eq!(c.corrected_position_ms(5_000), 3_000);
        assert_eq!(c.queued_ms(), 2_000.0);
    }

    #[test]
    fn leave_airplay_stops_the_route() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        let effects = c.leave_airplay();
        assert_eq!(state_change(&effects).unwrap().airplay_receivers, None);
        assert!(log.lock().unwrap().calls.contains(&"stop".into()));
        assert!(c.leave_airplay().is_empty());
    }

    #[test]
    fn switching_receivers_updates_names_without_restart() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        c.tick(Instant::now());
        log.lock().unwrap().receivers = vec!["Family Room".into()];
        let effects = c.tick(Instant::now());
        assert_eq!(state_change(&effects).unwrap().airplay_receivers, Some(vec!["Family Room".into()]));
        assert_eq!(log.lock().unwrap().calls.iter().filter(|c| c.starts_with("start")).count(), 1);
    }
}
```

- [ ] **Step 7: Run to verify failure, then implement the controller**

Run: `cargo test output::controller` → compile errors.

Implementation (above the tests). Move `OutputState` here from `mod.rs`:

```rust
//! Output mode state machine. Platform code lives behind `AirPlayRoute`.

use serde::{Deserialize, Serialize};
use std::time::Instant;

use super::devices::OutputDevice;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutputState {
    pub(crate) devices: Vec<OutputDevice>,
    pub(crate) selected_device_id: Option<String>,
    pub(crate) airplay_available: bool,
    /// `Some` means AirPlay mode. The names come from the active route.
    pub(crate) airplay_receivers: Option<Vec<String>>,
}

/// A rectangle in CSS pixels from the top-left corner of the window's web content.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PickerAnchor {
    #[serde(default)]
    pub(crate) window_label: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RouteError {
    PermissionDenied(String),
    Unavailable(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RoutePoll {
    /// Empty means the route is this Mac.
    pub(crate) receivers: Vec<String>,
    pub(crate) failed: bool,
}

pub(crate) trait AirPlayRoute: Send {
    fn available(&self) -> bool;
    fn open_picker(&mut self, anchor: &PickerAnchor) -> Result<(), RouteError>;
    /// Taps `mpv_pid`, mutes it locally, and starts sending its audio to the route.
    fn start(&mut self, mpv_pid: u32, playing: bool) -> Result<(), RouteError>;
    /// Stops sending audio and removes the tap, which restores local sound.
    fn stop(&mut self);
    fn set_playing(&mut self, playing: bool);
    /// Drops queued audio after a seek.
    fn flush(&mut self);
    fn queued_ms(&self) -> f64;
    fn poll(&mut self) -> RoutePoll;
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum OutputEffect {
    StateChanged(OutputState),
    Error { code: &'static str, message: String },
    PausePlayback,
    Notice(String),
}

pub(crate) struct OutputController {
    route: Box<dyn AirPlayRoute>,
    state: OutputState,
    mpv_pid: Option<u32>,
    playing: bool,
    /// Set when the picker opens. A route change while set is the listener's choice.
    picker_open: bool,
    /// AirPlay was chosen but mpv was not running yet (or mpv restarted).
    pending_start: bool,
    /// Receiver names to show once the pending start succeeds.
    pending_receivers: Vec<String>,
    /// The route runs (tap and renderer active).
    route_running: bool,
}

impl OutputController {
    pub(crate) fn new(route: Box<dyn AirPlayRoute>) -> Self {
        let airplay_available = route.available();
        Self {
            route,
            state: OutputState { airplay_available, ..OutputState::default() },
            mpv_pid: None,
            playing: false,
            picker_open: false,
            pending_start: false,
            pending_receivers: Vec::new(),
            route_running: false,
        }
    }

    pub(crate) fn state(&self) -> OutputState {
        self.state.clone()
    }

    fn in_airplay(&self) -> bool {
        self.state.airplay_receivers.is_some()
    }

    pub(crate) fn set_local_devices(
        &mut self,
        devices: Vec<OutputDevice>,
        selected_device_id: Option<String>,
    ) -> OutputState {
        self.state.devices = devices;
        self.state.selected_device_id = selected_device_id;
        self.state()
    }

    pub(crate) fn open_picker(&mut self, anchor: &PickerAnchor) -> Result<(), RouteError> {
        if !self.route.available() {
            return Err(RouteError::Unavailable("AirPlay is unavailable on this Mac.".into()));
        }
        self.route.open_picker(anchor)?;
        self.picker_open = true;
        Ok(())
    }

    fn start_route(&mut self, receivers: Vec<String>) -> Vec<OutputEffect> {
        let Some(pid) = self.mpv_pid else {
            self.pending_start = true;
            self.state.airplay_receivers = None;
            return Vec::new();
        };
        match self.route.start(pid, self.playing) {
            Ok(()) => {
                self.pending_start = false;
                self.route_running = true;
                self.state.airplay_receivers = Some(receivers);
                vec![OutputEffect::StateChanged(self.state())]
            }
            Err(error) => {
                self.pending_start = false;
                self.route_running = false;
                self.state.airplay_receivers = None;
                let (code, message) = match error {
                    RouteError::PermissionDenied(_) => (
                        "airplay_permission_denied",
                        "Allow G Music under System Settings → Privacy & Security → Screen & System Audio Recording, then choose the AirPlay speaker again.".to_string(),
                    ),
                    RouteError::Unavailable(detail) => ("airplay_unavailable", detail),
                };
                vec![OutputEffect::Error { code, message }, OutputEffect::StateChanged(self.state())]
            }
        }
    }

    fn stop_route(&mut self) {
        if self.route_running {
            self.route.stop();
            self.route_running = false;
        }
        self.state.airplay_receivers = None;
        self.pending_start = false;
    }

    pub(crate) fn tick(&mut self, _now: Instant) -> Vec<OutputEffect> {
        if !self.route.available() {
            return Vec::new();
        }
        let poll = self.route.poll();
        if !self.in_airplay() {
            if self.picker_open && !poll.receivers.is_empty() {
                self.picker_open = false;
                if self.mpv_pid.is_none() {
                    self.pending_start = true;
                    self.pending_receivers = poll.receivers;
                    return Vec::new();
                }
                return self.start_route(poll.receivers);
            }
            return Vec::new();
        }
        if poll.failed || poll.receivers.is_empty() {
            let chosen = self.picker_open && !poll.failed;
            self.picker_open = false;
            self.stop_route();
            let mut effects = Vec::new();
            if !chosen {
                effects.push(OutputEffect::PausePlayback);
                effects.push(OutputEffect::Notice("AirPlay speaker disconnected".into()));
            }
            effects.push(OutputEffect::StateChanged(self.state()));
            return effects;
        }
        if self.state.airplay_receivers.as_ref() != Some(&poll.receivers) {
            self.state.airplay_receivers = Some(poll.receivers);
            return vec![OutputEffect::StateChanged(self.state())];
        }
        Vec::new()
    }

    pub(crate) fn leave_airplay(&mut self) -> Vec<OutputEffect> {
        if !self.in_airplay() && !self.pending_start {
            return Vec::new();
        }
        self.picker_open = false;
        self.stop_route();
        vec![OutputEffect::StateChanged(self.state())]
    }

    pub(crate) fn mpv_started(&mut self, pid: u32) -> Vec<OutputEffect> {
        self.mpv_pid = Some(pid);
        if self.pending_start {
            let receivers = std::mem::take(&mut self.pending_receivers);
            return self.start_route(receivers);
        }
        Vec::new()
    }

    pub(crate) fn mpv_stopped(&mut self) {
        self.mpv_pid = None;
        if self.in_airplay() {
            if self.route_running {
                self.route.stop();
                self.route_running = false;
            }
            self.pending_start = true;
            self.pending_receivers = self.state.airplay_receivers.clone().unwrap_or_default();
        }
    }

    pub(crate) fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
        if self.route_running {
            self.route.set_playing(playing);
        }
    }

    pub(crate) fn seeked(&mut self) {
        if self.route_running {
            self.route.flush();
        }
    }

    pub(crate) fn queued_ms(&self) -> f64 {
        if self.route_running { self.route.queued_ms() } else { 0.0 }
    }

    pub(crate) fn corrected_position_ms(&self, raw_ms: u64) -> u64 {
        super::clock::corrected_position_ms(raw_ms, self.queued_ms())
    }
}
```

Behavior notes the tests depend on:

- After `mpv_stopped` in AirPlay mode, `airplay_receivers` stays `Some`, so the UI keeps showing AirPlay while mpv restarts. `mpv_started` then calls `start_route` with the stored receivers (`mpv_restart_moves_the_tap_to_the_new_pid`).
- A receiver picked before mpv runs produces no effects from `tick`; the state shows AirPlay only after `mpv_started` succeeds (`receiver_picked_before_mpv_runs_starts_when_mpv_starts`).

Update `output/mod.rs`:

```rust
pub(crate) mod clock;
pub(crate) mod controller;
pub(crate) mod devices;
pub(crate) mod ring;

pub(crate) use controller::OutputState;
```

Remove the old `OutputState` definition from `mod.rs`; keep `local_state` and `leave_airplay`.

- [ ] **Step 8: Run to verify pass**

Run: `cargo test output::`
Expected: all clock, ring, controller, and device tests pass.

- [ ] **Step 9: Lint and commit**

```bash
cargo clippy --all-targets --all-features -- -D warnings
git add src-tauri/src/output
git commit -m "feat: add AirPlay output state machine"
```

---

### Task 4: Global output facade and mpv hooks

**Files:**
- Modify: `src-tauri/src/output/mod.rs`
- Modify: `src-tauri/src/playback/youtube.rs` (`set_paused` ~2670, `seek` ~2684, `inspect` ~2699, `ensure_started` ~2724, `stop` ~3046)
- Modify: `src-tauri/src/visualizer/worker.rs` (~250)
- Modify: `src-tauri/src/lib.rs` (setup, ~30)

**Interfaces:**
- Consumes: `OutputController`, `OutputEffect`, `AirPlayRoute`, `PickerAnchor`, `RouteError` (Task 3).
- Produces (all `pub(crate)` in `crate::output`):
  - `install(app: tauri::AppHandle, route: Box<dyn AirPlayRoute>)` — starts a 250 ms tick thread.
  - `mpv_started(pid: u32)`, `mpv_stopped()`, `set_playing(playing: bool)`, `seeked()`.
  - `corrected_position_ms(raw_ms: u64) -> u64`, `queued_ms() -> f64`.
  - `local_state(devices, selected) -> OutputState`, `leave_airplay()`, `open_picker(anchor: PickerAnchor) -> Result<(), RouteError>`.
  - `NoAirPlay` struct implementing `AirPlayRoute` with `available() == false` (used on Linux and in tests).
  - Every function is a no-op (or identity) before `install`.

- [ ] **Step 1: Write failing facade tests**

Add to `output/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_are_inert_before_install() {
        mpv_started(1);
        set_playing(true);
        seeked();
        mpv_stopped();
        leave_airplay();
        assert_eq!(corrected_position_ms(5_000), 5_000);
        assert_eq!(queued_ms(), 0.0);
        assert!(!local_state(Vec::new(), None).airplay_available);
    }

    #[test]
    fn no_airplay_route_is_unavailable() {
        let mut route = NoAirPlay;
        assert!(!route.available());
        assert!(route.poll().receivers.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test output::tests`
Expected: compile errors.

- [ ] **Step 3: Implement the facade**

Replace the temporary `local_state` and `leave_airplay` in `output/mod.rs` with:

```rust
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use controller::{AirPlayRoute, OutputController, OutputEffect, PickerAnchor, RouteError, RoutePoll};

struct Shared {
    app: AppHandle,
    controller: Mutex<OutputController>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
const TICK: Duration = Duration::from_millis(250);

pub(crate) struct NoAirPlay;

impl AirPlayRoute for NoAirPlay {
    fn available(&self) -> bool { false }
    fn open_picker(&mut self, _anchor: &PickerAnchor) -> Result<(), RouteError> {
        Err(RouteError::Unavailable("AirPlay is unavailable on this system.".into()))
    }
    fn start(&mut self, _mpv_pid: u32, _playing: bool) -> Result<(), RouteError> {
        Err(RouteError::Unavailable("AirPlay is unavailable on this system.".into()))
    }
    fn stop(&mut self) {}
    fn set_playing(&mut self, _playing: bool) {}
    fn flush(&mut self) {}
    fn queued_ms(&self) -> f64 { 0.0 }
    fn poll(&mut self) -> RoutePoll { RoutePoll::default() }
}

pub(crate) fn install(app: AppHandle, route: Box<dyn AirPlayRoute>) {
    let shared = Shared { app, controller: Mutex::new(OutputController::new(route)) };
    if SHARED.set(shared).is_err() {
        return;
    }
    thread::Builder::new()
        .name("gmusic-output".into())
        .spawn(|| loop {
            thread::sleep(TICK);
            let effects = with_controller(|c| c.tick(Instant::now())).unwrap_or_default();
            apply(effects);
        })
        .ok();
}

fn with_controller<T>(operation: impl FnOnce(&mut OutputController) -> T) -> Option<T> {
    let shared = SHARED.get()?;
    let mut controller = shared.controller.lock().unwrap_or_else(|e| e.into_inner());
    Some(operation(&mut controller))
}

fn apply(effects: Vec<OutputEffect>) {
    let Some(shared) = SHARED.get() else { return };
    for effect in effects {
        match effect {
            OutputEffect::StateChanged(state) => {
                let _ = shared.app.emit("output-changed", state);
            }
            OutputEffect::Error { code, message } => {
                let _ = shared.app.emit(
                    "playback-error",
                    crate::commands::CommandError { code, message },
                );
            }
            OutputEffect::Notice(message) => {
                let _ = shared.app.emit(
                    "playback-error",
                    crate::commands::CommandError { code: "airplay_disconnected", message },
                );
            }
            OutputEffect::PausePlayback => {
                let app = shared.app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = crate::commands::pause(app).await {
                        tracing::warn!(code = error.code, "pause after AirPlay disconnect failed");
                    }
                });
            }
        }
    }
}

pub(crate) fn mpv_started(pid: u32) {
    apply(with_controller(|c| c.mpv_started(pid)).unwrap_or_default());
}

pub(crate) fn mpv_stopped() {
    with_controller(|c| c.mpv_stopped());
}

pub(crate) fn set_playing(playing: bool) {
    with_controller(|c| c.set_playing(playing));
}

pub(crate) fn seeked() {
    with_controller(|c| c.seeked());
}

pub(crate) fn corrected_position_ms(raw_ms: u64) -> u64 {
    with_controller(|c| c.corrected_position_ms(raw_ms)).unwrap_or(raw_ms)
}

pub(crate) fn queued_ms() -> f64 {
    with_controller(|c| c.queued_ms()).unwrap_or(0.0)
}

pub(crate) fn local_state(
    devices: Vec<devices::OutputDevice>,
    selected_device_id: Option<String>,
) -> OutputState {
    with_controller(|c| c.set_local_devices(devices.clone(), selected_device_id.clone()))
        .unwrap_or(OutputState { devices, selected_device_id, ..OutputState::default() })
}

pub(crate) fn leave_airplay() {
    apply(with_controller(|c| c.leave_airplay()).unwrap_or_default());
}

pub(crate) fn open_picker(anchor: PickerAnchor) -> Result<(), RouteError> {
    with_controller(|c| c.open_picker(&anchor))
        .unwrap_or_else(|| Err(RouteError::Unavailable("AirPlay is unavailable.".into())))
}
```

`CommandError` must be constructible here: in `commands.rs`, confirm `CommandError` derives `Serialize` and `Clone`, and that `code` and `message` are `pub`. They are `pub` already. `crate::commands::pause` is `pub async fn`.

The disconnect notice reuses the `playback-error` event with code `airplay_disconnected`; the UI already shows `playback-error` messages. Add `airplay_disconnected` to the error-code list in Global Constraints when reviewing.

- [ ] **Step 4: Hook `MpvPlayer`**

In `youtube.rs`:

- `set_paused`: after the `send` succeeds, add `crate::output::set_playing(!paused);`.
- `seek`: after the `send` succeeds, add `crate::output::seeked();`.
- `inspect`: replace the `position_ms` field expression with
  `position_ms: crate::output::corrected_position_ms((position_seconds.max(0.0) * 1000.0).round() as u64),`.
- `ensure_started`: after `self.apply_preferred_audio_device();` (added in Task 2) add `crate::output::mpv_started(child_pid);`.
- `stop` (line ~3046): inside the existing `if self.child.is_some() {` block, before the `quit` command, add `crate::output::mpv_stopped();`. Do not put it at the top of `stop()`: `Drop` calls `stop()`, and `prepare_transport`/`commit_transport` drop placeholder `MpvPlayer::new()` values on every transport call. A hook outside the `child` check would report a false mpv exit and stop AirPlay on each track change.

Add this test to `youtube.rs` tests to pin that rule (it needs no installed output module; it checks that dropping a placeholder does not reach `mpv_stopped` by asserting the placeholder has no child):

```rust
    #[test]
    fn placeholder_player_drop_does_not_report_an_mpv_exit() {
        let placeholder = MpvPlayer::new();
        assert!(placeholder.child.is_none());
        drop(placeholder); // `stop()` must skip `crate::output::mpv_stopped()` without a child.
    }
```

This test documents the guard; the reviewer checks that the hook sits inside the `child` branch.

- [ ] **Step 5: Hook the visualizer clock**

In `src-tauri/src/visualizer/worker.rs` (~250), change the clock mapping to:

```rust
                    .map(|time| time * 1000.0 - crate::output::queued_ms())
```

- [ ] **Step 6: Install in setup**

In `src-tauri/src/lib.rs` `setup`, after `app.manage(state);`:

```rust
            output::install(app.handle().clone(), output::platform_route());
```

Add to `output/mod.rs`:

```rust
/// macOS returns the AirPlay route in Task 6. Other systems have no AirPlay.
pub(crate) fn platform_route() -> Box<dyn AirPlayRoute> {
    Box::new(NoAirPlay)
}
```

- [ ] **Step 7: Run the whole suite**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: pass. The existing `youtube.rs` tests that assert exact command lists still pass, because the hooks send no mpv commands.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src
git commit -m "feat: route mpv playback events through the output module"
```

---

### Task 5: Frontend API and `useOutput`

**Files:**
- Modify: `src/api.ts` (types near `CommandError` ~170; new `outputApi` after `followingApi` ~316)
- Create: `src/composables/useOutput.ts`
- Create: `src/composables/__test__/useOutput.test.ts`
- Modify: `src/__test__/api.test.ts` (add wrapper tests in the existing style)

**Interfaces:**
- Consumes: commands `get_output_state`, `select_output_device` (Task 2), `open_airplay_picker` (Task 7), event `output-changed`.
- Produces:
  - Types `OutputDevice { id: string; name: string }`, `OutputState { devices: OutputDevice[]; selectedDeviceId: string | null; airplayAvailable: boolean; airplayReceivers: string[] | null }`, `PickerAnchor { x: number; y: number; width: number; height: number }`.
  - `outputApi.{getState(): Promise<OutputState>, selectDevice(id: string): Promise<OutputState>, openAirPlayPicker(anchor: PickerAnchor): Promise<void>}`.
  - `useOutput(client?: OutputClient, subscribe?: OutputSubscriber)` returning `{ state, isLoading, errorMessage, refresh, selectDevice, openAirPlayPicker, activeLabel, isAirPlay }`.

- [ ] **Step 1: Add API types and wrappers**

In `src/api.ts` after the `CommandError` interface:

```ts
export interface OutputDevice {
  id: string
  name: string
}

export interface OutputState {
  devices: OutputDevice[]
  selectedDeviceId: string | null
  airplayAvailable: boolean
  airplayReceivers: string[] | null
}

/** CSS pixels from the top-left corner of the window content. */
export interface PickerAnchor {
  x: number
  y: number
  width: number
  height: number
}
```

After `followingApi`:

```ts
export const outputApi = {
  getState: (): Promise<OutputState> => invoke<OutputState>('get_output_state'),
  selectDevice: (id: string): Promise<OutputState> =>
    invoke<OutputState>('select_output_device', { id }),
  openAirPlayPicker: (anchor: PickerAnchor): Promise<void> =>
    invoke<void>('open_airplay_picker', { anchor }),
}
```

Add tests to `src/__test__/api.test.ts` matching how that file asserts other wrappers (it mocks `invoke`). Assert `get_output_state` with no arguments, `select_output_device` with `{ id: 'coreaudio/DAC' }`, and `open_airplay_picker` with `{ anchor: { x: 1, y: 2, width: 3, height: 4 } }`.

- [ ] **Step 2: Write the failing composable tests**

`src/composables/__test__/useOutput.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest'

import type { OutputState } from '@/api'
import { useOutput } from '../useOutput'

const local: OutputState = {
  devices: [
    { id: 'coreaudio/Speakers', name: 'MacBook Pro Speakers' },
    { id: 'coreaudio/DAC', name: 'USB DAC' },
  ],
  selectedDeviceId: 'coreaudio/DAC',
  airplayAvailable: true,
  airplayReceivers: null,
}

function client(overrides = {}) {
  return {
    getState: vi.fn().mockResolvedValue(local),
    selectDevice: vi.fn().mockResolvedValue({ ...local, selectedDeviceId: 'coreaudio/Speakers' }),
    openAirPlayPicker: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  }
}

describe('useOutput', () => {
  it('loads state and labels the selected local device', async () => {
    const output = useOutput(client(), null)
    await output.refresh()
    expect(output.state.value.devices).toHaveLength(2)
    expect(output.activeLabel.value).toBe('USB DAC')
    expect(output.isAirPlay.value).toBe(false)
  })

  it('labels the system default when no device is saved', async () => {
    const output = useOutput(client({ getState: vi.fn().mockResolvedValue({ ...local, selectedDeviceId: null }) }), null)
    await output.refresh()
    expect(output.activeLabel.value).toBe('System default')
  })

  it('labels AirPlay receivers', async () => {
    const airplay = { ...local, airplayReceivers: ['Master Bedroom', 'Family Room'] }
    const output = useOutput(client({ getState: vi.fn().mockResolvedValue(airplay) }), null)
    await output.refresh()
    expect(output.isAirPlay.value).toBe(true)
    expect(output.activeLabel.value).toBe('Master Bedroom, Family Room')
  })

  it('selects a device and applies the returned state', async () => {
    const api = client()
    const output = useOutput(api, null)
    await output.selectDevice('coreaudio/Speakers')
    expect(api.selectDevice).toHaveBeenCalledWith('coreaudio/Speakers')
    expect(output.state.value.selectedDeviceId).toBe('coreaudio/Speakers')
  })

  it('reports command errors without throwing', async () => {
    const api = client({
      selectDevice: vi.fn().mockRejectedValue({ code: 'output_device_unavailable', message: 'The audio output is unavailable.' }),
    })
    const output = useOutput(api, null)
    await output.selectDevice('coreaudio/Gone')
    expect(output.errorMessage.value).toBe('The audio output is unavailable.')
  })

  it('opens the picker at the element rectangle', async () => {
    const api = client()
    const output = useOutput(api, null)
    const element = { getBoundingClientRect: () => ({ left: 10, top: 20, width: 28, height: 30 }) } as unknown as HTMLElement
    await output.openAirPlayPicker(element)
    expect(api.openAirPlayPicker).toHaveBeenCalledWith({ x: 10, y: 20, width: 28, height: 30 })
  })

  it('applies output-changed events', async () => {
    let handler: ((state: OutputState) => void) | undefined
    const subscribe = vi.fn(async (next: (state: OutputState) => void) => {
      handler = next
      return () => {}
    })
    const output = useOutput(client(), subscribe)
    await output.ready
    handler?.({ ...local, airplayReceivers: ['Master Bedroom'] })
    expect(output.isAirPlay.value).toBe(true)
  })
})
```

- [ ] **Step 3: Run to verify failure**

Run: `bun run test src/composables/__test__/useOutput.test.ts`
Expected: FAIL, cannot resolve `../useOutput`.

- [ ] **Step 4: Implement**

`src/composables/useOutput.ts`:

```ts
import { isTauri } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { computed, getCurrentScope, onScopeDispose, ref } from 'vue'

import { outputApi, type OutputState } from '@/api'

export type OutputClient = typeof outputApi
export type OutputSubscriber = (handler: (state: OutputState) => void) => Promise<() => void>

const empty: OutputState = {
  devices: [],
  selectedDeviceId: null,
  airplayAvailable: false,
  airplayReceivers: null,
}

function readErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error.trim()) return error
  if (error instanceof Error) return error.message
  if (typeof error === 'object' && error !== null && 'message' in error) {
    const message = (error as { message: unknown }).message
    if (typeof message === 'string' && message.trim()) return message
  }
  return 'The audio output command failed.'
}

const defaultSubscriber: OutputSubscriber = (handler) =>
  listen<OutputState>('output-changed', (event) => handler(event.payload))

/**
 * Output state for one window. The backend owns the mode. Each window loads state when its
 * menu opens and follows `output-changed` events. Outside Tauri (tests, previews) it stays empty.
 */
export function useOutput(
  client: OutputClient = outputApi,
  subscribe: OutputSubscriber | null = isTauri() ? defaultSubscriber : null,
) {
  const state = ref<OutputState>({ ...empty })
  const isLoading = ref(false)
  const errorMessage = ref('')

  const isAirPlay = computed(() => state.value.airplayReceivers !== null)
  const activeLabel = computed(() => {
    if (state.value.airplayReceivers) return state.value.airplayReceivers.join(', ')
    const device = state.value.devices.find((item) => item.id === state.value.selectedDeviceId)
    return device?.name ?? 'System default'
  })

  async function run(operation: () => Promise<OutputState | void>): Promise<void> {
    errorMessage.value = ''
    try {
      const next = await operation()
      if (next) state.value = next
    } catch (error) {
      errorMessage.value = readErrorMessage(error)
    }
  }

  async function refresh(): Promise<void> {
    isLoading.value = true
    await run(() => client.getState())
    isLoading.value = false
  }

  function selectDevice(id: string): Promise<void> {
    return run(() => client.selectDevice(id))
  }

  function openAirPlayPicker(element: HTMLElement): Promise<void> {
    const rect = element.getBoundingClientRect()
    return run(() =>
      client.openAirPlayPicker({ x: rect.left, y: rect.top, width: rect.width, height: rect.height }),
    )
  }

  let stop: (() => void) | undefined
  const ready = subscribe
    ? subscribe((next) => {
        state.value = next
      }).then((unlisten) => {
        stop = unlisten
      }, () => undefined)
    : Promise.resolve()
  if (getCurrentScope()) onScopeDispose(() => stop?.())

  return { state, isLoading, errorMessage, isAirPlay, activeLabel, refresh, selectDevice, openAirPlayPicker, ready }
}
```

- [ ] **Step 5: Run to verify pass**

Run: `bun run test src/composables/__test__/useOutput.test.ts src/__test__/api.test.ts`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/api.ts src/composables src/__test__/api.test.ts
git commit -m "feat: add output state composable"
```

---

### Task 6: `OutputMenu` component and placements

**Files:**
- Create: `src/components/OutputMenu.vue`
- Create: `src/components/__test__/OutputMenu.test.ts`
- Modify: `src/components/ArtworkWindow.vue` (volume row, ~414: after the `<Slider ... />` inside `<div class="flex w-28 ...">`)
- Modify: `src/components/MiniPlayer.vue` (~222: the volume grid; insert before the mute `<Button>`, and change the grid to four columns)
- Modify: `src/components/QueueWindow.vue` (~610: after the "Next track" `<Button>` inside `<nav aria-label="Queue playback controls">`)
- Modify: `src/components/library/LibraryPlaybackFooter.vue` (~232: inside `data-playback-control="volume"`, after the `<Slider ... />`)
- Modify: existing tests for those four components only if a selector breaks

**Interfaces:**
- Consumes: `useOutput` (Task 5).
- Produces: `<OutputMenu :client? :subscribe? size?="'icon-sm' | 'icon-xs'" />`. Accessible name: `Audio output: <activeLabel>`. Root has `data-output-menu`.

- [ ] **Step 1: Write the failing component tests**

`src/components/__test__/OutputMenu.test.ts`:

```ts
import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, it, vi } from 'vitest'

import type { OutputState } from '@/api'
import OutputMenu from '../OutputMenu.vue'

const local: OutputState = {
  devices: [
    { id: 'coreaudio/Speakers', name: 'MacBook Pro Speakers' },
    { id: 'coreaudio/DAC', name: 'USB DAC' },
  ],
  selectedDeviceId: 'coreaudio/DAC',
  airplayAvailable: true,
  airplayReceivers: null,
}

function client(state: OutputState = local) {
  return {
    getState: vi.fn().mockResolvedValue(state),
    selectDevice: vi.fn().mockResolvedValue({ ...state, selectedDeviceId: 'coreaudio/Speakers' }),
    openAirPlayPicker: vi.fn().mockResolvedValue(undefined),
  }
}

async function open(wrapper: ReturnType<typeof mount>) {
  await wrapper.get('[data-output-menu] button').trigger('pointerdown', { button: 0, ctrlKey: false })
  await flushPromises()
}

describe('OutputMenu', () => {
  it('loads devices when opened and marks the selected one', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, { props: { client: api, subscribe: null }, attachTo: document.body })
    await open(wrapper)
    expect(api.getState).toHaveBeenCalled()
    const items = document.querySelectorAll('[data-output-device]')
    expect([...items].map((item) => item.textContent?.trim())).toEqual(['MacBook Pro Speakers', 'USB DAC'])
    expect(document.querySelector('[data-output-device="coreaudio/DAC"]')?.getAttribute('aria-checked')).toBe('true')
    wrapper.unmount()
  })

  it('selects a device', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, { props: { client: api, subscribe: null }, attachTo: document.body })
    await open(wrapper)
    ;(document.querySelector('[data-output-device="coreaudio/Speakers"]') as HTMLElement).click()
    await flushPromises()
    expect(api.selectDevice).toHaveBeenCalledWith('coreaudio/Speakers')
    wrapper.unmount()
  })

  it('shows AirPlay only when available and opens the picker', async () => {
    const api = client()
    const wrapper = mount(OutputMenu, { props: { client: api, subscribe: null }, attachTo: document.body })
    await open(wrapper)
    const airplay = document.querySelector('[data-output-airplay]') as HTMLElement
    expect(airplay.textContent).toContain('AirPlay…')
    airplay.click()
    await flushPromises()
    expect(api.openAirPlayPicker).toHaveBeenCalled()
    wrapper.unmount()
  })

  it('hides AirPlay when unavailable', async () => {
    const api = client({ ...local, airplayAvailable: false })
    const wrapper = mount(OutputMenu, { props: { client: api, subscribe: null }, attachTo: document.body })
    await open(wrapper)
    expect(document.querySelector('[data-output-airplay]')).toBeNull()
    wrapper.unmount()
  })

  it('names the active output in the button label', async () => {
    const api = client({ ...local, airplayReceivers: ['Master Bedroom'] })
    const wrapper = mount(OutputMenu, { props: { client: api, subscribe: null }, attachTo: document.body })
    await open(wrapper)
    expect(wrapper.get('[data-output-menu] button').attributes('aria-label')).toBe('Audio output: Master Bedroom')
    expect(document.querySelector('[data-output-airplay]')?.textContent).toContain('Master Bedroom')
    wrapper.unmount()
  })
})
```

If Reka's trigger does not open on `pointerdown` in happy-dom/jsdom, copy the open helper used in an existing dropdown test (search `src/components/__test__` and `src/components/library/__test__` for `DropdownMenuTrigger` or `LibraryHeader`).

- [ ] **Step 2: Run to verify failure**

Run: `bun run test src/components/__test__/OutputMenu.test.ts`
Expected: FAIL, cannot resolve `../OutputMenu.vue`.

- [ ] **Step 3: Implement**

`src/components/OutputMenu.vue`:

```vue
<script setup lang="ts">
import { Airplay, Check, Speaker } from 'lucide-vue-next'
import { ref } from 'vue'

import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useOutput, type OutputClient, type OutputSubscriber } from '@/composables/useOutput'

const props = withDefaults(
  defineProps<{
    client?: OutputClient
    subscribe?: OutputSubscriber | null
    size?: 'icon-sm' | 'icon-xs'
    buttonClass?: string
  }>(),
  { size: 'icon-sm', buttonClass: '' },
)

const output =
  props.client === undefined
    ? useOutput()
    : useOutput(props.client, props.subscribe === undefined ? null : props.subscribe)
const trigger = ref<InstanceType<typeof Button>>()

function onOpenChange(open: boolean): void {
  if (open) void output.refresh()
}

function triggerElement(): HTMLElement | undefined {
  const value = trigger.value as unknown as { $el?: HTMLElement } | undefined
  return value?.$el
}

async function chooseAirPlay(): Promise<void> {
  const element = triggerElement()
  // Let the menu close first, so the popover is not dismissed with it.
  await new Promise((resolve) => setTimeout(resolve, 0))
  if (element) await output.openAirPlayPicker(element)
}
</script>

<template>
  <div data-output-menu class="contents">
    <DropdownMenu @update:open="onOpenChange">
      <DropdownMenuTrigger as-child>
        <Button
          ref="trigger"
          :aria-label="`Audio output: ${output.activeLabel.value}`"
          :title="`Audio output: ${output.activeLabel.value}`"
          :class="buttonClass"
          :size="size"
          variant="ghost"
        >
          <Airplay v-if="output.isAirPlay.value" aria-hidden="true" />
          <Speaker v-else aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" class="min-w-56">
        <DropdownMenuLabel>Audio output</DropdownMenuLabel>
        <DropdownMenuItem
          v-for="device in output.state.value.devices"
          :key="device.id"
          role="menuitemradio"
          :aria-checked="!output.isAirPlay.value && device.id === output.state.value.selectedDeviceId"
          :data-output-device="device.id"
          @select="output.selectDevice(device.id)"
        >
          <span class="flex-1 truncate">{{ device.name }}</span>
          <Check
            v-if="!output.isAirPlay.value && device.id === output.state.value.selectedDeviceId"
            aria-hidden="true"
          />
        </DropdownMenuItem>
        <template v-if="output.state.value.airplayAvailable">
          <DropdownMenuSeparator />
          <DropdownMenuItem
            data-output-airplay
            role="menuitemradio"
            :aria-checked="output.isAirPlay.value"
            @select="chooseAirPlay"
          >
            <Airplay aria-hidden="true" />
            <span class="flex-1 truncate">
              {{ output.isAirPlay.value ? `AirPlay: ${output.activeLabel.value}` : 'AirPlay…' }}
            </span>
            <Check v-if="output.isAirPlay.value" aria-hidden="true" />
          </DropdownMenuItem>
        </template>
        <p
          v-if="output.errorMessage.value"
          role="alert"
          class="px-2 py-1.5 text-xs text-(--muted-text)"
        >
          {{ output.errorMessage.value }}
        </p>
      </DropdownMenuContent>
    </DropdownMenu>
  </div>
</template>
```

Check that `lucide-vue-next` exports `Airplay` and `Speaker` (`grep -r "Airplay" node_modules/lucide-vue-next/dist/esm/icons/ | head -1`). If an export name differs, use the existing name (for example `Cast`) and keep the test selectors.

- [ ] **Step 4: Run to verify pass**

Run: `bun run test src/components/__test__/OutputMenu.test.ts`
Expected: PASS.

- [ ] **Step 5: Place the menu in four components**

Add `import OutputMenu from './OutputMenu.vue'` (or `'../OutputMenu.vue'` in `library/`) to each file.

1. `ArtworkWindow.vue`, inside `<div class="flex w-28 min-w-0 items-center gap-2">`, after the `<Slider ... />`:

```vue
            <OutputMenu :button-class="actionClass" />
```

Change that div's `w-28` to `w-36` so the slider keeps its width.

2. `MiniPlayer.vue`, in the volume grid: change `grid-cols-[14px_minmax(0,1fr)_18px]` to `grid-cols-[14px_minmax(0,1fr)_18px_20px]` and `w-24` to `w-30`, then add after the volume number `<span>`:

```vue
          <OutputMenu
            size="icon-xs"
            button-class="size-5 text-(--muted-text) hover:bg-(--surface-muted) hover:text-(--text) [&_svg]:size-3.5"
          />
```

3. `QueueWindow.vue`, inside `<nav aria-label="Queue playback controls">`, after the "Next track" button:

```vue
        <OutputMenu />
```

4. `LibraryPlaybackFooter.vue`, inside `data-playback-control="volume"`, after the `<Slider ... />`:

```vue
        <OutputMenu />
```

Change that div's `w-36` to `w-44`.

- [ ] **Step 6: Run the full frontend suite**

Run: `bun run test`
Expected: PASS. `OutputMenu` is inert outside Tauri (`isTauri()` is false in tests), so existing component tests need no mocks. If an existing test counts buttons in a region, update the expected count and say so in the commit body.

- [ ] **Step 7: Lint, type-check, format, commit**

```bash
bun run lint && bun run build && bun run format
git add src
git commit -m "feat: add audio output menu beside playback controls"
```

---

### Task 7: macOS AirPlay route (process tap, renderer, picker)

This task has no automated test for the native behavior. The controller (Task 3) is already tested with a fake. Here the goal is a compiling, clippy-clean implementation that follows the probe results in the spec. The human checkpoint after this task verifies audio.

**Files:**
- Modify: `src-tauri/Cargo.toml` (macOS dependencies)
- Create: `src-tauri/Info.plist`
- Create: `src-tauri/src/output/macos/mod.rs`
- Create: `src-tauri/src/output/macos/process_tap.rs`
- Create: `src-tauri/src/output/macos/airplay.rs`
- Modify: `src-tauri/src/output/mod.rs` (`#[cfg(target_os = "macos")] mod macos;`, `platform_route`)
- Modify: `src-tauri/src/commands.rs` (`open_airplay_picker`)
- Modify: `src-tauri/src/lib.rs` (register `open_airplay_picker`)

**Interfaces:**
- Consumes: `AirPlayRoute`, `PickerAnchor`, `RouteError`, `RoutePoll` (Task 3); `SampleRing` (Task 3); `clock::{queued_ms, DriftMonitor}` (Task 3); `output::open_picker` (Task 4).
- Produces: `crate::output::macos::MacAirPlayRoute::new(app: tauri::AppHandle) -> Box<dyn AirPlayRoute>`; Tauri command `open_airplay_picker(window: tauri::WebviewWindow, anchor: PickerAnchor) -> Result<(), CommandError>`.

- [ ] **Step 1: Dependencies and Info.plist**

In `src-tauri/Cargo.toml` under `[target.'cfg(target_os = "macos")'.dependencies]`:

```toml
objc2-core-audio = "0.3.2"
objc2-core-audio-types = "0.3.2"
objc2-core-media = "0.3.2"
objc2-core-foundation = "0.3.2"
```

Extend the existing `objc2-foundation` features with `"NSArray"`, `"NSDictionary"`, `"NSGeometry"`, `"NSUUID"`, `"NSValue"`. These crates use default features (all framework items). Run `cargo build` from `src-tauri/`; if a type is behind a missing feature, add the feature the compiler names.

`src-tauri/Info.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>NSAudioCaptureUsageDescription</key>
  <string>G Music captures its own audio to play it on AirPlay speakers.</string>
</dict>
</plist>
```

Tauri 2 merges `src-tauri/Info.plist` into the bundle. Verify with `bun run tauri build --debug --bundles app` later in the checkpoint, not in this task.

- [ ] **Step 2: Process tap**

`src-tauri/src/output/macos/process_tap.rs`. Behavior (from the spec's probe evidence):

1. Translate the PID to a process object: `AudioObjectGetPropertyData(kAudioObjectSystemObject, {kAudioHardwarePropertyTranslatePIDToProcessObject, global, main}, qualifier = pid (i32), out = AudioObjectID)`. A zero object or non-zero status → `RouteError::Unavailable("mpv is not producing audio yet.")`.
2. Create `CATapDescription` with `msg_send!`: `alloc`, then `initStereoMixdownOfProcesses:` with an `NSArray<NSNumber>` holding the process object ID. Then `setMuteBehavior: 2` (`CATapMutedWhenTapped`), `setPrivateTap: true`, `setName: "G Music AirPlay"`. Read `UUID` → `UUIDString`.
3. `AudioHardwareCreateProcessTap(Some(&description), &mut tap_id)`. Non-zero status → `RouteError::PermissionDenied(format!("tap status {status}"))`.
4. Read `kAudioTapPropertyFormat` into `AudioStreamBasicDescription`. Require 48 kHz or 44.1 kHz float32 interleaved stereo; store the sample rate. Otherwise destroy the tap and return `Unavailable`.
5. Create a private aggregate device with an `NSDictionary` (toll-free bridged to `CFDictionary`): `kAudioAggregateDeviceUIDKey` = new UUID string, `kAudioAggregateDeviceNameKey` = `"G Music AirPlay"`, `kAudioAggregateDeviceIsPrivateKey` = `true`, `kAudioAggregateDeviceTapAutoStartKey` = `true`, `kAudioAggregateDeviceTapListKey` = `[ { kAudioSubTapUIDKey: tap UUID string, kAudioSubTapDriftCompensationKey: true } ]`. The key constants are `&CStr`; convert with `NSString::from_str(key.to_str().unwrap())`.
6. Register a C IOProc with `AudioDeviceCreateIOProcID(aggregate, Some(io_proc), Arc::into_raw(ring) as *mut c_void, &mut proc_id)` and start it with `AudioDeviceStart`. The IOProc reads `in_input_data` (`AudioBufferList`), takes buffer 0 as interleaved `f32`, and calls `ring.push(samples)`. It must not allocate, lock (other than `try_lock` inside `push`), or log.
7. `Drop` for the tap handle: `AudioDeviceStop`, `AudioDeviceDestroyIOProcID`, `AudioHardwareDestroyAggregateDevice`, `AudioHardwareDestroyProcessTap`, then `Arc::from_raw` the ring pointer once. This order restores local sound.

Public shape:

```rust
pub(super) struct ProcessTap { /* tap_id, aggregate_id, proc_id, ring_ptr, sample_rate */ }

impl ProcessTap {
    pub(super) fn start(mpv_pid: u32, ring: std::sync::Arc<crate::output::ring::SampleRing>)
        -> Result<Self, crate::output::controller::RouteError>;
    pub(super) fn sample_rate(&self) -> f64;
}

impl Drop for ProcessTap { /* step 7 */ }
```

Mark `unsafe impl Send for ProcessTap {}` with a `// SAFETY:` comment: the handle holds only Core Audio object IDs and a raw `Arc` pointer.

Use the `objc2-core-audio` 0.3.2 generated signatures (see `~/.cargo/registry/src/*/objc2-core-audio-0.3.2/src/generated/AudioHardware.rs`): out-parameters are `NonNull<T>`, and `AudioDeviceIOProc` is `Option<unsafe extern "C-unwind" fn(...)>`. Match those exactly. `CATapDescription` has no generated methods in this crate version; cast your `Retained<AnyObject>` with `&*(Retained::as_ptr(&obj) as *const objc2_core_audio::CATapDescription)` when calling `AudioHardwareCreateProcessTap`.

- [ ] **Step 3: Renderer, synchronizer, and feeder**

`src-tauri/src/output/macos/airplay.rs`. Link frameworks:

```rust
#[link(name = "AVFoundation", kind = "framework")]
#[link(name = "AVKit", kind = "framework")]
unsafe extern "C" {
    static AVSampleBufferAudioRendererWasFlushedAutomaticallyNotification: &'static objc2_foundation::NSString;
}
```

Objects (all through `AnyClass::get(c"...")` and `msg_send!`; if any class or selector is missing, `available()` returns false):

- Context: `AVOutputContext` `+addSharedAudioOutputContext`. Create it once, on the main thread, inside `MacAirPlayRoute::new` (called from `setup`, which runs on the main thread). Read `contextID` (`NSString`).
- Renderer: `AVSampleBufferAudioRenderer` `new`, then `setOutputContext:` with the context.
- Synchronizer: `AVSampleBufferRenderSynchronizer` `new`, `addRenderer:`.
- Observer: `NSNotificationCenter defaultCenter` `addObserverForName:object:queue:usingBlock:` with the notification constant, the renderer, `queue = nil`, and an `RcBlock` that stores `true` in an `Arc<AtomicBool>` named `flush_requested`. Follow the observer pattern in `src-tauri/src/media_session.rs` (power module). Remove the observer in `Drop`.

Feeder thread (one per `start`, stopped by an `AtomicBool`), every 10 ms:

1. If `flush_requested` or `resync_requested` is set: `setRate:0.0`; `flush`; `ring.clear()`; `next_pts = synchronizer currentTime`; clear the flags; mark `waiting_for_prime = true`; `drift.reset(now)`.
2. While `isReadyForMoreMediaData` and the ring holds ≥ 4800 frames × 2 channels: pop 9600 samples; build a `CMSampleBuffer` (float32 interleaved stereo at the tap sample rate) with `CMBlockBufferCreateWithMemoryBlock` (copy the samples into a `kCFAllocatorMalloc`-owned block or let Core Media allocate and copy with `CMBlockBufferReplaceDataBytes`), `CMAudioFormatDescriptionCreate`, and `CMAudioSampleBufferCreateReadyWithPacketDescriptions` with presentation time `next_pts`; `enqueueSampleBuffer:`; advance `next_pts` by 4800 frames.
3. If `waiting_for_prime` and enqueued audio ≥ 300 ms ahead of `next_pts` at prime start: `setRate:time:` with `rate = if playing { 1.0 } else { 0.0 }` and `time = prime start pts`; clear `waiting_for_prime`.
4. Every second: `queued = clock::queued_ms(next_pts seconds, currentTime seconds)`; store it in an `AtomicU64` (`f64::to_bits`) for `queued_ms()`; if `drift.observe(now, queued)` → set `resync_requested`.

`set_playing(playing)`: store the flag; `ring.set_open(playing)`; `setRate:` 1.0 or 0.0 at the current time unless `waiting_for_prime`.

`flush()` (seek): set `resync_requested`.

`poll()`: read the context's `outputDevices` (`NSArray`), map each to `deviceName` (`NSString`), and read renderer `status` (`2` = failed). Return `RoutePoll { receivers, failed }`.

`stop()`: stop the feeder thread and join it; `setRate:0.0`; `flush`; drop the `ProcessTap`; `ring.set_open(false)`; `ring.clear()`; store 0 in the queued value.

`start(mpv_pid, playing)`: `ProcessTap::start(mpv_pid, ring.clone())?`; `ring.set_open(playing)`; reset `next_pts` to the synchronizer's current time; `waiting_for_prime = true`; spawn the feeder.

Wrap the `Retained<AnyObject>` handles in a struct with `unsafe impl Send` and a `// SAFETY:` comment: AVFoundation renderer and synchronizer calls are serialized by the controller mutex and the single feeder thread; the probe used the same objects from a background queue.

- [ ] **Step 4: Picker**

In `airplay.rs`, `open_picker(anchor)` runs on the main thread through `app.run_on_main_thread`:

1. `app.get_webview_window(&anchor.window_label)`; `window.ns_window()` → `*mut c_void` → `&AnyObject`; `contentView`; read its `frame` (`NSRect`).
2. Keep one `AVRoutePickerView` per window label in a `thread_local!` `RefCell<HashMap<String, Retained<AnyObject>>>` (main thread only). Create it with `alloc` + `initWithFrame:`, `setOutputContextID:` with the context ID, `setAlphaValue: 0.0`, and add it to the content view.
3. Set its frame to the anchor, flipping y: `ns_y = content_height - (anchor.y + anchor.height)`.
4. If the picker responds to `showRoutePickingControlsForOutputContext:relativeToRect:ofView:`, call it with the context, the picker's `bounds`, and the picker. Otherwise find the first `NSButton` in its `subviews` and send `performClick:` with `nil`.
5. Return `Ok(())` after dispatching. Errors before dispatch (window missing, `ns_window` error) → `RouteError::Unavailable`.

- [ ] **Step 5: `MacAirPlayRoute` and wiring**

`src-tauri/src/output/macos/mod.rs` defines `MacAirPlayRoute` holding the AVFoundation handles, the `Arc<SampleRing>` (capacity: 4 s × 48,000 × 2 samples), the optional `ProcessTap`, the feeder handle, and the shared atomics; it implements `AirPlayRoute` by delegating to Steps 2–4. `available()` is true only when `CATapDescription`, `AVOutputContext`, `AVSampleBufferAudioRenderer`, `AVSampleBufferRenderSynchronizer`, and `AVRoutePickerView` exist and the context was created.

In `output/mod.rs`:

```rust
#[cfg(target_os = "macos")]
mod macos;

pub(crate) fn platform_route(app: &AppHandle) -> Box<dyn AirPlayRoute> {
    #[cfg(target_os = "macos")]
    {
        return macos::MacAirPlayRoute::new(app.clone());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Box::new(NoAirPlay)
    }
}
```

Update the call in `lib.rs` to `output::install(app.handle().clone(), output::platform_route(app.handle()));`.

Command in `commands.rs`:

```rust
#[tauri::command]
pub async fn open_airplay_picker(
    window: tauri::WebviewWindow,
    mut anchor: crate::output::controller::PickerAnchor,
) -> Result<(), CommandError> {
    anchor.window_label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || crate::output::open_picker(anchor))
        .await
        .map_err(|_| unavailable())?
        .map_err(|error| match error {
            crate::output::controller::RouteError::PermissionDenied(message) => CommandError {
                code: "airplay_permission_denied",
                message,
            },
            crate::output::controller::RouteError::Unavailable(message) => CommandError {
                code: "airplay_unavailable",
                message,
            },
        })
}
```

Register `commands::open_airplay_picker` in `lib.rs`.

- [ ] **Step 6: Build and lint on macOS**

Run from `src-tauri/`: `cargo build && cargo test && cargo clippy --all-targets --all-features -- -D warnings`
Expected: pass. Every `unsafe` block has a `// SAFETY:` comment.

- [ ] **Step 7: Commit**

```bash
git add src-tauri
git commit -m "feat: play mpv audio on AirPlay through a process tap"
```

- [ ] **Step 8: Human checkpoint (controller runs this, not a subagent)**

The controller session stops here and asks the human partner to run the app (`bun run tauri dev`) and confirm spec smoke steps 1–6. Record results in the task report. Fix failures before Task 8.

---

### Task 8: Command palette entries

**Files:**
- Modify: `src/lib/command-palette.ts`
- Modify: `src/lib/__test__/` (the existing command palette test file; search for `searchCommands` tests)
- Modify: `src/components/LibraryWindow.vue` (`runPaletteAction` ~158 and the `<CommandPalette>` props ~1326)
- Modify: `src/components/CommandPalette.vue` (pass `outputs` through)

**Interfaces:**
- Consumes: `OutputState` (Task 5), `useOutput` (Task 5).
- Produces: `CommandResult.kind` gains `'output'`; `searchCommands(query, tracks, playlists, selectedIds = [], output?: OutputState)`; results `{ id: 'output:<deviceId>', title: 'Output: <name>', kind: 'output', detail: 'Audio output' }` and `{ id: 'output:airplay', title: 'AirPlay…', kind: 'output', detail: 'Audio output' }` when `airplayAvailable`.

- [ ] **Step 1: Write the failing tests**

Add to the existing `searchCommands` test file:

```ts
const outputState: OutputState = {
  devices: [{ id: 'coreaudio/DAC', name: 'USB DAC' }],
  selectedDeviceId: null,
  airplayAvailable: true,
  airplayReceivers: null,
}

it('offers output devices and AirPlay', () => {
  const results = searchCommands('output', [], [], [], outputState)
  expect(results.map((r) => r.id)).toContain('output:coreaudio/DAC')
  expect(searchCommands('airplay', [], [], [], outputState)[0]).toMatchObject({
    id: 'output:airplay',
    title: 'AirPlay…',
    kind: 'output',
  })
})

it('omits AirPlay when unavailable', () => {
  const results = searchCommands('airplay', [], [], [], { ...outputState, airplayAvailable: false })
  expect(results.find((r) => r.id === 'output:airplay')).toBeUndefined()
})
```

Import `OutputState` from `@/api`.

- [ ] **Step 2: Run to verify failure**

Run: `bun run test src/lib`
Expected: FAIL (wrong arity or missing results).

- [ ] **Step 3: Implement**

In `command-palette.ts`: extend the `kind` union with `'output'`; add the `output?: OutputState` parameter; after building `actions`, build:

```ts
  const outputs: CommandResult[] = output
    ? [
        ...output.devices.map((device) => ({
          id: `output:${device.id}`,
          title: `Output: ${device.name}`,
          detail: 'Audio output',
          kind: 'output' as const,
          trackIds: [],
        })),
        ...(output.airplayAvailable
          ? [{ id: 'output:airplay', title: 'AirPlay…', detail: 'Audio output', kind: 'output' as const, trackIds: [] }]
          : []),
      ]
    : []
```

Include `outputs` in `candidates` after `actions` in both branches (`[...results, ...actions, ...outputs]` and `[...actions, ...outputs, ...]`).

In `CommandPalette.vue`: add prop `output?: OutputState`; pass it as the fifth argument to `searchCommands`; treat `'output'` like `'action'` in `actionLabel` and in the `run` guard (`['action', 'selection', 'output'].includes(result.kind)`).

In `LibraryWindow.vue`: `const output = useOutput()`; call `output.refresh()` when the palette opens (in the place that sets `paletteOpen.value = true`, or with `watch(paletteOpen, (open) => open && output.refresh())`); pass `:output="output.state.value"` to `<CommandPalette>`; in `runPaletteAction` add before the final `else`:

```ts
  } else if (result.kind === 'output') {
    if (result.id === 'output:airplay') {
      paletteOpen.value = false
      await nextTick()
      const root = document.querySelector('main') ?? document.body
      const rect = root.getBoundingClientRect()
      await output.openAirPlayPicker({
        getBoundingClientRect: () => ({
          left: rect.left + rect.width / 2 - 14,
          top: rect.top + rect.height / 2 - 14,
          width: 28,
          height: 28,
        }),
      } as HTMLElement)
    } else {
      await output.selectDevice(result.id.slice('output:'.length))
    }
    if (output.errorMessage.value) throw new Error(output.errorMessage.value)
```

- [ ] **Step 4: Run to verify pass**

Run: `bun run test`
Expected: PASS.

- [ ] **Step 5: Lint, build, commit**

```bash
bun run lint && bun run build && bun run format
git add src
git commit -m "feat: add audio output commands to the palette"
```

---

### Task 9: Documentation

**Files:**
- Modify: `README.md` (add a section after "Visualizer window")
- Modify: `docs/release-readiness.md` (manual native checks)
- Modify: `docs/superpowers/specs/2026-10-10-playback-devices-design.md` (status line)

- [ ] **Step 1: README section**

Add after the "Visualizer window" section, in the same Simplified Technical English style:

```markdown
## Audio output

Select the speaker button beside the volume control in the Artwork window, mini
player, queue, or library footer. Choose a local output: built-in speakers, a
display, a USB DAC, Bluetooth, or HDMI. G Music remembers the choice. If the
device is not connected at launch, G Music uses the system default until the
device returns.

On macOS 14.2 or later, choose **AirPlay…** to open the system AirPlay menu and
select a speaker, including HomePod. The first time, macOS asks for permission
to record system audio. G Music captures only its own player audio and sends it
to the speaker. AirPlay adds about 2 seconds of delay. The progress bar and the
visualizer follow the speaker. Track titles change about 2 seconds before the
new track reaches the speaker.

Select AirPlay again after each launch. If the speaker disconnects, playback
pauses and returns to the local output. The command palette also lists
**Output:** entries and **AirPlay…**.
```

- [ ] **Step 2: Release-readiness smoke steps**

Add a subsection under the existing manual native checks in `docs/release-readiness.md` with the eight smoke steps from the spec's "Manual native smoke test" section, copied verbatim.

- [ ] **Step 3: Spec status**

Change the spec status line to `Status: Implemented. Manual AirPlay smoke test results are in the Task 7 checkpoint report.`

- [ ] **Step 4: Format and commit**

```bash
bun run format
git add README.md docs
git commit -m "docs: describe audio output and AirPlay"
```

---

## Final verification (controller)

Run all verification commands from Global Constraints. Then run the whole-branch review required by the execution skill.
