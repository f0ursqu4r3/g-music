use std::{
    collections::{HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::playback::{
    DirtyTrack, EditableTrackMetadata, LibrarySnapshot, PlaybackSnapshot, PlaybackStatus,
    PlaybackTransport, Playlist, YouTubePlaybackError, YouTubePlaybackProvider,
    resolve_youtube_imports,
};
use crate::{auth, windows};

pub struct AppState {
    active_import: Arc<Mutex<Option<ImportRun>>>,
    import_progress: Mutex<Option<ImportProgress>>,
    metadata_refreshes: Arc<Mutex<MetadataRefreshState>>,
    next_import_id: AtomicU64,
    playback: Arc<Mutex<YouTubePlaybackProvider>>,
    transport: Arc<Mutex<()>>,
    shutting_down: Arc<AtomicBool>,
    pause_epoch: AtomicU64,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active_import: Arc::new(Mutex::new(None)),
            import_progress: Mutex::new(None),
            metadata_refreshes: Arc::new(Mutex::new(MetadataRefreshState::default())),
            next_import_id: AtomicU64::new(1),
            playback: Arc::new(Mutex::new(YouTubePlaybackProvider::new())),
            transport: Arc::new(Mutex::new(())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            pause_epoch: AtomicU64::new(0),
        }
    }
}

impl AppState {
    pub fn from_library_directory(
        directory: std::path::PathBuf,
    ) -> Result<Self, YouTubePlaybackError> {
        Ok(Self {
            active_import: Arc::new(Mutex::new(None)),
            import_progress: Mutex::new(None),
            metadata_refreshes: Arc::new(Mutex::new(MetadataRefreshState::default())),
            next_import_id: AtomicU64::new(1),
            playback: Arc::new(Mutex::new(YouTubePlaybackProvider::from_library_directory(
                directory,
            )?)),
            transport: Arc::new(Mutex::new(())),
            shutting_down: Arc::new(AtomicBool::new(false)),
            pause_epoch: AtomicU64::new(0),
        })
    }

    pub fn start_dirty_refresh(&self, app: AppHandle) {
        let cookie_path = auth::session_cookie_path(&app)
            .ok()
            .filter(|path| auth::session_exists(path));
        let tracks = match self.playback.lock() {
            Ok(mut playback) => {
                if playback.set_session_cookie_path(cookie_path).is_err() {
                    tracing::warn!("Could not initialize the playback session.");
                }
                playback.dirty_tracks()
            }
            Err(_) => {
                tracing::error!(
                    "could not inspect dirty tracks because the playback mutex is poisoned"
                );
                return;
            }
        };
        schedule_metadata_refreshes(&app, &self.playback, &self.metadata_refreshes, tracks);
    }

    pub fn begin_shutdown(&self) -> bool {
        self.pause_epoch.fetch_add(1, Ordering::AcqRel);
        !self.shutting_down.swap(true, Ordering::AcqRel)
    }

    pub fn shutdown(&self) {
        crate::process::shutdown();
        self.shutting_down.store(true, Ordering::Release);
        if let Ok(active) = self.active_import.lock()
            && let Some(run) = active.as_ref()
        {
            run.cancelled.store(true, Ordering::Release);
        }
        let _serial = self.transport.lock();
        match self.playback.lock() {
            Ok(mut playback) => playback.shutdown(),
            Err(_) => tracing::error!("could not stop mpv because the playback mutex is poisoned"),
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self
            .active_import
            .lock()
            .is_ok_and(|active| active.is_some())
            || self
                .metadata_refreshes
                .lock()
                .is_ok_and(|refreshes| refreshes.running)
        {
            if std::time::Instant::now() >= deadline {
                tracing::error!("background workers did not finish within the shutdown deadline");
                break;
            }
            thread::sleep(std::time::Duration::from_millis(25));
        }
    }

    pub fn start_transport_monitor(&self, app: AppHandle) {
        let stopped = self.shutting_down.clone();
        thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let state = app.state::<AppState>();
                if let Ok(_serial) = state.transport.try_lock() {
                    let prepared = state.playback.lock().ok().and_then(|mut playback| {
                        (playback.cached_transport().current_item.is_some())
                            .then(|| playback.prepare_transport())
                    });
                    if let Some(mut prepared) = prepared {
                        let epoch = state.pause_epoch.load(Ordering::Acquire);
                        let before = prepared.cached_snapshot();
                        let mut result = prepared.transport_with_snapshot_update();
                        if (result.is_err() || epoch != state.pause_epoch.load(Ordering::Acquire))
                            && let Err(error) = prepared.halt_after_error()
                        {
                            result = Err(error);
                        }
                        let library_changed = prepared.has_pending_plays();
                        if let Ok(mut playback) = state.playback.lock() {
                            match playback.commit_transport(prepared) {
                                Ok(snapshot) => {
                                    if library_changed {
                                        let _ = app.emit("library-updated", ());
                                    }
                                    if before.current_item != snapshot.current_item
                                        || before.queue != snapshot.queue
                                        || before.status != snapshot.status
                                    {
                                        emit_playback_updated(&app, &snapshot);
                                    }
                                }
                                Err(error) => {
                                    emit_playback_updated(&app, &playback.cached_snapshot());
                                    let _ = app.emit("playback-error", CommandError::from(error));
                                }
                            }
                        }
                        if let Err(error) = result {
                            let _ = app.emit("playback-error", CommandError::from(error));
                        }
                    }
                }
                thread::sleep(std::time::Duration::from_millis(500));
            }
        });
    }

    pub fn library_snapshot(&self) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "inspect_library", |playback| {
            Ok(playback.library_snapshot())
        })
    }

    pub fn emit_cached_playback<R: Runtime>(&self, app: &AppHandle<R>) {
        if let Ok(playback) = self.playback.lock() {
            emit_playback_updated(app, &playback.cached_snapshot());
        }
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

    pub fn toggle_favorite(&self, id: &str) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "toggle_favorite", |playback| {
            playback.toggle_favorite(id)
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

    pub fn reorder_playlists(
        &self,
        playlist_ids: &[String],
    ) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "reorder_playlists", |playback| {
            playback.reorder_playlists(playlist_ids)
        })
    }

    pub fn delete_playlist(&self, id: &str) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "delete_playlist", |playback| {
            playback.delete_playlist(id)
        })
    }

    pub fn move_library_item(
        &self,
        from: usize,
        to: usize,
    ) -> Result<LibrarySnapshot, CommandError> {
        with_playback(self, "move_library_item", |playback| {
            playback.move_library_item(from, to)
        })
    }

    pub fn move_queue_item(
        &self,
        from: usize,
        to: usize,
    ) -> Result<PlaybackSnapshot, CommandError> {
        let _serial = self.transport.lock().map_err(|_| unavailable())?;
        with_playback(self, "move_queue_item", |playback| {
            playback.move_queue_item(from, to)?;
            Ok(playback.cached_snapshot())
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

struct ImportRun {
    id: u64,
    cancelled: Arc<AtomicBool>,
}

struct ImportCompletion {
    active: Arc<Mutex<Option<ImportRun>>>,
    run_id: u64,
}

impl Drop for ImportCompletion {
    fn drop(&mut self) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.as_ref().is_some_and(|run| run.id == self.run_id) {
            *active = None;
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadataUpdate {
    id: String,
    metadata: serde_json::Value,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
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

    fn fail_worker(&mut self) {
        for job in &mut self.jobs {
            if matches!(job.state, "queued" | "refreshing") {
                job.state = "failed";
                job.message = "Metadata worker stopped. Retry the refresh.".into();
                self.completed_tracks += 1;
            }
        }
        self.pending.clear();
        self.queued_ids.clear();
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
            code: if matches!(error, YouTubePlaybackError::SessionExpired) {
                "youtube_session_expired"
            } else {
                "youtube_playback_failed"
            },
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

fn unavailable() -> CommandError {
    CommandError {
        code: "playback_unavailable",
        message: "The playback service is unavailable.".into(),
    }
}

pub(crate) fn cached_transport(app: &AppHandle) -> Result<PlaybackTransport, CommandError> {
    Ok(app
        .state::<AppState>()
        .playback
        .lock()
        .map_err(|_| unavailable())?
        .cached_transport())
}

pub(crate) fn native_pause_hint(app: &AppHandle) {
    app.state::<AppState>()
        .pause_epoch
        .fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn native_transport(
    app: &AppHandle,
    action: crate::media_session::NativeAction,
) -> Result<PlaybackSnapshot, CommandError> {
    use crate::media_session::NativeAction;
    transport_action(&app.state::<AppState>(), app, |playback| match action {
        NativeAction::Play => playback.play(),
        NativeAction::Toggle if playback.cached_transport().status != PlaybackStatus::Playing => {
            playback.play()
        }
        NativeAction::Toggle | NativeAction::Pause | NativeAction::Sleep | NativeAction::Wake => {
            playback.pause()
        }
        NativeAction::Next => playback.next_track(),
        NativeAction::Previous => playback.previous_track(),
        NativeAction::Seek(position) => playback.seek(position),
        NativeAction::Stop => playback.clear_queue().map(|_| ()),
    })
}

async fn blocking<T: Send + 'static>(
    app: AppHandle,
    operation: impl FnOnce(&AppState, &AppHandle) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(move || operation(&app.state::<AppState>(), &app))
        .await
        .map_err(|_| unavailable())?
}

fn transport_action<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    operation: impl FnOnce(&mut YouTubePlaybackProvider) -> Result<(), YouTubePlaybackError>,
) -> Result<PlaybackSnapshot, CommandError> {
    let result = perform_transport_action(state, app, operation);
    if let Err(error) = &result {
        let _ = app.emit("playback-error", error);
    }
    result
}

fn perform_transport_action<R: Runtime>(
    state: &AppState,
    app: &AppHandle<R>,
    operation: impl FnOnce(&mut YouTubePlaybackProvider) -> Result<(), YouTubePlaybackError>,
) -> Result<PlaybackSnapshot, CommandError> {
    if state.shutting_down.load(Ordering::Acquire) {
        return Err(unavailable());
    }
    let epoch = state.pause_epoch.load(Ordering::Acquire);
    let _serial = state.transport.lock().map_err(|_| unavailable())?;
    if state.shutting_down.load(Ordering::Acquire) {
        return Err(unavailable());
    }
    let mut prepared = state
        .playback
        .lock()
        .map_err(|_| unavailable())?
        .prepare_transport();
    let mut result = operation(&mut prepared);
    if result.is_err()
        && let Err(error) = prepared.halt_after_error()
    {
        let snapshot = state
            .playback
            .lock()
            .map_err(|_| unavailable())?
            .recover_transport(prepared);
        let _ = app.emit("playback-updated", snapshot);
        return Err(CommandError::from(error));
    }
    if epoch != state.pause_epoch.load(Ordering::Acquire)
        && let Err(error) = prepared.halt_after_error()
    {
        result = Err(error);
    }
    let library_changed = prepared.has_pending_plays();
    let (snapshot, committed) = {
        let mut playback = state.playback.lock().map_err(|_| unavailable())?;
        let committed = playback.commit_transport(prepared);
        (playback.cached_snapshot(), committed)
    };
    if library_changed && committed.is_ok() {
        let _ = app.emit("library-updated", ());
    }
    let _ = app.emit("playback-updated", &snapshot);
    if let Err(error) = result.and(committed.map(|_| ())) {
        let error = CommandError::from(error);
        return Err(error);
    }
    Ok(snapshot)
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, item_id: &str) {
    if item_id == "help.keyboard-shortcuts" {
        let _ = app.emit("show-keyboard-shortcuts", ());
        return;
    }
    if !matches!(
        item_id,
        "playback.toggle" | "playback.previous" | "playback.next"
    ) {
        return;
    }
    let app = app.clone();
    let id = item_id.to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _ = transport_action(&state, &app, |playback| match id.as_str() {
            "playback.toggle" if playback.cached_transport().status == PlaybackStatus::Playing => {
                playback.pause()
            }
            "playback.toggle" => playback.play(),
            "playback.previous" => playback.previous_track(),
            _ => playback.next_track(),
        });
    });
}

#[tauri::command]
pub fn inspect_import_progress(
    state: State<'_, AppState>,
) -> Result<Option<ImportProgress>, CommandError> {
    state
        .import_progress
        .lock()
        .map(|progress| progress.clone())
        .map_err(|_| CommandError {
            code: "import_unavailable",
            message: "The import progress service is unavailable.".into(),
        })
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
        .spawn(move || {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_metadata_refresh_worker(app.clone(), playback, worker_refreshes.clone());
            }))
            .is_err()
            {
                let mut refreshes = worker_refreshes
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                refreshes.fail_worker();
                emit_metadata_refresh_progress(&app, &refreshes);
            }
        })
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

    loop {
        if crate::process::is_stopping() {
            if let Ok(mut refreshes) = metadata_refreshes.lock() {
                refreshes.finish_worker();
            }
            return;
        }
        let job = match metadata_refreshes.lock() {
            Ok(mut refreshes) => {
                let job = refreshes.take_next();
                if job.is_none() {
                    refreshes.finish_worker();
                    emit_metadata_refresh_progress(&app, &refreshes);
                }
                job
            }
            Err(_) => {
                tracing::error!("metadata refresh mutex is poisoned");
                return;
            }
        };
        let Some(job) = job else {
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
                            .and_then(|mut playback| {
                                if playback.dirty_track_source(&job.track_id).is_none() {
                                    return Ok(false);
                                }
                                let snapshot = playback.commit_youtube_import(resolved)?;
                                drop(playback);
                                let _ = app.emit("library-updated", ());
                                emit_playback_updated(&app, &snapshot);
                                Ok(true)
                            })
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
pub async fn inspect_playback(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, _app| {
        Ok(state
            .playback
            .lock()
            .map_err(|_| unavailable())?
            .cached_snapshot())
    })
    .await
}

#[tauri::command]
pub async fn inspect_playback_transport(app: AppHandle) -> Result<PlaybackTransport, CommandError> {
    blocking(app, move |state, _app| {
        Ok(state
            .playback
            .lock()
            .map_err(|_| unavailable())?
            .cached_transport())
    })
    .await
}

#[tauri::command]
pub async fn inspect_library(app: AppHandle) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, _app| state.library_snapshot()).await
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
pub async fn import_youtube_urls(app: AppHandle, urls: Vec<String>) -> Result<(), CommandError> {
    blocking(app, move |state, app| {
        if state.shutting_down.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        if urls.is_empty() || urls.len() > 100 {
            return Err(CommandError {
                code: "invalid_import",
                message: "Provide 1 to 100 YouTube sources.".into(),
            });
        }
        let cookie_path = auth::session_cookie_path(app).map_err(CommandError::from)?;
        let cookie_path = auth::session_exists(&cookie_path).then_some(cookie_path);
        let run_id = state.next_import_id.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        {
            let mut active = state.active_import.lock().map_err(|_| unavailable())?;
            if active.is_some() {
                return Err(CommandError {
                    code: "import_in_progress",
                    message: "A library import is already running.".into(),
                });
            }
            *active = Some(ImportRun {
                id: run_id,
                cancelled: cancelled.clone(),
            });
        }
        let playback = state.playback.clone();
        let active = state.active_import.clone();
        let app = app.clone();
        let worker = thread::Builder::new()
            .name("youtube-import".into())
            .spawn(move || {
                let _completion = ImportCompletion {
                    active: active.clone(),
                    run_id,
                };
                let total_sources = urls.len();
                let mut saved = 0;
                let mut completed = 0;
                let skipped_member_only = std::cell::Cell::new(0);
                let progress = |phase, message: String, saved, completed| ImportProgress {
                    completed_sources: completed,
                    imported_tracks: saved,
                    message,
                    phase,
                    run_id,
                    skipped_member_only: skipped_member_only.get(),
                    total_sources,
                };
                emit_import_progress(&app, progress("started", "Import started.".into(), 0, 0));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                    || -> Result<(), YouTubePlaybackError> {
                        for url in urls {
                            for discovery in [true, false] {
                                let imported = crate::playback::import_cancellable(
                                    std::slice::from_ref(&url),
                                    cookie_path.as_deref(),
                                    discovery,
                                    &cancelled,
                                    |_, _, _, count, _| {
                                        emit_import_progress(
                                            &app,
                                            progress(
                                                "resolving",
                                                if discovery {
                                                    "Discovering tracks.".into()
                                                } else {
                                                    "Refreshing metadata.".into()
                                                },
                                                saved + count,
                                                completed,
                                            ),
                                        );
                                    },
                                )?;
                                let count = imported.track_count();
                                if !discovery {
                                    skipped_member_only.set(
                                        skipped_member_only.get()
                                            + imported.skipped_member_only_count(),
                                    );
                                }
                                tracing::debug!(
                                    skipped = imported.skipped_member_only_count(),
                                    timed_out = imported.timed_out_source_count(),
                                    "Import source resolved"
                                );
                                let mut provider = playback.lock().map_err(|_| {
                                    YouTubePlaybackError::Library("service unavailable".into())
                                })?;
                                if cancelled.load(Ordering::Acquire) {
                                    return Err(YouTubePlaybackError::Cancelled);
                                }
                                let snapshot = provider.commit_youtube_import(imported)?;
                                drop(provider);
                                if discovery {
                                    saved += count;
                                }
                                let _ = app.emit("library-updated", ());
                                emit_playback_updated(&app, &snapshot);
                            }
                            completed += 1;
                        }
                        Ok(())
                    },
                ))
                .unwrap_or_else(|_| {
                    Err(YouTubePlaybackError::Metadata(
                        "import worker stopped".into(),
                    ))
                });
                if let Ok(mut active) = active.lock()
                    && active.as_ref().is_some_and(|run| run.id == run_id)
                {
                    let (phase, message) = match result {
                        Err(YouTubePlaybackError::Cancelled) => (
                            "cancelled",
                            "Import cancelled. Saved discoveries remain in the library.".into(),
                        ),
                        Ok(()) => ("completed", format!("Imported {saved} tracks.")),
                        Err(error) => ("failed", error.to_string()),
                    };
                    emit_import_progress(&app, progress(phase, message, saved, completed));
                    *active = None;
                }
            });
        if worker.is_err() {
            *state.active_import.lock().map_err(|_| unavailable())? = None;
            return Err(CommandError {
                code: "import_worker_failed",
                message: "Could not start the import worker.".into(),
            });
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn cancel_youtube_import(app: AppHandle, run_id: u64) -> Result<(), CommandError> {
    blocking(app, move |state, _app| {
        cancel_import(&state.active_import, run_id)
    })
    .await
}

fn cancel_import(active: &Mutex<Option<ImportRun>>, run_id: u64) -> Result<(), CommandError> {
    let active = active.lock().map_err(|_| unavailable())?;
    let run = active
        .as_ref()
        .filter(|run| run.id == run_id)
        .ok_or(CommandError {
            code: "stale_import",
            message: "This import is no longer running.".into(),
        })?;
    run.cancelled.store(true, Ordering::Release);
    Ok(())
}

#[tauri::command]
pub async fn search_youtube(
    query: String,
) -> Result<Vec<crate::playback::MediaItem>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::playback::search_youtube(&query).map_err(CommandError::from)
    })
    .await
    .map_err(|_| unavailable())?
}

#[tauri::command]
pub async fn reset_track_metadata(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = with_playback(state, "reset_track_metadata", |playback| {
            playback.reset_track_metadata(&ids)
        })?;
        let _ = app.emit("library-updated", ());
        emit_playback_updated(
            app,
            &state
                .playback
                .lock()
                .map_err(|_| unavailable())?
                .cached_snapshot(),
        );
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn retry_metadata_refreshes(
    app: AppHandle,
) -> Result<MetadataRefreshSnapshot, CommandError> {
    blocking(app, |state, app| {
        let failed: HashSet<_> = {
            let mut refreshes = state.metadata_refreshes.lock().map_err(|_| unavailable())?;
            let failed = refreshes
                .jobs
                .iter()
                .filter(|job| job.state == "failed")
                .map(|job| job.track_id.clone())
                .collect::<HashSet<_>>();
            refreshes.jobs.retain(|job| job.state != "failed");
            refreshes.completed_tracks = refreshes.completed_tracks.saturating_sub(failed.len());
            refreshes.total_tracks = refreshes.total_tracks.saturating_sub(failed.len());
            failed
        };
        let tracks = state
            .playback
            .lock()
            .map_err(|_| unavailable())?
            .dirty_tracks()
            .into_iter()
            .filter(|track| failed.contains(&track.id))
            .collect();
        schedule_metadata_refreshes(app, &state.playback, &state.metadata_refreshes, tracks);
        Ok(state
            .metadata_refreshes
            .lock()
            .map_err(|_| unavailable())?
            .snapshot())
    })
    .await
}

#[derive(Serialize)]
pub struct LibraryBackup {
    path: String,
}

#[tauri::command]
pub async fn export_library_backup(app: AppHandle) -> Result<LibraryBackup, CommandError> {
    blocking(app, |_state, app| {
        let directory = app.path().app_data_dir().map_err(|_| unavailable())?;
        let path = crate::persistence::export_library_backup(&directory.join("library.sqlite3"))
            .map_err(|_| CommandError {
                code: "backup_failed",
                message: "Could not export the library backup.".into(),
            })?;
        Ok(LibraryBackup {
            path: path.to_string_lossy().into_owned(),
        })
    })
    .await
}

#[tauri::command]
pub async fn inspect_diagnostics() -> crate::diagnostics::DiagnosticsSnapshot {
    tauri::async_runtime::spawn_blocking(crate::diagnostics::inspect)
        .await
        .unwrap_or_else(|_| crate::diagnostics::unavailable())
}

fn emit_import_progress<R: Runtime>(app: &AppHandle<R>, progress: ImportProgress) {
    // Cache before publishing so newly opened windows can recover missed events.
    // Keep terminal progress until the next run, but never persist it across boots.
    let state = app.state::<AppState>();
    match state.import_progress.lock() {
        Ok(mut cached) => *cached = Some(progress.clone()),
        Err(_) => tracing::error!("could not cache import progress"),
    }
    if let Err(error) = app.emit("import-progress", progress) {
        tracing::debug!(%error, "could not deliver import progress");
    }
}

fn emit_playback_updated<R: Runtime>(app: &AppHandle<R>, snapshot: &PlaybackSnapshot) {
    if let Err(error) = app.emit("playback-updated", snapshot) {
        tracing::debug!(%error, "could not deliver playback update");
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
pub async fn update_tracks_metadata(
    app: AppHandle,
    updates: Vec<TrackMetadataUpdate>,
) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = with_playback(state, "update_metadata_patches", |playback| {
            playback.update_metadata_patches(
                updates
                    .into_iter()
                    .map(|update| (update.id, update.metadata))
                    .collect(),
            )
        })?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        state.emit_cached_playback(app);
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn toggle_favorite(app: AppHandle, id: String) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = state.toggle_favorite(&id)?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn remove_tracks(
    app: AppHandle,
    ids: Vec<String>,
) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let result = state.remove_tracks(&ids);
        // A successful native stop can precede a failed database write.
        state.emit_cached_playback(app);
        let snapshot = result?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn upsert_playlist(
    app: AppHandle,
    playlist: Playlist,
) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = state.upsert_playlist(playlist)?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn reorder_playlists(
    app: AppHandle,
    playlist_ids: Vec<String>,
) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = state.reorder_playlists(&playlist_ids)?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn delete_playlist(app: AppHandle, id: String) -> Result<LibrarySnapshot, CommandError> {
    blocking(app, move |state, app| {
        let snapshot = state.delete_playlist(&id)?;
        if let Err(error) = app.emit("library-updated", ()) {
            tracing::debug!(%error, "could not deliver library update event");
        }
        Ok(snapshot)
    })
    .await
}

#[tauri::command]
pub async fn inspect_youtube_auth(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    blocking(app, |_state, app| {
        let cookie_path = auth::session_cookie_path(app).map_err(CommandError::from)?;
        Ok(auth::YouTubeAuthStatus {
            connected: auth::session_exists(&cookie_path),
        })
    })
    .await
}

#[tauri::command]
pub async fn open_youtube_login(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    blocking(app, |_state, app| {
        tracing::info!("opening YouTube login window");
        windows::show_youtube_login(app).map_err(|error| CommandError {
            code: "youtube_login_window_failed",
            message: error.to_string(),
        })?;
        let cookie_path = auth::session_cookie_path(app).map_err(CommandError::from)?;
        Ok(auth::YouTubeAuthStatus {
            connected: auth::session_exists(&cookie_path),
        })
    })
    .await
}

#[tauri::command]
pub async fn save_youtube_session(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    blocking(app, move |state, app| {
        state.pause_epoch.fetch_add(1, Ordering::AcqRel);
        let _serial = state.transport.lock().map_err(|_| unavailable())?;
        let window = app
            .get_webview_window("youtube-auth")
            .ok_or(auth::YouTubeAuthError::LoginWindowNotOpen)
            .map_err(CommandError::from)?;
        let cookie_path = auth::session_cookie_path(app).map_err(CommandError::from)?;
        auth::save_session_from_window(&window, &cookie_path).map_err(CommandError::from)?;
        with_playback(state, "save_youtube_session", |playback| {
            playback.set_session_cookie_path(Some(cookie_path))
        })?;
        window.close().map_err(|error| CommandError {
            code: "youtube_login_window_failed",
            message: error.to_string(),
        })?;
        tracing::info!("YouTube session saved");
        emit_playback_updated(
            app,
            &state
                .playback
                .lock()
                .map_err(|_| unavailable())?
                .cached_snapshot(),
        );

        Ok(auth::YouTubeAuthStatus { connected: true })
    })
    .await
}

#[tauri::command]
pub async fn disconnect_youtube(app: AppHandle) -> Result<auth::YouTubeAuthStatus, CommandError> {
    blocking(app, move |state, app| {
        state.pause_epoch.fetch_add(1, Ordering::AcqRel);
        if let Some(run) = state
            .active_import
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
        {
            run.cancelled.store(true, Ordering::Release);
        }
        let _serial = state.transport.lock().map_err(|_| unavailable())?;
        let cookie_path = auth::session_cookie_path(app).map_err(CommandError::from)?;
        with_playback(state, "disconnect_youtube", |playback| {
            playback.set_session_cookie_path(None)
        })?;
        auth::disconnect_session(app, &cookie_path).map_err(CommandError::from)?;
        emit_playback_updated(
            app,
            &state
                .playback
                .lock()
                .map_err(|_| unavailable())?
                .cached_snapshot(),
        );
        tracing::info!("YouTube session disconnected");

        Ok(auth::YouTubeAuthStatus { connected: false })
    })
    .await
}

#[tauri::command]
pub async fn play(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.play())
    })
    .await
}

#[tauri::command]
pub async fn pause(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.pause())
    })
    .await
}

