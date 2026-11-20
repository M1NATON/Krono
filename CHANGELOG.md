# Changelog

## 0.2.0-beta.1 (2026-11-20)

First public cross-platform beta of Krono (`Windows`, `Linux`, `macOS Apple Silicon`, `macOS Intel`).

### Highlights
- **Markdown Vault & Index**: Local-first `.md` storage with lossless YAML frontmatter (`order`, `pinned`, `status`, `tags`), automatic `[[wikilink]]` rewriting on rename, soft delete to `.trash/`, and self-healing SQLite FTS5 index.
- **Interactive Markdown Editor**: Edit / Split / Read modes, slash (`/`) block menu, custom snippets, `[[wikilink]]` autocomplete, selection wrapping, inline task due-date/priority badges (`@YYYY-MM-DD`, `@today`, `!high`/`!med`/`!low`), and one-click send/move tasks to Kanban boards.
- **Navigation History & Quick Tabs**: `Back` / `Forward` (`Alt+ArrowLeft` / `Alt+ArrowRight`) and closable recent tabs strip.
- **Built-in Kanban, Custom Boards & Tables**: Drag-and-drop reordering with smooth slot and settle animations, customizable fields, smart filters (overdue/today/week/tags/select), and saved collection views.
- **Knowledge Graph, Infinite Canvas & Vault Overview Map**: Force-directed local & global graph, infinite Canvas (cards, notes, images via picker/Ctrl+V, groups, styled arrows, Marquee multi-select, Smart Alignment Guides, mini-map, Markdown export), and folder-grouped Vault Overview map.
- **Diary Hierarchy & Asset Audit**: Year/month diary and diary-board organization (`daily/YYYY/MM`, `daily/boards/YYYY/MM`), monthly calendar widget, orphan attachment scanner & cleanup under **Settings -> Storage**, and optional 3-way private GitHub Sync.
