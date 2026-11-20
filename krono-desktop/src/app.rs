use crate::config::{open_in_file_manager, AppConfig};
use crate::i18n::{format_relative_time, strings, Lang};
#[cfg(debug_assertions)]
use crate::shot_runner::ShotRunner;
use crate::storage::{
    index::{BacklinkHit, CollectionMeta, RawLinkEdge, SearchHit, VaultIndex},
    vault::{
        audit_vault_assets, create_canvas_preset, create_collection, create_collection_preset,
        create_daily_note, create_note, create_note_with_preset, delete_note, duplicate_canvas,
        duplicate_collection, duplicate_note, ensure_daily_hubs_for_date, ensure_starter_vault,
        import_image_file_to_assets, migrate_diary_hierarchy, read_canvas, read_collection,
        read_note, rename_canvas, rename_collection, rename_note, rewrite_vault_canvas_links,
        rewrite_vault_collection_links, rewrite_vault_wikilinks, save_canvas, save_collection,
        save_image_bytes_to_assets, save_note, AssetAuditEntry,
    },
    watcher::VaultWatcher,
};
use crate::sync::{
    updater::{AvailableUpdate, UpdateApplyOutcome, UpdateEvent, UpdateWorker},
    worker::{SyncEvent, SyncWorker},
};
use crate::ui::{
    board_view::render_board_view,
    canvas_view::{render_canvas_view, CanvasEditorState},
    collection_view::{render_collection_view, CollectionEditorState},
    command_palette::render_command_palette,
    components::{render_toasts, Toast, ToastKind},
    dialogs::{
        render_create_item_modal, render_delete_modal, render_rename_modal, CreateModalState,
    },
    editor_view::render_editor_view,
    embedded_board::EmbeddedBoardAction,
    graph_view::{build_local_graph, render_graph_view, GraphState, LocalGraphData},
    image_cache::{
        encode_rgba_to_png, image_dimensions_from_bytes, render_image_lightbox, ImageTextureCache,
    },
    inspector::{render_inspector, render_inspector_drawer, OutgoingLinkInfo},
    markdown_preview::{hash_str, toggle_markdown_task},
    mobile_nav::{render_mobile_bottom_bar, render_mobile_notes_list},
    motion::{anim_bool_eased, anim_since, ease_in_out_cubic},
    settings_view::{render_settings_view, UpdateUiState},
    sidebar::{render_sidebar, render_sidebar_drawer},
    table_view::render_table_view,
    theme::{apply_palette, setup_fonts, ThemePalette},
    topbar::{render_recent_tabs_bar, render_topbar},
    vault_map_view::{render_vault_map_view, VaultMapState},
    CalendarUiState, CreateItemKind, EditorDocState, EditorMode, EditorPrefs, LayoutClass,
    NavTarget, PagesKindFilter, PagesSortOrder, PagesTreeUiState, RecentTabItem, SettingsTab,
    SyncUiState, TableSort, TopbarDocContext, UiAction, ViewMode,
};
use chrono::Datelike;
use eframe::egui::{self, Margin};
use krono_core::{
    collection::{CollectionDoc, CollectionViewKind, ColumnType, FieldValue},
    daily::{daily_legacy_rel_path, daily_note_rel_path, parse_daily_note_path, SimpleDate},
    markdown::{
        extract_embedded_boards, extract_headings, extract_nth_task_line, extract_wikilinks,
        parse_inline_task_meta, remove_nth_task_line, Heading, TaskPriority,
    },
    note::{normalize_tags, NoteDocument, NoteMeta, NoteStatus},
    path::{conflict_path, is_image_ext, vault_file_stem, VaultFileKind},
};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum ImagePickTarget {
    Editor,
    Canvas,
}

pub(crate) fn today_local() -> SimpleDate {
    let d = chrono::Local::now().date_naive();
    SimpleDate::from_ymd(d.year(), d.month(), d.day()).unwrap_or(SimpleDate {
        year: 2026,
        month: 1,
        day: 1,
    })
}

pub struct KronoApp {
    pub(crate) config: AppConfig,
    palette_from: ThemePalette,
    palette_target: ThemePalette,
    pub(crate) palette: ThemePalette,
    palette_anim_at: Option<Instant>,
    index: VaultIndex,
    watcher: VaultWatcher,
    sync_worker: SyncWorker,

    pub(crate) notes: Vec<NoteMeta>,
    pub(crate) collections: Vec<CollectionMeta>,
    pub(crate) all_links: Vec<RawLinkEdge>,
    tag_counts: Vec<(String, usize)>,
    cached_link_count: usize,
    resolved_titles: HashSet<String>,

    pub(crate) selected_path: Option<String>,
    pub(crate) current_doc: NoteDocument,
    current_modified: i64,
    pub(crate) dirty: bool,
    last_edit_at: Option<Instant>,

    pub(crate) active_collection: CollectionEditorState,
    pub(crate) active_canvas: CanvasEditorState,
    pub(crate) create_modal: CreateModalState,
    pub(crate) image_cache: ImageTextureCache,
    pub(crate) lightbox_image: Option<String>,
    pub(crate) embedded_boards: HashMap<String, CollectionDoc>,
    pending_image_pick: Option<(ImagePickTarget, mpsc::Receiver<Option<PathBuf>>)>,
    last_local_save_at: Option<Instant>,
    last_cmd_modifier_at: Option<Instant>,
    v_key_pressed_unmodified: bool,
    insert_key_pressed_unmodified: bool,
    saw_text_paste_at: Option<Instant>,
    saw_press_paste_at: Option<Instant>,

    // Cached inspector/preview/graph derivations (never recomputed per-frame unnecessarily)
    cached_headings: Vec<Heading>,
    cached_backlinks: Vec<BacklinkHit>,
    cached_outgoing: Vec<OutgoingLinkInfo>,
    pub(crate) graph_state: GraphState,
    pub(crate) local_graph_data: LocalGraphData,
    pub(crate) local_graph_depth: usize,

    pub(crate) view_mode: ViewMode,
    previous_view_mode: ViewMode,
    view_anim_at: Option<Instant>,
    note_anim_at: Option<Instant>,
    settings_tab_anim_at: Option<Instant>,
    pub(crate) editor_mode: EditorMode,
    active_tag_filter: Option<String>,
    pub(crate) expanded_folders: HashSet<String>,

    // Sidebar / Editor / Table / Board transient UI states
    pub(crate) pages_filter: PagesKindFilter,
    pub(crate) pages_sort: PagesSortOrder,
    pub(crate) pages_filter_open: bool,
    new_tag_input: String,
    adding_tag: bool,
    table_filter_query: String,
    table_status_filter: Option<NoteStatus>,
    table_sort: TableSort,
    quick_add_board_status: Option<NoteStatus>,
    quick_add_board_title: String,

    // Command palette
    pub(crate) palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    palette_hits: Vec<SearchHit>,

    // Settings & Dialogs & Toasts
    pub(crate) settings_tab: SettingsTab,
    vault_draft: String,
    show_token: bool,
    test_feedback: Option<(bool, String)>,
    pub(crate) rename_target: Option<String>,
    rename_draft: String,
    pub(crate) delete_target: Option<String>,
    toasts: Vec<Toast>,

    // Sync & Clipboard indicators
    last_sync_ts: Option<i64>,
    last_sync_commit: Option<String>,
    last_auto_sync_check: Instant,
    sync_angle: f32,

    // Stage 3: Calendar, Markdown Assist & Sync status
    pub(crate) calendar_year: i32,
    pub(crate) calendar_month: u32,
    pub(crate) daily_dates: HashSet<SimpleDate>,
    pending_sync_changes: usize,
    pub(crate) external_conflict_open_note: bool,
    focus_title_next: bool,
    focus_body_next: bool,
    snippet_draft_trigger: String,
    snippet_draft_label: String,
    snippet_draft_content: String,
    copied_code_hash: Option<u64>,
    copied_code_at: Option<Instant>,

    // Responsive layout & mobile navigation state
    pub(crate) last_layout_class: LayoutClass,
    pub(crate) drawer_sidebar_open: bool,
    pub(crate) drawer_inspector_open: bool,
    pub(crate) mobile_showing_list: bool,
    pub(crate) last_notes_tab_mode: ViewMode,
    pub(crate) last_notes_showing_list: bool,
    mobile_board_tab: NoteStatus,

    // Mega sprint: History/Recent Tabs, Global Vault Map, Global Attachment Archive
    pub(crate) nav_history: Vec<NavTarget>,
    pub(crate) nav_cursor: usize,
    nav_suppress_push: bool,
    pub(crate) recent_tabs: Vec<NavTarget>,
    pub(crate) vault_map: VaultMapState,
    pub(crate) asset_audit: Vec<AssetAuditEntry>,
    pub(crate) asset_audit_only_unused: bool,

    // In-app GitHub Releases auto-updater
    update_worker: UpdateWorker,
    pub(crate) available_update: Option<AvailableUpdate>,
    pub(crate) update_status_msg: Option<String>,

    #[cfg(debug_assertions)]
    pub(crate) shot_runner: Option<ShotRunner>,
}

