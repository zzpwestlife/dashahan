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
    /// 是否已询问过 API Key (避免每次启动都弹框); 真值只存钥匙串, config 不落盘明文.
    #[serde(default)]
    pub key_asked: bool,
    /// 对话完成/权限请求系统通知开关 (默认开)
    #[serde(default = "default_true")]
    pub notify_enabled: bool,
}

fn default_true() -> bool {
    true
}

pub fn data_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .expect("app_data_dir unavailable");
    let _ = fs::create_dir_all(dir.join("logs"));
    dir
}

/// 查找 Node.js: 优先 app 内嵌的 (完整版打包在 Resources/node-dist), 再 PATH, 再常见位置.
/// 优先返回"带 npm"的 node (Homebrew 新版 node 公式已不含 npm, 需跳过);
/// 若所有候选都不带 npm, 回退到第一个可用的 node (供 dsh web 运行).
/// 从候选里挑 node: 优先"带 npm"的 (Homebrew 新版 node 公式已不含 npm, 需跳过),
/// 都不带则回退第一个存在的 (供 dsh web 运行). 去重, 跳过不存在的路径.
fn pick_node(candidates: &[PathBuf]) -> Option<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    let mut fallback: Option<PathBuf> = None;
    for c in candidates {
        if !c.is_file() {
            continue;
        }
        if seen.insert(c.clone()) {
            if npm_cli(c).is_some() {
                return Some(c.clone());
            }
            if fallback.is_none() {
                fallback = Some(c.clone());
            }
        }
    }
    fallback
}

/// 收集所有候选 node 路径: 内嵌 (完整版) -> PATH -> 常见位置 -> nvm/mise.
fn node_candidates(app: &AppHandle) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    // 1) 内嵌 Node (完整版构建, 轻量版无此目录自动跳过)
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("node-dist/bin/node"));
    }
    // 2) PATH
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
    candidates
}

