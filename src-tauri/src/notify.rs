// INPUT: dsh 事件流 (ws://127.0.0.1:<port>/api/events.mux, 连上即推 JSON 帧)
// OUTPUT: 对话完成 / 要授权 / 要回答 时发 macOS 系统通知
// POS: src-tauri/src/notify.rs
// 设计: docs/specs/2026-08-15--conversation-notify.md
use crate::shared;
use crate::state::AppState;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// 事件流 WS 路径 (与 dsh-client-connection 的 MUX_EVENTS_PATH 一致)
const MUX_PATH: &str = "/api/events.mux";
/// 单帧大小上限 (防呆; 正常事件帧远小于此, session/jobs 大帧也不超过几百 KB)
const MAX_FRAME: u64 = 4 * 1024 * 1024;
/// 通知防抖: 同一窗口期内最多发一条, 防全会话广播刷屏
const DEBOUNCE: Duration = Duration::from_secs(10);
/// 断连 / dsh 未就绪时的重试间隔
const RETRY: Duration = Duration::from_secs(5);

#[derive(Debug, PartialEq)]
pub enum NotifyKind {
    /// 对话完成 (message.stopped 正常 / turnError 失败 / maxTokens 截断)
    Done { failed: bool, truncated: bool },
    /// 工具要授权 (approval/requested)
    Approval { tool: String },
    /// agent 要回答 (question/requested)
    Question,
}

/// 从 dsh web URL 提取 "host:port" (WS 握手目标)
fn ws_target(dsh_url: &str) -> Option<String> {
    let rest = dsh_url.strip_prefix("http://")?;
    let host_port = rest.split('/').next().filter(|s| !s.is_empty())?;
    Some(host_port.to_string())
}

/// 事件帧分类: 只认三种我们关心的帧, 其余 (session/subscribed, session/jobs,
/// approval/resolved, question/resolved 等) 返回 None 丢弃.
pub fn classify(raw: &str) -> Option<NotifyKind> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let payload = v.get("payload")?;
    match payload.get("type")?.as_str()? {
        "session/event" => {
            let et = payload.pointer("/event/type")?.as_str()?;
            match et {
                "message.stopped" => Some(NotifyKind::Done { failed: false, truncated: false }),
                "message.turnError" => Some(NotifyKind::Done { failed: true, truncated: false }),
                "message.maxTokens" => Some(NotifyKind::Done { failed: false, truncated: true }),
                _ => None,
            }
        }
        "approval/requested" => Some(NotifyKind::Approval {
            tool: payload
                .get("toolName")
                .and_then(|t| t.as_str())
                .unwrap_or("工具")
                .to_string(),
        }),
        "question/requested" => Some(NotifyKind::Question),
        _ => None,
    }
}

/// 主入口: 常驻线程, 等 dsh_url 就绪后连接事件流, 断连 5s 重连.
pub fn start_listener(app: AppHandle) {
    std::thread::spawn(move || loop {
        let target = app
            .state::<AppState>()
            .dsh_url
            .lock()
            .ok()
            .and_then(|u| u.clone())
            .and_then(|u| ws_target(&u));
        if let Some(target) = target {
            if let Err(e) = run_stream(&app, &target) {
                log(&app, &format!("stream closed: {e}"));
            }
        }
        std::thread::sleep(RETRY);
    });
}

/// 单次连接的生命周期: 握手 → 读帧循环 → 断连/Close 返回 Err/Ok
fn run_stream(app: &AppHandle, target: &str) -> io::Result<()> {
    let mut stream = connect(target)?;
    loop {
        match read_text_frame(&mut stream)? {
            Some(text) => {
                if let Some(kind) = classify(&text) {
                    maybe_notify(app, &kind);
                }
            }
            None => return Ok(()), // Close 帧: 正常结束, 上层重连
        }
    }
}

/// WebSocket 握手: 发送 Upgrade 请求, 校验 101 响应.
fn connect(target: &str) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(target)?;
    // 服务器定期 Ping 保活, 读超时兜底防挂死
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    // 固定随机种子即可: 服务端只校验 Sec-WebSocket-Accept (由 key+GUID 算出), 不校验 key 随机性
    let key = "dGhlIHNhbXBsZSBub25jZQ=="; // RFC 6455 示例 key
    let req = format!(
        "GET {MUX_PATH} HTTP/1.1\r\nHost: {target}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(req.as_bytes())?;

    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte)?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof during handshake"));
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
        if head.len() > 8192 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "handshake too large"));
        }
    }
    let head = String::from_utf8_lossy(&head);
    if !head.starts_with("HTTP/1.1 101") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("handshake rejected: {}", head.lines().next().unwrap_or("")),
        ));
    }
    Ok(stream)
}

