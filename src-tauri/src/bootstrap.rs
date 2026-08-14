// INPUT: app start / retry / install trigger
// OUTPUT: dsh installed, API key collected, server launched, webview navigated
// POS: src-tauri/src/bootstrap.rs
use crate::server;
use crate::shared;
use crate::state::AppState;
use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Manager};

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
        shared::progress(&app, "请输入 API Key（可跳过）…");
        // 原生对话框收集 key (可选): 弹一次, 取消不阻塞, 之后可在 dsh 设置页或菜单添加
        let key = tauri::async_runtime::spawn_blocking(|| {
            shared::prompt_api_key("请输入 DeepSeek API Key（可选，可稍后在 dsh 设置中添加）")
        })
        .await
        .ok()
        .flatten();
        if let Some(k) = key {
            let mut c = shared::read_config(&app);
            c.api_key = Some(k);
            c.key_asked = true;
            let _ = shared::write_config(&app, &c);
        } else {
            let mut c = shared::read_config(&app);
            c.key_asked = true;
            let _ = shared::write_config(&app, &c);
        }
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
    let node = shared::find_node()
        .ok_or("未检测到 Node.js. 请先安装 Node.js (https://nodejs.org) 后再试.")?;
    let npm_cli = shared::npm_cli(&node)
        .ok_or("找到 Node.js 但未找到 npm, 请确认 Node 安装完整.")?;
    let data = shared::data_dir(app);
    let log_path = data.join("logs/npm.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    let status = Command::new(&node)
        .arg(&npm_cli)
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
