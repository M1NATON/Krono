use crate::app::{today_local, KronoApp};
use crate::config::{AccentPreset, ThemePreset};
use crate::i18n::Lang;
use crate::storage::{
    index::{CollectionMeta, RawLinkEdge},
    vault::AssetAuditEntry,
};
use crate::ui::canvas_view::CanvasEditorState;
use crate::ui::collection_view::CollectionEditorState;
use crate::ui::graph_view::GraphState;
use crate::ui::vault_map_view::VaultMapState;
use crate::ui::{
    CreateItemKind, EditorMode, NavTarget, PagesKindFilter, PagesSortOrder, SettingsTab, ViewMode,
};
use eframe::egui;
use krono_core::{
    canvas::{CanvasDoc, CanvasEnd, CanvasNodeKind, CanvasSide},
    collection::{
        CollectionDoc, CollectionViewKind, ColumnType, DateFilterPreset, FieldValue,
        SavedCollectionView,
    },
    daily::SimpleDate,
    note::{Frontmatter, NoteDocument, NoteMeta, NoteStatus},
    path::VaultFileKind,
};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShotMode {
    Stage5,
    Polish,
    Features,
    Mega,
}

pub(crate) struct ShotRunner {
    pub(crate) out_dir: PathBuf,
    pub(crate) mode: ShotMode,
    pub(crate) step: usize,
    pub(crate) frames_in_step: usize,
    pub(crate) requested: bool,
}

impl ShotRunner {
    pub(crate) fn from_env() -> Option<Self> {
        let dir = std::env::var_os("KRONO_SHOT_DIR")?;
        if dir.is_empty() {
            return None;
        }
        let out_dir = PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&out_dir);
        let mode = match std::env::var("KRONO_SHOT_MODE").ok().as_deref() {
            Some("polish") => ShotMode::Polish,
            Some("features") => ShotMode::Features,
            Some("mega") => ShotMode::Mega,
            _ => ShotMode::Stage5,
        };
        Some(Self {
            out_dir,
            mode,
            step: 0,
            frames_in_step: 0,
            requested: false,
        })
    }
}

fn demo_preview_color_image() -> egui::ColorImage {
    let w = 360usize;
    let h = 190usize;
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let mut r = (24.0 + fx * 38.0 + (1.0 - fy) * 18.0) as u8;
            let mut g = (34.0 + fy * 46.0 + fx * 22.0) as u8;
            let mut b = (62.0 + (1.0 - fx) * 44.0 + fy * 34.0) as u8;

            let in_box1 = (26..135).contains(&x) && (36..108).contains(&y);
            let in_box2 = (188..324).contains(&x) && (64..148).contains(&y);
            let on_line = (135..=188).contains(&x)
                && (y as isize - (72 + (x as isize - 135) * 34 / 53)).abs() <= 2;
            if in_box1 || in_box2 {
                r = r.saturating_add(36);
                g = g.saturating_add(58);
                b = b.saturating_add(96);
            }
            if on_line {
                r = 120;
                g = 185;
                b = 255;
            }
            pixels.push(egui::Color32::from_rgb(r, g, b));
        }
    }
    egui::ColorImage {
        size: [w, h],
        pixels,
    }
}

fn demo_features_canvas() -> (CanvasDoc, String) {
    let mut doc = CanvasDoc {
        title: "Функции Canvas".into(),
        pinned: true,
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    doc.add_group_node(-520.0, -170.0, 720.0, 320.0, "Группа слоёв", Some("6"));
    let c1 = doc.add_text_node(
        -490.0,
        -112.0,
        "### Приоритет слоёв\nСтрелки внутри группы теперь выбираются с первого клика.",
        Some("5"),
    );
    let c2 = doc.add_text_node(
        -80.0,
        -25.0,
        "### Связи и поиск\nCtrl+F ищет карточки, а двойные стрелки меняют направление.",
        Some("2"),
    );
    let img_node = doc.add_file_node(310.0, -135.0, "assets/demo.png", Some("1"));
    if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == img_node) {
        n.width = 270.0;
        n.height = 195.0;
    }

    let edge_bi = doc
        .add_edge(
            &c1,
            Some(CanvasSide::Right),
            &c2,
            Some(CanvasSide::Left),
            Some("Двусторонняя связь"),
        )
        .unwrap_or_default();
    if let Some(edge) = doc.edges.iter_mut().find(|e| e.id == edge_bi) {
        edge.from_end = Some(CanvasEnd::Arrow);
        edge.to_end = Some(CanvasEnd::Arrow);
        edge.color = Some("4".into());
    }
    doc.add_edge(
        &c2,
        Some(CanvasSide::Right),
        &img_node,
        Some(CanvasSide::Left),
        Some("Превью PNG"),
    );

    (doc, edge_bi)
}

