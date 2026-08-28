use super::{MediaItem, PlaybackSnapshot, PlaybackStatus};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("queue index {index} is outside the current queue")]
    QueueIndexOutOfBounds { index: usize },

    #[error("volume must be between 0 and 100")]
    InvalidVolume,
}

pub struct FakePlaybackProvider {
    snapshot: PlaybackSnapshot,
}

impl FakePlaybackProvider {
    pub fn new() -> Self {
        let mut queue: Vec<MediaItem> =
            serde_json::from_str(include_str!("../../../src/lib/mock-tracks.json"))
                .expect("the shared track fixture is valid JSON");
        queue.truncate(3);
        let current_item = queue.first().cloned();

        Self {
            snapshot: PlaybackSnapshot {
                status: PlaybackStatus::Paused,
                current_item,
                position_ms: 0,
                volume_percent: 72,
                queue,
            },
        }
    }

    pub fn snapshot(&self) -> PlaybackSnapshot {
        self.snapshot.clone()
    }

    pub fn play(&mut self) {
        if self.snapshot.current_item.is_some() {
            self.snapshot.status = PlaybackStatus::Playing;
        }
    }

    pub fn pause(&mut self) {
        self.snapshot.status = PlaybackStatus::Paused;
    }

    pub fn seek(&mut self, position_ms: u64) {
        let Some(current_item) = self.snapshot.current_item.as_ref() else {
            return;
        };

        self.snapshot.position_ms = position_ms.min(current_item.duration_ms);
    }

    pub fn next(&mut self) {
        let Some(current_item) = self.snapshot.current_item.as_ref() else {
            return;
        };
        let Some(current_index) = self
            .snapshot
            .queue
            .iter()
            .position(|item| item.id == current_item.id)
        else {
            return;
        };

        let next_index = (current_index + 1) % self.snapshot.queue.len();
        let Some(next_item) = self.snapshot.queue.get(next_index).cloned() else {
            return;
        };

        self.snapshot.current_item = Some(next_item);
        self.snapshot.position_ms = 0;
    }

    pub fn previous(&mut self) {
        let Some(current_item) = self.snapshot.current_item.as_ref() else {
            return;
        };
        let Some(current_index) = self
            .snapshot
            .queue
            .iter()
            .position(|item| item.id == current_item.id)
        else {
            return;
        };

        let previous_index = if current_index == 0 {
            self.snapshot.queue.len() - 1
        } else {
            current_index - 1
        };
        let Some(previous_item) = self.snapshot.queue.get(previous_index).cloned() else {
            return;
        };

        self.snapshot.current_item = Some(previous_item);
        self.snapshot.position_ms = 0;
    }

    pub fn move_queue_item(&mut self, from: usize, to: usize) -> Result<(), PlaybackError> {
        if from >= self.snapshot.queue.len() {
            return Err(PlaybackError::QueueIndexOutOfBounds { index: from });
        }
        if to >= self.snapshot.queue.len() {
            return Err(PlaybackError::QueueIndexOutOfBounds { index: to });
        }

        let item = self.snapshot.queue.remove(from);
        self.snapshot.queue.insert(to, item);

        Ok(())
    }

    pub fn set_volume(&mut self, volume_percent: u8) -> Result<(), PlaybackError> {
        if volume_percent > 100 {
            return Err(PlaybackError::InvalidVolume);
        }

        self.snapshot.volume_percent = volume_percent;

        Ok(())
    }
}

impl Default for FakePlaybackProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::FakePlaybackProvider;
    use crate::playback::PlaybackStatus;

    #[test]
    fn play_changes_a_paused_track_to_playing() {
        let mut provider = FakePlaybackProvider::new();

        provider.play();

        assert_eq!(provider.snapshot().status, PlaybackStatus::Playing);
    }

    #[test]
    fn pause_changes_a_playing_track_to_paused() {
        let mut provider = FakePlaybackProvider::new();
        provider.play();

        provider.pause();

        assert_eq!(provider.snapshot().status, PlaybackStatus::Paused);
    }

    #[test]
    fn seek_clamps_position_to_the_current_track_duration() {
        let mut provider = FakePlaybackProvider::new();

        provider.seek(999_999);

        assert_eq!(provider.snapshot().position_ms, 238_000);
    }

    #[test]
    fn next_selects_the_next_track_and_resets_progress() {
        let mut provider = FakePlaybackProvider::new();
        provider.seek(12_000);

        provider.next();

        let snapshot = provider.snapshot();
        assert_eq!(
            snapshot.current_item.as_ref().map(|item| item.id.as_str()),
            Some("the-current")
        );
        assert_eq!(snapshot.position_ms, 0);
    }

    #[test]
    fn previous_wraps_to_the_last_track() {
        let mut provider = FakePlaybackProvider::new();

        provider.previous();

        assert_eq!(
            provider
                .snapshot()
                .current_item
                .as_ref()
                .map(|item| item.id.as_str()),
            Some("soft-focus")
        );
    }

    #[test]
    fn move_queue_item_reorders_without_changing_the_selected_track() {
        let mut provider = FakePlaybackProvider::new();

        provider
            .move_queue_item(2, 0)
            .expect("fixture indices are valid");

        let snapshot = provider.snapshot();
        assert_eq!(snapshot.queue[0].id, "soft-focus");
        assert_eq!(
            snapshot.current_item.as_ref().map(|item| item.id.as_str()),
            Some("night-drive")
        );
    }

    #[test]
    fn set_volume_updates_the_snapshot() {
        let mut provider = FakePlaybackProvider::new();

        provider.set_volume(45).expect("45 is an allowed volume");

        assert_eq!(provider.snapshot().volume_percent, 45);
    }

    #[test]
    fn playback_metadata_matches_the_shared_library_catalog() {
        let catalog: serde_json::Value =
            serde_json::from_str(include_str!("../../../src/lib/mock-tracks.json"))
                .expect("the shared track fixture is valid JSON");
        let expected = &catalog[0];
        let snapshot = FakePlaybackProvider::new().snapshot();
        let current_item = snapshot
            .current_item
            .expect("the shared fixture provides a current track");

        assert_eq!(current_item.id, expected["id"].as_str().unwrap());
        assert_eq!(current_item.title, expected["title"].as_str().unwrap());
        assert_eq!(current_item.artist, expected["artist"].as_str().unwrap());
        assert_eq!(
            current_item.duration_ms,
            expected["durationMs"].as_u64().unwrap()
        );
    }
}
