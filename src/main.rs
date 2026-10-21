#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod advanced;
mod archives;
mod cloudflare;
mod desktop;
mod documents;
mod duckdb;
mod external_links;
mod generate;
mod history;
mod library;
mod limits;
mod oauth;
mod ocr;
mod office;
mod office_suite;
mod patching;
mod pdf_design;
mod pdf_forms;
mod pdf_ops;
mod process_job;
mod server;
mod sqlite;
mod templates;
mod updater;
mod v1;
mod worker;
mod writing;

#[cfg(test)]
mod v04_tests;

use tauri::Manager;
fn main() {
    if updater::helper_main() {
        return;
    }
    if worker::worker_main() {
        return;
    }
    tauri::Builder::default()
        .manage(updater::Updater::default())
        .manage(office_suite::OfficeSuite::default())
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            v1::state::initialize(&folder);
            updater::cleanup();
            app.manage(desktop::Desktop::new(folder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop::snapshot,
            office_suite::office_new,
            office_suite::office_pick,
            office_suite::office_asset,
            office_suite::office_ocr,
            office_suite::office_open_shared,
            office_suite::office_save,
            office_suite::office_close,
            office_suite::office_lifecycle,
            office_suite::office_close_app,
            desktop::add_folder,
            desktop::remove_folder,
            desktop::set_folder_writable,
            desktop::save_settings,
            desktop::start_mcp,
            desktop::stop_mcp,
            desktop::list_history,
            desktop::restore_version,
            desktop::preview_file,
            external_links::open_external_link,
            v1::desktop::v1_workspace,
            v1::desktop::save_brand_profiles,
            v1::desktop::build_local_index,
            v1::desktop::revision_action,
            v1::desktop::preview_revision,
            v1::desktop::workflow_action,
            oauth::oauth_status,
            oauth::oauth_decision,
            updater::update_status,
            updater::check_update,
            updater::download_update,
            updater::install_update
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<office_suite::OfficeSuite>().prevent_close() {
                    api.prevent_close();
                }
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<desktop::Desktop>().stop();
            }
        })
        .run(tauri::generate_context!())
        .expect("Impossible de démarrer MCP Win");
}