pub fn find_node(app: &AppHandle) -> Option<PathBuf> {
    pick_node(&node_candidates(app))
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
    if let Some(node) = find_node(app) {
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

// ---------- macOS 钥匙串 (API Key 防丢) ----------

pub const KEYCHAIN_SERVICE: &str = "com.solo.dashahan";
pub const KEYCHAIN_ACCOUNT: &str = "deepseek-api-key";

/// 把 key 写入钥匙串 (已存在则覆盖). 失败返回 false (调用方可忽略, config 仍是兜底).
pub fn keychain_save(value: &str) -> bool {
    std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
            value,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 从钥匙串读 key; 不存在/失败返回 None.
pub fn keychain_load() -> Option<String> {
    let out = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-a",
            KEYCHAIN_ACCOUNT,
            "-s",
            KEYCHAIN_SERVICE,
            "-w",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 读取 API Key: 唯一真值在钥匙串. 老版本 config.json 里的明文 key 会先迁移到钥匙串并清理.
pub fn read_api_key(app: &AppHandle) -> Option<String> {
    migrate_legacy_key(app);
    keychain_load()
}

/// 老版本 (≤v0.1.4) 把 key 明文存在 config.json; 现在只存标记, 真值在钥匙串.
/// 读到遗留明文时: 先写入钥匙串 (成功才清理, 防止 key 丢失), 再标记 key_asked.
fn migrate_legacy_key(app: &AppHandle) {
    let path = data_dir(app).join("config.json");
    let Ok(raw) = fs::read_to_string(&path) else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else { return };
    let Some(key) = v
        .get("api_key")
        .and_then(|k| k.as_str())
        .filter(|k| !k.is_empty())
    else {
        return; // 无遗留明文, 不动
    };
    if keychain_save(key) {
        let mut cfg: Config = serde_json::from_value(v).unwrap_or_default();
        cfg.key_asked = true;
        let _ = write_config(app, &cfg);
    }
}

/// 保存 API Key: 只写钥匙串; config 仅记录"已设置"标记 (不落盘明文).
pub fn save_api_key(app: &AppHandle, key: &str) {
    let _ = keychain_save(key);
    let mut c = read_config(app);
    c.key_asked = true;
    let _ = write_config(app, &c);
}

// ---------- 配置自动备份 (防数据目录被外部清空) ----------

/// 每次启动把关键配置备份到 ~/DASH-backup/<时间戳>/, 只保留最近 5 份.
/// 排除 node_modules/npm-cache/logs; 包含 config.json、dsh-home(含 sessions/storages/profiles).
pub fn backup_config(app: &AppHandle) -> Result<(), String> {
    let data = data_dir(app);
    if !data.join("dsh-home").exists() {
        return Ok(()); // dsh 尚未安装, 无备份价值
    }
    let home = std::env::var("HOME").map_err(|_| "HOME 不可用".to_string())?;
    let root = Path::new(&home).join("DASH-backup");
    let _ = fs::create_dir_all(&root);
    let ts = String::from_utf8_lossy(
        &std::process::Command::new("date")
            .args(["+%Y%m%d-%H%M%S"])
            .output()
            .map_err(|e| e.to_string())?
            .stdout,
    )
    .trim()
    .to_string();
    let dst = root.join(&ts);
    let _ = fs::create_dir_all(&dst);
    let status = std::process::Command::new("rsync")
        .args([
            "-a",
            "--exclude",
            "node_modules",
            "--exclude",
            "npm-cache",
            "--exclude",
            "logs",
        ])
        .arg(format!("{}/", data.display()))
        .arg(format!("{}/", dst.display()))
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("rsync 退出码 {:?}", status.code()));
    }
    prune_backups(&root, 5);
    Ok(())
}

/// 只保留 root 下按名称排序最近的 keep 份, 删除更旧的.
fn prune_backups(root: &Path, keep: usize) {
    let Ok(mut entries) = fs::read_dir(root).map(|rd| rd.flatten().collect::<Vec<_>>()) else {
        return;
    };
    entries.sort_by_key(|e| e.file_name());
    while entries.len() > keep {
        let oldest = entries.remove(0);
        let _ = fs::remove_dir_all(oldest.path());
    }
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

// ---------- 版本与更新 ----------

/// 语义化版本比较: a < b (如 "0.1.0-rc.6" < "0.2.0").
/// 任一侧解析失败视为不升级 (返回 false), 保证异常不触发升级.
pub fn version_less(a: &str, b: &str) -> bool {
    match (semver::Version::parse(a), semver::Version::parse(b)) {
        (Ok(va), Ok(vb)) => va < vb,
        _ => false,
    }
}

/// 实际安装的 dsh 版本 (读 node_modules/@deepseek-ai/dsh/package.json).
pub fn installed_dsh_version(app: &AppHandle) -> Option<String> {
    let bin = dsh_bin(app);
    let pkg_root = bin.parent()?.parent()?; // .../node_modules/@deepseek-ai/dsh
    let raw = fs::read_to_string(pkg_root.join("package.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    v.get("version")?.as_str().map(|s| s.to_string())
}

/// curl 拉取 URL 文本 (macOS 自带 curl; 失败返回 None).
fn fetch_text(url: &str, extra: &[&str]) -> Option<String> {
    let out = std::process::Command::new("curl")
        .args(["-sS", "-L", "-m", "15"])
        .args(extra)
        .arg(url)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// npm 官方 registry 上 @deepseek-ai/dsh 的 latest 版本.
pub fn fetch_dsh_latest() -> Option<String> {
    let body = fetch_text("https://registry.npmjs.org/@deepseek-ai/dsh/latest", &[])?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v.get("version")?.as_str().map(|s| s.to_string())
}

/// GitHub Releases 最新 tag (去掉 v 前缀).
pub fn fetch_dash_latest() -> Option<String> {
    let body = fetch_text(
        "https://api.github.com/repos/zzpwestlife/dashahan/releases/latest",
        &["-H", "User-Agent: dashahan-updater"],
    )?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    v.get("tag_name")?
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
}

/// 备份当前 dsh 安装 (data/backups/dsh), 供升级失败回滚.
pub fn backup_dsh(app: &AppHandle) -> Result<PathBuf, String> {
    let src = dsh_bin(app).parent().and_then(|p| p.parent()).ok_or("路径异常")?.to_path_buf();
    if !src.join("package.json").is_file() {
        return Err("dsh 尚未安装, 无需备份".to_string());
    }
    let dst = data_dir(app).join("backups/dsh");
    let _ = fs::remove_dir_all(&dst);
    fs::create_dir_all(dst.parent().ok_or("路径异常")?).map_err(|e| e.to_string())?;
    let st = std::process::Command::new("cp")
        .arg("-R")
        .arg(&src)
        .arg(&dst)
        .status()
        .map_err(|e| format!("cp 启动失败: {e}"))?;
    if st.success() {
        Ok(dst)
    } else {
        Err(format!("备份失败 (cp 退出码 {:?})", st.code()))
    }
}

/// 从备份恢复 dsh (升级失败回滚).
pub fn restore_dsh(app: &AppHandle) -> Result<(), String> {
    let src = data_dir(app).join("backups/dsh");
    if !src.join("package.json").is_file() {
        return Err("无可用备份".to_string());
    }
    let dst = dsh_bin(app).parent().and_then(|p| p.parent()).ok_or("路径异常")?.to_path_buf();
    let _ = fs::remove_dir_all(&dst);
    fs::create_dir_all(dst.parent().ok_or("路径异常")?).map_err(|e| e.to_string())?;
    let st = std::process::Command::new("cp")
        .arg("-R")
        .arg(&src)
        .arg(&dst)
        .status()
        .map_err(|e| format!("cp 启动失败: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("恢复失败 (cp 退出码 {:?})", st.code()))
    }
}

/// 是否完整版 (内嵌 Node): 用于决定下载 full 还是 lite 的 DASH zip.
pub fn has_embedded_node(app: &AppHandle) -> bool {
    app.path()
        .resource_dir()
        .map(|r| r.join("node-dist/bin/node").is_file())
        .unwrap_or(false)
}

/// macOS 原生确认对话框; 返回 true 表示用户点了"确定".
pub fn confirm(message: &str) -> bool {
    let script = format!(
        "display dialog \"{}\" buttons {{\"取消\",\"确定\"}} default button \"确定\"",
        message
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "dashahan_test_{tag}_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn touch(p: &Path) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    // macOS 上 /var -> /private/var 符号链接: 规范化后再比较路径
    fn canon(p: PathBuf) -> PathBuf {
        fs::canonicalize(&p).unwrap_or(p)
    }

    #[test]
    fn npm_cli_std_layout() {
        let tmp = tempdir("npm_cli_std");
        let node = tmp.join("bin/node");
        touch(&node);
        let cli = tmp.join("lib/node_modules/npm/bin/npm-cli.js");
        touch(&cli);
        assert_eq!(npm_cli(&node).map(canon), Some(canon(cli)));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn npm_cli_two_level_layout() {
        // node 在 <prefix>/<sub>/bin 时, npm-cli 在 <prefix>/lib (../../lib 分支)
        let tmp = tempdir("npm_cli_two");
        let node = tmp.join("a/b/bin/node");
        touch(&node);
        let cli = tmp.join("a/lib/node_modules/npm/bin/npm-cli.js");
        touch(&cli);
        assert_eq!(npm_cli(&node).map(canon), Some(canon(cli)));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn npm_cli_three_level_layout() {
        // node 在 <prefix>/<x>/<y>/bin 时, npm-cli 在 <prefix>/lib (../../../lib 分支)
        let tmp = tempdir("npm_cli_three");
        let node = tmp.join("a/b/c/bin/node");
        touch(&node);
        let cli = tmp.join("a/lib/node_modules/npm/bin/npm-cli.js");
        touch(&cli);
        assert_eq!(npm_cli(&node).map(canon), Some(canon(cli)));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn npm_cli_missing_returns_none() {
        let tmp = tempdir("npm_cli_missing");
        let node = tmp.join("bin/node");
        touch(&node);
        assert_eq!(npm_cli(&node), None);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pick_node_prefers_one_with_npm() {
        let tmp = tempdir("pick_node_pref");
        let bare = tmp.join("bare/bin/node");
        touch(&bare);
        let with_npm = tmp.join("withnpm/bin/node");
        touch(&with_npm);
        let cli = tmp.join("withnpm/lib/node_modules/npm/bin/npm-cli.js");
        touch(&cli);
        let got = pick_node(&[bare.clone(), with_npm.clone()]).unwrap();
        assert_eq!(got, with_npm);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pick_node_falls_back_to_first_existing() {
        let tmp = tempdir("pick_node_fb");
        let missing = tmp.join("missing/bin/node");
        let bare = tmp.join("bare/bin/node");
        touch(&bare);
        let got = pick_node(&[missing, bare.clone()]).unwrap();
        assert_eq!(got, bare);
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pick_node_skips_missing_and_dedups() {
        let tmp = tempdir("pick_node_dedup");
        let missing = tmp.join("missing/bin/node");
        let bare = tmp.join("bare/bin/node");
        touch(&bare);
        assert_eq!(pick_node(&[missing, bare.clone(), bare.clone()]), Some(bare));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn prune_backups_keeps_newest_five() {
        let tmp = tempdir("prune");
        for i in 1..=7 {
            fs::create_dir_all(tmp.join(format!("20260814-{i:04}"))).unwrap();
        }
        prune_backups(&tmp, 5);
        let mut left: Vec<_> = fs::read_dir(&tmp)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        left.sort();
        assert_eq!(left.len(), 5);
        assert_eq!(left.first().unwrap(), "20260814-0003"); // 最旧的 0001/0002 被删
        assert_eq!(left.last().unwrap(), "20260814-0007");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn version_less_compare() {
        assert!(version_less("0.1.0-rc.6", "0.2.0"));
        assert!(version_less("0.1.0-rc.6", "0.1.0-rc.7"));
        assert!(version_less("0.1.5", "0.1.6"));
        assert!(!version_less("0.2.0", "0.1.0-rc.6"));
        assert!(!version_less("0.1.5", "0.1.5"));
        assert!(!version_less("garbage", "0.1.0"));
        assert!(!version_less("0.1.0", "garbage"));
    }
}
