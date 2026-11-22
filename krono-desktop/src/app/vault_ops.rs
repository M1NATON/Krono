use super::{
    split::{SplitContent, SplitPane},
    KronoApp,
};
use crate::i18n::{strings, Lang};
use crate::storage::{
    index::VaultIndex,
    vault::{
        audit_vault_assets, delete_note, delete_vault_folder, duplicate_canvas,
        duplicate_collection, duplicate_note, ensure_starter_vault, migrate_diary_hierarchy,
        move_vault_file_to_folder, read_canvas, read_collection, read_note, rename_canvas,
        rename_collection, rename_note, rewrite_vault_canvas_links,
        rewrite_vault_collection_links, rewrite_vault_wikilinks, save_canvas, save_collection,
        save_note,
    },
};
use crate::ui::{
    components::ToastKind, empty_state::EmptyRecentEntry, icons::Icon, NavTarget, RecentTabItem,
    ViewMode,
};
use krono_core::{
    note::NoteDocument,
    path::{vault_file_stem, VaultFileKind},
};
use std::{path::PathBuf, time::Instant};

pub(super) fn nav_target_key(t: &NavTarget) -> Option<String> {
    match t {
        NavTarget::Note(p) => Some(format!("note:{p}")),
        NavTarget::Collection(p) => Some(format!("col:{p}")),
        NavTarget::Canvas(p) => Some(format!("canvas:{p}")),
        NavTarget::View(_) => None,
    }
}

pub(super) fn nav_target_from_key(k: &str) -> Option<NavTarget> {
    if let Some(p) = k.strip_prefix("note:") {
        Some(NavTarget::Note(p.to_string()))
    } else if let Some(p) = k.strip_prefix("col:") {
        Some(NavTarget::Collection(p.to_string()))
    } else if let Some(p) = k.strip_prefix("canvas:") {
        Some(NavTarget::Canvas(p.to_string()))
    } else {
        None
    }
}

impl KronoApp {
    pub(super) fn is_tab_pinned(&self, t: &NavTarget) -> bool {
        nav_target_key(t).is_some_and(|k| self.config.pinned_tabs.contains(&k))
    }

    pub(super) fn sync_pinned_tabs_into_recent(&mut self) {
        let mut pinned_targets = Vec::new();
        for key in &self.config.pinned_tabs {
            if let Some(t) = nav_target_from_key(key) {
                pinned_targets.push(t);
            }
        }
        let unpinned: Vec<NavTarget> = self
            .recent_tabs
            .iter()
            .filter(|t| !pinned_targets.contains(t))
            .cloned()
            .collect();
        self.recent_tabs = pinned_targets;
        self.recent_tabs.extend(unpinned);
    }

    pub(super) fn toggle_pin_tab(&mut self, target: &NavTarget) {
        let Some(key) = nav_target_key(target) else {
            return;
        };
        if let Some(pos) = self.config.pinned_tabs.iter().position(|k| k == &key) {
            self.config.pinned_tabs.remove(pos);
        } else {
            self.config.pinned_tabs.push(key);
            if !self.recent_tabs.contains(target) {
                self.recent_tabs.push(target.clone());
            }
        }
        self.sync_pinned_tabs_into_recent();
        let _ = self.config.save();
    }

    pub(super) fn reorder_recent_tab(&mut self, target: &NavTarget, to_index: usize) {
        let Some(from_idx) = self.recent_tabs.iter().position(|t| t == target) else {
            return;
        };
        let is_pinned = self.is_tab_pinned(target);
        let pinned_count = self
            .recent_tabs
            .iter()
            .filter(|t| self.is_tab_pinned(t))
            .count();
        let (min_idx, max_idx) = if is_pinned {
            (0usize, pinned_count.saturating_sub(1))
        } else {
            (pinned_count, self.recent_tabs.len().saturating_sub(1))
        };
        if min_idx > max_idx {
            return;
        }
        let target_idx = to_index.clamp(min_idx, max_idx);
        if target_idx == from_idx {
            return;
        }
        let item = self.recent_tabs.remove(from_idx);
        self.recent_tabs.insert(target_idx, item);
        if is_pinned {
            self.config.pinned_tabs = self
                .recent_tabs
                .iter()
                .take(pinned_count)
                .filter_map(nav_target_key)
                .collect();
            let _ = self.config.save();
        }
    }

