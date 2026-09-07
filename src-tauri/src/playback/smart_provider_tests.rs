use super::*;
use crate::playback::{SmartPlaylistDefinition, SmartSortDirection};
use rusqlite::Connection;

struct Fixture {
    directory: PathBuf,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "gmusic-smart-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        Self {
            path: directory.join("library.sqlite3"),
            directory,
        }
    }

    fn provider(&self) -> YouTubePlaybackProvider {
        let entries = parse_import_metadata(
            r#"{"entries":[
            {"id":"M7lc1UVf-VE","title":"Zulu","channel":"Artist","duration":120},
            {"id":"BaW_jenozKc","title":"Alpha","channel":"Artist","duration":90},
            {"id":"aqz-KE-bpKQ","title":"Middle","channel":"Artist","duration":180}
        ]}"#,
        )
        .unwrap();
        crate::persistence::save_library(&self.path, &entries, &[]).unwrap();
        YouTubePlaybackProvider::from_library_path(self.path.clone()).unwrap()
    }

    fn reload(&self) -> YouTubePlaybackProvider {
        YouTubePlaybackProvider::from_library_path(self.path.clone()).unwrap()
    }

    fn connection(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn definition(field: &str, operator: &str, value: Value) -> SmartPlaylistDefinition {
    serde_json::from_value(json!({
        "match":"all", "rules":[{"field":field,"operator":operator,"value":value}],
        "sort":{"field":"title","direction":"asc"}, "limit":null
    }))
    .unwrap()
}

fn smart(definition: SmartPlaylistDefinition) -> Playlist {
    Playlist {
        id: "smart".into(),
        name: "Smart".into(),
        track_ids: vec![],
        smart: Some(definition),
    }
}

fn saved(snapshot: &LibrarySnapshot) -> &Playlist {
    snapshot
        .playlists
        .iter()
        .find(|playlist| playlist.id == "smart")
        .unwrap()
}

#[test]
fn smart_provider_saves_rules_ignores_membership_and_reloads() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    let before = provider.cached_snapshot();
    let mut playlist = smart(definition("title", "notContains", json!("  absent  ")));
    playlist.track_ids = vec!["nonexistent".into()];
    let snapshot = provider.upsert_playlist(playlist).unwrap();
    let playlist = saved(&snapshot);
    assert_eq!(
        playlist.track_ids,
        ["BaW_jenozKc", "aqz-KE-bpKQ", "M7lc1UVf-VE"]
    );
    assert_eq!(
        serde_json::to_value(&playlist.smart).unwrap()["rules"][0]["value"],
        "absent"
    );
    assert_eq!(fixture.reload().library_snapshot(), snapshot);
    assert_eq!(provider.cached_snapshot(), before);
    assert!(provider.playlists[0].track_ids.is_empty());
    let count: u64 = fixture
        .connection()
        .query_row(
            "SELECT count(*) FROM playlist_tracks WHERE playlist_id = 'smart'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn smart_provider_rejects_legacy_flattening_and_invalid_writes() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(smart(definition("playCount", "equals", json!(0))))
        .unwrap();
    let before = provider.library_snapshot();
    for payload in [
        json!({"id":"smart","name":"Legacy","trackIds":[]}),
        json!({"id":"smart","name":"Legacy","trackIds":[],"smart":null}),
    ] {
        let legacy: Playlist = serde_json::from_value(payload).unwrap();
        assert!(provider.upsert_playlist(legacy).is_err());
    }
    for def in [
        definition("title", "equals", json!(" ")),
        definition("playCount", "equals", json!(-1)),
        definition("favorite", "contains", json!(true)),
    ] {
        assert!(provider.upsert_playlist(smart(def)).is_err());
    }
    for id in ["favorites", "most-played"] {
        let mut playlist = smart(definition("playCount", "equals", json!(0)));
        playlist.id = id.into();
        assert!(provider.upsert_playlist(playlist).is_err());
    }
    assert_eq!(provider.library_snapshot(), before);
    assert_eq!(fixture.reload().library_snapshot(), before);
}

#[test]
fn smart_provider_recomputes_after_metadata_favorites_history_and_availability() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    // A user playlist named Favorites must not supply stable Favorites membership.
    provider
        .upsert_playlist(Playlist {
            id: "impostor".into(),
            name: "Favorites".into(),
            track_ids: vec!["M7lc1UVf-VE".into()],
            smart: None,
        })
        .unwrap();
    let mut def = definition("favorite", "equals", json!(true));
    def.rules
        .extend(definition("playCount", "greaterThan", json!(0)).rules);
    def.rules
        .extend(definition("title", "contains", json!("edit")).rules);
    provider.upsert_playlist(smart(def)).unwrap();
    assert!(saved(&provider.library_snapshot()).track_ids.is_empty());
    provider.toggle_favorite("M7lc1UVf-VE").unwrap();
    provider.record_playback_start("M7lc1UVf-VE", 42).unwrap();
    assert!(saved(&provider.library_snapshot()).track_ids.is_empty());
    provider
        .update_track_metadata(
            "M7lc1UVf-VE",
            EditableTrackMetadata {
                title: "Edited".into(),
                artist: "Artist".into(),
                album: None,
                label: None,
                genres: vec![],
            },
        )
        .unwrap();
    assert_eq!(
        saved(&provider.library_snapshot()).track_ids,
        ["M7lc1UVf-VE"]
    );
    let imported = ResolvedYouTubeImport {
        entries: vec![],
        skipped_member_only: 1,
        skipped_member_only_ids: HashSet::from(["M7lc1UVf-VE".into()]),
        timed_out_sources: 0,
    };
    provider.commit_youtube_import(imported).unwrap();
    assert!(saved(&provider.library_snapshot()).track_ids.is_empty());
    assert!(
        saved(&fixture.reload().library_snapshot())
            .track_ids
            .is_empty()
    );
    let entries = parse_import_metadata(
        r#"{"id":"M7lc1UVf-VE","title":"Provider title","channel":"Artist","duration":120}"#,
    )
    .unwrap();
    provider
        .commit_youtube_import(ResolvedYouTubeImport {
            entries,
            skipped_member_only: 0,
            skipped_member_only_ids: HashSet::new(),
            timed_out_sources: 0,
        })
        .unwrap();
    assert_eq!(
        saved(&provider.library_snapshot()).track_ids,
        ["M7lc1UVf-VE"]
    );
    provider.toggle_favorite("M7lc1UVf-VE").unwrap();
    assert!(saved(&provider.library_snapshot()).track_ids.is_empty());
    assert!(provider.cached_snapshot().queue.is_empty());
}

