use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::playback::{
    DirtyTrack, EditableTrackMetadata, LibrarySnapshot, PlaybackSnapshot, PlaybackStatus,
    PlaybackTransport, Playlist, YouTubePlaybackError, YouTubePlaybackProvider,
    discover_youtube_imports, resolve_youtube_imports,
};
use crate::{auth, windows};

pub struct AppState {
    import_in_progress: Arc<AtomicBool>,
    metadata_refreshes: Arc<Mutex<MetadataRefreshState>>,
    next_import_id: AtomicU64,
    playback: Arc<Mutex<YouTubePlaybackProvider>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            import_in_progress: Arc::new(AtomicBool::new(false)),
            metadata_refreshes: Arc::new(Mutex::new(MetadataRefreshState::default())),
            next_import_id: AtomicU64::new(1),
            playback: Arc::new(Mutex::new(YouTubePlaybackProvider::new())),
        }
    }
}

impl AppState {
    pub fn from_library_directory(
        directory: std::path::PathBuf,
    ) -> Result<Self, YouTubePlaybackError> {
        Ok(Self {
            import_in_progress: Arc::new(AtomicBool::new(false)),
            metadata_refreshes: Arc::new(Mutex::new(MetadataRefreshState::default())),
            next_import_id: AtomicU64::new(1),
            playback: Arc::new(Mutex::new(YouTubePlaybackProvider::from_library_directory(
                directory,
            )?)),
        })
    }

    pub fn start_dirty_refresh(&self, app: AppHandle) {
        let tracks = match self.playback.lock() {
            Ok(playback) => playback.dirty_tracks(),
            Err(_) => {
                tracing::error!(
                    "could not inspect dirty tracks because the playback mutex is poisoned"
                );
                return;
            }
        };
        schedule_metadata_refreshes(&app, &self.playback, &self.metadata_refreshes, tracks);
    }

    pub fn shutdown(&self) {
        match self.playback.lock() {
            Ok(mut playback) => playback.shutdown(),
            Err(_) => tracing::error!("could not stop mpv because the playback mutex is poisoned"),
        }
    }

