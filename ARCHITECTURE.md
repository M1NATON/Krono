# Krono — Architecture (карта для быстрой навигации)

Rust workspace, ~39k строк, 70 тестов. Нативный клиент на `eframe/egui` (OpenGL/glow), хранилище — папка файлов + SQLite-кэш, синхронизация — GitHub Git Data API (без локального `git`).

```
krono-core      чистая доменная логика (serde, pulldown-cmark, sha1) — НЕ зависит от UI/диска/сети
krono-desktop   UI + storage + sync + config  → зависит от krono-core
```
Правило зависимостей: `desktop → core`, обратно нельзя.

## krono-core (`krono-core/src/`)
| Файл | Что внутри |
|---|---|
| `note.rs` | `NoteDocument`, `Frontmatter` (title, status, tags, pinned), `NoteMeta`, `NoteStatus`; парсинг/сериализация YAML-frontmatter без потерь |
| `markdown.rs` | `extract_wikilinks`, `extract_headings`, `count_words`, `rewrite_wikilinks` (переименование ссылок, код-блоки пропускаются) |
| `path.rs` | `VaultFileKind` (md / board.json / table.json / canvas / images), `validate_vault_path`, `is_vault_file`, `vault_file_stem`. Отклоняет `..`, абсолютные пути, скрытые сегменты |
| `hash.rs` | `git_blob_sha1` — идентичен `git hash-object` |
| `sync_plan.rs` | `plan_sync(base, local, remote) → SyncPlan{ops, unchanged}`; `SyncOp` = Push / Pull / DeleteLocal / DeleteRemote / Conflict; `count_local_changes` |
| `daily.rs` | `SimpleDate`, сетка месяца, `daily_note_rel_path`, streak, подсчёт записей |
| `collection.rs` | **Кастомные доски/таблицы** (`*.board.json`, `*.table.json`): `CollectionDoc{columns: Vec<ColumnDef>, records: Vec<RecordItem>, views}`; `ColumnType` = Text/Number/Select/Checkbox/Date/NoteLink; `FieldValue`, `SavedCollectionView`, `CollectionViewKind`, `record_matches_smart_filter` |
| `canvas.rs` | **JSON Canvas** (`*.canvas`): `CanvasDoc{nodes, edges}`, `CanvasNodeKind` = Text/File/Link/Group, `CanvasSide`, `CanvasEnd` |

## krono-desktop (`krono-desktop/src/`)
### Точка входа и состояние
- `main.rs` — окно, шрифты, mod-декларации.
- `app.rs` — **`KronoApp`** (единственный владелец состояния, impl `eframe::App`). Ключевые методы:
  - `update` — кадр: `poll_background_events` → `handle_shortcuts` → отрисовка по `ViewMode` → `handle_actions(Vec<UiAction>)`.
  - `handle_actions` — **единый диспетчер**: UI только возвращает `UiAction`, мутирует состояние только `app.rs`.
  - `perform_rename / perform_duplicate / perform_delete`, `save_current_{note,collection,canvas}`, `open_or_create_daily_note`, `*_external_conflict_copy` (внешняя правка открытого файла).
- `shot_runner.rs` — автоскриншоты для проверки адаптивности (debug).
- `config.rs` — `AppConfig` (тема, акцент, язык, vault_dir, GitHub, `md_*` флаги, сниппеты). JSON в `ProjectDirs("io","krono","Krono")`, атомарная запись.
- `i18n.rs` — `Lang{Ru,En}`, `Strings` (~430 полей), статики `RU`/`EN`, `strings(lang)`. Любая новая строка = поле в `Strings` + значение в обеих таблицах.

### Слой UI (`ui/`)
`ui/mod.rs` — общие типы: **`UiAction`** (~60 вариантов), `ViewMode` (Editor, Table, Board, Graph, Overview, Collection, Canvas, Settings), `SettingsTab`, `EditorPrefs`, `LayoutClass` (Desktop ≥900 / Compact / Mobile <600), контекст-структуры `CalendarUiState`, `SyncUiState`, `EditorDocState`, `TopbarDocContext`.

