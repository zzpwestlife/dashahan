// INPUT: installed dsh package, free local port
// OUTPUT: running `dsh web` child + URL to navigate to
// POS: src-tauri/src/server.rs
use crate::shared::{self};
use crate::state::AppState;
use std::fs::OpenOptions;
use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

fn tail_log(path: &std::path::Path) -> String {
    let mut buf = String::new();
    if let Ok(mut f) = std::fs::File::open(path) {
        let _ = f.read_to_string(&mut buf);
    }
    buf.lines().rev().take(8).collect::<Vec<_>>().join("\n")
}

pub fn launch(app: &AppHandle) -> Result<String, String> {
    let port = free_port()?;
    let data = shared::data_dir(app);
    let log_path = data.join("logs/dsh.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| e.to_string())?;
    let log_err = log.try_clone().map_err(|e| e.to_string())?;

    let node = shared::find_node()
        .ok_or("未检测到 Node.js. 请先安装 Node.js (https://nodejs.org) 后再试.")?;
    let mut cmd = Command::new(node);
    cmd.arg(shared::dsh_bin(app))
        .args(["web", "--host", "127.0.0.1", "--port", &port.to_string()])
        .env("PATH", shared::path_env(app))
        .env("DSH_HOME", data.join("dsh-home"))
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    let cfg = shared::read_config(app);
    if let Some(key) = cfg.api_key.as_deref().filter(|k| !k.is_empty()) {
        cmd.env("DEEPSEEK_API_KEY", key);
    }

    let mut child = cmd.spawn().map_err(|e| format!("dsh 启动失败: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(45);
    let addr = format!("127.0.0.1:{port}");
    loop {
        if TcpStream::connect(&addr).is_ok() {
            app.state::<AppState>()
                .child
                .lock()
                .map_err(|_| "state lock poisoned")?
                .replace(child);
            return Ok(format!("http://{addr}/"));
        }
        if let Ok(Some(code)) = child.try_wait() {
            return Err(format!("dsh 提前退出 ({code}).\n{}", tail_log(&log_path)));
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(format!("等待服务超时.\n{}", tail_log(&log_path)));
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}
