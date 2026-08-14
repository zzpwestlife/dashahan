# CONTEXT.md — 领域术语表

> 纯术语表，不含实现细节。新增术语确定即追加。

## dsh 事件流（event stream）
- **事件流**：dsh web 暴露的实时事件通道，路径 `/api/events.mux`（WebSocket，连接后服务端直接推送 JSON 帧，无需订阅握手）。壳层在 Rust 侧连接并消费。
- **会话事件（session event）**：`session/event` 帧内的 `event.type`，描述单条消息/单轮的生命周期，带 `seq` 序号可去重。
- **对话完成（conversation done）**：会话终态事件。包含三种：
  - `message.stopped` — 正常结束（含用户主动停止）
  - `message.turnError` — 本轮运行失败
  - `message.maxTokens` — 输出达到 token 上限被截断
- **授权请求（approval request）**：`approval/requested` 帧，agent 调用工具前需要用户授权（如执行命令/读写文件）。
- **提问请求（question request）**：`question/requested` 帧，agent 需要用户回答问题/选择。
- **通知时机（notification timing）**：默认「仅非前台时通知」——窗口隐藏/最小化/失焦时发系统通知，窗口可见且有焦点时不发；用户可在菜单开关调整。

## 版本锚点
- 上述事件/帧类型均基于锁定版本 `DSH_VERSION = 0.1.0-rc.6` 验证；升级 dsh 后需回归。
