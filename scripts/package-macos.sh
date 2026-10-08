#!/usr/bin/env bash
# Package an existing native executable; all icon sizes derive from the legacy app icon.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="${1:?Usage: package-macos.sh <native-binary> <output.app>}"
APP="${2:?Usage: package-macos.sh <native-binary> <output.app>}"
[[ -f "$SOURCE" ]] || { printf 'Missing native binary: %s\n' "$SOURCE" >&2; exit 1; }
ICON_WORK="$(mktemp -d "${TMPDIR:-/tmp}/chu-icon.XXXXXX")"
trap 'rm -rf "$ICON_WORK"' EXIT
ICONSET="$ICON_WORK/appicon.iconset"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ICONSET"
cp "$SOURCE" "$APP/Contents/MacOS/chu"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$ROOT/assets/app-icon.png" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double="$((size * 2))"
  if [[ "$double" == 1024 ]]; then
    # Keep the full-resolution artwork unchanged, including transparency and colors.
    cp "$ROOT/assets/app-icon.png" "$ICONSET/icon_${size}x${size}@2x.png"
  else
    sips -z "$double" "$double" "$ROOT/assets/app-icon.png" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  fi
done
iconutil --convert icns "$ICONSET" --output "$APP/Contents/Resources/iconfile.icns"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Chu</string>
<key>CFBundleDisplayName</key><string>Chu Agent Manager</string>
<key>CFBundleExecutable</key><string>chu</string>
<key>CFBundleIdentifier</key><string>dev.chu.agent-manager</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>iconfile.icns</string>
<key>CFBundleShortVersionString</key><string>0.2.0</string>
<key>CFBundleVersion</key><string>2</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
# Make Finder notice the changed bundle metadata without deleting system icon caches.
touch "$APP"
printf 'Built %s (unsigned local application, original Chu icon)\n' "$APP"
