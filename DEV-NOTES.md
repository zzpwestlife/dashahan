# DASH 开发运维笔记 (DEV-NOTES)

> 记录本项目踩过的坑与解决方案, 遇到类似问题时先来这里找答案.
> 最后更新: 2026-08-14 (v0.1.5 发布, CI 全链路打通; v0.1.6 增加升级提醒与一键升级)

## 0. 升级提醒 + 一键升级 (v0.1.6 新功能)

两条更新线, 启动时后台静默检查 (独立线程, 失败无感), 发现新版时菜单文案自动变为带版本号.

### dsh 上游升级 (菜单「检查 dsh 更新」→「升级 dsh 到 vX.Y.Z」)
- 版本来源: **运行时读** `node_modules/@deepseek-ai/dsh/package.json` (`shared::installed_dsh_version`);
  `main.rs` 的 `DSH_VERSION` 降级为**出厂默认** (仅首次安装/重装用), 升级不再需要改常量+重建.
- 检测: `curl https://registry.npmjs.org/@deepseek-ai/dsh/latest` (官方 registry, 不受内网 ~/.npmrc 影响).
- 升级流程: 备份 (`data/backups/dsh`) → `npm install @deepseek-ai/dsh@<latest> --registry=官方`
  (`bootstrap::npm_install_version`) → 重启 dsh web (`bootstrap::restart_dsh`) → 失败自动回滚 (`shared::restore_dsh`).
- 门禁: npm 成功 + 进程启动 + 本地 HTTP 200 (server::launch 自带端口探测). 真实消息验证留人工回归.

### DASH 自更新 (菜单「检查 DASH 更新」→「更新 DASH 到 vX.Y.Z」)
- 检测: `curl https://api.github.com/repos/zzpwestlife/dashahan/releases/latest`
  (**必须带 User-Agent 头**, 否则 GitHub API 403) → tag_name 去 v 前缀.
- 下载: 按本地形态选 zip (`shared::has_embedded_node` 判 full/lite), GitHub releases 下载.
- 替换: 备份当前 app 到 `~/DASH-backup/DASH-app-<ver>/` → 写辅助脚本 (等主进程退出 → rm+cp →
  xattr 清隔离 → open; 失败恢复备份) → `app.exit(0)` 触发替换重启. 脚本用 raw string 存, 参数传路径.
- 定位当前 app: `std::env::current_exe()` 向上三级 (Contents/MacOS/<bin> → .app).

### 通知与反馈 (v0.1.7 补全)
- 发现新版: 后台检查线程弹 **macOS 系统通知** (`shared::notify`, osascript `display notification`, 零依赖无需权限) + 菜单文案变化.
- 升级过程/结果: 开始时 notify; 完成/失败弹**模态对话框** (`shared::alert`, osascript `display alert`) —— 即使 dsh 正在运行(窗口显示 dsh UI)用户也能看到结果, 不依赖启动页.
- dsh 升级门禁增强: `bootstrap::restart_dsh` 在 TCP 探测后**轮询 HTTP 200 最多 15s** (`http_ready`), 未就绪视为升级失败触发回滚.
- DASH 替换后验证: 辅助脚本 `open` 新 app 后 **pgrep 验证进程起来 (最多 15s)**, 失败恢复备份 + osascript 通知. 注意脚本里 osascript 用单引号包裹避免引号嵌套问题.

### 实现要点/坑
- 版本比较用 **semver crate** (新增依赖), `shared::version_less`; 解析失败返回 false (宁可不提示不误升级).
- 菜单动态文案: `menu::refresh_update_items` 用 `app.menu().get(id)` (返回 `MenuItemKind` 枚举, 需 match
  取 `MenuItem` 变体) + `item.set_text(text)` (**tauri 2.11 的 set_text 只收 1 个参数, 不要传 app**).
- **DSH 沙箱的 write 工具会把源码里的 `\"` 转义和 `${...}` 当模板处理** (写 Rust 字符串时 `\"` 会被吃掉,
  写 shell 时 `${1:-x}` 会报错). 规避: 用 edit 修正 / 字符串避免 `\"` / shell 用 `$#` 短路代替 `${1:-}`.

## 1. 本机环境速查 (2026-08-14 实测)

