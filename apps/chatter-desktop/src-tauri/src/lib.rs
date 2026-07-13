// Unit-test modules: panic-family clippy lints relaxed by policy
// (see the workspace [lints] table for the production deny).
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented,
    )
)]

pub mod commands;
pub mod events;
pub mod protocol;
pub mod validation;

use commands::ValidationState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Wry};

/// Menu-item id for the "Check for Updates..." app-menu entry. The
/// `on_menu_event` handler matches on this to trigger a manual update check.
const CHECK_FOR_UPDATES_MENU_ID: &str = "check-for-updates";

/// Webview event emitted when the user clicks "Check for Updates...". The
/// frontend (`App.tsx`, via the runtime seam) listens for this and runs the
/// manual update check. Keep in sync with `MENU_CHECK_FOR_UPDATES_EVENT` in
/// `src/runtime/tauriTransport.ts`.
const CHECK_FOR_UPDATES_EVENT: &str = "menu://check-for-updates";

/// Menu-item id for "About Chatter". A custom item (not the predefined native
/// about panel) so the frontend can show a rich, link-carrying modal instead.
const ABOUT_MENU_ID: &str = "about";

/// Webview event emitted when the user clicks "About Chatter". The frontend
/// opens its custom `AboutModal`. Keep in sync with `MENU_ABOUT_EVENT` in
/// `src/runtime/tauriTransport.ts`.
const ABOUT_EVENT: &str = "menu://about";

/// Build the application menu.
///
/// Replaces Tauri's default menu, so the standard Edit/Window items are
/// re-added explicitly (dropping them would break copy/paste/close). The app
/// submenu carries a custom "About Chatter" item (a rich, link-carrying React
/// modal, since the native about panel cannot show clickable links) plus a
/// "Check for Updates..." item that the launch-time auto-updater does not
/// provide on its own.
fn build_app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let about = MenuItem::with_id(app, ABOUT_MENU_ID, "About Chatter", true, None::<&str>)?;
    let check_updates = MenuItem::with_id(
        app,
        CHECK_FOR_UPDATES_MENU_ID,
        "Check for Updates...",
        true,
        None::<&str>,
    )?;

    let app_menu = Submenu::with_items(
        app,
        "Chatter",
        true,
        &[
            &about,
            &PredefinedMenuItem::separator(app)?,
            &check_updates,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
}

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
        .menu(build_app_menu)
        .on_menu_event(|app, event| {
            // Hand menu clicks off to the frontend, which owns the update
            // capability and the About modal, keeping `@tauri-apps/*` behind
            // the runtime seam. Best-effort: a failed emit must not crash.
            match event.id().as_ref() {
                CHECK_FOR_UPDATES_MENU_ID => {
                    let _ = app.emit(CHECK_FOR_UPDATES_EVENT, ());
                }
                ABOUT_MENU_ID => {
                    let _ = app.emit(ABOUT_EVENT, ());
                }
                _ => {}
            }
        })
        .manage(ValidationState::new())
        .invoke_handler(tauri::generate_handler![
            commands::validate,
            commands::cancel_validation,
            commands::check_clan_available,
            commands::open_in_clan,
            commands::export_results,
            commands::reveal_in_file_manager,
            commands::install_cli,
            commands::open_external,
        ])
        .run(tauri::generate_context!())
        .expect("error while running chatter desktop");
}
