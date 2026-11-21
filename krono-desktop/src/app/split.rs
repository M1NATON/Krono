use super::KronoApp;
use crate::i18n::strings;
use crate::storage::vault::{read_collection, read_note, save_collection, save_note};
use crate::ui::{
    collection_view::CollectionEditorState,
    components::ToastKind,
    EditorMode, NavTarget, UiAction, ViewMode,
};
use eframe::egui;
use krono_core::{
    note::NoteDocument,
    path::VaultFileKind,
    tasks::toggle_task_in_markdown,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitPane {
    Left,
    Right,
}

pub struct SplitNoteState {
    pub path: String,
    pub doc: NoteDocument,
    pub modified: i64,
    pub dirty: bool,
    pub last_edit_at: Option<Instant>,
    pub editor_mode: EditorMode,
    pub new_tag_input: String,
    pub adding_tag: bool,
    pub focus_title_next: bool,
    pub focus_body_next: bool,
}

pub enum SplitContent {
    Note(SplitNoteState),
    Collection(CollectionEditorState),
}

impl SplitContent {
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Note(n) => Some(n.path.as_str()),
            Self::Collection(c) => c.path.as_deref(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            Self::Note(n) => n.dirty,
            Self::Collection(c) => c.dirty,
        }
    }

    pub fn last_edit_at(&self) -> Option<Instant> {
        match self {
            Self::Note(n) => n.last_edit_at,
            Self::Collection(c) => c.last_edit_at,
        }
    }
}

pub struct SplitViewState {
    pub right: Option<SplitContent>,
    pub ratio: f32,
    pub active_pane: SplitPane,
}

impl Default for SplitViewState {
    fn default() -> Self {
        Self {
            right: None,
            ratio: 0.50,
            active_pane: SplitPane::Left,
        }
    }
}

impl KronoApp {
    pub(super) fn primary_active_file_path(&self) -> Option<&str> {
        match self.view_mode {
            ViewMode::Editor => self.selected_path.as_deref(),
            ViewMode::Collection => self.active_collection.path.as_deref(),
            _ => self.selected_path.as_deref(),
        }
    }

