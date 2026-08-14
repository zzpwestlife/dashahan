# 设计：对话完成 / 权限请求系统通知

## 概述
dsh web 是远程页（WKWebView 加载 `http://127.0.0.1:<port>`，无 IPC），用户把窗口隐藏到托盘后，
对话完成、工具要授权、agent 提问等关键事件用户无从感知。本功能让壳层在 Rust 侧直连 dsh 原生事件流
（`/api/events.mux`，WebSocket），识别三类事件并发送 macOS 系统通知（osascript `display notification`）。

价值：后台运行时用户不再错过「对话完成了」「需要你授权」「需要你回答」三类关键节点。

## 范围

### 构建
- 新增 `src-tauri/src/notify.rs`：零依赖 RFC 6455 最小 WS 客户端 + 事件分类 + 通知递送
- `AppState` 增加：`notify_enabled: AtomicBool`、`last_notify: Mutex<Instant>`
- `Config` 增加：`notify_enabled: bool`（`#[serde(default)]`，默认 true）
- `menu.rs`：大傻憨子菜单加 CheckMenuItem「对话完成通知」（勾选状态 ↔ config）
- `main.rs`：setup 中 `start_notify_listener()` 常驻线程

### 不构建
- 点击通知聚焦/打开窗口（后续版本）
- 通知历史/分组/会话级过滤（全会话广播，不做 session 过滤）
- 「正在生成」忙碌状态跟踪（终态事件出现即算完成）
- 补发断连期间错过的旧事件
- 窗口内嵌通知中心（仅系统通知）

## 架构
常驻线程模式，与现有 `start_heal_watchdog` 并列：
```
setup()
 ├─ start_heal_watchdog(app)      # 已有：自愈探针
 └─ start_notify_listener(app)    # 新增：事件流监听
```
线程内主循环：`connect → read 帧 → classify → maybe_notify → 断连 sleep 5s 重连`。
无 async 运行时，全部 std 线程 + TcpStream。端口从 `AppState.dsh_url` 解析（server.rs 已存）。

## 组件

### notify.rs
| 组件 | 职责 |
|---|---|
| `connect(addr) -> io::Result<TcpStream>` | HTTP Upgrade 握手：`Connection: Upgrade` / `Upgrade: websocket` / `Sec-WebSocket-Version: 13` / 随机 Key；校验响应 `101` |
| `read_text_frame(s) -> io::Result<Option<String>>` | 帧解析：FIN/opcode/长度（7/16/64bit）；1=文本→返回负载；8=Close→None（触发重连）；9=Ping→回 Pong；0=Continuation→丢弃（**实现偏差**：分片消息整条丢弃，事件流为小 JSON 帧分片罕见，仅损失一条事件，换取无状态实现；spec 原定拼接，因需要跨调用缓冲状态而简化） |
| `classify(raw: &str) -> Option<NotifyKind>` | serde_json 反序列化 `{method, payload}`，按 `payload.type` 匹配 |
| `maybe_notify(app, kind)` | 判定（开关 && 窗口非前台 && 距上次 ≥10s）→ osascript |
| `start_listener(app)` | 主循环 + 断连重连 + 日志（`logs/notify.log`） |

### NotifyKind
```rust
enum NotifyKind {
    Done { failed: bool, truncated: bool },
    Approval { tool: String },
    Question,
}
```

### 分类规则（基于锁定 dsh 0.1.0-rc.6 实测协议）
| 帧 `payload.type` | 条件 | NotifyKind | 通知文案 |
|---|---|---|---|
| `session/event` | `event.type == "message.stopped"` | `Done{failed:false,truncated:false}` | 对话已完成 |
| `session/event` | `event.type == "message.turnError"` | `Done{failed:true,..}` | 本轮运行失败 |
| `session/event` | `event.type == "message.maxTokens"` | `Done{truncated:true,..}` | 回答被截断（已达输出上限） |
| `approval/requested` | — | `Approval{tool}` | 需要你授权：{tool} |
| `question/requested` | — | `Question` | 需要你回答 |

其余帧类型（`session/subscribed`、`session/jobs`、`approval/resolved` 等）直接丢弃。

## 数据流
```
dsh web(:port) ──WS──▶ notify 线程
                          │ read_text_frame → classify → NotifyKind
                          │ maybe_notify（notify_enabled && 窗口非前台 && 防抖≥10s）
                          ▼
              osascript display notification "DASH" ──▶ 通知中心
```
单向，无循环依赖。线程只读 `AppState`（`dsh_url`、`notify_enabled`、`last_notify`）+ 写日志。

## 错误处理
| 场景 | 行为 |
|---|---|
| 连接失败 / 断连 / Close 帧 | sleep 5s 无限重连，不 panic，不影响壳层其余功能 |
| 帧解析失败（垃圾/协议漂移） | 丢弃该帧，写日志计数，继续 |
| osascript 失败 / 用户拒绝通知权限 | 静默忽略 + 日志（macOS 自动处理权限弹窗） |
| 窗口前台（is_focused） | 不发通知（用户在看，不需要） |
| 开关关闭 | 不发通知，监听照常（省得重连） |

## 测试策略
- **happy path**：构造 5 类帧 JSON → `classify` 产出正确 `NotifyKind`；`maybe_notify` 在「开关开 + 非前台 + 过防抖」时调用 osascript
- **error path**：垃圾字符串 / 空负载 / 断流 → 不 panic，主循环继续
- **edge case**：同 seq 重复帧 → 10s 防抖不重复；开关关 → 不发；前台 → 不发；双会话高频事件 → 通知频率 ≤ 1/10s
- **集成**：真连本地 dsh（若在跑）验证收到帧；手动触发一次授权/问答场景验证通知内容

## 关键决策
1. **WS 客户端零依赖手写**（vs tokio-tungstenite）：需求只消费无掩码文本帧（RFC 6455 最简子集），实测帧结构已知；项目全 std 线程风格；回滚零成本。风险（分片/心跳/64bit 长度）已在组件设计中覆盖。
2. **Rust 侧连事件流**（vs 页面 JS 上报）：远程页无 IPC（自愈探针教训），approval/question 是事件流帧非 DOM 状态，DOM 探测不可靠。
3. **osascript 递送**（vs tauri-plugin-notification）：零新依赖，与 API Key 弹窗同通道，实测可用；归因异常时切换成本低。
4. **默认「仅非前台时通知」+ 菜单开关**：用户确认；开关持久化 config.json。
5. **10s 防抖 + seq 去重**：全会话广播下防刷屏的简单有效手段，不做忙碌状态跟踪。

## 未知项（显式延期）
- 点击通知聚焦窗口 — 需要 notification click handler，本期不做（负责人：下期）。
- 通知归因显示（是否显示为「DASH」）— 取决于 macOS 对 osascript 调用的归因策略，实装后验证，异常则换 tauri-plugin-notification（负责人：本期验证项）。
- dsh 升级后事件 schema 漂移 — 跟随既有升级策略回归（升级 dsh 时验证 notify 仍工作）。
