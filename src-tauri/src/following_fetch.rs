//! Bounded yt-dlp discovery of a channel's most recent uploads. Nothing here touches the library.
use std::{
    collections::HashSet,
    process::Command,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use serde::Deserialize;

use crate::following::{
    FETCH_LIMIT, FetchedUpload, UploadFetcher, channel_uploads_url, validate_channel_id,
    validate_video_id,
};

const FETCH_TIMEOUT: Duration = Duration::from_secs(45);
const OUTPUT_LIMIT: usize = 512 * 1024;
const TITLE_LIMIT: usize = 200;

pub(crate) struct YtDlpUploadFetcher {
    pub(crate) stopping: Arc<AtomicBool>,
}

#[derive(Deserialize)]
struct FlatEntry {
    id: Option<String>,
    title: Option<String>,
    duration: Option<f64>,
    playlist_channel_id: Option<String>,
}

pub(crate) fn fetch_arguments(channel_id: &str) -> Result<Vec<String>, String> {
    let url = channel_uploads_url(channel_id).map_err(|_| "Invalid channel ID.".to_string())?;
    Ok([
        "--ignore-config",
        "--flat-playlist",
        "--dump-json",
        "--skip-download",
        "--no-warnings",
        "--playlist-end",
        &FETCH_LIMIT.to_string(),
        "--socket-timeout",
        "15",
        "--retries",
        "0",
        "--extractor-retries",
        "0",
        "--",
        &url,
    ]
    .map(String::from)
    .into())
}

pub(crate) fn parse_flat_uploads(
    output: &[u8],
    channel_id: &str,
) -> Result<Vec<FetchedUpload>, String> {
    let channel_id = validate_channel_id(channel_id).map_err(|_| "Invalid channel ID.")?;
    let text = String::from_utf8_lossy(output);
    let mut uploads = Vec::new();
    let mut seen = HashSet::new();
    let mut parsed_lines = 0;
    let mut lines = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        lines += 1;
        let Ok(entry) = serde_json::from_str::<FlatEntry>(line) else {
            continue;
        };
        parsed_lines += 1;
        let Some(id) = entry.id.filter(|id| validate_video_id(id)) else {
            continue;
        };
        if entry
            .playlist_channel_id
            .as_deref()
            .is_some_and(|owner| owner != channel_id)
            || uploads.len() >= FETCH_LIMIT
            || !seen.insert(id.clone())
        {
            continue;
        }
        let title = entry.title.unwrap_or_default();
        let title = title.trim();
        uploads.push(FetchedUpload {
            title: if title.is_empty() {
                "Untitled".into()
            } else {
                title.chars().take(TITLE_LIMIT).collect()
            },
            duration_ms: entry
                .duration
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
                .map(|seconds| (seconds * 1000.0).round() as u64),
            video_id: id,
        });
    }
    if lines > 0 && parsed_lines == 0 {
        return Err("YouTube returned an unreadable upload list.".into());
    }
    Ok(uploads)
}

impl UploadFetcher for YtDlpUploadFetcher {
    fn fetch(&self, channel_id: &str) -> Result<Vec<FetchedUpload>, String> {
        let arguments = fetch_arguments(channel_id)?;
        let executable = crate::playback::dependency_executable(
            "GMUSIC_YT_DLP_PATH",
            &["/opt/homebrew/bin/yt-dlp", "/usr/local/bin/yt-dlp"],
            "yt-dlp",
        );
        let output = crate::process::capture(
            Command::new(executable)
                .args(arguments)
                // yt-dlp finds its JavaScript runtime on PATH. Apps started from the Dock get a minimal one.
                .env("PATH", crate::playback::playback_path()),
            &self.stopping,
            FETCH_TIMEOUT,
            OUTPUT_LIMIT,
        )
        .map_err(String::from)?;
        parse_flat_uploads(&output, channel_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNEL: &str = "UCMYSjwig4-XrzqY6Ka28t4g";

    fn line(id: &str, channel: &str) -> String {
        format!(
            r#"{{"id":"{id}","title":"Song {id}","duration":215.5,"playlist_channel_id":"{channel}"}}"#
        )
    }

    #[test]
    fn requests_only_the_trusted_uploads_url_with_bounded_flags() {
        let arguments = fetch_arguments(CHANNEL).unwrap();
        assert_eq!(
            arguments.last().unwrap(),
            &format!("https://www.youtube.com/channel/{CHANNEL}/videos")
        );
        let joined = arguments.join(" ");
        assert!(arguments.contains(&"--flat-playlist".to_string()));
        assert!(joined.contains("--playlist-end 15"));
        assert!(joined.contains("--socket-timeout"));
        assert!(!arguments.contains(&"--cookies".to_string()));
        assert!(arguments.contains(&"--ignore-config".to_string()));
        assert!(fetch_arguments("@lemurian.archives").is_err());
    }

    #[test]
    fn parses_flat_listing_lines_in_order() {
        let output = format!(
            "{}\n{}\n",
            line("aaaaaaaaaaa", CHANNEL),
            line("bbbbbbbbbbb", CHANNEL)
        );
        let uploads = parse_flat_uploads(output.as_bytes(), CHANNEL).unwrap();
        assert_eq!(uploads.len(), 2);
        assert_eq!(uploads[0].video_id, "aaaaaaaaaaa");
        assert_eq!(uploads[0].title, "Song aaaaaaaaaaa");
        assert_eq!(uploads[0].duration_ms, Some(215_500));
    }

    #[test]
    fn skips_malformed_lines_invalid_ids_and_duplicates() {
        let output = format!(
            "not json\n{}\n{}\n{}\n{{\"title\":\"no id\"}}\n",
            line("short", CHANNEL),
            line("aaaaaaaaaaa", CHANNEL),
            line("aaaaaaaaaaa", CHANNEL),
        );
        let uploads = parse_flat_uploads(output.as_bytes(), CHANNEL).unwrap();
        assert_eq!(uploads.len(), 1);
    }

    #[test]
    fn rejects_entries_from_a_different_channel() {
        let output = format!(
            "{}\n{}\n",
            line("aaaaaaaaaaa", "UC_x5XG1OV2P6uZZ5FSM9Ttw"),
            line("bbbbbbbbbbb", CHANNEL)
        );
        let uploads = parse_flat_uploads(output.as_bytes(), CHANNEL).unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].video_id, "bbbbbbbbbbb");
    }

