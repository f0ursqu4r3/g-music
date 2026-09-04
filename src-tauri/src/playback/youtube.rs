use std::{
    collections::HashSet,
    env, fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

use super::{
    EditableTrackMetadata, LibrarySnapshot, MediaItem, PlaybackSnapshot, PlaybackStatus,
    PlaybackTransport, Playlist, RepeatMode, shuffle_upcoming,
};

const IPC_TIMEOUT: Duration = Duration::from_secs(3);
const MEDIA_LOAD_TIMEOUT: Duration = Duration::from_secs(30);
const MEDIA_LOAD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LIBRARY_VERSION: u32 = 2;
const MAX_PLAY_HISTORY: usize = 500;
const METADATA_IDLE_TIMEOUT: Duration = Duration::from_secs(60);
const FAVORITES_PLAYLIST_ID: &str = "favorites";
const MOST_PLAYED_PLAYLIST_ID: &str = "most-played";
const PARENT_EXIT_WATCHDOG: &str = r#"
parent_pid="$1"
player_pid="$2"

while kill -0 "$parent_pid" 2>/dev/null; do
    sleep 1
done

kill -TERM "$player_pid" 2>/dev/null || exit 0
sleep 1
kill -KILL "$player_pid" 2>/dev/null || true
"#;

#[derive(Debug, Error)]
pub enum YouTubePlaybackError {
    #[error("only YouTube URLs are supported")]
    UnsupportedUrl,

    #[error("{name} is not available: {detail}")]
    DependencyUnavailable { name: &'static str, detail: String },

    #[error("could not resolve YouTube metadata: {0}")]
    Metadata(String),

    #[error("the audio player failed: {0}")]
    Player(String),

    #[error("queue index {index} is outside the current queue")]
    QueueIndexOutOfBounds { index: usize },

    #[error("the current queue item cannot be removed")]
    CurrentQueueItem,

    #[error("track {id} is not in the current queue")]
    TrackNotFound { id: String },

    #[error("volume must be between 0 and 100")]
    InvalidVolume,

    #[error("track metadata is invalid: {0}")]
    InvalidTrackMetadata(String),

    #[error("playlist is invalid: {0}")]
    InvalidPlaylist(String),

    #[error("could not access the imported library: {0}")]
    Library(String),
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct QueueEntry {
    pub(crate) item: MediaItem,
    pub(crate) source_url: String,
}

pub(crate) struct ResolvedYouTubeImport {
    entries: Vec<QueueEntry>,
    skipped_member_only: usize,
    skipped_member_only_ids: HashSet<String>,
    timed_out_sources: usize,
}

impl ResolvedYouTubeImport {
    pub(crate) fn track_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn skipped_member_only_count(&self) -> usize {
        self.skipped_member_only
    }

    pub(crate) fn timed_out_source_count(&self) -> usize {
        self.timed_out_sources
    }
}

#[derive(Clone)]
pub(crate) struct DirtyTrack {
    pub(crate) id: String,
    pub(crate) title: String,
}

struct MetadataResolution {
    entries: Vec<QueueEntry>,
    skipped_member_only: usize,
    skipped_member_only_ids: HashSet<String>,
    timed_out: bool,
}

pub struct YouTubePlaybackProvider {
    entries: Vec<QueueEntry>,
    library_path: Option<PathBuf>,
    player: MpvPlayer,
    playlists: Vec<Playlist>,
    session_cookie_path: Option<PathBuf>,
    shuffle_order: Vec<String>,
    snapshot: PlaybackSnapshot,
}

impl YouTubePlaybackProvider {
    pub fn new() -> Self {
        Self::with_entries(Vec::new(), None)
    }

    pub fn from_library_path(path: PathBuf) -> Result<Self, YouTubePlaybackError> {
        let library = load_library(&path)?;
        let saved_state = crate::persistence::load_playback_state(&path)
            .map_err(YouTubePlaybackError::Library)?;
        tracing::info!(
            tracks = library.entries.len(),
            playlists = library.playlists.len(),
            "imported library loaded"
        );
        let mut provider = Self::with_library(library.entries, library.playlists, Some(path));
        if let Some(saved_state) = saved_state {
            provider.restore_playback_state(saved_state);
        }
        Ok(provider)
    }

    pub fn from_library_directory(directory: PathBuf) -> Result<Self, YouTubePlaybackError> {
        let database_path = directory.join("library.sqlite3");
        if !crate::persistence::database_exists(&database_path) {
            let legacy_path = directory.join("library.json");
            if legacy_path.is_file() {
                let library = load_legacy_library(&legacy_path)?;
                crate::persistence::save_library(
                    &database_path,
                    &library.entries,
                    &library.playlists,
                )
                .map_err(YouTubePlaybackError::Library)?;
                fs::rename(&legacy_path, directory.join("library.json.migrated"))
                    .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
            }
        }
        Self::from_library_path(database_path)
    }

    fn with_entries(entries: Vec<QueueEntry>, library_path: Option<PathBuf>) -> Self {
        Self::with_library(entries, Vec::new(), library_path)
    }

    fn with_library(
        entries: Vec<QueueEntry>,
        playlists: Vec<Playlist>,
        library_path: Option<PathBuf>,
    ) -> Self {
        Self {
            entries,
            library_path,
            player: MpvPlayer::new(),
            playlists,
            session_cookie_path: None,
            shuffle_order: Vec::new(),
            snapshot: PlaybackSnapshot {
                status: PlaybackStatus::Paused,
                current_item: None,
                position_ms: 0,
                volume_percent: 72,
                shuffle_enabled: false,
                repeat_mode: RepeatMode::Off,
                queue: Vec::new(),
                playback_order: Vec::new(),
            },
        }
    }

    pub fn import_youtube_urls(
        &mut self,
        source_urls: &[String],
    ) -> Result<PlaybackSnapshot, YouTubePlaybackError> {
        let imported = resolve_youtube_imports(
            source_urls,
            self.session_cookie_path.as_deref(),
            |_, _, _, _, _| {},
        )?;

        self.commit_youtube_import(imported)
    }

    pub(crate) fn commit_youtube_import(
        &mut self,
        imported: ResolvedYouTubeImport,
    ) -> Result<PlaybackSnapshot, YouTubePlaybackError> {
        let previous_queue_size = self.entries.len();
        let imported_count = imported.track_count();
        self.entries
            .retain(|entry| !imported.skipped_member_only_ids.contains(&entry.item.id));
        merge_queue_entries(&mut self.entries, imported.entries);
        let inserted_count = self.entries.len().saturating_sub(previous_queue_size);
        tracing::debug!(
            imported = imported_count,
            inserted = inserted_count,
            updated = imported_count.saturating_sub(inserted_count),
            queue_size = self.entries.len(),
            "queue merged"
        );
        self.reconcile_queue();
        self.persist_library()?;
        tracing::info!(
            imported = imported_count,
            inserted = inserted_count,
            queue_size = self.snapshot.queue.len(),
            "YouTube batch import completed"
        );

        Ok(self.snapshot.clone())
    }

    pub fn set_session_cookie_path(&mut self, path: Option<PathBuf>) {
        tracing::debug!(authenticated = path.is_some(), "playback session updated");
        self.session_cookie_path = path;
    }

    pub fn shutdown(&mut self) {
        if let Err(error) = self.update_transport() {
            tracing::warn!(%error, "could not sample playback state before shutdown");
        }
        if let Err(error) = self.persist_playback_state() {
            tracing::error!(%error, "could not save playback state before shutdown");
        }
        self.player.stop();
    }

    pub(crate) fn dirty_tracks(&self) -> Vec<DirtyTrack> {
        self.entries
            .iter()
            .filter(|entry| entry.item.metadata_dirty)
            .map(|entry| DirtyTrack {
                id: entry.item.id.clone(),
                title: entry.item.title.clone(),
            })
            .collect()
    }

    pub(crate) fn dirty_track_source(&self, id: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|entry| entry.item.id == id && entry.item.metadata_dirty)
            .map(|entry| entry.source_url.clone())
    }

    pub fn library_snapshot(&self) -> LibrarySnapshot {
        LibrarySnapshot {
            playlists: self.playlists_with_defaults(),
            total_plays: self.entries.iter().map(|entry| entry.item.play_count).sum(),
            tracks: self
                .entries
                .iter()
                .map(|entry| entry.item.clone())
                .collect(),
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        if !self.entries.iter().any(|entry| entry.item.id == id) {
            return Err(YouTubePlaybackError::TrackNotFound { id: id.into() });
        }

        let favorites = if let Some(playlist) = self
            .playlists
            .iter_mut()
            .find(|playlist| playlist.id == FAVORITES_PLAYLIST_ID)
        {
            playlist
        } else {
            self.playlists.push(Playlist {
                id: FAVORITES_PLAYLIST_ID.into(),
                name: "Favorites".into(),
                track_ids: Vec::new(),
            });
            self.playlists
                .last_mut()
                .expect("the favorites playlist was added")
        };

        if let Some(position) = favorites
            .track_ids
            .iter()
            .position(|track_id| track_id == id)
        {
            favorites.track_ids.remove(position);
        } else {
            favorites.track_ids.push(id.into());
        }

        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn update_track_metadata(
        &mut self,
        id: &str,
        metadata: EditableTrackMetadata,
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let title = required_metadata_value(metadata.title, "title")?;
        let artist = required_metadata_value(metadata.artist, "artist")?;
        let album = optional_metadata_value(metadata.album);
        let label = optional_metadata_value(metadata.label);
        let genres = normalize_genres(metadata.genres);
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.item.id == id)
            .ok_or_else(|| YouTubePlaybackError::TrackNotFound { id: id.into() })?;

        entry.item.title = title;
        entry.item.artist = artist;
        entry.item.album = album;
        entry.item.label = label;
        entry.item.genres = genres;
        self.reconcile_queue();
        if self
            .snapshot
            .current_item
            .as_ref()
            .is_some_and(|item| item.id == id)
        {
            self.snapshot.current_item = self
                .entries
                .iter()
                .find(|entry| entry.item.id == id)
                .map(|entry| entry.item.clone());
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn update_tracks_metadata(
        &mut self,
        updates: Vec<(String, EditableTrackMetadata)>,
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let updates = updates
            .into_iter()
            .map(|(id, metadata)| {
                Ok((
                    id,
                    EditableTrackMetadata {
                        title: required_metadata_value(metadata.title, "title")?,
                        artist: required_metadata_value(metadata.artist, "artist")?,
                        album: optional_metadata_value(metadata.album),
                        label: optional_metadata_value(metadata.label),
                        genres: normalize_genres(metadata.genres),
                    },
                ))
            })
            .collect::<Result<Vec<_>, YouTubePlaybackError>>()?;
        let ids = updates
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<HashSet<_>>();
        if ids.len() != updates.len() {
            return Err(YouTubePlaybackError::InvalidTrackMetadata(
                "the batch contains duplicate track IDs".into(),
            ));
        }
        if let Some(id) = ids
            .iter()
            .find(|id| !self.entries.iter().any(|entry| entry.item.id == ***id))
        {
            return Err(YouTubePlaybackError::TrackNotFound { id: (*id).into() });
        }

        for (id, metadata) in updates {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.item.id == id)
                .expect("batch IDs are checked against the library before mutation");
            entry.item.title = metadata.title;
            entry.item.artist = metadata.artist;
            entry.item.album = metadata.album;
            entry.item.label = metadata.label;
            entry.item.genres = metadata.genres;
        }
        self.reconcile_queue();
        if let Some(current_id) = self
            .snapshot
            .current_item
            .as_ref()
            .map(|item| item.id.as_str())
        {
            self.snapshot.current_item = self
                .entries
                .iter()
                .find(|entry| entry.item.id == current_id)
                .map(|entry| entry.item.clone());
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn remove_tracks(
        &mut self,
        track_ids: &[String],
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let track_ids = track_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        if track_ids.is_empty() {
            return Err(YouTubePlaybackError::InvalidTrackMetadata(
                "at least one track ID is required".into(),
            ));
        }
        if let Some(id) = track_ids
            .iter()
            .find(|id| !self.entries.iter().any(|entry| entry.item.id == ***id))
        {
            return Err(YouTubePlaybackError::TrackNotFound { id: (*id).into() });
        }

        let removed_current_track = self
            .snapshot
            .current_item
            .as_ref()
            .is_some_and(|item| track_ids.contains(item.id.as_str()));
        self.entries
            .retain(|entry| !track_ids.contains(entry.item.id.as_str()));
        for playlist in &mut self.playlists {
            playlist
                .track_ids
                .retain(|id| !track_ids.contains(id.as_str()));
        }
        self.reconcile_queue();
        if removed_current_track {
            self.pause()?;
            self.snapshot.current_item = None;
            self.snapshot.position_ms = 0;
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn upsert_playlist(
        &mut self,
        playlist: Playlist,
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let id = required_metadata_value(playlist.id, "playlist ID")?;
        let name = required_metadata_value(playlist.name, "playlist name")?;
        if is_default_playlist(&id) {
            return Err(YouTubePlaybackError::InvalidPlaylist(
                "default playlists cannot be edited directly".into(),
            ));
        }
        let mut track_ids = Vec::new();
        for track_id in playlist.track_ids {
            if !self.entries.iter().any(|entry| entry.item.id == track_id) {
                return Err(YouTubePlaybackError::InvalidPlaylist(format!(
                    "track {track_id} is not in the library"
                )));
            }
            if !track_ids.contains(&track_id) {
                track_ids.push(track_id);
            }
        }
        let playlist = Playlist {
            id,
            name,
            track_ids,
        };
        if let Some(existing) = self
            .playlists
            .iter_mut()
            .find(|existing| existing.id == playlist.id)
        {
            *existing = playlist;
        } else {
            self.playlists.push(playlist);
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn reorder_playlists(
        &mut self,
        playlist_ids: &[String],
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let user_playlist_ids = self
            .playlists
            .iter()
            .filter(|playlist| !is_default_playlist(&playlist.id))
            .map(|playlist| playlist.id.as_str())
            .collect::<HashSet<_>>();
        let requested_ids = playlist_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if requested_ids.len() != playlist_ids.len() || requested_ids != user_playlist_ids {
            return Err(YouTubePlaybackError::InvalidPlaylist(
                "playlist order must contain every user playlist exactly once".into(),
            ));
        }

        let (mut default_playlists, mut user_playlists): (Vec<_>, Vec<_>) = self
            .playlists
            .drain(..)
            .partition(|playlist| is_default_playlist(&playlist.id));
        for id in playlist_ids {
            let position = user_playlists
                .iter()
                .position(|playlist| playlist.id == *id)
                .expect("playlist IDs are validated before reordering");
            default_playlists.push(user_playlists.remove(position));
        }
        self.playlists = default_playlists;
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn delete_playlist(&mut self, id: &str) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        if is_default_playlist(id) {
            return Err(YouTubePlaybackError::InvalidPlaylist(
                "default playlists cannot be deleted".into(),
            ));
        }
        let old_len = self.playlists.len();
        self.playlists.retain(|playlist| playlist.id != id);
        if self.playlists.len() == old_len {
            return Err(YouTubePlaybackError::InvalidPlaylist(format!(
                "playlist {id} does not exist"
            )));
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn record_playback_start(
        &mut self,
        id: &str,
        played_at_ms: u64,
    ) -> Result<LibrarySnapshot, YouTubePlaybackError> {
        let updated_item = {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.item.id == id)
                .ok_or_else(|| YouTubePlaybackError::TrackNotFound { id: id.into() })?;
            entry.item.play_count = entry.item.play_count.saturating_add(1);
            entry.item.last_played_at_ms = Some(played_at_ms);
            entry.item.play_history_ms.push(played_at_ms);
            let excess = entry
                .item
                .play_history_ms
                .len()
                .saturating_sub(MAX_PLAY_HISTORY);
            if excess > 0 {
                entry.item.play_history_ms.drain(..excess);
            }
            entry.item.clone()
        };
        self.reconcile_queue();
        if self
            .snapshot
            .current_item
            .as_ref()
            .is_some_and(|item| item.id == id)
        {
            self.snapshot.current_item = Some(updated_item);
        }
        self.persist_library()?;
        Ok(self.library_snapshot())
    }

    pub fn snapshot(&mut self) -> Result<PlaybackSnapshot, YouTubePlaybackError> {
        self.update_transport()?;
        Ok(self.complete_snapshot())
    }

    pub fn transport(&mut self) -> Result<PlaybackTransport, YouTubePlaybackError> {
        self.update_transport()?;
        Ok(PlaybackTransport::from(&self.snapshot))
    }

    pub fn transport_with_snapshot_update(
        &mut self,
    ) -> Result<(PlaybackTransport, Option<PlaybackSnapshot>), YouTubePlaybackError> {
        let before = self.snapshot.clone();
        self.update_transport()?;
        let transport = PlaybackTransport::from(&self.snapshot);
        let snapshot_update = Self::snapshot_requires_broadcast(&before, &self.snapshot)
            .then(|| self.complete_snapshot());

        Ok((transport, snapshot_update))
    }

    fn complete_snapshot(&self) -> PlaybackSnapshot {
        let mut snapshot = self.snapshot.clone();
        snapshot.playback_order = self.shuffle_order.clone();
        snapshot
    }

    fn snapshot_requires_broadcast(before: &PlaybackSnapshot, after: &PlaybackSnapshot) -> bool {
        before.current_item != after.current_item
            || before.queue != after.queue
            || before.repeat_mode != after.repeat_mode
            || before.shuffle_enabled != after.shuffle_enabled
    }

    fn update_transport(&mut self) -> Result<(), YouTubePlaybackError> {
        if self.snapshot.current_item.is_some() && self.player.is_running() {
            let was_playing = self.snapshot.status == PlaybackStatus::Playing;
            let state = self.player.inspect()?;
            if was_playing && state.eof_reached {
                self.advance_after_end()?;
                return Ok(());
            }
            self.snapshot.status = if state.paused {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            };
            self.snapshot.position_ms = state.position_ms;
            tracing::trace!(
                paused = state.paused,
                position_ms = state.position_ms,
                "playback state sampled"
            );
        }

        Ok(())
    }

    pub fn play(&mut self) -> Result<(), YouTubePlaybackError> {
        if self.snapshot.current_item.is_some() {
            if !self.player.has_loaded_file()? {
                let index = self.current_index().ok_or_else(|| {
                    YouTubePlaybackError::Player("the restored track is not in the queue".into())
                })?;
                let position_ms = self.snapshot.position_ms;
                self.select_queue_index(index)?;
                if position_ms > 0 {
                    self.player.seek(position_ms)?;
                    self.snapshot.position_ms = position_ms;
                }
            }
            self.player.set_paused(false)?;
            self.snapshot.status = PlaybackStatus::Playing;
            tracing::info!("playback resumed");
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), YouTubePlaybackError> {
        if self.snapshot.current_item.is_some() {
            self.player.set_paused(true)?;
            tracing::info!("playback paused");
        }
        self.snapshot.status = PlaybackStatus::Paused;
        Ok(())
    }

    pub fn seek(&mut self, position_ms: u64) -> Result<(), YouTubePlaybackError> {
        let Some(current_item) = self.snapshot.current_item.as_ref() else {
            return Ok(());
        };
        let position_ms = position_ms.min(current_item.duration_ms);
        if self.player.is_running() {
            self.player.seek(position_ms)?;
        }
        self.snapshot.position_ms = position_ms;
        tracing::debug!(position_ms, "playback seeked");
        Ok(())
    }

    pub fn next_track(&mut self) -> Result<(), YouTubePlaybackError> {
        let Some(current_index) = self.current_index() else {
            return Ok(());
        };
        self.advance_from(current_index)
    }

    pub fn previous_track(&mut self) -> Result<(), YouTubePlaybackError> {
        let Some(current_index) = self.current_index() else {
            return Ok(());
        };
        if self.snapshot.shuffle_enabled {
            let Some(current_order_index) = self.current_shuffle_order_index() else {
                return Ok(());
            };
            let previous_order_index = if current_order_index == 0 {
                self.shuffle_order.len() - 1
            } else {
                current_order_index - 1
            };
            let previous_id = &self.shuffle_order[previous_order_index];
            let previous_index = self
                .snapshot
                .queue
                .iter()
                .position(|item| &item.id == previous_id)
                .ok_or_else(|| YouTubePlaybackError::TrackNotFound {
                    id: previous_id.clone(),
                })?;
            return self.select_queue_index(previous_index);
        }
        let previous_index = if current_index == 0 {
            self.snapshot.queue.len() - 1
        } else {
            current_index - 1
        };
        self.select_queue_index(previous_index)
    }

    pub fn play_track(&mut self, id: &str) -> Result<(), YouTubePlaybackError> {
        let index = self
            .snapshot
            .queue
            .iter()
            .position(|item| item.id == id)
            .ok_or_else(|| YouTubePlaybackError::TrackNotFound { id: id.into() })?;

        if self.snapshot.shuffle_enabled {
            self.reset_shuffle_order(Some(id));
        }
        self.select_queue_index(index)?;
        self.player.set_paused(false)?;
        self.snapshot.status = PlaybackStatus::Playing;
        self.record_playback_start(id, now_epoch_ms()?)?;
        tracing::info!(video_id = id, index, "library track playback started");
        Ok(())
    }

    pub fn move_queue_item(&mut self, from: usize, to: usize) -> Result<(), YouTubePlaybackError> {
        if from >= self.snapshot.queue.len() {
            return Err(YouTubePlaybackError::QueueIndexOutOfBounds { index: from });
        }
        if to >= self.snapshot.queue.len() {
            return Err(YouTubePlaybackError::QueueIndexOutOfBounds { index: to });
        }

        let item = self.snapshot.queue.remove(from);
        self.snapshot.queue.insert(to, item);
        self.reconcile_shuffle_order();
        tracing::info!(from, to, "queue item moved");
        Ok(())
    }

    pub fn remove_queue_item(&mut self, index: usize) -> Result<(), YouTubePlaybackError> {
        if index >= self.snapshot.queue.len() {
            return Err(YouTubePlaybackError::QueueIndexOutOfBounds { index });
        }
        if self
            .current_index()
            .is_some_and(|current_index| current_index == index)
        {
            return Err(YouTubePlaybackError::CurrentQueueItem);
        }

        self.snapshot.queue.remove(index);
        self.reconcile_shuffle_order();
        tracing::info!(index, "queue item removed");
        Ok(())
    }

    pub fn replace_queue(&mut self, track_ids: &[String]) -> Result<(), YouTubePlaybackError> {
        self.snapshot.queue = self.items_for_ids(track_ids)?;
        self.snapshot.current_item = None;
        self.snapshot.position_ms = 0;
        self.snapshot.status = PlaybackStatus::Paused;
        self.reconcile_shuffle_order();
        Ok(())
    }

    pub fn queue_track_next(&mut self, id: &str) -> Result<(), YouTubePlaybackError> {
        let item = self.library_item(id)?;
        self.snapshot.queue.retain(|queued| queued.id != id);
        let insert_at = self.current_index().map_or(0, |index| index + 1);
        self.snapshot.queue.insert(insert_at, item);
        self.reconcile_shuffle_order();
        Ok(())
    }

    pub fn add_to_queue(&mut self, id: &str) -> Result<(), YouTubePlaybackError> {
        let item = self.library_item(id)?;
        self.snapshot.queue.retain(|queued| queued.id != id);
        self.snapshot.queue.push(item);
        self.reconcile_shuffle_order();
        Ok(())
    }

    pub fn set_volume(&mut self, volume_percent: u8) -> Result<(), YouTubePlaybackError> {
        if volume_percent > 100 {
            return Err(YouTubePlaybackError::InvalidVolume);
        }
        if self.snapshot.current_item.is_some() && self.player.is_running() {
            self.player.set_volume(volume_percent)?;
        }
        self.snapshot.volume_percent = volume_percent;
        tracing::debug!(volume_percent, "playback volume changed");
        Ok(())
    }

    pub fn toggle_shuffle(&mut self) -> Result<(), YouTubePlaybackError> {
        self.snapshot.shuffle_enabled = !self.snapshot.shuffle_enabled;
        if self.snapshot.shuffle_enabled {
            let current_id = self
                .snapshot
                .current_item
                .as_ref()
                .map(|item| item.id.clone());
            self.reset_shuffle_order(current_id.as_deref());
        } else {
            self.shuffle_order.clear();
        }
        Ok(())
    }

    pub fn cycle_repeat_mode(&mut self) -> Result<(), YouTubePlaybackError> {
        self.snapshot.repeat_mode = self.snapshot.repeat_mode.cycle();
        if self.snapshot.current_item.is_some() && self.player.is_running() {
            self.player
                .set_repeat_one(self.snapshot.repeat_mode == RepeatMode::One)?;
        }
        Ok(())
    }

    fn current_index(&self) -> Option<usize> {
        let current_id = &self.snapshot.current_item.as_ref()?.id;
        self.snapshot
            .queue
            .iter()
            .position(|item| &item.id == current_id)
    }

    fn advance_after_end(&mut self) -> Result<(), YouTubePlaybackError> {
        let Some(current_index) = self.current_index() else {
            return Ok(());
        };
        self.advance_from(current_index)
    }

    fn advance_from(&mut self, current_index: usize) -> Result<(), YouTubePlaybackError> {
        let queue_len = self.snapshot.queue.len();
        if queue_len == 0 {
            return Ok(());
        }
        if self.snapshot.repeat_mode == RepeatMode::One {
            return self.select_queue_index(current_index);
        }
        if self.snapshot.shuffle_enabled {
            self.reconcile_shuffle_order();
            let Some(current_order_index) = self.current_shuffle_order_index() else {
                return self.finish_queue();
            };
            let next_order_index = if current_order_index + 1 < self.shuffle_order.len() {
                current_order_index + 1
            } else if self.snapshot.repeat_mode == RepeatMode::All {
                0
            } else {
                return self.finish_queue();
            };
            let next_id = &self.shuffle_order[next_order_index];
            let next_index = self
                .snapshot
                .queue
                .iter()
                .position(|item| &item.id == next_id)
                .ok_or_else(|| YouTubePlaybackError::TrackNotFound {
                    id: next_id.clone(),
                })?;
            return self.select_queue_index(next_index);
        }
        if current_index + 1 < queue_len {
            return self.select_queue_index(current_index + 1);
        }
        if self.snapshot.repeat_mode == RepeatMode::All {
            return self.select_queue_index(0);
        }

        self.finish_queue()
    }

    fn finish_queue(&mut self) -> Result<(), YouTubePlaybackError> {
        self.snapshot.queue.clear();
        self.snapshot.current_item = None;
        self.snapshot.status = PlaybackStatus::Paused;
        self.snapshot.position_ms = 0;
        self.reconcile_shuffle_order();
        Ok(())
    }

    fn select_queue_index(&mut self, index: usize) -> Result<(), YouTubePlaybackError> {
        let was_playing = self.snapshot.status == PlaybackStatus::Playing;
        let item = self
            .snapshot
            .queue
            .get(index)
            .cloned()
            .ok_or(YouTubePlaybackError::QueueIndexOutOfBounds { index })?;
        let source_url = self
            .entries
            .iter()
            .find(|entry| entry.item.id == item.id)
            .map(|entry| entry.source_url.clone())
            .ok_or_else(|| YouTubePlaybackError::TrackNotFound {
                id: item.id.clone(),
            })?;

        self.player.load(
            &source_url,
            self.snapshot.volume_percent,
            self.session_cookie_path.as_deref(),
        )?;
        self.player
            .set_repeat_one(self.snapshot.repeat_mode == RepeatMode::One)?;
        if !was_playing {
            self.player.set_paused(true)?;
        }
        self.snapshot.current_item = Some(item);
        self.snapshot.position_ms = 0;
        if was_playing {
            let current_id = self
                .snapshot
                .current_item
                .as_ref()
                .map(|item| item.id.clone())
                .expect("selected queue entries always have an item");
            self.record_playback_start(&current_id, now_epoch_ms()?)?;
        }
        if let Some(item) = self.snapshot.current_item.as_ref() {
            tracing::info!(
                video_id = item.id,
                title = item.title,
                index,
                "queue track selected"
            );
        }
        Ok(())
    }

    fn library_item(&self, id: &str) -> Result<MediaItem, YouTubePlaybackError> {
        self.entries
            .iter()
            .find(|entry| entry.item.id == id)
            .map(|entry| entry.item.clone())
            .ok_or_else(|| YouTubePlaybackError::TrackNotFound { id: id.into() })
    }

    fn items_for_ids(&self, track_ids: &[String]) -> Result<Vec<MediaItem>, YouTubePlaybackError> {
        track_ids.iter().map(|id| self.library_item(id)).collect()
    }

    fn playlists_with_defaults(&self) -> Vec<Playlist> {
        let favorite_track_ids = self
            .playlists
            .iter()
            .find(|playlist| playlist.id == FAVORITES_PLAYLIST_ID)
            .map(|playlist| playlist.track_ids.clone())
            .unwrap_or_default();
        let mut most_played = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.item.play_count > 0)
            .collect::<Vec<_>>();
        most_played.sort_by(|(left_index, left), (right_index, right)| {
            right
                .item
                .play_count
                .cmp(&left.item.play_count)
                .then_with(|| left_index.cmp(right_index))
        });

        let mut playlists = vec![
            Playlist {
                id: FAVORITES_PLAYLIST_ID.into(),
                name: "Favorites".into(),
                track_ids: favorite_track_ids,
            },
            Playlist {
                id: MOST_PLAYED_PLAYLIST_ID.into(),
                name: "Most Played".into(),
                track_ids: most_played
                    .into_iter()
                    .map(|(_, entry)| entry.item.id.clone())
                    .collect(),
            },
        ];
        playlists.extend(
            self.playlists
                .iter()
                .filter(|playlist| !is_default_playlist(&playlist.id))
                .cloned(),
        );
        playlists
    }

    fn reconcile_queue(&mut self) {
        self.snapshot.queue = self
            .snapshot
            .queue
            .iter()
            .filter_map(|item| self.entries.iter().find(|entry| entry.item.id == item.id))
            .map(|entry| entry.item.clone())
            .collect();
        if let Some(current_id) = self
            .snapshot
            .current_item
            .as_ref()
            .map(|item| item.id.as_str())
        {
            self.snapshot.current_item = self
                .entries
                .iter()
                .find(|entry| entry.item.id == current_id)
                .map(|entry| entry.item.clone());
        }
        self.reconcile_shuffle_order();
    }

    fn current_shuffle_order_index(&self) -> Option<usize> {
        let current_id = &self.snapshot.current_item.as_ref()?.id;
        self.shuffle_order.iter().position(|id| id == current_id)
    }

    fn reset_shuffle_order(&mut self, current_id: Option<&str>) {
        let mut shuffled_ids = self
            .snapshot
            .queue
            .iter()
            .filter(|item| Some(item.id.as_str()) != current_id)
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        shuffle_upcoming(
            &mut shuffled_ids,
            0,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
        );
        if let Some(current_id) = current_id {
            shuffled_ids.insert(0, current_id.to_owned());
        }
        self.shuffle_order = shuffled_ids;
    }

    fn reconcile_shuffle_order(&mut self) {
        if !self.snapshot.shuffle_enabled {
            self.shuffle_order.clear();
            return;
        }
        self.shuffle_order
            .retain(|id| self.snapshot.queue.iter().any(|item| item.id == *id));
        for item in &self.snapshot.queue {
            if !self.shuffle_order.iter().any(|id| id == &item.id) {
                self.shuffle_order.push(item.id.clone());
            }
        }
    }

    fn persist_library(&self) -> Result<(), YouTubePlaybackError> {
        let Some(path) = self.library_path.as_ref() else {
            return Ok(());
        };
        write_library(path, &self.entries, &self.playlists)?;
        tracing::info!(
            tracks = self.entries.len(),
            playlists = self.playlists.len(),
            "imported library persisted"
        );
        Ok(())
    }

    fn persist_playback_state(&self) -> Result<(), YouTubePlaybackError> {
        let Some(path) = self.library_path.as_ref() else {
            return Ok(());
        };
        let state = crate::persistence::SavedPlaybackState {
            current_item_id: self
                .snapshot
                .current_item
                .as_ref()
                .map(|item| item.id.clone()),
            position_ms: self.snapshot.position_ms,
            volume_percent: self.snapshot.volume_percent,
            shuffle_enabled: self.snapshot.shuffle_enabled,
            repeat_mode: self.snapshot.repeat_mode,
            queue_ids: self
                .snapshot
                .queue
                .iter()
                .map(|item| item.id.clone())
                .collect(),
            shuffle_order: self.shuffle_order.clone(),
        };
        crate::persistence::save_playback_state(path, &state).map_err(YouTubePlaybackError::Library)
    }

    fn restore_playback_state(&mut self, state: crate::persistence::SavedPlaybackState) {
        self.snapshot.queue = state
            .queue_ids
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| entry.item.id == *id))
            .map(|entry| entry.item.clone())
            .collect();
        self.snapshot.current_item = state.current_item_id.and_then(|id| {
            self.snapshot
                .queue
                .iter()
                .find(|item| item.id == id)
                .cloned()
        });
        self.snapshot.position_ms = self
            .snapshot
            .current_item
            .as_ref()
            .map_or(0, |item| state.position_ms.min(item.duration_ms));
        self.snapshot.status = PlaybackStatus::Paused;
        self.snapshot.volume_percent = state.volume_percent;
        self.snapshot.shuffle_enabled = state.shuffle_enabled;
        self.snapshot.repeat_mode = state.repeat_mode;
        self.shuffle_order = state.shuffle_order;
        self.reconcile_shuffle_order();
    }
}

fn is_default_playlist(id: &str) -> bool {
    id == FAVORITES_PLAYLIST_ID || id == MOST_PLAYED_PLAYLIST_ID
}

impl Default for YouTubePlaybackProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct YtDlpMetadata {
    id: Option<String>,
    title: Option<String>,
    track: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    album_artist: Option<String>,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    release_date: Option<String>,
    upload_date: Option<String>,
    description: Option<String>,
    channel: Option<String>,
    channel_id: Option<String>,
    uploader: Option<String>,
    uploader_id: Option<String>,
    thumbnail: Option<String>,
    categories: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    language: Option<String>,
    availability: Option<String>,
    is_live: Option<bool>,
    view_count: Option<u64>,
    like_count: Option<u64>,
    duration: Option<f64>,
    webpage_url: Option<String>,
    entries: Option<Vec<Option<YtDlpMetadata>>>,
}

pub(crate) fn validate_youtube_url(source_url: &str) -> Result<Url, YouTubePlaybackError> {
    let parsed = Url::parse(source_url.trim()).map_err(|_| YouTubePlaybackError::UnsupportedUrl)?;
    let supported_host = matches!(
        parsed.host_str(),
        Some(
            "youtube.com" | "www.youtube.com" | "music.youtube.com" | "m.youtube.com" | "youtu.be"
        )
    );

    if parsed.scheme() != "https" || !supported_host {
        return Err(YouTubePlaybackError::UnsupportedUrl);
    }

    Ok(parsed)
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn required_metadata_value(value: String, field: &str) -> Result<String, YouTubePlaybackError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(YouTubePlaybackError::InvalidTrackMetadata(format!(
            "{field} must not be empty"
        )));
    }
    Ok(value.into())
}

fn optional_metadata_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| nonempty(Some(value.trim().into())))
}

fn normalize_genres(genres: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for genre in genres {
        let genre = genre.trim();
        if genre.is_empty()
            || normalized
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(genre))
        {
            continue;
        }
        normalized.push(genre.into());
    }
    normalized
}

fn normalize_values(values: Option<Vec<String>>) -> Vec<String> {
    normalize_genres(values.unwrap_or_default())
}

fn normalize_date(value: Option<String>) -> Option<String> {
    let value = nonempty(value)?;
    if value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Some(format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..]));
    }
    Some(value)
}

fn now_epoch_ms() -> Result<u64, YouTubePlaybackError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))
}

