#!/bin/bash
# 下载 Node 发行包 (darwin-arm64) 到 src-tauri/resources/node-dist, 供完整版构建内嵌.
# 用法: ./scripts/fetch-node-dist.sh [版本号]   (默认 v22.15.0)
# 完整版构建: ./scripts/fetch-node-dist.sh && cd src-tauri && npx tauri build --config tauri.full.conf.json
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/src-tauri/resources/node-dist"
NODE_VERSION="${1:-v22.15.0}"

if [ -x "$DEST/bin/node" ]; then
  echo "✅ node-dist 已存在: $($DEST/bin/node --version)"
  exit 0
fi

URL="https://nodejs.org/dist/${NODE_VERSION}/node-${NODE_VERSION}-darwin-arm64.tar.gz"
echo "下载 $URL ..."
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -fsSL "$URL" -o "$TMP/node.tar.gz"
mkdir -p "$TMP/x"
tar -xzf "$TMP/node.tar.gz" -C "$TMP/x"

mkdir -p "$DEST"
cp -R "$TMP/x/node-${NODE_VERSION}-darwin-arm64/bin" "$DEST/bin"
cp -R "$TMP/x/node-${NODE_VERSION}-darwin-arm64/lib" "$DEST/lib"
chmod +x "$DEST/bin/node"
echo "✅ node-dist 就绪: $($DEST/bin/node --version) (npm: $($DEST/bin/node "$DEST/lib/node_modules/npm/bin/npm-cli.js" --version))"