    pub(super) fn reorder_folder_in_config(
        &mut self,
        parent_prefix: &str,
        folder_path: &str,
        to_index: usize,
    ) {
        let mut siblings: Vec<String> = Vec::new();
        for f in &self.vault_folders {
            let parent = f.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
            if parent == parent_prefix && !f.starts_with("daily/") {
                siblings.push(f.clone());
            }
        }
        if !siblings.contains(&folder_path.to_string()) {
            return;
        }
        siblings.sort_by(|a, b| {
            let ap = self.config.pinned_folders.contains(a);
            let bp = self.config.pinned_folders.contains(b);
            let ao = self
                .config
                .folder_order
                .iter()
                .position(|x| x == a)
                .unwrap_or(usize::MAX);
            let bo = self
                .config
                .folder_order
                .iter()
                .position(|x| x == b)
                .unwrap_or(usize::MAX);
            bp.cmp(&ap)
                .then_with(|| ao.cmp(&bo))
                .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
        });
        let Some(from_idx) = siblings.iter().position(|s| s == folder_path) else {
            return;
        };
        let is_pinned = self.config.pinned_folders.contains(&folder_path.to_string());
        let pinned_cnt = siblings
            .iter()
            .filter(|s| self.config.pinned_folders.contains(s))
            .count();
        let (min_i, max_i) = if is_pinned {
            (0usize, pinned_cnt.saturating_sub(1))
        } else {
            (pinned_cnt, siblings.len().saturating_sub(1))
        };
        let dest = to_index.clamp(min_i, max_i);
        if dest != from_idx {
            let item = siblings.remove(from_idx);
            siblings.insert(dest, item);
        }
        self.config
            .folder_order
            .retain(|x| !siblings.iter().any(|s| s == x));
        self.config.folder_order.extend(siblings);
        let _ = self.config.save();
    }

    pub(super) fn persist_open_tabs_state(&mut self) {
        self.config.open_tab_keys = self
            .recent_tabs
            .iter()
            .filter_map(nav_target_key)
            .collect();
        self.config.last_active_tab_key = self
            .primary_active_file_path()
            .and_then(|p| match VaultFileKind::from_path(p) {
                Some(VaultFileKind::Canvas) => Some(format!("canvas:{p}")),
                Some(VaultFileKind::Board | VaultFileKind::Table) => Some(format!("col:{p}")),
                Some(VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown) => {
                    Some(format!("note:{p}"))
                }
                _ => None,
            });
        let _ = self.config.save();
    }

    pub(super) fn enter_empty_state(&mut self) {
        self.selected_path = None;
        self.dirty = false;
        self.external_conflict_open_note = false;
        self.current_doc = NoteDocument::parse("", "");
        self.active_collection.path = None;
        self.active_collection.dirty = false;
        self.active_collection.reset_transient();
        self.active_canvas.path = None;
        self.active_canvas.dirty = false;
        self.active_canvas.reset_transient();
        self.split.right = None;
        self.split.active_pane = SplitPane::Left;
        self.view_mode = ViewMode::Editor;
        self.refresh_document_derivations();
        self.refresh_embedded_boards();
    }