| Область | Файлы |
|---|---|
| Каркас | `sidebar.rs`, `topbar.rs`, `mobile_nav.rs`, `inspector.rs`, `command_palette.rs`, `dialogs.rs` |
| Заметки | `editor_view.rs`, `markdown_editor.rs`, `markdown_assist.rs` (slash `/`, `[[`, авто-списки, обёртки), `markdown_preview.rs`, `calendar_widget.rs` |
| Глобальные обзоры | `table_view.rs`, `board_view.rs` (по `NoteStatus`) |
| Коллекции | `collection_view.rs`, `collection_board.rs`, `embedded_board.rs` (доска внутри заметки) |
| Графика | `viewport_2d.rs` (зум/пан), `graph_view.rs`, `vault_map_view.rs`, `canvas_view.rs`, `image_cache.rs` |
| Тема/виджеты | `theme.rs` (`ThemePalette`, шрифты), `components.rs`, `icons.rs`, `motion.rs` (анимации) |
| Настройки | `settings_view.rs` (вкладки Appearance/Editor/MarkdownHelp/Storage/GitHub/Shortcuts) |

### Хранилище (`storage/`)
- `vault.rs` — файловые операции над Vault: `read/save/create/rename/duplicate/delete` для заметок, коллекций (`*_collection`) и канвасов (`*_canvas`); `rewrite_vault_{wikilinks,collection_links,canvas_links}`; daily-заметки и хабы; ассеты (картинки) и `audit_vault_assets`. Всё через `validate_vault_path`, запись атомарная, LF.
- `index.rs` — **`VaultIndex`** (SQLite): таблицы `notes`, `notes_fts` (FTS5), `links`, `collections`, `assets`, `sync_state`, `meta`. `sync_from_disk` сверяет `(mtime,size)`; индекс — **кэш**, источник правды — файлы.
- `watcher.rs` — поток-поллер (2.2 с), шлёт сигнал → `poll_background_events`.

### Синхронизация (`sync/`)
- `github.rs` — `sync_with_github`: 1) локальные хэши, 2) ref/bootstrap ветки, 3) recursive tree, 4) `plan_sync`, 5) Pull/Conflict пишутся на диск (конфликт → `<stem>.conflict-<ts>.md`), 6) blobs → tree → commit → `PATCH ref`, один коммит.
- `worker.rs` — `SyncWorker` (поток + mpsc, `SyncEvent`), `updater.rs` — проверка и установка релизов GitHub.

## Поток данных
```
Input → ui::render_* → Vec<UiAction> → KronoApp::handle_actions
      → storage::vault (диск, атомарно) → VaultIndex (SQLite-кэш) → reload caches → следующий кадр
Sync:  SyncWorker(thread) → github::sync_with_github → SyncEvent → poll_background_events
```

## Как добавить…
- **Действие UI:** вариант в `UiAction` (`ui/mod.rs`) → отправка из виджета → ветка в `handle_actions` (`app.rs`). Компилятор укажет все `match`.
- **Вкладку настроек:** вариант `SettingsTab` → оба списка навигации и `match` в `render_tab_scroll` (`settings_view.rs`) → строки в `i18n.rs`.
- **Тип файла Vault:** `VaultFileKind` + `from_path` (`path.rs`) → `list_vault_files`/`vault.rs` → `index.rs` → синхронизация подхватит сама (общий предикат `is_vault_file`).
- **Тип столбца коллекции:** `ColumnType` + `FieldValue` (`collection.rs`) → `collection_view.rs` (ячейка) → `i18n::column_type_label`.
- **Настройку:** поле в `AppConfig` (`#[serde(default)]`) → `EditorPrefs`, если влияет на редактор → переключатель в `settings_view.rs` → i18n.

## Инварианты
`unwrap/panic` только в тестах · параметризованный SQL · атомарные записи · LF для хэшируемого контента · все пути `/`-нормализованы · core без UI/IO · тексты через `Lang`.

## Известные слабые места
Монолиты (`app.rs` ~4k, `canvas_view.rs` ~2.9k, `settings_view.rs` ~2.2k, `collection_view.rs` ~2k строк) · функции рендера с 16–23 параметрами · поллинг watcher'а · токен GitHub в `config.json` открытым текстом · локальная запись раньше коммита при sync.
