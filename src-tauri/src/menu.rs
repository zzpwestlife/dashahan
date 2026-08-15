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
const ID_RESET: &str = "reset-conversations";
/// 退出: 自定义菜单项 (不走 PredefinedMenuItem::quit 的 macOS terminate 流程,
/// 因为该流程下 prevent_exit 在 macOS 上不可靠 —— 应用会在 Rust 事件循环拦截前就终止。
/// 改为点击即弹确认框, 确认后才真正 exit(0)。
pub const ID_QUIT: &str = "quit-app";

pub fn install(app: &App) -> tauri::Result<()> {
    // 日常项
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
    let reset = MenuItem::with_id(app, ID_RESET, "清空对话历史", true, None::<&str>)?;
    // ⌘Q 绑定到本自定义项: macOS 下 tao/muda 用 set_menu 后不再有默认 Quit 项,
    // 故 ⌘Q 唯一命中本项 -> request_quit -> 确认框 (而非走系统 terminate, 无法拦截).
    let quit = MenuItem::with_id(app, ID_QUIT, "退出 大傻憨", true, Some("Cmd+Q"))?;
    // 「检查更新」子菜单: 两条更新线 + 重装 dsh (收起, 避免主菜单拥挤)
    let dsh_update = MenuItem::with_id(app, ID_DSH_UPDATE, "检查 dsh 更新", true, None::<&str>)?;
    let dash_update = MenuItem::with_id(app, ID_DASH_UPDATE, "检查 DASH 更新", true, None::<&str>)?;
    let update = MenuItem::with_id(app, ID_UPDATE, "重装 dsh", true, None::<&str>)?;
    let updates = Submenu::with_items(
        app,
        "检查更新",
        true,
        &[
            &dsh_update,
            &dash_update,
            &PredefinedMenuItem::separator(app)?,
            &update,
        ],
    )?;
    let sub = Submenu::with_items(
        app,
        "大傻憨",
        true,
        &[
            &apikey,
            &notify,
            &PredefinedMenuItem::separator(app)?,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &logs,
            &reset,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
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
        ID_RESET => reset_conversations(app.clone()),
        ID_QUIT => request_quit(app.clone()),
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
/// 反馈走系统通知 (progress/emit 到远程页收不到, 是"点了没反应"的根因).
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
            None => {
                shared::notify("检查 dsh 更新", "正在检查…");
                match update::check_dsh(&app) {
                    Some(v) => v,
                    None => {
                        shared::notify("检查 dsh 更新", "dsh 已是最新版本");
                        return;
                    }
                }
            }
        };
        if shared::confirm(&format!("升级 dsh 到 v{latest}?
(将自动备份当前版本, 失败可回滚)")) {
            // 结果反馈由 upgrade_dsh 内部 notify+alert 覆盖, 此处不重复弹窗
            let _ = update::upgrade_dsh(&app);
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
            None => {
                shared::notify("检查 DASH 更新", "正在检查…");
                match update::check_dash(&app) {
                    Some(v) => v,
                    None => {
                        shared::notify("检查 DASH 更新", "DASH 已是最新版本");
                        return;
                    }
                }
            }
        };
        if shared::confirm(&format!(
            "更新 DASH 到 v{latest}?
(将自动下载替换并重启, 失败自动恢复)"
        )) {
            match update::update_dash(&app) {
                // Ok = 已触发重启脚本, 进程即将退出, 无需再提示
                Ok(_) => {}
                // Err 时 update_dash 内部无弹窗, 这里补一个
                Err(e) => shared::alert("DASH 更新失败", &e),
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

/// 菜单「清空对话历史」: 确认后先备份 dsh-home, 删除 sessions 目录 (含损坏记录),
/// 再重启 dsh 重建干净的会话并刷新窗口. 用于修复 "Failed to load history: corrupt session log"
/// 这类 dsh 会话日志损坏导致的打不开历史问题.
fn reset_conversations(app: AppHandle) {
    std::thread::spawn(move || {
        if !shared::confirm(
            "清空所有对话历史？\n将删除 dsh 全部会话记录（含损坏记录），用于修复 “Failed to load history” 等损坏问题。\n不可撤销，但重启前已自动备份到 ~/DASH-backup。",
        ) {
            return;
        }
        // 先快照当前数据 (含 dsh-home), 失败不阻塞
        if let Err(e) = shared::backup_config(&app) {
            shared::log_line(&app, &format!("reset 前备份失败: {e}"));
        }
        // 停 dsh 再删, 避免文件被占用 (flow 内部也会 kill_child, 双杀无害)
        app.state::<AppState>().kill_child();
        let sess = shared::data_dir(&app).join("dsh-home/sessions");
        let _ = std::fs::remove_dir_all(&sess);
        // 重启 dsh: 重建干净会话目录并刷新窗口
        bootstrap::start(app);
        shared::notify("对话历史已清空", "dsh 已重启，损坏记录已移除。");
    });
}

/// 退出二次确认: 自定义菜单项触发 (不走 PredefinedMenuItem::quit 的 macOS terminate 流程,
/// 因为该流程下 prevent_exit 在 macOS 上不可靠 —— 应用会在 Rust 事件循环拦截前就终止)。
/// 点击/⌘Q 均命中本函数: 弹确认框, 确认后才真正 exit(0)。
/// (Dock 右键 Quit 仍走系统 terminate, 需 objc2 applicationShouldTerminate 才能拦截, 暂未做。)
///
/// 去重: ⌘Q / 菜单项 / 托盘项/ 系统 terminate 兜底 都可能进入 request_quit;
/// 用 AtomicBool 闸门保证同一时间只有一张确认框, 避免"点一次确定还要再点一次"。
pub fn request_quit(app: AppHandle) {
    let st = app.state::<AppState>();
    if st
        .quit_dialog_pending
        .compare_exchange(false, true, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst)
        .is_err()
    {
        // 已有确认框在屏 (或正在退出), 忽略本次触发
        return;
    }
    std::thread::spawn(move || {
        let confirmed = shared::confirm("确认退出 DASH？\n退出后本地 dsh 服务会停止，后台对话将关闭。");
        if confirmed {
            // 确认: 不重置 flag, 让任何 straggler 事件也被门挡掉, app.exit 后进程随即结束
            app.exit(0);
        } else {
            // 取消: 重置, 让用户后续再次点退出能重新弹框
            app.state::<AppState>()
                .quit_dialog_pending
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
}
