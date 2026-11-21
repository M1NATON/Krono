mod actions;
mod canvas;
mod capture;
mod collections;
mod frame;
mod notes;
mod shortcuts;
mod sync;
mod tasks;
mod vault_ops;

use crate::config::AppConfig;
use crate::i18n::strings;
use crate::notifications::TaskNotificationTracker;
#[cfg(debug_assertions)]
use crate::shot_runner::ShotRunner;
use crate::storage::{
    index::{BacklinkHit, CollectionMeta, RawLinkEdge, SearchHit, VaultIndex, VaultTaskRow},
    vault::{
        ensure_starter_vault, list_vault_folders, migrate_diary_hierarchy, AssetAuditEntry,
    },
    watcher::VaultWatcher,
};
use crate::sync::{
    secrets::{has_github_token, load_github_token, migrate_legacy_token},
    updater::{AvailableUpdate, UpdateWorker},
    worker::SyncWorker,
};
use crate::tray::SystemTrayHandle;
use crate::ui::{
    canvas_view::CanvasEditorState,
    collection_view::CollectionEditorState,
    components::{render_toasts, Toast, ToastKind},
    dialogs::{render_first_run_screen, CreateModalState, FirstRunAction},
    graph_view::{GraphState, LocalGraphData},
    image_cache::ImageTextureCache,
    inspector::OutgoingLinkInfo,
    motion::ease_in_out_cubic,
    scratchpad::ScratchpadState,
    theme::{apply_palette, setup_fonts, ThemePalette},
    tasks_view::TasksViewState,
    vault_map_view::VaultMapState,
    EditorMode, LayoutClass, NavTarget, PagesKindFilter, PagesSortOrder, SettingsTab,
    BoardSort, TableColumnWidths, TableSort, VaultCounts, ViewMode,
};
use chrono::Datelike;
use eframe::egui;
use krono_core::{
    collection::CollectionDoc,
    daily::{parse_daily_note_path, SimpleDate},
    markdown::Heading,
    note::{NoteDocument, NoteMeta, NoteStatus},
    path::VaultFileKind,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

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
    pub(crate) vault_tasks: Vec<VaultTaskRow>,
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
    table_col_widths: TableColumnWidths,
    board_sort: BoardSort,
    pub(crate) tasks_view_state: TasksViewState,
    task_notifier: TaskNotificationTracker,
    quick_add_board_status: Option<NoteStatus>,
    quick_add_board_title: String,
    quick_add_board_tags: Vec<String>,
    quick_add_board_tag_input: String,

    // Command palette
    pub(crate) palette_open: bool,
    palette_query: String,
    palette_selected: usize,
    palette_hits: Vec<SearchHit>,

    // Settings & Dialogs & Toasts
    pub(crate) settings_tab: SettingsTab,
    vault_draft: String,
    pub(crate) has_saved_github_token: bool,
    token_replace_mode: bool,
    token_draft: Zeroizing<String>,
    test_feedback: Option<(bool, String)>,
    pub(crate) rename_target: Option<String>,
    rename_draft: String,
    pub(crate) delete_target: Option<String>,
    first_run_open: bool,
    vault_folders: Vec<String>,
    create_subfolder_parent: Option<String>,
    create_subfolder_draft: String,
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

    // Quick capture (Scratchpad + Tray)
    pub(crate) scratchpad: ScratchpadState,
    tray: SystemTrayHandle,

    #[cfg(debug_assertions)]
    pub(crate) shot_runner: Option<ShotRunner>,
}