fn is_youtube_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn metadata_entry(metadata: YtDlpMetadata, metadata_dirty: bool) -> Option<QueueEntry> {
    let id = nonempty(metadata.id).filter(|id| is_youtube_video_id(id))?;
    let title = nonempty(metadata.track.clone()).or_else(|| nonempty(metadata.title.clone()))?;
    let artist = nonempty(metadata.artist.clone())
        .or_else(|| nonempty(metadata.channel.clone()))
        .or_else(|| nonempty(metadata.uploader.clone()))
        .unwrap_or_else(|| "YouTube".into());
    let duration_ms = metadata
        .duration
        .filter(|duration| duration.is_finite() && *duration >= 0.0)
        .map(|duration| (duration * 1000.0).round() as u64)
        .unwrap_or(0);
    let source_url = metadata
        .webpage_url
        .and_then(|url| validate_youtube_url(&url).ok().map(|url| url.to_string()))
        .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={id}"));

    Some(QueueEntry {
        item: MediaItem {
            id,
            provider: "youtube".into(),
            source_url: Some(source_url.clone()),
            title,
            artist,
            album: nonempty(metadata.album),
            album_artist: nonempty(metadata.album_artist),
            track_number: metadata.track_number,
            disc_number: metadata.disc_number,
            release_date: normalize_date(metadata.release_date),
            upload_date: normalize_date(metadata.upload_date),
            description: nonempty(metadata.description),
            channel: nonempty(metadata.channel),
            channel_id: nonempty(metadata.channel_id),
            uploader: nonempty(metadata.uploader),
            uploader_id: nonempty(metadata.uploader_id),
            thumbnail_url: nonempty(metadata.thumbnail),
            label: None,
            genres: Vec::new(),
            categories: normalize_values(metadata.categories),
            tags: normalize_values(metadata.tags),
            language: nonempty(metadata.language),
            availability: nonempty(metadata.availability),
            is_live: metadata.is_live.unwrap_or(false),
            view_count: metadata.view_count,
            like_count: metadata.like_count,
            duration_ms,
            metadata_dirty,
            play_count: 0,
            last_played_at_ms: None,
            play_history_ms: Vec::new(),
        },
        source_url,
    })
}

