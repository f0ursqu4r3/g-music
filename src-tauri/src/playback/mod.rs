mod fake;
mod youtube;

use serde::{Deserialize, Serialize};

pub use fake::{FakePlaybackProvider, PlaybackError};
pub use youtube::{YouTubePlaybackError, YouTubePlaybackProvider};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub title: String,
    pub artist: String,
    #[serde(default)]
    pub album: Option<String>,
    pub duration_ms: u64,
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
