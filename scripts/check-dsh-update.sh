#!/bin/bash
# 检查 @deepseek-ai/dsh 上游是否有新版本 (对比 main.rs 锁定的 DSH_VERSION 与 npm latest)
# 用法: ./scripts/check-dsh-update.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAIN_RS="$ROOT/src-tauri/src/main.rs"
PACKAGE="@deepseek-ai/dsh"

PINNED="$(grep -oE 'DSH_VERSION: &str = "[^"]+"' "$MAIN_RS" | sed 's/.*"\(.*\)"/\1/')"
if [ -z "$PINNED" ]; then
  echo "❌ 无法从 $MAIN_RS 解析 DSH_VERSION" >&2
  exit 1
fi

echo "当前锁定: $PINNED  (main.rs)"
echo "正在查询 npm 最新版本…"
LATEST="$(npm view "$PACKAGE" version 2>/dev/null || true)"
if [ -z "$LATEST" ]; then
  echo "❌ 查询 npm 失败 (网络或 registry 问题)" >&2
  exit 1
fi
echo "npm 最新: $LATEST"

if [ "$PINNED" = "$LATEST" ]; then
  echo "✅ 已是最新, 无需升级"
else
  echo "🚀 发现新版本: $PINNED -> $LATEST"
  echo "升级流程: 改 main.rs 的 DSH_VERSION -> 重建 -> 标准回归 -> 发新 tag"
fi