fn demo_storyboard_canvas() -> CanvasDoc {
    let mut doc = CanvasDoc {
        title: "Сценарий выпуска #12".into(),
        pinned: true,
        nodes: Vec::new(),
        edges: Vec::new(),
    };

    doc.add_group_node(-520.0, -170.0, 290.0, 290.0, "01. Хук", Some("1"));
    doc.add_group_node(-60.0, -170.0, 310.0, 320.0, "02. Архитектура", Some("6"));
    doc.add_group_node(420.0, -170.0, 290.0, 290.0, "03. Финал", Some("4"));

    let hook = doc.add_text_node(
        -498.0,
        -118.0,
        "### 00:00 — Проблема\nПочему Electron-редакторы едят 600 МБ ОЗУ и тормозят при старте.",
        Some("1"),
    );
    let core_file = doc.add_file_node(-34.0, -118.0, "architecture.md", Some("2"));
    let graph_note = doc.add_text_node(
        -34.0,
        24.0,
        "### Демо графа и Canvas\nПоказываем [[Дорожная карта]] и live-открытие заметок на OpenGL 60 fps.",
        Some("5"),
    );
    let outro = doc.add_text_node(
        442.0,
        -80.0,
        "### 06:40 — Итоги\n- [x] Экспорт цепочки в Markdown\n- [ ] Ссылка на GitHub в описании",
        Some("4"),
    );

    doc.add_edge(
        &hook,
        Some(CanvasSide::Right),
        &core_file,
        Some(CanvasSide::Left),
        Some("Переход к делу"),
    );
    doc.add_edge(
        &core_file,
        Some(CanvasSide::Bottom),
        &graph_note,
        Some(CanvasSide::Top),
        None,
    );
    doc.add_edge(
        &graph_note,
        Some(CanvasSide::Right),
        &outro,
        Some(CanvasSide::Left),
        Some("Сборка сценария"),
    );

    doc
}

impl KronoApp {
    fn seed_stage5_demo(&mut self) {
        self.notes = vec![
            NoteMeta {
                path: "welcome.md".into(),
                title: "Добро пожаловать в Krono".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["krono".into(), "pkm".into()],
                excerpt: "Нативное рабочее пространство для заметок на Rust и OpenGL.".into(),
                modified: 100,
                word_count: 140,
                pinned: true,
                board_order: 0,
            },
            NoteMeta {
                path: "architecture.md".into(),
                title: "Architecture".into(),
                status: Some(NoteStatus::Done),
                tags: vec!["rust".into(), "sqlite".into()],
                excerpt: "OpenGL eframe, SQLite FTS5 и прямая 3-way синхронизация GitHub.".into(),
                modified: 110,
                word_count: 185,
                pinned: true,
                board_order: 0,
            },
            NoteMeta {
                path: "projects/roadmap.md".into(),
                title: "Дорожная карта".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["planning".into()],
                excerpt: "Этапы 1–6: кастомные доски, граф связей, Canvas и PWA.".into(),
                modified: 95,
                word_count: 90,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "tech/sqlite-fts5.md".into(),
                title: "SQLite FTS5".into(),
                status: Some(NoteStatus::Done),
                tags: vec!["sqlite".into(), "rust".into()],
                excerpt: "Мгновенный полнотекстовый индекс и обратные ссылки за O(1).".into(),
                modified: 88,
                word_count: 110,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "tech/git-sync.md".into(),
                title: "Git Data API".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["github".into(), "rust".into()],
                excerpt: "Blobs + Trees + Commits без локального git и VPS.".into(),
                modified: 84,
                word_count: 130,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "daily/2026-11-19.md".into(),
                title: "19 ноября 2026".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["daily".into()],
                excerpt: "Тестирование локального мини-графа и экспорта Canvas в сценарий.".into(),
                modified: 120,
                word_count: 65,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "ideas/orphan-note.md".into(),
                title: "Черновик идей".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["ideas".into()],
                excerpt: "Отдельная заметка без входящих и исходящих ссылок.".into(),
                modified: 40,
                word_count: 35,
                pinned: false,
                board_order: 0,
            },
        ];

        self.collections = vec![
            CollectionMeta {
                path: "youtube/video_script.canvas".into(),
                kind: VaultFileKind::Canvas,
                title: "Сценарий выпуска #12".into(),
                item_count: 7,
                column_count: 3,
                excerpt: "01. Хук, Architecture, 03. Финал".into(),
                modified: 125,
                pinned: true,
            },
            CollectionMeta {
                path: "youtube/plan.board.json".into(),
                kind: VaultFileKind::Board,
                title: "Контент-план YouTube".into(),
                item_count: 5,
                column_count: 4,
                excerpt: "Разбор Krono, Git Data API".into(),
                modified: 115,
                pinned: false,
            },
        ];

        self.all_links = vec![
            RawLinkEdge { source_path: "welcome.md".into(), target_raw: "Architecture".into() },
            RawLinkEdge { source_path: "welcome.md".into(), target_raw: "Дорожная карта".into() },
            RawLinkEdge { source_path: "architecture.md".into(), target_raw: "SQLite FTS5".into() },
            RawLinkEdge { source_path: "architecture.md".into(), target_raw: "Git Data API".into() },
            RawLinkEdge { source_path: "projects/roadmap.md".into(), target_raw: "Architecture".into() },
            RawLinkEdge { source_path: "tech/sqlite-fts5.md".into(), target_raw: "Дорожная карта".into() },
            RawLinkEdge { source_path: "tech/git-sync.md".into(), target_raw: "Architecture".into() },
            RawLinkEdge { source_path: "daily/2026-11-19.md".into(), target_raw: "Добро пожаловать в Krono".into() },
            RawLinkEdge { source_path: "daily/2026-11-19.md".into(), target_raw: "Сценарий выпуска #12".into() },
            RawLinkEdge { source_path: "youtube/video_script.canvas".into(), target_raw: "architecture.md".into() },
            RawLinkEdge { source_path: "youtube/video_script.canvas".into(), target_raw: "Дорожная карта".into() },
            RawLinkEdge { source_path: "youtube/plan.board.json".into(), target_raw: "Architecture".into() },
        ];

        self.expanded_folders.insert("youtube".into());
        self.expanded_folders.insert("projects".into());
        self.expanded_folders.insert("tech".into());
        self.expanded_folders.insert("daily".into());

        self.graph_state = GraphState::default();
        self.graph_state
            .rebuild(&self.notes, &self.collections, &self.all_links);

        let mut canvas_st = CanvasEditorState::default();
        canvas_st.path = Some("youtube/video_script.canvas".into());
        canvas_st.doc = demo_storyboard_canvas();
        canvas_st.needs_fit = true;
        self.active_canvas = canvas_st;
    }

