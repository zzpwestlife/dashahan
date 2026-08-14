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
1. 未签名提示: Finder 中 **右键 DASH.app → 打开** → 再点"打开" (或 `xattr -d com.apple.quarantine /Applications/DASH.app`).
2. 首次启动自动下载 dsh (需联网, 约 2 分钟), 之后跳过.
3. 弹出系统对话框 → 粘贴自己的 **DeepSeek API Key** → 保存.
4. 之后双击秒开.

## 从源码构建 / Build from source

要求: macOS arm64, Rust (`brew install rust`), Node (系统已装).

```sh
scripts/make-icon.sh       # 生成 🐶 图标集 (已有图标可跳过)
npm install
npm run build              # 产出 src-tauri/target/release/bundle/dmg/
```

## 菜单 / Menu

- `更新 dsh`: 重新执行 npm install (升级常量版本需改 `src-tauri/src/main.rs` 的 `DSH_VERSION`).
- `设置 API Key`: 弹出 macOS 原生对话框输入 DeepSeek API Key (写入 app config, 注入 `DEEPSEEK_API_KEY`). 首次启动无 key 时同样弹原生对话框.
- `打开日志目录`: 定位 `logs/dsh.log` 与 `logs/npm.log`.

## 数据位置 / Data

| 内容 | 路径 |
|---|---|
| dsh 依赖 | `~/Library/Application Support/com.solo.dashahan/node_modules` |
| dsh 数据 (DSH_HOME) | `~/Library/Application Support/com.solo.dashahan/dsh-home` |
| 配置 | `~/Library/Application Support/com.solo.dashahan/config.json` |
| 日志 | `~/Library/Application Support/com.solo.dashahan/logs/` |
