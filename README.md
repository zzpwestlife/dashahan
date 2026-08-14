# 大傻憨 / DASH

双击即用的 macOS 桌面 app, 包装 [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) 的 `dsh web`. 无需记 `npx` 命令.

Double-click macOS app wrapping deepseek-harness (`dsh web`). No `npx` commands needed.

## 原理 / How it works

- Tauri 2 原生窗口 (系统 WKWebView).
- **使用系统 Node.js** 运行 dsh (自动检测 PATH / Homebrew / nvm / mise), app 本体约 5MB, 不再内嵌运行时.
- 首次启动自动把 `@deepseek-ai/dsh` 安装到 `~/Library/Application Support/com.solo.dashahan/` (约 2 分钟, 仅一次), 之后跳过安装直接启动.
- 随机空闲端口启动 `dsh web`, 窗口直接加载本地页面.
- 单实例: 重复双击只会聚焦已有窗口. 关窗即结束后端进程.

## 要求 / Requirements

- macOS (Apple Silicon, M 系列), **已安装 Node.js ≥ 18 且带 npm** (任一路径均可: PATH、Homebrew、nvm、mise).
- ⚠️ **Homebrew 新版 node 公式已不带 npm**: 用 `brew install node` 装的, 需再执行 `brew install npm`. 官网 nodejs.org 下载的安装包自带 npm, 无此问题.
- 未检测到 Node.js 或缺少 npm 时, app 会提示后重试.

## 安装 / Install

分发方式(二选一):
- **zip 包**: `dist/DASH-macOS.zip`, 解压后拖入 Applications. —— 当前开发环境打不出 DMG, 推荐用此方式.
- **DMG**: 在一台正常 macOS 上执行 `npm run build`, 产物在 `src-tauri/target/release/bundle/dmg/`.

接收方首次打开 (一次性, 约 5 分钟):
1. 未签名提示: Finder 中 **右键 DASH.app → 打开** → 再点"打开" (或终端执行 `xattr -cr /Applications/DASH.app` 清除隔离标记).
2. 首次启动自动下载 dsh (需联网, 约 2 分钟), 之后跳过.
3. 可能弹出 API Key 对话框 — **可选**: 粘贴 DeepSeek API Key 点"保存", 或直接点"取消"都会继续.
4. 之后双击秒开. 未设 key 时, 随时可在 dsh 的设置页或菜单「设置 API Key」中添加.

## 从源码构建 / Build from source

要求: macOS arm64, Rust (`brew install rust`), Node (系统已装).

```sh
scripts/make-icon.sh        # 生成 🐶 图标集 (已有图标可跳过)
npm install
npm run build               # 轻量版 (3.7MB, 用系统 Node, 自用)
./scripts/smoke-test.sh     # 冒烟测试: 启动→dsh→HTTP200→钥匙串
```

完整版 (内嵌 Node, 对外分发, 开箱即用, 约 126MB):

```sh
./scripts/fetch-node-dist.sh   # 下载 Node 到 src-tauri/resources/node-dist
cd src-tauri && npx tauri build --config tauri.full.conf.json
```

## 菜单与托盘 / Menu & Tray

- **关窗不退出**: 关闭窗口只隐藏, dsh 后台保持运行; 点菜单栏 🐶 图标或再次双击可唤回.
- `更新 dsh`: 重新执行 npm install (升级常量版本需改 `src-tauri/src/main.rs` 的 `DSH_VERSION`).
- `设置 API Key`: 弹出 macOS 原生对话框输入 DeepSeek API Key (写入 app config + 钥匙串, 注入 `DEEPSEEK_API_KEY`). 首次启动无 key 时同样弹原生对话框 (可选, 可跳过).
- `打开日志目录`: 定位 `logs/dsh.log` 与 `logs/npm.log`.

## 升级 dsh (上游更新) / Updating dsh

当 `@deepseek-ai/dsh` 发布新版本时的同步流程:

1. **检查**: `./scripts/check-dsh-update.sh` (比对 main.rs 锁定版本与 npm 最新版).
2. **升级**: 修改 `src-tauri/src/main.rs` 的 `DSH_VERSION` 为新版本.
3. **验证 (标准回归)** — 重建后本机启动, 依次检查:
   - [ ] dsh web 正常启动 (boot.log 无错误, 端口绑定成功)
   - [ ] 页面可打开 (curl 本地端口返回 HTTP 200)
   - [ ] 发一条真实消息, 确认模型能回复 (页面能开 ≠ 能对话)
   - [ ] `dsh --profile web --dump-config` 对比 profile 结构是否有大变化
   - [ ] **旧 dsh-home 不删直接跑**是否兼容 (老用户带对话记录升级的路径)
4. **发布**: DASH 版本号 +1 (v0.1.1, v0.1.2...), 新建 GitHub Release tag + 挂新 zip, notes 注明"内含 dsh x.y.z"; 旧版本保留可回退.

### 壳与 dsh 的耦合点 (升级时重点检查)

| 耦合点 | 位置 | dsh 变更后可能的表现 |
|---|---|---|
| 启动参数 `dsh web --host --port` | `src-tauri/src/server.rs` | web 起不来/页面空白 |
| 安装命令 `npm install --prefix ... @deepseek-ai/dsh@<ver>` | `src-tauri/src/bootstrap.rs` | 装不上 |
| 环境变量 `DEEPSEEK_API_KEY` / `DSH_HOME` | `src-tauri/src/server.rs` | key 不生效/数据错位 |
| dsh-home profile 结构 | `~/Library/Application Support/com.solo.dashahan/dsh-home` | 老用户升级后崩溃 |

### 已知限制

- 菜单「更新 dsh」安装的是**当前二进制锁定的版本** —— 升级依赖重新发布 app, 老用户需下载新版 (不会自动升级).

## 数据位置 / Data

| 内容 | 路径 |
|---|---|
| dsh 依赖 | `~/Library/Application Support/com.solo.dashahan/node_modules` |
| dsh 数据 (DSH_HOME) | `~/Library/Application Support/com.solo.dashahan/dsh-home` |
| 配置 | `~/Library/Application Support/com.solo.dashahan/config.json` |
| 日志 | `~/Library/Application Support/com.solo.dashahan/logs/` |
