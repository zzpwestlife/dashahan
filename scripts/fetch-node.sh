#!/usr/bin/env bash
set -euo pipefail

NODE_VERSION="v25.9.0"
ARCH="darwin-arm64"
DEST="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/resources"
TMP="$(mktemp -d)"

cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

cd "$TMP"
curl -fsSL "https://nodejs.org/dist/${NODE_VERSION}/node-${NODE_VERSION}-${ARCH}.tar.gz" -o node.tgz
tar -xzf node.tgz

mkdir -p "$DEST"
cp "node-${NODE_VERSION}-${ARCH}/bin/node" "$DEST/node"
chmod +x "$DEST/node"
rm -rf "${DEST:?}/npm"
cp -R "node-${NODE_VERSION}-${ARCH}/lib/node_modules/npm" "$DEST/npm"

"$DEST/node" --version
echo "node ${NODE_VERSION} -> ${DEST}"
