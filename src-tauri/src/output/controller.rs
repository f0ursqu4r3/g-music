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
            state: OutputState {
                airplay_available,
                ..OutputState::default()
            },
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
            return Err(RouteError::Unavailable(
                "AirPlay is unavailable on this Mac.".into(),
            ));
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
                vec![
                    OutputEffect::Error { code, message },
                    OutputEffect::StateChanged(self.state()),
                ]
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
        if self.route_running {
            self.route.queued_ms()
        } else {
            0.0
        }
    }

    pub(crate) fn corrected_position_ms(&self, raw_ms: u64) -> u64 {
        super::clock::corrected_position_ms(raw_ms, self.queued_ms())
    }
}

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
        fn available(&self) -> bool {
            true
        }
        fn open_picker(&mut self, anchor: &PickerAnchor) -> Result<(), RouteError> {
            self.0
                .lock()
                .unwrap()
                .calls
                .push(format!("open_picker:{}", anchor.window_label));
            Ok(())
        }
        fn start(&mut self, mpv_pid: u32, playing: bool) -> Result<(), RouteError> {
            let mut log = self.0.lock().unwrap();
            log.calls.push(format!("start:{mpv_pid}:{playing}"));
            match log.start_error.take() {
                Some(e) => Err(e),
                None => Ok(()),
            }
        }
        fn stop(&mut self) {
            self.0.lock().unwrap().calls.push("stop".into());
        }
        fn set_playing(&mut self, playing: bool) {
            self.0
                .lock()
                .unwrap()
                .calls
                .push(format!("playing:{playing}"));
        }
        fn flush(&mut self) {
            self.0.lock().unwrap().calls.push("flush".into());
        }
        fn queued_ms(&self) -> f64 {
            self.0.lock().unwrap().queued_ms
        }
        fn poll(&mut self) -> RoutePoll {
            let log = self.0.lock().unwrap();
            RoutePoll {
                receivers: log.receivers.clone(),
                failed: log.failed,
            }
        }
    }

    fn controller() -> (OutputController, Arc<Mutex<FakeLog>>) {
        let log = Arc::new(Mutex::new(FakeLog::default()));
        (OutputController::new(Box::new(FakeRoute(log.clone()))), log)
    }

    fn anchor() -> PickerAnchor {
        PickerAnchor {
            window_label: "artwork".into(),
            x: 10.0,
            y: 20.0,
            width: 28.0,
            height: 28.0,
        }
    }

    fn state_change(effects: &[OutputEffect]) -> Option<&OutputState> {
        effects.iter().find_map(|e| match e {
            OutputEffect::StateChanged(s) => Some(s),
            _ => None,
        })
    }

    #[test]
    fn picking_a_receiver_enters_airplay() {
        let (mut c, log) = controller();
        c.mpv_started(42);
        c.set_playing(true);
        c.open_picker(&anchor()).unwrap();
        log.lock().unwrap().receivers = vec!["Master Bedroom".into()];
        let effects = c.tick(Instant::now());
        assert_eq!(
            state_change(&effects).unwrap().airplay_receivers,
            Some(vec!["Master Bedroom".into()])
        );
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
        assert!(effects.iter().any(|e| matches!(
            e,
            OutputEffect::Error {
                code: "airplay_permission_denied",
                ..
            }
        )));
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
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, OutputEffect::PausePlayback | OutputEffect::Notice(_)))
        );
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
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, OutputEffect::PausePlayback))
        );
        assert!(
            effects.iter().any(
                |e| matches!(e, OutputEffect::Notice(m) if m == "AirPlay speaker disconnected")
            )
        );
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
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, OutputEffect::PausePlayback))
        );
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
        assert_eq!(
            c.state().airplay_receivers,
            Some(vec!["Master Bedroom".into()])
        );
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
        assert_eq!(
            state_change(&effects).unwrap().airplay_receivers,
            Some(vec!["Family Room".into()])
        );
        assert_eq!(
            log.lock()
                .unwrap()
                .calls
                .iter()
                .filter(|c| c.starts_with("start"))
                .count(),
            1
        );
    }
}
