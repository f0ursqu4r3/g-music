mod auth;
mod commands;
mod diagnostics;
pub mod playback;
mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::init();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting G Music");

    let result = tauri::Builder::default()
        .menu(windows::build_menu)
        .on_menu_event(|app, event| windows::handle_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            use tauri::Manager;

            let library_path = app.path().app_data_dir()?.join("library.json");
            app.manage(commands::AppState::from_library_path(library_path)?);

            if let Some(window) = app.get_webview_window("main") {
                windows::apply_native_glass(&window);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::inspect_playback,
            commands::inspect_youtube_auth,
            commands::open_youtube_login,
            commands::save_youtube_session,
            commands::disconnect_youtube,
            commands::import_youtube_urls,
            commands::show_import_window,
            commands::play,
            commands::pause,
            commands::previous,
            commands::next,
            commands::play_track,
            commands::seek,
            commands::set_volume,
            commands::move_queue_item,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        tracing::error!(%error, "G Music stopped with an error");
    }
}
