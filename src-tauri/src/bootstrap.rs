// INPUT: app start / retry / install trigger
// OUTPUT: dsh installed, API key collected, server launched, webview navigated
// POS: src-tauri/src/bootstrap.rs
use crate::server;
use crate::shared;
use crate::state::AppState;
use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter, Manager};

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        flow(app).await;
    });
}

pub fn install_then_start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        shared::progress(&app, "正在更新 dsh…");
        match npm_install(&app) {
            Ok(()) => flow(app).await,
            Err(e) => shared::boot_error(&app, &format!("更新失败: {e}")),
        }
    });
}

async fn flow(app: AppHandle) {
    app.state::<AppState>().kill_child();
    shared::progress(&app, "正在检查运行环境…");

    if !shared::dsh_installed(&app) {
        shared::progress(&app, "首次运行, 正在安装 dsh (约 1~3 分钟)…");
        if let Err(e) = npm_install(&app) {
            shared::boot_error(&app, &format!("安装失败, 请检查网络后重试.\n{e}"));
            return;
        }
    }

    let cfg = shared::read_config(&app);
    if cfg.api_key.is_none() && !cfg.key_asked {
        let _ = app.emit("show-api-key", ());
        return;
    }

    shared::progress(&app, "正在启动本地服务…");
    match server::launch(&app) {
        Ok(url) => {
            if let Some(w) = app.get_webview_window("main") {
                if let Err(e) = w.navigate(url.parse().expect("valid url")) {
                    shared::boot_error(&app, &format!("页面加载失败: {e}"));
                }
            }
        }
        Err(e) => shared::boot_error(&app, &format!("服务启动失败.\n{e}")),
    }
}

pub fn npm_install(app: &AppHandle) -> Result<(), String> {
    let data = shared::data_dir(app);
    let log_path = data.join("logs/npm.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    let status = Command::new(shared::resource_node(app))
        .arg(shared::npm_cli(app))
        .args([
            "install",
            "--prefix",
            &data.to_string_lossy(),
            &format!("{}@{}", crate::DSH_PACKAGE, crate::DSH_VERSION),
            "--no-audit",
            "--no-fund",
            "--loglevel=warn",
        ])
        .env("PATH", shared::path_env(app))
        .env("npm_config_cache", data.join("npm-cache"))
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .status()
        .map_err(|e| format!("npm 启动失败: {e}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "npm install 退出码 {:?}, 详见日志 {}",
            status.code(),
            log_path.display()
        ))
    }
}

#[tauri::command]
pub fn retry_boot(app: AppHandle) {
    start(app);
}

#[tauri::command]
pub fn set_api_key(app: AppHandle, key: String) {
    let mut cfg = shared::read_config(&app);
    cfg.api_key = if key.is_empty() { None } else { Some(key) };
    cfg.key_asked = true;
    let _ = shared::write_config(&app, &cfg);
    start(app);
}

#[tauri::command]
pub fn skip_api_key(app: AppHandle) {
    let mut cfg = shared::read_config(&app);
    cfg.key_asked = true;
    let _ = shared::write_config(&app, &cfg);
    start(app);
}