fn flatten_metadata(metadata: YtDlpMetadata, candidates: &mut Vec<YtDlpMetadata>) {
    match metadata {
        YtDlpMetadata {
            entries: Some(entries),
            ..
        } => {
            for entry in entries.into_iter().flatten() {
                flatten_metadata(entry, candidates);
            }
        }
        leaf => candidates.push(leaf),
    }
}

fn parse_import_metadata_with_dirty_state(
    output: &str,
    metadata_dirty: bool,
) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    let metadata: YtDlpMetadata = serde_json::from_str(output)
        .map_err(|error| YouTubePlaybackError::Metadata(error.to_string()))?;
    let mut candidates = Vec::new();
    flatten_metadata(metadata, &mut candidates);
    let mut seen = HashSet::new();
    let tracks = candidates
        .into_iter()
        .filter_map(|metadata| metadata_entry(metadata, metadata_dirty))
        .filter(|entry| seen.insert(entry.item.id.clone()))
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return Err(YouTubePlaybackError::Metadata(
            "no playable tracks found in the YouTube URL".into(),
        ));
    }

    Ok(tracks)
}

#[cfg(test)]
fn parse_import_metadata(output: &str) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    parse_import_metadata_with_dirty_state(output, false)
}

fn merge_queue_entries(entries: &mut Vec<QueueEntry>, imported: Vec<QueueEntry>) {
    for mut entry in imported {
        if let Some(existing) = entries
            .iter_mut()
            .find(|existing| existing.item.id == entry.item.id)
        {
            entry.item.label = existing.item.label.clone();
            entry.item.genres = existing.item.genres.clone();
            entry.item.play_count = existing.item.play_count;
            entry.item.last_played_at_ms = existing.item.last_played_at_ms;
            entry.item.play_history_ms = existing.item.play_history_ms.clone();
            *existing = entry;
        } else {
            entries.push(entry);
        }
    }
}