    #[test]
    fn keeps_entries_without_channel_metadata() {
        let output = r#"{"id":"aaaaaaaaaaa","title":"Song"}"#;
        assert_eq!(
            parse_flat_uploads(output.as_bytes(), CHANNEL)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn caps_results_and_trims_titles() {
        let output = (0..40)
            .map(|index| line(&format!("v{index:010}"), CHANNEL))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            parse_flat_uploads(output.as_bytes(), CHANNEL)
                .unwrap()
                .len(),
            15
        );
        let long = format!(r#"{{"id":"aaaaaaaaaaa","title":"{}"}}"#, "t".repeat(1000));
        let uploads = parse_flat_uploads(long.as_bytes(), CHANNEL).unwrap();
        assert!(uploads[0].title.chars().count() <= 200);
    }

    #[test]
    fn output_with_only_garbage_is_an_error() {
        assert!(parse_flat_uploads(b"<html>blocked</html>\n", CHANNEL).is_err());
    }

    #[test]
    fn empty_channel_listing_is_valid() {
        assert_eq!(parse_flat_uploads(b"", CHANNEL).unwrap(), Vec::new());
    }

    #[test]
    #[ignore = "requires network access to a public YouTube channel and yt-dlp"]
    fn live_uploads_persist_without_importing_tracks() {
        use crate::following::FollowingService;
        use std::time::{SystemTime, UNIX_EPOCH};

        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        let now_ms = || {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        };
        let directory = std::env::temp_dir().join(format!(
            "gmusic-following-live-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir(&directory).unwrap();
        let _cleanup = Cleanup(directory.clone());
        let path = directory.join("library.sqlite3");
        let stopping = Arc::new(AtomicBool::new(false));
        let service = FollowingService::new(path.clone(), stopping.clone());
        let fetcher = YtDlpUploadFetcher { stopping };
        let channel_id = "UC_x5XG1OV2P6uZZ5FSM9Ttw";
        service
            .follow(channel_id, "Google for Developers", now_ms())
            .unwrap();
        service.check(&fetcher, None, now_ms, &|| {}).unwrap();
        let snapshot = service.snapshot().unwrap();
        let artist = &snapshot.artists[0];
        assert_eq!(artist.last_error, None);
        assert!(artist.baseline_established);
        assert!(!artist.uploads.is_empty());
        assert!(artist.uploads.len() <= FETCH_LIMIT);
        assert!(artist.uploads.iter().all(|upload| {
            validate_video_id(&upload.video_id) && !upload.is_new && !upload.title.is_empty()
        }));
        let reopened = FollowingService::new(path.clone(), Arc::new(AtomicBool::new(false)));
        assert_eq!(reopened.snapshot().unwrap(), snapshot);
        assert!(
            crate::persistence::load_library(&path)
                .unwrap()
                .0
                .is_empty()
        );
        println!(
            "Live public-channel check stored {} uploads in an isolated database; no tracks imported.",
            artist.uploads.len()
        );
    }

    #[test]
    fn a_stopped_app_fetches_nothing() {
        let fetcher = YtDlpUploadFetcher {
            stopping: Arc::new(AtomicBool::new(true)),
        };
        assert_eq!(fetcher.fetch(CHANNEL), Err("Operation cancelled.".into()));
    }
}