impl KronoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config = AppConfig::load();
        setup_fonts(&cc.egui_ctx);
        let palette = ThemePalette::for_preset(config.theme, config.accent);
        apply_palette(&cc.egui_ctx, &palette);

        #[cfg(debug_assertions)]
        let shot_runner = ShotRunner::from_env();
        let is_shot_mode = {
            #[cfg(debug_assertions)]
            {
                shot_runner.is_some()
            }
            #[cfg(not(debug_assertions))]
            {
                false
            }
        };

        let _ = ensure_starter_vault(&config.vault_dir, config.language);
        if !is_shot_mode {
            let _ = migrate_diary_hierarchy(&config.vault_dir, config.language);
        }
        let (mut index, index_recovered) = VaultIndex::open_or_recover(&config.vault_dir);
        let _ = index.sync_from_disk(&config.vault_dir);
        let today = today_local();

        let last_sync_ts = index
            .get_meta("last_sync_time")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok());
        let last_sync_commit = index.get_meta("last_sync_commit").ok().flatten();

        let watcher = VaultWatcher::spawn(config.vault_dir.clone(), cc.egui_ctx.clone());
        let mut sync_worker = SyncWorker::new();
        let mut update_worker = UpdateWorker::new();

        if config.auto_sync_on_start && config.is_github_configured() {
            if let Ok(base) = index.load_sync_state() {
                sync_worker.start_sync(
                    config.vault_dir.clone(),
                    base,
                    config.github_repo.clone(),
                    config.github_branch.clone(),
                    config.github_token.clone(),
                    config.language,
                    cc.egui_ctx.clone(),
                );
            }
        }

        if !is_shot_mode && config.auto_check_updates {
            update_worker.start_check(
                config.include_prerelease_updates,
                true,
                config.language,
                cc.egui_ctx.clone(),
            );
        }

        let vault_draft = config.vault_dir.to_string_lossy().to_string();
        let mut app = Self {
            config,
            palette_from: palette,
            palette_target: palette,
            palette,
            palette_anim_at: None,
            index,
            watcher,
            sync_worker,
            notes: Vec::new(),
            collections: Vec::new(),
            all_links: Vec::new(),
            tag_counts: Vec::new(),
            cached_link_count: 0,
            resolved_titles: HashSet::new(),
            selected_path: None,
            current_doc: NoteDocument::parse("", ""),
            current_modified: 0,
            dirty: false,
            last_edit_at: None,
            active_collection: CollectionEditorState::default(),
            active_canvas: CanvasEditorState::default(),
            create_modal: CreateModalState::default(),
            image_cache: ImageTextureCache::default(),
            lightbox_image: None,
            embedded_boards: HashMap::new(),
            pending_image_pick: None,
            last_local_save_at: None,
            last_cmd_modifier_at: None,
            v_key_pressed_unmodified: false,
            insert_key_pressed_unmodified: false,
            saw_text_paste_at: None,
            saw_press_paste_at: None,
            cached_headings: Vec::new(),
            cached_backlinks: Vec::new(),
            cached_outgoing: Vec::new(),
            graph_state: GraphState::default(),
            local_graph_data: LocalGraphData::default(),
            local_graph_depth: 1,
            view_mode: ViewMode::Editor,
            previous_view_mode: ViewMode::Editor,
            view_anim_at: None,
            note_anim_at: None,
            settings_tab_anim_at: None,
            editor_mode: EditorMode::Edit,
            active_tag_filter: None,
            expanded_folders: HashSet::new(),
            pages_filter: PagesKindFilter::All,
            pages_sort: PagesSortOrder::NameAsc,
            pages_filter_open: false,
            new_tag_input: String::new(),
            adding_tag: false,
            table_filter_query: String::new(),
            table_status_filter: None,
            table_sort: TableSort::ModifiedDesc,
            quick_add_board_status: None,
            quick_add_board_title: String::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            palette_hits: Vec::new(),
            settings_tab: SettingsTab::Appearance,
            vault_draft,
            show_token: false,
            test_feedback: None,
            rename_target: None,
            rename_draft: String::new(),
            delete_target: None,
            toasts: Vec::new(),
            last_sync_ts,
            last_sync_commit,
            last_auto_sync_check: Instant::now(),
            sync_angle: 0.0,
            calendar_year: today.year,
            calendar_month: today.month,
            daily_dates: HashSet::new(),
            pending_sync_changes: 0,
            external_conflict_open_note: false,
            focus_title_next: false,
            focus_body_next: false,
            snippet_draft_trigger: String::new(),
            snippet_draft_label: String::new(),
            snippet_draft_content: String::new(),
            copied_code_hash: None,
            copied_code_at: None,
            last_layout_class: LayoutClass::Desktop,
            drawer_sidebar_open: false,
            drawer_inspector_open: false,
            mobile_showing_list: false,
            last_notes_tab_mode: ViewMode::Editor,
            last_notes_showing_list: false,
            mobile_board_tab: NoteStatus::Todo,
            nav_history: Vec::new(),
            nav_cursor: 0,
            nav_suppress_push: false,
            recent_tabs: Vec::new(),
            vault_map: VaultMapState::default(),
            asset_audit: Vec::new(),
            asset_audit_only_unused: false,
            update_worker,
            available_update: None,
            update_status_msg: None,
            #[cfg(debug_assertions)]
            shot_runner,
        };

        app.reload_index_caches();
        if index_recovered {
            let s = strings(app.config.language);
            app.push_toast(s.toast_index_recovered, ToastKind::Info);
        }
        for n in &app.notes {
            if let Some((folder, _)) = n.path.rsplit_once('/') {
                app.expanded_folders.insert(folder.to_string());
            }
        }
        for c in &app.collections {
            if let Some((folder, _)) = c.path.rsplit_once('/') {
                app.expanded_folders.insert(folder.to_string());
            }
        }
        let initial_path = app
            .notes
            .iter()
            .find(|n| n.path.eq_ignore_ascii_case("welcome.md"))
            .or_else(|| app.notes.first())
            .map(|n| n.path.clone());
        if let Some(first) = initial_path {
            app.open_note(&first);
        }
        app
    }

    fn push_toast(&mut self, msg: impl Into<String>, kind: ToastKind) {
        self.toasts.push(Toast::new(msg, kind));
    }

    pub(crate) fn reload_index_caches(&mut self) {
        self.notes = self.index.list_notes().unwrap_or_default();
        self.collections = self.index.list_collections().unwrap_or_default();
        self.all_links = self.index.list_all_links().unwrap_or_default();
        self.tag_counts = self.index.tag_counts().unwrap_or_default();
        self.cached_link_count = self.index.total_links_count();
        self.resolved_titles.clear();
        self.daily_dates.clear();
        for n in &self.notes {
            self.resolved_titles.insert(n.title.to_lowercase());
            if let Some(no_ext) = n.path.strip_suffix(".md") {
                self.resolved_titles.insert(no_ext.to_lowercase());
            }
            if let Some(stem) = std::path::Path::new(&n.path)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                self.resolved_titles.insert(stem.to_lowercase());
            }
            if let Some(date) = parse_daily_note_path(&n.path) {
                self.daily_dates.insert(date);
            }
        }
        self.pending_sync_changes = if self.config.is_github_configured() {
            self.index.count_unsynced_changes()
        } else {
            0
        };
        self.refresh_palette_hits();
    }

    pub(crate) fn refresh_document_derivations(&mut self) {
        self.cached_headings = extract_headings(&self.current_doc.body);
        self.cached_backlinks = if let Some(path) = &self.selected_path {
            self.index
                .backlinks_for(path, &self.current_doc.frontmatter.title)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let outgoing_raw = extract_wikilinks(&self.current_doc.body);
        self.cached_outgoing = outgoing_raw
            .into_iter()
            .map(|wl| {
                let resolved_path = self.find_note_by_wikilink(&wl.target);
                OutgoingLinkInfo {
                    target: wl.target,
                    resolved_path,
                }
            })
            .collect();

        self.local_graph_data = if let Some(path) = &self.selected_path {
            build_local_graph(
                path,
                &self.notes,
                &self.collections,
                &self.all_links,
                self.local_graph_depth,
            )
        } else {
            LocalGraphData::default()
        };
    }

    fn find_note_by_wikilink(&self, target: &str) -> Option<String> {
        let q = target.trim().to_lowercase();
        if q.is_empty() {
            return None;
        }
        self.notes
            .iter()
            .find(|n| {
                n.title.to_lowercase() == q
                    || n.path.to_lowercase() == q
                    || n.path
                        .strip_suffix(".md")
                        .is_some_and(|p| p.to_lowercase() == q)
                    || std::path::Path::new(&n.path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|st| st.to_lowercase() == q)
            })
            .map(|n| n.path.clone())
    }

    fn existing_paths_set(&self) -> HashSet<String> {
        let mut set: HashSet<String> = self.notes.iter().map(|n| n.path.clone()).collect();
        for c in &self.collections {
            set.insert(c.path.clone());
        }
        if let Ok(assets) = self.index.list_asset_paths() {
            for a in assets {
                set.insert(a);
            }
        }
        set
    }

    pub(crate) fn set_target_palette(&mut self, ctx: &egui::Context, snap: bool) {
        let next = ThemePalette::for_preset(self.config.theme, self.config.accent);
        if snap {
            self.palette_from = next;
            self.palette_target = next;
            self.palette = next;
            self.palette_anim_at = None;
            apply_palette(ctx, &next);
        } else if self.palette_target.is_dark != next.is_dark
            || self.palette_target.bg_canvas != next.bg_canvas
            || self.palette_target.accent != next.accent
        {
            self.palette_from = self.palette;
            self.palette_target = next;
            self.palette_anim_at = Some(Instant::now());
        }
    }

    fn flush_unsaved_documents(&mut self) {
        if self.dirty {
            self.save_current_note();
        }
        if self.active_collection.dirty {
            self.save_current_collection();
        }
        if self.active_canvas.dirty {
            self.save_current_canvas();
        }
    }

    fn record_nav(&mut self, target: NavTarget) {
        if matches!(
            target,
            NavTarget::Note(_) | NavTarget::Collection(_) | NavTarget::Canvas(_)
        ) && !self.recent_tabs.contains(&target)
        {
            self.recent_tabs.push(target.clone());
            if self.recent_tabs.len() > 10 {
                self.recent_tabs.remove(0);
            }
        }
        if self.nav_suppress_push {
            return;
        }
        if self.nav_history.get(self.nav_cursor) == Some(&target) {
            return;
        }
        if self.nav_cursor + 1 < self.nav_history.len() {
            self.nav_history.truncate(self.nav_cursor + 1);
        }
        self.nav_history.push(target);
        if self.nav_history.len() > 64 {
            let drop_n = self.nav_history.len() - 64;
            self.nav_history.drain(0..drop_n);
        }
        self.nav_cursor = self.nav_history.len().saturating_sub(1);
    }

    fn can_nav_back(&self) -> bool {
        self.nav_cursor > 0 && self.nav_cursor < self.nav_history.len()
    }

    fn can_nav_forward(&self) -> bool {
        self.nav_cursor + 1 < self.nav_history.len()
    }

    fn navigate_to_target(&mut self, target: NavTarget, push_history: bool) {
        if !push_history {
            self.nav_suppress_push = true;
        }
        match target {
            NavTarget::Note(p) => {
                self.open_note(&p);
                if self.view_mode != ViewMode::Editor {
                    self.view_anim_at = Some(Instant::now());
                }
                self.view_mode = ViewMode::Editor;
                self.previous_view_mode = ViewMode::Editor;
                self.last_notes_tab_mode = ViewMode::Editor;
            }
            NavTarget::Collection(p) => self.open_collection(&p),
            NavTarget::Canvas(p) => self.open_canvas(&p),
            NavTarget::View(mode) => {
                if self.view_mode != mode {
                    self.view_anim_at = Some(Instant::now());
                }
                self.view_mode = mode;
                if mode != ViewMode::Settings {
                    self.previous_view_mode = mode;
                }
                self.record_nav(NavTarget::View(mode));
            }
        }
        self.nav_suppress_push = false;
        self.mobile_showing_list = false;
        self.drawer_sidebar_open = false;
        self.drawer_inspector_open = false;
    }

    fn build_recent_tab_items(&self) -> Vec<RecentTabItem> {
        let mut out = Vec::with_capacity(self.recent_tabs.len());
        for t in &self.recent_tabs {
            match t {
                NavTarget::Note(p) => {
                    let title = self
                        .notes
                        .iter()
                        .find(|n| &n.path == p)
                        .map(|n| n.title.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| vault_file_stem(p).to_string());
                    let is_cur = self.selected_path.as_deref() == Some(p.as_str());
                    out.push(RecentTabItem {
                        target: t.clone(),
                        title,
                        kind: VaultFileKind::Markdown,
                        dirty: is_cur && self.dirty,
                        active: self.view_mode == ViewMode::Editor && is_cur,
                    });
                }
                NavTarget::Collection(p) => {
                    let meta = self.collections.iter().find(|c| &c.path == p);
                    let title = meta
                        .map(|c| c.title.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| vault_file_stem(p).to_string());
                    let kind = meta
                        .map(|c| c.kind)
                        .or_else(|| VaultFileKind::from_path(p))
                        .unwrap_or(VaultFileKind::Board);
                    let is_cur = self.active_collection.path.as_deref() == Some(p.as_str());
                    out.push(RecentTabItem {
                        target: t.clone(),
                        title,
                        kind,
                        dirty: is_cur && self.active_collection.dirty,
                        active: self.view_mode == ViewMode::Collection && is_cur,
                    });
                }
                NavTarget::Canvas(p) => {
                    let title = self
                        .collections
                        .iter()
                        .find(|c| &c.path == p)
                        .map(|c| c.title.clone())
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| vault_file_stem(p).to_string());
                    let is_cur = self.active_canvas.path.as_deref() == Some(p.as_str());
                    out.push(RecentTabItem {
                        target: t.clone(),
                        title,
                        kind: VaultFileKind::Canvas,
                        dirty: is_cur && self.active_canvas.dirty,
                        active: self.view_mode == ViewMode::Canvas && is_cur,
                    });
                }
                NavTarget::View(_) => {}
            }
        }
        out
    }

    pub(crate) fn open_note(&mut self, rel_path: &str) {
        self.flush_unsaved_documents();
        let switched = self.selected_path.as_deref() != Some(rel_path);
        match read_note(&self.config.vault_dir, rel_path) {
            Ok((doc, modified, _)) => {
                self.selected_path = Some(rel_path.to_string());
                self.current_doc = doc;
                self.current_modified = modified;
                self.dirty = false;
                self.external_conflict_open_note = false;
                self.focus_title_next = false;
                self.focus_body_next = false;
                self.last_edit_at = None;
                self.adding_tag = false;
                self.new_tag_input.clear();
                self.expand_parents(rel_path);
                self.refresh_document_derivations();
                if switched {
                    self.note_anim_at = Some(Instant::now());
                }
                self.refresh_embedded_boards();
                self.record_nav(NavTarget::Note(rel_path.to_string()));
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
            }
        }
    }

    fn expand_parents(&mut self, rel_path: &str) {
        let parts: Vec<&str> = rel_path.split('/').collect();
        if parts.len() <= 1 {
            return;
        }
        let mut acc = String::new();
        for seg in &parts[..parts.len() - 1] {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(seg);
            self.expanded_folders.insert(acc.clone());
        }
    }

    fn save_current_note(&mut self) {
        let Some(rel_path) = self.selected_path.clone() else {
            return;
        };
        if self.current_doc.frontmatter.title.trim().is_empty() {
            let fallback = std::path::Path::new(&rel_path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string();
            self.current_doc.frontmatter.title = fallback;
        }
        self.current_doc.frontmatter.tags =
            normalize_tags(self.current_doc.frontmatter.tags.clone());

        self.watcher.mark_internal_change();
        match save_note(&self.config.vault_dir, &rel_path, &self.current_doc) {
            Ok((modified, size)) => {
                self.current_modified = modified;
                self.dirty = false;
                self.external_conflict_open_note = false;
                self.last_edit_at = None;
                let _ = self
                    .index
                    .upsert_note(&rel_path, &self.current_doc, modified, size);
                self.reload_index_caches();
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
            }
        }
    }

    fn create_and_open_note(&mut self, title: &str, status: NoteStatus, body: &str) {
        self.flush_unsaved_documents();
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        match create_note(&self.config.vault_dir, title, status, body, &existing) {
            Ok((rel_path, doc, modified, size)) => {
                let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                self.reload_index_caches();
                self.selected_path = Some(rel_path.clone());
                self.current_doc = doc;
                self.current_modified = modified;
                self.dirty = false;
                self.last_edit_at = None;
                self.expand_parents(&rel_path);
                self.refresh_document_derivations();
                self.view_mode = ViewMode::Editor;
                self.previous_view_mode = ViewMode::Editor;
                self.last_notes_tab_mode = ViewMode::Editor;
                self.last_notes_showing_list = false;
                self.editor_mode = EditorMode::Edit;
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn existing_folders_list(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for n in &self.notes {
            if let Some((dir, _)) = n.path.rsplit_once('/') {
                if dir != "daily" && !dir.starts_with("daily/") {
                    set.insert(dir.to_string());
                }
            }
        }
        for c in &self.collections {
            if let Some((dir, _)) = c.path.rsplit_once('/') {
                if dir != "daily" && !dir.starts_with("daily/") {
                    set.insert(dir.to_string());
                }
            }
        }
        set.into_iter().collect()
    }

    fn create_from_modal(
        &mut self,
        kind: CreateItemKind,
        title: &str,
        folder: &str,
        preset_id: &str,
    ) {
        self.flush_unsaved_documents();
        let raw_path = if folder.trim().is_empty() {
            title.trim().to_string()
        } else {
            format!("{}/{}", folder.trim().trim_matches('/'), title.trim())
        };
        let existing = self.existing_paths_set();
        let is_ru = self.config.language == Lang::Ru;
        self.watcher.mark_internal_change();

        match kind {
            CreateItemKind::Note => {
                match create_note_with_preset(
                    &self.config.vault_dir,
                    &raw_path,
                    preset_id,
                    self.config.language,
                    &existing,
                ) {
                    Ok((rel_path, doc, modified, size)) => {
                        let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                        self.reload_index_caches();
                        self.selected_path = Some(rel_path.clone());
                        self.current_doc = doc;
                        self.current_modified = modified;
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        self.last_edit_at = None;
                        self.expand_parents(&rel_path);
                        self.refresh_document_derivations();
                        self.refresh_embedded_boards();
                        self.view_mode = ViewMode::Editor;
                        self.previous_view_mode = ViewMode::Editor;
                        self.last_notes_tab_mode = ViewMode::Editor;
                        self.last_notes_showing_list = false;
                        self.mobile_showing_list = false;
                        self.editor_mode = EditorMode::Edit;
                        self.focus_body_next = true;
                        self.view_anim_at = Some(Instant::now());
                        self.last_local_save_at = Some(Instant::now());
                        self.record_nav(NavTarget::Note(rel_path));
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            CreateItemKind::Board | CreateItemKind::Table => {
                let vk = if kind == CreateItemKind::Board {
                    CollectionViewKind::Board
                } else {
                    CollectionViewKind::Table
                };
                match create_collection_preset(
                    &self.config.vault_dir,
                    &raw_path,
                    vk,
                    preset_id,
                    is_ru,
                    &existing,
                ) {
                    Ok((rel_path, fkind, doc, modified, size)) => {
                        let _ = self
                            .index
                            .upsert_collection(&rel_path, fkind, &doc, modified, size);
                        self.reload_index_caches();
                        self.active_collection.path = Some(rel_path.clone());
                        self.active_collection.active_view = doc.default_view;
                        self.active_collection.doc = doc;
                        self.active_collection.modified = modified;
                        self.active_collection.dirty = false;
                        self.active_collection.last_edit_at = None;
                        self.active_collection.reset_transient();
                        self.expand_parents(&rel_path);
                        self.view_mode = ViewMode::Collection;
                        self.previous_view_mode = ViewMode::Collection;
                        self.last_notes_tab_mode = ViewMode::Collection;
                        self.last_notes_showing_list = false;
                        self.mobile_showing_list = false;
                        self.view_anim_at = Some(Instant::now());
                        self.last_local_save_at = Some(Instant::now());
                        self.record_nav(NavTarget::Collection(rel_path));
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            CreateItemKind::Canvas => {
                match create_canvas_preset(
                    &self.config.vault_dir,
                    &raw_path,
                    preset_id,
                    is_ru,
                    &existing,
                ) {
                    Ok((rel_path, doc, modified, size)) => {
                        let _ = self.index.upsert_canvas(&rel_path, &doc, modified, size);
                        self.reload_index_caches();
                        self.active_canvas.path = Some(rel_path.clone());
                        self.active_canvas.doc = doc;
                        self.active_canvas.modified = modified;
                        self.active_canvas.dirty = false;
                        self.active_canvas.last_edit_at = None;
                        self.active_canvas.reset_transient();
                        self.expand_parents(&rel_path);
                        self.view_mode = ViewMode::Canvas;
                        self.previous_view_mode = ViewMode::Canvas;
                        self.last_notes_tab_mode = ViewMode::Canvas;
                        self.last_notes_showing_list = false;
                        self.mobile_showing_list = false;
                        self.view_anim_at = Some(Instant::now());
                        self.last_local_save_at = Some(Instant::now());
                        self.record_nav(NavTarget::Canvas(rel_path));
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
        }
    }

    fn update_note_status(&mut self, rel_path: &str, new_status: NoteStatus) {
        if self.selected_path.as_deref() == Some(rel_path) {
            self.current_doc.frontmatter.status = new_status;
            self.save_current_note();
            return;
        }
        self.watcher.mark_internal_change();
        match read_note(&self.config.vault_dir, rel_path) {
            Ok((mut doc, _, _)) => {
                doc.frontmatter.status = new_status;
                if let Ok((modified, size)) =
                    save_note(&self.config.vault_dir, rel_path, &doc)
                {
                    let _ = self.index.upsert_note(rel_path, &doc, modified, size);
                    self.reload_index_caches();
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = Some(Instant::now());
                }
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn sorted_board_column_paths(&self, status: NoteStatus, exclude: Option<&str>) -> Vec<String> {
        let mut items: Vec<&NoteMeta> = self
            .notes
            .iter()
            .filter(|n| n.status == status && exclude.is_none_or(|ex| n.path != ex))
            .collect();
        items.sort_by(|a, b| {
            a.board_order
                .cmp(&b.board_order)
                .then_with(|| b.modified.cmp(&a.modified))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        items.into_iter().map(|n| n.path.clone()).collect()
    }

    fn apply_board_column_order(&mut self, status: NoteStatus, ordered_paths: &[String]) {
        self.watcher.mark_internal_change();
        let mut any_changed = false;
        for (idx, rel_path) in ordered_paths.iter().enumerate() {
            let new_order = ((idx + 1) * 10) as i64;
            let meta = self.notes.iter().find(|n| &n.path == rel_path);
            let needs_write = meta.is_none_or(|m| m.status != status || m.board_order != new_order);
            if !needs_write {
                continue;
            }
            if self.selected_path.as_deref() == Some(rel_path.as_str()) {
                self.current_doc.frontmatter.status = status;
                self.current_doc.frontmatter.board_order = Some(new_order);
                if let Ok((modified, size)) =
                    save_note(&self.config.vault_dir, rel_path, &self.current_doc)
                {
                    self.current_modified = modified;
                    self.dirty = false;
                    let _ = self
                        .index
                        .upsert_note(rel_path, &self.current_doc, modified, size);
                    any_changed = true;
                }
            } else if let Ok((mut doc, _, _)) = read_note(&self.config.vault_dir, rel_path) {
                doc.frontmatter.status = status;
                doc.frontmatter.board_order = Some(new_order);
                if let Ok((modified, size)) = save_note(&self.config.vault_dir, rel_path, &doc) {
                    let _ = self.index.upsert_note(rel_path, &doc, modified, size);
                    any_changed = true;
                }
            }
        }
        if any_changed {
            self.reload_index_caches();
            self.refresh_document_derivations();
            self.refresh_embedded_boards();
            self.last_local_save_at = Some(Instant::now());
        }
    }

    fn move_note_in_board(
        &mut self,
        rel_path: &str,
        new_status: NoteStatus,
        insert_before: Option<&str>,
    ) {
        let mut col_paths = self.sorted_board_column_paths(new_status, Some(rel_path));
        let ins_idx = insert_before
            .and_then(|b| col_paths.iter().position(|p| p == b))
            .unwrap_or(col_paths.len());
        col_paths.insert(ins_idx, rel_path.to_string());
        self.apply_board_column_order(new_status, &col_paths);
    }

    fn shift_note_in_board(&mut self, rel_path: &str, delta: isize) {
        let Some(cur_status) = self
            .notes
            .iter()
            .find(|n| n.path == rel_path)
            .map(|n| n.status)
        else {
            return;
        };
        let mut col_paths = self.sorted_board_column_paths(cur_status, None);
        let Some(idx) = col_paths.iter().position(|p| p == rel_path) else {
            return;
        };
        let max_i = col_paths.len().saturating_sub(1) as isize;
        let target = (idx as isize + delta).clamp(0, max_i) as usize;
        if target == idx {
            return;
        }
        col_paths.swap(idx, target);
        self.apply_board_column_order(cur_status, &col_paths);
    }

    fn handle_send_task_to_board(&mut self, task_idx: usize, board_path: &str, move_task: bool) {
        let Some(note_path) = self.selected_path.clone() else {
            return;
        };
        let Some(raw_line) = extract_nth_task_line(&self.current_doc.body, task_idx) else {
            return;
        };
        let meta = parse_inline_task_meta(&raw_line, today_local());
        let title = if meta.clean_text.trim().is_empty() {
            raw_line.trim().to_string()
        } else {
            meta.clean_text.trim().to_string()
        };

        let is_open_board = self.active_collection.path.as_deref() == Some(board_path);
        let mut doc = if is_open_board {
            self.active_collection.doc.clone()
        } else {
            match read_collection(&self.config.vault_dir, board_path) {
                Ok((d, _, _, _)) => d,
                Err(e) => {
                    self.push_toast(e, ToastKind::Error);
                    return;
                }
            }
        };

        let stage_col = doc.effective_group_column_id().map(str::to_string);
        let first_opt = stage_col.as_ref().and_then(|cid| {
            doc.columns
                .iter()
                .find(|c| &c.id == cid)
                .and_then(|c| c.options.first())
                .map(|o| o.id.clone())
        });
        let stage_arg = stage_col
            .as_deref()
            .zip(first_opt.as_deref());
        let rec_id = doc.add_record(&title, stage_arg, chrono::Utc::now().timestamp());

        let date_col = doc
            .columns
            .iter()
            .find(|c| c.col_type == ColumnType::Date)
            .map(|c| c.id.clone());
        let note_col = doc
            .columns
            .iter()
            .find(|c| c.col_type == ColumnType::NoteLink)
            .map(|c| c.id.clone());
        let prio_match = meta.priority.and_then(|prio| {
            let group_id = stage_col.as_deref().unwrap_or("");
            let needle_words: &[&str] = match prio {
                TaskPriority::High => &["high", "высок", "срочн", "p1", "🔥"],
                TaskPriority::Medium => &["med", "средн", "p2", "normal"],
                TaskPriority::Low => &["low", "низк", "p3"],
            };
            doc.columns
                .iter()
                .filter(|c| c.col_type == ColumnType::Select && c.id != group_id)
                .find_map(|c| {
                    c.options.iter().find_map(|o| {
                        let lbl = o.label.to_lowercase();
                        if needle_words.iter().any(|w| lbl.contains(w)) {
                            Some((c.id.clone(), o.id.clone()))
                        } else {
                            None
                        }
                    })
                })
        });

        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == rec_id) {
            rec.tags = meta.tags;
            if let (Some(dc), Some(due)) = (date_col, meta.due_date) {
                rec.values.insert(dc, FieldValue::Text(due));
            }
            if let Some(nc) = note_col {
                rec.values.insert(nc, FieldValue::Text(note_path));
            }
            if let Some((pc, po)) = prio_match {
                rec.values.insert(pc, FieldValue::Text(po));
            }
        }

        let board_title = doc.title.clone();
        if is_open_board {
            self.active_collection.doc = doc;
            self.save_current_collection();
        } else {
            self.watcher.mark_internal_change();
            match save_collection(&self.config.vault_dir, board_path, &doc) {
                Ok((modified, size)) => {
                    let kind = VaultFileKind::from_path(board_path).unwrap_or(VaultFileKind::Board);
                    let _ = self
                        .index
                        .upsert_collection(board_path, kind, &doc, modified, size);
                    self.reload_index_caches();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = Some(Instant::now());
                }
                Err(e) => {
                    self.push_toast(e, ToastKind::Error);
                    return;
                }
            }
        }

        if move_task {
            if let Some(updated_body) = remove_nth_task_line(&self.current_doc.body, task_idx) {
                self.current_doc.body = updated_body;
                self.save_current_note();
            }
        }

        let s = strings(self.config.language);
        self.push_toast(format!("{}: {}", s.task_sent_toast, board_title), ToastKind::Success);
    }

    fn refresh_asset_audit(&mut self) {
        self.asset_audit = audit_vault_assets(&self.config.vault_dir);
    }

    fn delete_single_asset(&mut self, rel_path: &str) {
        self.watcher.mark_internal_change();
        match delete_note(&self.config.vault_dir, rel_path) {
            Ok(()) => {
                let _ = self.index.remove_asset(rel_path);
                self.refresh_asset_audit();
                self.last_local_save_at = Some(Instant::now());
                let s = strings(self.config.language);
                self.push_toast(s.toast_moved_to_trash, ToastKind::Info);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn delete_unused_assets(&mut self) {
        let unused: Vec<String> = self
            .asset_audit
            .iter()
            .filter(|a| a.ref_count == 0)
            .map(|a| a.rel_path.clone())
            .collect();
        if unused.is_empty() {
            return;
        }
        self.watcher.mark_internal_change();
        for rel in unused {
            if delete_note(&self.config.vault_dir, &rel).is_ok() {
                let _ = self.index.remove_asset(&rel);
            }
        }
        self.refresh_asset_audit();
        self.last_local_save_at = Some(Instant::now());
        let s = strings(self.config.language);
        self.push_toast(s.toast_moved_to_trash, ToastKind::Success);
    }

    pub(crate) fn open_collection(&mut self, rel_path: &str) {
        self.flush_unsaved_documents();
        match read_collection(&self.config.vault_dir, rel_path) {
            Ok((doc, modified, _, _)) => {
                self.active_collection.path = Some(rel_path.to_string());
                self.active_collection.active_view = doc.default_view;
                self.active_collection.doc = doc;
                self.active_collection.modified = modified;
                self.active_collection.dirty = false;
                self.active_collection.last_edit_at = None;
                self.active_collection.reset_transient();
                self.expand_parents(rel_path);
                if self.view_mode != ViewMode::Collection {
                    self.view_anim_at = Some(Instant::now());
                }
                self.view_mode = ViewMode::Collection;
                self.previous_view_mode = ViewMode::Collection;
                self.last_notes_tab_mode = ViewMode::Collection;
                self.last_notes_showing_list = false;
                self.record_nav(NavTarget::Collection(rel_path.to_string()));
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn save_current_collection(&mut self) {
        let Some(rel_path) = self.active_collection.path.clone() else {
            return;
        };
        if self.active_collection.doc.title.trim().is_empty() {
            let stem = krono_core::path::vault_file_stem(&rel_path).to_string();
            self.active_collection.doc.title = if stem.is_empty() {
                "Untitled".to_string()
            } else {
                stem
            };
        }
        let kind = VaultFileKind::from_path(&rel_path).unwrap_or(VaultFileKind::Board);
        self.watcher.mark_internal_change();
        match save_collection(&self.config.vault_dir, &rel_path, &self.active_collection.doc) {
            Ok((modified, size)) => {
                self.active_collection.modified = modified;
                self.active_collection.dirty = false;
                self.active_collection.external_conflict = false;
                self.active_collection.last_edit_at = None;
                let _ = self.index.upsert_collection(
                    &rel_path,
                    kind,
                    &self.active_collection.doc,
                    modified,
                    size,
                );
                self.reload_index_caches();
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn create_and_open_collection(&mut self, title: &str, view_kind: CollectionViewKind) {
        self.flush_unsaved_documents();
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        let is_ru = self.config.language == Lang::Ru;
        match create_collection(&self.config.vault_dir, title, view_kind, is_ru, &existing) {
            Ok((rel_path, kind, doc, modified, size)) => {
                let _ = self.index.upsert_collection(&rel_path, kind, &doc, modified, size);
                self.reload_index_caches();
                self.active_collection.path = Some(rel_path.clone());
                self.active_collection.active_view = doc.default_view;
                self.active_collection.doc = doc;
                self.active_collection.modified = modified;
                self.active_collection.dirty = false;
                self.active_collection.last_edit_at = None;
                self.active_collection.reset_transient();
                self.expand_parents(&rel_path);
                self.view_mode = ViewMode::Collection;
                self.previous_view_mode = ViewMode::Collection;
                self.last_notes_tab_mode = ViewMode::Collection;
                self.last_notes_showing_list = false;
                self.view_anim_at = Some(Instant::now());
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn save_collection_external_conflict_copy(&mut self) {
        let Some(sel) = self.active_collection.path.clone() else {
            return;
        };
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let copy_rel = conflict_path(&sel, &ts);
        let kind = VaultFileKind::from_path(&copy_rel).unwrap_or(VaultFileKind::Board);
        self.watcher.mark_internal_change();
        match save_collection(&self.config.vault_dir, &copy_rel, &self.active_collection.doc) {
            Ok((modified, size)) => {
                let _ = self.index.upsert_collection(
                    &copy_rel,
                    kind,
                    &self.active_collection.doc,
                    modified,
                    size,
                );
                self.active_collection.dirty = false;
                self.active_collection.external_conflict = false;
                self.open_collection(&sel);
                self.reload_index_caches();
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(crate) fn open_canvas(&mut self, rel_path: &str) {
        self.flush_unsaved_documents();
        match read_canvas(&self.config.vault_dir, rel_path) {
            Ok((doc, modified, _)) => {
                self.active_canvas.path = Some(rel_path.to_string());
                self.active_canvas.doc = doc;
                self.active_canvas.modified = modified;
                self.active_canvas.dirty = false;
                self.active_canvas.last_edit_at = None;
                self.active_canvas.reset_transient();
                self.expand_parents(rel_path);
                if self.view_mode != ViewMode::Canvas {
                    self.view_anim_at = Some(Instant::now());
                }
                self.view_mode = ViewMode::Canvas;
                self.previous_view_mode = ViewMode::Canvas;
                self.last_notes_tab_mode = ViewMode::Canvas;
                self.last_notes_showing_list = false;
                self.record_nav(NavTarget::Canvas(rel_path.to_string()));
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn save_current_canvas(&mut self) {
        let Some(rel_path) = self.active_canvas.path.clone() else {
            return;
        };
        if self.active_canvas.doc.title.trim().is_empty() {
            let stem = krono_core::path::vault_file_stem(&rel_path).to_string();
            self.active_canvas.doc.title = if stem.is_empty() {
                "Untitled".to_string()
            } else {
                stem
            };
        }
        self.watcher.mark_internal_change();
        match save_canvas(&self.config.vault_dir, &rel_path, &self.active_canvas.doc) {
            Ok((modified, size)) => {
                self.active_canvas.modified = modified;
                self.active_canvas.dirty = false;
                self.active_canvas.external_conflict = false;
                self.active_canvas.last_edit_at = None;
                let _ = self
                    .index
                    .upsert_canvas(&rel_path, &self.active_canvas.doc, modified, size);
                self.reload_index_caches();
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn save_canvas_external_conflict_copy(&mut self) {
        let Some(sel) = self.active_canvas.path.clone() else {
            return;
        };
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let copy_rel = conflict_path(&sel, &ts);
        self.watcher.mark_internal_change();
        match save_canvas(&self.config.vault_dir, &copy_rel, &self.active_canvas.doc) {
            Ok((modified, size)) => {
                let _ = self
                    .index
                    .upsert_canvas(&copy_rel, &self.active_canvas.doc, modified, size);
                self.active_canvas.dirty = false;
                self.active_canvas.external_conflict = false;
                self.open_canvas(&sel);
                self.reload_index_caches();
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn export_active_canvas_to_markdown(&mut self) {
        if self.active_canvas.dirty {
            self.save_current_canvas();
        }
        let Some(canvas_path) = self.active_canvas.path.clone() else {
            return;
        };
        let vault_dir = self.config.vault_dir.clone();
        let md_body = self.active_canvas.doc.export_to_markdown(|file_rel| {
            if let Ok((doc, _, _)) = read_note(&vault_dir, file_rel) {
                return Some((doc.frontmatter.title, doc.body));
            }
            if let Ok((col, _, _, _)) = read_collection(&vault_dir, file_rel) {
                let txt = col.search_text();
                return Some((col.title, txt));
            }
            None
        });
        let base_title = if self.active_canvas.doc.title.trim().is_empty() {
            krono_core::path::vault_file_stem(&canvas_path).to_string()
        } else {
            self.active_canvas.doc.title.trim().to_string()
        };
        let script_title = if self.config.language == Lang::Ru {
            format!("{base_title} (Сценарий)")
        } else {
            format!("{base_title} (Script)")
        };
        let target_arg = match canvas_path.rsplit_once('/') {
            Some((folder, _)) => format!("{folder}/{script_title}"),
            None => script_title,
        };
        self.create_and_open_note(&target_arg, NoteStatus::InProgress, &md_body);
        let s = strings(self.config.language);
        self.push_toast(s.canvas_export_toast, ToastKind::Success);
    }

    fn perform_rename(&mut self, old_path: &str, new_title: &str) {
        if VaultFileKind::from_path(old_path) == Some(VaultFileKind::Canvas) {
            if self.active_canvas.dirty && self.active_canvas.path.as_deref() == Some(old_path) {
                self.save_current_canvas();
            }
            let existing = self.existing_paths_set();
            self.watcher.mark_internal_change();
            match rename_canvas(&self.config.vault_dir, old_path, new_title, &existing) {
                Ok((new_path, doc, modified, size)) => {
                    if new_path != old_path {
                        let _ = self.index.remove_collection(old_path);
                    }
                    let _ = self.index.upsert_canvas(&new_path, &doc, modified, size);
                    if self.active_canvas.path.as_deref() == Some(old_path) {
                        self.active_canvas.path = Some(new_path.clone());
                        self.active_canvas.doc = doc;
                        self.active_canvas.modified = modified;
                        self.active_canvas.dirty = false;
                    }
                    self.reload_index_caches();
                    self.expand_parents(&new_path);
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = Some(Instant::now());
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }
        if matches!(
            VaultFileKind::from_path(old_path),
            Some(VaultFileKind::Board | VaultFileKind::Table)
        ) {
            if self.active_collection.dirty && self.active_collection.path.as_deref() == Some(old_path) {
                self.save_current_collection();
            }
            let existing = self.existing_paths_set();
            self.watcher.mark_internal_change();
            match rename_collection(&self.config.vault_dir, old_path, new_title, &existing) {
                Ok((new_path, doc, modified, size, kind)) => {
                    if new_path != old_path {
                        let _ = self.index.remove_collection(old_path);
                    }
                    let _ = self.index.upsert_collection(&new_path, kind, &doc, modified, size);
                    if self.active_collection.path.as_deref() == Some(old_path) {
                        self.active_collection.path = Some(new_path.clone());
                        self.active_collection.doc = doc;
                        self.active_collection.modified = modified;
                        self.active_collection.dirty = false;
                    }
                    self.reload_index_caches();
                    self.expand_parents(&new_path);
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = Some(Instant::now());
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }

        if self.dirty && self.selected_path.as_deref() == Some(old_path) {
            self.save_current_note();
        }
        let old_meta = self.notes.iter().find(|n| n.path == old_path).cloned();
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        match rename_note(&self.config.vault_dir, old_path, new_title, &existing) {
            Ok((new_path, doc, modified, size)) => {
                if new_path != old_path {
                    let _ = self.index.remove_note(old_path);
                }
                let _ = self.index.upsert_note(&new_path, &doc, modified, size);

                // Collect all aliases that previously resolved to the renamed note.
                let mut old_targets_owned = Vec::new();
                if let Some(meta) = old_meta {
                    old_targets_owned.push(meta.title);
                }
                if let Some(no_ext) = old_path.strip_suffix(".md") {
                    old_targets_owned.push(no_ext.to_string());
                }
                if let Some(stem) = std::path::Path::new(old_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    old_targets_owned.push(stem.to_string());
                }
                let target_refs: Vec<&str> = old_targets_owned
                    .iter()
                    .map(String::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                let new_target = doc.frontmatter.title.trim().to_string();
                let mut rewritten_links = 0usize;
                if !target_refs.is_empty() && !new_target.is_empty() {
                    if let Ok((updated, count)) = rewrite_vault_wikilinks(
                        &self.config.vault_dir,
                        &new_path,
                        &target_refs,
                        &new_target,
                    ) {
                        rewritten_links += count;
                        for (rel, udoc, umod, usize_bytes) in &updated {
                            let _ = self.index.upsert_note(rel, udoc, *umod, *usize_bytes);
                            if self.selected_path.as_deref() == Some(rel.as_str()) {
                                self.current_doc = udoc.clone();
                                self.current_modified = *umod;
                            }
                        }
                    }
                    if let Ok((updated_cols, col_count)) = rewrite_vault_collection_links(
                        &self.config.vault_dir,
                        &target_refs,
                        &new_target,
                    ) {
                        rewritten_links += col_count;
                        for (rel, ckind, cdoc, cmod, csize) in &updated_cols {
                            let _ = self.index.upsert_collection(rel, *ckind, cdoc, *cmod, *csize);
                            if self.active_collection.path.as_deref() == Some(rel.as_str()) {
                                self.active_collection.doc = cdoc.clone();
                                self.active_collection.modified = *cmod;
                            }
                        }
                    }
                    if let Ok((updated_canvases, canvas_count)) = rewrite_vault_canvas_links(
                        &self.config.vault_dir,
                        old_path,
                        &new_path,
                        &target_refs,
                        &new_target,
                    ) {
                        rewritten_links += canvas_count;
                        for (rel, cdoc, cmod, csize) in &updated_canvases {
                            let _ = self.index.upsert_canvas(rel, cdoc, *cmod, *csize);
                            if self.active_canvas.path.as_deref() == Some(rel.as_str()) {
                                self.active_canvas.doc = cdoc.clone();
                                self.active_canvas.modified = *cmod;
                            }
                        }
                    }
                }

                self.reload_index_caches();
                if self.selected_path.as_deref() == Some(old_path) {
                    self.selected_path = Some(new_path.clone());
                    self.current_doc = doc;
                    self.current_modified = modified;
                    self.dirty = false;
                }
                self.expand_parents(&new_path);
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
                if rewritten_links > 0 {
                    let s = strings(self.config.language);
                    self.push_toast(
                        format!("{}: {rewritten_links}", s.toast_links_updated),
                        ToastKind::Info,
                    );
                }
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn toggle_pin(&mut self, rel_path: &str) {
        if VaultFileKind::from_path(rel_path) == Some(VaultFileKind::Canvas) {
            if self.active_canvas.path.as_deref() == Some(rel_path) {
                self.active_canvas.doc.pinned = !self.active_canvas.doc.pinned;
                self.save_current_canvas();
                return;
            }
            self.watcher.mark_internal_change();
            match read_canvas(&self.config.vault_dir, rel_path) {
                Ok((mut doc, _, _)) => {
                    doc.pinned = !doc.pinned;
                    if let Ok((modified, size)) = save_canvas(&self.config.vault_dir, rel_path, &doc) {
                        let _ = self.index.upsert_canvas(rel_path, &doc, modified, size);
                        self.reload_index_caches();
                        self.last_local_save_at = Some(Instant::now());
                    }
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }
        if matches!(
            VaultFileKind::from_path(rel_path),
            Some(VaultFileKind::Board | VaultFileKind::Table)
        ) {
            if self.active_collection.path.as_deref() == Some(rel_path) {
                self.active_collection.doc.pinned = !self.active_collection.doc.pinned;
                self.save_current_collection();
                return;
            }
            self.watcher.mark_internal_change();
            match read_collection(&self.config.vault_dir, rel_path) {
                Ok((mut doc, _, _, kind)) => {
                    doc.pinned = !doc.pinned;
                    if let Ok((modified, size)) = save_collection(&self.config.vault_dir, rel_path, &doc) {
                        let _ = self.index.upsert_collection(rel_path, kind, &doc, modified, size);
                        self.reload_index_caches();
                    }
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }

        if self.selected_path.as_deref() == Some(rel_path) {
            self.current_doc.frontmatter.pinned = !self.current_doc.frontmatter.pinned;
            self.save_current_note();
            return;
        }
        self.watcher.mark_internal_change();
        match read_note(&self.config.vault_dir, rel_path) {
            Ok((mut doc, _, _)) => {
                doc.frontmatter.pinned = !doc.frontmatter.pinned;
                if let Ok((modified, size)) = save_note(&self.config.vault_dir, rel_path, &doc) {
                    let _ = self.index.upsert_note(rel_path, &doc, modified, size);
                    self.reload_index_caches();
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = Some(Instant::now());
                }
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn open_or_create_daily_note(&mut self, date: SimpleDate) {
        let nested_rel = daily_note_rel_path(date);
        let legacy_rel = daily_legacy_rel_path(date);
        let existing_rel = if self.notes.iter().any(|n| n.path == nested_rel)
            || self.config.vault_dir.join(&nested_rel).exists()
        {
            Some(nested_rel)
        } else if self.notes.iter().any(|n| n.path == legacy_rel)
            || self.config.vault_dir.join(&legacy_rel).exists()
        {
            Some(legacy_rel)
        } else {
            None
        };

        if let Some(rel) = existing_rel {
            let day_title = self
                .notes
                .iter()
                .find(|n| n.path == rel)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| crate::i18n::format_daily_title(self.config.language, date));
            if let Ok(hubs) = ensure_daily_hubs_for_date(
                &self.config.vault_dir,
                date,
                self.config.language,
                &day_title,
            ) {
                let any_hubs = !hubs.is_empty();
                for (hp, hdoc, hm, hsz) in hubs {
                    let _ = self.index.upsert_note(&hp, &hdoc, hm, hsz);
                }
                if any_hubs {
                    self.reload_index_caches();
                }
            }
            self.open_note(&rel);
            self.view_mode = ViewMode::Editor;
            self.previous_view_mode = ViewMode::Editor;
            self.last_notes_tab_mode = ViewMode::Editor;
            self.last_notes_showing_list = false;
            self.focus_body_next = true;
            return;
        }

        if self.dirty {
            self.save_current_note();
        }
        self.watcher.mark_internal_change();
        match create_daily_note(&self.config.vault_dir, date, self.config.language) {
            Ok(res) => {
                let _ = self
                    .index
                    .upsert_note(&res.day_path, &res.day_doc, res.day_modified, res.day_size);
                for (hp, hdoc, hm, hsz) in &res.hubs {
                    let _ = self.index.upsert_note(hp, hdoc, *hm, *hsz);
                }
                self.reload_index_caches();
                self.selected_path = Some(res.day_path.clone());
                self.current_doc = res.day_doc;
                self.current_modified = res.day_modified;
                self.dirty = false;
                self.external_conflict_open_note = false;
                self.last_edit_at = None;
                self.expand_parents(&res.day_path);
                self.refresh_document_derivations();
                self.view_mode = ViewMode::Editor;
                self.previous_view_mode = ViewMode::Editor;
                self.last_notes_tab_mode = ViewMode::Editor;
                self.last_notes_showing_list = false;
                self.editor_mode = EditorMode::Edit;
                self.focus_body_next = true;
                self.record_nav(NavTarget::Note(res.day_path));
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn save_external_conflict_copy(&mut self) {
        let Some(sel) = self.selected_path.clone() else {
            return;
        };
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let copy_rel = conflict_path(&sel, &ts);
        self.watcher.mark_internal_change();
        match save_note(&self.config.vault_dir, &copy_rel, &self.current_doc) {
            Ok((modified, size)) => {
                let _ = self
                    .index
                    .upsert_note(&copy_rel, &self.current_doc, modified, size);
                self.dirty = false;
                self.external_conflict_open_note = false;
                self.open_note(&sel);
                self.reload_index_caches();
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn perform_duplicate(&mut self, rel_path: &str) {
        let existing = self.existing_paths_set();
        if VaultFileKind::from_path(rel_path) == Some(VaultFileKind::Canvas) {
            if self.active_canvas.dirty && self.active_canvas.path.as_deref() == Some(rel_path) {
                self.save_current_canvas();
            }
            self.watcher.mark_internal_change();
            match duplicate_canvas(&self.config.vault_dir, rel_path, &existing) {
                Ok((new_path, doc, modified, size)) => {
                    let _ = self.index.upsert_canvas(&new_path, &doc, modified, size);
                    self.reload_index_caches();
                    self.open_canvas(&new_path);
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }
        if matches!(
            VaultFileKind::from_path(rel_path),
            Some(VaultFileKind::Board | VaultFileKind::Table)
        ) {
            if self.active_collection.dirty && self.active_collection.path.as_deref() == Some(rel_path) {
                self.save_current_collection();
            }
            self.watcher.mark_internal_change();
            match duplicate_collection(&self.config.vault_dir, rel_path, &existing) {
                Ok((new_path, doc, modified, size, kind)) => {
                    let _ = self.index.upsert_collection(&new_path, kind, &doc, modified, size);
                    self.reload_index_caches();
                    self.open_collection(&new_path);
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }

        if self.dirty && self.selected_path.as_deref() == Some(rel_path) {
            self.save_current_note();
        }
        self.watcher.mark_internal_change();
        match duplicate_note(&self.config.vault_dir, rel_path, &existing) {
            Ok((new_path, doc, modified, size)) => {
                let _ = self.index.upsert_note(&new_path, &doc, modified, size);
                self.reload_index_caches();
                self.open_note(&new_path);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn perform_delete(&mut self, rel_path: &str) {
        let kind = VaultFileKind::from_path(rel_path);
        let is_collection = matches!(kind, Some(VaultFileKind::Board | VaultFileKind::Table));
        let is_canvas = kind == Some(VaultFileKind::Canvas);
        self.watcher.mark_internal_change();
        match delete_note(&self.config.vault_dir, rel_path) {
            Ok(()) => {
                if is_canvas {
                    let _ = self.index.remove_collection(rel_path);
                    self.reload_index_caches();
                    if self.active_canvas.path.as_deref() == Some(rel_path) {
                        self.active_canvas.path = None;
                        self.active_canvas.dirty = false;
                        self.active_canvas.reset_transient();
                        if self.view_mode == ViewMode::Canvas {
                            self.view_mode = ViewMode::Editor;
                        }
                    }
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                } else if is_collection {
                    let _ = self.index.remove_collection(rel_path);
                    self.reload_index_caches();
                    if self.active_collection.path.as_deref() == Some(rel_path) {
                        self.active_collection.path = None;
                        self.active_collection.dirty = false;
                        self.active_collection.reset_transient();
                        if self.view_mode == ViewMode::Collection {
                            self.view_mode = ViewMode::Editor;
                        }
                    }
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                } else {
                    let _ = self.index.remove_note(rel_path);
                    self.reload_index_caches();
                    if self.selected_path.as_deref() == Some(rel_path) {
                        self.selected_path = None;
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        if let Some(next) = self.notes.first().map(|n| n.path.clone()) {
                            self.open_note(&next);
                        } else {
                            self.current_doc = NoteDocument::parse("", "");
                            self.refresh_document_derivations();
                            self.refresh_embedded_boards();
                        }
                    } else {
                        self.refresh_document_derivations();
                        self.refresh_embedded_boards();
                    }
                }
                self.recent_tabs.retain(|t| match t {
                    NavTarget::Note(p) | NavTarget::Collection(p) | NavTarget::Canvas(p) => {
                        p != rel_path
                    }
                    NavTarget::View(_) => true,
                });
                self.nav_history.retain(|t| match t {
                    NavTarget::Note(p) | NavTarget::Collection(p) | NavTarget::Canvas(p) => {
                        p != rel_path
                    }
                    NavTarget::View(_) => true,
                });
                if self.nav_cursor >= self.nav_history.len() {
                    self.nav_cursor = self.nav_history.len().saturating_sub(1);
                }
                self.last_local_save_at = Some(Instant::now());
                let s = strings(self.config.language);
                self.push_toast(s.toast_moved_to_trash, ToastKind::Info);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn switch_vault(&mut self, new_dir_str: &str) {
        if new_dir_str.is_empty() {
            return;
        }
        self.flush_unsaved_documents();
        let new_dir = PathBuf::from(new_dir_str);
        if let Err(e) = ensure_starter_vault(&new_dir, self.config.language) {
            self.push_toast(e, ToastKind::Error);
            return;
        }
        let _ = migrate_diary_hierarchy(&new_dir, self.config.language);
        let (mut new_idx, recovered) = VaultIndex::open_or_recover(&new_dir);
        let _ = new_idx.sync_from_disk(&new_dir);
        self.index = new_idx;
        self.config.vault_dir = new_dir.clone();
        let _ = self.config.save();
        self.vault_draft = new_dir.to_string_lossy().to_string();
        self.watcher.set_vault_dir(new_dir);
        self.selected_path = None;
        self.dirty = false;
        self.external_conflict_open_note = false;
        self.active_collection.path = None;
        self.active_collection.dirty = false;
        self.active_collection.reset_transient();
        self.active_canvas.path = None;
        self.active_canvas.dirty = false;
        self.active_canvas.reset_transient();
        self.last_sync_ts = self
            .index
            .get_meta("last_sync_time")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<i64>().ok());
        self.last_sync_commit = self.index.get_meta("last_sync_commit").ok().flatten();
        self.reload_index_caches();
        if let Some(first) = self.notes.first().map(|n| n.path.clone()) {
            self.open_note(&first);
        }
        if recovered {
            let s = strings(self.config.language);
            self.push_toast(s.toast_index_recovered, ToastKind::Info);
        } else {
            self.push_toast(
                if self.config.language == Lang::Ru {
                    "Хранилище обновлено"
                } else {
                    "Vault switched"
                },
                ToastKind::Success,
            );
        }
    }

    fn refresh_palette_hits(&mut self) {
        self.palette_hits = self.index.search(&self.palette_query).unwrap_or_default();
    }

    fn trigger_sync(&mut self, ctx: &egui::Context) {
        self.flush_unsaved_documents();
        self.last_local_save_at = None;
        if !self.config.is_github_configured() {
            if self.view_mode != ViewMode::Settings {
                self.previous_view_mode = self.view_mode;
            }
            self.view_mode = ViewMode::Settings;
            self.settings_tab = SettingsTab::GitHub;
            let s = strings(self.config.language);
            self.push_toast(s.sync_not_configured, ToastKind::Info);
            return;
        }
        let base_hashes = self.index.load_sync_state().unwrap_or_default();
        self.sync_worker.start_sync(
            self.config.vault_dir.clone(),
            base_hashes,
            self.config.github_repo.clone(),
            self.config.github_branch.clone(),
            self.config.github_token.clone(),
            self.config.language,
            ctx.clone(),
        );
    }

    fn refresh_embedded_boards(&mut self) {
        self.embedded_boards.clear();
        let targets = extract_embedded_boards(&self.current_doc.body);
        for target in targets {
            let norm_t = target.trim().replace('\\', "/").to_lowercase();
            let resolved = self
                .collections
                .iter()
                .find(|c| {
                    c.kind == VaultFileKind::Board
                        && (c.path.to_lowercase() == norm_t
                            || c.path.to_lowercase().ends_with(&format!("/{norm_t}"))
                            || vault_file_stem(&c.path).eq_ignore_ascii_case(vault_file_stem(&norm_t))
                            || c.title.trim().eq_ignore_ascii_case(vault_file_stem(&norm_t)))
                })
                .map(|c| c.path.clone())
                .unwrap_or_else(|| target.trim().replace('\\', "/"));
            if self.embedded_boards.contains_key(&resolved) {
                continue;
            }
            if let Ok((doc, _, _, VaultFileKind::Board)) =
                read_collection(&self.config.vault_dir, &resolved)
            {
                self.embedded_boards.insert(resolved, doc);
            }
        }
    }

    fn insert_markdown_into_current_note(&mut self, ctx: &egui::Context, snippet: &str) {
        if self.selected_path.is_none() {
            return;
        }
        let editor_id = if self.editor_mode == EditorMode::Split {
            egui::Id::new("krono_md_editor_split")
        } else {
            egui::Id::new("krono_md_editor_main")
        };
        let char_count = self.current_doc.body.chars().count();
        let mut inserted = false;

        if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
            if let Some(range) = state.cursor.char_range() {
                let char_pos = range.primary.index.min(char_count);
                let byte_pos = self
                    .current_doc
                    .body
                    .char_indices()
                    .nth(char_pos)
                    .map(|(b, _)| b)
                    .unwrap_or(self.current_doc.body.len());
                let need_lead = byte_pos > 0 && !self.current_doc.body[..byte_pos].ends_with('\n');
                let need_trail = byte_pos < self.current_doc.body.len()
                    && !self.current_doc.body[byte_pos..].starts_with('\n');
                let mut to_insert = String::new();
                if need_lead {
                    to_insert.push('\n');
                }
                to_insert.push_str(snippet);
                if need_trail {
                    to_insert.push('\n');
                }
                self.current_doc.body.insert_str(byte_pos, &to_insert);
                let new_char = char_pos + to_insert.chars().count();
                let cc = egui::text::CCursor::new(new_char);
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                state.store(ctx, editor_id);
                inserted = true;
            }
        }

        if !inserted {
            if !self.current_doc.body.is_empty() && !self.current_doc.body.ends_with('\n') {
                self.current_doc.body.push('\n');
            }
            if !self.current_doc.body.trim_end().is_empty() {
                self.current_doc.body.push('\n');
            }
            self.current_doc.body.push_str(snippet);
            self.current_doc.body.push('\n');
        }
        self.save_current_note();
    }

    fn add_image_card_to_active_canvas(
        &mut self,
        rel_path: &str,
        aspect_dims: Option<(u32, u32)>,
        stagger_idx: usize,
    ) {
        if self.active_canvas.path.is_none() {
            return;
        }
        self.active_canvas.push_undo();
        let (cw, ch) = match aspect_dims {
            Some((w, h)) if w > 0 && h > 0 => {
                let wf = w as f32;
                let hf = h as f32;
                let scale = (340.0 / wf).min(260.0 / hf);
                let nw = (wf * scale).clamp(150.0, 360.0);
                let nh = (hf * scale).clamp(110.0, 280.0);
                ((nw / 10.0).round() * 10.0, (nh / 10.0).round() * 10.0)
            }
            _ => (300.0, 220.0),
        };
        let center = self.active_canvas.viewport.pan;
        let offset = (stagger_idx as f32) * 28.0;
        let x = ((center.x - cw * 0.5 + offset) / 20.0).round() * 20.0;
        let y = ((center.y - ch * 0.5 + offset) / 20.0).round() * 20.0;
        let id = self.active_canvas.doc.add_file_node(x, y, rel_path, None);
        if let Some(node) = self.active_canvas.doc.nodes.iter_mut().find(|n| n.id == id) {
            node.width = cw;
            node.height = ch;
        }
        self.active_canvas.selected_node_id = Some(id);
        self.active_canvas.selected_edge_id = None;
        self.save_current_canvas();
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_imported_image(
        &mut self,
        ctx: &egui::Context,
        target: ImagePickTarget,
        rel_path: String,
        bytes: &[u8],
        modified: i64,
        size: u64,
        aspect_dims: Option<(u32, u32)>,
        stagger_idx: usize,
    ) {
        let _ = self.index.upsert_asset(&rel_path, bytes, modified, size);
        self.reload_index_caches();
        self.last_local_save_at = Some(Instant::now());

        match target {
            ImagePickTarget::Editor => {
                let alt = vault_file_stem(&rel_path).to_string();
                let md = format!("![{alt}]({rel_path})");
                self.insert_markdown_into_current_note(ctx, &md);
            }
            ImagePickTarget::Canvas => {
                self.add_image_card_to_active_canvas(&rel_path, aspect_dims, stagger_idx);
            }
        }
        let s = strings(self.config.language);
        self.push_toast(format!("{} ({rel_path})", s.toast_image_inserted), ToastKind::Success);
    }

    fn spawn_image_picker(&mut self, ctx: &egui::Context, target: ImagePickTarget) {
        if self.pending_image_pick.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx_wake = ctx.clone();
        self.pending_image_pick = Some((target, rx));
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                .pick_file();
            let _ = tx.send(picked);
            ctx_wake.request_repaint();
        });
    }

    fn try_paste_image_from_clipboard(&mut self, ctx: &egui::Context, warn_if_empty: bool) {
        let target = if self.view_mode == ViewMode::Canvas && self.active_canvas.path.is_some() {
            ImagePickTarget::Canvas
        } else if self.view_mode == ViewMode::Editor && self.selected_path.is_some() {
            ImagePickTarget::Editor
        } else {
            return;
        };

        let Ok(mut cb) = arboard::Clipboard::new() else {
            return;
        };
        if let Ok(img) = cb.get_image() {
            match encode_rgba_to_png(img.width, img.height, &img.bytes) {
                Ok(png_bytes) => {
                    let existing = self.existing_paths_set();
                    let stem = format!("paste-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                    self.watcher.mark_internal_change();
                    match save_image_bytes_to_assets(
                        &self.config.vault_dir,
                        &stem,
                        "png",
                        &png_bytes,
                        &existing,
                    ) {
                        Ok((rel_path, modified, size)) => {
                            let dims = Some((img.width as u32, img.height as u32));
                            self.handle_imported_image(
                                ctx,
                                target,
                                rel_path,
                                &png_bytes,
                                modified,
                                size,
                                dims,
                                0,
                            );
                            ctx.request_repaint();
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }

        if let Ok(files) = cb.get().file_list() {
            let mut imported = 0usize;
            for path in files {
                if !is_image_ext(&path.to_string_lossy()) {
                    continue;
                }
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match import_image_file_to_assets(&self.config.vault_dir, &path, &existing) {
                    Ok((rel_path, bytes, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            imported,
                        );
                        imported += 1;
                        ctx.request_repaint();
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            if imported > 0 {
                return;
            }
        }

        if warn_if_empty {
            let msg = if self.config.language == Lang::Ru {
                "В буфере обмена нет изображения"
            } else {
                "No image found in clipboard"
            };
            self.push_toast(msg, ToastKind::Info);
        }
    }

    fn process_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let target = if self.view_mode == ViewMode::Canvas && self.active_canvas.path.is_some() {
            ImagePickTarget::Canvas
        } else if self.selected_path.is_some() {
            ImagePickTarget::Editor
        } else {
            return;
        };

        for (idx, df) in dropped.into_iter().enumerate() {
            if let Some(path) = df.path {
                if !is_image_ext(&path.to_string_lossy()) {
                    continue;
                }
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match import_image_file_to_assets(&self.config.vault_dir, &path, &existing) {
                    Ok((rel_path, bytes, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            idx,
                        );
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            } else if let Some(bytes) = df.bytes {
                let name = if df.name.is_empty() {
                    "dropped.png".to_string()
                } else {
                    df.name
                };
                if !is_image_ext(&name) {
                    continue;
                }
                let ext = std::path::Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let stem = std::path::Path::new(&name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("dropped");
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match save_image_bytes_to_assets(&self.config.vault_dir, stem, ext, &bytes, &existing) {
                    Ok((rel_path, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            idx,
                        );
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
        }
    }

    fn create_linked_board_for_current_note(&mut self) {
        let Some(note_path) = self.selected_path.clone() else {
            return;
        };
        let is_ru = self.config.language == Lang::Ru;
        let note_title = if self.current_doc.frontmatter.title.trim().is_empty() {
            vault_file_stem(&note_path).to_string()
        } else {
            self.current_doc.frontmatter.title.trim().to_string()
        };
        let board_title = if is_ru {
            format!("{note_title} — Задачи")
        } else {
            format!("{note_title} — Tasks")
        };
        let raw_path = if note_path.starts_with("daily/") {
            let ym = parse_daily_note_path(&note_path)
                .map(|d| (d.year, d.month))
                .unwrap_or_else(|| {
                    let t = today_local();
                    (t.year, t.month)
                });
            format!("daily/boards/{:04}/{:02}/{board_title}", ym.0, ym.1)
        } else {
            match note_path.rsplit_once('/') {
                Some((dir, _)) if !dir.is_empty() => format!("{dir}/{board_title}"),
                _ => board_title,
            }
        };
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        match create_collection_preset(
            &self.config.vault_dir,
            &raw_path,
            CollectionViewKind::Board,
            "kanban",
            is_ru,
            &existing,
        ) {
            Ok((board_path, fkind, mut doc, mut modified, mut size)) => {
                if let Some(nl_col_id) = doc
                    .columns
                    .iter()
                    .find(|c| c.col_type == ColumnType::NoteLink)
                    .map(|c| c.id.clone())
                {
                    if doc.records.is_empty() {
                        doc.add_record(
                            if is_ru { "Первая задача" } else { "First task" },
                            None,
                            chrono::Utc::now().timestamp(),
                        );
                    }
                    if let Some(first_rec) = doc.records.first_mut() {
                        first_rec
                            .values
                            .insert(nl_col_id, FieldValue::Text(note_path.clone()));
                    }
                    if let Ok((m2, s2)) = save_collection(&self.config.vault_dir, &board_path, &doc) {
                        modified = m2;
                        size = s2;
                    }
                }
                let _ = self
                    .index
                    .upsert_collection(&board_path, fkind, &doc, modified, size);
                self.expand_parents(&board_path);

                let heading = if is_ru { "## Задачи" } else { "## Tasks" };
                let embed = format!("![[{board_path}]]");
                if !self.current_doc.body.is_empty() && !self.current_doc.body.ends_with('\n') {
                    self.current_doc.body.push('\n');
                }
                if !self.current_doc.body.trim_end().is_empty() {
                    self.current_doc.body.push('\n');
                }
                self.current_doc.body.push_str(&format!("{heading}\n{embed}\n"));
                self.save_current_note();
                self.reload_index_caches();
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
                self.push_toast(
                    if is_ru {
                        format!("Создана и привязана доска: {}", doc.title)
                    } else {
                        format!("Created & linked board: {}", doc.title)
                    },
                    ToastKind::Success,
                );
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    fn handle_embedded_board_action(&mut self, act: EmbeddedBoardAction) {
        match act {
            EmbeddedBoardAction::OpenFull(path) => {
                self.open_collection(&path);
                self.mobile_showing_list = false;
                self.drawer_sidebar_open = false;
            }
            EmbeddedBoardAction::AddCard {
                board_path,
                option_id,
                title,
            } => {
                self.watcher.mark_internal_change();
                match read_collection(&self.config.vault_dir, &board_path) {
                    Ok((mut doc, _, _, kind)) => {
                        let gid = doc.effective_group_column_id().map(str::to_string);
                        let rec_id = doc.add_record(
                            &title,
                            gid.as_deref().map(|c| (c, option_id.as_str())),
                            chrono::Utc::now().timestamp(),
                        );
                        if let Some(note_path) = self.selected_path.clone() {
                            if let Some(nl_col_id) = doc
                                .columns
                                .iter()
                                .find(|c| c.col_type == ColumnType::NoteLink)
                                .map(|c| c.id.clone())
                            {
                                if let Some(rec) = doc.records.iter_mut().find(|r| r.id == rec_id) {
                                    rec.values.insert(nl_col_id, FieldValue::Text(note_path));
                                }
                            }
                        }
                        if let Ok((modified, size)) =
                            save_collection(&self.config.vault_dir, &board_path, &doc)
                        {
                            let _ = self
                                .index
                                .upsert_collection(&board_path, kind, &doc, modified, size);
                            self.embedded_boards.insert(board_path.clone(), doc.clone());
                            if self.active_collection.path.as_deref() == Some(board_path.as_str()) {
                                self.active_collection.doc = doc;
                                self.active_collection.modified = modified;
                            }
                            self.reload_index_caches();
                            self.refresh_document_derivations();
                            self.refresh_embedded_boards();
                            self.last_local_save_at = Some(Instant::now());
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            EmbeddedBoardAction::MoveCard {
                board_path,
                record_id,
                target_option_id,
            } => {
                self.watcher.mark_internal_change();
                match read_collection(&self.config.vault_dir, &board_path) {
                    Ok((mut doc, _, _, kind)) => {
                        if let Some(gid) = doc.effective_group_column_id().map(str::to_string) {
                            doc.move_record_to_group(&record_id, &gid, &target_option_id, None);
                            if let Ok((modified, size)) =
                                save_collection(&self.config.vault_dir, &board_path, &doc)
                            {
                                let _ = self
                                    .index
                                    .upsert_collection(&board_path, kind, &doc, modified, size);
                                self.embedded_boards.insert(board_path.clone(), doc.clone());
                                if self.active_collection.path.as_deref() == Some(board_path.as_str()) {
                                    self.active_collection.doc = doc;
                                    self.active_collection.modified = modified;
                                }
                                self.reload_index_caches();
                                self.last_local_save_at = Some(Instant::now());
                            }
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            EmbeddedBoardAction::ReorderCard {
                board_path,
                record_id,
                delta,
            } => {
                self.watcher.mark_internal_change();
                match read_collection(&self.config.vault_dir, &board_path) {
                    Ok((mut doc, _, _, kind)) => {
                        if let Some(gid) = doc.effective_group_column_id().map(str::to_string) {
                            if doc.shift_record_in_group(&record_id, &gid, delta) {
                                if let Ok((modified, size)) =
                                    save_collection(&self.config.vault_dir, &board_path, &doc)
                                {
                                    let _ = self
                                        .index
                                        .upsert_collection(&board_path, kind, &doc, modified, size);
                                    self.embedded_boards.insert(board_path.clone(), doc.clone());
                                    if self.active_collection.path.as_deref() == Some(board_path.as_str()) {
                                        self.active_collection.doc = doc;
                                        self.active_collection.modified = modified;
                                    }
                                    self.reload_index_caches();
                                    self.last_local_save_at = Some(Instant::now());
                                }
                            }
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            EmbeddedBoardAction::DeleteCard {
                board_path,
                record_id,
            } => {
                self.watcher.mark_internal_change();
                match read_collection(&self.config.vault_dir, &board_path) {
                    Ok((mut doc, _, _, kind)) => {
                        doc.remove_record(&record_id);
                        if let Ok((modified, size)) =
                            save_collection(&self.config.vault_dir, &board_path, &doc)
                        {
                            let _ = self
                                .index
                                .upsert_collection(&board_path, kind, &doc, modified, size);
                            self.embedded_boards.insert(board_path.clone(), doc.clone());
                            if self.active_collection.path.as_deref() == Some(board_path.as_str()) {
                                self.active_collection.doc = doc;
                                self.active_collection.modified = modified;
                            }
                            self.reload_index_caches();
                            self.last_local_save_at = Some(Instant::now());
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
        }
    }

    fn poll_background_events(&mut self, ctx: &egui::Context) {
        if let Some((target, rx)) = self.pending_image_pick.take() {
            match rx.try_recv() {
                Ok(Some(src_path)) => {
                    let existing = self.existing_paths_set();
                    self.watcher.mark_internal_change();
                    match import_image_file_to_assets(&self.config.vault_dir, &src_path, &existing) {
                        Ok((rel_path, bytes, modified, size)) => {
                            let dims = image_dimensions_from_bytes(&bytes);
                            self.handle_imported_image(
                                ctx,
                                target,
                                rel_path,
                                &bytes,
                                modified,
                                size,
                                dims,
                                0,
                            );
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
                Ok(None) | Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => {
                    self.pending_image_pick = Some((target, rx));
                }
            }
        }
        if self.watcher.poll_changed() {
            if let Ok(report) = self.index.sync_from_disk(&self.config.vault_dir) {
                if !report.is_empty() {
                    self.reload_index_caches();
                    if let Some(sel) = self.selected_path.clone() {
                        if report.touches(&sel) {
                            if self.dirty {
                                self.external_conflict_open_note = true;
                            } else if let Ok((doc, modified, _)) =
                                read_note(&self.config.vault_dir, &sel)
                            {
                                self.current_doc = doc;
                                self.current_modified = modified;
                                self.external_conflict_open_note = false;
                            } else {
                                self.selected_path = self.notes.first().map(|n| n.path.clone());
                            }
                        }
                    }
                    if let Some(col_path) = self.active_collection.path.clone() {
                        if report.touches(&col_path) {
                            if self.active_collection.dirty {
                                self.active_collection.external_conflict = true;
                            } else if let Ok((doc, modified, _, _)) =
                                read_collection(&self.config.vault_dir, &col_path)
                            {
                                self.active_collection.doc = doc;
                                self.active_collection.modified = modified;
                                self.active_collection.external_conflict = false;
                            } else {
                                self.active_collection.path = None;
                                if self.view_mode == ViewMode::Collection {
                                    self.view_mode = ViewMode::Editor;
                                }
                            }
                        }
                    }
                    if let Some(canvas_path) = self.active_canvas.path.clone() {
                        if report.touches(&canvas_path) {
                            if self.active_canvas.dirty {
                                self.active_canvas.external_conflict = true;
                            } else if let Ok((doc, modified, _)) =
                                read_canvas(&self.config.vault_dir, &canvas_path)
                            {
                                self.active_canvas.doc = doc;
                                self.active_canvas.modified = modified;
                                self.active_canvas.external_conflict = false;
                            } else {
                                self.active_canvas.path = None;
                                if self.view_mode == ViewMode::Canvas {
                                    self.view_mode = ViewMode::Editor;
                                }
                            }
                        }
                    }
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                }
            }
        }

        for ev in self.sync_worker.poll() {
            match ev {
                SyncEvent::TestResult(Ok(info)) => {
                    let msg = match self.config.language {
                        Lang::Ru => format!(
                            "Подключено к {} ({}, ветка: {})",
                            info.full_name,
                            if info.private { "приватный" } else { "публичный" },
                            info.default_branch
                        ),
                        Lang::En => format!(
                            "Connected to {} ({}, default branch: {})",
                            info.full_name,
                            if info.private { "private" } else { "public" },
                            info.default_branch
                        ),
                    };
                    self.test_feedback = Some((true, msg.clone()));
                    self.push_toast(msg, ToastKind::Success);
                }
                SyncEvent::TestResult(Err(err)) => {
                    self.test_feedback = Some((false, err.clone()));
                    self.push_toast(err, ToastKind::Error);
                }
                SyncEvent::SyncResult(Ok(summary)) => {
                    self.watcher.mark_internal_change();
                    let _ = self.index.save_sync_state(&summary.new_base_hashes);
                    let now = chrono::Utc::now().timestamp();
                    let _ = self.index.set_meta("last_sync_time", &now.to_string());
                    let _ = self.index.set_meta("last_sync_commit", &summary.head_sha);
                    self.last_sync_ts = Some(now);
                    self.last_sync_commit = Some(summary.head_sha.clone());

                    let _ = self.index.sync_from_disk(&self.config.vault_dir);
                    self.reload_index_caches();
                    if let Some(sel) = self.selected_path.clone() {
                        if let Ok((doc, modified, _)) = read_note(&self.config.vault_dir, &sel) {
                            self.current_doc = doc;
                            self.current_modified = modified;
                        } else {
                            self.selected_path = None;
                            if let Some(first) = self.notes.first().map(|n| n.path.clone()) {
                                self.open_note(&first);
                            }
                        }
                    }
                    if let Some(col_path) = self.active_collection.path.clone() {
                        if let Ok((doc, modified, _, _)) =
                            read_collection(&self.config.vault_dir, &col_path)
                        {
                            self.active_collection.doc = doc;
                            self.active_collection.modified = modified;
                            self.active_collection.external_conflict = false;
                        } else {
                            self.active_collection.path = None;
                            if self.view_mode == ViewMode::Collection {
                                self.view_mode = ViewMode::Editor;
                            }
                        }
                    }
                    if let Some(canvas_path) = self.active_canvas.path.clone() {
                        if let Ok((doc, modified, _)) =
                            read_canvas(&self.config.vault_dir, &canvas_path)
                        {
                            self.active_canvas.doc = doc;
                            self.active_canvas.modified = modified;
                            self.active_canvas.external_conflict = false;
                        } else {
                            self.active_canvas.path = None;
                            if self.view_mode == ViewMode::Canvas {
                                self.view_mode = ViewMode::Editor;
                            }
                        }
                    }
                    self.refresh_document_derivations();
                    self.refresh_embedded_boards();
                    self.last_local_save_at = None;

                    let removed = summary.deleted_local + summary.deleted_remote;
                    let msg = match self.config.language {
                        Lang::Ru => format!(
                            "GitHub Sync: +{} / ↓{} / -{} · конфликты: {}",
                            summary.pushed,
                            summary.pulled,
                            removed,
                            summary.conflicts.len()
                        ),
                        Lang::En => format!(
                            "GitHub Sync: +{} / ↓{} / -{} · conflicts: {}",
                            summary.pushed,
                            summary.pulled,
                            removed,
                            summary.conflicts.len()
                        ),
                    };
                    let kind = if summary.conflicts.is_empty() {
                        ToastKind::Success
                    } else {
                        ToastKind::Info
                    };
                    self.test_feedback = Some((true, msg.clone()));
                    self.push_toast(msg, kind);
                }
                SyncEvent::SyncResult(Err(err)) => {
                    self.test_feedback = Some((false, err.clone()));
                    self.push_toast(err, ToastKind::Error);
                }
            }
        }

        for ev in self.update_worker.poll() {
            let s = strings(self.config.language);
            match ev {
                UpdateEvent::CheckFinished {
                    update: Ok(Some(upd)),
                    silent: _,
                } => {
                    let msg = format!("{}: {}", s.update_available_badge, upd.tag_name);
                    self.update_status_msg = None;
                    self.available_update = Some(upd);
                    self.push_toast(msg, ToastKind::Info);
                }
                UpdateEvent::CheckFinished {
                    update: Ok(None),
                    silent,
                } => {
                    self.available_update = None;
                    self.update_status_msg = Some(s.update_up_to_date.to_string());
                    if !silent {
                        self.push_toast(s.update_up_to_date, ToastKind::Success);
                    }
                }
                UpdateEvent::CheckFinished {
                    update: Err(err),
                    silent,
                } => {
                    self.update_status_msg = Some(err.clone());
                    if !silent {
                        self.push_toast(err, ToastKind::Error);
                    }
                }
                UpdateEvent::InstallFinished(Ok(outcome)) => match outcome {
                    UpdateApplyOutcome::LaunchedInstallerExitApp(msg)
                    | UpdateApplyOutcome::ReplacedAppImageExitApp(msg) => {
                        self.push_toast(msg, ToastKind::Success);
                        self.flush_unsaved_documents();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    UpdateApplyOutcome::OpenedPackage(msg) => {
                        self.update_status_msg = Some(msg.clone());
                        self.push_toast(msg, ToastKind::Success);
                    }
                },
                UpdateEvent::InstallFinished(Err(err)) => {
                    self.update_status_msg = Some(err.clone());
                    self.push_toast(err, ToastKind::Error);
                }
            }
        }
    }

    fn handle_actions(&mut self, ctx: &egui::Context, actions: Vec<UiAction>) {
        for act in actions {
            match act {
                UiAction::OpenNote(path) => {
                    match VaultFileKind::from_path(&path) {
                        Some(VaultFileKind::Board | VaultFileKind::Table) => {
                            self.open_collection(&path);
                        }
                        Some(VaultFileKind::Canvas) => {
                            self.open_canvas(&path);
                        }
                        _ => {
                            self.open_note(&path);
                            if self.view_mode != ViewMode::Editor {
                                self.view_anim_at = Some(Instant::now());
                            }
                            self.view_mode = ViewMode::Editor;
                            self.previous_view_mode = ViewMode::Editor;
                            self.last_notes_tab_mode = ViewMode::Editor;
                        }
                    }
                    self.mobile_showing_list = false;
                    self.last_notes_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::CreateNote { title, status } => {
                    self.create_and_open_note(&title, status, "");
                    self.focus_title_next = true;
                    self.mobile_showing_list = false;
                    self.last_notes_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                    self.last_notes_tab_mode = ViewMode::Editor;
                }
                UiAction::OpenOrCreateWikilink(target) => {
                    self.mobile_showing_list = false;
                    self.drawer_inspector_open = false;
                    if let Some(path) = self.find_note_by_wikilink(&target) {
                        self.open_note(&path);
                        self.view_mode = ViewMode::Editor;
                    } else {
                        self.create_and_open_note(&target, NoteStatus::Todo, "");
                        let msg = match self.config.language {
                            Lang::Ru => format!("Создана заметка «{target}»"),
                            Lang::En => format!("Created note \"{target}\""),
                        };
                        self.push_toast(msg, ToastKind::Info);
                    }
                }
                UiAction::SaveCurrentNote => self.save_current_note(),
                UiAction::UpdateNoteStatus { path, status } => {
                    self.update_note_status(&path, status);
                }
                UiAction::MoveNoteInBoard {
                    path,
                    status,
                    insert_before,
                } => {
                    self.move_note_in_board(&path, status, insert_before.as_deref());
                }
                UiAction::ShiftNoteInBoard { path, delta } => {
                    self.shift_note_in_board(&path, delta);
                }
                UiAction::NavBack => {
                    if self.can_nav_back() {
                        self.nav_cursor -= 1;
                        if let Some(tgt) = self.nav_history.get(self.nav_cursor).cloned() {
                            self.navigate_to_target(tgt, false);
                        }
                    }
                }
                UiAction::NavForward => {
                    if self.can_nav_forward() {
                        self.nav_cursor += 1;
                        if let Some(tgt) = self.nav_history.get(self.nav_cursor).cloned() {
                            self.navigate_to_target(tgt, false);
                        }
                    }
                }
                UiAction::OpenNavTarget(target) => {
                    self.navigate_to_target(target, true);
                }
                UiAction::CloseRecentTab(target) => {
                    self.recent_tabs.retain(|t| t != &target);
                }
                UiAction::OpenCreateModalInFolder(folder) => {
                    self.create_modal.open_with(CreateItemKind::Note);
                    self.create_modal.folder = folder;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::SendTaskToBoard {
                    task_idx,
                    board_path,
                    move_task,
                } => {
                    self.handle_send_task_to_board(task_idx, &board_path, move_task);
                }
                UiAction::RefreshAssetAudit => {
                    self.refresh_asset_audit();
                }
                UiAction::DeleteUnusedAssets => {
                    self.delete_unused_assets();
                }
                UiAction::DeleteSingleAsset(rel) => {
                    self.delete_single_asset(&rel);
                }
                UiAction::StartRename(path) => {
                    let current_title = self
                        .notes
                        .iter()
                        .find(|n| n.path == path)
                        .map(|n| n.title.clone())
                        .or_else(|| {
                            self.collections
                                .iter()
                                .find(|c| c.path == path)
                                .map(|c| c.title.clone())
                        })
                        .unwrap_or_else(|| path.clone());
                    self.rename_draft = current_title;
                    self.rename_target = Some(path);
                }
                UiAction::DuplicateNote(path) => self.perform_duplicate(&path),
                UiAction::RequestDelete(path) => self.delete_target = Some(path),
                UiAction::SetViewMode(mode) => {
                    if self.view_mode != mode {
                        self.view_anim_at = Some(Instant::now());
                    }
                    if mode != ViewMode::Settings {
                        self.previous_view_mode = mode;
                    }
                    if matches!(mode, ViewMode::Editor | ViewMode::Collection | ViewMode::Canvas) {
                        self.last_notes_tab_mode = mode;
                    }
                    self.view_mode = mode;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    if matches!(
                        mode,
                        ViewMode::Table | ViewMode::Board | ViewMode::Graph | ViewMode::Overview
                    ) {
                        self.record_nav(NavTarget::View(mode));
                    }
                }
                UiAction::ReturnToNotesTab => {
                    let target = match self.last_notes_tab_mode {
                        ViewMode::Canvas if self.active_canvas.path.is_some() => ViewMode::Canvas,
                        ViewMode::Collection if self.active_collection.path.is_some() => {
                            ViewMode::Collection
                        }
                        _ => ViewMode::Editor,
                    };
                    if self.view_mode != target {
                        self.view_anim_at = Some(Instant::now());
                    }
                    self.view_mode = target;
                    self.previous_view_mode = target;
                    self.mobile_showing_list = self.last_notes_showing_list;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::SetEditorMode(mode) => {
                    self.editor_mode = mode;
                    self.note_anim_at = Some(Instant::now());
                }
                UiAction::MobileShowNotesList => {
                    self.view_mode = ViewMode::Editor;
                    self.previous_view_mode = ViewMode::Editor;
                    self.mobile_showing_list = true;
                    self.last_notes_tab_mode = ViewMode::Editor;
                    self.last_notes_showing_list = true;
                    self.drawer_inspector_open = false;
                }
                UiAction::OpenCreateModal(opt_kind) => {
                    self.create_modal.open_with(opt_kind.unwrap_or(CreateItemKind::Note));
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::CreateFromModal {
                    kind,
                    title,
                    folder,
                    preset_id,
                } => {
                    self.create_from_modal(kind, &title, &folder, &preset_id);
                }
                UiAction::ToggleSidebar => {
                    if self.last_layout_class.is_desktop() {
                        self.config.sidebar_open = !self.config.sidebar_open;
                        let _ = self.config.save();
                    } else {
                        self.drawer_sidebar_open = !self.drawer_sidebar_open;
                        if self.drawer_sidebar_open {
                            self.drawer_inspector_open = false;
                        }
                    }
                }
                UiAction::ToggleInspector => {
                    if self.last_layout_class.is_desktop() {
                        self.config.inspector_open = !self.config.inspector_open;
                        let _ = self.config.save();
                    } else {
                        self.drawer_inspector_open = !self.drawer_inspector_open;
                        if self.drawer_inspector_open {
                            self.drawer_sidebar_open = false;
                        }
                    }
                }
                UiAction::ToggleTheme => {
                    self.config.toggle_theme_polarity();
                    self.set_target_palette(ctx, false);
                    let _ = self.config.save();
                }
                UiAction::ToggleLang => {
                    self.config.language = self.config.language.toggle();
                    let _ = self.config.save();
                }
                UiAction::OpenPalette => {
                    self.palette_open = true;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.refresh_palette_hits();
                }
                UiAction::OpenSettings(tab) => {
                    if self.view_mode != ViewMode::Settings {
                        self.previous_view_mode = self.view_mode;
                        self.view_anim_at = Some(Instant::now());
                    }
                    if self.settings_tab != tab {
                        self.settings_tab_anim_at = Some(Instant::now());
                    }
                    self.view_mode = ViewMode::Settings;
                    self.settings_tab = tab;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    self.vault_draft = self.config.vault_dir.to_string_lossy().to_string();
                    if tab == SettingsTab::Storage {
                        self.refresh_asset_audit();
                    }
                }
                UiAction::CloseSettings => {
                    if self.view_mode == ViewMode::Settings {
                        self.view_anim_at = Some(Instant::now());
                        self.view_mode = if self.previous_view_mode == ViewMode::Settings {
                            ViewMode::Editor
                        } else {
                            self.previous_view_mode
                        };
                    }
                }
                UiAction::SyncNow => self.trigger_sync(ctx),
                UiAction::TestGitHubConnection => {
                    let _ = self.config.save();
                    self.sync_worker.start_test(
                        self.config.github_repo.clone(),
                        self.config.github_token.clone(),
                        self.config.language,
                        ctx.clone(),
                    );
                }
                UiAction::SetTagFilter(tag) => self.active_tag_filter = tag,
                UiAction::ToggleFolder(folder) => {
                    if !self.expanded_folders.remove(&folder) {
                        self.expanded_folders.insert(folder);
                    }
                }
                UiAction::MarkDirty => {
                    self.dirty = true;
                    self.last_edit_at = Some(Instant::now());
                    self.cached_headings = extract_headings(&self.current_doc.body);
                    let refs = extract_embedded_boards(&self.current_doc.body);
                    if refs.len() != self.embedded_boards.len() {
                        self.refresh_embedded_boards();
                    }
                }
                UiAction::ToggleTask(task_idx) => {
                    if let Some(updated) = toggle_markdown_task(&self.current_doc.body, task_idx) {
                        self.current_doc.body = updated;
                        self.dirty = true;
                        self.last_edit_at = Some(Instant::now());
                        self.save_current_note();
                    }
                }
                UiAction::CopyText(text) => {
                    self.copied_code_hash = Some(hash_str(&text));
                    self.copied_code_at = Some(Instant::now());
                    ctx.output_mut(|o| o.copied_text = text);
                }
                UiAction::ApplyVaultDir(dir) => self.switch_vault(&dir),
                UiAction::OpenVaultFolder => {
                    if let Err(e) = open_in_file_manager(&self.config.vault_dir) {
                        self.push_toast(e, ToastKind::Error);
                    }
                }
                UiAction::RebuildIndex => {
                    match self.index.rebuild_from_disk(&self.config.vault_dir) {
                        Ok(()) => {
                            self.reload_index_caches();
                            self.refresh_document_derivations();
                            self.refresh_embedded_boards();
                            let s = strings(self.config.language);
                            self.push_toast(s.toast_index_rebuilt, ToastKind::Success);
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
                UiAction::ConfigChanged => {
                    self.set_target_palette(ctx, false);
                    let _ = self.config.save();
                }
                UiAction::TogglePin(path) => {
                    self.toggle_pin(&path);
                }
                UiAction::OpenOrCreateDailyNote(date) => {
                    self.open_or_create_daily_note(date);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                }
                UiAction::OpenTodayNote => {
                    let today = today_local();
                    self.calendar_year = today.year;
                    self.calendar_month = today.month;
                    self.open_or_create_daily_note(today);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                }
                UiAction::ToggleCalendar => {
                    self.config.calendar_open = !self.config.calendar_open;
                    let _ = self.config.save();
                }
                UiAction::ExternalReloadFromDisk => {
                    if let Some(path) = self.selected_path.clone() {
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        self.open_note(&path);
                    }
                }
                UiAction::ExternalKeepMine => {
                    self.external_conflict_open_note = false;
                    self.save_current_note();
                }
                UiAction::ExternalSaveConflictCopy => {
                    self.save_external_conflict_copy();
                }
                UiAction::OpenCollection(path) => {
                    self.open_collection(&path);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::CreateCollection { title, kind } => {
                    self.create_and_open_collection(&title, kind);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::SetCollectionView(vkind) => {
                    self.active_collection.active_view = vkind;
                }
                UiAction::SaveCurrentCollection => {
                    self.save_current_collection();
                }
                UiAction::MarkCollectionDirty => {
                    self.active_collection.dirty = true;
                    self.active_collection.last_edit_at = Some(Instant::now());
                }
                UiAction::CollectionExternalReload => {
                    if let Some(p) = self.active_collection.path.clone() {
                        self.active_collection.dirty = false;
                        self.active_collection.external_conflict = false;
                        self.open_collection(&p);
                    }
                }
                UiAction::CollectionExternalKeepMine => {
                    self.active_collection.external_conflict = false;
                    self.save_current_collection();
                }
                UiAction::CollectionExternalSaveConflictCopy => {
                    self.save_collection_external_conflict_copy();
                }
                UiAction::OpenCanvas(path) => {
                    self.open_canvas(&path);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::SaveCurrentCanvas => {
                    self.save_current_canvas();
                }
                UiAction::MarkCanvasDirty => {
                    self.active_canvas.dirty = true;
                    self.active_canvas.last_edit_at = Some(Instant::now());
                }
                UiAction::CanvasExternalReload => {
                    if let Some(p) = self.active_canvas.path.clone() {
                        self.active_canvas.dirty = false;
                        self.active_canvas.external_conflict = false;
                        self.open_canvas(&p);
                    }
                }
                UiAction::CanvasExternalKeepMine => {
                    self.active_canvas.external_conflict = false;
                    self.save_current_canvas();
                }
                UiAction::CanvasExternalSaveConflictCopy => {
                    self.save_canvas_external_conflict_copy();
                }
                UiAction::ExportCanvasToMarkdown => {
                    self.export_active_canvas_to_markdown();
                }
                UiAction::OpenGraphForNode(path) => {
                    self.graph_state
                        .sync_if_needed(&self.notes, &self.collections, &self.all_links);
                    self.graph_state.focus_node(&path, ctx.screen_rect());
                    if self.view_mode != ViewMode::Graph {
                        self.view_anim_at = Some(Instant::now());
                    }
                    self.view_mode = ViewMode::Graph;
                    self.previous_view_mode = ViewMode::Graph;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    self.mobile_showing_list = false;
                }
                UiAction::PickImageForEditor => {
                    self.spawn_image_picker(ctx, ImagePickTarget::Editor);
                }
                UiAction::PickImageForCanvas => {
                    self.spawn_image_picker(ctx, ImagePickTarget::Canvas);
                }
                UiAction::PasteClipboardImage => {
                    self.try_paste_image_from_clipboard(ctx, true);
                }
                UiAction::OpenImageLightbox(path) => {
                    self.lightbox_image = Some(path);
                }
                UiAction::CreateLinkedBoardForCurrentNote => {
                    self.create_linked_board_for_current_note();
                }
                UiAction::EmbeddedBoard(eb) => {
                    self.handle_embedded_board_action(eb);
                }
                UiAction::CheckForUpdates => {
                    self.update_status_msg = None;
                    self.update_worker.start_check(
                        self.config.include_prerelease_updates,
                        false,
                        self.config.language,
                        ctx.clone(),
                    );
                }
                UiAction::InstallUpdate => {
                    if let Some(upd) = self.available_update.clone() {
                        self.flush_unsaved_documents();
                        self.update_worker.start_install(
                            upd,
                            self.config.language,
                            ctx.clone(),
                        );
                    }
                }
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let editor_focused = ctx.memory(|m| {
            m.has_focus(egui::Id::new("krono_md_editor_main"))
                || m.has_focus(egui::Id::new("krono_md_editor_split"))
        });
        let title_focused = ctx.memory(|m| m.has_focus(egui::Id::new("krono_doc_title_input")));
        let canvas_paste_ready = self.view_mode == ViewMode::Canvas
            && self.active_canvas.path.is_some()
            && self.active_canvas.editing_node_id.is_none()
            && !self.active_canvas.search_open
            && !self.active_canvas.file_picker_open
            && !ctx.wants_keyboard_input();
        let mut paste_image_requested = false;
        let mut actions = Vec::new();
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let ctrl = i.modifiers.ctrl;
            let shift = i.modifiers.shift;
            if cmd || ctrl {
                self.last_cmd_modifier_at = Some(Instant::now());
            }

            let mut saw_text_paste_this_frame = false;
            for ev in &i.events {
                if let egui::Event::Paste(t) = ev {
                    if !t.is_empty() {
                        saw_text_paste_this_frame = true;
                        self.saw_text_paste_at = Some(Instant::now());
                        if canvas_paste_ready {
                            paste_image_requested = true;
                        }
                    }
                }
            }

            // On desktop winit backends, Ctrl+V is intercepted on press for text clipboards:
            // when the clipboard holds an image or file list, egui-winit drops the Press event
            // and only delivers Key::V Release (with `logical_key.or(physical_key)` -> Key::V
            // on both Latin and Cyrillic keyboard layouts).
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed,
                    modifiers,
                    ..
                } = ev
                {
                    let is_v = *key == egui::Key::V || *physical_key == Some(egui::Key::V);
                    if is_v {
                        let cmd_held = modifiers.command || modifiers.ctrl || cmd || ctrl;
                        if *pressed {
                            if cmd_held && !modifiers.shift {
                                self.v_key_pressed_unmodified = false;
                                if !saw_text_paste_this_frame {
                                    paste_image_requested = true;
                                    self.saw_press_paste_at = Some(Instant::now());
                                }
                            } else {
                                self.v_key_pressed_unmodified = true;
                            }
                        } else if self.v_key_pressed_unmodified {
                            self.v_key_pressed_unmodified = false;
                        } else {
                            let recent_cmd = cmd_held
                                || self
                                    .last_cmd_modifier_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(700));
                            let recent_text = saw_text_paste_this_frame
                                || self
                                    .saw_text_paste_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
                            let already_pressed = self
                                .saw_press_paste_at
                                .is_some_and(|t| t.elapsed() < Duration::from_millis(450));
                            if recent_cmd && !modifiers.shift && !recent_text && !already_pressed {
                                paste_image_requested = true;
                            }
                        }
                    }

                    let is_insert =
                        *key == egui::Key::Insert || *physical_key == Some(egui::Key::Insert);
                    if is_insert {
                        if *pressed {
                            if modifiers.shift && !(modifiers.command || modifiers.ctrl) {
                                self.insert_key_pressed_unmodified = false;
                                if !saw_text_paste_this_frame {
                                    paste_image_requested = true;
                                    self.saw_press_paste_at = Some(Instant::now());
                                }
                            } else {
                                self.insert_key_pressed_unmodified = true;
                            }
                        } else if self.insert_key_pressed_unmodified {
                            self.insert_key_pressed_unmodified = false;
                        } else if modifiers.shift {
                            let recent_text = saw_text_paste_this_frame
                                || self
                                    .saw_text_paste_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
                            let already_pressed = self
                                .saw_press_paste_at
                                .is_some_and(|t| t.elapsed() < Duration::from_millis(450));
                            if !recent_text && !already_pressed {
                                paste_image_requested = true;
                            }
                        }
                    }
                }
            }
            if cmd && i.key_pressed(egui::Key::K) {
                if self.palette_open {
                    self.palette_open = false;
                } else {
                    actions.push(UiAction::OpenPalette);
                }
            }
            if cmd && !shift && i.key_pressed(egui::Key::S) {
                match self.view_mode {
                    ViewMode::Collection => actions.push(UiAction::SaveCurrentCollection),
                    ViewMode::Canvas => actions.push(UiAction::SaveCurrentCanvas),
                    _ => actions.push(UiAction::SaveCurrentNote),
                }
            }
            if cmd && !shift && i.key_pressed(egui::Key::G) {
                if self.view_mode == ViewMode::Graph {
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                } else {
                    actions.push(UiAction::SetViewMode(ViewMode::Graph));
                }
            }
            if cmd && shift && i.key_pressed(egui::Key::S) {
                actions.push(UiAction::SyncNow);
            }
            let canvas_duplicating = self.view_mode == ViewMode::Canvas
                && !self.active_canvas.selected_node_ids_vec().is_empty()
                && self.active_canvas.editing_node_id.is_none()
                && !self.active_canvas.file_picker_open;
            if cmd && !shift && !canvas_duplicating && i.key_pressed(egui::Key::D) {
                actions.push(UiAction::OpenTodayNote);
            }
            if i.modifiers.alt && !cmd && !ctrl && !shift && i.key_pressed(egui::Key::ArrowLeft) {
                actions.push(UiAction::NavBack);
            }
            if i.modifiers.alt && !cmd && !ctrl && !shift && i.key_pressed(egui::Key::ArrowRight) {
                actions.push(UiAction::NavForward);
            }
            if cmd && i.key_pressed(egui::Key::Backslash) {
                actions.push(UiAction::ToggleSidebar);
            }
            if cmd && ((shift && i.key_pressed(egui::Key::I)) || (!shift && !editor_focused && i.key_pressed(egui::Key::I))) {
                actions.push(UiAction::ToggleInspector);
            }
            if cmd && i.key_pressed(egui::Key::N) {
                actions.push(UiAction::OpenCreateModal(None));
            }
            if cmd && i.key_pressed(egui::Key::Comma) {
                if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                } else {
                    actions.push(UiAction::OpenSettings(self.settings_tab));
                }
            }
            if !self.palette_open
                && !self.create_modal.open
                && self.rename_target.is_none()
                && self.delete_target.is_none()
                && i.key_pressed(egui::Key::Escape)
            {
                if self.drawer_sidebar_open || self.drawer_inspector_open {
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                } else if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                }
            }
        });
        if !actions.is_empty() {
            self.handle_actions(ctx, actions);
        }
        if paste_image_requested
            && !self.palette_open
            && !self.create_modal.open
            && self.rename_target.is_none()
            && self.delete_target.is_none()
            && !title_focused
            && !self.adding_tag
            && (canvas_paste_ready || (self.view_mode == ViewMode::Editor && self.selected_path.is_some()))
        {
            self.try_paste_image_from_clipboard(ctx, false);
        }
    }
}

impl eframe::App for KronoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_events(ctx);
        self.process_dropped_files(ctx);
        self.handle_shortcuts(ctx);

        #[cfg(debug_assertions)]
        self.drive_screenshots(ctx);

        // Smooth 60fps cross-fade between themes and accent presets
        if let Some(at) = self.palette_anim_at {
            let raw = (at.elapsed().as_secs_f32() / 0.24).clamp(0.0, 1.0);
            let t = ease_in_out_cubic(raw);
            self.palette = ThemePalette::lerp(&self.palette_from, &self.palette_target, t);
            apply_palette(ctx, &self.palette);
            if raw >= 0.999 {
                self.palette = self.palette_target;
                apply_palette(ctx, &self.palette);
                self.palette_anim_at = None;
            } else {
                ctx.request_repaint();
            }
        }

        // Debounced auto-save (~1.4s after last keystroke)
        if self.dirty && self.config.auto_save_enabled {
            if let Some(t) = self.last_edit_at {
                if t.elapsed() >= Duration::from_millis(1400) {
                    self.save_current_note();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(350));
                }
            }
        }
        if self.active_collection.dirty && self.config.auto_save_enabled {
            if let Some(t) = self.active_collection.last_edit_at {
                if t.elapsed() >= Duration::from_millis(1200) {
                    self.save_current_collection();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(350));
                }
            }
        }
        if self.active_canvas.dirty && self.config.auto_save_enabled {
            if let Some(t) = self.active_canvas.last_edit_at {
                if t.elapsed() >= Duration::from_millis(1200) {
                    self.save_current_canvas();
                } else {
                    ctx.request_repaint_after(Duration::from_millis(350));
                }
            }
        }

        // Optional background auto-sync
        #[cfg(debug_assertions)]
        let running_shots = self.shot_runner.is_some();
        #[cfg(not(debug_assertions))]
        let running_shots = false;

        if !running_shots
            && self.config.auto_sync_interval_mins > 0
            && !self.dirty
            && !self.active_collection.dirty
            && !self.active_canvas.dirty
            && !self.sync_worker.is_busy()
            && self.config.is_github_configured()
        {
            let interval_secs = u64::from(self.config.auto_sync_interval_mins) * 60;
            let due = match self.last_sync_ts {
                Some(ts) => {
                    (chrono::Utc::now().timestamp() - ts).max(0) as u64 >= interval_secs
                }
                None => self.last_auto_sync_check.elapsed().as_secs() >= interval_secs,
            };
            if due {
                self.last_auto_sync_check = Instant::now();
                self.trigger_sync(ctx);
            }
        }
        if !running_shots
            && self.config.auto_sync_after_save
            && !self.dirty
            && !self.active_collection.dirty
            && !self.active_canvas.dirty
            && !self.sync_worker.is_busy()
            && self.pending_sync_changes > 0
            && self.config.is_github_configured()
        {
            if let Some(saved_at) = self.last_local_save_at {
                if saved_at.elapsed() >= Duration::from_secs(15) {
                    self.last_local_save_at = None;
                    self.trigger_sync(ctx);
                } else {
                    ctx.request_repaint_after(Duration::from_secs(2));
                }
            }
        }

        if let Some(at) = self.copied_code_at {
            if at.elapsed() >= Duration::from_secs(2) {
                self.copied_code_hash = None;
                self.copied_code_at = None;
            }
        }

        if self.sync_worker.is_busy() {
            let dt = ctx.input(|i| i.stable_dt.min(0.1));
            self.sync_angle = (self.sync_angle + dt * 5.2) % std::f32::consts::TAU;
            ctx.request_repaint();
        } else {
            self.sync_angle = 0.0;
        }

        let layout = LayoutClass::from_width(ctx.screen_rect().width());
        if layout != self.last_layout_class {
            self.drawer_sidebar_open = false;
            self.drawer_inspector_open = false;
            self.last_layout_class = layout;
        }

        let desktop_sidebar_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_sidebar_anim"),
            layout.is_desktop() && self.config.sidebar_open,
            0.18,
        );
        let drawer_sidebar_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_sidebar_drawer_anim"),
            layout == LayoutClass::Compact && self.drawer_sidebar_open,
            0.18,
        );
        let desktop_inspector_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_inspector_anim"),
            layout.is_desktop() && self.view_mode == ViewMode::Editor && self.config.inspector_open,
            0.18,
        );
        let drawer_inspector_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_inspector_drawer_anim"),
            !layout.is_desktop()
                && self.view_mode == ViewMode::Editor
                && self.drawer_inspector_open,
            0.18,
        );

        let palette = self.palette;
        let lang = self.config.language;
        let s = strings(lang);
        let sync_busy = self.sync_worker.is_busy();
        let sync_configured = self.config.is_github_configured();
        let sync_status_text = if sync_busy {
            s.syncing_label.to_string()
        } else if !sync_configured {
            s.sync_not_configured.to_string()
        } else if let Some(ts) = self.last_sync_ts {
            format_relative_time(lang, ts)
        } else {
            s.synced_label.to_string()
        };

        let sync_ui = SyncUiState {
            busy: sync_busy,
            angle: self.sync_angle,
            configured: sync_configured,
            pending_changes: self.pending_sync_changes,
            status_text: &sync_status_text,
        };
        let can_back = self.can_nav_back();
        let can_forward = self.can_nav_forward();
        let recent_items = self.build_recent_tab_items();
        let vault_dir_snap = self.config.vault_dir.clone();

        let mut cal_ui = CalendarUiState {
            open: self.config.calendar_open,
            year: &mut self.calendar_year,
            month: &mut self.calendar_month,
            today: today_local(),
            daily_dates: &self.daily_dates,
        };

        let mut actions = Vec::new();

        // 1. Desktop Left Sidebar (smoothly animated width)
        if layout.is_desktop() && desktop_sidebar_t > 0.02 {
            let mut sb_w = self.config.sidebar_width;
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
            };
            actions.extend(render_sidebar(
                ctx,
                &palette,
                lang,
                &mut sb_w,
                desktop_sidebar_t,
                &self.notes,
                &self.collections,
                &self.tag_counts,
                self.selected_path.as_deref(),
                self.active_collection.path.as_deref(),
                self.active_canvas.path.as_deref(),
                self.active_tag_filter.as_deref(),
                &self.expanded_folders,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
            ));
            if desktop_sidebar_t > 0.99 && (sb_w - self.config.sidebar_width).abs() > 1.0 {
                self.config.sidebar_width = sb_w;
            }
        }

        // 2. Mobile Bottom Tab Bar (declared BEFORE CentralPanel so it reserves bottom inset)
        if layout.is_mobile() {
            actions.extend(render_mobile_bottom_bar(
                ctx,
                &palette,
                lang,
                self.view_mode,
                self.mobile_showing_list,
            ));
        }

        // 3. Top Bar
        let eff_sidebar_open = if layout.is_desktop() {
            self.config.sidebar_open
        } else {
            self.drawer_sidebar_open
        };
        let eff_inspector_open = if layout.is_desktop() {
            self.config.inspector_open
        } else {
            self.drawer_inspector_open
        };
        let topbar_doc = TopbarDocContext {
            selected_note_path: self.selected_path.as_deref(),
            note_title: &self.current_doc.frontmatter.title,
            note_dirty: self.dirty,
            active_collection_path: self.active_collection.path.as_deref(),
            collection_title: &self.active_collection.doc.title,
            collection_dirty: self.active_collection.dirty,
            collection_view: self.active_collection.active_view,
            active_canvas_path: self.active_canvas.path.as_deref(),
            canvas_title: &self.active_canvas.doc.title,
            canvas_dirty: self.active_canvas.dirty,
        };
        actions.extend(render_topbar(
            ctx,
            &palette,
            lang,
            layout,
            self.mobile_showing_list,
            eff_sidebar_open,
            eff_inspector_open,
            self.view_mode,
            &mut self.editor_mode,
            topbar_doc,
            sync_ui,
            can_back,
            can_forward,
        ));

        actions.extend(render_recent_tabs_bar(
            ctx,
            &palette,
            layout,
            &recent_items,
            self.view_mode,
        ));

        let prev_lg_depth = self.local_graph_depth;

        // 4. Desktop Right Inspector (smoothly animated panel, hidden by default)
        if layout.is_desktop() && desktop_inspector_t > 0.02 {
            actions.extend(render_inspector(
                ctx,
                &palette,
                lang,
                desktop_inspector_t,
                self.selected_path.as_deref(),
                &self.current_doc.body,
                &self.cached_headings,
                &self.cached_backlinks,
                &self.cached_outgoing,
                &self.local_graph_data,
                &mut self.local_graph_depth,
            ));
        }

        // 5. Central Panel
        let inner_pad = match layout {
            LayoutClass::Mobile => match self.view_mode {
                ViewMode::Editor if self.mobile_showing_list => Margin::symmetric(12.0, 10.0),
                ViewMode::Editor => Margin::symmetric(14.0, 8.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Table | ViewMode::Board | ViewMode::Collection => {
                    Margin::symmetric(12.0, 10.0)
                }
                ViewMode::Settings => Margin::symmetric(10.0, 10.0),
            },
            LayoutClass::Compact => match self.view_mode {
                ViewMode::Editor => Margin::symmetric(16.0, 12.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Table | ViewMode::Board | ViewMode::Collection => Margin::same(16.0),
                ViewMode::Settings => Margin::symmetric(16.0, 14.0),
            },
            LayoutClass::Desktop => match self.view_mode {
                ViewMode::Editor => Margin::symmetric(20.0, 14.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Table | ViewMode::Board | ViewMode::Collection => Margin::same(22.0),
                ViewMode::Settings => Margin::symmetric(24.0, 20.0),
            },
        };
        let prefs = EditorPrefs {
            font_size: self.config.editor_font_size,
            heading_serif: self.config.heading_serif,
            max_width: self.config.editor_width.max_px(),
            show_footer: self.config.show_editor_footer,
            md_slash_menu: self.config.md_slash_menu,
            md_wikilink_autocomplete: self.config.md_wikilink_autocomplete,
            md_auto_lists: self.config.md_auto_lists,
            md_auto_pairs: self.config.md_auto_pairs,
            md_show_toolbar: self.config.md_show_toolbar,
            md_slash_trigger: self.config.md_slash_trigger,
        };
        let last_sync_info = self.last_sync_ts.map(|ts| {
            let rel = format_relative_time(lang, ts);
            match &self.last_sync_commit {
                Some(sha) if sha.len() >= 7 => format!("{rel} ({})", &sha[..7]),
                _ => rel,
            }
        });

        let view_t = anim_since(ctx, self.view_anim_at, 0.15);
        let note_t = anim_since(ctx, self.note_anim_at, 0.15);
        let tab_anim_t = anim_since(ctx, self.settings_tab_anim_at, 0.15);

        let today = today_local();
        let available_boards: Vec<(String, String)> = self
            .collections
            .iter()
            .filter(|c| c.kind == VaultFileKind::Board)
            .map(|c| (c.path.clone(), c.title.clone()))
            .collect();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette.bg_canvas)
                    .inner_margin(inner_pad),
            )
            .show(ctx, |ui| match self.view_mode {
                ViewMode::Editor => {
                    if layout.is_mobile() && self.mobile_showing_list {
                        let mut pages_ui = PagesTreeUiState {
                            filter: &mut self.pages_filter,
                            sort: &mut self.pages_sort,
                            filter_open: &mut self.pages_filter_open,
                        };
                        actions.extend(render_mobile_notes_list(
                            ctx,
                            ui,
                            &palette,
                            lang,
                            &self.notes,
                            &self.collections,
                            &self.tag_counts,
                            self.selected_path.as_deref(),
                            self.active_collection.path.as_deref(),
                            self.active_canvas.path.as_deref(),
                            self.active_tag_filter.as_deref(),
                            &self.expanded_folders,
                            &mut pages_ui,
                            &mut cal_ui,
                        ));
                    } else {
                        let doc_state = EditorDocState {
                            selected_path: self.selected_path.as_deref(),
                            title: &mut self.current_doc.frontmatter.title,
                            status: &mut self.current_doc.frontmatter.status,
                            tags: &mut self.current_doc.frontmatter.tags,
                            pinned: &mut self.current_doc.frontmatter.pinned,
                            body: &mut self.current_doc.body,
                            modified: self.current_modified,
                            external_conflict: self.external_conflict_open_note,
                        };
                        actions.extend(render_editor_view(
                            ui,
                            &palette,
                            lang,
                            layout,
                            doc_state,
                            &mut self.new_tag_input,
                            &mut self.adding_tag,
                            self.editor_mode,
                            prefs,
                            &self.config.custom_slash_snippets,
                            &self.notes,
                            &mut self.focus_title_next,
                            &mut self.focus_body_next,
                            note_t.min(view_t),
                            &self.resolved_titles,
                            self.copied_code_hash,
                            &self.config.vault_dir,
                            &mut self.image_cache,
                            &self.embedded_boards,
                            today,
                            &available_boards,
                        ));
                    }
                }
                ViewMode::Table => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(render_table_view(
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.notes,
                        &mut self.table_filter_query,
                        &mut self.table_status_filter,
                        self.active_tag_filter.as_deref(),
                        &mut self.table_sort,
                    ));
                }
                ViewMode::Board => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(render_board_view(
                        ui,
                        &palette,
                        lang,
                        layout,
                        &mut self.mobile_board_tab,
                        &self.notes,
                        self.active_tag_filter.as_deref(),
                        &mut self.quick_add_board_status,
                        &mut self.quick_add_board_title,
                    ));
                }
                ViewMode::Graph => {
                    actions.extend(render_graph_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.notes,
                        &self.collections,
                        &self.all_links,
                        &mut self.graph_state,
                    ));
                }
                ViewMode::Overview => {
                    actions.extend(render_vault_map_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.config.vault_dir,
                        &mut self.vault_map,
                        &self.notes,
                        &self.collections,
                        &self.all_links,
                    ));
                }
                ViewMode::Collection => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(render_collection_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        today,
                        &mut self.active_collection,
                        &self.notes,
                    ));
                }
                ViewMode::Canvas => {
                    actions.extend(render_canvas_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &mut self.active_canvas,
                        &self.notes,
                        &self.collections,
                        &self.config.vault_dir,
                        &mut self.image_cache,
                    ));
                }
                ViewMode::Settings => {
                    let prev_tab = self.settings_tab;
                    actions.extend(render_settings_view(
                        ui,
                        &palette,
                        &mut self.config,
                        &mut self.settings_tab,
                        tab_anim_t,
                        &mut self.vault_draft,
                        &mut self.show_token,
                        &self.test_feedback,
                        last_sync_info.as_deref(),
                        sync_busy,
                        self.notes.len(),
                        self.tag_counts.len(),
                        self.cached_link_count,
                        &mut self.snippet_draft_trigger,
                        &mut self.snippet_draft_label,
                        &mut self.snippet_draft_content,
                        &self.asset_audit,
                        &mut self.asset_audit_only_unused,
                        &vault_dir_snap,
                        &mut self.image_cache,
                        UpdateUiState {
                            checking: self.update_worker.is_checking(),
                            installing: self.update_worker.is_installing(),
                            available: self.available_update.as_ref(),
                            status_msg: self.update_status_msg.as_deref(),
                        },
                    ));
                    if self.settings_tab != prev_tab {
                        self.settings_tab_anim_at = Some(Instant::now());
                        if self.settings_tab == SettingsTab::Storage {
                            actions.push(UiAction::RefreshAssetAudit);
                        }
                    }
                }
            });

        // 6. Overlay Drawers for Compact & Mobile viewports (AFTER CentralPanel)
        if layout == LayoutClass::Compact && drawer_sidebar_t > 0.02 {
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
            };
            let (d_actions, dismissed) = render_sidebar_drawer(
                ctx,
                &palette,
                lang,
                drawer_sidebar_t,
                &self.notes,
                &self.collections,
                &self.tag_counts,
                self.selected_path.as_deref(),
                self.active_collection.path.as_deref(),
                self.active_canvas.path.as_deref(),
                self.active_tag_filter.as_deref(),
                &self.expanded_folders,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
            );
            actions.extend(d_actions);
            if dismissed {
                self.drawer_sidebar_open = false;
            }
        }

        if !layout.is_desktop()
            && self.view_mode == ViewMode::Editor
            && drawer_inspector_t > 0.02
        {
            let (i_actions, dismissed) = render_inspector_drawer(
                ctx,
                &palette,
                lang,
                drawer_inspector_t,
                self.selected_path.as_deref(),
                &self.current_doc.body,
                &self.cached_headings,
                &self.cached_backlinks,
                &self.cached_outgoing,
                &self.local_graph_data,
                &mut self.local_graph_depth,
            );
            actions.extend(i_actions);
            if dismissed {
                self.drawer_inspector_open = false;
            }
        }

        if self.local_graph_depth != prev_lg_depth {
            self.refresh_document_derivations();
        }

        // 5. Command Palette Modal
        let (q_changed, pal_actions) = render_command_palette(
            ctx,
            &palette,
            lang,
            &mut self.palette_open,
            &mut self.palette_query,
            &mut self.palette_selected,
            &self.palette_hits,
        );
        if q_changed {
            self.refresh_palette_hits();
        }
        actions.extend(pal_actions);

        // 6. Action Dialogs (Create Modal, Rename & Delete)
        let folders_for_modal = if self.create_modal.open {
            self.existing_folders_list()
        } else {
            Vec::new()
        };
        if let Some(act) = render_create_item_modal(
            ctx,
            &palette,
            lang,
            &mut self.create_modal,
            &folders_for_modal,
        ) {
            actions.push(act);
        }

        if let Some((old_path, new_title)) = render_rename_modal(
            ctx,
            &palette,
            lang,
            &mut self.rename_target,
            &mut self.rename_draft,
        ) {
            self.perform_rename(&old_path, &new_title);
        }

        if let Some(del_path) =
            render_delete_modal(ctx, &palette, lang, &mut self.delete_target)
        {
            self.perform_delete(&del_path);
        }

        // 6b. Fullscreen Image Lightbox
        render_image_lightbox(
            ctx,
            &palette,
            lang,
            &self.config.vault_dir,
            &mut self.image_cache,
            &mut self.lightbox_image,
        );

        // 7. Animated Toasts
        render_toasts(ctx, &palette, &mut self.toasts);

        if !actions.is_empty() {
            self.handle_actions(ctx, actions);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_unsaved_documents();
        #[cfg(debug_assertions)]
        if self.shot_runner.is_some() {
            return;
        }
        let _ = self.config.save();
    }
}