    fn seed_polish_demo(&mut self) {
        self.notes = vec![
            NoteMeta {
                path: "welcome.md".into(),
                title: "Добро пожаловать в Krono".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["krono".into(), "pkm".into()],
                excerpt: "Нативное рабочее пространство для заметок, досок и холстов.".into(),
                modified: 200,
                word_count: 140,
                pinned: true,
                board_order: 0,
            },
            NoteMeta {
                path: "architecture.md".into(),
                title: "Architecture".into(),
                status: Some(NoteStatus::Done),
                tags: vec!["rust".into(), "sqlite".into()],
                excerpt: "OpenGL eframe, SQLite FTS5 и прямая 3-way синхронизация GitHub.".into(),
                modified: 190,
                word_count: 185,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "projects/roadmap.md".into(),
                title: "Дорожная карта".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["planning".into()],
                excerpt: "Этапы 1–6: кастомные доски, граф связей, Canvas и PWA.".into(),
                modified: 180,
                word_count: 90,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "daily/2026/2026.md".into(),
                title: "Дневник 2026".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["daily".into(), "year".into()],
                excerpt: "Годовой хаб дневника: [[Ноябрь 2026]]".into(),
                modified: 150,
                word_count: 42,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "daily/2026/11/2026-11.md".into(),
                title: "Ноябрь 2026".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["daily".into(), "month".into()],
                excerpt: "Месяц дневника в [[Дневник 2026]].".into(),
                modified: 175,
                word_count: 68,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "daily/2026/11/2026-11-19.md".into(),
                title: "19 ноября 2026".into(),
                status: Some(NoteStatus::Done),
                tags: vec!["daily".into()],
                excerpt: "Месяц: [[Ноябрь 2026]] • Граф и холст проверены.".into(),
                modified: 170,
                word_count: 74,
                pinned: false,
                board_order: 0,
            },
            NoteMeta {
                path: "daily/2026/11/2026-11-20.md".into(),
                title: "20 ноября 2026".into(),
                status: Some(NoteStatus::Todo),
                tags: vec!["daily".into()],
                excerpt: "Месяц: [[Ноябрь 2026]] • Обзор [[Сценарий выпуска #12]].".into(),
                modified: 210,
                word_count: 58,
                pinned: false,
                board_order: 0,
            },
        ];

        self.collections = vec![
            CollectionMeta {
                path: "youtube/video_script.canvas".into(),
                kind: VaultFileKind::Canvas,
                title: "Сценарий выпуска #12".into(),
                item_count: 7,
                column_count: 3,
                excerpt: "01. Хук, Architecture, 03. Финал".into(),
                modified: 205,
                pinned: true,
            },
            CollectionMeta {
                path: "youtube/plan.board.json".into(),
                kind: VaultFileKind::Board,
                title: "Контент-план YouTube".into(),
                item_count: 5,
                column_count: 4,
                excerpt: "Разбор Krono, Git Data API".into(),
                modified: 185,
                pinned: false,
            },
            CollectionMeta {
                path: "projects/tracker.table.json".into(),
                kind: VaultFileKind::Table,
                title: "Трекер задач".into(),
                item_count: 6,
                column_count: 5,
                excerpt: "Спринт, статус и приоритет релизов.".into(),
                modified: 1_795_100_000,
                pinned: false,
            },
        ];

        self.all_links = vec![
            RawLinkEdge { source_path: "daily/2026/11/2026-11-19.md".into(), target_raw: "Ноябрь 2026".into() },
            RawLinkEdge { source_path: "daily/2026/11/2026-11-20.md".into(), target_raw: "Ноябрь 2026".into() },
            RawLinkEdge { source_path: "daily/2026/11/2026-11.md".into(), target_raw: "Дневник 2026".into() },
            RawLinkEdge { source_path: "daily/2026/11/2026-11-20.md".into(), target_raw: "Сценарий выпуска #12".into() },
            RawLinkEdge { source_path: "welcome.md".into(), target_raw: "Architecture".into() },
            RawLinkEdge { source_path: "architecture.md".into(), target_raw: "Дорожная карта".into() },
            RawLinkEdge { source_path: "youtube/video_script.canvas".into(), target_raw: "architecture.md".into() },
        ];

        self.daily_dates.clear();
        if let Some(d) = SimpleDate::from_ymd(2026, 11, 19) {
            self.daily_dates.insert(d);
        }
        if let Some(d) = SimpleDate::from_ymd(2026, 11, 20) {
            self.daily_dates.insert(d);
        }

        self.expanded_folders.insert("daily".into());
        self.expanded_folders.insert("daily/2026".into());
        self.expanded_folders.insert("daily/2026/11".into());
        self.expanded_folders.insert("youtube".into());
        self.expanded_folders.insert("projects".into());

        self.selected_path = Some("welcome.md".into());
        self.current_doc = NoteDocument {
            frontmatter: Frontmatter {
                title: "Добро пожаловать в Krono".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["krono".into(), "pkm".into()],
                pinned: true,
                board_order: None,
                extra_lines: Vec::new(),
            },
            body: "## Рабочее пространство\n\n- [x] Быстрая иерархия дневника по годам и месяцам\n- [x] Модальное окно создания с пресетами\n- [ ] Этап 6: кроссплатформенный krono-web\n\nСмотри также [[Architecture]] и [[Дорожная карта]].\n".into(),
        };
        self.dirty = false;

        self.graph_state = GraphState::default();
        self.graph_state
            .rebuild(&self.notes, &self.collections, &self.all_links);

        let mut canvas_st = CanvasEditorState::default();
        canvas_st.path = Some("youtube/video_script.canvas".into());
        canvas_st.doc = demo_storyboard_canvas();
        canvas_st.show_minimap = true;
        canvas_st.needs_fit = true;
        self.active_canvas = canvas_st;
    }

