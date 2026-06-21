pub mod commands;
pub mod events;
pub mod protocol;
pub mod validation;

use commands::ValidationState;

// The Tauri startup contract: if the runtime fails to build/launch,
// the desktop app cannot run. The mobile_entry_point macro requires
// `fn() -> ()`, so we can't bubble out via Result. expect() is the
// idiomatic Tauri startup pattern.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Auto-update: the updater plugin backs the launch-time update check;
        // the process plugin's relaunch() restarts into the installed version.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(ValidationState::new())
        .invoke_handler(tauri::generate_handler![
            commands::validate,
            commands::cancel_validation,
            commands::check_clan_available,
            commands::open_in_clan,
            commands::export_results,
            commands::reveal_in_file_manager,
            commands::install_cli,
        ])
        .run(tauri::generate_context!())
        .expect("error while running chatter desktop");
}
