#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BIN_SRC="${SCRIPT_DIR}/krono-desktop"
DESKTOP_SRC="${SCRIPT_DIR}/krono.desktop"
ICON_SRC="${SCRIPT_DIR}/krono.png"

if [[ ! -f "$BIN_SRC" && -f "${SCRIPT_DIR}/../../target/release/krono-desktop" ]]; then
  BIN_SRC="${SCRIPT_DIR}/../../target/release/krono-desktop"
fi
if [[ ! -f "$ICON_SRC" && -f "${SCRIPT_DIR}/../icons/krono.png" ]]; then
  ICON_SRC="${SCRIPT_DIR}/../icons/krono.png"
fi

BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/512x512/apps"

mkdir -p "$BIN_DIR" "$APP_DIR" "$ICON_DIR"
install -m 755 "$BIN_SRC" "${BIN_DIR}/krono-desktop"
install -m 644 "$DESKTOP_SRC" "${APP_DIR}/krono.desktop"
install -m 644 "$ICON_SRC" "${ICON_DIR}/krono.png"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" || true
fi

echo "Krono installed to ${BIN_DIR}/krono-desktop"
echo "Make sure ${HOME}/.local/bin is in your PATH."
