use std::{
    collections::HashSet,
    env, fs,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        net::UnixStream,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use url::Url;

use super::{MediaItem, PlaybackSnapshot, PlaybackStatus};

const IPC_TIMEOUT: Duration = Duration::from_secs(3);
const LIBRARY_VERSION: u32 = 1;

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

    #[error("track {id} is not in the current queue")]
    TrackNotFound { id: String },

    #[error("volume must be between 0 and 100")]
    InvalidVolume,

    #[error("could not access the imported library: {0}")]
    Library(String),
}

#[derive(Clone, Deserialize, Serialize)]
struct QueueEntry {
    item: MediaItem,
    source_url: String,
}

pub struct YouTubePlaybackProvider {
    entries: Vec<QueueEntry>,
    library_path: Option<PathBuf>,
    player: MpvPlayer,
    session_cookie_path: Option<PathBuf>,
    snapshot: PlaybackSnapshot,
}

impl YouTubePlaybackProvider {
    pub fn new() -> Self {
        Self::with_entries(Vec::new(), None)
    }

    pub fn from_library_path(path: PathBuf) -> Result<Self, YouTubePlaybackError> {
        let entries = load_library(&path)?;
        tracing::info!(tracks = entries.len(), "imported library loaded");
        Ok(Self::with_entries(entries, Some(path)))
    }

    fn with_entries(entries: Vec<QueueEntry>, library_path: Option<PathBuf>) -> Self {
        let queue = entries.iter().map(|entry| entry.item.clone()).collect();
        Self {
            entries,
            library_path,
            player: MpvPlayer::new(),
            session_cookie_path: None,
            snapshot: PlaybackSnapshot {
                status: PlaybackStatus::Paused,
                current_item: None,
                position_ms: 0,
                volume_percent: 72,
                queue,
            },
        }
    }

    pub fn import_youtube_url(
        &mut self,
        source_url: &str,
    ) -> Result<PlaybackSnapshot, YouTubePlaybackError> {
        let parsed = validate_youtube_url(source_url)?;
        let source_host = parsed.host_str().unwrap_or("unknown").to_owned();
        let video_id = parsed
            .query_pairs()
            .find_map(|(key, value)| (key == "v").then(|| value.into_owned()))
            .or_else(|| {
                (source_host == "youtu.be")
                    .then(|| parsed.path().trim_matches('/').to_owned())
                    .filter(|value| !value.is_empty())
            });
        let playlist_id = parsed
            .query_pairs()
            .find_map(|(key, value)| (key == "list").then(|| value.into_owned()));
        tracing::info!(
            source_host,
            video_id,
            playlist_id,
            authenticated = self.session_cookie_path.is_some(),
            "YouTube import started"
        );

        let source_url = parsed.to_string();
        let started = Instant::now();
        let imported = resolve_metadata(&source_url, self.session_cookie_path.as_deref())?;
        tracing::info!(
            tracks = imported.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "YouTube metadata resolved"
        );
        let first_imported_id = imported
            .first()
            .map(|entry| entry.item.id.clone())
            .ok_or_else(|| YouTubePlaybackError::Metadata("no playable tracks found".into()))?;

        let previous_queue_size = self.entries.len();
        let imported_count = imported.len();
        merge_queue_entries(&mut self.entries, imported);
        let inserted_count = self.entries.len().saturating_sub(previous_queue_size);
        tracing::debug!(
            imported = imported_count,
            inserted = inserted_count,
            updated = imported_count.saturating_sub(inserted_count),
            queue_size = self.entries.len(),
            "queue merged"
        );
        self.sync_queue();
        self.persist_library()?;

        let imported = self
            .entries
            .iter()
            .find(|entry| entry.item.id == first_imported_id)
            .cloned()
            .ok_or_else(|| {
                YouTubePlaybackError::Metadata("imported track was not queued".into())
            })?;

        self.player.load(
            &imported.source_url,
            self.snapshot.volume_percent,
            self.session_cookie_path.as_deref(),
        )?;
        self.snapshot.current_item = Some(imported.item);
        self.snapshot.position_ms = 0;
        self.snapshot.status = PlaybackStatus::Playing;
        if let Some(item) = self.snapshot.current_item.as_ref() {
            tracing::info!(
                video_id = item.id,
                title = item.title,
                artist = item.artist,
                queue_size = self.snapshot.queue.len(),
                "YouTube import completed and playback started"
            );
        }

        Ok(self.snapshot.clone())
    }