| 项 | 值 | 说明 |
|---|---|---|
| 系统代理 | HTTP/HTTPS `127.0.0.1:8118`, SOCKS `8119` | GitHub Desktop 走系统代理所以一直正常 |
| git 直连 github | **不稳定** (DNS 轮询到不同 IP, 部分不可达) | 仓库级已配代理解决 |
| npm registry (用户级) | `http://registry.npm.oa.com/` (~/.npmrc, 腾讯内网镜像, 带 authToken) | 官方镜像: `https://registry.npmjs.org/` |
| gh CLI | 已登录 `zzpwestlife` (keyring), 走 api.github.com 稳定 | 比 git 协议可靠 |
| 仓库级 git 配置 (.git/config) | `lfs.locksverify=false`, credential helper=gh, `http(s).proxy=http://127.0.0.1:8118` | 见问题 1 |
| pre-push 钩子 | `~/.git-hooks/pre-push` (腾讯推送统计) | DSH 沙箱下报 /bin/ps 拒绝但非致命 (返回 0) |

---

## 2. 问题 1: git push 命令行失败 (此前只能靠 GitHub Desktop)

### 症状
- push 卡住/超时: `dial tcp 20.205.243.166:443: i/o timeout`
- 报错涉及 `info/lfs/locks/verify` 请求超时
- `Failed to connect to github.com port 443 after 75017 ms`

### 根因 (三层叠加)
1. **LFS 残留配置**: 仓库 .git/config 有 git-lfs 配置 (仓库实际不用 LFS, 无 .gitattributes).
   git-lfs 每次 push 前发 locks verify 请求, 而 git-lfs 是 Go 写的、**不读系统代理**,
   直连 github 恰好命中不通的 IP → 超时.
2. **无凭据助手**: git 没有 credential helper, 命令行 push 到认证那步也会卡.
3. **git 直连 github 网络不稳**: DNS 轮询到不同 IP, 有的可达有的超时 (curl 通是因为解析到了别的 IP).

### 解决步骤 (全部仓库级, 写 .git/config, 不影响全局)
```sh
# ① 跳过 LFS locks verify (仓库不用 LFS)
git config lfs.locksverify false

# ② 凭据助手指向 gh (用 gh token 认证)
git config credential.https://github.com.helper '!/opt/homebrew/bin/gh auth git-credential'

# ③ 走系统代理 (和 GitHub Desktop 一致的网络路径)
git config http.proxy http://127.0.0.1:8118
git config https.proxy http://127.0.0.1:8118
```

### 验证
```sh
git push origin <branch|tag>   # 应直接成功
```

### 注意事项
- `gh auth setup-git` 写 ~/.gitconfig 会被 DSH 沙箱文件策略拦 (Operation not permitted),
  用**仓库级** credential helper 可绕过; 用户自己的终端可直接跑 gh auth setup-git 全局生效.
- 代理 8118 依赖本机代理软件在跑; 若代理未开, git 走代理也会失败 (此时 GitHub Desktop 同样失败).
- 沙箱内 push 时 pre-push 钩子报 `/bin/ps: Operation not permitted` 是沙箱限制, 钩子返回 0 不拦 push.

### 诊断命令 (可复用)
```sh
git ls-remote origin                              # 快速测 git 协议连通
scutil --proxy                                    # 看系统代理 (Enable/Server/Port)
curl -I https://github.com                        # 测连通 (注意 curl 可能走不同 IP)
env | grep -i proxy                               # 看代理环境变量
nslookup github.com                               # 看 DNS 解析到的 IP
git config --list --show-origin                   # 看配置来源
```

---

## 3. 问题 2: GitHub Actions CI 一直失败 (v0.1.3 ~ v0.1.5 前期)

### 症状
- run 12~16s 即失败
- `npm error 404 Not Found - GET http://registry.npm.oa.com/@tauri-apps/cli/...`

### 根因链 (4 个 bug, 修一个暴露下一个)
1. **package-lock.json 锁内网 registry**: 本机 ~/.npmrc 指向腾讯内网镜像,
   生成的 lock 文件 12 处 resolved 都是 `http://registry.npm.oa.com/`.
   Actions runner 访问不到 → npm install 404. (v0.1.3/v0.1.4 的 CI 都死在这)
2. **tauri 只出 dmg 并清理 .app**: tauri.conf.json 的 bundle targets 只有 `["dmg"]`,
   打 dmg 后 tauri 会删掉中间产物 `bundle/macos/DASH.app` → 后续 cp .app 报 No such file.
