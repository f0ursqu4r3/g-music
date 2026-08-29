mod fake;
mod youtube;

use serde::{Deserialize, Serialize};

pub use fake::{FakePlaybackProvider, PlaybackError};
pub(crate) use youtube::{DirtyTrack, discover_youtube_imports, resolve_youtube_imports};
pub use youtube::{YouTubePlaybackError, YouTubePlaybackProvider};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    pub duration_ms: u64,
    #[serde(default)]
    pub metadata_dirty: bool,
    #[serde(default)]
    pub play_count: u64,
    #[serde(default)]
    pub last_played_at_ms: Option<u64>,
    #[serde(default)]
    pub play_history_ms: Vec<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableTrackMetadata {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub label: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub track_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySnapshot {
    pub playlists: Vec<Playlist>,
    pub total_plays: u64,
    pub tracks: Vec<MediaItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackStatus {
    Paused,
    Playing,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    pub current_item: Option<MediaItem>,
    pub position_ms: u64,
    pub volume_percent: u8,
    pub queue: Vec<MediaItem>,
}
