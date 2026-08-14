#!/bin/bash
# DASH 冒烟测试: 启动 -> dsh web 起来 -> HTTP 200 -> 无错误 -> 钥匙串存在
# 用法: ./scripts/smoke-test.sh [app路径]   (默认 /Applications/DASH.app)
# 退出码: 0=通过, 1=失败 (可作为 CI 发版门禁)
set -uo pipefail

APP="${1:-/Applications/DASH.app}"
D="$HOME/Library/Application Support/com.solo.dashahan"

echo "=== [1/6] 清理旧进程 ==="
pkill -9 -f "Contents/MacOS/dashahan" 2>/dev/null
pkill -9 -f "dsh/lib/bin.js" 2>/dev/null
sleep 2

echo "=== [2/6] 清日志 + 启动 $APP ==="
: > "$D/logs/boot.log" 2>/dev/null || true
: > "$D/logs/dsh.log" 2>/dev/null || true
open "$APP"
sleep 15

echo "=== [3/6] 处理首次 key 对话框(模拟跳过, 不阻塞) ==="
if pgrep -f "osascript.*DeepSeek" >/dev/null 2>&1; then
  echo "⚠️ 检测到 key 对话框, 模拟点取消"
  pkill -9 -f "osascript.*DeepSeek"
  sleep 8
fi

echo "=== [4/6] 检查 boot.log 无错误 ==="
cat "$D/logs/boot.log" 2>/dev/null
if grep -q "error" "$D/logs/boot.log" 2>/dev/null; then
  echo "❌ boot.log 出现错误"
  exit 1
fi

echo "=== [5/6] dsh web HTTP 检查 ==="
PORT=$(grep -oE "[0-9]+" "$D/logs/dsh.log" 2>/dev/null | tail -1)
if [ -z "$PORT" ]; then
  echo "❌ dsh web 未启动 (dsh.log 无端口)"
  exit 1
fi
CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://127.0.0.1:$PORT/" 2>/dev/null)
echo "端口 $PORT → HTTP $CODE"
if [ "$CODE" != "200" ]; then
  echo "❌ HTTP 非 200"
  exit 1
fi

echo "=== [6/6] 钥匙串检查 ==="
K=$(security find-generic-password -a deepseek-api-key -s com.solo.dashahan -w 2>/dev/null)
if [ -n "$K" ]; then
  echo "✅ 钥匙串 key 存在 (${#K} 字符)"
else
  echo "⚠️ 钥匙串无 key (首次运行/未配置, 不阻塞)"
fi

echo ""
echo "✅ 冒烟测试全部通过"
exit 0
