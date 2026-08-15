# 大傻憨 / DASH

双击即用的 macOS 桌面 app, 包装 [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) 的 `dsh web`, 免去 `npx` 命令.

Double-click macOS app wrapping deepseek-harness (`dsh web`). No `npx` commands needed.

## 快速开始 / Quick Start

三步跑起来（完整上手见 [最新 Release](https://github.com/zzpwestlife/dashahan/releases/latest)）：

1. 下载 `DASH-macOS-full.zip`（内嵌 Node，开箱即用）或 `DASH-macOS-lite.zip`（需系统 Node ≥ 18 + npm）
2. 拖入「应用程序」，首次打开在菜单 **大傻憨 → 设置 API Key** 填入 DeepSeek API Key
3. 选模型、选模式，开聊

> DASH 是 macOS 双击即用的 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)（`dsh`）Web 外壳。

## 原理

- Tauri 2 原生窗口 (系统 WKWebView).
- 完整版内嵌 Node.js (约 126MB, 开箱即用); 开发者轻量版用系统 Node.js (约 3.7MB).
- 首次启动自动安装 `@deepseek-ai/dsh` 到 `~/Library/Application Support/com.solo.dashahan/` (约 2 分钟, 仅一次).
- 随机空闲端口启动 `dsh web`, 窗口加载本地页面.
- API Key 只存 macOS 钥匙串 (config 不落盘明文); 启动自动备份配置到 `~/DASH-backup/` (保留 5 份, 不含 key).
- 单实例, 关窗不退出 (隐藏到托盘, dsh 后台保持).

## 要求

- **完整版** (对外分发): 仅需 macOS (Apple Silicon), 无需安装 Node.js.
- **轻量版** (开发者自用): 需本机 Node.js ≥ 18 且带 npm. 若用 `brew install node` 安装, 需额外 `brew install npm`.

## 安装

1. 下载 `dist/DASH-macOS.zip` (或 Releases 页的 DMG), 解压后拖入 Applications.
2. 首次打开: Finder 中右键 DASH.app → 打开 (或执行 `xattr -cr /Applications/DASH.app` 清除隔离标记).
3. 首次启动自动下载 dsh (需联网, 约 2 分钟), 之后秒开.
4. 可选弹出 API Key 对话框, 也可跳过, 之后随时在设置中添加.
5. 关窗 = 隐藏, 非退出; 点菜单栏 🐶 图标唤回, 托盘菜单「退出 DASH」才真正结束.

## 从源码构建

要求: macOS arm64, Rust (`brew install rust`), 已装 Node.

```sh
scripts/make-icon.sh        # 生成图标集 (已有可跳过)
npm install
npm run build               # 轻量版, 用系统 Node
./scripts/smoke-test.sh     # 冒烟测试
```

完整版 (内嵌 Node, 约 126MB):

```sh
./scripts/fetch-node-dist.sh
cd src-tauri && npx tauri build --config tauri.full.conf.json
```

## 菜单与托盘

- **重装 dsh**: 重新 npm install 出厂默认版本 (`src-tauri/src/main.rs` 的 `DSH_VERSION`, 修复安装用).
- **检查 dsh 更新 / 升级 dsh 到 vX.Y.Z**: 启动自动静默检测 npm 官方最新版; 一键升级 dsh (自动备份, 失败回滚, 无需重建 app).
- **检查 DASH 更新 / 更新 DASH 到 vX.Y.Z**: 检测 GitHub 新版本; 一键自动下载替换并重启 (失败自动恢复).
- **设置 API Key**: 写入钥匙串, 注入 `DEEPSEEK_API_KEY`.
- **打开日志目录**: `logs/dsh.log` 与 `logs/npm.log`.

## 升级

- **升级 dsh**: 菜单一键完成 (检测 npm 最新版 → 备份 → 安装 → 重启, 失败自动回滚), 或等启动时的自动检测提示. 上游大版本变更后注意 dsh-home/profile 兼容性, 见 [DEV-NOTES.md](DEV-NOTES.md).
- **升级 DASH**: 菜单一键完成 (自动下载对应版本 zip → 替换 → 重启).
- 发版流程 (改 DSH_VERSION / bump 版本 / 打 tag 触发 CI) 见 [DEV-NOTES.md](DEV-NOTES.md).

## 排障

- 使用问题见 [TROUBLESHOOTING.md](TROUBLESHOOTING.md).
- 开发/CI 运维问题 (push、构建、发版) 见 [DEV-NOTES.md](DEV-NOTES.md).

## 数据位置

| 内容 | 路径 |
|---|---|
| dsh 依赖 | `~/Library/Application Support/com.solo.dashahan/node_modules` |
| dsh 数据 | `~/Library/Application Support/com.solo.dashahan/dsh-home` |
| 配置 | `~/Library/Application Support/com.solo.dashahan/config.json` |
| 日志 | `~/Library/Application Support/com.solo.dashahan/logs/` |