    pub(super) fn close_tab_and_update_view(&mut self, target: &NavTarget) {
        self.recent_tabs.retain(|t| t != target);

        if let Some(split_right) = &self.split.right {
            let matches_split = match (target, split_right) {
                (NavTarget::Note(p), SplitContent::Note(st)) => st.path == *p,
                (NavTarget::Collection(p), SplitContent::Collection(st)) => {
                    st.path.as_deref() == Some(p.as_str())
                }
                _ => false,
            };
            if matches_split {
                self.split.right = None;
                self.split.active_pane = SplitPane::Left;
            }
        }

        let is_active = match target {
            NavTarget::Note(p) => {
                self.view_mode == ViewMode::Editor && self.selected_path.as_deref() == Some(p.as_str())
            }
            NavTarget::Collection(p) => {
                self.view_mode == ViewMode::Collection
                    && self.active_collection.path.as_deref() == Some(p.as_str())
            }
            NavTarget::Canvas(p) => {
                self.view_mode == ViewMode::Canvas
                    && self.active_canvas.path.as_deref() == Some(p.as_str())
            }
            NavTarget::View(_) => false,
        };

        if self.recent_tabs.is_empty() {
            self.enter_empty_state();
        } else if is_active {
            if let Some(next) = self.recent_tabs.last().cloned() {
                self.navigate_to_target(next, false);
            }
        }
        self.persist_open_tabs_state();
    }

    pub(super) fn sync_and_save_vault_ui_state(&mut self) {
        let mut next = self.vault_ui_state.clone();
        next.sidebar = crate::config::SidebarFilterState {
            pages_filter: self.pages_filter,
            pages_sort: self.pages_sort,
            tree_tab: self.sidebar_tree_tab,
            active_tags: self.active_tag_filters.clone(),
            tag_mode: self.tag_filter_mode,
        };
        next.table = crate::config::TableFilterState {
            filter_query: self.table_filter_query.clone(),
            status_filter: self
                .table_status_filter
                .map(|st| st.as_yaml_str().to_string()),
            sort: self.table_sort,
        };
        next.board_sort = self.board_sort;
        if let Some(ref path) = self.active_collection.path {
            next.collections
                .insert(path.clone(), self.active_collection.to_filter_state());
        }
        if next != self.vault_ui_state {
            self.vault_ui_state = next;
            let _ = self.vault_ui_state.save(&self.config.vault_dir);
        }
    }

    fn active_tab_index(&self) -> Option<usize> {
        self.recent_tabs.iter().position(|t| match t {
            NavTarget::Note(p) => {
                self.view_mode == ViewMode::Editor && self.selected_path.as_deref() == Some(p.as_str())
            }
            NavTarget::Collection(p) => {
                self.view_mode == ViewMode::Collection
                    && self.active_collection.path.as_deref() == Some(p.as_str())
            }
            NavTarget::Canvas(p) => {
                self.view_mode == ViewMode::Canvas
                    && self.active_canvas.path.as_deref() == Some(p.as_str())
            }
            NavTarget::View(_) => false,
        })
    }

    pub(super) fn switch_tab_relative(&mut self, delta: isize) {
        let len = self.recent_tabs.len();
        if len == 0 {
            return;
        }
        let cur = self.active_tab_index().unwrap_or(0) as isize;
        let next = (cur + delta).rem_euclid(len as isize) as usize;
        let target = self.recent_tabs[next].clone();
        self.navigate_to_target(target, false);
    }

    pub(super) fn switch_tab_index(&mut self, idx: usize) {
        if let Some(target) = self.recent_tabs.get(idx).cloned() {
            self.navigate_to_target(target, false);
        }
    }

    pub(super) fn close_active_tab(&mut self) {
        if let Some(idx) = self.active_tab_index() {
            let target = self.recent_tabs[idx].clone();
            self.close_tab_and_update_view(&target);
        }
    }