#[derive(Deserialize, Serialize)]
struct StoredLibrary {
    version: u32,
    entries: Vec<QueueEntry>,
    #[serde(default)]
    playlists: Vec<Playlist>,
}

fn load_library(path: &Path) -> Result<StoredLibrary, YouTubePlaybackError> {
    let (entries, playlists) =
        crate::persistence::load_library(path).map_err(YouTubePlaybackError::Library)?;
    validate_stored_playlists(&entries, &playlists)?;
    Ok(StoredLibrary {
        version: LIBRARY_VERSION,
        entries,
        playlists,
    })
}

fn load_legacy_library(path: &Path) -> Result<StoredLibrary, YouTubePlaybackError> {
    let contents = fs::read_to_string(path)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    let library: StoredLibrary = serde_json::from_str(&contents)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    if !(1..=LIBRARY_VERSION).contains(&library.version) {
        return Err(YouTubePlaybackError::Library(format!(
            "unsupported library version {}",
            library.version
        )));
    }
    for entry in &library.entries {
        if !is_youtube_video_id(&entry.item.id) || validate_youtube_url(&entry.source_url).is_err()
        {
            return Err(YouTubePlaybackError::Library(
                "the library contains an invalid YouTube track".into(),
            ));
        }
    }

    validate_stored_playlists(&library.entries, &library.playlists)?;
    Ok(library)
}

fn validate_stored_playlists(
    entries: &[QueueEntry],
    playlists: &[Playlist],
) -> Result<(), YouTubePlaybackError> {
    let mut playlist_ids = HashSet::new();
    for playlist in playlists {
        if playlist.id.trim().is_empty() || playlist.name.trim().is_empty() {
            return Err(YouTubePlaybackError::Library(
                "the library contains an invalid playlist".into(),
            ));
        }
        if !playlist_ids.insert(&playlist.id) {
            return Err(YouTubePlaybackError::Library(
                "the library contains duplicate playlist IDs".into(),
            ));
        }
        for track_id in &playlist.track_ids {
            if !entries.iter().any(|entry| entry.item.id == *track_id) {
                return Err(YouTubePlaybackError::Library(
                    "the library playlist references an unknown track".into(),
                ));
            }
        }
    }
    Ok(())
}

fn write_library(
    path: &Path,
    entries: &[QueueEntry],
    playlists: &[Playlist],
) -> Result<(), YouTubePlaybackError> {
    crate::persistence::save_library(path, entries, playlists)
        .map_err(YouTubePlaybackError::Library)
}

pub(crate) fn resolve_youtube_imports(
    source_urls: &[String],
    cookie_path: Option<&Path>,
    report_progress: impl FnMut(usize, usize, bool, usize, usize),
) -> Result<ResolvedYouTubeImport, YouTubePlaybackError> {
    resolve_youtube_imports_with(
        source_urls,
        cookie_path,
        metadata_arguments,
        false,
        report_progress,
    )
}

pub(crate) fn discover_youtube_imports(
    source_urls: &[String],
    cookie_path: Option<&Path>,
    report_progress: impl FnMut(usize, usize, bool, usize, usize),
) -> Result<ResolvedYouTubeImport, YouTubePlaybackError> {
    resolve_youtube_imports_with(
        source_urls,
        cookie_path,
        discovery_metadata_arguments,
        true,
        report_progress,
    )
}

fn resolve_youtube_imports_with(
    source_urls: &[String],
    cookie_path: Option<&Path>,
    metadata_args: impl Fn(&str, Option<&Path>) -> Vec<String>,
    metadata_dirty: bool,
    mut report_progress: impl FnMut(usize, usize, bool, usize, usize),
) -> Result<ResolvedYouTubeImport, YouTubePlaybackError> {
    if source_urls.is_empty() {
        return Err(YouTubePlaybackError::Metadata(
            "at least one YouTube URL is required".into(),
        ));
    }

    let parsed_urls = source_urls
        .iter()
        .map(|source_url| validate_youtube_url(source_url))
        .collect::<Result<Vec<_>, _>>()?;
    tracing::info!(
        sources = parsed_urls.len(),
        authenticated = cookie_path.is_some(),
        "YouTube batch import started"
    );

    let started = Instant::now();
    let mut imported = Vec::new();
    let mut skipped_member_only = 0;
    let mut skipped_member_only_ids = HashSet::new();
    let mut timed_out_sources = 0;
    let total_sources = parsed_urls.len();
    for (source_index, parsed) in parsed_urls.into_iter().enumerate() {
        let source_host = parsed.host_str().unwrap_or("unknown").to_owned();
        report_progress(
            source_index,
            total_sources,
            false,
            imported.len(),
            skipped_member_only,
        );
        tracing::debug!(source_index, source_host, "resolving YouTube import source");
        let resolved = resolve_metadata_with_progress(
            parsed.as_str(),
            cookie_path,
            metadata_args(parsed.as_str(), cookie_path),
            metadata_dirty,
            |source_tracks| {
                report_progress(
                    source_index,
                    total_sources,
                    false,
                    imported.len() + source_tracks,
                    skipped_member_only,
                );
            },
        )?;
        skipped_member_only += resolved.skipped_member_only;
        skipped_member_only_ids.extend(resolved.skipped_member_only_ids);
        timed_out_sources += usize::from(resolved.timed_out);
        merge_queue_entries(&mut imported, resolved.entries);
        report_progress(
            source_index + 1,
            total_sources,
            true,
            imported.len(),
            skipped_member_only,
        );
    }
    tracing::info!(
        tracks = imported.len(),
        elapsed_ms = started.elapsed().as_millis(),
        "YouTube batch metadata resolved"
    );
    if imported.is_empty() {
        return Err(YouTubePlaybackError::Metadata(
            "no playable tracks found".into(),
        ));
    }

    Ok(ResolvedYouTubeImport {
        entries: imported,
        skipped_member_only,
        skipped_member_only_ids,
        timed_out_sources,
    })
}

fn metadata_arguments(source_url: &str, cookie_path: Option<&Path>) -> Vec<String> {
    let mut arguments = vec![
        "--dump-json".into(),
        "--yes-playlist".into(),
        "--lazy-playlist".into(),
        "--ignore-errors".into(),
        "--retries".into(),
        "1".into(),
        "--extractor-retries".into(),
        "1".into(),
        "--skip-download".into(),
        "--no-warnings".into(),
        "--socket-timeout".into(),
        "30".into(),
    ];
    if let Some(cookie_path) = cookie_path {
        arguments.push("--cookies".into());
        arguments.push(cookie_path.to_string_lossy().into_owned());
    }
    arguments.push(source_url.into());
    arguments
}

fn discovery_metadata_arguments(source_url: &str, cookie_path: Option<&Path>) -> Vec<String> {
    let mut arguments = vec![
        "--dump-json".into(),
        "--flat-playlist".into(),
        "--yes-playlist".into(),
        "--ignore-errors".into(),
        "--skip-download".into(),
        "--no-warnings".into(),
        "--socket-timeout".into(),
        "30".into(),
    ];
    if let Some(cookie_path) = cookie_path {
        arguments.push("--cookies".into());
        arguments.push(cookie_path.to_string_lossy().into_owned());
    }
    arguments.push(source_url.into());
    arguments
}

#[cfg(test)]
fn resolve_metadata(
    source_url: &str,
    cookie_path: Option<&Path>,
) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    Ok(resolve_metadata_with_progress(
        source_url,
        cookie_path,
        metadata_arguments(source_url, cookie_path),
        false,
        |_| {},
    )?
    .entries)
}

fn resolve_metadata_with_progress(
    _source_url: &str,
    cookie_path: Option<&Path>,
    arguments: Vec<String>,
    metadata_dirty: bool,
    mut report_tracks: impl FnMut(usize),
) -> Result<MetadataResolution, YouTubePlaybackError> {
    let yt_dlp = find_executable(
        "GMUSIC_YT_DLP_PATH",
        &["/opt/homebrew/bin/yt-dlp", "/usr/local/bin/yt-dlp"],
        "yt-dlp",
    );
    tracing::debug!(
        executable = %yt_dlp.display(),
        authenticated = cookie_path.is_some(),
        "starting yt-dlp metadata resolution"
    );
    let mut child = Command::new(&yt_dlp)
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| YouTubePlaybackError::DependencyUnavailable {
            name: "yt-dlp",
            detail: error.to_string(),
        })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        YouTubePlaybackError::Metadata("yt-dlp did not provide a metadata stream".into())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        YouTubePlaybackError::Metadata("yt-dlp did not provide an error stream".into())
    })?;
    let (sender, receiver) = mpsc::channel();
    let stdout_reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut output);
        output
    });

    let mut last_progress = Instant::now();
    let mut entries = Vec::new();
    let mut timed_out = false;
    loop {
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(Ok(line)) => {
                let streamed = parse_import_metadata_with_dirty_state(&line, metadata_dirty)?;
                merge_queue_entries(&mut entries, streamed);
                last_progress = Instant::now();
                report_tracks(entries.len());
            }
            Ok(Err(error)) => return Err(YouTubePlaybackError::Metadata(error.to_string())),
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if child
                    .try_wait()
                    .map_err(|error| YouTubePlaybackError::Metadata(error.to_string()))?
                    .is_some()
                {
                    break;
                }
                if last_progress.elapsed() >= METADATA_IDLE_TIMEOUT {
                    timed_out = true;
                    let _ = child.kill();
                    break;
                }
            }
        }
    }
    let _ = stdout_reader.join();
    while let Ok(Ok(line)) = receiver.try_recv() {
        let streamed = parse_import_metadata_with_dirty_state(&line, metadata_dirty)?;
        merge_queue_entries(&mut entries, streamed);
        report_tracks(entries.len());
    }
    let status = child
        .wait()
        .map_err(|error| YouTubePlaybackError::Metadata(error.to_string()))?;
    let stderr = stderr_reader.join().unwrap_or_default();
    let resolution = finish_metadata_resolution(entries, &stderr, timed_out)?;
    if !status.success() {
        tracing::warn!(
            status = %status,
            skipped_member_only = resolution.skipped_member_only,
            tracks = resolution.entries.len(),
            "yt-dlp skipped unavailable import entries"
        );
    }
    if resolution.timed_out {
        tracing::warn!(
            idle_timeout_seconds = METADATA_IDLE_TIMEOUT.as_secs(),
            tracks = resolution.entries.len(),
            "yt-dlp metadata resolution stopped after inactivity"
        );
    }
    tracing::debug!(
        tracks = resolution.entries.len(),
        skipped_member_only = resolution.skipped_member_only,
        "yt-dlp metadata received"
    );

    Ok(resolution)
}

#[cfg(test)]
fn parse_streamed_metadata(output: &str) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    let mut entries = Vec::new();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        merge_queue_entries(&mut entries, parse_import_metadata(line)?);
    }
    if entries.is_empty() {
        return Err(YouTubePlaybackError::Metadata(
            "no playable tracks found".into(),
        ));
    }

    Ok(entries)
}

fn member_only_error_count(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|line| line.contains("members-only content") || line.contains("members on level"))
        .count()
}

fn member_only_video_ids(stderr: &str) -> HashSet<String> {
    stderr
        .lines()
        .filter(|line| line.contains("members-only content") || line.contains("members on level"))
        .filter_map(|line| line.split("ERROR: [youtube] ").nth(1))
        .filter_map(|line| line.split(':').next())
        .filter(|id| is_youtube_video_id(id))
        .map(str::to_owned)
        .collect()
}

fn metadata_error_message(stderr: &str, playable_tracks: usize) -> String {
    let skipped_member_only = member_only_error_count(stderr);
    if playable_tracks == 0 && skipped_member_only > 0 {
        return format!(
            "no playable tracks found; skipped {skipped_member_only} members-only tracks"
        );
    }

    let detail = stderr
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("");
    if detail.is_empty() {
        "no playable tracks found".into()
    } else {
        format!("could not resolve YouTube metadata: {detail}")
    }
}

fn finish_metadata_resolution(
    entries: Vec<QueueEntry>,
    stderr: &str,
    timed_out: bool,
) -> Result<MetadataResolution, YouTubePlaybackError> {
    let skipped_member_only = member_only_error_count(stderr);
    if entries.is_empty() {
        if timed_out {
            return Err(YouTubePlaybackError::Metadata(format!(
                "metadata resolution stopped after {} seconds without a playable track",
                METADATA_IDLE_TIMEOUT.as_secs()
            )));
        }
        return Err(YouTubePlaybackError::Metadata(metadata_error_message(
            stderr, 0,
        )));
    }

    Ok(MetadataResolution {
        entries,
        skipped_member_only,
        skipped_member_only_ids: member_only_video_ids(stderr),
        timed_out,
    })
}