    fn seed_features_demo(&mut self, ctx: &egui::Context) {
        self.seed_polish_demo();
        self.expanded_folders.insert("daily/boards".into());
        self.collections.push(CollectionMeta {
            path: "projects/tasks.board.json".into(),
            kind: VaultFileKind::Board,
            title: "Задачи релиза".into(),
            item_count: 4,
            column_count: 4,
            excerpt: "Связанный канбан внутри заметки.".into(),
            modified: 220,
            pinned: false,
        });
        self.collections.push(CollectionMeta {
            path: "daily/boards/20-noyabrya-2026-zadachi.board.json".into(),
            kind: VaultFileKind::Board,
            title: "20 ноября 2026 — Задачи".into(),
            item_count: 4,
            column_count: 4,
            excerpt: "Задачи дня в отдельной папке дневника.".into(),
            modified: 230,
            pinned: false,
        });

        let mut demo_board = CollectionDoc::new_board_preset("Задачи релиза", "kanban", true);
        let stage_col = demo_board
            .effective_group_column_id()
            .unwrap_or("stage")
            .to_string();
        let opt_ids: Vec<String> = demo_board
            .columns
            .iter()
            .find(|c| c.id == stage_col)
            .map(|c| c.options.iter().map(|o| o.id.clone()).collect())
            .unwrap_or_default();
        if opt_ids.len() >= 3 {
            let r1 = demo_board.add_record(
                "Сверстать быстрый + ввод карточки",
                Some((&stage_col, &opt_ids[0])),
                100,
            );
            let r2 = demo_board.add_record(
                "Сортировка вверх/вниз и FLIP-анимации",
                Some((&stage_col, &opt_ids[0])),
                110,
            );
            let _r3 = demo_board.add_record(
                "Вставка картинок через Ctrl+V",
                Some((&stage_col, &opt_ids[1])),
                120,
            );
            let _r4 = demo_board.add_record(
                "Разделить дневник и папку Доски",
                Some((&stage_col, &opt_ids[opt_ids.len() - 1])),
                130,
            );
            if let Some(nl_col) = demo_board
                .columns
                .iter()
                .find(|c| c.col_type == krono_core::collection::ColumnType::NoteLink)
                .map(|c| c.id.clone())
            {
                if let Some(rec) = demo_board.records.iter_mut().find(|r| r.id == r1) {
                    rec.values.insert(
                        nl_col.clone(),
                        krono_core::collection::FieldValue::Text("projects/release.md".into()),
                    );
                }
                if let Some(rec) = demo_board.records.iter_mut().find(|r| r.id == r2) {
                    rec.tags.push("ui".into());
                }
            }
        }

        self.image_cache
            .insert_demo_texture(ctx, "assets/demo.png", demo_preview_color_image());

        self.selected_path = Some("projects/release.md".into());
        self.current_doc = NoteDocument {
            frontmatter: Frontmatter {
                title: "Релиз Krono v0.3".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["krono".into(), "release".into()],
                pinned: true,
                board_order: None,
                extra_lines: Vec::new(),
            },
            body: "## Визуальный обзор\n\n![Архитектура Krono](assets/demo.png)\n\n## Связанная доска задач\n\n![[projects/tasks.board.json]]\n\n- [x] Контраст модального окна\n- [x] Приоритет слоёв и связи Canvas\n".into(),
        };
        self.dirty = false;
        self.refresh_document_derivations();
        self.embedded_boards.clear();
        self.embedded_boards
            .insert("projects/tasks.board.json".into(), demo_board.clone());

        let mut col_st = CollectionEditorState::default();
        col_st.path = Some("daily/boards/20-noyabrya-2026-zadachi.board.json".into());
        col_st.active_view = CollectionViewKind::Board;
        demo_board.title = "20 ноября 2026 — Задачи".into();
        col_st.doc = demo_board;
        if let Some(first_opt) = opt_ids.first() {
            col_st.quick_add_col_opt = Some(first_opt.clone());
            col_st.quick_add_at_top = true;
            col_st.quick_add_title = "Проверить Ctrl+V".into();
        }
        self.active_collection = col_st;

        let (feat_canvas, sel_edge_id) = demo_features_canvas();
        let mut canvas_st = CanvasEditorState::default();
        canvas_st.path = Some("youtube/features.canvas".into());
        canvas_st.doc = feat_canvas;
        canvas_st.selected_edge_id = Some(sel_edge_id);
        canvas_st.edge_label_draft = "Двусторонняя связь".into();
        canvas_st.search_open = true;
        canvas_st.search_query = "связ".into();
        canvas_st.show_minimap = true;
        canvas_st.needs_fit = true;
        self.active_canvas = canvas_st;
    }

