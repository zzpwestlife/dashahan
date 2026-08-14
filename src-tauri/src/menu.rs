// INPUT: user clicks native menu
// OUTPUT: update / api-key / open-logs actions
// POS: src-tauri/src/menu.rs
use crate::bootstrap;
use crate::shared;
use crate::state::AppState;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{App, AppHandle, Manager};

const ID_UPDATE: &str = "update-dsh";
const ID_APIKEY: &str = "set-api-key";
const ID_LOGS: &str = "open-logs";

pub fn install(app: &App) -> tauri::Result<()> {
    let update = MenuItem::with_id(app, ID_UPDATE, "重装 dsh", true, None::<&str>)?;
    let apikey = MenuItem::with_id(app, ID_APIKEY, "设置 API Key", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, ID_LOGS, "打开日志目录", true, None::<&str>)?;
    let quit = PredefinedMenuItem::quit(app, Some("退出 大傻憨"))?;
    let sub = Submenu::with_items(app, "大傻憨", true, &[&update, &apikey, &logs, &quit])?;
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
        ID_APIKEY => prompt_and_set_key(app.clone()),
        ID_LOGS => open_logs(app),
        _ => {}
    });
    Ok(())
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
