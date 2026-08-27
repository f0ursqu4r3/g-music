use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

use crate::playback::{FakePlaybackProvider, PlaybackError, PlaybackSnapshot};

pub struct AppState {
    playback: Mutex<FakePlaybackProvider>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            playback: Mutex::new(FakePlaybackProvider::new()),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<PlaybackError> for CommandError {
    fn from(error: PlaybackError) -> Self {
        Self {
            code: "invalid_playback_input",
            message: error.to_string(),
        }
    }
}

fn with_playback<T>(
    state: &AppState,
    operation: impl FnOnce(&mut FakePlaybackProvider) -> Result<T, PlaybackError>,
) -> Result<T, CommandError> {
    let mut playback = state.playback.lock().map_err(|_| CommandError {
        code: "playback_unavailable",
        message: "The playback service is unavailable.".into(),
    })?;

    operation(&mut playback).map_err(CommandError::from)
}

#[tauri::command]
pub fn inspect_playback(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| Ok(playback.snapshot()))
}

#[tauri::command]
pub fn play(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.play();
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn pause(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.pause();
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn previous(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.previous();
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.next();
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn seek(
    state: State<'_, AppState>,
    position_ms: u64,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.seek(position_ms);
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn set_volume(
    state: State<'_, AppState>,
    volume_percent: u8,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.set_volume(volume_percent)?;
        Ok(playback.snapshot())
    })
}

#[tauri::command]
pub fn move_queue_item(
    state: State<'_, AppState>,
    from: usize,
    to: usize,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, |playback| {
        playback.move_queue_item(from, to)?;
        Ok(playback.snapshot())
    })
}
