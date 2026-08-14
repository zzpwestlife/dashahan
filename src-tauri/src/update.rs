// INPUT: user clicks update menu / app start
// OUTPUT: dsh upgraded in place, or DASH.app replaced + restarted
// POS: src-tauri/src/update.rs
use crate::bootstrap;
use crate::menu;
use crate::shared;
use crate::state::AppState;
use std::path::Path;
use std::process::Command;
use tauri::{AppHandle, Manager};

/// 检查 dsh 是否有新版; 返回最新版本号 (仅当确实有更新时).
pub fn check_dsh(app: &AppHandle) -> Option<String> {
    let latest = shared::fetch_dsh_latest()?;
    let installed = shared::installed_dsh_version(app).unwrap_or_default();
    if shared::version_less(&installed, &latest) {
        Some(latest)
    } else {
        None
    }
}

/// 检查 DASH 是否有新版; 返回最新版本号 (仅当确实有更新时).
pub fn check_dash(app: &AppHandle) -> Option<String> {
    let latest = shared::fetch_dash_latest()?;
    let local = app.package_info().version.to_string();
    if shared::version_less(&local, &latest) {
        Some(latest)
    } else {
        None
    }
}

/// 启动时后台静默检查两条更新线, 发现新版则缓存并刷新菜单文案.
/// 独立线程: 不阻塞启动, 网络失败静默 (无感).
pub fn check_all_background(app: AppHandle) {
    std::thread::spawn(move || {
        let mut changed = false;
        if let Some(v) = check_dsh(&app) {
            *app.state::<AppState>().dsh_latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(v.clone());
            shared::notify("dsh 有新版本", &format!("v{v} 可用, 点菜单「升级 dsh」一键升级"));
            changed = true;
        }
        if let Some(v) = check_dash(&app) {
            *app.state::<AppState>().dash_latest.lock().unwrap_or_else(|p| p.into_inner()) = Some(v.clone());
            shared::notify("DASH 有新版本", &format!("v{v} 可用, 点菜单「更新 DASH」一键更新"));
            changed = true;
        }
        if changed {
            menu::refresh_update_items(&app);
        }
    });
}

/// 一键升级 dsh: 备份 -> 官方 registry 装 latest -> 重启验证 -> 失败回滚.
pub fn upgrade_dsh(app: &AppHandle) -> Result<String, String> {
    let latest =
        shared::fetch_dsh_latest().ok_or("无法获取 dsh 最新版本 (网络不可达, 请稍后重试)")?;
    let installed = shared::installed_dsh_version(app).unwrap_or_default();
    if installed == latest {
        return Ok(format!("dsh 已是最新版本 v{latest}"));
    }
    shared::notify("正在升级 dsh", &format!("{installed} → v{latest}, 约 1~3 分钟"));
    shared::progress(app, &format!("正在升级 dsh {installed} → {latest}…"));
    shared::backup_dsh(app)?;
    let result = match bootstrap::npm_install_version(app, crate::DSH_PACKAGE, &latest) {
        Ok(()) => match bootstrap::restart_dsh(app) {
            Ok(_) => {
                // 已是最新, 清缓存状态
                *app.state::<AppState>().dsh_latest.lock().unwrap_or_else(|p| p.into_inner()) = None;
                menu::refresh_update_items(app);
                Ok(format!("dsh 已升级到 v{latest}"))
            }
            Err(e) => {
                let _ = shared::restore_dsh(app);
                let _ = bootstrap::restart_dsh(app);
                Err(format!("升级后启动失败, 已自动回滚: {e}"))
            }
        },
        Err(e) => {
            let _ = shared::restore_dsh(app);
            Err(format!("安装失败, 已自动回滚: {e}"))
        }
    };
    match &result {
        Ok(msg) => {
            shared::notify("dsh 升级完成", msg);
            shared::alert("dsh 升级完成", msg);
        }
        Err(e) => {
            shared::notify("dsh 升级失败", e);
            shared::alert("dsh 升级失败", e);
        }
    }
    result
}

