use super::*;
use serde_json::{Value, json};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri::{Listener, Manager};

#[test]
fn smart_ipc_contract_validates_payloads_and_emits_only_after_successful_writes() {
    let directory = std::env::temp_dir().join(format!("gmusic-smart-ipc-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let path = directory.join("library.sqlite3");
    let entry = serde_json::from_value::<crate::playback::QueueEntry>(json!({
        "source_url":"https://www.youtube.com/watch?v=M7lc1UVf-VE",
        "item":{"id":"M7lc1UVf-VE","title":"Fixture","artist":"Artist","durationMs":90000}
    }))
    .unwrap();
    crate::persistence::save_library(&path, &[entry], &[]).unwrap();
    let app = mock_builder()
        .manage(AppState::from_library_directory(directory).unwrap())
        .invoke_handler(tauri::generate_handler![
            super::preview_smart_playlist,
            super::freeze_smart_playlist,
            super::upsert_playlist
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let writes = Arc::new(AtomicU64::new(0));
    let count = writes.clone();
    let event_path = path.clone();
    app.listen("library-updated", move |_| {
        // The saved row must already be readable when listeners receive the event.
        assert_eq!(
            crate::persistence::load_library(&event_path)
                .unwrap()
                .1
                .len(),
            1
        );
        count.fetch_add(1, Ordering::Relaxed);
    });
    let invoke = |cmd: &str, body: Value| -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            &window,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|response| response.deserialize::<Value>().unwrap())
    };
    let definition = json!({"match":"all","rules":[{"field":"playCount","operator":"equals","value":0}],"sort":{"field":"title","direction":"asc"},"limit":1});
    let before = app.state::<AppState>().library_snapshot().unwrap();
    let queue = app
        .state::<AppState>()
        .playback
        .lock()
        .unwrap()
        .cached_snapshot();
    assert_eq!(
        invoke("preview_smart_playlist", json!({"definition":definition})).unwrap(),
        json!({"totalMatches":1,"matches":[{"trackId":"M7lc1UVf-VE","matchedRuleIndexes":[0]}]})
    );
    assert_eq!(app.state::<AppState>().library_snapshot().unwrap(), before);
    assert!(
        crate::persistence::load_library(&path)
            .unwrap()
            .1
            .is_empty()
    );
    assert_eq!(writes.load(Ordering::Relaxed), 0);
    for patch in [
        json!({"field":"favorite","operator":"equals","value":1}),
        json!({"field":"unknown","operator":"equals","value":0}),
        json!({"field":"title","operator":"contains","value":" "}),
    ] {
        let mut invalid = definition.clone();
        invalid["rules"] = json!([patch]);
        assert!(invoke("preview_smart_playlist", json!({"definition": invalid})).is_err());
        assert!(
            invoke(
                "upsert_playlist",
                json!({"playlist":{"id":"smart","name":"Smart","trackIds":[],"smart":invalid}})
            )
            .is_err()
        );
    }
    assert_eq!(writes.load(Ordering::Relaxed), 0);
    let result = invoke(
        "upsert_playlist",
        json!({"playlist":{"id":"smart","name":"Smart","trackIds":["ignored"],"smart":definition}}),
    )
    .unwrap();
    assert_eq!(result["playlists"][2]["smart"], definition);
    assert_eq!(result["playlists"][2]["trackIds"], json!(["M7lc1UVf-VE"]));
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    assert!(
        invoke(
            "upsert_playlist",
            json!({"playlist":{"id":"smart","name":"Legacy","trackIds":[]}})
        )
        .is_err()
    );
    assert!(invoke("freeze_smart_playlist", json!({"id":"favorites"})).is_err());
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_freeze BEFORE DELETE ON smart_playlists BEGIN SELECT RAISE(FAIL, 'fixture failure'); END;").unwrap();
    assert!(invoke("freeze_smart_playlist", json!({"id":"smart"})).is_err());
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    assert!(
        app.state::<AppState>()
            .library_snapshot()
            .unwrap()
            .playlists[2]
            .smart
            .is_some()
    );
    connection
        .execute_batch("DROP TRIGGER reject_freeze;")
        .unwrap();
    let result = invoke("freeze_smart_playlist", json!({"id":"smart"})).unwrap();
    assert_eq!(
        result["playlists"][2],
        json!({"id":"smart","name":"Smart","trackIds":["M7lc1UVf-VE"]})
    );
    assert_eq!(writes.load(Ordering::Relaxed), 2);
    assert_eq!(
        app.state::<AppState>()
            .playback
            .lock()
            .unwrap()
            .cached_snapshot(),
        queue
    );
}
