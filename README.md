# 大傻憨 / DASH

双击即用的 macOS 桌面 app, 包装 [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) 的 `dsh web`, 免去 `npx` 命令.

Double-click macOS app wrapping deepseek-harness (`dsh web`). No `npx` commands needed.

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

- **重装 dsh**: 重新执行 npm install (升级 dsh 版本需改 `src-tauri/src/main.rs` 的 `DSH_VERSION` 并重新构建).
- **设置 API Key**: 写入 config + 钥匙串, 注入 `DEEPSEEK_API_KEY`.
- **打开日志目录**: `logs/dsh.log` 与 `logs/npm.log`.

## 升级 dsh

1. `./scripts/check-dsh-update.sh` 检查新版本.
2. 修改 `src-tauri/src/main.rs` 的 `DSH_VERSION`.
3. 重建后验证: web 能启动、页面 200、能真实对话、profile 结构无破坏性变化、老 dsh-home 兼容.
4. DASH 版本号 +1, 发布新 Release, notes 注明内含 dsh 版本.

> 已知限制: 菜单「重装 dsh」安装的是当前二进制锁定的版本, 老用户需下载新版才能升级.

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