/// 一键更新 DASH: 下载对应 zip -> 解压 -> 备份当前 app -> 脚本替换并重启.
pub fn update_dash(app: &AppHandle) -> Result<String, String> {
    let latest =
        shared::fetch_dash_latest().ok_or("无法获取 DASH 最新版本 (网络不可达, 请稍后重试)")?;
    let local = app.package_info().version.to_string();
    if local == latest {
        return Ok(format!("DASH 已是最新版本 v{latest}"));
    }
    let zip_name = if shared::has_embedded_node(app) {
        "DASH-macOS-full.zip"
    } else {
        "DASH-macOS-lite.zip"
    };
    let url = format!(
        "https://github.com/zzpwestlife/dashahan/releases/download/v{latest}/{zip_name}"
    );

    let tmp = std::env::temp_dir().join(format!("dash-update-{latest}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let zip_path = tmp.join(zip_name);

    shared::notify("DASH 更新", &format!("正在下载 v{latest} ({zip_name})…"));
    shared::progress(app, &format!("正在下载 DASH v{latest} ({zip_name})…"));
    download(&url, &zip_path)?;

    let unzip_dir = tmp.join("unzip");
    std::fs::create_dir_all(&unzip_dir).map_err(|e| e.to_string())?;
    shared::progress(app, "正在解压…");
    extract_zip(&zip_path, &unzip_dir)?;

    let new_app = unzip_dir.join("DASH.app");
    let ok_bin = new_app.join("Contents/MacOS/dashahan").is_file()
        || new_app.join("Contents/MacOS/DASH").is_file();
    if !ok_bin {
        return Err("下载包结构异常 (缺少可执行文件)".to_string());
    }

    // 定位当前 app bundle (可执行文件向上三级: Contents/MacOS/<bin> -> Contents -> .app)
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let cur_app = exe
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .ok_or("无法定位当前 app")?;

    // 备份当前 app 到 ~/DASH-backup/
    let home = std::env::var("HOME").map_err(|_| "HOME 不可用".to_string())?;
    let bak = Path::new(&home)
        .join("DASH-backup")
        .join(format!("DASH-app-{local}"));
    let _ = std::fs::remove_dir_all(&bak);
    std::fs::create_dir_all(bak.parent().ok_or("路径异常")?).map_err(|e| e.to_string())?;
    let st = Command::new("cp")
        .arg("-R")
        .arg(cur_app)
        .arg(&bak)
        .status()
        .map_err(|e| format!("cp 启动失败: {e}"))?;
    if !st.success() {
        return Err(format!("备份当前 app 失败 (cp 退出码 {:?})", st.code()));
    }

    // 辅助安装脚本: 等主进程退出 -> 替换 -> 清隔离 -> 重开;
    // 验证新 app 进程确实起来 (最多 15s), 失败恢复备份 + 系统通知.
    let script_path = tmp.join("install.sh");
    let script = r#"#!/bin/bash
sleep 3
APP="$1"; NEW="$2"; BAK="$3"
notify_fail() {
  osascript -e 'display notification "DASH 更新失败, 已自动恢复原版本" with title "DASH"' >/dev/null 2>&1
}
restore() {
  rm -rf "$APP"
  [ -d "$BAK" ] && cp -R "$BAK" "$APP"
  open "$APP" 2>/dev/null
  notify_fail
}
if [ -d "$NEW" ]; then
  rm -rf "$APP"
  cp -R "$NEW" "$APP"
  xattr -dr com.apple.quarantine "$APP" 2>/dev/null
  open "$APP"
  ok=0
  for i in $(seq 1 15); do
    if pgrep -f "$APP/Contents/MacOS/" >/dev/null 2>&1; then ok=1; break; fi
    sleep 1
  done
  [ "$ok" = "1" ] || restore
else
  restore
fi
"#;
    std::fs::write(&script_path, script).map_err(|e| e.to_string())?;
    let _ = Command::new("chmod").arg("+x").arg(&script_path).status();

    shared::notify("DASH 更新", &format!("v{latest} 已就绪, 正在重启…"));
    shared::progress(app, &format!("安装完成, 正在重启 DASH (v{latest})…"));
    let _ = Command::new(&script_path)
        .arg(cur_app)
        .arg(&new_app)
        .arg(&bak)
        .spawn();
    app.exit(0);
    Ok(format!("DASH 已更新到 v{latest}, 正在重启"))
}

fn download(url: &str, dest: &Path) -> Result<(), String> {
    let st = Command::new("curl")
        .args(["-fL", "-m", "300", "-o"])
        .arg(dest)
        .arg(url)
        .status()
        .map_err(|e| format!("curl 启动失败: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("下载失败 (curl 退出码 {:?})", st.code()))
    }
}

fn extract_zip(zip: &Path, dest: &Path) -> Result<(), String> {
    let st = Command::new("ditto")
        .args(["-x", "-k"])
        .arg(zip)
        .arg(dest)
        .status()
        .map_err(|e| format!("ditto 启动失败: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("解压失败 (ditto 退出码 {:?})", st.code()))
    }
}
