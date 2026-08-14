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

    // 防丢: 启动时把关键配置备份一份 (失败不阻塞, 仅记录; API Key 在钥匙串, 不进备份)
    if let Err(e) = shared::backup_config(&app) {
        shared::log_line(&app, &format!("backup 失败: {e}"));
    }

    let cfg = shared::read_config(&app);
    if shared::read_api_key(&app).is_none() && !cfg.key_asked {
        shared::progress(&app, "请输入 API Key（可跳过）…");
        // 原生对话框收集 key (可选): 弹一次, 取消不阻塞, 之后可在 dsh 设置页或菜单添加
        let key = tauri::async_runtime::spawn_blocking(|| {
            shared::prompt_api_key("请输入 DeepSeek API Key（可选，可稍后在 dsh 设置中添加）")
        })
        .await
        .ok()
        .flatten();
        if let Some(k) = key {
            shared::save_api_key(&app, &k);
        } else {
            let mut c = shared::read_config(&app);
            c.key_asked = true;
            let _ = shared::write_config(&app, &c);
        }
    }

    shared::progress(&app, "正在启动本地服务…");
    match launch_with_retry(&app) {
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

/// 健康自愈: dsh web 启动失败/崩溃时自动重试一次 (杀残留 -> 重启), 两次失败才报错.
fn launch_with_retry(app: &AppHandle) -> Result<String, String> {
    match server::launch(app) {
        Ok(url) => Ok(url),
        Err(first) => {
            shared::log_line(app, &format!("首次启动异常: {first}"));
            app.state::<AppState>().kill_child();
            std::thread::sleep(std::time::Duration::from_secs(2));
            shared::progress(app, "服务启动异常, 自动重试…");
            match server::launch(app) {
                Ok(url) => {
                    shared::log_line(app, "重试后启动成功");
                    Ok(url)
                }
                Err(second) => Err(format!("首次: {first}\n重试: {second}")),
            }
        }
    }
}

pub fn npm_install(app: &AppHandle) -> Result<(), String> {
    npm_install_version(app, crate::DSH_PACKAGE, crate::DSH_VERSION)
}

/// 安装指定版本 dsh. 显式官方 registry: 绕过用户 ~/.npmrc 的内网镜像
/// (如腾讯 registry.npm.oa.com, 上面没有 @deepseek-ai/dsh 会 404).
pub fn npm_install_version(app: &AppHandle, pkg: &str, version: &str) -> Result<(), String> {
    let node = shared::find_node(app)
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
            &format!("{pkg}@{version}"),
            "--no-audit",
            "--no-fund",
            "--loglevel=warn",
            "--registry=https://registry.npmjs.org/",
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

/// 重启 dsh web: 杀残留 -> 启动 -> HTTP 200 门禁 -> 导航窗口 (升级/回滚后复用).
pub fn restart_dsh(app: &AppHandle) -> Result<String, String> {
    app.state::<AppState>().kill_child();
    let url = server::launch(app)?;
    // 升级路径门禁: TCP 端口通不代表 HTTP 就绪, 必须确认页面可访问
    if !http_ready(&url) {
        return Err(format!("服务端口已开但 HTTP 未就绪: {url}"));
    }
    if let Some(w) = app.get_webview_window("main") {
        if let Err(e) = w.navigate(url.parse().expect("valid url")) {
            shared::boot_error(app, &format!("页面加载失败: {e}"));
        }
    }
    Ok(url)
}

/// 轮询等待本地 HTTP 返回 200 (最多 15s).
fn http_ready(url: &str) -> bool {
    for _ in 0..15 {
        if let Ok(out) = Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "--max-time", "3"])
            .arg(url)
            .output()
        {
            if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "200" {
                return true;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    false
}

#[tauri::command]
pub fn retry_boot(app: AppHandle) {
    start(app);
}
