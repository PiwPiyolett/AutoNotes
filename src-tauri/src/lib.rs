pub mod commands;
pub mod corrector;
pub mod export;
pub mod store;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("aplikasi butuh direktori data");
            // Folder kamus yang dibundel Tauri (ada saat aplikasi terpasang).
            let resource_dict = app
                .path()
                .resource_dir()
                .ok()
                .map(|r| r.join("resources").join("dict"));
            let state = AppState::init(data_dir, resource_dict).expect("gagal menyiapkan aplikasi");
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_text,
            commands::fix_all,
            commands::corrector_settings,
            commands::set_corrector_settings,
            commands::dict_status,
            commands::add_to_user_dict,
            commands::record_undo,
            commands::list_notes,
            commands::load_note,
            commands::create_note,
            commands::save_note,
            commands::trash_note,
            commands::search_notes,
            commands::stats,
            commands::aggressiveness_options,
            commands::export_note,
        ])
        .run(tauri::generate_context!())
        .expect("gagal menjalankan aplikasi tauri");
}