#[test]
fn smart_persistence_retains_definitions_in_backups_and_cascades_deletion() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    let playlist = smart(definition("durationMs", "lessThan", json!(130000)));
    crate::persistence::save_library(
        &fixture.path,
        &provider.entries,
        std::slice::from_ref(&playlist),
    )
    .unwrap();
    assert_eq!(
        crate::persistence::load_library(&fixture.path)
            .unwrap()
            .1
            .as_slice(),
        std::slice::from_ref(&playlist)
    );
    let backup = crate::persistence::export_library_backup(&fixture.path).unwrap();
    assert_eq!(
        crate::persistence::load_library(&backup).unwrap().1,
        [playlist]
    );
    let mut restored = fixture.reload();
    restored.delete_playlist("smart").unwrap();
    let count: u64 = fixture
        .connection()
        .query_row("SELECT count(*) FROM smart_playlists", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn smart_persistence_failure_rolls_back_definition_metadata_and_membership() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    let def = definition("playCount", "equals", json!(0));
    provider.upsert_playlist(smart(def.clone())).unwrap();
    let before = provider.library_snapshot();
    let playback = provider.cached_snapshot();
    fixture.connection().execute_batch("CREATE TRIGGER reject_smart_update BEFORE UPDATE ON smart_playlists BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;").unwrap();
    let mut changed = smart(def);
    changed.name = "Changed".into();
    changed.smart.as_mut().unwrap().sort.direction = SmartSortDirection::Desc;
    assert!(provider.upsert_playlist(changed).is_err());
    assert_eq!(provider.library_snapshot(), before);
    assert_eq!(fixture.reload().library_snapshot(), before);
    assert_eq!(provider.cached_snapshot(), playback);
}

