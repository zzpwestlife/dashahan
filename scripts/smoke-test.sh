#!/bin/bash
# DASH 冒烟测试: 启动 -> dsh web 起来 -> HTTP 200 -> 无错误 -> 钥匙串存在
# 用法: ./scripts/smoke-test.sh [app路径]   (默认 /Applications/DASH.app)
# 退出码: 0=通过, 1=失败 (可作为 CI 发版门禁)
set -uo pipefail

APP="$1"
if [ -z "$APP" ]; then
  APP="/Applications/DASH.app"
fi
D="$HOME/Library/Application Support/com.solo.dashahan"
LOG="$D/logs"

echo "=== [1/6] 清理旧进程 ==="
pkill -9 -f "Contents/MacOS/dashahan" 2>/dev/null
pkill -9 -f "dsh/lib/bin.js" 2>/dev/null
sleep 2

echo "=== [2/6] 清日志 + 启动 $APP ==="
mkdir -p "$LOG"
: > "$LOG/boot.log" 2>/dev/null || true
: > "$LOG/dsh.log" 2>/dev/null || true
open "$APP"

echo "=== [3/6] 等待 dsh web 就绪 (首次运行安装 dsh 可能需 1~3 分钟) ==="
WAITED=0
PORT=""
while [ "$WAITED" -lt 180 ]; do
  # 首次 key 对话框: 模拟点取消, 不阻塞
  if pgrep -f "osascript.*DeepSeek" >/dev/null 2>&1; then
    echo "⚠️ 检测到 key 对话框, 模拟点取消"
    pkill -9 -f "osascript.*DeepSeek"
  fi
  PORT=$(grep -oE "[0-9]+" "$LOG/dsh.log" 2>/dev/null | tail -1)
  if [ -n "$PORT" ]; then
    CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 "http://127.0.0.1:$PORT/" 2>/dev/null)
    if [ "$CODE" = "200" ]; then
      break
    fi
  fi
  sleep 5
  WAITED=$((WAITED + 5))
done

if [ -z "$PORT" ]; then
  echo "❌ dsh web 180s 内未就绪 (boot.log 尾部:)"
  tail -5 "$LOG/boot.log" 2>/dev/null
  exit 1
fi

echo "=== [4/6] 检查 boot.log 无错误 ==="
cat "$LOG/boot.log" 2>/dev/null
if grep -q "error" "$LOG/boot.log" 2>/dev/null; then
  echo "❌ boot.log 出现错误"
  exit 1
fi

echo "=== [5/6] dsh web HTTP 复检 ==="
CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://127.0.0.1:$PORT/" 2>/dev/null)
echo "端口 $PORT → HTTP $CODE"
if [ "$CODE" != "200" ]; then
  echo "❌ HTTP 非 200"
  exit 1
fi

echo "=== [6/6] 钥匙串检查 ==="
K=$(security find-generic-password -a deepseek-api-key -s com.solo.dashahan -w 2>/dev/null)
if [ -n "$K" ]; then
  K_LEN=$(printf '%s' "$K" | wc -c | tr -d ' ')
  echo "✅ 钥匙串 key 存在 ($K_LEN 字符)"
else
  echo "⚠️ 钥匙串无 key (首次运行/未配置, 不阻塞)"
fi

echo ""
echo "✅ 冒烟测试全部通过"
exit 0