    pub fn set_session_cookie_path(&mut self, path: Option<PathBuf>) {
        tracing::debug!(authenticated = path.is_some(), "playback session updated");
        self.session_cookie_path = path;
    }

    pub fn snapshot(&mut self) -> Result<PlaybackSnapshot, YouTubePlaybackError> {
        if self.snapshot.current_item.is_some() {
            let state = self.player.inspect()?;
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

        Ok(self.snapshot.clone())
    }

    pub fn play(&mut self) -> Result<(), YouTubePlaybackError> {
        if self.snapshot.current_item.is_some() {
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
        self.player.seek(position_ms)?;
        self.snapshot.position_ms = position_ms;
        tracing::debug!(position_ms, "playback seeked");
        Ok(())
    }

    pub fn next_track(&mut self) -> Result<(), YouTubePlaybackError> {
        let Some(current_index) = self.current_index() else {
            return Ok(());
        };
        let next_index = (current_index + 1) % self.entries.len();
        self.select_queue_index(next_index)
    }

    pub fn previous_track(&mut self) -> Result<(), YouTubePlaybackError> {
        let Some(current_index) = self.current_index() else {
            return Ok(());
        };
        let previous_index = if current_index == 0 {
            self.entries.len() - 1
        } else {
            current_index - 1
        };
        self.select_queue_index(previous_index)
    }

    pub fn play_track(&mut self, id: &str) -> Result<(), YouTubePlaybackError> {
        let index = self
            .entries
            .iter()
            .position(|entry| entry.item.id == id)
            .ok_or_else(|| YouTubePlaybackError::TrackNotFound { id: id.into() })?;

        self.select_queue_index(index)?;
        self.player.set_paused(false)?;
        self.snapshot.status = PlaybackStatus::Playing;
        tracing::info!(video_id = id, index, "library track playback started");
        Ok(())
    }

    pub fn move_queue_item(&mut self, from: usize, to: usize) -> Result<(), YouTubePlaybackError> {
        if from >= self.entries.len() {
            return Err(YouTubePlaybackError::QueueIndexOutOfBounds { index: from });
        }
        if to >= self.entries.len() {
            return Err(YouTubePlaybackError::QueueIndexOutOfBounds { index: to });
        }

        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        self.sync_queue();
        self.persist_library()?;
        tracing::info!(from, to, "queue item moved");
        Ok(())
    }

    pub fn set_volume(&mut self, volume_percent: u8) -> Result<(), YouTubePlaybackError> {
        if volume_percent > 100 {
            return Err(YouTubePlaybackError::InvalidVolume);
        }
        if self.snapshot.current_item.is_some() {
            self.player.set_volume(volume_percent)?;
        }
        self.snapshot.volume_percent = volume_percent;
        tracing::debug!(volume_percent, "playback volume changed");
        Ok(())
    }

    fn current_index(&self) -> Option<usize> {
        let current_id = &self.snapshot.current_item.as_ref()?.id;
        self.entries
            .iter()
            .position(|entry| &entry.item.id == current_id)
    }

    fn select_queue_index(&mut self, index: usize) -> Result<(), YouTubePlaybackError> {
        let was_playing = self.snapshot.status == PlaybackStatus::Playing;
        let entry = self
            .entries
            .get(index)
            .cloned()
            .ok_or(YouTubePlaybackError::QueueIndexOutOfBounds { index })?;

        self.player.load(
            &entry.source_url,
            self.snapshot.volume_percent,
            self.session_cookie_path.as_deref(),
        )?;
        if !was_playing {
            self.player.set_paused(true)?;
        }
        self.snapshot.current_item = Some(entry.item);
        self.snapshot.position_ms = 0;
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

    fn sync_queue(&mut self) {
        self.snapshot.queue = self
            .entries
            .iter()
            .map(|entry| entry.item.clone())
            .collect();
    }

    fn persist_library(&self) -> Result<(), YouTubePlaybackError> {
        let Some(path) = self.library_path.as_ref() else {
            return Ok(());
        };
        write_library(path, &self.entries)?;
        tracing::info!(tracks = self.entries.len(), "imported library persisted");
        Ok(())
    }
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
    channel: Option<String>,
    uploader: Option<String>,
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

fn is_youtube_video_id(value: &str) -> bool {
    value.len() == 11
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn metadata_entry(metadata: YtDlpMetadata) -> Option<QueueEntry> {
    let id = nonempty(metadata.id).filter(|id| is_youtube_video_id(id))?;
    let title = nonempty(metadata.track).or_else(|| nonempty(metadata.title))?;
    let artist = nonempty(metadata.artist)
        .or_else(|| nonempty(metadata.channel))
        .or_else(|| nonempty(metadata.uploader))
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
            title,
            artist,
            album: nonempty(metadata.album),
            duration_ms,
        },
        source_url,
    })
}

fn parse_import_metadata(output: &str) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    let metadata: YtDlpMetadata = serde_json::from_str(output)
        .map_err(|error| YouTubePlaybackError::Metadata(error.to_string()))?;
    let candidates = match metadata.entries {
        Some(entries) => entries.into_iter().flatten().collect::<Vec<_>>(),
        None => vec![metadata],
    };
    let mut seen = HashSet::new();
    let tracks = candidates
        .into_iter()
        .filter_map(metadata_entry)
        .filter(|entry| seen.insert(entry.item.id.clone()))
        .collect::<Vec<_>>();

    if tracks.is_empty() {
        return Err(YouTubePlaybackError::Metadata(
            "no playable tracks found in the YouTube URL".into(),
        ));
    }

    Ok(tracks)
}

fn merge_queue_entries(entries: &mut Vec<QueueEntry>, imported: Vec<QueueEntry>) {
    for entry in imported {
        if let Some(existing) = entries
            .iter_mut()
            .find(|existing| existing.item.id == entry.item.id)
        {
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
}

fn load_library(path: &Path) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(path)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    let library: StoredLibrary = serde_json::from_str(&contents)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    if library.version != LIBRARY_VERSION {
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

    Ok(library.entries)
}

fn write_library(path: &Path, entries: &[QueueEntry]) -> Result<(), YouTubePlaybackError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    }
    let temporary_path = path.with_extension("json.tmp");
    let contents = serde_json::to_vec_pretty(&StoredLibrary {
        version: LIBRARY_VERSION,
        entries: entries.to_vec(),
    })
    .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&temporary_path)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    file.write_all(&contents)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;
    fs::rename(&temporary_path, path)
        .map_err(|error| YouTubePlaybackError::Library(error.to_string()))?;