    fn seed_mega_demo(&mut self, ctx: &egui::Context) {
        self.seed_features_demo(ctx);

        // Organize diary boards into daily/boards/YYYY/MM/ and add a 2nd diary month folder
        if let Some(c) = self
            .collections
            .iter_mut()
            .find(|c| c.path == "daily/boards/20-noyabrya-2026-zadachi.board.json")
        {
            c.path = "daily/boards/2026/11/20-noyabrya-2026-zadachi.board.json".into();
        }
        self.collections.push(CollectionMeta {
            path: "daily/boards/2026/10/obzor-oktyabrya-zadachi.board.json".into(),
            kind: VaultFileKind::Board,
            title: "Обзор октября — Задачи".into(),
            item_count: 3,
            column_count: 4,
            excerpt: "Архив октябрьского спринта.".into(),
            modified: 195,
            pinned: false,
        });
        self.expanded_folders.insert("daily/boards/2026".into());
        self.expanded_folders.insert("daily/boards/2026/11".into());

        // Set explicit board_order on notes for built-in Kanban showcase
        for (i, n) in self.notes.iter_mut().enumerate() {
            n.board_order = ((i + 1) * 10) as i64;
        }

        // Rich task metadata in current note + recent tabs + nav history
        self.selected_path = Some("daily/2026/11/2026-11-20.md".into());
        self.current_doc = NoteDocument {
            frontmatter: Frontmatter {
                title: "20 ноября 2026".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["daily".into(), "sprint".into()],
                pinned: false,
                board_order: None,
                extra_lines: Vec::new(),
            },
            body: "## Задачи дня\n\n- [ ] Закрыть ревью Smart-фильтров и Saved Views @2026-11-18 !high #ui\n- [ ] Собрать релиз Krono под Windows и проверить иконки @2026-11-20 !med #release\n- [ ] Документировать архитектуру Canvas Marquee @2026-11-25 !low #canvas\n- [x] Перенести доски дневника в daily/boards/2026/11\n\n## Связи\n\nСмотри [[Architecture]] и [[Задачи релиза]].\n".into(),
        };
        self.refresh_document_derivations();

        self.recent_tabs = vec![
            NavTarget::Note("daily/2026/11/2026-11-20.md".into()),
            NavTarget::Collection("daily/boards/2026/11/20-noyabrya-2026-zadachi.board.json".into()),
            NavTarget::Canvas("youtube/features.canvas".into()),
            NavTarget::Note("architecture.md".into()),
        ];
        self.nav_history = vec![
            NavTarget::Note("welcome.md".into()),
            NavTarget::Note("daily/2026/11/2026-11-20.md".into()),
        ];
        self.nav_cursor = 1;

        // Custom collection with smart filter panel + saved views + due dates
        let today = today_local();
        let mut col_doc = CollectionDoc::new_board_preset("Спринт Krono v0.4", "kanban", true);
        let stage_col = col_doc
            .effective_group_column_id()
            .unwrap_or("stage")
            .to_string();
        let opt_ids: Vec<String> = col_doc
            .columns
            .iter()
            .find(|c| c.id == stage_col)
            .map(|c| c.options.iter().map(|o| o.id.clone()).collect())
            .unwrap_or_default();
        let date_col = col_doc
            .columns
            .iter()
            .find(|c| c.col_type == ColumnType::Date)
            .map(|c| c.id.clone());
        if opt_ids.len() >= 3 {
            let r1 = col_doc.add_record("Фикс просроченных задач", Some((&stage_col, &opt_ids[0])), 100);
            let r2 = col_doc.add_record("Смарт-фильтры + Saved Views", Some((&stage_col, &opt_ids[1])), 110);
            let r3 = col_doc.add_record("Marquee выделение и Smart Guides", Some((&stage_col, &opt_ids[2])), 120);
            if let Some(rec1) = col_doc.records.iter_mut().find(|r| r.id == r1) {
                rec1.tags = vec!["ui".into(), "urgent".into()];
                if let Some(ref dc) = date_col {
                    rec1.values.insert(dc.clone(), FieldValue::Text(today.format_iso()));
                }
            }
            if let Some(rec2) = col_doc.records.iter_mut().find(|r| r.id == r2) {
                rec2.tags = vec!["feature".into()];
                if let Some(ref dc) = date_col {
                    rec2.values.insert(dc.clone(), FieldValue::Text(today.next_day().format_iso()));
                }
            }
            if let Some(rec3) = col_doc.records.iter_mut().find(|r| r.id == r3) {
                rec3.tags = vec!["canvas".into()];
            }
        }
        col_doc.saved_views = vec![
            SavedCollectionView {
                id: "sv_today".into(),
                name: "На сегодня".into(),
                view_kind: CollectionViewKind::Board,
                query: String::new(),
                date_preset: DateFilterPreset::Today,
                tag: None,
                select_col_id: None,
                select_opt_id: None,
            },
            SavedCollectionView {
                id: "sv_ui".into(),
                name: "Тег #ui".into(),
                view_kind: CollectionViewKind::Board,
                query: String::new(),
                date_preset: DateFilterPreset::All,
                tag: Some("ui".into()),
                select_col_id: None,
                select_opt_id: None,
            },
        ];

        let mut col_st = CollectionEditorState::default();
        col_st.path = Some("projects/tasks.board.json".into());
        col_st.active_view = CollectionViewKind::Board;
        col_st.doc = col_doc;
        col_st.smart_filter_open = true;
        self.active_collection = col_st;

        // Multi-selected canvas cards for Shot 04
        let multi_ids: Vec<String> = self
            .active_canvas
            .doc
            .nodes
            .iter()
            .filter(|n| n.kind != CanvasNodeKind::Group)
            .take(2)
            .map(|n| n.id.clone())
            .collect();
        self.active_canvas.search_open = false;
        self.active_canvas.selected_edge_id = None;
        self.active_canvas.marquee_mode = true;
        self.active_canvas.selected_node_ids = multi_ids.iter().cloned().collect();
        self.active_canvas.selected_node_id = multi_ids.first().cloned();

        // Vault overview state
        self.vault_map = VaultMapState::default();
        self.vault_map.needs_fit = true;
        self.vault_map.selected_path = Some("architecture.md".into());

        // Demo Attachment Audit textures + entries
        self.image_cache
            .insert_demo_texture(ctx, "assets/orphan_old_mockup.png", demo_preview_color_image());
        self.asset_audit = vec![
            AssetAuditEntry {
                rel_path: "assets/orphan_old_mockup.png".into(),
                size: 148_220,
                ref_count: 0,
                referenced_by: Vec::new(),
            },
            AssetAuditEntry {
                rel_path: "assets/demo.png".into(),
                size: 84_512,
                ref_count: 2,
                referenced_by: vec![
                    "projects/release.md".into(),
                    "youtube/features.canvas".into(),
                ],
            },
        ];
    }

