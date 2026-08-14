// INPUT: AppHandle
// OUTPUT: resolved paths, config IO, event emits shared by bootstrap/server/menu
// POS: src-tauri/src/shared.rs
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
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

pub fn resource_node(app: &AppHandle) -> PathBuf {
    app.path().resource_dir().expect("resource_dir missing").join("node")
}

pub fn npm_cli(app: &AppHandle) -> PathBuf {
    app.path().resource_dir().expect("resource_dir missing").join("npm/bin/npm-cli.js")
}

pub fn dsh_bin(app: &AppHandle) -> PathBuf {
    data_dir(app).join("node_modules/@deepseek-ai/dsh/lib/bin.js")
}

pub fn dsh_installed(app: &AppHandle) -> bool {
    dsh_bin(app).is_file()
}

pub fn path_env(app: &AppHandle) -> String {
    let res = app.path().resource_dir().expect("resource_dir missing");
    let bin = data_dir(app).join("node_modules/.bin");
    let old = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}:{}", res.display(), bin.display(), old)
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
