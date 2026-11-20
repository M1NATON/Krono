#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <binary_path> <output_appimage>" >&2
  exit 1
fi

BIN_PATH="$1"
OUT_APPIMAGE="$2"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

APPDIR="${WORK_DIR}/Krono.AppDir"
mkdir -p \
  "${APPDIR}/usr/bin" \
  "${APPDIR}/usr/share/applications" \
  "${APPDIR}/usr/share/icons/hicolor/512x512/apps"

cp "$BIN_PATH" "${APPDIR}/usr/bin/krono-desktop"
chmod +x "${APPDIR}/usr/bin/krono-desktop"

cp "${REPO_ROOT}/packaging/linux/krono.desktop" "${APPDIR}/krono.desktop"
cp "${REPO_ROOT}/packaging/linux/krono.desktop" "${APPDIR}/usr/share/applications/krono.desktop"
cp "${REPO_ROOT}/packaging/icons/krono.png" "${APPDIR}/krono.png"
cp "${REPO_ROOT}/packaging/icons/krono.png" "${APPDIR}/.DirIcon"
cp "${REPO_ROOT}/packaging/icons/krono.png" "${APPDIR}/usr/share/icons/hicolor/512x512/apps/krono.png"

cat > "${APPDIR}/AppRun" <<'EOF'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "$0")")"
exec "${HERE}/usr/bin/krono-desktop" "$@"
EOF
chmod +x "${APPDIR}/AppRun"

TOOL="${WORK_DIR}/appimagetool-x86_64.AppImage"
curl -fsSL \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
  -o "$TOOL"
chmod +x "$TOOL"

mkdir -p "$(dirname "$OUT_APPIMAGE")"
rm -f "$OUT_APPIMAGE"
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT_APPIMAGE"
chmod +x "$OUT_APPIMAGE"

echo "Created ${OUT_APPIMAGE}"
