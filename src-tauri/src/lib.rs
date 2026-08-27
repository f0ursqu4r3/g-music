mod commands;
pub mod playback;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
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
