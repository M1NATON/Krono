# Krono

Local-first Markdown knowledge workspace built in Rust (`eframe`/`egui` + SQLite FTS5) — combining notes, Kanban boards, custom tables, an infinite Canvas, a Knowledge Graph, a global Vault Overview map, Diary calendar & optional private GitHub Sync.

> **Status:** Public Beta (`v0.2.0-beta.1`) — native builds for Windows, macOS (Apple Silicon & Intel), and Linux.

---

## Key Features

- **Plain-file Markdown Vault + Self-Healing SQLite Index**: Notes live as regular `.md` files with lossless YAML frontmatter (`title`, `status`, `tags`, `pinned`, `order`, plus custom fields). FTS5 full-text search, instant backlinks, automatic `[[wikilink]]` rewriting on rename, and soft delete into `.trash/`.
- **Interactive Markdown Editor**: Edit, Split, and Read modes with cursor-anchored Slash (`/`) command menu, custom snippets, `[[wikilink]]` autocomplete, inline task badges (`@YYYY-MM-DD`, `@today`, `!high`/`!med`/`!low`), and one-click copy/move of Markdown tasks onto any Kanban board.
- **Built-in Kanban & Custom Boards / Tables**: Drag-and-drop card reordering (`order` persisted cleanly), animated drop slots, custom columns (`Select`, `Date`, `Checkbox`, `Number`, `NoteLink`), smart date/tag filters, and persistent **Saved Views**.
- **Infinite Canvas & Knowledge Graph**:
  - Cards, linked files, groups, clipboard image paste (`Ctrl/Cmd+V`), curved labeled edges, `Ctrl+F` search, Marquee multi-select (`Shift+Drag` or `V`), Smart Alignment Guides, mini-map, undo/redo, and linear `.md` export.
  - Global & local force-directed Knowledge Graph + spatial **Vault Overview** (`Обзор`) grouping folders and links across the vault.
- **Diary Hierarchy & Attachment Audit**: Structured `daily/YYYY/MM/` diary notes and `daily/boards/YYYY/MM/` task boards, monthly calendar & streak counter, plus unused-asset (`assets/`) scanner and cleanup under **Settings -> Storage**.
- **Navigation History & Quick Tabs**: Browser-like `Back` / `Forward` (`Alt+←` / `Alt+→`) and scrollable recent tabs strip.
- **Optional Private GitHub Sync**: Git Data API 3-way merge with LF normalization before SHA-1 hashing, SQLite `blob_sha` tracking, case-collision protection, and non-destructive conflict resolution.
- **Themes & Languages**: Curated dark & light themes (`Graphite`, `Paper`, `Obsidian OLED`, `Editorial Sand`, `Warm Espresso`, `Everforest`), accent presets, and bilingual UI (`RU` / `EN`).

---

## Downloads & Installation (Beta)

Pre-built binaries are published on the [GitHub Releases](../../releases) page:

- **Windows (`x86_64`)**: Download `krono-<tag>-windows-x86_64.zip`, extract, and launch `krono-desktop.exe`.
- **macOS (Apple Silicon `arm64` & Intel `x86_64`)**: Download the matching `.dmg` (`macos-arm64.dmg` for M-series Macs, `macos-x86_64.dmg` for Intel Macs) and drag **Krono.app** into **Applications**.
  - *Unsigned beta note:* Builds are ad-hoc signed. If macOS Gatekeeper blocks the first launch, run in Terminal:
    ```sh
    xattr -cr /Applications/Krono.app
    ```
- **Linux (`x86_64`)**: Download `krono-<tag>-linux-x86_64.tar.gz`, extract, and run:
  ```sh
  ./install.sh
  ```
  This installs `krono-desktop` into `~/.local/bin`, adds the desktop launcher, and registers the application icon. Make sure runtime libraries (`libxkbcommon`, `mesa`/`libGL`, `wayland`/`libX11`) are installed on your distribution.

---

## Keyboard Shortcuts

| Action | Windows / Linux | macOS |
| --- | --- | --- |
| Command Palette | `Ctrl+K` | `Cmd+K` |
| New Item Modal | `Ctrl+N` | `Cmd+N` |
| Today's Daily Note | `Ctrl+D` | `Cmd+D` |
| Back / Forward | `Alt+←` / `Alt+→` | `Option+←` / `Option+→` |
| Save Active Document | `Ctrl+S` | `Cmd+S` |
| GitHub Sync Now | `Ctrl+Shift+S` | `Cmd+Shift+S` |
| Knowledge Graph | `Ctrl+G` | `Cmd+G` |
| Canvas Marquee Mode | `V` (or `Shift+Drag`) | `V` (or `Shift+Drag`) |
| Toggle Sidebar | `Ctrl+\` | `Cmd+\` |
| Toggle Inspector | `Ctrl+Shift+I` | `Cmd+Shift+I` |
| Settings | `Ctrl+,` | `Cmd+,` |

---

## Build From Source

Prerequisites: [Rust toolchain (`stable`)](https://rustup.rs/).

### Windows / macOS
```sh
cargo test --workspace --locked
cargo build --release --locked -p krono-desktop
```

### Linux (Ubuntu / Debian dependencies)
```sh
sudo apt-get install -y pkg-config libxkbcommon-dev libwayland-dev libx11-dev \
  libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libgl1-mesa-dev libegl1-mesa-dev
cargo test --workspace --locked
cargo build --release --locked -p krono-desktop
```

### Cross-Compile macOS Intel on Apple Silicon
```sh
rustup target add x86_64-apple-darwin
MACOSX_DEPLOYMENT_TARGET=10.15 cargo build --release --locked --target x86_64-apple-darwin -p krono-desktop
```

---

## Publishing a Release

GitHub Actions workflows live in [.github/workflows](.github/workflows):
- `ci.yml` builds and tests every push/PR to `main` across Windows, Linux, macOS Apple Silicon, and macOS Intel.
- `release.yml` packages `.zip` (Windows), `.tar.gz` (Linux), and `.dmg` (macOS `arm64` & `x86_64`) with `SHA256SUMS.txt` automatically when pushing a version tag:

```sh
git tag v0.2.0-beta.1
git push origin v0.2.0-beta.1
```

---

## License

Licensed under the [MIT License](LICENSE).
