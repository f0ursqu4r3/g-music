//! Audio output selection: local devices through mpv and AirPlay on macOS.

pub(crate) mod devices;

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod clock;
pub(crate) mod controller;
#[cfg(target_os = "macos")]
mod macos;
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod ring;

pub(crate) use controller::OutputState;

use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use controller::{
    AirPlayRoute, OutputController, OutputEffect, PickerAnchor, RouteError, RoutePoll,
};

struct Shared {
    app: AppHandle,
    controller: Mutex<OutputController>,
}

static SHARED: OnceLock<Shared> = OnceLock::new();
const TICK: Duration = Duration::from_millis(250);

/// A route for systems without AirPlay, and for tests. macOS builds use it only in tests.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(crate) struct NoAirPlay;

impl AirPlayRoute for NoAirPlay {
    fn available(&self) -> bool {
        false
    }
    fn open_picker(&mut self, _anchor: &PickerAnchor) -> Result<(), RouteError> {
        Err(RouteError::Unavailable(
            "AirPlay is unavailable on this system.".into(),
        ))
    }
    fn start(&mut self, _mpv_pid: u32, _playing: bool) -> Result<(), RouteError> {
        Err(RouteError::Unavailable(
            "AirPlay is unavailable on this system.".into(),
        ))
    }
    fn stop(&mut self) {}
    fn set_playing(&mut self, _playing: bool) {}
    fn flush(&mut self) {}
    fn queued_ms(&self) -> f64 {
        0.0
    }
    fn poll(&mut self) -> RoutePoll {
        RoutePoll::default()
    }
}

/// macOS returns the AirPlay route. Other systems have no AirPlay.
///
/// On macOS, call this from the main thread (Tauri `setup`).
pub(crate) fn platform_route(app: &AppHandle) -> Box<dyn AirPlayRoute> {
    #[cfg(target_os = "macos")]
    {
        macos::MacAirPlayRoute::new(app.clone())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Box::new(NoAirPlay)
    }
}

/// Installs the process-global output controller and starts its tick thread.
pub(crate) fn install(app: AppHandle, route: Box<dyn AirPlayRoute>) {
    let shared = Shared {
        app,
        controller: Mutex::new(OutputController::new(route)),
    };
    if SHARED.set(shared).is_err() {
        return;
    }
    let spawned = thread::Builder::new()
        .name("gmusic-output".into())
        .spawn(|| {
            loop {
                thread::sleep(TICK);
                let effects = with_controller(|c| c.tick(Instant::now())).unwrap_or_default();
                apply(effects);
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the audio output tick thread did not start");
    }
}

fn with_controller<T>(operation: impl FnOnce(&mut OutputController) -> T) -> Option<T> {
    let shared = SHARED.get()?;
    let mut controller = shared
        .controller
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    Some(operation(&mut controller))
}

/// Runs effects after the controller lock is released.
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
                    crate::commands::CommandError {
                        code: "airplay_disconnected",
                        message,
                    },
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

#[cfg(test)]
thread_local! {
    /// Counts `mpv_stopped` calls on this thread so tests can pin where the hook fires.
    pub(crate) static MPV_STOPPED_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn mpv_stopped() {
    #[cfg(test)]
    MPV_STOPPED_CALLS.with(|calls| calls.set(calls.get() + 1));
    with_controller(|c| c.mpv_stopped());
}

#[cfg(test)]
thread_local! {
    /// Counts `set_playing(false)` calls on this thread.
    pub(crate) static PAUSED_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Counts `seeked` calls on this thread.
    pub(crate) static SEEKED_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) fn set_playing(playing: bool) {
    #[cfg(test)]
    if !playing {
        PAUSED_CALLS.with(|calls| calls.set(calls.get() + 1));
    }
    with_controller(|c| c.set_playing(playing));
}

pub(crate) fn seeked() {
    #[cfg(test)]
    SEEKED_CALLS.with(|calls| calls.set(calls.get() + 1));
    with_controller(|c| c.seeked());
}

pub(crate) fn corrected_position_ms(raw_ms: u64) -> u64 {
    with_controller(|c| c.corrected_position_ms(raw_ms)).unwrap_or(raw_ms)
}

pub(crate) fn queued_ms() -> f64 {
    with_controller(|c| c.queued_ms()).unwrap_or(0.0)
}

/// Merges the latest local device list into the shared output state.
pub(crate) fn local_state(
    devices: Vec<devices::OutputDevice>,
    selected_device_id: Option<String>,
) -> OutputState {
    with_controller(|c| c.set_local_devices(devices.clone(), selected_device_id.clone())).unwrap_or(
        OutputState {
            devices,
            selected_device_id,
            ..OutputState::default()
        },
    )
}

/// Returns to Local mode when AirPlay is active.
pub(crate) fn leave_airplay() {
    apply(with_controller(|c| c.leave_airplay()).unwrap_or_default());
}

pub(crate) fn open_picker(anchor: PickerAnchor) -> Result<(), RouteError> {
    with_controller(|c| c.open_picker(&anchor))
        .unwrap_or_else(|| Err(RouteError::Unavailable("AirPlay is unavailable.".into())))
}

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
