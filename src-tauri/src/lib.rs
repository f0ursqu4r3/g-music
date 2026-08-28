mod commands;
pub mod playback;
mod windows;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .menu(windows::build_menu)
        .on_menu_event(|app, event| windows::handle_menu_event(app, event.id().as_ref()))
        .setup(|app| {
            use tauri::Manager;

            if let Some(window) = app.get_webview_window("main") {
                windows::apply_native_glass(&window);
            }

            Ok(())
        })
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::inspect_playback,
            commands::play,
            commands::pause,
            commands::previous,
            commands::next,
            commands::seek,
            commands::set_volume,
            commands::move_queue_item,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        eprintln!("failed to run gmusic: {error}");
    }
}
