#!/usr/bin/env bash
# PROTOTYPE: packages the stakeholder demo as a double-clickable macOS app,
# zipped for sending: dist/Cellworks-Demo.zip. Apple silicon only, ad-hoc signed.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
DIST="$REPO/dist"
APP="$DIST/Cellworks Demo.app"

cargo build --release --manifest-path "$REPO/Cargo.toml"
# Fill the npm cache so the recipient's first run works offline.
# In a world of its own, so a demo that is running keeps its world.
mkdir -p "$DIST/.world"
TMPDIR="$DIST/.world/" bash "$HERE/setup.sh" >/dev/null

rm -rf "$APP" "$DIST/Cellworks-Demo.zip"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/demo"
cp "$REPO/target/release/cellworks" "$APP/Contents/MacOS/cellworks"
tar -C "$HERE" --exclude ./package.sh --exclude 'node_modules' -cf - . | tar -C "$APP/Contents/demo" -xf -

cat >"$APP/Contents/MacOS/launch" <<'SH'
#!/bin/bash
# Starts the guided Cellworks demo on a fresh demo world.
CONTENTS="$(cd "$(dirname "$0")/.." && pwd)"
# Apps started from Finder get a bare PATH; take the login shell's for git, node and npm.
PATH="$(/bin/zsh -lc 'printf %s "$PATH"' 2>/dev/null):/opt/homebrew/bin:/usr/local/bin:$PATH"
export PATH
CACHE="$HOME/Library/Caches/cellworks-demo"
mkdir -p "$CACHE"
[ -d "$CACHE/npm" ] || cp -R "$CONTENTS/demo/.cache/npm" "$CACHE/npm"
export CELLWORKS_NPM_CACHE="$CACHE/npm" CELLWORKS_DEMO_ASSETS="$CONTENTS/demo" CELLWORKS_DEMO=1
if ! command -v git >/dev/null; then
  osascript -e 'display alert "Cellworks demo needs git" message "Install the Xcode command line tools: xcode-select --install"'
  exit 1
fi
command -v npm >/dev/null || osascript -e 'display alert "Node.js not found" message "The demo runs without it, but the shop app window will not start. Install Node.js from nodejs.org to see it."'
bash "$CONTENTS/demo/setup.sh" >"$CACHE/setup.log" 2>&1
exec "$CONTENTS/MacOS/cellworks"
SH
chmod +x "$APP/Contents/MacOS/launch"

cat >"$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Cellworks Demo</string>
  <key>CFBundleIdentifier</key><string>io.squer.cellworks.demo</string>
  <key>CFBundleExecutable</key><string>launch</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
ditto -c -k --keepParent "$APP" "$DIST/Cellworks-Demo.zip"
du -h "$DIST/Cellworks-Demo.zip"