3. **完整版 --config 路径**: tauri CLI 的 `--config` 相对**工作目录**解析,
   CI 在仓库根跑 `npm run build -- --config tauri.full.conf.json` 找不到 (文件在 src-tauri/ 下).
4. **冒烟测试固定 15s 等待**: CI 干净环境首次启动 app 要现场 npm install dsh (1~3 分钟),
   15s 后 dsh 未就绪 → dsh.log 无端口 → 失败; 且 `set -u` 下脚本读未设置的 `$1` 直接报 unbound.

### 解决步骤 (release.yml + smoke-test.sh 修改)
```yaml
# ① 安装依赖: 不用内网 lock, 用官方 registry 重新解析
- run: |
    rm -f package-lock.json
    npm install --registry=https://registry.npmjs.org/

# ② 构建显式产出 app+dmg (两处构建都加)
- run: npm run build -- --target aarch64-apple-darwin --bundles app,dmg

# ③ 完整版 --config 带 src-tauri/ 前缀
- run: npm run build -- --config src-tauri/tauri.full.conf.json --target aarch64-apple-darwin --bundles app,dmg
```

```sh
# ④ smoke-test.sh: 轮询等待 dsh 就绪 (最多 180s), 参数用 $# 短路保护
while [ "$WAITED" -lt 180 ]; do
  # 处理 key 对话框 + 检查 dsh.log 端口 + curl HTTP 200
  sleep 5; WAITED=$((WAITED + 5))
done
if [ $# -ge 1 ] && [ -n "$1" ]; then APP="$1"; else APP="/Applications/DASH.app"; fi
```

另: workflow 加了 `workflow_dispatch` 便于手动触发.

### 诊断命令 (可复用)
```sh
gh run list --repo zzpwestlife/dashahan          # 看 run 状态
gh run view <runId> --log-failed                  # 失败日志 (沙箱下需 GH_CACHE_DIR 或走 API)
# API 直取日志 (沙箱友好, 不写缓存):
JOB=$(gh api repos/zzpwestlife/dashahan/actions/runs/<id>/jobs --jq '.jobs[0].id')
gh api "repos/zzpwestlife/dashahan/actions/jobs/$JOB/logs"
# 注意: grep 日志时 'thiserror' 等包名会误匹配 'error', 直接看尾部更准
```

### 教训 (预防)
- **本地能构建 ≠ CI 能构建**: 本机内网 npm 镜像会污染 lock 文件. 提交 package-lock.json 前检查 resolved 域名:
  `grep -o 'registry.npm.[a-z.]*' package-lock.json | sort -u`
- **不要**把 lock 的 resolved 改成官方 URL 提交: 用户本机依赖内网镜像, 改后本机构建会失败.
- tauri 需要 .app 产物时, 构建加 `--bundles app,dmg` (或 conf 里 targets 配 `["app","dmg"]`).
- tauri `--config` 路径相对 cwd.
- CI 干净环境首次运行 app 会现场装依赖, 测试脚本要**轮询等待**而不是固定 sleep.

---

## 4. 发版流程速查 (v0.1.5 一次通过)

```sh
# 0. 如同步上游 dsh: 改 src-tauri/src/main.rs 的 DSH_VERSION
# 1. bump 版本 (两处保持一致): src-tauri/tauri.conf.json + package.json
# 2. commit + push main
# 3. 打 tag 并推送 (触发 CI 自动构建 + 建 Release)
git tag -f v0.1.x HEAD
git push origin :refs/tags/v0.1.x   # 若 tag 已存在需先删远端旧 tag
git push origin v0.1.x
# 4. 等 CI (~6 分钟): gh run list 观察; 成功后 gh release view v0.1.x
# 5. 可选: gh release edit v0.1.x --notes "..." 补充说明
```

Release 资产: `DASH-macOS-full.zip` + `DASH-macOS-lite.zip` + `DASH_0.1.x_aarch64.dmg`.
CI notes 是模板 (含"内含 dsh <版本>"), 需要详细说明时用 gh release edit 补.

---

## 5. 沙箱 (DSH) 环境注意事项

- 文件沙箱默认只允许写工作区 (`/Users/admin/openSource/dashahan`); 写 ~/.gitconfig、/tmp 部分路径会被拦.
  绕过: 仓库级配置 / 设 `GH_CACHE_DIR` 到可写目录 / 申请提权.
- `/bin/ps` 不可用 → pre-push 钩子报错但非致命.
- gh 日志缓存写 ~/.cache 会被拦 → 用 `gh api .../logs` 直取.