#[tauri::command]
pub async fn previous(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.previous_track())
    })
    .await
}

#[tauri::command]
pub async fn next(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.next_track())
    })
    .await
}

#[tauri::command]
pub async fn toggle_shuffle(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.toggle_shuffle())
    })
    .await
}

#[tauri::command]
pub async fn cycle_repeat_mode(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.cycle_repeat_mode())
    })
    .await
}

#[tauri::command]
pub async fn play_track(
    app: AppHandle,
    id: String,
    queue_ids: Option<Vec<String>>,
) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| {
            if let Some(ids) = queue_ids {
                playback.replace_queue(&ids)?;
            }
            playback.play_track(&id)
        })
    })
    .await
}

#[tauri::command]
pub async fn queue_track_next(
    app: AppHandle,
    id: String,
) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.queue_track_next(&id))
    })
    .await
}

#[tauri::command]
pub async fn add_to_queue(app: AppHandle, id: String) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.add_to_queue(&id))
    })
    .await
}

#[tauri::command]
pub async fn seek(app: AppHandle, position_ms: u64) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| playback.seek(position_ms))
    })
    .await
}

#[tauri::command]
pub async fn move_queue_item(
    app: AppHandle,
    from: usize,
    to: usize,
) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| {
            playback.move_queue_item(from, to)
        })
    })
    .await
}