    pub(super) fn open_in_split(&mut self, target: NavTarget) {
        let s = strings(self.config.language);
        match target {
            NavTarget::Note(path) => {
                if self.primary_active_file_path() == Some(path.as_str()) {
                    self.push_toast(s.toast_split_same_file, ToastKind::Info);
                    return;
                }
                self.save_split_pane();
                match read_note(&self.config.vault_dir, &path) {
                    Ok((doc, modified, _)) => {
                        self.split.right = Some(SplitContent::Note(SplitNoteState {
                            path,
                            doc,
                            modified,
                            dirty: false,
                            last_edit_at: None,
                            editor_mode: EditorMode::Edit,
                            new_tag_input: String::new(),
                            adding_tag: false,
                            focus_title_next: false,
                            focus_body_next: false,
                        }));
                        self.split.active_pane = SplitPane::Right;
                        if !matches!(self.view_mode, ViewMode::Editor | ViewMode::Collection) {
                            self.view_mode = ViewMode::Editor;
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            NavTarget::Collection(path) => {
                if self.primary_active_file_path() == Some(path.as_str()) {
                    self.push_toast(s.toast_split_same_file, ToastKind::Info);
                    return;
                }
                self.save_split_pane();
                match read_collection(&self.config.vault_dir, &path) {
                    Ok((doc, modified, _, _)) => {
                        let mut st = CollectionEditorState::default();
                        st.path = Some(path);
                        st.active_view = doc.default_view;
                        st.doc = doc;
                        st.modified = modified;
                        self.split.right = Some(SplitContent::Collection(st));
                        self.split.active_pane = SplitPane::Right;
                        if !matches!(self.view_mode, ViewMode::Editor | ViewMode::Collection) {
                            self.view_mode = ViewMode::Editor;
                        }
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            _ => {}
        }
    }

    pub(super) fn toggle_split_right(&mut self) {
        if self.split.right.is_some() {
            self.close_split();
            return;
        }
        let primary = self.primary_active_file_path().map(str::to_string);
        let candidate = self
            .recent_tabs
            .iter()
            .rev()
            .find(|t| match t {
                NavTarget::Note(p) | NavTarget::Collection(p) => {
                    primary.as_deref() != Some(p.as_str())
                }
                _ => false,
            })
            .cloned()
            .or_else(|| {
                self.notes
                    .iter()
                    .find(|n| primary.as_deref() != Some(n.path.as_str()))
                    .map(|n| NavTarget::Note(n.path.clone()))
            })
            .or_else(|| {
                self.collections
                    .iter()
                    .find(|c| {
                        c.kind != VaultFileKind::Canvas
                            && primary.as_deref() != Some(c.path.as_str())
                    })
                    .map(|c| NavTarget::Collection(c.path.clone()))
            });

        if let Some(target) = candidate {
            self.open_in_split(target);
        } else {
            let s = strings(self.config.language);
            self.push_toast(s.toast_split_no_other, ToastKind::Info);
        }
    }

    pub(super) fn close_split(&mut self) {
        self.save_split_pane();
        self.split.right = None;
        self.split.active_pane = SplitPane::Left;
    }

    pub(super) fn save_split_pane(&mut self) {
        let Some(ref mut content) = self.split.right else {
            return;
        };
        match content {
            SplitContent::Note(n) => {
                if !n.dirty {
                    return;
                }
                self.watcher.mark_internal_change();
                match save_note(&self.config.vault_dir, &n.path, &n.doc) {
                    Ok((modified, size)) => {
                        n.modified = modified;
                        n.dirty = false;
                        n.last_edit_at = None;
                        let _ = self.index.upsert_note(&n.path, &n.doc, modified, size);
                        self.reload_index_caches();
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            SplitContent::Collection(c) => {
                if !c.dirty {
                    return;
                }
                let Some(ref path) = c.path else {
                    return;
                };
                self.watcher.mark_internal_change();
                match save_collection(&self.config.vault_dir, path, &c.doc) {
                    Ok((modified, size)) => {
                        c.modified = modified;
                        c.dirty = false;
                        c.last_edit_at = None;
                        let kind = VaultFileKind::from_path(path).unwrap_or(VaultFileKind::Board);
                        let _ = self
                            .index
                            .upsert_collection(path, kind, &c.doc, modified, size);
                        self.reload_index_caches();
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
        }
    }

    pub(super) fn handle_right_pane_actions(&mut self, ctx: &egui::Context, actions: Vec<UiAction>) {
        let mut forward = Vec::new();
        for act in actions {
            match act {
                UiAction::MarkDirty => {
                    if let Some(SplitContent::Note(ref mut n)) = self.split.right {
                        n.dirty = true;
                        n.last_edit_at = Some(Instant::now());
                    }
                }
                UiAction::SaveCurrentNote | UiAction::SaveCurrentCollection => {
                    self.save_split_pane();
                }
                UiAction::CommitCurrentNoteTitle => {
                    self.save_split_pane();
                }
                UiAction::SetEditorMode(mode) => {
                    if let Some(SplitContent::Note(ref mut n)) = self.split.right {
                        n.editor_mode = mode;
                    }
                }
                UiAction::ToggleTask(task_idx) => {
                    if let Some(SplitContent::Note(ref mut n)) = self.split.right {
                        if let Some(updated) =
                            toggle_task_in_markdown(&n.doc.body, task_idx, super::today_local())
                        {
                            n.doc.body = updated;
                            n.dirty = true;
                            n.last_edit_at = Some(Instant::now());
                        }
                    }
                }
                UiAction::MarkCollectionDirty => {
                    if let Some(SplitContent::Collection(ref mut c)) = self.split.right {
                        c.dirty = true;
                        c.last_edit_at = Some(Instant::now());
                    }
                }
                UiAction::SetCollectionView(v) => {
                    if let Some(SplitContent::Collection(ref mut c)) = self.split.right {
                        c.active_view = v;
                    }
                }
                UiAction::OpenNote(path) => {
                    match VaultFileKind::from_path(&path) {
                        Some(VaultFileKind::Board | VaultFileKind::Table) => {
                            self.open_in_split(NavTarget::Collection(path));
                        }
                        _ => {
                            self.open_in_split(NavTarget::Note(path));
                        }
                    }
                }
                UiAction::OpenCollection(path) => {
                    self.open_in_split(NavTarget::Collection(path));
                }
                UiAction::OpenOrCreateWikilink(target) => {
                    if let Some(path) = self.find_note_by_wikilink(&target) {
                        self.open_in_split(NavTarget::Note(path));
                    } else {
                        forward.push(UiAction::OpenOrCreateWikilink(target));
                    }
                }
                UiAction::CloseSplit => {
                    self.close_split();
                }
                other => forward.push(other),
            }
        }
        if !forward.is_empty() {
            self.handle_actions(ctx, forward);
        }
    }
}
