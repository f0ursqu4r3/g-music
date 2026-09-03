mod agent;
mod artwork;
mod auth;
mod commands;
mod diagnostics;
mod persistence;
pub mod playback;
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
            }
            agent::start(app.handle().clone())?;

            if let Some(window) = app.get_webview_window("main") {
                windows::apply_native_glass(&window);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::inspect_playback,
            commands::inspect_playback_transport,
            commands::inspect_library,
            commands::inspect_metadata_refreshes,
            commands::inspect_youtube_auth,
            commands::open_youtube_login,
            commands::save_youtube_session,
            commands::disconnect_youtube,
            commands::import_youtube_urls,
            commands::show_import_window,
            commands::update_tracks_metadata,
            commands::toggle_favorite,
            commands::upsert_playlist,
            commands::delete_playlist,
            commands::play,
            commands::pause,
            commands::previous,
            commands::next,
            commands::play_track,
            commands::queue_track_next,
            commands::add_to_queue,
            commands::seek,
            commands::set_volume,
            commands::move_queue_item,
            commands::resolve_youtube_artwork,
        ])
        .build(tauri::generate_context!());

    let app = match app {
        Ok(app) => app,
        Err(error) => {
            tracing::error!(%error, "G Music stopped with an error");
            return;
        }
    };

    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            if let Some(state) = app.try_state::<commands::AppState>() {
                state.shutdown();
            }
            agent::shutdown(app);
        }
    });
}