    Ok(())
}

fn metadata_arguments(source_url: &str, cookie_path: Option<&Path>) -> Vec<String> {
    let mut arguments = vec![
        "--dump-single-json".into(),
        "--yes-playlist".into(),
        "--ignore-errors".into(),
        "--skip-download".into(),
        "--no-warnings".into(),
    ];
    if let Some(cookie_path) = cookie_path {
        arguments.push("--cookies".into());
        arguments.push(cookie_path.to_string_lossy().into_owned());
    }
    arguments.push(source_url.into());
    arguments
}

fn resolve_metadata(
    source_url: &str,
    cookie_path: Option<&Path>,
) -> Result<Vec<QueueEntry>, YouTubePlaybackError> {
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
    let output = Command::new(&yt_dlp)
        .args(metadata_arguments(source_url, cookie_path))
        .output()
        .map_err(|error| YouTubePlaybackError::DependencyUnavailable {
            name: "yt-dlp",
            detail: error.to_string(),
        })?;

    if !output.status.success() {
        tracing::error!(status = %output.status, "yt-dlp metadata resolution failed");
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(YouTubePlaybackError::Metadata(if detail.is_empty() {
            format!("yt-dlp exited with {}", output.status)
        } else {
            detail
        }));
    }

    let output = String::from_utf8(output.stdout)
        .map_err(|error| YouTubePlaybackError::Metadata(error.to_string()))?;
    tracing::debug!(output_bytes = output.len(), "yt-dlp metadata received");
    parse_import_metadata(&output)
}

