mod agent;
mod artwork;
mod auth;
mod commands;
mod diagnostics;
mod media_session;
mod persistence;
pub mod playback;
mod process;
mod windows;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting G Music");

    let app = tauri::Builder::default()
        .menu(windows::build_menu)
        .on_menu_event(|app, event| windows::handle_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            let library_directory = app.path().app_data_dir()?;
            let state = commands::AppState::from_library_directory(library_directory)?;
            app.manage(state);
            if let Some(state) = app.try_state::<commands::AppState>() {
                state.start_dirty_refresh(app.handle().clone());
                state.start_transport_monitor(app.handle().clone());
            }
            if media_session::start(app.handle()).is_err() {
                tracing::warn!("Native media controls are unavailable.");
            }
            agent::start(app.handle().clone())?;

            if let Some(window) = app.get_webview_window("main") {
                windows::apply_native_glass(&window);
            }

            Ok(())
        })
        .invoke_handler(|invoke: tauri::ipc::Invoke<tauri::Wry>| {
            if !trusted_command_window(invoke.message.webview_ref().label()) {
                invoke
                    .resolver
                    .reject("This window cannot use application commands.");
                return true;
            }
            let handler: Box<dyn Fn(tauri::ipc::Invoke<tauri::Wry>) -> bool> =
                Box::new(tauri::generate_handler![
                    commands::search_youtube,
                    commands::cancel_youtube_import,
                    commands::retry_metadata_refreshes,
                    commands::clear_queue,
                    commands::reset_track_metadata,
                    commands::inspect_diagnostics,
                    commands::export_library_backup,
                    commands::inspect_playback,
                    commands::inspect_playback_transport,
                    commands::inspect_library,
                    commands::inspect_import_progress,
                    commands::inspect_metadata_refreshes,
                    commands::inspect_youtube_auth,
                    commands::open_youtube_login,
                    commands::save_youtube_session,
                    commands::disconnect_youtube,
                    commands::import_youtube_urls,
                    commands::show_import_window,
                    windows::show_app_window,
                    commands::update_tracks_metadata,
                    commands::toggle_favorite,
                    commands::remove_tracks,
                    commands::upsert_playlist,
                    commands::preview_smart_playlist,
                    commands::freeze_smart_playlist,
                    commands::reorder_playlists,
                    commands::delete_playlist,
                    commands::play,
                    commands::pause,
                    commands::previous,
                    commands::next,
                    commands::toggle_shuffle,
                    commands::cycle_repeat_mode,
                    commands::play_track,
                    commands::queue_track_next,
                    commands::add_to_queue,
                    commands::seek,
                    commands::set_volume,
                    commands::move_queue_item,
                    commands::remove_queue_item,
                    commands::resolve_youtube_artwork,
                ]);
            handler(invoke)
        })
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            tracing::error!(%error, "G Music stopped with an error");
            return;
        }
    };

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && let Some(state) = app.try_state::<commands::AppState>()
            && state.begin_shutdown()
        {
            api.prevent_exit();
            media_session::stop(app);
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                app.state::<commands::AppState>().shutdown();
                agent::shutdown(&app);
                app.exit(0);
            });
        }
    });
}

fn trusted_command_window(label: &str) -> bool {
    [
        "main",
        "artwork",
        "queue",
        "mini-player",
        "settings",
        "import",
    ]
    .contains(&label)
}

#[cfg(test)]
mod security_tests {
    #[test]
    fn external_login_window_has_no_application_commands() {
        assert!(!super::trusted_command_window("youtube-auth"));
        assert!(!super::trusted_command_window("unknown"));
        assert!(super::trusted_command_window("main"));
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert!(
            !capability["windows"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "youtube-auth")
        );
    }
}