    pub(crate) fn build_empty_recent_entries(&self) -> Vec<EmptyRecentEntry> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for key in &self.config.recent_history {
            if out.len() >= 4 {
                break;
            }
            let Some(target) = nav_target_from_key(key) else {
                continue;
            };
            match &target {
                NavTarget::Note(path) => {
                    if let Some(meta) = self.notes.iter().find(|n| n.path == *path) {
                        if seen.insert(path.clone()) {
                            let folder_hint = path
                                .rsplit_once('/')
                                .map(|(d, _)| d.to_string())
                                .unwrap_or_default();
                            out.push(EmptyRecentEntry {
                                title: meta.title.clone(),
                                folder_hint,
                                icon: if meta.is_encrypted() {
                                    Icon::Lock
                                } else {
                                    Icon::Document
                                },
                                target,
                            });
                        }
                    }
                }
                NavTarget::Collection(path) | NavTarget::Canvas(path) => {
                    if let Some(col) = self.collections.iter().find(|c| c.path == *path) {
                        if seen.insert(path.clone()) {
                            let folder_hint = path
                                .rsplit_once('/')
                                .map(|(d, _)| d.to_string())
                                .unwrap_or_default();
                            let icon = match col.kind {
                                VaultFileKind::Board => Icon::Board,
                                VaultFileKind::Table => Icon::Table,
                                VaultFileKind::Canvas => Icon::Canvas,
                                _ => Icon::Document,
                            };
                            out.push(EmptyRecentEntry {
                                title: col.title.clone(),
                                folder_hint,
                                icon,
                                target,
                            });
                        }
                    }
                }
                NavTarget::View(_) => {}
            }
        }