    pub(crate) fn drive_screenshots(&mut self, ctx: &egui::Context) {
        const STAGE5_SHOTS: [&str; 6] = [
            "01_s5_desktop_graph_dark",
            "02_s5_desktop_local_graph_inspector",
            "03_s5_desktop_canvas_storyboard_dark",
            "04_s5_desktop_canvas_paper_light",
            "05_s5_mobile_390_graph",
            "06_s5_mobile_390_canvas",
        ];
        const POLISH_SHOTS: [&str; 6] = [
            "01_polish_desktop_sidebar_diary",
            "02_polish_create_modal",
            "03_polish_canvas_enhanced",
            "04_polish_settings_github",
            "05_polish_graph_diary_hubs",
            "06_polish_mobile_create_modal",
        ];
        const FEATURES_SHOTS: [&str; 5] = [
            "01_feat_create_modal_contrast",
            "02_feat_canvas_edge_search_image",
            "03_feat_note_split_image_kanban",
            "04_feat_settings_autosync",
            "05_feat_board_quickadd_diary",
        ];
        const MEGA_SHOTS: [&str; 6] = [
            "01_mega_tabs_task_meta_diary_boards",
            "02_mega_builtin_board_order",
            "03_mega_collection_smart_filters",
            "04_mega_canvas_marquee_multi",
            "05_mega_vault_overview",
            "06_mega_settings_attachments",
        ];

        let Some(runner) = self.shot_runner.as_ref() else {
            return;
        };
        let mode = runner.mode;
        let step = runner.step;
        let frames = runner.frames_in_step;
        let requested = runner.requested;
        let out_dir = runner.out_dir.clone();
        let shots: &[&str] = match mode {
            ShotMode::Stage5 => &STAGE5_SHOTS,
            ShotMode::Polish => &POLISH_SHOTS,
            ShotMode::Features => &FEATURES_SHOTS,
            ShotMode::Mega => &MEGA_SHOTS,
        };

        if step >= shots.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        let captured = ctx.input(|i| {
            i.events.iter().find_map(|ev| match ev {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });

        if let Some(img) = captured {
            let path = out_dir.join(format!("{}.bmp", shots[step]));
            let _ = write_color_image_bmp(&path, &img);
            if let Some(r) = self.shot_runner.as_mut() {
                r.step += 1;
                r.frames_in_step = 0;
                r.requested = false;
            }
            ctx.request_repaint();
            return;
        }

        if frames == 0 {
            let today = today_local();
            self.calendar_year = if mode == ShotMode::Stage5 { today.year } else { 2026 };
            self.calendar_month = if mode == ShotMode::Stage5 { today.month } else { 11 };
            self.palette_open = false;
            self.drawer_sidebar_open = false;
            self.drawer_inspector_open = false;
            self.rename_target = None;
            self.delete_target = None;
            self.create_modal.open = false;
            self.pages_filter_open = false;
            self.config.language = Lang::Ru;
            self.config.calendar_open = false;
            self.config.sidebar_open = true;
            self.config.inspector_open = false;

            match mode {
                ShotMode::Mega => {
                    self.seed_mega_demo(ctx);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        1280.0, 800.0,
                    )));
                    self.config.select_theme(ThemePreset::Graphite);
                    self.config.accent = AccentPreset::SteelBlue;
                    self.set_target_palette(ctx, true);
                    match step {
                        0 => {
                            self.view_mode = ViewMode::Editor;
                            self.editor_mode = EditorMode::Read;
                        }
                        1 => {
                            self.view_mode = ViewMode::Board;
                        }
                        2 => {
                            self.view_mode = ViewMode::Collection;
                        }
                        3 => {
                            self.view_mode = ViewMode::Canvas;
                        }
                        4 => {
                            self.view_mode = ViewMode::Overview;
                        }
                        5 => {
                            self.view_mode = ViewMode::Settings;
                            self.settings_tab = SettingsTab::Storage;
                        }
                        _ => {}
                    }
                }
                ShotMode::Features => {
                    self.seed_features_demo(ctx);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        1280.0, 800.0,
                    )));
                    self.config.select_theme(ThemePreset::Graphite);
                    self.config.accent = AccentPreset::SteelBlue;
                    self.set_target_palette(ctx, true);
                    match step {
                        0 => {
                            self.view_mode = ViewMode::Editor;
                            self.create_modal.open_with(CreateItemKind::Board);
                            self.create_modal.preset_id = "kanban".into();
                            self.create_modal.folder = "projects".into();
                            self.create_modal.title = "Спринт v1.2".into();
                        }
                        1 => {
                            self.view_mode = ViewMode::Canvas;
                        }
                        2 => {
                            self.view_mode = ViewMode::Editor;
                            self.editor_mode = EditorMode::Split;
                        }
                        3 => {
                            self.config.github_repo = "krono-notes/demo-vault".into();
                            self.has_saved_github_token = true;
                            self.config.auto_sync_after_save = true;
                            self.view_mode = ViewMode::Settings;
                            self.settings_tab = SettingsTab::GitHub;
                        }
                        4 => {
                            self.view_mode = ViewMode::Collection;
                        }
                        _ => {}
                    }
                }
                ShotMode::Polish => {
                    self.seed_polish_demo();
                    match step {
                    0 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.pages_filter_open = true;
                        self.pages_filter = PagesKindFilter::All;
                        self.pages_sort = PagesSortOrder::ModifiedDesc;
                        self.view_mode = ViewMode::Editor;
                        self.editor_mode = EditorMode::Edit;
                    }
                    1 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.view_mode = ViewMode::Editor;
                        self.create_modal.open_with(CreateItemKind::Note);
                        self.create_modal.folder = "projects".into();
                        self.create_modal.title = "Спринт 12".into();
                    }
                    2 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.view_mode = ViewMode::Canvas;
                        self.active_canvas.show_minimap = true;
                        let sel_id = self
                            .active_canvas
                            .doc
                            .nodes
                            .iter()
                            .rev()
                            .find(|n| n.kind == CanvasNodeKind::Text)
                            .map(|n| n.id.clone());
                        self.active_canvas.selected_node_id = sel_id;
                    }
                    3 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.view_mode = ViewMode::Settings;
                        self.settings_tab = SettingsTab::GitHub;
                    }
                    4 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.graph_state.filters_open = Some(false);
                        self.graph_state.selected_path = Some("daily/2026/11/2026-11.md".into());
                        self.view_mode = ViewMode::Graph;
                    }
                    5 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            390.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.view_mode = ViewMode::Editor;
                        self.mobile_showing_list = true;
                        self.create_modal.open_with(CreateItemKind::Canvas);
                    }
                    _ => {}
                }
                }
                ShotMode::Stage5 => {
                    self.seed_stage5_demo();

                match step {
                    0 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.graph_state.filters_open = Some(true);
                        self.graph_state.selected_path = Some("architecture.md".into());
                        self.view_mode = ViewMode::Graph;
                    }
                    1 => {
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.open_note("architecture.md");
                        self.seed_stage5_demo();
                        self.config.inspector_open = true;
                        self.local_graph_depth = 2;
                        self.refresh_document_derivations();
                        self.view_mode = ViewMode::Editor;
                        self.editor_mode = crate::ui::EditorMode::Read;
                    }
                    2 => {
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.active_canvas.selected_node_id = Some("n_2".into());
                        self.view_mode = ViewMode::Canvas;
                    }
                    3 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            1280.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Paper);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.active_canvas.selected_node_id = None;
                        self.view_mode = ViewMode::Canvas;
                    }
                    4 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            390.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Graphite);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.graph_state.filters_open = Some(false);
                        self.graph_state.selected_path = Some("architecture.md".into());
                        self.view_mode = ViewMode::Graph;
                    }
                    5 => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            390.0, 800.0,
                        )));
                        self.config.select_theme(ThemePreset::Paper);
                        self.config.accent = AccentPreset::SteelBlue;
                        self.set_target_palette(ctx, true);
                        self.mobile_showing_list = false;
                        self.active_canvas.selected_node_id = None;
                        self.view_mode = ViewMode::Canvas;
                    }
                    _ => {}
                }
                }
            }
        } else if frames == 4 {
            self.graph_state.needs_fit = true;
            self.active_canvas.needs_fit = true;
        }

        if let Some(r) = self.shot_runner.as_mut() {
            r.frames_in_step += 1;
            if frames >= 24 && !requested {
                r.requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
        }
        ctx.request_repaint();
    }
}

fn write_color_image_bmp(path: &Path, img: &egui::ColorImage) -> std::io::Result<()> {
    let w = img.width() as u32;
    let h = img.height() as u32;
    let pixel_bytes = w * h * 4;
    let file_size = 54 + pixel_bytes;

    let mut out = Vec::with_capacity(file_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(-(h as i32)).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&pixel_bytes.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    for px in &img.pixels {
        out.push(px.b());
        out.push(px.g());
        out.push(px.r());
        out.push(255);
    }
    std::fs::write(path, out)
}
