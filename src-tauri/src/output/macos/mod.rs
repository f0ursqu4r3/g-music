//! macOS AirPlay route: a process tap on mpv feeds an AVFoundation audio renderer
//! on a shared audio output context.
//!
//! The controller mutex is held while the facade calls these methods, so nothing
//! here takes the app's transport or playback locks, and nothing waits on the main
//! thread or on the user.

mod airplay;
mod process_tap;

use std::sync::Arc;

use objc2::MainThreadMarker;
use tauri::AppHandle;

use super::controller::{AirPlayRoute, PickerAnchor, RouteError, RoutePoll};
use super::ring::SampleRing;
use airplay::{Av, Engine};
use process_tap::ProcessTap;

/// Four seconds of 48 kHz stereo.
const RING_CAPACITY_SAMPLES: usize = 4 * 48_000 * 2;

pub(crate) struct MacAirPlayRoute {
    app: AppHandle,
    /// `None` when a class is missing or the output context could not be created.
    engine: Option<Engine>,
    ring: Arc<SampleRing>,
    tap: Option<ProcessTap>,
}

impl MacAirPlayRoute {
    /// Call from the main thread (Tauri `setup`). The output context is created here.
    // The facade owns the route as a trait object, so the constructor returns the box.
    #[allow(clippy::new_ret_no_self)]
    pub(crate) fn new(app: AppHandle) -> Box<dyn AirPlayRoute> {
        let engine = match MainThreadMarker::new() {
            Some(main) if Av::classes_exist() => Av::create(main).map(Engine::new),
            _ => None,
        };
        if engine.is_none() {
            tracing::info!("AirPlay is unavailable on this Mac.");
        }
        Box::new(Self {
            app,
            engine,
            ring: Arc::new(SampleRing::new(RING_CAPACITY_SAMPLES)),
            tap: None,
        })
    }

    fn unavailable() -> RouteError {
        RouteError::Unavailable("AirPlay is unavailable on this Mac.".into())
    }
}

impl AirPlayRoute for MacAirPlayRoute {
    fn available(&self) -> bool {
        self.engine.is_some()
    }

    fn open_picker(&mut self, anchor: &PickerAnchor) -> Result<(), RouteError> {
        let engine = self.engine.as_ref().ok_or_else(Self::unavailable)?;
        airplay::open_picker(&self.app, engine.av(), anchor)
    }

    fn start(&mut self, mpv_pid: u32, playing: bool) -> Result<(), RouteError> {
        if self.engine.is_none() {
            return Err(Self::unavailable());
        }
        // A restart moves the tap to the new process.
        self.stop();
        let Some(engine) = self.engine.as_mut() else {
            return Err(Self::unavailable());
        };
        self.ring.clear();
        self.ring.set_open(playing);
        let tap = match ProcessTap::start(mpv_pid, self.ring.clone()) {
            Ok(tap) => tap,
            Err(error) => {
                self.ring.set_open(false);
                return Err(error);
            }
        };
        engine.start(self.ring.clone(), tap.sample_rate(), playing);
        self.tap = Some(tap);
        Ok(())
    }

    fn stop(&mut self) {
        let Some(engine) = self.engine.as_mut() else {
            return;
        };
        engine.stop();
        // Dropping the tap restores local sound.
        self.tap = None;
        self.ring.set_open(false);
        self.ring.clear();
    }

    fn set_playing(&mut self, playing: bool) {
        self.ring.set_open(playing);
        if let Some(engine) = &self.engine {
            engine.set_playing(playing);
        }
    }

    fn flush(&mut self) {
        if let Some(engine) = &self.engine {
            engine.request_resync();
        }
    }

    fn queued_ms(&self) -> f64 {
        self.engine.as_ref().map_or(0.0, Engine::queued_ms)
    }

    fn poll(&mut self) -> RoutePoll {
        self.engine.as_ref().map(Engine::poll).unwrap_or_default()
    }
}

impl Drop for MacAirPlayRoute {
    fn drop(&mut self) {
        self.stop();
    }
}
