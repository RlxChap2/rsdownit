pub mod commands;
pub mod downloader;
pub mod models;
pub mod providers;
pub mod security;
pub mod settings;
pub mod storage;
pub mod tools;

use commands::{
    cancel_download, check_tool_updates, check_tools, get_app_settings, get_default_output_dir,
    open_file, open_link, plan_download, preflight, save_app_settings, setup_tools, show_in_folder,
    start_download, AppState,
};
use tauri::Manager;

#[cfg(test)]
mod backend_tests;
#[cfg(test)]
mod engine_tests;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            app.manage(AppState::init(app.handle()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_settings,
            save_app_settings,
            get_default_output_dir,
            check_tools,
            check_tool_updates,
            setup_tools,
            start_download,
            cancel_download,
            open_file,
            show_in_folder,
            open_link,
            preflight,
            plan_download,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
