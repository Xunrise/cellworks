#!/usr/bin/env bash
# Builds dist/Cellworks.app, zips it and writes the release manifest the
# in-app updater reads (src/update.rs). Apple silicon, ad-hoc signed.
#
#   scripts/package-app.sh [repository]   # repository defaults to $GITHUB_REPOSITORY
#
# Release notes come from docs/release-notes/<version>.md when it exists.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPOSITORY="${1:-${GITHUB_REPOSITORY:-Xunrise/cellworks}}"
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$REPO_ROOT/Cargo.toml" | head -1)
DIST="$REPO_ROOT/dist"
APP="$DIST/Cellworks.app"
ZIP="cellworks-app-$VERSION.zip"

cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

rm -rf "$APP" "$DIST/$ZIP" "$DIST/manifest.json"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$REPO_ROOT/target/release/cellworks" "$APP/Contents/MacOS/cellworks"
cp "$REPO_ROOT/assets/icon/Cellworks.icns" "$APP/Contents/Resources/Cellworks.icns"

# Apps started from Finder get a bare PATH; Cellworks runs git, claude, gh,
# kubectl and nvim, so it takes the login shell's PATH first.
cat >"$APP/Contents/MacOS/launch" <<'SH'
#!/bin/bash
PATH="$(/bin/zsh -lc 'printf %s "$PATH"' 2>/dev/null):/opt/homebrew/bin:/usr/local/bin:$PATH"
export PATH
exec "$(dirname "$0")/cellworks" "$@"
SH
chmod +x "$APP/Contents/MacOS/launch"

cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Cellworks</string>
  <key>CFBundleDisplayName</key><string>Cellworks</string>
  <key>CFBundleIdentifier</key><string>io.github.xunrise.cellworks</string>
  <key>CFBundleExecutable</key><string>launch</string>
  <key>CFBundleIconFile</key><string>Cellworks</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
ditto -c -k --keepParent "$APP" "$DIST/$ZIP"

NOTES_FILE="$REPO_ROOT/docs/release-notes/$VERSION.md"
NOTES=""
[ -f "$NOTES_FILE" ] && NOTES=$(cat "$NOTES_FILE")

SIZE=$(stat -f %z "$DIST/$ZIP")
SHA=$(shasum -a 256 "$DIST/$ZIP" | cut -d' ' -f1)
URL="https://github.com/$REPOSITORY/releases/download/v$VERSION/$ZIP"
python3 - "$DIST/manifest.json" "$VERSION" "$ZIP" "$URL" "$SIZE" "$SHA" "$NOTES" <<'PY'
import json, sys
out, version, name, url, size, sha, notes = sys.argv[1:]
manifest = {"schema": 1, "version": version, "arch": "arm64",
            "app": {"name": name, "url": url, "size": int(size), "sha256": sha},
            "notes": notes}
open(out, "w").write(json.dumps(manifest, indent=2) + "\n")
PY

echo "$DIST/$ZIP"
echo "$DIST/manifest.json"
