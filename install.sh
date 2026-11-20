#!/bin/sh
# Cellworks installer for macOS on Apple silicon.
#
#   curl -fsSL https://raw.githubusercontent.com/Xunrise/cellworks/main/install.sh | sh
#   curl -fsSL .../install.sh | sh -s -- --version 0.3.0
#
# Environment:
#   CELLWORKS_INSTALL_DIR  folder for Cellworks.app (default /Applications if
#                          writable, else ~/Applications)
#   CELLWORKS_VERSION      version to install (default: latest release)
#
# Downloads the release zip named in the release's manifest.json, checks its
# size and SHA-256, and installs Cellworks.app without sudo. An existing
# Cellworks.app is replaced, and put back if the install fails. Re-run to upgrade.
set -eu

REPO="Xunrise/cellworks"
version="${CELLWORKS_VERSION:-}"

while [ $# -gt 0 ]; do
  case "$1" in
    --version) shift; version="${1:?--version needs a value}" ;;
    --version=*) version="${1#*=}" ;;
    -h|--help) sed -n '2,15p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "install.sh: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

say() { printf '%s\n' "$*"; }
die() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- platform ---------------------------------------------------------------
[ "$(uname -s)" = Darwin ] || die "Cellworks is a macOS app; there is no build for $(uname -s). Build from source: https://github.com/$REPO#from-source"
arch=$(uname -m)
# A shell running under Rosetta reports x86_64 on an Apple silicon Mac.
if [ "$arch" = x86_64 ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
  arch=arm64
fi
[ "$arch" = arm64 ] || die "Cellworks ships for Apple silicon (arm64) only; this Mac is $arch. Build from source: https://github.com/$REPO#from-source"

for tool in curl ditto shasum; do have "$tool" || die "need $tool"; done

# --- manifest ---------------------------------------------------------------
version="${version#v}"
if [ -n "$version" ]; then
  manifest_url="https://github.com/$REPO/releases/download/v$version/manifest.json"
else
  manifest_url="https://github.com/$REPO/releases/latest/download/manifest.json"
fi

tmp=$(mktemp -d) || die "mktemp failed"
backup=""
restore() {
  if [ -n "$backup" ] && [ -e "$backup" ] && [ ! -e "$dest" ]; then
    mv "$backup" "$dest" && say "Put the previous Cellworks.app back"
  fi
  rm -rf "$tmp"
}
trap restore EXIT
trap 'exit 1' INT TERM

curl -fsSL "$manifest_url" -o "$tmp/manifest.json" || die "could not fetch $manifest_url"

# One flat field of manifest.json, without jq: "key": "value" or "key": number.
field() {
  sed -n "s/^ *\"$1\": *\"\{0,1\}\([^\",]*\)\"\{0,1\},\{0,1\} *$/\1/p" "$tmp/manifest.json" | head -1
}
got_version=$(field version)
url=$(field url)
size=$(field size)
sha=$(field sha256)
name=$(field name)
[ -n "$got_version" ] && [ -n "$url" ] && [ -n "$size" ] && [ -n "$sha" ] && [ -n "$name" ] \
  || die "manifest.json is missing a field ($manifest_url)"
[ "$(field arch)" = arm64 ] || die "the release is not built for arm64"
case "$name" in */*|.*) die "unexpected file name in the manifest: $name" ;; esac

# --- download and verify ----------------------------------------------------
say "Downloading Cellworks $got_version"
curl -fL --progress-bar "$url" -o "$tmp/$name" || die "download failed: $url"

got_size=$(wc -c <"$tmp/$name" | tr -d ' ')
[ "$got_size" = "$size" ] || die "size mismatch for $name (expected $size bytes, got $got_size)"
got_sha=$(shasum -a 256 "$tmp/$name" | cut -d' ' -f1)
[ "$got_sha" = "$sha" ] || die "checksum mismatch for $name (expected $sha, got $got_sha)"

ditto -x -k "$tmp/$name" "$tmp/unpacked" || die "could not unpack $name"
app="$tmp/unpacked/Cellworks.app"
[ -x "$app/Contents/MacOS/cellworks" ] || die "the zip holds no Cellworks.app"
# curl sets no quarantine flag, but clear one if something else did, so the
# ad-hoc signed app opens without the "unidentified developer" dialog.
xattr -dr com.apple.quarantine "$app" 2>/dev/null || true

# --- install ----------------------------------------------------------------
if [ -n "${CELLWORKS_INSTALL_DIR:-}" ]; then
  dir="$CELLWORKS_INSTALL_DIR"
elif [ -w /Applications ]; then
  dir=/Applications
else
  dir="$HOME/Applications"
fi
mkdir -p "$dir" 2>/dev/null || die "could not create $dir"
[ -w "$dir" ] || die "$dir is not writable; set CELLWORKS_INSTALL_DIR to a folder you own"
dest="$dir/Cellworks.app"

old=""
if [ -e "$dest" ]; then
  old=$("$dest/Contents/MacOS/cellworks" --version 2>/dev/null | awk '{print $NF}' || true)
  backup="$dir/.Cellworks.app.old.$$"
  mv "$dest" "$backup" || die "could not move the existing $dest aside (is Cellworks running?)"
fi

# Copy next to the target, then rename: the swap itself is one step.
ditto "$app" "$dir/.Cellworks.app.new.$$" || { rm -rf "$dir/.Cellworks.app.new.$$"; die "could not copy Cellworks.app into $dir"; }
mv "$dir/.Cellworks.app.new.$$" "$dest" || die "could not move Cellworks.app into $dir"
if [ -n "$backup" ]; then rm -rf "$backup"; backup=""; fi

if [ -z "$old" ]; then say "Installed Cellworks $got_version to $dest"
elif [ "$old" = "$got_version" ]; then say "Cellworks $got_version was already installed; reinstalled in $dest"
else say "Upgraded Cellworks $old -> $got_version in $dest"; fi

say ""
say "Next:"
say "  open \"$dest\""
say "Cellworks needs git; claude, gh, nvim, kubectl and cockpit light up more of the map when present."
say "It updates itself from inside the app."
