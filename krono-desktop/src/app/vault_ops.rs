use super::KronoApp;
use crate::i18n::{strings, Lang};
use crate::storage::{
    index::VaultIndex,
    vault::{
        audit_vault_assets, delete_note, duplicate_canvas, duplicate_collection, duplicate_note,
        ensure_starter_vault, migrate_diary_hierarchy, read_canvas, read_collection, read_note,
        rename_canvas, rename_collection, rename_note, rewrite_vault_canvas_links,
        rewrite_vault_collection_links, rewrite_vault_wikilinks, save_canvas, save_collection,
        save_note,
    },
};
use crate::ui::{components::ToastKind, NavTarget, RecentTabItem, ViewMode};
use krono_core::{
    note::NoteDocument,
    path::{vault_file_stem, VaultFileKind},
};
use std::{path::PathBuf, time::Instant};

impl KronoApp {
    pub(super) fn record_nav(&mut self, target: NavTarget) {
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

}