#[test]
fn smart_persistence_rejects_invalid_saved_rules_and_loads_old_schema() {
    let fixture = Fixture::new();
    let provider = fixture.provider();
    // Simulate a database created before smart playlists existed.
    fixture
        .connection()
        .execute_batch("DROP TABLE IF EXISTS smart_playlists;")
        .unwrap();
    assert_eq!(
        fixture.reload().library_snapshot(),
        provider.library_snapshot()
    );
    let mut playlist = smart(definition("title", "equals", json!(" ")));
    assert!(
        crate::persistence::save_library(&fixture.path, &provider.entries, &[playlist.clone()])
            .is_err()
    );
    playlist.smart = Some(definition("playCount", "equals", json!(0)));
    crate::persistence::save_library(&fixture.path, &provider.entries, &[playlist]).unwrap();
    fixture.connection().execute("UPDATE smart_playlists SET definition_json = ?1", [r#"{"match":"all","rules":[],"sort":{"field":"title","direction":"asc"},"limit":null}"#]).unwrap();
    assert!(YouTubePlaybackProvider::from_library_path(fixture.path.clone()).is_err());
}

#[test]
fn smart_ordinary_playlist_json_shape_is_unchanged() {
    let value = json!({"id":"manual","name":"Manual","trackIds":[]});
    let playlist: Playlist = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(playlist).unwrap(), value);
    let nullable: Playlist =
        serde_json::from_value(json!({"id":"manual","name":"Manual","trackIds":[],"smart":null}))
            .unwrap();
    assert_eq!(serde_json::to_value(nullable).unwrap(), value);
}

#[test]
fn smart_preview_and_freeze_use_current_order_and_do_not_change_playback() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider.replace_queue(&["M7lc1UVf-VE".into()]).unwrap();
    let before = provider.library_snapshot();
    let queue = provider.cached_snapshot();
    let mut def = definition("durationMs", "lessThan", json!(200000));
    def.limit = Some(2);
    let preview = provider.preview_smart_playlist(def.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(preview).unwrap(),
        json!({
            "totalMatches":3,"matches":[
                {"trackId":"BaW_jenozKc","matchedRuleIndexes":[0]},
                {"trackId":"aqz-KE-bpKQ","matchedRuleIndexes":[0]}
            ]
        })
    );
    assert_eq!(provider.library_snapshot(), before);
    assert_eq!(fixture.reload().library_snapshot(), before);
    assert_eq!(provider.cached_snapshot(), queue);
    assert!(
        provider
            .preview_smart_playlist(definition("title", "equals", json!(" ")))
            .is_err()
    );
    provider.upsert_playlist(smart(def)).unwrap();
    // Freeze must resolve again after a semantic change.
    provider
        .update_track_metadata(
            "M7lc1UVf-VE",
            EditableTrackMetadata {
                title: "A first".into(),
                artist: "Artist".into(),
                album: None,
                label: None,
                genres: vec![],
            },
        )
        .unwrap();
    let queue = provider.cached_snapshot();
    let frozen = provider.freeze_smart_playlist("smart").unwrap();
    assert_eq!(saved(&frozen).track_ids, ["M7lc1UVf-VE", "BaW_jenozKc"]);
    assert_eq!(saved(&frozen).name, "Smart");
    assert!(saved(&frozen).smart.is_none());
    assert_eq!(provider.cached_snapshot(), queue);
    assert_eq!(fixture.reload().library_snapshot(), frozen);
    for id in ["smart", "favorites", "most-played", "missing"] {
        assert!(provider.freeze_smart_playlist(id).is_err());
    }
    provider.record_playback_start("aqz-KE-bpKQ", 1).unwrap();
    assert_eq!(saved(&provider.library_snapshot()), saved(&frozen));
}

#[test]
fn smart_freeze_and_delete_failure_restore_memory_and_database() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(smart(definition("playCount", "equals", json!(0))))
        .unwrap();
    let before = provider.library_snapshot();
    let playback = provider.cached_snapshot();
    fixture.connection().execute_batch("CREATE TRIGGER reject_smart_delete BEFORE DELETE ON smart_playlists BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;").unwrap();
    assert!(provider.freeze_smart_playlist("smart").is_err());
    assert_eq!(provider.library_snapshot(), before);
    assert_eq!(fixture.reload().library_snapshot(), before);
    assert!(provider.delete_playlist("smart").is_err());
    assert_eq!(provider.library_snapshot(), before);
    assert_eq!(fixture.reload().library_snapshot(), before);
    assert_eq!(provider.cached_snapshot(), playback);
}

#[test]
fn smart_snapshots_reevaluate_time_without_library_mutations() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    let now = 10 * 86_400_000;
    provider
        .record_playback_start("M7lc1UVf-VE", now - 86_400_000)
        .unwrap();
    provider
        .upsert_playlist(smart(definition("lastPlayedDays", "within", json!(1))))
        .unwrap();
    assert_eq!(
        saved(&provider.library_snapshot_at(now)).track_ids,
        ["M7lc1UVf-VE"]
    );
    assert!(
        saved(&provider.library_snapshot_at(now + 1))
            .track_ids
            .is_empty()
    );
    assert_eq!(
        saved(&provider.library_snapshot_at(now)).track_ids,
        ["M7lc1UVf-VE"]
    );
}

