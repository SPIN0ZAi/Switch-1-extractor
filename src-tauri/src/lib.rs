pub mod commands;

use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            detect_keys,
            inspect_game,
            start_extraction,
            convert_xci_to_nsp,
            pack_pfs0,
            cancel_operation,
            open_folder,
            get_settings,
            save_settings,
            get_initial_file,
            find_matching_base_game,
            find_matching_update,
            find_matching_dlcs,
            get_app_version,
            check_for_updates,
            download_and_install_update,
            window_minimize,
            window_toggle_maximize,
            window_close,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
