// INPUT: user double-click (app lifecycle)
// OUTPUT: dsh web UI loaded in a native window
// POS: src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap;
mod menu;
mod server;
mod shared;
mod state;

use tauri::{Manager, RunEvent};

pub const DSH_PACKAGE: &str = "@deepseek-ai/dsh";
pub const DSH_VERSION: &str = "0.1.0-rc.6";

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .manage(state::AppState::default())
        .setup(|app| {
            menu::install(app)?;
            bootstrap::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap::retry_boot,
            bootstrap::set_api_key,
            bootstrap::skip_api_key
        ])
        .build(tauri::generate_context!())
        .expect("大傻憨 build failed")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
                app.state::<state::AppState>().kill_child();
            }
        });
}