    pub fn library_snapshot(&self) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "inspect_library", |playback| {
            Ok(playback.library_snapshot())
        })
    }

    pub fn update_track_metadata(
        &self,
        id: &str,
        metadata: EditableTrackMetadata,
    ) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "update_track_metadata", |playback| {
            playback.update_track_metadata(id, metadata)
        })
    }

    pub fn update_tracks_metadata(
        &self,
        updates: Vec<(String, EditableTrackMetadata)>,
    ) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "update_tracks_metadata", |playback| {
            playback.update_tracks_metadata(updates)
        })
    }

    pub fn remove_tracks(&self, ids: &[String]) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "remove_tracks", |playback| {
            playback.remove_tracks(ids)
        })
    }

    pub fn upsert_playlist(&self, playlist: Playlist) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "upsert_playlist", |playback| {
            playback.upsert_playlist(playlist)
        })
    }

    pub fn delete_playlist(&self, id: &str) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "delete_playlist", |playback| {
            playback.delete_playlist(id)
        })
    }

    pub fn move_queue_item(
        &self,
        from: usize,
        to: usize,
    ) -> Result<PlaybackSnapshot, CommandError> {
        with_playback(self, "move_queue_item", |playback| {
            playback.move_queue_item(from, to)?;
            playback.snapshot()
        })
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportProgress {
    completed_sources: usize,
    imported_tracks: usize,
    message: String,
    phase: &'static str,
    run_id: u64,
    skipped_member_only: usize,
    total_sources: usize,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshJob {
    message: String,
    state: &'static str,
    track_id: String,
    title: String,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRefreshSnapshot {
    completed_tracks: usize,
    jobs: Vec<MetadataRefreshJob>,
    total_tracks: usize,
}

#[derive(Default)]
struct MetadataRefreshState {
    completed_tracks: usize,
    jobs: Vec<MetadataRefreshJob>,
    pending: VecDeque<DirtyTrack>,
    queued_ids: HashSet<String>,
    running: bool,
    total_tracks: usize,
}

impl MetadataRefreshState {
    fn enqueue(&mut self, tracks: Vec<DirtyTrack>) -> bool {
        for track in tracks {
            if !self.queued_ids.insert(track.id.clone()) {
                continue;
            }
            self.total_tracks += 1;
            self.jobs.push(MetadataRefreshJob {
                message: "Waiting for metadata refresh.".into(),
                state: "queued",
                track_id: track.id.clone(),
                title: track.title.clone(),
            });
            self.pending.push_back(track);
        }
        if self.pending.is_empty() || self.running {
            return false;
        }

        self.running = true;
        true
    }

    fn take_next(&mut self) -> Option<MetadataRefreshJob> {
        let track = self.pending.pop_front()?;
        let job = self
            .jobs
            .iter_mut()
            .find(|job| job.track_id == track.id && job.state == "queued")?;
        job.message = "Fetching full YouTube metadata.".into();
        job.state = "refreshing";
        Some(job.clone())
    }

    fn finish(&mut self, track_id: &str, result: Result<bool, YouTubePlaybackError>) {
        let Some(job) = self
            .jobs
            .iter_mut()
            .find(|job| job.track_id == track_id && job.state == "refreshing")
        else {
            return;
        };
        self.queued_ids.remove(track_id);
        self.completed_tracks += 1;
        match result {
            Ok(true) => {
                job.message = "Full metadata saved to the library.".into();
                job.state = "completed";
            }
            Ok(false) => {
                job.message = "Metadata was already refreshed.".into();
                job.state = "completed";
            }
            Err(error) => {
                job.message = error.to_string();
                job.state = "failed";
            }
        }
    }

    fn finish_worker(&mut self) {
        self.running = false;
    }

    fn snapshot(&self) -> MetadataRefreshSnapshot {
        MetadataRefreshSnapshot {
            completed_tracks: self.completed_tracks,
            jobs: self.jobs.clone(),
            total_tracks: self.total_tracks,
        }
    }
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
    if matches!(
        operation_name,
        "inspect_playback" | "inspect_playback_transport"
    ) {
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
            if matches!(
                operation_name,
                "inspect_playback" | "inspect_playback_transport"
            ) {
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

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, item_id: &str) {
    if item_id == "help.keyboard-shortcuts" {
        if let Err(error) = app.emit("show-keyboard-shortcuts", ()) {
            tracing::error!(%error, "could not open keyboard shortcuts from the native menu");
        }
        return;
    }

    let Some(state) = app.try_state::<AppState>() else {
        tracing::error!(
            item_id,
            "native menu action ran before application state was ready"
        );
        return;
    };
    let snapshot = match item_id {
        "playback.toggle" => with_playback(&state, "menu_toggle_playback", |playback| {
            if playback.transport()?.status == PlaybackStatus::Playing {
                playback.pause()?;
            } else {
                playback.play()?;
            }
            playback.snapshot()
        }),
        "playback.previous" => with_playback(&state, "menu_previous_track", |playback| {
            playback.previous_track()?;
            playback.snapshot()
        }),
        "playback.next" => with_playback(&state, "menu_next_track", |playback| {
            playback.next_track()?;
            playback.snapshot()
        }),
        _ => return,
    };

    match snapshot {
        Ok(snapshot) => {
            if let Err(error) = app.emit("playback-updated", snapshot) {
                tracing::error!(item_id, %error, "could not broadcast native menu playback update");
            }
        }
        Err(error) => {
            tracing::error!(item_id, %error.message, "native menu playback action failed")
        }
    }
}

#[tauri::command]
pub fn inspect_metadata_refreshes(
    state: State<'_, AppState>,
) -> Result<MetadataRefreshSnapshot, CommandError> {
    state
        .metadata_refreshes
        .lock()
        .map(|refreshes| refreshes.snapshot())
        .map_err(|_| CommandError {
            code: "refresh_unavailable",
            message: "The metadata refresh service is unavailable.".into(),
        })
}

fn schedule_metadata_refreshes(
    app: &AppHandle,
    playback: &Arc<Mutex<YouTubePlaybackProvider>>,
    metadata_refreshes: &Arc<Mutex<MetadataRefreshState>>,
    tracks: Vec<DirtyTrack>,
) {
    let should_start = match metadata_refreshes.lock() {
        Ok(mut refreshes) => refreshes.enqueue(tracks),
        Err(_) => {
            tracing::error!(
                "could not queue metadata refreshes because the refresh mutex is poisoned"
            );
            return;
        }
    };
    if !should_start {
        return;
    }

    let app = app.clone();
    let playback = Arc::clone(playback);
    let worker_refreshes = Arc::clone(metadata_refreshes);
    if let Err(error) = thread::Builder::new()
        .name("youtube-metadata-refresh".into())
        .spawn(move || run_metadata_refresh_worker(app, playback, worker_refreshes))
    {
        tracing::error!(%error, "could not start metadata refresh worker");
        if let Ok(mut refreshes) = metadata_refreshes.lock() {
            refreshes.finish_worker();
        }
    }
}

fn run_metadata_refresh_worker(
    app: AppHandle,
    playback: Arc<Mutex<YouTubePlaybackProvider>>,
    metadata_refreshes: Arc<Mutex<MetadataRefreshState>>,
) {
    let cookie_path = auth::session_cookie_path(&app)
        .ok()
        .filter(|path| auth::session_exists(path));
    if let Ok(mut playback) = playback.lock() {
        playback.set_session_cookie_path(cookie_path.clone());
    }

    loop {
        let job = match metadata_refreshes.lock() {
            Ok(mut refreshes) => refreshes.take_next(),
            Err(_) => {
                tracing::error!("metadata refresh mutex is poisoned");
                return;
            }
        };
        let Some(job) = job else {
            if let Ok(mut refreshes) = metadata_refreshes.lock() {
                refreshes.finish_worker();
                emit_metadata_refresh_progress(&app, &refreshes);
            }
            return;
        };

        tracing::info!(
            track_id = job.track_id,
            title = job.title,
            "refreshing dirty YouTube metadata"
        );
        if let Ok(refreshes) = metadata_refreshes.lock() {
            emit_metadata_refresh_progress(&app, &refreshes);
        }
        let source_url = playback
            .lock()
            .map_err(|_| {
                YouTubePlaybackError::Library("the playback service is unavailable".into())
            })
            .map(|playback| playback.dirty_track_source(&job.track_id));
        let result = match source_url {
            Err(error) => Err(error),
            Ok(Some(source_url)) => {
                resolve_youtube_imports(&[source_url], cookie_path.as_deref(), |_, _, _, _, _| {})
                    .and_then(|resolved| {
                        playback
                            .lock()
                            .map_err(|_| {
                                YouTubePlaybackError::Library(
                                    "the playback service is unavailable".into(),
                                )
                            })
                            .and_then(|mut playback| playback.commit_youtube_import(resolved))
                            .map(|_| true)
                    })
            }
            Ok(None) => Ok(false),
        };
        if let Ok(mut refreshes) = metadata_refreshes.lock() {
            refreshes.finish(&job.track_id, result);
            emit_metadata_refresh_progress(&app, &refreshes);
        }
    }
}

fn emit_metadata_refresh_progress(app: &AppHandle, refreshes: &MetadataRefreshState) {
    if let Err(error) = app.emit("metadata-refresh-progress", refreshes.snapshot()) {
        tracing::debug!(%error, "could not deliver metadata refresh progress");
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
pub fn inspect_playback_transport(
    state: State<'_, AppState>,
) -> Result<PlaybackTransport, CommandError> {
    with_playback(
        &state,
        "inspect_playback_transport",
        YouTubePlaybackProvider::transport,
    )
}

#[tauri::command]
pub fn inspect_library(state: State<'_, AppState>) -> Result<LibrarySnapshot, CommandError> {
    state.library_snapshot()
}

#[tauri::command]
pub async fn resolve_youtube_artwork(
    app: AppHandle,
    video_id: String,
) -> Result<Option<String>, CommandError> {
    crate::artwork::resolve_youtube_artwork(&app, &video_id)
        .await
        .map_err(|message| CommandError {
            code: "artwork_unavailable",
            message,
        })
}

#[tauri::command]
pub fn import_youtube_urls(
    app: AppHandle,
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<(), CommandError> {
    let cookie_path = auth::session_cookie_path(&app).map_err(CommandError::from)?;
    let cookie_path = auth::session_exists(&cookie_path).then_some(cookie_path);
    if state
        .import_in_progress
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(CommandError {
            code: "import_in_progress",
            message: "A library import is already running.".into(),
        });
    }

    let run_id = state.next_import_id.fetch_add(1, Ordering::Relaxed);
    let total_sources = urls.len();
    let playback = Arc::clone(&state.playback);
    let import_in_progress = Arc::clone(&state.import_in_progress);
    let metadata_refreshes = Arc::clone(&state.metadata_refreshes);
    tracing::info!(
        authenticated = cookie_path.is_some(),
        run_id,
        sources = total_sources,
        "YouTube batch import queued"
    );

    let worker = thread::Builder::new()
        .name("youtube-import".into())
        .spawn(move || {
            emit_import_progress(
                &app,
                ImportProgress {
                    completed_sources: 0,
                    imported_tracks: 0,
                    message: format!("Import worker started for {total_sources} source(s)."),
                    phase: "started",
                    run_id,
                    skipped_member_only: 0,
                    total_sources,
                },
            );

            let result = discover_youtube_imports(
                &urls,
                cookie_path.as_deref(),
                |completed_sources, total_sources, resolved, imported_tracks, _| {
                    emit_import_progress(
                        &app,
                        ImportProgress {
                            completed_sources,
                            imported_tracks,
                            message: if resolved {
                                format!(
                                    "Discovered {imported_tracks} track(s) across {completed_sources} source(s)."
                                )
                            } else {
                                format!(
                                    "Discovering source {} of {total_sources}; {imported_tracks} track(s) found so far…",
                                    completed_sources + 1
                                )
                            },
                            phase: "resolving",
                            run_id,
                            skipped_member_only: 0,
                            total_sources,
                        },
                    );
                },
            )
            .and_then(|discovered| {
                let discovered_tracks = discovered.track_count();
                let mut provider = playback.lock().map_err(|_| {
                    YouTubePlaybackError::Library("the playback service is unavailable".into())
                })?;
                provider.set_session_cookie_path(cookie_path.clone());
                provider.commit_youtube_import(discovered)?;
                drop(provider);
                emit_import_progress(
                    &app,
                    ImportProgress {
                        completed_sources: total_sources,
                        imported_tracks: discovered_tracks,
                        message: format!(
                            "Saved {discovered_tracks} discovered track(s). Enriching metadata in the background…"
                        ),
                        phase: "merging",
                        run_id,
                        skipped_member_only: 0,
                        total_sources,
                    },
                );
                Ok(discovered_tracks)
            })
            .and_then(|_discovered_tracks| resolve_youtube_imports(
                &urls,
                cookie_path.as_deref(),
                |completed_sources, total_sources, resolved, imported_tracks, skipped_member_only| {
                    let message = if resolved {
                        if skipped_member_only == 0 {
                            format!(
                                "Resolved source {completed_sources} of {total_sources}; {imported_tracks} track(s) found."
                            )
                        } else {
                            format!(
                                "Resolved source {completed_sources} of {total_sources}; {imported_tracks} track(s) found, {skipped_member_only} members-only track(s) skipped."
                            )
                        }
                    } else {
                        format!(
                            "Resolving source {} of {total_sources}; {imported_tracks} track(s) found so far…",
                            completed_sources + 1
                        )
                    };
                    emit_import_progress(
                        &app,
                        ImportProgress {
                            completed_sources,
                            imported_tracks,
                            message,
                            phase: "resolving",
                            run_id,
                            skipped_member_only,
                            total_sources,
                        },
                    );
                },
            ))
            .and_then(|resolved| {
                let imported_tracks = resolved.track_count();
                let skipped_member_only = resolved.skipped_member_only_count();
                let timed_out_sources = resolved.timed_out_source_count();
                emit_import_progress(
                    &app,
                    ImportProgress {
                        completed_sources: total_sources,
                        imported_tracks,
                        message: if timed_out_sources == 0 {
                            "Saving imported tracks to the library…".into()
                        } else {
                            format!(
                                "Saving {imported_tracks} collected track(s); {timed_out_sources} source(s) stopped after inactivity."
                            )
                        },
                        phase: "merging",
                        run_id,
                        skipped_member_only,
                        total_sources,
                    },
                );
                let mut provider = playback.lock().map_err(|_| {
                    YouTubePlaybackError::Library("the playback service is unavailable".into())
                })?;
                provider.set_session_cookie_path(cookie_path);
                provider.commit_youtube_import(resolved)?;
                Ok((imported_tracks, skipped_member_only, timed_out_sources))
            });

            match result {
                Ok((imported_tracks, skipped_member_only, timed_out_sources)) => {
                    tracing::info!(run_id, imported_tracks, skipped_member_only, timed_out_sources, "YouTube batch import completed");
                    emit_import_progress(
                        &app,
                        ImportProgress {
                            completed_sources: total_sources,
                            imported_tracks,
                            message: if timed_out_sources > 0 {
                                format!(
                                    "Imported {imported_tracks} track(s) before {timed_out_sources} source(s) stopped after inactivity."
                                )
                            } else if skipped_member_only == 0 {
                                format!("Imported {imported_tracks} track(s) into the library.")
                            } else {
                                format!(
                                    "Imported {imported_tracks} track(s); skipped {skipped_member_only} members-only track(s)."
                                )
                            },
                            phase: "completed",
                            run_id,
                            skipped_member_only,
                            total_sources,
                        },
                    );
                }
                Err(error) => {
                    tracing::error!(run_id, %error, "YouTube batch import failed");
                    emit_import_progress(
                        &app,
                        ImportProgress {
                            completed_sources: 0,
                            imported_tracks: 0,
                            message: error.to_string(),
                            phase: "failed",
                            run_id,
                            skipped_member_only: 0,
                            total_sources,
                        },
                    );
                    let tracks = playback
                        .lock()
                        .map(|provider| provider.dirty_tracks())
                        .unwrap_or_default();
                    schedule_metadata_refreshes(
                        &app,
                        &playback,
                        &metadata_refreshes,
                        tracks,
                    );
                }
            }

            import_in_progress.store(false, Ordering::Release);
        });

    if let Err(error) = worker {
        state.import_in_progress.store(false, Ordering::Release);
        return Err(CommandError {
            code: "import_worker_failed",
            message: error.to_string(),
        });
    }

    Ok(())
}

fn emit_import_progress(app: &AppHandle, progress: ImportProgress) {
    if let Err(error) = app.emit_to("import", "import-progress", progress) {
        tracing::debug!(%error, "could not deliver import progress to the import window");
    }
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
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<PlaybackSnapshot, CommandError> {
    let snapshot = with_playback(&state, "play_track", |playback| {
        playback.play_track(&id)?;
        playback.snapshot()
    })?;
    if snapshot
        .current_item
        .as_ref()
        .is_some_and(|item| item.metadata_dirty)
    {
        state.start_dirty_refresh(app);
    }

    Ok(snapshot)
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
pub fn set_volume(state: State<'_, AppState>, volume_percent: u8) -> Result<(), CommandError> {
    with_playback(&state, "set_volume", |playback| {
        playback.set_volume(volume_percent)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing::{
        Dispatch, Event, Id, Level, Metadata, Subscriber, dispatcher::with_default,
        subscriber::Interest,
    };

    use crate::playback::{EditableTrackMetadata, Playlist};

    use super::{AppState, with_playback};

    struct RecordingSubscriber {
        levels: Arc<Mutex<Vec<Level>>>,
    }

    impl Subscriber for RecordingSubscriber {
        fn enabled(&self, _: &Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> Id {
            Id::from_u64(1)
        }

        fn record(&self, _: &Id, _: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _: &Id, _: &Id) {}

        fn event(&self, event: &Event<'_>) {
            self.levels
                .lock()
                .expect("test recording mutex is available")
                .push(*event.metadata().level());
        }

        fn enter(&self, _: &Id) {}

        fn exit(&self, _: &Id) {}

        fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
            Interest::always()
        }
    }

    #[test]
    fn transport_inspection_does_not_emit_debug_command_logs() {
        let state = AppState::default();
        let levels = Arc::new(Mutex::new(Vec::new()));
        let subscriber = RecordingSubscriber {
            levels: Arc::clone(&levels),
        };

        with_default(&Dispatch::new(subscriber), || {
            with_playback(
                &state,
                "inspect_playback_transport",
                crate::playback::YouTubePlaybackProvider::transport,
            )
            .expect("transport inspection succeeds");
        });

        assert!(
            levels
                .lock()
                .expect("test recording mutex is available")
                .iter()
                .all(|level| *level != Level::DEBUG),
            "transport inspection must not emit debug command logs",
        );
    }

    #[test]
    fn agent_library_operations_return_durable_library_snapshots() {
        let state = AppState::default();

        let library = state
            .library_snapshot()
            .expect("library inspection succeeds");
        assert!(library.tracks.is_empty());
        assert!(library.playlists.is_empty());

        let error = state
            .update_track_metadata(
                "missing",
                EditableTrackMetadata {
                    title: "Edited".into(),
                    artist: "Artist".into(),
                    album: None,
                    label: None,
                    genres: vec![],
                },
            )
            .expect_err("unknown tracks are rejected");
        assert_eq!(error.code, "youtube_playback_failed");

        let error = state
            .upsert_playlist(Playlist {
                id: "focus".into(),
                name: "Focus".into(),
                track_ids: vec!["missing".into()],
            })
            .expect_err("playlists cannot reference missing tracks");
        assert_eq!(error.code, "youtube_playback_failed");
    }
}
