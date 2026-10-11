//! IPC and scheduling glue for followed artists. Network work never holds the playback mutex.
use std::{
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::commands::{AppState, CommandError};
use crate::following::{
    FollowingError, FollowingService, FollowingSnapshot, UploadFetcher, run_poller,
};
use crate::following_fetch::YtDlpUploadFetcher;

pub(crate) const POLL_INTERVAL: Duration = Duration::from_secs(60 * 60);

fn unavailable() -> CommandError {
    CommandError {
        code: "following_unavailable",
        message: "Following is unavailable.".into(),
    }
}

fn service<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<FollowingService>, CommandError> {
    app.state::<AppState>().following().ok_or_else(unavailable)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

fn notify<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit("following-updated", ());
}

async fn blocking<R: Runtime, T: Send + 'static>(
    app: AppHandle<R>,
    operation: impl FnOnce(&AppHandle<R>, &FollowingService) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let service = service(&app)?;
        operation(&app, &service)
    })
    .await
    .map_err(|_| unavailable())?
}

pub(crate) async fn inspect<R: Runtime>(
    app: AppHandle<R>,
) -> Result<FollowingSnapshot, CommandError> {
    blocking(app, |_, service| Ok(service.snapshot()?)).await
}

pub(crate) async fn follow<R: Runtime>(
    app: AppHandle<R>,
    channel_id: String,
    name: String,
) -> Result<FollowingSnapshot, CommandError> {
    blocking(app, move |app, service| {
        let snapshot = service.follow(&channel_id, &name, now_ms())?;
        notify(app);
        Ok(snapshot)
    })
    .await
}

pub(crate) async fn unfollow<R: Runtime>(
    app: AppHandle<R>,
    channel_id: String,
) -> Result<FollowingSnapshot, CommandError> {
    blocking(app, move |app, service| {
        let snapshot = service.unfollow(&channel_id)?;
        notify(app);
        Ok(snapshot)
    })
    .await
}

pub(crate) async fn check<R: Runtime>(
    app: AppHandle<R>,
    channel_id: Option<String>,
) -> Result<FollowingSnapshot, CommandError> {
    blocking(app, move |app, service| {
        let fetcher = YtDlpUploadFetcher {
            stopping: service.stopping(),
        };
        check_with(app, service, &fetcher, channel_id.as_deref())
    })
    .await
}

pub(crate) fn check_with<R: Runtime>(
    app: &AppHandle<R>,
    service: &FollowingService,
    fetcher: &dyn UploadFetcher,
    channel_id: Option<&str>,
) -> Result<FollowingSnapshot, CommandError> {
    service.check(fetcher, channel_id, now_ms, &|| notify(app))?;
    Ok(service.snapshot()?)
}

/// Starts the only polling worker. It checks once on startup, then hourly.
pub(crate) fn start_poller(app: AppHandle) {
    let Ok(service) = service(&app) else {
        return;
    };
    let spawned = thread::Builder::new()
        .name("following-poller".into())
        .spawn(move || {
            let fetcher = YtDlpUploadFetcher {
                stopping: service.stopping(),
            };
            run_poller(&service, &fetcher, POLL_INTERVAL, now_ms, &|| notify(&app));
        });
    if spawned.is_err() {
        tracing::warn!("Could not start the followed artist checks.");
    }
}

