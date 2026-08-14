# DASH 排障手册

本文记录已经踩过的故障、**能证实的部分**、以及对应处理。
写这条规则：没有证据的不写成因；推断单独标出来。

## 1. Failed to load plugins（最高频）

### 你看到什么

窗口白底红字，大意是：

```
Failed to load plugins
web boot: 33 entries did not activate
@deepseek-ai/dsh-client-runtime: pending (waiting for services: connection, ...)
@deepseek-ai/dsh-client-ui-layout: pending (waiting for services: slots, theme)
...
```

一长串插件都是 `pending`，后面列着在等哪个服务（最常见是 `slots`、`connection`、`locale`）。

### 这句话到底是谁抛的

**不是 DASH 壳抛的。** 文案来自 dsh 自己的前端启动器：

- 包：`@deepseek-ai/dsh-client-web`
- 函数：`runPluginBoot()` → `loader.await()` → `assertEntriesActive()`
- 源码注释原文：cordis 的 inject 等待**没有超时**；sweep 是 fail-loud 补偿。某个上游服务一直没挂上，下游就永远 `pending`。sweep 扫到非 `ACTIVE` 就整页抛错。

所以这页的意思只有一句：

> **客户端插件图没 settle。** 不是 npm 装坏了，也不是「33 个插件文件丢失」。

### 分层诊断（先分清到底哪一层坏了）

| 层 | 怎么验 | 正常长什么样 | 本次故障的实测 |
|---|---|---|---|
| 1. 壳进程 | `pgrep -fl Contents/MacOS/dashahan` | 有进程 | 有 |
| 2. dsh web 进程 | `pgrep -fl dsh/lib/bin.js` | 有 `node .../dsh/lib/bin.js web --host 127.0.0.1 --port N` | 有 |
| 3. HTTP | `curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:N/` | `200`，约 12KB | 200 / 12076 bytes |
| 4. 插件资源 | `curl` 首页里的 `/plugins/.../client.js?rev=...` | `200` | 抽查 3 个全 200 |
| 5. WebSocket | `new WebSocket("ws://127.0.0.1:N/api/events.mux")` | `onopen` | 握手成功 |
| 6. WebView 真连上了吗 | `lsof -iTCP:N \| grep ESTABLISHED` | 至少 2 条（`com.apple` ↔ `node`） | 2～6 条都见过 |
| 7. 客户端插件图 | 窗口内容 | dsh 聊天界面 | **卡在 Failed to load plugins** |

**关键结论（已证实）：** 1～6 全绿时，第 7 层仍可能红。  
`HTTP 200` 和 `ESTABLISHED` **不等于** 插件激活成功。以前把第 6 层当成功标准，是误判。

### 已经证实的

1. **故障在客户端插件图，不在服务端。** 上面 1～6 层全部健康时，页面仍报错。
2. **清 WKWebView 浏览数据后立刻恢复。** 2026-08-14 21:54 做过对照：
   - 备份后清空  
     `~/Library/WebKit/com.solo.dashahan/WebsiteData`  
     `~/Library/Caches/com.solo.dashahan/WebKit`
   - 重启后窗口恢复正常。
3. **同一份数据目录（config / dsh-home / node_modules）不动。** 丢的不是 API Key，也不是会话文件。
4. **覆盖安装新 `.app` 清不掉这个状态。** 浏览数据在 app 包外面，换版本装上去缓存还在。所以「下了完整版一打开就报错」看起来像安装包坏了，其实是本机 WebView 带着旧状态进了新包。
5. **`pkill -9` / 反复强杀 dsh 之后更容易再发。** 开发当天多次 SIGKILL 子进程，和两次复现时间对得上。

### 没有证实、只是推断（别当事实引用）

下面这些**说得通，但没抓到单条脏数据当物证**：

- 客户端把 boot / connection 状态写进 WKWebView 的 `localStorage` / `IndexedDB` / Service Worker；进程被 SIGKILL 时写出半截，下次启动按坏快照恢复。
- 上游某个「立即层」插件（`immediately: true`，例如 `dsh-client-runtime` / connection）没真正 `apply` 成功，整棵依赖树卡在 `pending`。
- dsh 自身偶发的 race：boot 扫得太早，服务还没挂上。这个**单独解释不了「清缓存立刻好、换包不好」**，所以最多是加重因素，不是主因。

没做的事：没有把 `WebsiteData` 解包，指出具体是哪一个 origin、哪一张表坏了。所以「缓存里到底哪一条脏」仍然未知。

### 用户怎么修（立刻恢复）

在终端执行（先完全退出 DASH：托盘 → 退出 DASH）：

```sh
pkill -f 'Contents/MacOS/dashahan' 2>/dev/null
pkill -f 'dsh/lib/bin.js' 2>/dev/null
rm -rf ~/Library/WebKit/com.solo.dashahan/WebsiteData \
       ~/Library/Caches/com.solo.dashahan/WebKit
open /Applications/DASH.app
```

这只清 WebView 浏览数据。`~/Library/Application Support/com.solo.dashahan/`（Key、会话、dsh 安装）不动。

图形界面等价操作：Safari / 系统设置里清「网站数据」对 DASH 无效，必须清上面两个目录。

### 开发者怎么确认修没修好

```sh
# 1. 服务端还活着吗
D="$HOME/Library/Application Support/com.solo.dashahan"
cat "$D/logs/dsh.log"        # 应有 dsh web: http://127.0.0.1:PORT
curl -s -o /dev/null -w "%{http_code}\n" "http://127.0.0.1:PORT/"

# 2. WebView 连上了吗（连上 ≠ 插件成功）
lsof -iTCP:PORT | grep ESTABLISHED

# 3. 最终标准：窗口是不是聊天界面
#    不是 HTTP 200，不是 ESTABLISHED 条数
```

