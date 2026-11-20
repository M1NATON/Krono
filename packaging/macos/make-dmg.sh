#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 4 ]]; then
  echo "Usage: $0 <binary_path> <version> <min_macos> <output_dmg>" >&2
  exit 1
fi

BIN_PATH="$1"
VERSION="$2"
MIN_MACOS="$3"
OUT_DMG="$4"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ICON_PNG="${REPO_ROOT}/packaging/icons/krono.png"
PLIST_TPL="${REPO_ROOT}/packaging/macos/Info.plist"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

ICONSET_DIR="${WORK_DIR}/Krono.iconset"
mkdir -p "$ICONSET_DIR"
for sz in 16 32 128 256 512; do
  sips -z "$sz" "$sz" "$ICON_PNG" --out "${ICONSET_DIR}/icon_${sz}x${sz}.png" >/dev/null
  sz2=$((sz * 2))
  sips -z "$sz2" "$sz2" "$ICON_PNG" --out "${ICONSET_DIR}/icon_${sz}x${sz}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET_DIR" -o "${WORK_DIR}/Krono.icns"

STAGING_DIR="${WORK_DIR}/dmg_root"
APP_DIR="${STAGING_DIR}/Krono.app"
mkdir -p "${APP_DIR}/Contents/MacOS" "${APP_DIR}/Contents/Resources"

cp "$BIN_PATH" "${APP_DIR}/Contents/MacOS/krono-desktop"
chmod +x "${APP_DIR}/Contents/MacOS/krono-desktop"
cp "${WORK_DIR}/Krono.icns" "${APP_DIR}/Contents/Resources/Krono.icns"

CLEAN_VER="${VERSION#v}"
sed -e "s/__VERSION__/${CLEAN_VER}/g" -e "s/__MIN_MACOS__/${MIN_MACOS}/g" \
  "$PLIST_TPL" > "${APP_DIR}/Contents/Info.plist"

codesign --force --deep --sign - "$APP_DIR"
ln -s /Applications "${STAGING_DIR}/Applications"

mkdir -p "$(dirname "$OUT_DMG")"
rm -f "$OUT_DMG"
hdiutil create -volname "Krono" -srcfolder "$STAGING_DIR" -ov -format UDZO "$OUT_DMG" >/dev/null

echo "Created ${OUT_DMG}"
