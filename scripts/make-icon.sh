#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SWIFT="$(mktemp /tmp/dashahan-icon.XXXXXX.swift)"
trap 'rm -f "$ICON_SWIFT"' EXIT

mkdir -p "$ROOT/icon-src"

cat > "$ICON_SWIFT" <<'SWIFT'
import AppKit

let size: CGFloat = 1024
let emoji = "🐶"
let image = NSImage(size: NSSize(width: size, height: size), flipped: false) { rect in
    let fontSize = size * 0.82
    let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: fontSize)]
    let s = emoji as NSString
    let bounds = s.size(withAttributes: attrs)
    let point = NSPoint(x: (rect.width - bounds.width) / 2, y: (rect.height - bounds.height) / 2 - size * 0.06)
    s.draw(at: point, withAttributes: attrs)
    return true
}
guard let tiff = image.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let png = rep.representation(using: .png, properties: [:]) else {
    fatalError("png encode failed")
}
try png.write(to: URL(fileURLWithPath: CommandLine.arguments[1]))
SWIFT

swift "$ICON_SWIFT" "$ROOT/icon-src/icon.png"
(cd "$ROOT" && npx --yes @tauri-apps/cli@2 icon -o src-tauri/icons icon-src/icon.png)
echo "icons -> $ROOT/src-tauri/icons"