/// 读取下一帧文本消息. 只返回完整文本帧 (FIN=1, opcode=1).
/// Ping → 回 Pong 继续; Close → None; 分片/二进制/未知 → 丢弃继续 (事件流为小 JSON 帧, 分片罕见).
fn read_text_frame(stream: &mut TcpStream) -> io::Result<Option<String>> {
    loop {
        let mut hdr = [0u8; 2];
        read_exact(stream, &mut hdr)?;
        let fin = hdr[0] & 0x80 != 0;
        let opcode = hdr[0] & 0x0F;
        let masked = hdr[1] & 0x80 != 0;
        let mut len = (hdr[1] & 0x7F) as u64;
        if len == 126 {
            let mut b = [0u8; 2];
            read_exact(stream, &mut b)?;
            len = u16::from_be_bytes(b) as u64;
        } else if len == 127 {
            let mut b = [0u8; 8];
            read_exact(stream, &mut b)?;
            len = u64::from_be_bytes(b);
        }
        let mut mask = [0u8; 4];
        if masked {
            read_exact(stream, &mut mask)?;
        }
        if len > MAX_FRAME {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "frame too large"));
        }
        let mut payload = vec![0u8; len as usize];
        read_exact(stream, &mut payload)?;
        if masked {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        match opcode {
            0x1 if fin => return Ok(Some(String::from_utf8_lossy(&payload).into_owned())),
            0x9 => write_pong(stream, &payload)?, // Ping → Pong, 继续读
            0x8 => return Ok(None),               // Close
            _ => continue,                        // 分片/二进制/未知: 丢弃
        }
    }
}

fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) -> io::Result<()> {
    let mut read = 0;
    while read < buf.len() {
        let n = stream.read(&mut buf[read..])?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
        }
        read += n;
    }
    Ok(())
}

