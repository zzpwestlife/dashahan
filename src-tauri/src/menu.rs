// INPUT: user clicks native menu
// OUTPUT: update / api-key / open-logs actions
// POS: src-tauri/src/menu.rs
use crate::bootstrap;
use crate::shared;
use crate::state::AppState;
use crate::update;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{App, AppHandle, Manager};

const ID_UPDATE: &str = "update-dsh";
const ID_DSH_UPDATE: &str = "check-dsh-update";
const ID_DASH_UPDATE: &str = "check-dash-update";
const ID_APIKEY: &str = "set-api-key";
const ID_NOTIFY: &str = "notify-toggle";
const ID_LOGS: &str = "open-logs";

pub fn install(app: &App) -> tauri::Result<()> {
    let update = MenuItem::with_id(app, ID_UPDATE, "重装 dsh", true, None::<&str>)?;
    let dsh_update = MenuItem::with_id(app, ID_DSH_UPDATE, "检查 dsh 更新", true, None::<&str>)?;
    let dash_update = MenuItem::with_id(app, ID_DASH_UPDATE, "检查 DASH 更新", true, None::<&str>)?;
    let apikey = MenuItem::with_id(app, ID_APIKEY, "设置 API Key", true, None::<&str>)?;
    let notify_checked = shared::read_config(app.handle()).notify_enabled;
    let notify = tauri::menu::CheckMenuItem::with_id(
        app,
        ID_NOTIFY,
        "对话完成通知",
        true,
        notify_checked,
        None::<&str>,
    )?;
    let logs = MenuItem::with_id(app, ID_LOGS, "打开日志目录", true, None::<&str>)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出 大傻憨"))?;
    let sub = Submenu::with_items(
        app,
        "大傻憨",
        true,
        &[&update, &dsh_update, &dash_update, &apikey, &notify, &logs, &quit],
    )?;
    // 标准「编辑」菜单: 注册 ⌘C/⌘V/⌘X 等快捷键到响应链, 否则 WebView 输入框无法复制/粘贴
    let edit = Submenu::with_items(
        app,
        "编辑",
        true,
        &[
            &PredefinedMenuItem::undo(app, Some("撤销"))?,
            &PredefinedMenuItem::redo(app, Some("重做"))?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("剪切"))?,
            &PredefinedMenuItem::copy(app, Some("复制"))?,
            &PredefinedMenuItem::paste(app, Some("粘贴"))?,
            &PredefinedMenuItem::select_all(app, Some("全选"))?,
        ],
    )?;
    let menu = Menu::with_items(app, &[&sub, &edit])?;
    app.set_menu(menu)?;

    app.on_menu_event(|app: &AppHandle, event| match event.id().as_ref() {
        ID_UPDATE => bootstrap::install_then_start(app.clone()),
        ID_DSH_UPDATE => handle_dsh_update(app.clone()),
        ID_DASH_UPDATE => handle_dash_update(app.clone()),
        ID_APIKEY => prompt_and_set_key(app.clone()),
        ID_NOTIFY => toggle_notify(app.clone()),
        ID_LOGS => open_logs(app),
        _ => {}
    });
    Ok(())
}

/// 菜单「对话完成通知」勾选切换: 更新运行时开关 + 持久化 config.json + 同步勾选态.
fn toggle_notify(app: AppHandle) {
    let st = app.state::<AppState>();
    let enabled = !st.notify_enabled.load(std::sync::atomic::Ordering::SeqCst);
    st.notify_enabled.store(enabled, std::sync::atomic::Ordering::SeqCst);
    let mut cfg = shared::read_config(&app);
    cfg.notify_enabled = enabled;
    let _ = shared::write_config(&app, &cfg);
    if let Some(menu) = app.menu() {
        if let Some(tauri::menu::MenuItemKind::Check(item)) = menu.get(ID_NOTIFY) {
            let _ = item.set_checked(enabled);
        }
    }
}

/// 刷新两个更新菜单项文案: 发现新版显示"升级/更新到 vX", 否则"检查…".
pub fn refresh_update_items(app: &AppHandle) {
    let dsh_text = match app
        .state::<AppState>()
        .dsh_latest
        .lock()
        .ok()
        .and_then(|g| g.clone())
    {
        Some(v) => format!("升级 dsh 到 v{v}"),
        None => "检查 dsh 更新".to_string(),
    };
    let dash_text = match app
        .state::<AppState>()
        .dash_latest
        .lock()
        .ok()
        .and_then(|g| g.clone())
    {
        Some(v) => format!("更新 DASH 到 v{v}"),
        None => "检查 DASH 更新".to_string(),
    };
    if let Some(menu) = app.menu() {
        if let Some(tauri::menu::MenuItemKind::MenuItem(item)) = menu.get(ID_DSH_UPDATE) {
            let _ = item.set_text(dsh_text);
        }
        if let Some(tauri::menu::MenuItemKind::MenuItem(item)) = menu.get(ID_DASH_UPDATE) {
            let _ = item.set_text(dash_text);
        }
    }
}

/// 菜单「检查/升级 dsh」: 有缓存新版直接确认升级, 否则现查.
fn handle_dsh_update(app: AppHandle) {
    std::thread::spawn(move || {
        let latest = match app
            .state::<AppState>()
            .dsh_latest
            .lock()
            .ok()
            .and_then(|g| g.clone())
        {
            Some(v) => v,
            None => match update::check_dsh(&app) {
                Some(v) => v,
                None => {
                    shared::progress(&app, "dsh 已是最新版本");
                    return;
                }
            },
        };
        if shared::confirm(&format!("升级 dsh 到 v{latest}?
(将自动备份当前版本, 失败可回滚)")) {
            match update::upgrade_dsh(&app) {
                Ok(msg) => shared::progress(&app, &msg),
                Err(e) => shared::boot_error(&app, &e),
            }
        }
    });
}

/// 菜单「检查/更新 DASH」: 有缓存新版直接确认更新, 否则现查.
fn handle_dash_update(app: AppHandle) {
    std::thread::spawn(move || {
        let latest = match app
            .state::<AppState>()
            .dash_latest
            .lock()
            .ok()
            .and_then(|g| g.clone())
        {
            Some(v) => v,
            None => match update::check_dash(&app) {
                Some(v) => v,
                None => {
                    shared::progress(&app, "DASH 已是最新版本");
                    return;
                }
            },
        };
        if shared::confirm(&format!(
            "更新 DASH 到 v{latest}?
(将自动下载替换并重启, 失败自动恢复)"
        )) {
            match update::update_dash(&app) {
                Ok(msg) => shared::progress(&app, &msg),
                Err(e) => shared::boot_error(&app, &e),
            }
        }
    });
}

/// 菜单「设置 API Key」: 弹出 macOS 原生对话框收集 key, 不阻塞主线程.
fn prompt_and_set_key(app: AppHandle) {
    std::thread::spawn(move || {
        if let Some(key) = shared::prompt_api_key("请输入 DeepSeek API Key（可稍后在 dsh 设置中添加）") {
            shared::save_api_key(&app, &key);
            app.state::<AppState>().kill_child();
            if let Some(w) = app.get_webview_window("main") {
                // 回到启动页, 由 flow 重新驱动
                let _ = w.navigate("tauri://localhost/index.html".parse().expect("boot url"));
            }
            bootstrap::start(app);
        }
    });
}

fn open_logs(app: &AppHandle) {
    let dir = shared::data_dir(app).join("logs");
    let _ = std::process::Command::new("open").arg(dir).spawn();
}
