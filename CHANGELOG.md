# Changelog

## 0.3.1 (2026-11-22)

### Highlights
- **Media Library 2.0**: компактная 2-строчная шапка, Spotlight-модалка добавления (TMDB для кино/сериалов + OpenLibrary для книг + ручной ввод), защита от дублей, 2-колоночный просмотр карточки с постером и адаптивными полями, случайный подбор «Подобрать».
- **Standalone Floating Scratchpad**: независимый always-on-top viewport поверх всех окон без разворачивания главного окна Krono, перетаскивание за шапку, глобальный хоткей (`Ctrl+Shift+Space` с fallback).
- **Settings 2.0 & Customizable Keybindings**: 3 логические группы настроек, живой поиск по настройкам (RU/EN), настраиваемые горячие клавиши с детектором и подсветкой конфликтов.
- **UX & Navigation**: сохранение фильтров в `.krono/ui_state.json`, архив выполненных задач с датой (`✅ YYYY-MM-DD`), кириллический поиск/автодополнение тегов, полная клавиатурная навигация (`Ctrl+Tab`, `Ctrl+1..9`, `Alt+1..3`, стрелки в сайдбаре/доске/галерее).
- **Editor, Performance & Release Automation**: фикс выделения больших участков текста и автоскролла в редакторе, оптимизация SQLite FTS5 и графа (spatial hash grid), автоматическое определение версии приложения из Git-тега (`KRONO_APP_VERSION`).

## 0.2.0-beta.1 (2026-11-20)

First public cross-platform beta of Krono (`Windows`, `Linux`, `macOS Apple Silicon`, `macOS Intel`).

### Highlights
- **Markdown Vault & Index**: Local-first `.md` storage with lossless YAML frontmatter (`order`, `pinned`, `status`, `tags`), automatic `[[wikilink]]` rewriting on rename, soft delete to `.trash/`, and self-healing SQLite FTS5 index.
- **Interactive Markdown Editor**: Edit / Split / Read modes, slash (`/`) block menu, custom snippets, `[[wikilink]]` autocomplete, selection wrapping, inline task due-date/priority badges (`@YYYY-MM-DD`, `@today`, `!high`/`!med`/`!low`), and one-click send/move tasks to Kanban boards.
- **Navigation History & Quick Tabs**: `Back` / `Forward` (`Alt+ArrowLeft` / `Alt+ArrowRight`) and closable recent tabs strip.
- **Built-in Kanban, Custom Boards & Tables**: Drag-and-drop reordering with smooth slot and settle animations, customizable fields, smart filters (overdue/today/week/tags/select), and saved collection views.
- **Knowledge Graph, Infinite Canvas & Vault Overview Map**: Force-directed local & global graph, infinite Canvas (cards, notes, images via picker/Ctrl+V, groups, styled arrows, Marquee multi-select, Smart Alignment Guides, mini-map, Markdown export), and folder-grouped Vault Overview map.
- **Diary Hierarchy & Asset Audit**: Year/month diary and diary-board organization (`daily/YYYY/MM`, `daily/boards/YYYY/MM`), monthly calendar widget, orphan attachment scanner & cleanup under **Settings -> Storage**, and optional 3-way private GitHub Sync.