### 壳现在做了什么（治标 + 防再写坏）

| 机制 | 文件 | 做什么 | 局限 |
|---|---|---|---|
| 优雅退出 | `src-tauri/src/state.rs` `kill_child()` | 先 `SIGTERM`，等 2 秒，再 `SIGKILL` | 防的是**下次**写坏；治不了已经脏的缓存 |
| Watchdog 自愈 | `src-tauri/src/main.rs` `start_heal_watchdog()` | 每 3 秒注入探针；页面出现 `Failed to load plugins` 就跳到 `/__dash_heal__`；壳看到这个 URL 后 `clear_all_browsing_data()` 并重载。最多 1 次 | 依赖探针 JS 能在失败页跑起来。未做「故意弄脏缓存再看自愈」的端到端红测 |

第一版自愈用 `window.__TAURI__.event.emit`，对 `http://127.0.0.1` 远程页**不可靠**（Tauri 默认不给远程页 IPC）。已改成 URL 信标，不经过 `__TAURI__`。

### 不要做的事

- 不要为这个错重装 dsh（`node_modules` 没坏）。
- 不要为这个错重填 API Key（`config.json` / 钥匙串没坏）。
- 不要只看 `curl` 200 就宣称「好了」。

---

## 2. 覆盖安装后「配置全没了」

### 已证实

- 0.1.0 / 0.1.1 源码里**没有**删除 `Application Support` 的逻辑。
- 数据在包外：`~/Library/Application Support/com.solo.dashahan/`。换 `/Applications/DASH.app` 物理上碰不到它。
- identifier 一直是 `com.solo.dashahan`，没有第二套数据目录。
- 本机对照：目录还在时，key、会话、dsh 安装都在。

### 未证实

另一台电脑上升 0.1.0 → 0.1.1 后用户看到配置没了。现场已被重配覆盖，无法复盘。可能是外部清理、看错空壳页、或一次瞬时插件失败被当成「数据没了」。**没有日志能钉死其中任何一个。**

### 现在的防护

- API Key 双写：`config.json` + macOS 钥匙串。config 丢了启动会从钥匙串写回。
- 每次启动 rsync 到 `~/DASH-backup/<时间戳>/`（排除 `node_modules` / npm-cache / logs，留最近 5 份）。

手动恢复备份：

```sh
ls ~/DASH-backup
# 挑一份
cp ~/DASH-backup/<ts>/config.json \
   ~/Library/Application\ Support/com.solo.dashahan/
# dsh-home 按需整目录拷回
```

---

## 3. 输入框能打字、不能 ⌘V

### 已证实

根因是菜单栏只有「大傻憨」，没有标准「编辑」菜单。macOS 的 ⌘C / ⌘V / ⌘X 走编辑菜单动作，不走按键直达。缺这一栏，WKWebView 里粘贴全废。

当初只改 CSS `user-select`，方向不对。

### 修复

`src-tauri/src/menu.rs` 加「编辑」：撤销 / 重做 / 剪切 / 复制 / 粘贴 / 全选。

---

## 4. 「找到 Node.js 但未找到 npm」

### 已证实

Homebrew 新公式的 `node`（本机见过 `25.9.0`）**不带 npm**。`find_node()` 以前按路径顺序拿第一个 `node`，命中 `/opt/homebrew/bin/node` 后 `npm_cli()` 失败。

### 修复

`find_node()` 优先返回「旁边找得到 `npm-cli.js`」的 node；都没有才回退。完整版再优先 `Contents/Resources/node-dist/bin/node`。

轻量版用户若自己 `brew install node`，还需要 `brew install npm`，或改用 nodejs.org 安装包。

---

## 5. Gatekeeper /「已损坏，无法打开」

未签名、未公证。首次下载后系统打 quarantine。

```sh
xattr -cr /Applications/DASH.app
```

或 Finder 里右键 → 打开。不是包坏了。

---

## 6. 首次启动很慢 / 一直停在「正在安装 dsh」

正常。第一次要把 `@deepseek-ai/dsh` 装进数据目录（大约 2 分钟、需联网）。看进度：

```sh
tail -f ~/Library/Application\ Support/com.solo.dashahan/logs/npm.log
```

装完之后再启动应是：`检查环境 → 启动本地服务`，秒开。

---

## 7. 怎么判断「窗口正常」

唯一标准：**窗口是 dsh 聊天界面，能开会话。**

下面这些都不够：

- `dsh.log` 打出了 `dsh web: http://127.0.0.1:PORT`
- `curl` 返回 200
- `lsof` 有 ESTABLISHED
- 进程还在

以上只说明服务端活着。

---

## 关键路径速查

| 东西 | 路径 |
|---|---|
| App | `/Applications/DASH.app` |
| 数据 / 配置 / dsh / 日志 | `~/Library/Application Support/com.solo.dashahan/` |
| WebView 浏览数据（本手册第 1 节清的就是它） | `~/Library/WebKit/com.solo.dashahan/WebsiteData` |
| WebKit 缓存 | `~/Library/Caches/com.solo.dashahan/WebKit` |
| 配置备份 | `~/DASH-backup/` |
| 启动日志 | `.../logs/boot.log` |
| dsh 日志 | `.../logs/dsh.log` |
| npm 安装日志 | `.../logs/npm.log` |
