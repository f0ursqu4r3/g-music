mod fake;
mod youtube;

use serde::{Deserialize, Serialize};

pub use fake::{FakePlaybackProvider, PlaybackError};
pub(crate) use youtube::{
    DirtyTrack, QueueEntry, discover_youtube_imports, resolve_youtube_imports,
};
pub use youtube::{YouTubePlaybackError, YouTubePlaybackProvider};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub source_url: Option<String>,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub album_artist: Option<String>,
    #[serde(default)]
    pub track_number: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub upload_date: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub uploader: Option<String>,
    #[serde(default)]
    pub uploader_id: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub availability: Option<String>,
    #[serde(default)]
    pub is_live: bool,
    #[serde(default)]
    pub view_count: Option<u64>,
    #[serde(default)]
    pub like_count: Option<u64>,
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

fn default_provider() -> String {
    "youtube".into()
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

impl Default for RepeatMode {
    fn default() -> Self {
        Self::Off
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub status: PlaybackStatus,
    pub current_item: Option<MediaItem>,
    pub position_ms: u64,
    pub volume_percent: u8,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
    pub queue: Vec<MediaItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTransport {
    pub status: PlaybackStatus,
    pub current_item: Option<MediaItem>,
    pub position_ms: u64,
    pub volume_percent: u8,
    pub shuffle_enabled: bool,
    pub repeat_mode: RepeatMode,
}

impl From<&PlaybackSnapshot> for PlaybackTransport {
    fn from(snapshot: &PlaybackSnapshot) -> Self {
        Self {
            status: snapshot.status,
            current_item: snapshot.current_item.clone(),
            position_ms: snapshot.position_ms,
            volume_percent: snapshot.volume_percent,
            shuffle_enabled: snapshot.shuffle_enabled,
            repeat_mode: snapshot.repeat_mode,
        }
    }
}

pub(crate) fn shuffle_upcoming<T: Clone + PartialEq>(items: &mut [T], start: usize, mut seed: u64) {
    if items.len().saturating_sub(start) < 2 {
        return;
    }

    let original = items[start..].to_vec();
    for index in (start + 1..items.len()).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let swap_index = start + (seed as usize % (index - start + 1));
        items.swap(index, swap_index);
    }

    if items[start..] == original {
        items[start..].reverse();
    }
}