#[test]
fn smart_prepared_transport_and_background_import_preserve_current_definitions() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(smart(definition("playCount", "equals", json!(0))))
        .unwrap();
    let mut prepared = provider.prepare_transport();
    prepared.record_playback_start("M7lc1UVf-VE", 100).unwrap();
    provider
        .upsert_playlist(smart(definition("playCount", "greaterThan", json!(0))))
        .unwrap();
    let imported = ResolvedYouTubeImport {
        entries: parse_import_metadata(
            r#"{"id":"M7lc1UVf-VE","title":"Imported update","channel":"Artist","duration":120}"#,
        )
        .unwrap(),
        skipped_member_only: 0,
        skipped_member_only_ids: HashSet::new(),
        timed_out_sources: 0,
    };
    provider.commit_youtube_import(imported).unwrap();
    provider.commit_transport(prepared).unwrap();
    let snapshot = provider.library_snapshot();
    assert_eq!(saved(&snapshot).track_ids, ["M7lc1UVf-VE"]);
    assert_eq!(snapshot.tracks[0].title, "Imported update");
    assert_eq!(snapshot.tracks[0].play_count, 1);
    assert_eq!(fixture.reload().library_snapshot(), snapshot);
    // An in-flight transport must not restore a definition removed by freezing.
    let prepared = provider.prepare_transport();
    let frozen = provider.freeze_smart_playlist("smart").unwrap();
    provider.commit_transport(prepared).unwrap();
    assert_eq!(provider.library_snapshot(), frozen);
    assert_eq!(fixture.reload().library_snapshot(), frozen);
}

#[test]
fn smart_library_and_playback_save_rolls_back_together() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(smart(definition("playCount", "equals", json!(0))))
        .unwrap();
    let before = provider.library_snapshot();
    provider.persist_playback_state().unwrap();
    let playback = crate::persistence::load_playback_state(&fixture.path)
        .unwrap()
        .unwrap();
    let mut changed_entries = provider.entries.as_ref().clone();
    changed_entries[0].item.title = "Changed".into();
    let mut changed_playlists = provider.playlists.clone();
    changed_playlists[0].smart.as_mut().unwrap().limit = Some(1);
    let mut changed_playback = playback.clone();
    changed_playback.volume_percent = 5;
    fixture.connection().execute_batch("CREATE TRIGGER reject_playback BEFORE UPDATE ON playback_state BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;").unwrap();
    assert!(
        crate::persistence::save_library_and_playback(
            &fixture.path,
            &changed_entries,
            &changed_playlists,
            &changed_playback
        )
        .is_err()
    );
    assert_eq!(fixture.reload().library_snapshot(), before);
    assert_eq!(
        serde_json::to_value(
            crate::persistence::load_playback_state(&fixture.path)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(playback).unwrap()
    );
}

#[test]
fn smart_reordering_removal_and_manual_conversion_preserve_rules_and_order() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(Playlist {
            id: "smart".into(),
            name: "Manual".into(),
            track_ids: vec!["M7lc1UVf-VE".into()],
            smart: None,
        })
        .unwrap();
    let mut def = definition("playCount", "equals", json!(0));
    def.sort.field = crate::playback::SmartSortField::LibraryOrder;
    let snapshot = provider.upsert_playlist(smart(def)).unwrap();
    assert_eq!(
        saved(&snapshot).track_ids,
        ["M7lc1UVf-VE", "BaW_jenozKc", "aqz-KE-bpKQ"]
    );
    let snapshot = provider.move_library_item(0, 2).unwrap();
    assert_eq!(
        saved(&snapshot).track_ids,
        ["BaW_jenozKc", "aqz-KE-bpKQ", "M7lc1UVf-VE"]
    );
    provider
        .upsert_playlist(Playlist {
            id: "manual".into(),
            name: "Manual".into(),
            track_ids: vec![],
            smart: None,
        })
        .unwrap();
    provider
        .reorder_playlists(&["manual".into(), "smart".into()])
        .unwrap();
    let snapshot = provider.remove_tracks(&["aqz-KE-bpKQ".into()]).unwrap();
    assert!(saved(&snapshot).smart.is_some());
    assert_eq!(saved(&snapshot).track_ids, ["BaW_jenozKc", "M7lc1UVf-VE"]);
    assert_eq!(snapshot.playlists[2].id, "manual");
    assert_eq!(fixture.reload().library_snapshot(), snapshot);
    assert!(provider.cached_snapshot().queue.is_empty());
}

#[test]
fn smart_unchanged_rules_are_not_rewritten_by_library_mutations() {
    let fixture = Fixture::new();
    let mut provider = fixture.provider();
    provider
        .upsert_playlist(smart(definition("playCount", "equals", json!(0))))
        .unwrap();
    fixture.connection().execute_batch("CREATE TRIGGER reject_smart_rewrite BEFORE UPDATE ON smart_playlists BEGIN SELECT RAISE(FAIL, 'unchanged rule rewrite'); END;").unwrap();
    provider.toggle_favorite("M7lc1UVf-VE").unwrap();
    provider.record_playback_start("M7lc1UVf-VE", 42).unwrap();
    assert_eq!(
        saved(&provider.library_snapshot()).track_ids,
        ["BaW_jenozKc", "aqz-KE-bpKQ"]
    );
    assert_eq!(
        fixture.reload().library_snapshot(),
        provider.library_snapshot()
    );
}
