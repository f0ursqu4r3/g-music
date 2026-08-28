use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::playback::{PlaybackSnapshot, YouTubePlaybackError, YouTubePlaybackProvider};
use crate::{auth, windows};

pub struct AppState {
    playback: Mutex<YouTubePlaybackProvider>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            playback: Mutex::new(YouTubePlaybackProvider::new()),
        }
    }
}

impl AppState {
    pub fn from_library_path(path: std::path::PathBuf) -> Result<Self, YouTubePlaybackError> {
        Ok(Self {
            playback: Mutex::new(YouTubePlaybackProvider::from_library_path(path)?),
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<YouTubePlaybackError> for CommandError {
    fn from(error: YouTubePlaybackError) -> Self {
        Self {
            code: "youtube_playback_failed",
            message: error.to_string(),
        }
    }
}

impl From<auth::YouTubeAuthError> for CommandError {
    fn from(error: auth::YouTubeAuthError) -> Self {
        Self {
            code: "youtube_auth_failed",
            message: error.to_string(),
        }
    }
}

fn with_playback<T>(
    state: &AppState,
    operation_name: &'static str,
    operation: impl FnOnce(&mut YouTubePlaybackProvider) -> Result<T, YouTubePlaybackError>,
) -> Result<T, CommandError> {
    if operation_name == "inspect_playback" {
        tracing::trace!(operation = operation_name, "playback command started");
    } else {
        tracing::debug!(operation = operation_name, "playback command started");
    }
    let mut playback = state.playback.lock().map_err(|_| {
        tracing::error!(operation = operation_name, "playback mutex is poisoned");
        CommandError {
            code: "playback_unavailable",
            message: "The playback service is unavailable.".into(),
        }
    })?;

    match operation(&mut playback) {
        Ok(value) => {
            if operation_name == "inspect_playback" {
                tracing::trace!(operation = operation_name, "playback command completed");
            } else {
                tracing::debug!(operation = operation_name, "playback command completed");
            }
            Ok(value)
        }
        Err(error) => {
            tracing::error!(operation = operation_name, %error, "playback command failed");
            Err(CommandError::from(error))
        }
    }
}

#[tauri::command]
pub fn inspect_playback(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(
        &state,
        "inspect_playback",
        YouTubePlaybackProvider::snapshot,
    )
}

#[tauri::command]
pub fn import_youtube_urls(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<PlaybackSnapshot, CommandError> {
    let cookie_path = auth::session_cookie_path(&app).map_err(CommandError::from)?;
    let cookie_path = auth::session_exists(&cookie_path).then_some(cookie_path);
    tracing::info!(
        authenticated = cookie_path.is_some(),
        sources = urls.len(),
        "YouTube batch import requested"
    );
    with_playback(&state, "import_youtube_urls", |playback| {
        playback.set_session_cookie_path(cookie_path);
        playback.import_youtube_urls(&urls)
    })
}

#[tauri::command]
pub fn show_import_window(app: AppHandle) -> Result<(), CommandError> {
    windows::show_import(&app).map_err(|error| CommandError {
        code: "import_window_failed",
        message: error.to_string(),
    })
}

#[tauri::command]
pub fn inspect_youtube_auth(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    let cookie_path = auth::session_cookie_path(&app).map_err(CommandError::from)?;
    Ok(auth::YouTubeAuthStatus {
        connected: auth::session_exists(&cookie_path),
    })
}

#[tauri::command]
pub fn open_youtube_login(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    tracing::info!("opening YouTube login window");
    windows::show_youtube_login(&app).map_err(|error| CommandError {
        code: "youtube_login_window_failed",
        message: error.to_string(),
    })?;
    inspect_youtube_auth(app)
}

#[tauri::command]
pub fn save_youtube_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<auth::YouTubeAuthStatus, CommandError> {
    let window = app
        .get_webview_window("youtube-auth")
        .ok_or(auth::YouTubeAuthError::LoginWindowNotOpen)
        .map_err(CommandError::from)?;
    let cookie_path = auth::session_cookie_path(&app).map_err(CommandError::from)?;
    auth::save_session_from_window(&window, &cookie_path).map_err(CommandError::from)?;
    with_playback(&state, "save_youtube_session", |playback| {
        playback.set_session_cookie_path(Some(cookie_path));
        Ok(())
    })?;
    window.close().map_err(|error| CommandError {
        code: "youtube_login_window_failed",
        message: error.to_string(),
    })?;
    tracing::info!("YouTube session saved");

    Ok(auth::YouTubeAuthStatus { connected: true })
}

#[tauri::command]
pub fn disconnect_youtube(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<auth::YouTubeAuthStatus, CommandError> {
    let cookie_path = auth::session_cookie_path(&app).map_err(CommandError::from)?;
    auth::disconnect_session(&app, &cookie_path).map_err(CommandError::from)?;
    with_playback(&state, "disconnect_youtube", |playback| {
        playback.set_session_cookie_path(None);
        Ok(())
    })?;
    tracing::info!("YouTube session disconnected");

    Ok(auth::YouTubeAuthStatus { connected: false })
}

#[tauri::command]
pub fn play(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "play", |playback| {
        playback.play()?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn pause(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "pause", |playback| {
        playback.pause()?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn previous(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "previous", |playback| {
        playback.previous_track()?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn next(state: State<'_, AppState>) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "next", |playback| {
        playback.next_track()?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn play_track(
    state: State<'_, AppState>,
    id: String,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "play_track", |playback| {
        playback.play_track(&id)?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn seek(
    state: State<'_, AppState>,
    position_ms: u64,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "seek", |playback| {
        playback.seek(position_ms)?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn set_volume(
    state: State<'_, AppState>,
    volume_percent: u8,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "set_volume", |playback| {
        playback.set_volume(volume_percent)?;
        playback.snapshot()
    })
}

#[tauri::command]
pub fn move_queue_item(
    state: State<'_, AppState>,
    from: usize,
    to: usize,
) -> Result<PlaybackSnapshot, CommandError> {
    with_playback(&state, "move_queue_item", |playback| {
        playback.move_queue_item(from, to)?;
        playback.snapshot()
    })
}
