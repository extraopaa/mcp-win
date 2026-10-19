#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod advanced;
mod desktop;
mod generate;
mod library;
mod ocr;
mod office;
mod process_job;
mod server;
mod tunnel;
mod writing;

use tauri::Manager;
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            app.manage(desktop::Desktop::new(folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop::snapshot,
            desktop::add_folder,
            desktop::remove_folder,
            desktop::set_folder_writable,
            desktop::save_settings,
            desktop::start_mcp,
            desktop::stop_mcp,
            desktop::preview_file
        ])
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<desktop::Desktop>().stop();
            }
        })
        .run(tauri::generate_context!())
        .expect("Impossible de démarrer MCP Win");
}