struct PlayerState {
    eof_reached: bool,
    paused: bool,
    position_ms: u64,
}

struct MpvPlayer {
    child: Option<Child>,
    request_id: u64,
    session_cookie_path: Option<PathBuf>,
    socket_path: PathBuf,
    watchdog: Option<Child>,
}

impl MpvPlayer {
    fn new() -> Self {
        Self {
            child: None,
            request_id: 0,
            session_cookie_path: None,
            socket_path: env::temp_dir().join(format!("gmusic-mpv-{}.sock", std::process::id())),
            watchdog: None,
        }
    }

    fn is_running(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none())
    }

    fn has_loaded_file(&mut self) -> Result<bool, YouTubePlaybackError> {
        if !self.is_running() {
            return Ok(false);
        }
        Ok(self
            .send(json!(["get_property", "path"]))?
            .as_str()
            .is_some())
    }

    fn wait_until_loaded(&mut self) -> Result<(), YouTubePlaybackError> {
        let deadline = Instant::now() + MEDIA_LOAD_TIMEOUT;
        while Instant::now() < deadline {
            if self.has_loaded_file()? {
                return Ok(());
            }
            if !self.is_running() {
                return Err(YouTubePlaybackError::Player(
                    "mpv exited while loading media".into(),
                ));
            }
            thread::sleep(MEDIA_LOAD_POLL_INTERVAL);
        }
        Err(YouTubePlaybackError::Player(
            "timed out while loading media".into(),
        ))
    }

    fn load(
        &mut self,
        source_url: &str,
        volume_percent: u8,
        cookie_path: Option<&Path>,
    ) -> Result<(), YouTubePlaybackError> {
        if self.session_cookie_path.as_deref() != cookie_path {
            tracing::info!(
                authenticated = cookie_path.is_some(),
                "mpv session changed; restarting player"
            );
            self.stop();
            self.session_cookie_path = cookie_path.map(Path::to_path_buf);
        }
        self.ensure_started(volume_percent)?;
        tracing::debug!("sending track to mpv");
        self.send(json!(["loadfile", source_url, "replace"]))?;
        self.wait_until_loaded()
    }

    fn set_paused(&mut self, paused: bool) -> Result<(), YouTubePlaybackError> {
        self.send(json!(["set_property", "pause", paused]))?;
        Ok(())
    }

    fn set_repeat_one(&mut self, enabled: bool) -> Result<(), YouTubePlaybackError> {
        self.send(json!([
            "set_property",
            "loop-file",
            if enabled { "inf" } else { "no" }
        ]))?;
        Ok(())
    }

    fn seek(&mut self, position_ms: u64) -> Result<(), YouTubePlaybackError> {
        self.send(json!([
            "set_property",
            "time-pos",
            position_ms as f64 / 1000.0
        ]))?;
        Ok(())
    }

    fn set_volume(&mut self, volume_percent: u8) -> Result<(), YouTubePlaybackError> {
        self.send_without_reply(json!(["set_property", "volume", volume_percent]))?;
        Ok(())
    }

    fn inspect(&mut self) -> Result<PlayerState, YouTubePlaybackError> {
        let eof_reached = self
            .send(json!(["get_property", "eof-reached"]))?
            .as_bool()
            .unwrap_or(false);
        let idle_active = self
            .send(json!(["get_property", "idle-active"]))?
            .as_bool()
            .unwrap_or(false);
        let paused = self
            .send(json!(["get_property", "pause"]))?
            .as_bool()
            .unwrap_or(true);
        let position_seconds = self
            .send(json!(["get_property", "time-pos"]))?
            .as_f64()
            .unwrap_or(0.0);

        Ok(PlayerState {
            eof_reached: eof_reached || idle_active,
            paused,
            position_ms: (position_seconds.max(0.0) * 1000.0).round() as u64,
        })
    }

    fn ensure_started(&mut self, volume_percent: u8) -> Result<(), YouTubePlaybackError> {
        if self
            .child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none())
        {
            tracing::debug!("reusing running mpv process");
            return Ok(());
        }

        self.stop();
        let _ = fs::remove_file(&self.socket_path);
        let mpv = find_executable(
            "GMUSIC_MPV_PATH",
            &["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv"],
            "mpv",
        );
        let mut command = Command::new(&mpv);
        tracing::info!(
            executable = %mpv.display(),
            volume_percent,
            authenticated = self.session_cookie_path.is_some(),
            "starting mpv audio process"
        );
        command
            .args(mpv_arguments(
                &self.socket_path,
                volume_percent,
                self.session_cookie_path.as_deref(),
            ))
            .env("PATH", playback_path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child =
            command
                .spawn()
                .map_err(|error| YouTubePlaybackError::DependencyUnavailable {
                    name: "mpv",
                    detail: error.to_string(),
                })?;
        tracing::info!(pid = child.id(), "mpv process started");
        self.child = Some(child);

        let deadline = Instant::now() + IPC_TIMEOUT;
        while Instant::now() < deadline {
            if self.socket_path.exists() {
                tracing::debug!("mpv IPC socket is ready");
                let child_pid = self.child.as_ref().map(Child::id).ok_or_else(|| {
                    YouTubePlaybackError::Player("mpv exited before opening its IPC socket".into())
                })?;
                self.watchdog = Some(spawn_parent_exit_watchdog(std::process::id(), child_pid)?);
                return Ok(());
            }
            if let Some(status) = self
                .child
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
            {
                return Err(YouTubePlaybackError::Player(format!(
                    "mpv exited before opening its IPC socket ({status})"
                )));
            }
            thread::sleep(Duration::from_millis(25));
        }

        Err(YouTubePlaybackError::Player(
            "timed out while starting mpv".into(),
        ))
    }

    fn send(&mut self, command: Value) -> Result<Value, YouTubePlaybackError> {
        self.request_id += 1;
        let request_id = self.request_id;
        let command_name = command
            .as_array()
            .and_then(|parts| parts.first())
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if command_name == "get_property" {
            tracing::trace!(
                request_id,
                command = command_name,
                "sending mpv IPC command"
            );
        } else {
            tracing::debug!(
                request_id,
                command = command_name,
                "sending mpv IPC command"
            );
        }
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        stream
            .set_read_timeout(Some(IPC_TIMEOUT))
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        let request = json!({
            "command": command,
            "request_id": request_id,
        });
        serde_json::to_writer(&mut stream, &request)
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        stream
            .write_all(b"\n")
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            let bytes = reader
                .read_line(&mut line)
                .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
            if bytes == 0 {
                return Err(YouTubePlaybackError::Player(
                    "mpv closed the IPC connection".into(),
                ));
            }
            let response: Value = serde_json::from_str(&line)
                .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
            if response.get("request_id").and_then(Value::as_u64) != Some(request_id) {
                continue;
            }
            let error = response.get("error").and_then(Value::as_str);
            if error == Some("property unavailable") {
                tracing::debug!(
                    request_id,
                    command = command_name,
                    "mpv property is unavailable"
                );
                return Ok(Value::Null);
            }
            if error != Some("success") {
                return Err(YouTubePlaybackError::Player(
                    error.unwrap_or("unknown mpv error").to_owned(),
                ));
            }
            if command_name == "get_property" {
                tracing::trace!(
                    request_id,
                    command = command_name,
                    "mpv IPC command completed"
                );
            } else {
                tracing::debug!(
                    request_id,
                    command = command_name,
                    "mpv IPC command completed"
                );
            }
            return Ok(response.get("data").cloned().unwrap_or(Value::Null));
        }
    }

    fn send_without_reply(&mut self, command: Value) -> Result<(), YouTubePlaybackError> {
        self.request_id += 1;
        let request_id = self.request_id;
        let command_name = command
            .as_array()
            .and_then(|parts| parts.first())
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        tracing::debug!(
            request_id,
            command = command_name,
            "sending one-way mpv IPC command"
        );
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        let request = json!({
            "command": command,
            "request_id": request_id,
        });
        serde_json::to_writer(&mut stream, &request)
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        stream
            .write_all(b"\n")
            .map_err(|error| YouTubePlaybackError::Player(error.to_string()))?;
        tracing::debug!(
            request_id,
            command = command_name,
            "one-way mpv IPC command sent"
        );
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut watchdog) = self.watchdog.take() {
            let _ = watchdog.kill();
            let _ = watchdog.wait();
        }
        if self.child.is_some() {
            tracing::info!("stopping mpv process");
            let _ = self.send(json!(["quit"]));
        }
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_file(&self.socket_path);
    }
}

