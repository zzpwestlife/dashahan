// INPUT: user double-click (app lifecycle)
// OUTPUT: dsh web UI loaded in a native window
// POS: src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bootstrap;
mod menu;
mod server;
mod shared;
mod state;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent};

pub const DSH_PACKAGE: &str = "@deepseek-ai/dsh";
pub const DSH_VERSION: &str = "0.1.0-rc.6";

const TRAY_TOGGLE: &str = "tray-toggle";

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 再次双击/启动: 显示并聚焦已有窗口 (若被隐藏/最小化)
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .manage(state::AppState::default())
        .setup(|app| {
            menu::install(app)?;
            install_tray(app)?;
            bootstrap::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 关窗不退出: 隐藏窗口, dsh 后台保持运行 (托盘可随时唤回)
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![bootstrap::retry_boot])
        .build(tauri::generate_context!())
        .expect("大傻憨 build failed")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit | RunEvent::ExitRequested { .. }) {
                app.state::<state::AppState>().kill_child();
            }
        });
}

/// 菜单栏托盘: 左键/菜单 显示隐藏窗口; 退出菜单才真正结束 (同时杀掉 dsh).
fn install_tray(app: &tauri::App) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, TRAY_TOGGLE, "显示/隐藏窗口", true, None::<&str>)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出 DASH"))?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;
    TrayIconBuilder::new()
        .icon(app.default_window_icon().expect("default icon").clone())
        .tooltip("DASH - dsh web")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_TOGGLE => toggle_window(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
    }
}
