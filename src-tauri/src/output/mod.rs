//! Audio output selection: local devices through mpv and AirPlay on macOS.

pub(crate) mod devices;

// Wired to mpv and macOS in Tasks 4 and 7.
#[allow(dead_code)]
pub(crate) mod clock;
#[allow(dead_code)]
pub(crate) mod controller;
#[allow(dead_code)]
pub(crate) mod ring;

pub(crate) use controller::OutputState;

/// Merges the latest local device list into the shared output state.
pub(crate) fn local_state(
    devices: Vec<devices::OutputDevice>,
    selected_device_id: Option<String>,
) -> OutputState {
    OutputState {
        devices,
        selected_device_id,
        ..OutputState::default()
    }
}

/// Returns to Local mode when AirPlay is active. No-op until Task 5.
pub(crate) fn leave_airplay() {}