fn spawn_parent_exit_watchdog(
    parent_pid: u32,
    player_pid: u32,
) -> Result<Child, YouTubePlaybackError> {
    Command::new("/bin/sh")
        .args([
            "-c",
            PARENT_EXIT_WATCHDOG,
            "gmusic-mpv-watchdog",
            &parent_pid.to_string(),
            &player_pid.to_string(),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| YouTubePlaybackError::Player(error.to_string()))
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn mpv_arguments(
    socket_path: &Path,
    volume_percent: u8,
    cookie_path: Option<&Path>,
) -> Vec<String> {
    let mut arguments = vec![
        "--no-config".into(),
        "--idle=yes".into(),
        "--no-terminal".into(),
        "--video=no".into(),
        "--audio-display=no".into(),
        "--ytdl-format=bestaudio".into(),
        format!("--input-ipc-server={}", socket_path.display()),
        format!("--volume={volume_percent}"),
    ];
    if let Some(cookie_path) = cookie_path {
        arguments.push(format!(
            "--ytdl-raw-options=cookies={}",
            cookie_path.display()
        ));
    }
    arguments
}

fn find_executable(environment_variable: &str, candidates: &[&str], fallback: &str) -> PathBuf {
    if let Some(path) = env::var_os(environment_variable).filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    candidates
        .iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn playback_path() -> String {
    let inherited = env::var("PATH").unwrap_or_default();
    format!("/opt/homebrew/bin:/usr/local/bin:{inherited}")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{BufRead, BufReader, Write},
        os::unix::net::UnixListener,
        process::{Command, Stdio},
        sync::mpsc,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use std::path::Path;

    use super::{
        LIBRARY_VERSION, MpvPlayer, StoredLibrary, YouTubePlaybackProvider,
        discovery_metadata_arguments, finish_metadata_resolution, member_only_error_count,
        member_only_video_ids, merge_queue_entries, metadata_arguments, metadata_error_message,
        mpv_arguments, parse_import_metadata, parse_import_metadata_with_dirty_state,
        parse_streamed_metadata, resolve_metadata, spawn_parent_exit_watchdog,
        validate_youtube_url,
    };
    use crate::playback::{
        EditableTrackMetadata, PlaybackSnapshot, PlaybackStatus, Playlist, RepeatMode,
    };

    #[test]
    fn accepts_supported_youtube_urls() {
        for url in [
            "https://www.youtube.com/watch?v=M7lc1UVf-VE",
            "https://music.youtube.com/watch?v=M7lc1UVf-VE",
            "https://youtu.be/M7lc1UVf-VE",
        ] {
            assert!(validate_youtube_url(url).is_ok(), "rejected {url}");
        }
    }

    #[test]
    fn rejects_non_youtube_urls() {
        let error = validate_youtube_url("https://example.com/watch?v=M7lc1UVf-VE")
            .expect_err("non-YouTube hosts must be rejected");

        assert_eq!(error.to_string(), "only YouTube URLs are supported");
    }

    #[test]
    fn sends_volume_ipc_without_waiting_for_mpv_reply() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let socket_path = std::env::temp_dir().join(format!(
            "gmusic-volume-ipc-{}-{unique}.sock",
            std::process::id()
        ));
        let listener = UnixListener::bind(&socket_path).expect("volume test socket should bind");
        let (request_tx, request_rx) = mpsc::channel();
        let (reply_tx, reply_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("volume request should connect");
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            reader
                .read_line(&mut request)
                .expect("volume request should be readable");
            request_tx
                .send(request)
                .expect("volume request should be reported");
            reply_rx.recv().expect("test should release mpv reply");
            let mut stream = reader.into_inner();
            let _ = stream.write_all(b"{\"request_id\":1,\"error\":\"success\"}\n");
        });

        let mut player = MpvPlayer::new();
        player.socket_path = socket_path.clone();
        let (result_tx, result_rx) = mpsc::channel();
        thread::spawn(move || {
            result_tx
                .send(player.set_volume(45))
                .expect("volume result should be reported");
        });

        let request = request_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("mpv should receive the volume command");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&request)
                .expect("volume request should be JSON")["command"],
            serde_json::json!(["set_property", "volume", 45])
        );
        let result_before_reply = result_rx.recv_timeout(Duration::from_millis(100)).ok();
        let returned_before_reply = result_before_reply.is_some();
        reply_tx.send(()).expect("mpv reply should be released");
        let result = result_before_reply.unwrap_or_else(|| {
            result_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("volume command should complete after the reply")
        });
        server.join().expect("volume IPC server should stop");
        let _ = fs::remove_file(socket_path);

        assert!(
            returned_before_reply,
            "volume command waited for the mpv IPC response"
        );
        result.expect("volume command should succeed");
    }

    #[test]
    fn shutdown_terminates_the_running_mpv_process() {
        let mut provider = YouTubePlaybackProvider::new();
        let child = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("sleep fixture should start");
        let pid = child.id();
        provider.player.child = Some(child);

        provider.shutdown();

        let status = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .expect("kill should inspect the fixture process");
        assert!(!status.success(), "mpv process {pid} should be stopped");
    }

    #[test]
    fn play_reloads_the_selected_track_when_mpv_is_alive_but_idle() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let socket_path = std::env::temp_dir().join(format!(
            "gmusic-idle-mpv-{}-{unique}.sock",
            std::process::id()
        ));
        let listener = UnixListener::bind(&socket_path).expect("idle player socket should bind");
        let server = thread::spawn(move || {
            let mut commands = Vec::new();
            let mut loaded = false;
            loop {
                let (mut stream, _) = listener.accept().expect("player command should connect");
                let mut request = String::new();
                BufReader::new(
                    stream
                        .try_clone()
                        .expect("player stream should be cloneable"),
                )
                .read_line(&mut request)
                .expect("player command should be readable");
                let request: serde_json::Value =
                    serde_json::from_str(&request).expect("player command should be JSON");
                let command = request["command"].clone();
                let request_id = request["request_id"].clone();
                let response = if command == serde_json::json!(["get_property", "path"]) && !loaded
                {
                    serde_json::json!({
                        "request_id": request_id,
                        "error": "property unavailable"
                    })
                } else if command == serde_json::json!(["get_property", "path"]) {
                    serde_json::json!({
                        "request_id": request_id,
                        "error": "success",
                        "data": "https://www.youtube.com/watch?v=M7lc1UVf-VE"
                    })
                } else {
                    serde_json::json!({"request_id": request_id, "error": "success"})
                };
                if command
                    .as_array()
                    .and_then(|parts| parts.first())
                    .and_then(serde_json::Value::as_str)
                    == Some("loadfile")
                {
                    loaded = true;
                }
                commands.push(command.clone());
                serde_json::to_writer(&mut stream, &response)
                    .expect("player response should be writable");
                stream
                    .write_all(b"\n")
                    .expect("player response should terminate");
                if command == serde_json::json!(["set_property", "pause", false]) {
                    break;
                }
            }
            commands
        });

        let entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Track","channel":"Artist","duration":120}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);
        provider
            .replace_queue(&["M7lc1UVf-VE".into()])
            .expect("known library tracks create a play queue");
        provider.snapshot.current_item = Some(provider.snapshot.queue[0].clone());
        provider.player.socket_path = socket_path.clone();
        provider.player.child = Some(
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("idle player fixture should start"),
        );

        provider.play().expect("the selected track should resume");

        let commands = server.join().expect("idle player server should stop");
        let mut child = provider
            .player
            .child
            .take()
            .expect("idle player fixture should still exist");
        child.kill().expect("idle player fixture should stop");
        child.wait().expect("idle player fixture should be reaped");
        let _ = fs::remove_file(socket_path);

        assert!(
            commands.iter().any(|command| {
                command
                    .as_array()
                    .and_then(|parts| parts.first())
                    .and_then(serde_json::Value::as_str)
                    == Some("loadfile")
            }),
            "an idle mpv process must reload the selected track"
        );
    }

    #[test]
    fn load_waits_until_mpv_reports_a_media_path() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let socket_path = std::env::temp_dir().join(format!(
            "gmusic-loading-mpv-{}-{unique}.sock",
            std::process::id()
        ));
        let listener = UnixListener::bind(&socket_path).expect("loading player socket should bind");
        listener
            .set_nonblocking(true)
            .expect("loading player socket should become non-blocking");
        let server = thread::spawn(move || {
            let mut path_checks = 0;
            let mut idle_deadline = Instant::now() + Duration::from_millis(100);
            loop {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= idle_deadline {
                            break;
                        }
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("loading player socket failed: {error}"),
                };
                idle_deadline = Instant::now() + Duration::from_millis(100);
                let mut request = String::new();
                BufReader::new(
                    stream
                        .try_clone()
                        .expect("player stream should be cloneable"),
                )
                .read_line(&mut request)
                .expect("player command should be readable");
                let request: serde_json::Value =
                    serde_json::from_str(&request).expect("player command should be JSON");
                let command = &request["command"];
                let request_id = request["request_id"].clone();
                let response = if command == &serde_json::json!(["get_property", "path"]) {
                    path_checks += 1;
                    if path_checks < 2 {
                        serde_json::json!({
                            "request_id": request_id,
                            "error": "property unavailable"
                        })
                    } else {
                        serde_json::json!({
                            "request_id": request_id,
                            "error": "success",
                            "data": "https://www.youtube.com/watch?v=M7lc1UVf-VE"
                        })
                    }
                } else {
                    serde_json::json!({"request_id": request_id, "error": "success"})
                };
                serde_json::to_writer(&mut stream, &response)
                    .expect("player response should be writable");
                stream
                    .write_all(b"\n")
                    .expect("player response should terminate");
                if path_checks == 2 {
                    break;
                }
            }
            path_checks
        });

        let mut player = MpvPlayer::new();
        player.socket_path = socket_path.clone();
        player.child = Some(
            Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("loading player fixture should start"),
        );

        player
            .load("https://www.youtube.com/watch?v=M7lc1UVf-VE", 72, None)
            .expect("player load should complete after media is ready");

        let path_checks = server.join().expect("loading player server should stop");
        let mut child = player
            .child
            .take()
            .expect("loading player fixture should still exist");
        child.kill().expect("loading player fixture should stop");
        child
            .wait()
            .expect("loading player fixture should be reaped");
        let _ = fs::remove_file(socket_path);

        assert_eq!(
            path_checks, 2,
            "load must remain pending until mpv detects the media"
        );
    }

    #[test]
    fn inspect_treats_mpv_idle_after_a_track_ends_as_end_of_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let socket_path = std::env::temp_dir().join(format!(
            "gmusic-idle-inspect-{}-{unique}.sock",
            std::process::id()
        ));
        let listener = UnixListener::bind(&socket_path).expect("idle inspect socket should bind");
        listener
            .set_nonblocking(true)
            .expect("idle inspect socket should become non-blocking");
        let server = thread::spawn(move || {
            let mut commands = Vec::new();
            let mut idle_deadline = Instant::now() + Duration::from_millis(100);
            loop {
                let (mut stream, _) = match listener.accept() {
                    Ok(connection) => connection,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= idle_deadline {
                            break;
                        }
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("idle inspect socket failed: {error}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("idle inspect player stream should become blocking");
                idle_deadline = Instant::now() + Duration::from_millis(100);
                let mut request = String::new();
                BufReader::new(
                    stream
                        .try_clone()
                        .expect("player stream should be cloneable"),
                )
                .read_line(&mut request)
                .expect("player command should be readable");
                let request: serde_json::Value =
                    serde_json::from_str(&request).expect("player command should be JSON");
                let command = request["command"].clone();
                let request_id = request["request_id"].clone();
                let response = if command == serde_json::json!(["get_property", "pause"]) {
                    serde_json::json!({"request_id": request_id, "error": "success", "data": false})
                } else if command == serde_json::json!(["get_property", "idle-active"]) {
                    serde_json::json!({"request_id": request_id, "error": "success", "data": true})
                } else {
                    serde_json::json!({"request_id": request_id, "error": "property unavailable"})
                };
                commands.push(command);
                serde_json::to_writer(&mut stream, &response)
                    .expect("player response should be writable");
                stream
                    .write_all(b"\n")
                    .expect("player response should terminate");
            }
            commands
        });

        let mut player = MpvPlayer::new();
        player.socket_path = socket_path.clone();

        let state = player.inspect().expect("idle player state is inspectable");
        let commands = server.join().expect("idle inspect server should stop");
        let _ = fs::remove_file(socket_path);

        assert!(state.eof_reached);
        assert!(!state.paused);
        assert_eq!(state.position_ms, 0);
        assert!(
            commands.contains(&serde_json::json!(["get_property", "idle-active"])),
            "mpv idle state must be inspected after its end-of-file property is unavailable"
        );
    }

    #[test]
    fn parent_exit_watchdog_terminates_mpv_after_an_ungraceful_app_exit() {
        let mut app = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("app fixture should start");
        let mut player = Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("player fixture should start");
        let player_pid = player.id();
        let mut watchdog = spawn_parent_exit_watchdog(app.id(), player_pid)
            .expect("watchdog fixture should start");

        app.kill().expect("app fixture should stop");
        app.wait().expect("app fixture should reap");
        watchdog.wait().expect("watchdog should exit after cleanup");

        assert!(
            player
                .try_wait()
                .expect("player fixture status should be readable")
                .is_some(),
            "player process {player_pid} should stop when its app exits"
        );
    }

    #[test]
    fn parses_rich_yt_dlp_metadata_into_an_imported_track() {
        let tracks = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Video title","track":"YouTube Developers Live","artist":"Google for Developers","album":"API Sessions","album_artist":"Google","track_number":3,"disc_number":1,"release_date":"20250102","upload_date":"20250103","description":"A complete metadata fixture.","channel":"Google Developers","channel_id":"UC_x5XG1OV2P6uZZ5FSM9Ttw","uploader":"Google for Developers","uploader_id":"GoogleDevelopers","thumbnail":"https://i.ytimg.com/vi/M7lc1UVf-VE/maxresdefault.jpg","categories":["Science & Technology","Music"],"tags":["API","Developers"],"language":"en","availability":"public","is_live":false,"view_count":42,"like_count":7,"duration":238.25,"webpage_url":"https://www.youtube.com/watch?v=M7lc1UVf-VE"}"#,
        )
        .expect("fixture metadata is valid");
        let track = tracks.first().expect("one track is imported");

        assert_eq!(tracks.len(), 1);
        assert_eq!(track.item.id, "M7lc1UVf-VE");
        assert_eq!(track.item.title, "YouTube Developers Live");
        assert_eq!(track.item.artist, "Google for Developers");
        assert_eq!(track.item.album.as_deref(), Some("API Sessions"));
        assert_eq!(track.item.album_artist.as_deref(), Some("Google"));
        assert_eq!(track.item.track_number, Some(3));
        assert_eq!(track.item.disc_number, Some(1));
        assert_eq!(track.item.release_date.as_deref(), Some("2025-01-02"));
        assert_eq!(track.item.upload_date.as_deref(), Some("2025-01-03"));
        assert_eq!(
            track.item.description.as_deref(),
            Some("A complete metadata fixture.")
        );
        assert_eq!(track.item.channel.as_deref(), Some("Google Developers"));
        assert_eq!(
            track.item.channel_id.as_deref(),
            Some("UC_x5XG1OV2P6uZZ5FSM9Ttw")
        );
        assert_eq!(
            track.item.uploader.as_deref(),
            Some("Google for Developers")
        );
        assert_eq!(track.item.uploader_id.as_deref(), Some("GoogleDevelopers"));
        assert_eq!(
            track.item.thumbnail_url.as_deref(),
            Some("https://i.ytimg.com/vi/M7lc1UVf-VE/maxresdefault.jpg")
        );
        assert_eq!(track.item.categories, ["Science & Technology", "Music"]);
        assert_eq!(track.item.tags, ["API", "Developers"]);
        assert_eq!(track.item.language.as_deref(), Some("en"));
        assert_eq!(track.item.availability.as_deref(), Some("public"));
        assert!(!track.item.is_live);
        assert_eq!(track.item.view_count, Some(42));
        assert_eq!(track.item.like_count, Some(7));
        assert_eq!(track.item.duration_ms, 238_250);
        assert_eq!(
            track.source_url,
            "https://www.youtube.com/watch?v=M7lc1UVf-VE"
        );
    }

    #[test]
    fn keeps_flat_discovery_tracks_dirty_until_enrichment_replaces_them() {
        let mut entries = parse_import_metadata_with_dirty_state(
            r#"{"id":"M7lc1UVf-VE","title":"Fast discovery title","channel":"Channel"}"#,
            true,
        )
        .expect("flat discovery fixture is valid");
        let enriched = parse_import_metadata_with_dirty_state(
            r#"{"id":"M7lc1UVf-VE","track":"Full metadata title","artist":"Artist","album":"Album","duration":120}"#,
            false,
        )
        .expect("full metadata fixture is valid");

        assert!(entries[0].item.metadata_dirty);
        merge_queue_entries(&mut entries, enriched);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].item.title, "Full metadata title");
        assert!(!entries[0].item.metadata_dirty);
    }

    #[test]
    fn dirty_discoveries_survive_a_provider_restart_for_background_refresh() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gmusic-dirty-library-{}-{unique}.json",
            std::process::id()
        ));
        let entries = parse_import_metadata_with_dirty_state(
            r#"{"id":"M7lc1UVf-VE","title":"Fast discovery title","channel":"Channel"}"#,
            true,
        )
        .expect("flat discovery fixture is valid");

        let mut provider = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("an absent library should initialize empty");
        provider.entries = entries;
        provider
            .persist_library()
            .expect("the dirty discovery should persist");
        drop(provider);

        let mut restored = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("the persisted library should load");
        restored.snapshot().expect("restored state is readable");

        assert!(restored.library_snapshot().tracks[0].metadata_dirty);
        fs::remove_file(path).expect("temporary library should be removable");
    }

    #[test]
    fn migrates_a_legacy_json_library_to_sqlite_once() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "gmusic-library-migration-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("temporary library directory should exist");
        let legacy_path = directory.join("library.json");
        fs::write(
            &legacy_path,
            serde_json::to_vec(&StoredLibrary {
                version: LIBRARY_VERSION,
                entries: parse_import_metadata(
                    r#"{"id":"M7lc1UVf-VE","title":"Legacy track","channel":"Channel"}"#,
                )
                .expect("legacy fixture is valid"),
                playlists: Vec::new(),
            })
            .expect("legacy fixture serializes"),
        )
        .expect("legacy library should write");

        let provider = YouTubePlaybackProvider::from_library_directory(directory.clone())
            .expect("legacy library should migrate");

        assert_eq!(provider.library_snapshot().tracks[0].title, "Legacy track");
        assert!(directory.join("library.sqlite3").is_file());
        assert!(directory.join("library.json.migrated").is_file());
        assert!(!legacy_path.exists());
        drop(provider);

        let restored = YouTubePlaybackProvider::from_library_directory(directory.clone())
            .expect("existing SQLite library should load");
        assert_eq!(restored.library_snapshot().tracks.len(), 1);
        fs::remove_dir_all(directory).expect("temporary library directory should be removable");
    }

    #[test]
    fn expands_playlist_metadata_and_skips_unavailable_entries() {
        let tracks = parse_import_metadata(
            r#"{
                "id":"PL-example",
                "title":"Imported playlist",
                "entries":[
                    {"id":"M7lc1UVf-VE","title":"First video","channel":"First channel","duration":120.0,"webpage_url":"https://www.youtube.com/watch?v=M7lc1UVf-VE"},
                    null,
                    {"id":"BaW_jenozKc","title":"Second video","uploader":"Second uploader","duration":90.5}
                ]
            }"#,
        )
        .expect("playlist fixture is valid");

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].item.title, "First video");
        assert_eq!(tracks[0].item.artist, "First channel");
        assert_eq!(tracks[1].item.title, "Second video");
        assert_eq!(tracks[1].item.artist, "Second uploader");
        assert_eq!(tracks[1].item.duration_ms, 90_500);
        assert_eq!(
            tracks[1].source_url,
            "https://www.youtube.com/watch?v=BaW_jenozKc"
        );
    }

    #[test]
    fn expands_nested_artist_metadata_into_tracks() {
        let tracks = parse_import_metadata(
            r#"{
                "id":"UC-artist",
                "title":"Artist",
                "entries":[{
                    "id":"videos",
                    "title":"Videos",
                    "entries":[
                        {"id":"M7lc1UVf-VE","title":"First video","channel":"Artist","duration":120.0},
                        {"id":"BaW_jenozKc","title":"Second video","channel":"Artist","duration":90.5}
                    ]
                }]
            }"#,
        )
        .expect("artist fixture is valid");

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].item.title, "First video");
        assert_eq!(tracks[1].item.title, "Second video");
    }

    #[test]
    fn asks_yt_dlp_for_video_or_playlist_metadata_without_downloading() {
        let arguments = metadata_arguments("https://youtube.com/playlist?list=PL-example", None);

        assert!(arguments.contains(&"--dump-json".into()));
        assert!(!arguments.contains(&"--dump-single-json".into()));
        assert!(arguments.contains(&"--yes-playlist".into()));
        assert!(arguments.contains(&"--ignore-errors".into()));
        assert!(arguments.contains(&"--lazy-playlist".into()));
        assert!(arguments.contains(&"--retries".into()));
        assert!(arguments.contains(&"--extractor-retries".into()));
        assert!(arguments.contains(&"--skip-download".into()));
        assert!(!arguments.contains(&"--no-playlist".into()));
    }

    #[test]
    fn asks_yt_dlp_to_discover_collection_entries_without_per_video_resolution() {
        let arguments = discovery_metadata_arguments("https://youtube.com/@channel/videos", None);

        assert!(arguments.contains(&"--dump-json".into()));
        assert!(arguments.contains(&"--flat-playlist".into()));
        assert!(arguments.contains(&"--yes-playlist".into()));
        assert!(arguments.contains(&"--skip-download".into()));
        assert!(!arguments.contains(&"--extractor-retries".into()));
    }

    #[test]
    fn parses_playable_tracks_from_streamed_yt_dlp_metadata() {
        let tracks = parse_streamed_metadata(
            concat!(
                "{\"id\":\"M7lc1UVf-VE\",\"title\":\"First\",\"channel\":\"Channel\",\"duration\":120}\n",
                "{\"id\":\"BaW_jenozKc\",\"title\":\"Second\",\"channel\":\"Channel\",\"duration\":90}"
            ),
        )
        .expect("streamed metadata is valid");

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].item.title, "First");
        assert_eq!(tracks[1].item.title, "Second");
    }

    #[test]
    fn reports_member_only_entries_without_dumping_provider_errors() {
        let stderr = concat!(
            "ERROR: [youtube] first: This video is available to this channel's members on level: Supporters\n",
            "ERROR: [youtube] second: This video is available to this channel's members on level: Supporters\n"
        );

        assert_eq!(member_only_error_count(stderr), 2);
        assert_eq!(
            metadata_error_message(stderr, 0),
            "no playable tracks found; skipped 2 members-only tracks"
        );
    }

    #[test]
    fn identifies_member_only_video_ids_for_removal_after_enrichment() {
        let stderr = concat!(
            "ERROR: [youtube] M7lc1UVf-VE: This video is available to this channel's members on level: Supporters\n",
            "ERROR: [youtube] BaW_jenozKc: This video is available to this channel's members on level: Supporters\n"
        );

        assert_eq!(
            member_only_video_ids(stderr),
            ["M7lc1UVf-VE".to_string(), "BaW_jenozKc".to_string()]
                .into_iter()
                .collect()
        );
    }

    #[test]
    fn keeps_streamed_tracks_when_metadata_stops_after_inactivity() {
        let entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Imported before timeout","channel":"Channel","duration":120}"#,
        )
        .expect("fixture metadata is valid");

        let resolution = finish_metadata_resolution(entries, "", true)
            .expect("playable metadata should survive an inactivity timeout");

        assert_eq!(resolution.entries.len(), 1);
        assert!(resolution.timed_out);
    }

    #[test]
    fn repeated_imports_update_metadata_without_duplicate_queue_entries() {
        let mut entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Old title","channel":"Channel","duration":120}"#,
        )
        .expect("initial fixture is valid");
        let imported = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"Updated title","channel":"Channel","duration":121},{"id":"BaW_jenozKc","title":"New track","channel":"Channel","duration":90}]}"#,
        )
        .expect("updated fixture is valid");

        merge_queue_entries(&mut entries, imported);

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].item.id, "M7lc1UVf-VE");
        assert_eq!(entries[0].item.title, "Updated title");
        assert_eq!(entries[1].item.id, "BaW_jenozKc");
    }

    #[test]
    fn keeps_a_loaded_library_out_of_the_new_play_queue() {
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);

        assert_eq!(provider.library_snapshot().tracks.len(), 2);
        assert!(provider.snapshot.queue.is_empty());

        provider
            .replace_queue(&["BaW_jenozKc".into(), "M7lc1UVf-VE".into()])
            .expect("known library tracks create a play queue");
        provider
            .queue_track_next("M7lc1UVf-VE")
            .expect("a library track can be placed next");
        provider
            .add_to_queue("BaW_jenozKc")
            .expect("a library track can be added to the queue");

        assert_eq!(
            provider
                .snapshot
                .queue
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["M7lc1UVf-VE", "BaW_jenozKc"]
        );
        assert_eq!(provider.library_snapshot().tracks.len(), 2);
    }

    #[test]
    fn removes_the_final_track_from_a_non_repeating_queue_when_it_ends() {
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Final","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);
        provider
            .replace_queue(&["M7lc1UVf-VE".into(), "BaW_jenozKc".into()])
            .expect("known library tracks create a play queue");
        provider.snapshot.current_item = Some(provider.snapshot.queue[1].clone());
        provider.snapshot.status = PlaybackStatus::Playing;
        provider.snapshot.position_ms = 90_000;

        provider
            .advance_from(1)
            .expect("the final queue track can finish");

        assert!(provider.snapshot.queue.is_empty());
        assert!(provider.snapshot.current_item.is_none());
        assert_eq!(provider.snapshot.position_ms, 0);
        assert_eq!(provider.snapshot.status, PlaybackStatus::Paused);
    }

    #[test]
    fn queue_completion_requires_a_playback_snapshot_broadcast() {
        let entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Finished","channel":"Artist","duration":90}"#,
        )
        .expect("fixture metadata is valid");
        let item = entries[0].item.clone();
        let before = PlaybackSnapshot {
            current_item: Some(item.clone()),
            playback_order: vec![item.id.clone()],
            position_ms: item.duration_ms,
            queue: vec![item],
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            status: PlaybackStatus::Playing,
            volume_percent: 72,
        };
        let after = PlaybackSnapshot {
            current_item: None,
            playback_order: vec![],
            position_ms: 0,
            queue: vec![],
            repeat_mode: RepeatMode::Off,
            shuffle_enabled: false,
            status: PlaybackStatus::Paused,
            volume_percent: 72,
        };

        assert!(YouTubePlaybackProvider::snapshot_requires_broadcast(
            &before, &after
        ));

        let position_only = PlaybackSnapshot {
            position_ms: 1_000,
            ..before.clone()
        };
        assert!(!YouTubePlaybackProvider::snapshot_requires_broadcast(
            &before,
            &position_only
        ));
    }

    #[test]
    fn toggling_shuffle_preserves_the_authoritative_queue_order() {
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90},{"id":"aqz-KE-bpKQ","title":"Third","channel":"Artist","duration":100}]}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);
        let queue_ids = vec![
            "M7lc1UVf-VE".to_owned(),
            "BaW_jenozKc".to_owned(),
            "aqz-KE-bpKQ".to_owned(),
        ];
        provider
            .replace_queue(&queue_ids)
            .expect("known library tracks create a play queue");

        provider.toggle_shuffle().expect("shuffle can be enabled");
        assert!(provider.snapshot.shuffle_enabled);
        assert_eq!(
            provider
                .snapshot
                .queue
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            queue_ids
        );
        assert_eq!(
            provider
                .snapshot()
                .expect("the shuffled playback snapshot is available")
                .playback_order,
            provider.shuffle_order
        );

        provider.toggle_shuffle().expect("shuffle can be disabled");
        assert!(!provider.snapshot.shuffle_enabled);
        assert_eq!(
            provider
                .snapshot
                .queue
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            queue_ids
        );
    }

    #[test]
    fn removes_an_upcoming_item_without_changing_the_library() {
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);
        provider
            .replace_queue(&["M7lc1UVf-VE".into(), "BaW_jenozKc".into()])
            .expect("known library tracks create a play queue");

        provider
            .remove_queue_item(1)
            .expect("an upcoming queue item can be removed");

        assert_eq!(provider.snapshot.queue.len(), 1);
        assert_eq!(provider.snapshot.queue[0].id, "M7lc1UVf-VE");
        assert_eq!(provider.library_snapshot().tracks.len(), 2);
    }

    #[test]
    fn maintains_favorites_and_most_played_default_playlists() {
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        let mut provider = YouTubePlaybackProvider::with_entries(entries, None);

        provider
            .toggle_favorite("BaW_jenozKc")
            .expect("a library track can be favorited");
        provider
            .record_playback_start("M7lc1UVf-VE", 10)
            .expect("a library track can be played");
        provider
            .record_playback_start("BaW_jenozKc", 20)
            .expect("a library track can be played");
        provider
            .record_playback_start("BaW_jenozKc", 30)
            .expect("a library track can be played again");

        let library = provider.library_snapshot();
        assert_eq!(
            library
                .playlists
                .iter()
                .map(|playlist| (playlist.id.as_str(), playlist.name.as_str()))
                .collect::<Vec<_>>(),
            [("favorites", "Favorites"), ("most-played", "Most Played")]
        );
        assert_eq!(library.playlists[0].track_ids, ["BaW_jenozKc"]);
        assert_eq!(
            library.playlists[1].track_ids,
            ["BaW_jenozKc", "M7lc1UVf-VE"]
        );
    }

    #[test]
    fn updates_user_owned_track_metadata_without_changing_its_source() {
        let mut provider = YouTubePlaybackProvider::new();
        provider.entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Original","channel":"Channel","duration":120}"#,
        )
        .expect("fixture metadata is valid");

        provider
            .update_track_metadata(
                "M7lc1UVf-VE",
                EditableTrackMetadata {
                    title: "Edited title".into(),
                    artist: "Edited artist".into(),
                    album: Some("Edited album".into()),
                    label: Some("Edited label".into()),
                    genres: vec!["Electronic".into(), "electronic".into(), "Ambient".into()],
                },
            )
            .expect("known tracks accept metadata updates");

        let track = &provider.entries[0];
        assert_eq!(track.item.title, "Edited title");
        assert_eq!(track.item.artist, "Edited artist");
        assert_eq!(track.item.album.as_deref(), Some("Edited album"));
        assert_eq!(track.item.label.as_deref(), Some("Edited label"));
        assert_eq!(track.item.genres, ["Electronic", "Ambient"]);
        assert_eq!(
            track.source_url,
            "https://www.youtube.com/watch?v=M7lc1UVf-VE"
        );
    }

    #[test]
    fn removes_tracks_from_the_library_queue_and_playlists() {
        let mut provider = YouTubePlaybackProvider::new();
        provider.entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        provider.playlists = vec![Playlist {
            id: "focus".into(),
            name: "Focus".into(),
            track_ids: vec!["M7lc1UVf-VE".into(), "BaW_jenozKc".into()],
        }];

        provider
            .remove_tracks(&["M7lc1UVf-VE".into()])
            .expect("known tracks can be removed");

        let library = provider.library_snapshot();
        assert_eq!(library.tracks.len(), 1);
        assert_eq!(library.tracks[0].id, "BaW_jenozKc");
        assert_eq!(
            library
                .playlists
                .iter()
                .find(|playlist| playlist.id == "focus")
                .expect("the custom playlist remains in the library")
                .track_ids,
            ["BaW_jenozKc"]
        );
        assert!(provider.snapshot.queue.is_empty());
    }

    #[test]
    fn updates_multiple_tracks_metadata_in_one_library_operation() {
        let mut provider = YouTubePlaybackProvider::new();
        provider.entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Artist","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Artist","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");

        provider
            .update_tracks_metadata(vec![
                (
                    "M7lc1UVf-VE".into(),
                    EditableTrackMetadata {
                        title: "Updated first".into(),
                        artist: "First artist".into(),
                        album: None,
                        label: Some("First label".into()),
                        genres: vec!["Ambient".into()],
                    },
                ),
                (
                    "BaW_jenozKc".into(),
                    EditableTrackMetadata {
                        title: "Updated second".into(),
                        artist: "Second artist".into(),
                        album: Some("Second album".into()),
                        label: Some("Second label".into()),
                        genres: vec!["Electronic".into()],
                    },
                ),
            ])
            .expect("known tracks accept batch metadata updates");

        let library = provider.library_snapshot();
        assert_eq!(library.tracks[0].title, "Updated first");
        assert_eq!(library.tracks[0].label.as_deref(), Some("First label"));
        assert_eq!(library.tracks[1].title, "Updated second");
        assert_eq!(library.tracks[1].album.as_deref(), Some("Second album"));
    }

    #[test]
    fn stores_playlists_against_stable_track_ids() {
        let mut provider = YouTubePlaybackProvider::new();
        provider.entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Channel","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Channel","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");

        provider
            .upsert_playlist(Playlist {
                id: "focus".into(),
                name: "Focus".into(),
                track_ids: vec!["BaW_jenozKc".into(), "M7lc1UVf-VE".into()],
            })
            .expect("a playlist with known tracks is valid");

        assert_eq!(provider.playlists.len(), 1);
        assert_eq!(provider.playlists[0].name, "Focus");
        assert_eq!(
            provider.playlists[0].track_ids,
            ["BaW_jenozKc", "M7lc1UVf-VE"]
        );
    }

    #[test]
    fn removing_playlist_membership_preserves_the_library_and_other_playlists() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gmusic-playlist-removal-{}-{unique}.sqlite",
            std::process::id()
        ));
        let mut provider = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("a temporary library should initialize");
        provider.entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First","channel":"Channel","duration":120},{"id":"BaW_jenozKc","title":"Second","channel":"Channel","duration":90}]}"#,
        )
        .expect("fixture metadata is valid");
        let ids = vec!["M7lc1UVf-VE".into(), "BaW_jenozKc".into()];
        provider
            .replace_queue(&ids)
            .expect("known tracks can be queued");
        let before = provider.snapshot.clone();
        for id in ["focus", "other"] {
            provider
                .upsert_playlist(Playlist {
                    id: id.into(),
                    name: id.into(),
                    track_ids: ids.clone(),
                })
                .expect("known tracks can be added to a playlist");
        }
        provider
            .toggle_favorite("M7lc1UVf-VE")
            .expect("known tracks can be favorited");
        provider
            .upsert_playlist(Playlist {
                id: "focus".into(),
                name: "focus".into(),
                track_ids: vec!["BaW_jenozKc".into()],
            })
            .expect("playlist membership can be removed");
        assert_eq!(provider.snapshot, before);
        drop(provider);

        let restored = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("the updated library should reload");
        let library = restored.library_snapshot();
        assert_eq!(library.tracks.len(), 2);
        assert_eq!(
            library
                .playlists
                .iter()
                .find(|p| p.id == "focus")
                .unwrap()
                .track_ids,
            ["BaW_jenozKc"]
        );
        assert_eq!(
            library
                .playlists
                .iter()
                .find(|p| p.id == "other")
                .unwrap()
                .track_ids,
            ids
        );
        assert_eq!(
            library
                .playlists
                .iter()
                .find(|p| p.id == "favorites")
                .unwrap()
                .track_ids,
            ["M7lc1UVf-VE"]
        );
        drop(restored);
        fs::remove_file(path).expect("temporary library should be removable");
    }

    #[test]
    fn reorders_user_playlists_and_persists_the_new_order() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gmusic-playlists-{}-{unique}.sqlite",
            std::process::id()
        ));
        let mut provider = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("an absent library should initialize empty");

        provider
            .upsert_playlist(Playlist {
                id: "focus".into(),
                name: "Focus".into(),
                track_ids: Vec::new(),
            })
            .expect("the first user playlist is valid");
        provider
            .upsert_playlist(Playlist {
                id: "road-trip".into(),
                name: "Road Trip".into(),
                track_ids: Vec::new(),
            })
            .expect("the second user playlist is valid");

        provider
            .reorder_playlists(&["road-trip".into(), "focus".into()])
            .expect("the user playlist order is valid");
        drop(provider);

        let restored = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("the persisted library should load");
        assert_eq!(
            restored
                .library_snapshot()
                .playlists
                .iter()
                .map(|playlist| playlist.id.as_str())
                .collect::<Vec<_>>(),
            ["favorites", "most-played", "road-trip", "focus"]
        );

        fs::remove_file(path).expect("temporary library should be removable");
    }

    #[test]
    fn records_each_started_play_without_losing_track_metadata() {
        let mut provider = YouTubePlaybackProvider::new();
        provider.entries = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Track","channel":"Channel","duration":120}"#,
        )
        .expect("fixture metadata is valid");
        provider.snapshot.current_item = Some(provider.entries[0].item.clone());

        provider
            .record_playback_start("M7lc1UVf-VE", 1_734_000_000_000)
            .expect("known tracks can record a play");
        provider
            .record_playback_start("M7lc1UVf-VE", 1_734_000_060_000)
            .expect("repeated track plays are counted");

        let library = provider.library_snapshot();
        assert_eq!(library.total_plays, 2);
        assert_eq!(library.tracks[0].play_count, 2);
        assert_eq!(library.tracks[0].last_played_at_ms, Some(1_734_000_060_000));
        assert_eq!(
            library.tracks[0].play_history_ms,
            [1_734_000_000_000, 1_734_000_060_000]
        );
        assert_eq!(
            provider
                .snapshot
                .current_item
                .as_ref()
                .map(|item| item.play_count),
            Some(2)
        );
    }

    #[test]
    fn selecting_an_unknown_library_track_returns_a_specific_error() {
        let mut provider = YouTubePlaybackProvider::new();

        let error = provider
            .play_track("missing-track")
            .expect_err("an unknown track must not start the player");

        assert_eq!(
            error.to_string(),
            "track missing-track is not in the current queue"
        );
    }

    #[test]
    fn restores_application_state_after_the_provider_closes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "gmusic-library-{}-{unique}.json",
            std::process::id()
        ));
        let entries = parse_import_metadata(
            r#"{"id":"PL-example","title":"Playlist","entries":[{"id":"M7lc1UVf-VE","title":"First track","channel":"First artist","duration":120},{"id":"BaW_jenozKc","title":"Second track","channel":"Second artist","duration":90}]}"#,
        )
        .expect("library fixture is valid");

        let mut provider = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("an absent library should initialize empty");
        provider.entries = entries;
        provider
            .persist_library()
            .expect("the imported library should persist");

        provider.snapshot.queue = vec![
            provider.entries[1].item.clone(),
            provider.entries[0].item.clone(),
        ];
        provider.snapshot.current_item = Some(provider.entries[1].item.clone());
        provider.snapshot.position_ms = 34_000;
        provider.snapshot.volume_percent = 41;
        provider.snapshot.shuffle_enabled = true;
        provider.snapshot.repeat_mode = RepeatMode::All;
        provider.shuffle_order = vec!["BaW_jenozKc".into(), "M7lc1UVf-VE".into()];
        provider.shutdown();

        let mut restored = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("the persisted library should load");
        let snapshot = restored
            .snapshot()
            .expect("restored application state is inspectable without a player process");

        assert_eq!(
            snapshot
                .queue
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["BaW_jenozKc", "M7lc1UVf-VE"]
        );
        assert_eq!(
            snapshot.current_item.as_ref().map(|item| item.id.as_str()),
            Some("BaW_jenozKc")
        );
        assert_eq!(snapshot.position_ms, 34_000);
        assert_eq!(snapshot.volume_percent, 41);
        assert!(snapshot.shuffle_enabled);
        assert_eq!(snapshot.repeat_mode, RepeatMode::All);
        assert_eq!(snapshot.status, PlaybackStatus::Paused);
        assert_eq!(restored.shuffle_order, ["BaW_jenozKc", "M7lc1UVf-VE"]);

        fs::remove_file(path).expect("temporary library should be removable");
    }

    #[test]
    fn passes_the_dedicated_cookie_file_to_youtube_tools() {
        let cookie_path = Path::new("/tmp/gmusic-youtube-cookies.txt");
        let metadata = metadata_arguments(
            "https://www.youtube.com/watch?v=M7lc1UVf-VE",
            Some(cookie_path),
        );
        let player = mpv_arguments(Path::new("/tmp/gmusic-mpv.sock"), 72, Some(cookie_path));

        assert!(
            metadata
                .windows(2)
                .any(|arguments| arguments == ["--cookies", "/tmp/gmusic-youtube-cookies.txt"])
        );
        assert!(
            player.contains(&"--ytdl-raw-options=cookies=/tmp/gmusic-youtube-cookies.txt".into())
        );
    }

    #[test]
    fn starts_mpv_as_an_audio_only_best_audio_player() {
        let arguments = mpv_arguments(Path::new("/tmp/gmusic-mpv.sock"), 72, None);

        assert!(arguments.contains(&"--video=no".into()));
        assert!(arguments.contains(&"--ytdl-format=bestaudio".into()));
        assert!(!arguments.contains(&"--no-video".into()));
        assert!(!arguments.contains(&"--ytdl-format=bestaudio/best".into()));
    }

    #[test]
    #[ignore = "requires network access and yt-dlp"]
    fn imports_a_live_youtube_playlist_with_track_metadata() {
        let tracks = resolve_metadata(
            "https://www.youtube.com/playlist?list=PL6kxQfTqpokMPgk1X7u1gqCV_I1JbwVph",
            None,
        )
        .expect("the public two-track playlist should resolve");

        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().all(|track| !track.item.id.is_empty()));
        assert!(tracks.iter().all(|track| !track.item.title.is_empty()));
        assert!(tracks.iter().all(|track| !track.item.artist.is_empty()));
        assert!(tracks.iter().all(|track| track.item.duration_ms > 0));
    }

    #[test]
    #[ignore = "requires network access and yt-dlp"]
    fn imports_the_reported_youtu_be_url() {
        let mut provider = YouTubePlaybackProvider::new();

        let imported = provider
            .import_youtube_urls(&["https://youtu.be/wEsuJoBKAvA".into()])
            .expect("the reported public video should import");

        assert_eq!(imported.queue.len(), 1);
        assert_eq!(imported.queue[0].id, "wEsuJoBKAvA");
        assert_eq!(
            imported.queue[0].title,
            "Goblin War [ゴブリン] - Dungeon Synth Mix"
        );
        assert_eq!(imported.queue[0].artist, "Cryo Crypt");
        assert_eq!(imported.queue[0].duration_ms, 2_738_000);
    }

    #[test]
    #[ignore = "requires network access, yt-dlp, mpv, and an audio device"]
    fn streams_youtube_audio_through_mpv() {
        let mut provider = YouTubePlaybackProvider::new();

        provider
            .import_youtube_urls(&["https://www.youtube.com/watch?v=M7lc1UVf-VE".into()])
            .expect("the documented YouTube test video should import");
        provider
            .play_track("M7lc1UVf-VE")
            .expect("the imported YouTube test video should load");
        let loaded = provider.snapshot().expect("mpv state should be readable");
        assert_eq!(
            loaded.current_item.as_ref().map(|item| item.id.as_str()),
            Some("M7lc1UVf-VE")
        );

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let playing = provider.snapshot().expect("mpv state should be readable");
            if playing.position_ms > 0 {
                break;
            }
            assert!(Instant::now() < deadline, "mpv did not start playback");
            thread::sleep(Duration::from_millis(250));
        }

        provider.pause().expect("mpv should accept pause");
        let paused = provider
            .snapshot()
            .expect("paused state should be readable");
        assert_eq!(paused.status, super::PlaybackStatus::Paused);
    }
}