struct PlayerState {
    paused: bool,
    position_ms: u64,
}

struct MpvPlayer {
    child: Option<Child>,
    request_id: u64,
    session_cookie_path: Option<PathBuf>,
    socket_path: PathBuf,
}

impl MpvPlayer {
    fn new() -> Self {
        Self {
            child: None,
            request_id: 0,
            session_cookie_path: None,
            socket_path: env::temp_dir().join(format!("gmusic-mpv-{}.sock", std::process::id())),
        }
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
        Ok(())
    }

    fn set_paused(&mut self, paused: bool) -> Result<(), YouTubePlaybackError> {
        self.send(json!(["set_property", "pause", paused]))?;
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
        self.send(json!(["set_property", "volume", volume_percent]))?;
        Ok(())
    }

    fn inspect(&mut self) -> Result<PlayerState, YouTubePlaybackError> {
        let paused = self
            .send(json!(["get_property", "pause"]))?
            .as_bool()
            .unwrap_or(true);
        let position_seconds = self
            .send(json!(["get_property", "time-pos"]))?
            .as_f64()
            .unwrap_or(0.0);

        Ok(PlayerState {
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

    fn stop(&mut self) {
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
        fs, thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use std::path::Path;

    use super::{
        YouTubePlaybackProvider, merge_queue_entries, metadata_arguments, mpv_arguments,
        parse_import_metadata, resolve_metadata, validate_youtube_url,
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
    fn parses_rich_yt_dlp_metadata_into_an_imported_track() {
        let tracks = parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Video title","track":"YouTube Developers Live","artist":"Google for Developers","album":"API Sessions","channel":"Google Developers","duration":238.25,"webpage_url":"https://www.youtube.com/watch?v=M7lc1UVf-VE"}"#,
        )
        .expect("fixture metadata is valid");
        let track = tracks.first().expect("one track is imported");

        assert_eq!(tracks.len(), 1);
        assert_eq!(track.item.id, "M7lc1UVf-VE");
        assert_eq!(track.item.title, "YouTube Developers Live");
        assert_eq!(track.item.artist, "Google for Developers");
        assert_eq!(track.item.album.as_deref(), Some("API Sessions"));
        assert_eq!(track.item.duration_ms, 238_250);
        assert_eq!(
            track.source_url,
            "https://www.youtube.com/watch?v=M7lc1UVf-VE"
        );
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
    fn asks_yt_dlp_for_video_or_playlist_metadata_without_downloading() {
        let arguments = metadata_arguments("https://youtube.com/playlist?list=PL-example", None);

        assert!(arguments.contains(&"--yes-playlist".into()));
        assert!(arguments.contains(&"--ignore-errors".into()));
        assert!(arguments.contains(&"--skip-download".into()));
        assert!(!arguments.contains(&"--no-playlist".into()));
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
    fn imported_library_survives_a_provider_restart() {
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
        provider.sync_queue();
        provider
            .persist_library()
            .expect("the imported library should persist");
        drop(provider);

        let mut restored = YouTubePlaybackProvider::from_library_path(path.clone())
            .expect("the persisted library should load");
        let snapshot = restored.snapshot().expect("restored state is readable");

        assert_eq!(snapshot.queue.len(), 2);
        assert_eq!(snapshot.queue[0].title, "First track");
        assert_eq!(snapshot.queue[1].title, "Second track");
        assert_eq!(snapshot.current_item, None);

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
    #[ignore = "requires network access, yt-dlp, mpv, and an audio device"]
    fn imports_the_reported_youtu_be_url_through_mpv() {
        let mut provider = YouTubePlaybackProvider::new();

        let imported = provider
            .import_youtube_url("https://youtu.be/wEsuJoBKAvA")
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

        let loaded = provider
            .import_youtube_url("https://www.youtube.com/watch?v=M7lc1UVf-VE")
            .expect("the documented YouTube test video should load");
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