        for note in &self.notes {
            if out.len() >= 4 {
                break;
            }
            if seen.insert(note.path.clone()) {
                let folder_hint = note
                    .path
                    .rsplit_once('/')
                    .map(|(d, _)| d.to_string())
                    .unwrap_or_default();
                out.push(EmptyRecentEntry {
                    title: note.title.clone(),
                    folder_hint,
                    icon: if note.is_encrypted() {
                        Icon::Lock
                    } else {
                        Icon::Document
                    },
                    target: NavTarget::Note(note.path.clone()),
                });
            }
        }
        out
    }

    pub(super) fn record_nav(&mut self, target: NavTarget) {
        if matches!(
            target,
            NavTarget::Note(_) | NavTarget::Collection(_) | NavTarget::Canvas(_)
        ) {
            if let Some(key) = nav_target_key(&target) {
                self.config.recent_history.retain(|k| k != &key);
                self.config.recent_history.insert(0, key.clone());
                self.config.recent_history.truncate(12);
                self.config.last_active_tab_key = Some(key);
            }
            if !self.recent_tabs.contains(&target) {
                self.recent_tabs.push(target.clone());
                self.sync_pinned_tabs_into_recent();
                let unpinned_count = self
                    .recent_tabs
                    .iter()
                    .filter(|t| !self.is_tab_pinned(t))
                    .count();
                if unpinned_count > 10 {
                    if let Some(idx) = self.recent_tabs.iter().position(|t| !self.is_tab_pinned(t)) {
                        self.recent_tabs.remove(idx);
                    }
                }
            }
            self.config.open_tab_keys = self
                .recent_tabs
                .iter()
                .filter_map(nav_target_key)
                .collect();
            let _ = self.config.save();
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

    pub(super) fn can_nav_back(&self) -> bool {
        self.nav_cursor > 0 && self.nav_cursor < self.nav_history.len()
    }

    pub(super) fn can_nav_forward(&self) -> bool {
        self.nav_cursor + 1 < self.nav_history.len()
    }

    pub(super) fn navigate_to_target(&mut self, target: NavTarget, push_history: bool) {
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

    pub(super) fn build_recent_tab_items(&self) -> Vec<RecentTabItem> {
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
                        pinned: self.is_tab_pinned(t),
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
                        pinned: self.is_tab_pinned(t),
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
                        pinned: self.is_tab_pinned(t),
                    });
                }
                NavTarget::View(_) => {}
            }
        }
        out
    }


    pub(super) fn refresh_asset_audit(&mut self) {
        self.asset_audit = audit_vault_assets(&self.config.vault_dir);
    }

    pub(super) fn delete_single_asset(&mut self, rel_path: &str) {
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

    pub(super) fn delete_unused_assets(&mut self) {
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


    pub(super) fn perform_rename(&mut self, old_path: &str, new_title: &str) {
        self.perform_rename_with_prev_title(old_path, new_title, None);
    }

    pub(super) fn perform_rename_with_prev_title(
        &mut self,
        old_path: &str,
        new_title: &str,
        extra_prev_title: Option<&str>,
    ) {
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
                if let Some(prev) = extra_prev_title {
                    old_targets_owned.push(prev.to_string());
                }
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
                for item in &mut self.nav_history {
                    if let NavTarget::Note(p) = item {
                        if p == old_path {
                            *p = new_path.clone();
                        }
                    }
                }
                for item in &mut self.recent_tabs {
                    if let NavTarget::Note(p) = item {
                        if p == old_path {
                            *p = new_path.clone();
                        }
                    }
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

    pub(super) fn toggle_pin(&mut self, rel_path: &str) {
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


    pub(super) fn perform_duplicate(&mut self, rel_path: &str) {
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

    pub(super) fn perform_delete(&mut self, rel_path: &str) {
        let kind = VaultFileKind::from_path(rel_path);
        let is_collection = matches!(kind, Some(VaultFileKind::Board | VaultFileKind::Table));
        let is_canvas = kind == Some(VaultFileKind::Canvas);
        self.watcher.mark_internal_change();
        match delete_note(&self.config.vault_dir, rel_path) {
            Ok(()) => {
                let target = if is_canvas {
                    let _ = self.index.remove_collection(rel_path);
                    NavTarget::Canvas(rel_path.to_string())
                } else if is_collection {
                    let _ = self.index.remove_collection(rel_path);
                    NavTarget::Collection(rel_path.to_string())
                } else {
                    let _ = self.index.remove_note(rel_path);
                    NavTarget::Note(rel_path.to_string())
                };
                self.reload_index_caches();
                self.close_tab_and_update_view(&target);
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

    pub(super) fn delete_folder(&mut self, folder: &str) {
        self.flush_unsaved_documents();
        self.watcher.mark_internal_change();
        match delete_vault_folder(&self.config.vault_dir, folder) {
            Ok(removed) => {
                let prefix = format!("{folder}/");
                let mut active_removed = false;
                for (path, kind) in &removed {
                    match kind {
                        VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => {
                            let _ = self.index.remove_note(path);
                            self.recent_tabs
                                .retain(|t| !matches!(t, NavTarget::Note(p) if p == path));
                            if self.selected_path.as_deref() == Some(path.as_str()) {
                                active_removed = true;
                            }
                        }
                        VaultFileKind::Board | VaultFileKind::Table | VaultFileKind::Canvas => {
                            let _ = self.index.remove_collection(path);
                            self.recent_tabs.retain(|t| {
                                !matches!(t, NavTarget::Collection(p) | NavTarget::Canvas(p) if p == path)
                            });
                            if self.active_collection.path.as_deref() == Some(path.as_str())
                                || self.active_canvas.path.as_deref() == Some(path.as_str())
                            {
                                active_removed = true;
                            }
                        }
                        VaultFileKind::Asset => {
                            let _ = self.index.remove_asset(path);
                        }
                    }
                    self.config.recent_history.retain(|k| {
                        nav_target_from_key(k)
                            .and_then(|t| match t {
                                NavTarget::Note(p)
                                | NavTarget::Collection(p)
                                | NavTarget::Canvas(p) => Some(p),
                                NavTarget::View(_) => None,
                            })
                            .is_none_or(|p| p != *path)
                    });
                    self.config.pinned_tabs.retain(|k| {
                        nav_target_from_key(k)
                            .and_then(|t| match t {
                                NavTarget::Note(p)
                                | NavTarget::Collection(p)
                                | NavTarget::Canvas(p) => Some(p),
                                NavTarget::View(_) => None,
                            })
                            .is_none_or(|p| p != *path)
                    });
                }

                self.expanded_folders
                    .retain(|f| f != folder && !f.starts_with(&prefix));
                self.config
                    .pinned_folders
                    .retain(|f| f != folder && !f.starts_with(&prefix));
                self.config
                    .folder_order
                    .retain(|f| f != folder && !f.starts_with(&prefix));

                self.reload_index_caches();
                if self.recent_tabs.is_empty() || active_removed {
                    if let Some(next) = self.recent_tabs.last().cloned() {
                        self.navigate_to_target(next, false);
                    } else {
                        self.enter_empty_state();
                    }
                }
                self.persist_open_tabs_state();
                self.last_local_save_at = Some(Instant::now());
                let s = strings(self.config.language);
                self.push_toast(s.toast_folder_deleted, ToastKind::Info);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn switch_vault(&mut self, new_dir_str: &str) {
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
        self.cover_cache.set_vault_dir(new_dir.clone());
        self.spellchecker.reload_user_dict(&new_dir);
        self.embedded_notes_cache.clear();
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

    pub(super) fn move_item_to_folder(&mut self, old_path: &str, target_folder: &str) {
        self.flush_unsaved_documents();
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();

        let (new_path, kind, modified, size) = match move_vault_file_to_folder(
            &self.config.vault_dir,
            old_path,
            target_folder,
            &existing,
        ) {
            Ok(res) => res,
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
                return;
            }
        };

        if new_path == old_path {
            return;
        }

        match kind {
            VaultFileKind::Markdown => {
                let _ = self.index.remove_note(old_path);
                if let Ok((doc, _, _)) = read_note(&self.config.vault_dir, &new_path) {
                    let _ = self.index.upsert_note(&new_path, &doc, modified, size);
                }
            }
            VaultFileKind::EncryptedMarkdown => {
                let _ = self.index.remove_note(old_path);
                let title = vault_file_stem(&new_path);
                let _ = self
                    .index
                    .upsert_encrypted_note(&new_path, title, modified, size, "");
            }
            VaultFileKind::Board | VaultFileKind::Table => {
                let _ = self.index.remove_collection(old_path);
                if let Ok((doc, _, _, col_kind)) = read_collection(&self.config.vault_dir, &new_path)
                {
                    let _ = self
                        .index
                        .upsert_collection(&new_path, col_kind, &doc, modified, size);
                }
                let _ = rewrite_vault_collection_links(
                    &self.config.vault_dir,
                    &[old_path],
                    &new_path,
                );
            }
            VaultFileKind::Canvas => {
                let _ = self.index.remove_collection(old_path);
                if let Ok((doc, _, _)) = read_canvas(&self.config.vault_dir, &new_path) {
                    let _ = self.index.upsert_canvas(&new_path, &doc, modified, size);
                }
                let _ = rewrite_vault_canvas_links(
                    &self.config.vault_dir,
                    old_path,
                    &new_path,
                    &[old_path],
                    &new_path,
                );
            }
            VaultFileKind::Asset => return,
        }

        if self.selected_path.as_deref() == Some(old_path) {
            self.selected_path = Some(new_path.clone());
        }
        if self.active_collection.path.as_deref() == Some(old_path) {
            self.active_collection.path = Some(new_path.clone());
        }
        if self.active_canvas.path.as_deref() == Some(old_path) {
            self.active_canvas.path = Some(new_path.clone());
        }
        for tab in &mut self.recent_tabs {
            match tab {
                NavTarget::Note(p) | NavTarget::Collection(p) | NavTarget::Canvas(p)
                    if p == old_path =>
                {
                    *p = new_path.clone();
                }
                _ => {}
            }
        }
        if let Some(split_right) = &mut self.split.right {
            match split_right {
                SplitContent::Note(st) if st.path == old_path => st.path = new_path.clone(),
                SplitContent::Collection(st) if st.path.as_deref() == Some(old_path) => {
                    st.path = Some(new_path.clone());
                }
                _ => {}
            }
        }

        self.reload_index_caches();
        self.expand_parents(&new_path);
        self.refresh_document_derivations();
        self.refresh_embedded_boards();
        self.last_local_save_at = Some(Instant::now());
        self.push_toast(format!("→ {new_path}"), ToastKind::Success);
    }

}