impl From<FollowingError> for CommandError {
    fn from(error: FollowingError) -> Self {
        match error {
            FollowingError::Invalid(message) => CommandError {
                code: "invalid_following",
                message,
            },
            FollowingError::Busy => CommandError {
                code: "following_check_in_progress",
                message: "A check for new uploads is already running.".into(),
            },
            FollowingError::Storage(message) => {
                tracing::warn!(%message, "followed artist storage failed");
                unavailable()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::{Value, json};
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::{Listener, Manager};

    use super::*;
    use crate::following::FetchedUpload;

    const CHANNEL: &str = "UCMYSjwig4-XrzqY6Ka28t4g";

    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Fixed(Vec<FetchedUpload>);
    impl UploadFetcher for Fixed {
        fn fetch(&self, _: &str) -> Result<Vec<FetchedUpload>, String> {
            Ok(self.0.clone())
        }
    }

    fn harness(
        name: &str,
    ) -> (
        tauri::App<tauri::test::MockRuntime>,
        tauri::WebviewWindow<tauri::test::MockRuntime>,
        Cleanup,
    ) {
        let directory = std::env::temp_dir().join(format!(
            "gmusic-following-ipc-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let cleanup = Cleanup(directory.clone());
        let app = mock_builder()
            .manage(AppState::from_library_directory(directory).unwrap())
            .invoke_handler(tauri::generate_handler![
                crate::commands::inspect_following,
                crate::commands::follow_artist,
                crate::commands::unfollow_artist,
                crate::commands::check_following
            ])
            .build(mock_context(noop_assets()))
            .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        (app, window, cleanup)
    }

    fn invoke(
        window: &tauri::WebviewWindow<tauri::test::MockRuntime>,
        cmd: &str,
        body: Value,
    ) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            window,
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
    }

    #[test]
    fn follow_contract_validates_ids_and_emits_only_after_saved_changes() {
        let (app, window, _cleanup) = harness("contract");
        let events = Arc::new(AtomicU64::new(0));
        let count = events.clone();
        app.listen("following-updated", move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });

        assert_eq!(
            invoke(&window, "inspect_following", json!({})).unwrap(),
            json!({"artists": []})
        );
        let error = invoke(
            &window,
            "follow_artist",
            json!({"channelId":"@lemurian.archives","name":"Lemuria"}),
        )
        .unwrap_err();
        assert_eq!(error["code"], "invalid_following");
        assert_eq!(events.load(Ordering::Relaxed), 0);

        let followed = invoke(
            &window,
            "follow_artist",
            json!({"channelId":CHANNEL,"name":"Lemuria"}),
        )
        .unwrap();
        assert_eq!(followed["artists"][0]["channelId"], CHANNEL);
        assert_eq!(followed["artists"][0]["name"], "Lemuria");
        assert_eq!(followed["artists"][0]["baselineEstablished"], false);
        assert_eq!(events.load(Ordering::Relaxed), 1);

        let unfollowed = invoke(&window, "unfollow_artist", json!({"channelId":CHANNEL})).unwrap();
        assert_eq!(unfollowed, json!({"artists": []}));
        assert_eq!(events.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn manual_check_rejects_unfollowed_or_malformed_channels_without_network() {
        let (_app, window, _cleanup) = harness("check-invalid");
        for channel in [json!(CHANNEL), json!("@handle")] {
            let error =
                invoke(&window, "check_following", json!({"channelId": channel})).unwrap_err();
            assert_eq!(error["code"], "invalid_following");
        }
    }

    #[test]
    fn check_with_stores_uploads_and_notifies_the_windows() {
        let (app, _window, _cleanup) = harness("check-with");
        let events = Arc::new(AtomicU64::new(0));
        let count = events.clone();
        app.listen("following-updated", move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        let service = app.state::<AppState>().following().unwrap();
        service.follow(CHANNEL, "Lemuria", 1).unwrap();
        let fetcher = Fixed(vec![FetchedUpload {
            video_id: "aaaaaaaaaaa".into(),
            title: "Song".into(),
            duration_ms: None,
        }]);
        let snapshot = check_with(app.handle(), &service, &fetcher, None).unwrap();
        assert_eq!(snapshot.artists[0].uploads.len(), 1);
        assert!(snapshot.artists[0].baseline_established);
        assert!(events.load(Ordering::Relaxed) >= 2);
    }

    #[test]
    fn a_running_check_reports_a_busy_error_code() {
        let error = CommandError::from(FollowingError::Busy);
        assert_eq!(error.code, "following_check_in_progress");
    }

    #[test]
    fn default_state_without_a_library_reports_unavailable() {
        let state = AppState::default();
        assert!(state.following().is_none());
    }
}
