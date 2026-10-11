use super::*;

const FIRST: &str = "M7lc1UVf-VE";
const SECOND: &str = "BaW_jenozKc";
const THIRD: &str = "aqz-KE-bpKQ";

struct Fixture {
    directory: PathBuf,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "gmusic-import-dedup-{}-{unique}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self {
            path: directory.join("library.sqlite3"),
            directory,
        }
    }

    fn reload(&self) -> YouTubePlaybackProvider {
        YouTubePlaybackProvider::from_library_path(self.path.clone()).unwrap()
    }

    fn assert_saved_ids(&self, expected: &[&str]) {
        let (entries, _) = crate::persistence::load_library(&self.path).unwrap();
        let ids: Vec<_> = entries.iter().map(|entry| entry.item.id.as_str()).collect();
        assert_eq!(ids, expected);
        assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn metadata(id: &str, title: &str, source_url: &str) -> Value {
    json!({
        "id": id,
        "title": title,
        "channel": "Fixture artist",
        "duration": 120,
        "webpage_url": source_url,
    })
}

fn resolved(metadata: Value, discovery: bool) -> ResolvedYouTubeImport {
    ResolvedYouTubeImport {
        entries: parse_import_metadata_with_dirty_state(&metadata.to_string(), discovery).unwrap(),
        skipped_member_only: 0,
        skipped_member_only_ids: HashSet::new(),
        timed_out_sources: 0,
    }
}

fn ids(provider: &YouTubePlaybackProvider) -> Vec<&str> {
    provider
        .entries
        .iter()
        .map(|entry| entry.item.id.as_str())
        .collect()
}

#[test]
fn url_aliases_import_one_durable_track() {
    let fixture = Fixture::new();
    let mut provider = fixture.reload();
    for url in [
        "https://youtu.be/M7lc1UVf-VE",
        "https://www.youtube.com/watch?v=M7lc1UVf-VE",
        "https://www.youtube.com/watch?v=M7lc1UVf-VE&list=PL-fixture&index=3",
        "https://www.youtube.com/shorts/M7lc1UVf-VE",
    ] {
        provider
            .commit_youtube_import(resolved(metadata(FIRST, "Song", url), false))
            .unwrap();
        assert_eq!(ids(&provider), [FIRST]);
        fixture.assert_saved_ids(&[FIRST]);
    }
    let restored = fixture.reload();
    assert_eq!(ids(&restored), [FIRST]);
    assert_eq!(
        restored.entries[0].source_url,
        format!("https://www.youtube.com/watch?v={FIRST}")
    );
}

#[test]
fn nested_collection_duplicates_use_video_id_not_title() {
    let fixture = Fixture::new();
    let mut provider = fixture.reload();
    let shared = metadata(FIRST, "Same song title", "https://youtu.be/M7lc1UVf-VE");
    let other = metadata(SECOND, "Same song title", "https://youtu.be/BaW_jenozKc");
    let batch = resolved(
        json!({"entries": [
            shared.clone(),
            {"entries": [shared, other.clone(), null]},
            other,
        ]}),
        false,
    );
    assert_eq!(batch.track_count(), 2);
    provider.commit_youtube_import(batch).unwrap();
    assert_eq!(ids(&provider), [FIRST, SECOND]);
    fixture.assert_saved_ids(&[FIRST, SECOND]);
}

#[test]
fn overlapping_collection_imports_only_append_unseen_video_ids() {
    let fixture = Fixture::new();
    let mut provider = fixture.reload();
    for discovery in [true, false] {
        for videos in [
            vec![FIRST, SECOND],
            vec![SECOND, THIRD, FIRST],
            vec![THIRD, SECOND],
        ] {
            let entries: Vec<_> = videos
                .into_iter()
                .map(|id| metadata(id, "Song", &format!("https://youtu.be/{id}")))
                .collect();
            provider
                .commit_youtube_import(resolved(json!({"entries": entries}), discovery))
                .unwrap();
        }
    }
    assert_eq!(ids(&provider), [FIRST, SECOND, THIRD]);
    assert!(
        provider.snapshot.queue.is_empty(),
        "imports must not create or populate a play queue"
    );
    fixture.assert_saved_ids(&[FIRST, SECOND, THIRD]);
}

#[test]
fn reimports_after_restart_preserve_edits_history_playlists_and_queue() {
    let fixture = Fixture::new();
    let mut provider = fixture.reload();
    provider
        .commit_youtube_import(resolved(
            metadata(FIRST, "Original title", "https://youtu.be/M7lc1UVf-VE"),
            false,
        ))
        .unwrap();
    provider
        .update_track_metadata(
            FIRST,
            EditableTrackMetadata {
                title: "My title".into(),
                artist: "My artist".into(),
                album: Some("My album".into()),
                label: Some("My label".into()),
                genres: vec!["Jazz".into()],
            },
        )
        .unwrap();
    {
        let entry = &mut std::sync::Arc::make_mut(&mut provider.entries)[0];
        entry.item.play_count = 2;
        entry.item.last_played_at_ms = Some(2_000);
        entry.item.play_history_ms = vec![1_000, 2_000];
    }
    provider.toggle_favorite(FIRST).unwrap();
    provider
        .upsert_playlist(Playlist {
            id: "fixture-playlist".into(),
            name: "Fixture playlist".into(),
            track_ids: vec![FIRST.into()],
            smart: None,
        })
        .unwrap();
    provider.replace_queue(&[FIRST.into()]).unwrap();
    provider.snapshot.current_item = Some(provider.entries[0].item.clone());
    provider.snapshot.position_ms = 1_234;
    provider.persist_playback_state().unwrap();
    drop(provider);

    let mut restored = fixture.reload();
    let before = restored.snapshot.clone();
    for _ in 0..3 {
        for discovery in [true, false] {
            let mut refreshed = metadata(
                FIRST,
                "Changed provider title",
                "https://www.youtube.com/watch?v=M7lc1UVf-VE&list=PL-fixture",
            );
            refreshed["duration"] = json!(130);
            restored
                .commit_youtube_import(resolved(refreshed, discovery))
                .unwrap();
        }
    }
    let entry = &restored.entries[0];
    assert_eq!(ids(&restored), [FIRST]);
    assert_eq!(entry.item.title, "My title");
    assert_eq!(entry.item.artist, "My artist");
    assert_eq!(entry.item.album.as_deref(), Some("My album"));
    assert_eq!(entry.item.label.as_deref(), Some("My label"));
    assert_eq!(entry.item.genres, ["Jazz"]);
    assert_eq!(entry.item.play_count, 2);
    assert_eq!(entry.item.last_played_at_ms, Some(2_000));
    assert_eq!(entry.item.play_history_ms, [1_000, 2_000]);
    assert_eq!(
        entry.item.duration_ms, 130_000,
        "provider fields still refresh in place"
    );
    assert!(!entry.item.metadata_dirty);
    assert_eq!(
        restored
            .snapshot
            .queue
            .iter()
            .map(|item| &item.id)
            .collect::<Vec<_>>(),
        before.queue.iter().map(|item| &item.id).collect::<Vec<_>>()
    );
    assert_eq!(
        restored.snapshot.current_item.as_ref().map(|item| &item.id),
        before.current_item.as_ref().map(|item| &item.id)
    );
    assert_eq!(restored.snapshot.position_ms, before.position_ms);
    assert_eq!(restored.snapshot.status, before.status);
    let library = restored.library_snapshot();
    for id in ["favorites", "fixture-playlist"] {
        assert_eq!(
            library
                .playlists
                .iter()
                .find(|playlist| playlist.id == id)
                .unwrap()
                .track_ids,
            [FIRST]
        );
    }
    drop(restored);
    fixture.assert_saved_ids(&[FIRST]);
    let persisted = fixture.reload();
    assert_eq!(persisted.library_snapshot(), library);
}

#[test]
fn repeated_streamed_entries_resolve_and_commit_once() {
    let fixture = Fixture::new();
    let mut provider = fixture.reload();
    let lines: Vec<_> = [FIRST, SECOND, FIRST, SECOND]
        .into_iter()
        .map(|id| metadata(id, "Song", &format!("https://youtu.be/{id}")).to_string())
        .collect();
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "printf '%s\\n' \"$@\"", "metadata-fixture"])
        .args(&lines);
    let resolution = resolve_metadata_command(&mut command, false, |_| {}, None).unwrap();
    assert_eq!(resolution.entries.len(), 2);
    provider
        .commit_youtube_import(ResolvedYouTubeImport {
            entries: resolution.entries,
            skipped_member_only: resolution.skipped_member_only,
            skipped_member_only_ids: resolution.skipped_member_only_ids,
            timed_out_sources: usize::from(resolution.timed_out),
        })
        .unwrap();
    fixture.assert_saved_ids(&[FIRST, SECOND]);
}
