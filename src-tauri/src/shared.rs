// INPUT: AppHandle
// OUTPUT: resolved paths, config IO, event emits shared by bootstrap/server/menu
// POS: src-tauri/src/shared.rs
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub key_asked: bool,
}

pub fn data_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("app_data_dir unavailable");
    let _ = fs::create_dir_all(dir.join("logs"));
    dir
}

/// 查找系统 Node.js: 先 PATH, 再常见安装位置 (Finder 启动时 PATH 不完整).
/// 优先返回"带 npm"的 node (Homebrew 新版 node 公式已不含 npm, 需跳过);
/// 若所有候选都不带 npm, 回退到第一个可用的 node (供 dsh web 运行).
pub fn find_node() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.push(dir.join("node"));
        }
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin/node"));
    candidates.push(PathBuf::from("/usr/local/bin/node"));
    if let Some(home) = std::env::var_os("HOME") {
        let nvm = Path::new(&home).join(".nvm/versions/node");
        if let Ok(rd) = fs::read_dir(&nvm) {
            for e in rd.flatten() {
                candidates.push(e.path().join("bin/node"));
            }
        }
        let mise = Path::new(&home).join(".local/share/mise/installs/node");
        if let Ok(rd) = fs::read_dir(&mise) {
            for e in rd.flatten() {
                candidates.push(e.path().join("bin/node"));
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut fallback: Option<PathBuf> = None;
    for c in candidates {
        if !c.is_file() {
            continue;
        }
        if seen.insert(c.clone()) {
            if npm_cli(&c).is_some() {
                return Some(c);
            }
            if fallback.is_none() {
                fallback = Some(c);
            }
        }
    }
    fallback
}

/// npm CLI 入口 (node <npm-cli.js>), 兼容 homebrew/nvm/官方 tarball 布局.
pub fn npm_cli(node: &Path) -> Option<PathBuf> {
    let real = fs::canonicalize(node).unwrap_or_else(|_| node.to_path_buf());
    let dir = real.parent()?;
    let rels = [
        "../lib/node_modules/npm/bin/npm-cli.js",
        "../../lib/node_modules/npm/bin/npm-cli.js",
        "../../../lib/node_modules/npm/bin/npm-cli.js",
    ];
    for rel in rels {
        let p = dir.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn dsh_bin(app: &AppHandle) -> PathBuf {
    data_dir(app).join("node_modules/@deepseek-ai/dsh/lib/bin.js")
}

pub fn dsh_installed(app: &AppHandle) -> bool {
    dsh_bin(app).is_file()
}

/// 用 macOS 原生对话框收集 API Key (hidden answer, 支持系统级 ⌘V 粘贴).
/// 返回 Some(key) 表示用户点"保存"并输入了内容; None 表示取消或失败.
pub fn prompt_api_key(message: &str) -> Option<String> {
    let script = format!(
        "display dialog \"{}\" default answer \"\" with hidden answer buttons {{\"取消\",\"保存\"}} default button \"保存\"",
        message
    );
    let out = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .ok()?;
    // 用户点"取消"时 osascript 退出码非零
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // 输出形如: button returned:保存, text returned:sk-xxxx[, gave up:true]
    let needle = "text returned:";
    let idx = text.find(needle)?;
    let rest = &text[idx + needle.len()..];
    // 值到下一个 ", " 或行尾结束
    let end = rest.find(", ").unwrap_or(rest.len());
    let val = rest[..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

pub fn path_env(app: &AppHandle) -> String {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(node) = find_node() {
        if let Some(d) = node.parent() {
            dirs.push(d.to_path_buf());
        }
    }
    dirs.push(PathBuf::from("/opt/homebrew/bin"));
    dirs.push(PathBuf::from("/usr/local/bin"));
    dirs.push(data_dir(app).join("node_modules/.bin"));
    let old = std::env::var("PATH").unwrap_or_default();
    dirs.into_iter()
        .map(|d| d.to_string_lossy().to_string())
        .chain(std::iter::once(old))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn read_config(app: &AppHandle) -> Config {
    let path = data_dir(app).join("config.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_config(app: &AppHandle, cfg: &Config) -> std::io::Result<()> {
    let path = data_dir(app).join("config.json");
    fs::write(path, serde_json::to_string_pretty(cfg).expect("config serialize"))
}

pub fn log_line(app: &AppHandle, message: &str) {
    let path = data_dir(app).join("logs/boot.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "{message}");
    }
}

pub fn progress(app: &AppHandle, message: &str) {
    log_line(app, &format!("progress: {message}"));
    let _ = app.emit("boot-progress", serde_json::json!({ "message": message }));
}

pub fn boot_error(app: &AppHandle, message: &str) {
    log_line(app, &format!("error: {message}"));
    let _ = app.emit("boot-error", serde_json::json!({ "message": message }));
}