#[tauri::command]
pub async fn remove_queue_item(
    app: AppHandle,
    index: usize,
) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| {
            playback.remove_queue_item(index)
        })
    })
    .await
}

#[tauri::command]
pub async fn clear_queue(app: AppHandle) -> Result<PlaybackSnapshot, CommandError> {
    blocking(app, move |state, app| {
        transport_action(state, app, move |playback| {
            playback.clear_queue().map(|_| ())
        })
    })
    .await
}
#[tauri::command]
pub async fn set_volume(app: AppHandle, volume_percent: u8) -> Result<(), CommandError> {
    blocking(app, move |state, app| {
        let _serial = state.transport.lock().map_err(|_| unavailable())?;
        if state.shutting_down.load(Ordering::Acquire) {
            return Err(unavailable());
        }
        with_playback(state, "set_volume", |playback| {
            playback.set_volume(volume_percent)
        })?;
        let transport = state
            .playback
            .lock()
            .map_err(|_| unavailable())?
            .cached_transport();
        let _ = app.emit("playback-transport-updated", transport);
        Ok(())
    })
    .await
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

    #[test]
    fn import_progress_is_inspectable_on_boot_and_broadcast_before_windows_reopen() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        use tauri::{Listener, Manager};

        let app = mock_builder()
            .manage(AppState::default())
            .invoke_handler(tauri::generate_handler![super::inspect_import_progress])
            .build(mock_context(noop_assets()))
            .unwrap();
        let library = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        let import = tauri::WebviewWindowBuilder::new(&app, "import", Default::default())
            .build()
            .unwrap();
        let inspect = || {
            tauri::test::get_ipc_response(
                &library,
                tauri::webview::InvokeRequest {
                    cmd: "inspect_import_progress".into(),
                    callback: tauri::ipc::CallbackFn(0),
                    error: tauri::ipc::CallbackFn(1),
                    url: if cfg!(any(windows, target_os = "android")) {
                        "http://tauri.localhost"
                    } else {
                        "tauri://localhost"
                    }
                    .parse()
                    .unwrap(),
                    body: tauri::ipc::InvokeBody::default(),
                    headers: Default::default(),
                    invoke_key: tauri::test::INVOKE_KEY.to_string(),
                },
            )
            .unwrap()
            .deserialize::<serde_json::Value>()
            .unwrap()
        };
        assert_eq!(inspect(), serde_json::Value::Null);

        let (tx, rx) = std::sync::mpsc::channel();
        for window in [&library, &import] {
            let tx = tx.clone();
            let handle = app.handle().clone();
            window.listen("import-progress", move |event| {
                let cached = super::inspect_import_progress(handle.state::<AppState>()).unwrap();
                assert_eq!(
                    serde_json::to_value(cached).unwrap(),
                    serde_json::from_str::<serde_json::Value>(event.payload()).unwrap(),
                    "cache must be updated before listeners receive the event",
                );
                tx.send(()).unwrap();
            });
        }
        for phase in ["started", "resolving", "completed", "failed", "cancelled"] {
            let progress = super::ImportProgress {
                completed_sources: 1,
                imported_tracks: 4,
                message: format!("Import {phase}"),
                phase,
                run_id: 7,
                skipped_member_only: 2,
                total_sources: 3,
            };
            let expected = serde_json::to_value(&progress).unwrap();
            super::emit_import_progress(app.handle(), progress);
            for _ in 0..2 {
                rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
            }
            assert_eq!(inspect(), expected);
        }
    }

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
    fn worker_cleanup_preserves_new_import_runs_and_releases_failed_jobs() {
        use std::sync::atomic::AtomicBool;
        let active = Arc::new(Mutex::new(Some(super::ImportRun {
            id: 2,
            cancelled: Arc::new(AtomicBool::new(false)),
        })));
        drop(super::ImportCompletion {
            active: active.clone(),
            run_id: 1,
        });
        assert_eq!(active.lock().unwrap().as_ref().unwrap().id, 2);
        drop(super::ImportCompletion {
            active: active.clone(),
            run_id: 2,
        });
        assert!(active.lock().unwrap().is_none());

        let mut refreshes = super::MetadataRefreshState::default();
        assert!(refreshes.enqueue(vec![crate::playback::DirtyTrack {
            id: "fixture".into(),
            title: "Fixture".into(),
        }]));
        refreshes.take_next().unwrap();
        refreshes.fail_worker();
        assert!(!refreshes.running);
        assert_eq!(refreshes.jobs[0].state, "failed");
        assert!(refreshes.queued_ids.is_empty());
        assert!(refreshes.pending.is_empty());
    }

    #[test]
    fn cancellation_rejects_stale_runs_without_cancelling_the_current_child() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = Arc::new(AtomicBool::new(false));
        let active = Mutex::new(Some(super::ImportRun {
            id: 2,
            cancelled: cancelled.clone(),
        }));
        assert!(super::cancel_import(&active, 1).is_err());
        assert!(!cancelled.load(Ordering::Acquire));
        super::cancel_import(&active, 2).unwrap();
        assert!(cancelled.load(Ordering::Acquire));
        *active.lock().unwrap() = None;
        assert!(super::cancel_import(&active, 2).is_err());
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
        assert_eq!(
            library
                .playlists
                .iter()
                .map(|playlist| playlist.name.as_str())
                .collect::<Vec<_>>(),
            ["Favorites", "Most Played"]
        );

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
