// INPUT: dsh web child process handle
// OUTPUT: shared killable state for the app
// POS: src-tauri/src/state.rs
use std::process::Child;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Default)]
pub struct AppState {
    pub child: Mutex<Option<Child>>,
    /// 当前 dsh web 的 URL, 供插件失败自愈时重载
    pub dsh_url: Mutex<Option<String>>,
    /// 插件加载失败自愈次数 (最多 1 次, 防死循环)
    pub heal_count: AtomicU8,
}

impl AppState {
    /// 优雅终止 dsh: 先 SIGTERM 让 WebSocket/事件流正常关闭 (避免客户端把坏状态写进
    /// WKWebView 缓存导致下次启动 "Failed to load plugins"), 2 秒未退再 SIGKILL 兜底.
    pub fn kill_child(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut c) = guard.take() {
                let _ = std::process::Command::new("kill")
                    .arg("-TERM")
                    .arg(c.id().to_string())
                    .status();
                for _ in 0..20 {
                    if let Ok(Some(_)) = c.try_wait() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if c.try_wait().ok().flatten().is_none() {
                    let _ = c.kill();
                }
                let _ = c.wait();
            }
        }
    }

    /// 标记一次自愈; 返回是否允许执行 (false = 已自愈过, 忽略)
    pub fn try_heal(&self) -> bool {
        self.heal_count
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}
