# 大傻憨 / Dashahan

双击即用的 macOS 桌面 app, 包装 [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) 的 `dsh web`. 无需安装 Node, 无需记 `npx` 命令.

Double-click macOS app wrapping deepseek-harness (`dsh web`). No Node install, no `npx` commands needed.

## 原理 / How it works

- Tauri 2 原生窗口 (系统 WKWebView).
- 内嵌 Node.js (arm64) 与 npm 于 app 包内.
- 首次启动自动把 `@deepseek-ai/dsh` 安装到 `~/Library/Application Support/com.solo.dashahan/`, 之后秒开.
- 随机空闲端口启动 `dsh web`, 窗口直接加载本地页面.
- 单实例: 重复双击只会聚焦已有窗口. 关窗即结束后端进程.

## 安装 / Install

1. 打开 `大傻憨_0.1.0_aarch64.dmg`, 拖入 Applications.
2. 未签名: 首次打开请在 Finder 中**右键 → 打开**, 或执行:

   ```sh
   xattr -d com.apple.quarantine /Applications/大傻憨.app
   ```

## 从源码构建 / Build from source

要求: macOS arm64, Rust (`brew install rust`), Node (仅构建期用).

```sh
scripts/fetch-node.sh      # 下载内嵌 node 到 src-tauri/resources
scripts/make-icon.sh       # 生成 🐶 图标集
npm install
npm run build              # 产出 src-tauri/target/release/bundle/dmg/
```

## 菜单 / Menu

- `更新 dsh`: 重新执行 npm install (升级常量版本需改 `src-tauri/src/main.rs` 的 `DSH_VERSION`).
- `设置 API Key`: 回到启动页填写 DeepSeek API Key (写入 app config, 注入 `DEEPSEEK_API_KEY`).
- `打开日志目录`: 定位 `logs/dsh.log` 与 `logs/npm.log`.

## 数据位置 / Data

| 内容 | 路径 |
|---|---|
| dsh 依赖 | `~/Library/Application Support/com.solo.dashahan/node_modules` |
| dsh 数据 (DSH_HOME) | `~/Library/Application Support/com.solo.dashahan/dsh-home` |
| 配置 | `~/Library/Application Support/com.solo.dashahan/config.json` |
| 日志 | `~/Library/Application Support/com.solo.dashahan/logs/` |