impl KronoApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (mut config, config_existed) = AppConfig::load_with_existed();
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

        let first_run_open = !config_existed && !is_shot_mode;

        let mut startup_migration_err: Option<String> = None;
        if !is_shot_mode && !config.legacy_github_token.trim().is_empty() {
            match migrate_legacy_token(&mut config.legacy_github_token) {
                Ok(true) => {
                    if config_existed {
                        let _ = config.save();
                    }
                }
                Ok(false) => {}
                Err(e) => {
                    startup_migration_err = Some(e);
                }
            }
        }
        let has_saved_github_token = if is_shot_mode {
            false
        } else {
            has_github_token()
        };

        let (index, index_recovered) = if first_run_open {
            match VaultIndex::open_in_memory_checked() {
                Ok(idx) => (idx, false),
                Err(_) => VaultIndex::open_or_recover(&config.vault_dir),
            }
        } else {
            let _ = ensure_starter_vault(&config.vault_dir, config.language);
            if !is_shot_mode {
                let _ = migrate_diary_hierarchy(&config.vault_dir, config.language);
            }
            let (mut idx, rec) = VaultIndex::open_or_recover(&config.vault_dir);
            let _ = idx.sync_from_disk(&config.vault_dir);
            (idx, rec)
        };
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
        let tray = if is_shot_mode {
            SystemTrayHandle::start(cc.egui_ctx.clone(), config.language) // still safe no-op if unavailable
        } else {
            SystemTrayHandle::start(cc.egui_ctx.clone(), config.language)
        };

        if config.auto_sync_on_start && config.is_github_configured(has_saved_github_token) {
            if let Ok(Some(token)) = load_github_token() {
                if let Ok(base) = index.load_sync_state() {
                    sync_worker.start_sync(
                        config.vault_dir.clone(),
                        base,
                        config.github_repo.clone(),
                        config.github_branch.clone(),
                        token,
                        config.language,
                        cc.egui_ctx.clone(),
                    );
                }
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
            vault_tasks: Vec::new(),
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
            table_col_widths: TableColumnWidths::default(),
            board_sort: BoardSort::Manual,
            tasks_view_state: TasksViewState::default(),
            task_notifier: TaskNotificationTracker::default(),
            quick_add_board_status: None,
            quick_add_board_title: String::new(),
            quick_add_board_tags: Vec::new(),
            quick_add_board_tag_input: String::new(),
            palette_open: false,
            palette_query: String::new(),
            palette_selected: 0,
            palette_hits: Vec::new(),
            settings_tab: SettingsTab::Appearance,
            vault_draft,
            has_saved_github_token,
            token_replace_mode: false,
            token_draft: Zeroizing::new(String::new()),
            test_feedback: None,
            rename_target: None,
            rename_draft: String::new(),
            delete_target: None,
            first_run_open,
            vault_folders: Vec::new(),
            create_subfolder_parent: None,
            create_subfolder_draft: String::new(),
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
            scratchpad: ScratchpadState::default(),
            tray,
            #[cfg(debug_assertions)]
            shot_runner,
        };

        app.reload_index_caches();
        if let Some(err) = startup_migration_err {
            app.push_toast(err, ToastKind::Error);
        }
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
        self.vault_tasks = self.index.list_tasks().unwrap_or_default();
        self.vault_folders = if self.first_run_open {
            Vec::new()
        } else {
            list_vault_folders(&self.config.vault_dir)
        };
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
        self.pending_sync_changes = if self.config.is_github_configured(self.has_saved_github_token) {
            self.index.count_unsynced_changes()
        } else {
            0
        };
        self.refresh_palette_hits();
    }

    fn vault_counts(&self) -> VaultCounts {
        let mut counts = VaultCounts {
            notes: self.notes.len(),
            tags: self.tag_counts.len(),
            links: self.cached_link_count,
            ..Default::default()
        };
        for c in &self.collections {
            match c.kind {
                VaultFileKind::Board => counts.boards += 1,
                VaultFileKind::Table => counts.tables += 1,
                VaultFileKind::Canvas => counts.canvases += 1,
                _ => {}
            }
        }
        counts
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


    fn refresh_palette_hits(&mut self) {
        self.palette_hits = self.index.search(&self.palette_query).unwrap_or_default();
    }

}

impl eframe::App for KronoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_events(ctx);
        self.poll_tray_commands(ctx);
        self.process_dropped_files(ctx);
        self.handle_shortcuts(ctx);

        if ctx.input(|i| {
            ((i.modifiers.ctrl || i.modifiers.command)
                && i.modifiers.shift
                && i.key_pressed(egui::Key::Space))
                || (i.modifiers.alt && !i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::N))
        }) {
            self.open_scratchpad(ctx);
        }

        #[cfg(debug_assertions)]
        self.drive_screenshots(ctx);

        if self.first_run_open {
            let palette = self.palette;
            let lang = self.config.language;
            let mut first_run_action = None;
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(palette.bg_canvas))
                .show(ctx, |ui| {
                    first_run_action =
                        render_first_run_screen(ui, &palette, lang, &mut self.vault_draft);
                });
            render_toasts(ctx, &palette, &mut self.toasts);
            if let Some(act) = first_run_action {
                let target = self.vault_draft.trim().to_string();
                if !target.is_empty() {
                    let target_path = std::path::PathBuf::from(&target);
                    let _ = ensure_starter_vault(&target_path, self.config.language);
                    self.first_run_open = false;
                    self.switch_vault(&target);
                    let _ = self.config.save();
                    if act == FirstRunAction::StartAndConfigureSync {
                        self.view_mode = ViewMode::Settings;
                        self.settings_tab = SettingsTab::GitHub;
                    }
                }
            }
            return;
        }

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
        self.tick_task_notifications();
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
            && self.config.is_github_configured(self.has_saved_github_token)
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
            && self.config.is_github_configured(self.has_saved_github_token)
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


        self.render_workspace_frame(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush_unsaved_documents();
        #[cfg(debug_assertions)]
        if self.shot_runner.is_some() {
            return;
        }
        if !self.first_run_open {
            let _ = self.config.save();
        }
    }
}
