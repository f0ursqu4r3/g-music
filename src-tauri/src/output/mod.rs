//! Audio output selection: local devices through mpv and AirPlay on macOS.

use serde::Serialize;

pub(crate) mod devices;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OutputState {
    pub(crate) devices: Vec<devices::OutputDevice>,
    pub(crate) selected_device_id: Option<String>,
    pub(crate) airplay_available: bool,
    /// `Some` means AirPlay mode. The names come from the active route.
    pub(crate) airplay_receivers: Option<Vec<String>>,
}

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
