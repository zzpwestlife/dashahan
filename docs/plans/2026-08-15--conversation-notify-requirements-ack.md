# 需求确认书：对话完成 / 权限请求系统通知

日期：2026-08-15 ｜ 状态：已确认 ｜ Spec：`docs/specs/2026-08-15--conversation-notify.md`

## Agent 理解版功能描述
壳层在 Rust 侧连接 dsh 原生事件流（`/api/events.mux` WebSocket），识别三类事件并发送 macOS 系统通知：
1. **对话完成**：正常完成（`message.stopped`）/ 失败（`message.turnError`）/ 截断（`message.maxTokens`），文案区分
2. **工具要授权**（`approval/requested`）：通知「需要你授权：{tool}」
3. **agent 提问**（`question/requested`）：通知「需要你回答」

菜单提供「对话完成通知」勾选开关（持久化 config.json），默认开启且**仅窗口非前台时通知**。

## 边界条件（用户确认）
- 通知时机：默认仅非前台（隐藏/最小化/失焦），用户可在菜单开关调整；前台不打扰
- 完成判定：三种终态都通知（正常/失败/截断），文案区分
- 权限范围：授权 + 提问都通知
- 事件流为全会话广播，不做 session 过滤
- 通知只发一条摘要，不含完整对话内容
- 点击通知不聚焦窗口（本期不做）
- 断连自动重连（5s），不补发错过的旧事件

## 已确认假设
1. 终态事件出现即算完成，不跟踪忙碌状态；防刷屏靠 seq 去重 + 10s 时间防抖
2. 通知递送用 osascript `display notification`（零新依赖，复用 API Key 弹窗同款通道）；归因异常则切换 tauri-plugin-notification
3. macOS 首次弹通知权限询问；用户拒绝则静默降级
4. 事件/帧类型基于锁定 dsh 0.1.0-rc.6 验证；升级 dsh 需回归

## Out-of-scope
点击通知聚焦窗口 ｜ 通知历史/分组 ｜ 会话级过滤 ｜ 忙碌状态跟踪 ｜ 补发旧事件 ｜ 窗口内嵌通知中心