/// 回 Pong 帧 (echo 原 payload)
fn write_pong(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 2);
    frame.push(0x8A); // FIN + opcode 0xA (pong)
    if payload.len() < 126 {
        frame.push(payload.len() as u8);
    } else {
        frame.push(126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    frame.extend_from_slice(payload);
    stream.write_all(&frame)
}

/// 判定 + 递送: 开关开 && 窗口非前台 && 距上次 ≥10s → osascript 系统通知
fn maybe_notify(app: &AppHandle, kind: &NotifyKind) {
    let st = app.state::<AppState>();
    if !st.notify_enabled.load(Ordering::SeqCst) {
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        if w.is_focused().unwrap_or(false) {
            return;
        }
    }
    {
        let guard = match st.last_notify.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if let Some(last) = guard.as_ref() {
            if last.elapsed() < DEBOUNCE {
                return;
            }
        }
    }
    if let Ok(mut last) = st.last_notify.lock() {
        *last = Some(Instant::now());
    }

    let msg = match kind {
        NotifyKind::Done { failed: true, .. } => "本轮运行失败".to_string(),
        NotifyKind::Done { truncated: true, .. } => "回答被截断（已达输出上限）".to_string(),
        NotifyKind::Done { .. } => "对话已完成".to_string(),
        NotifyKind::Approval { tool } => format!("需要你授权：{tool}"),
        NotifyKind::Question => "需要你回答".to_string(),
    };
    let script = format!(
        "display notification \"{}\" with title \"DASH\"",
        msg.replace('"', "\\\"")
    );
    match std::process::Command::new("osascript").arg("-e").arg(script).output() {
        Ok(_) => log(app, &format!("notified: {msg}")),
        Err(e) => log(app, &format!("notify failed: {e}")),
    }
}

/// 追加日志到 logs/notify.log
fn log(app: &AppHandle, msg: &str) {
    use std::io::Write;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(shared::data_dir(app).join("logs/notify.log"))
    {
        let _ = writeln!(f, "[{ts}] {msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(payload_type: &str, extra: &str) -> String {
        format!(
            r#"{{"type":"server-request","rpcId":"r","method":"{payload_type}","payload":{{"type":"{payload_type}","sessionId":"s1"{extra}}}}}"#
        )
    }

    #[test]
    fn classify_done_normal() {
        let raw = frame("session/event", r#","event":{"type":"message.stopped","seq":1,"time":1,"data":{}}"#);
        assert_eq!(
            classify(&raw),
            Some(NotifyKind::Done { failed: false, truncated: false })
        );
    }

    #[test]
    fn classify_done_failed() {
        let raw = frame("session/event", r#","event":{"type":"message.turnError","seq":2,"time":1,"data":{}}"#);
        assert_eq!(
            classify(&raw),
            Some(NotifyKind::Done { failed: true, truncated: false })
        );
    }

    #[test]
    fn classify_done_truncated() {
        let raw = frame("session/event", r#","event":{"type":"message.maxTokens","seq":3,"time":1,"data":{}}"#);
        assert_eq!(
            classify(&raw),
            Some(NotifyKind::Done { failed: false, truncated: true })
        );
    }

    #[test]
    fn classify_approval() {
        let raw = frame(
            "approval/requested",
            r#","approvalId":"a1","toolName":"bash","reason":"run cmd""#,
        );
        assert_eq!(classify(&raw), Some(NotifyKind::Approval { tool: "bash".into() }));
    }

    #[test]
    fn classify_approval_missing_tool() {
        let raw = frame("approval/requested", r#","approvalId":"a1""#);
        assert_eq!(classify(&raw), Some(NotifyKind::Approval { tool: "工具".into() }));
    }

    #[test]
    fn classify_question() {
        let raw = frame("question/requested", r#","questions":[{"label":"y?"}]"#);
        assert_eq!(classify(&raw), Some(NotifyKind::Question));
    }

    #[test]
    fn classify_ignores_other_frames() {
        // session/subscribed / session/jobs / resolved 帧全部忽略
        for raw in [
            frame("session/subscribed", r#","lastSeq":3"#),
            frame("session/jobs", r#","jobs":[]"#),
            frame("approval/resolved", r#","approvalId":"a1","outcome":"allowed-once""#),
            frame("question/resolved", r#","questionRpcId":"q1","outcome":"answered""#),
        ] {
            assert_eq!(classify(&raw), None, "should ignore: {raw}");
        }
    }

    #[test]
    fn classify_garbage_no_panic() {
        for raw in ["", "not json", "{\"type\":\"server-request\"}", "null", "{"] {
            assert_eq!(classify(raw), None, "should be None for: {raw:?}");
        }
    }

    #[test]
    fn classify_other_event_types_none() {
        // 会话事件里非终态的 (比如普通消息更新) 不通知
        let raw = frame("session/event", r#","event":{"type":"message.update","seq":4,"time":1,"data":{}}"#);
        assert_eq!(classify(&raw), None);
    }

    #[test]
    fn ws_target_parses() {
        assert_eq!(ws_target("http://127.0.0.1:49598/"), Some("127.0.0.1:49598".into()));
        assert_eq!(ws_target("http://127.0.0.1:49598"), Some("127.0.0.1:49598".into()));
        assert_eq!(ws_target("garbage"), None);
        assert_eq!(ws_target(""), None);
    }

    /// 真连本机 dsh 事件流: 验证握手/读帧/分类全链路.
    /// 运行: DASH_TEST_MUX_PORT=<port> cargo test -- --ignored live_connect_reads_frames
    #[test]
    #[ignore = "requires local dsh web running"]
    fn live_connect_reads_frames() {
        let Ok(port) = std::env::var("DASH_TEST_MUX_PORT") else {
            eprintln!("skip: DASH_TEST_MUX_PORT not set");
            return;
        };
        let target = format!("127.0.0.1:{port}");
        let mut stream = connect(&target).expect("ws handshake");
        let mut seen = 0;
        for _ in 0..10 {
            match read_text_frame(&mut stream).expect("read frame") {
                Some(text) => {
                    assert!(text.contains("server-request"), "unexpected frame: {text}");
                    let _ = classify(&text); // 分类器不 panic
                    seen += 1;
                    if seen >= 3 {
                        break;
                    }
                }
                None => break,
            }
        }
        assert!(seen >= 1, "no frames received");
    }
}
