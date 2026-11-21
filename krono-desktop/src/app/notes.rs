use super::KronoApp;
use crate::i18n::Lang;
use crate::storage::vault::{
    create_canvas_preset, create_collection_preset, create_daily_note_with_template,
    create_language_lesson_note, create_note_with_preset,
    create_note_with_tags, ensure_daily_hubs_for_date, read_note, save_note,
};
use crate::ui::{
    components::ToastKind,
    graph_view::{build_local_graph, LocalGraphData},
    inspector::OutgoingLinkInfo,
    CreateItemKind, EditorMode, NavTarget, ViewMode,
};
use krono_core::{
    collection::CollectionViewKind,
    daily::{daily_legacy_rel_path, daily_note_rel_path, SimpleDate},
    markdown::{extract_headings, extract_wikilinks},
    note::{normalize_tags, NoteMeta, NoteStatus},
    path::{conflict_path, unique_note_path},
};
use std::{
    collections::{BTreeSet, HashSet},
    time::Instant,
};

impl KronoApp {
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

    pub(super) fn find_note_by_wikilink(&self, target: &str) -> Option<String> {
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

    pub(super) fn existing_paths_set(&self) -> HashSet<String> {
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

    pub(super) fn expand_parents(&mut self, rel_path: &str) {
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

    pub(super) fn save_current_note(&mut self) {
        self.save_current_note_inner(false);
    }

    pub(super) fn save_current_note_inner(&mut self, sync_filename: bool) {
        let Some(rel_path) = self.selected_path.clone() else {
            return;
        };
        let prev_title = self
            .notes
            .iter()
            .find(|n| n.path == rel_path)
            .map(|n| n.title.clone());
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

                if sync_filename && !rel_path.starts_with("daily/") {
                    let clean_title = self.current_doc.frontmatter.title.trim().to_string();
                    if !clean_title.is_empty() {
                        let folder = rel_path.rsplit_once('/').map(|(d, _)| d);
                        let mut filtered_existing = self.existing_paths_set();
                        filtered_existing.remove(&rel_path);
                        let desired = unique_note_path(folder, &clean_title, &filtered_existing);
                        if desired != rel_path {
                            self.perform_rename_with_prev_title(
                                &rel_path,
                                &clean_title,
                                prev_title.as_deref(),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
            }
        }
    }

    pub(super) fn create_and_open_note(&mut self, title: &str, status: Option<NoteStatus>, body: &str) {
        self.create_and_open_note_with_tags(title, status, Vec::new(), body);
    }

    pub(super) fn create_and_open_note_with_tags(
        &mut self,
        title: &str,
        status: Option<NoteStatus>,
        tags: Vec<String>,
        body: &str,
    ) {
        self.flush_unsaved_documents();
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        match create_note_with_tags(&self.config.vault_dir, title, status, tags, body, &existing) {
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

    pub(super) fn existing_folders_list(&self) -> Vec<String> {
        let mut set = BTreeSet::new();
        for f in &self.vault_folders {
            if !f.is_empty() && f != "daily" && !f.starts_with("daily/") {
                set.insert(f.clone());
            }
        }
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

    pub(super) fn create_from_modal(
        &mut self,
        kind: CreateItemKind,
        title: &str,
        folder: &str,
        preset_id: &str,
    ) {
        self.flush_unsaved_documents();
        let is_ru = self.config.language == Lang::Ru;
        let study_dir = self.config.localized_study_language().replace(['/', '\\'], " ");
        let study_lessons_dir = if is_ru {
            format!("{study_dir}/Уроки")
        } else {
            format!("{study_dir}/Lessons")
        };
        let auto_folder: Option<String> = if folder.trim().is_empty() && !title.contains('/') {
            match preset_id {
                "media_library" => Some(if is_ru { "Кинотека" } else { "Media" }.to_string()),
                "vocabulary" => Some(study_dir.clone()),
                "english_lesson" => Some(study_lessons_dir),
                _ => None,
            }
        } else {
            None
        };
        let effective_folder = if folder.trim().is_empty() {
            auto_folder.as_deref().unwrap_or("")
        } else {
            folder.trim()
        };
        let raw_path = if effective_folder.is_empty() {
            title.trim().to_string()
        } else {
            format!("{}/{}", effective_folder.trim_matches('/'), title.trim())
        };
        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();

        match kind {
            CreateItemKind::Note => {
                let res = if preset_id == "english_lesson" {
                    create_language_lesson_note(
                        &self.config.vault_dir,
                        &raw_path,
                        self.config.effective_study_language(),
                        self.config.language,
                        &existing,
                    )
                } else {
                    create_note_with_preset(
                        &self.config.vault_dir,
                        &raw_path,
                        preset_id,
                        self.config.language,
                        &existing,
                    )
                };
                match res {
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

    pub(super) fn update_note_status(&mut self, rel_path: &str, new_status: Option<NoteStatus>) {
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

    pub(super) fn sorted_board_column_paths(&self, status: NoteStatus, exclude: Option<&str>) -> Vec<String> {
        let mut items: Vec<&NoteMeta> = self
            .notes
            .iter()
            .filter(|n| n.status == Some(status) && exclude.is_none_or(|ex| n.path != ex))
            .collect();
        items.sort_by(|a, b| {
            a.board_order
                .cmp(&b.board_order)
                .then_with(|| b.modified.cmp(&a.modified))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        items.into_iter().map(|n| n.path.clone()).collect()
    }

    pub(super) fn apply_board_column_order(&mut self, status: NoteStatus, ordered_paths: &[String]) {
        self.watcher.mark_internal_change();
        let mut any_changed = false;
        for (idx, rel_path) in ordered_paths.iter().enumerate() {
            let new_order = ((idx + 1) * 10) as i64;
            let meta = self.notes.iter().find(|n| &n.path == rel_path);
            let needs_write =
                meta.is_none_or(|m| m.status != Some(status) || m.board_order != new_order);
            if !needs_write {
                continue;
            }
            if self.selected_path.as_deref() == Some(rel_path.as_str()) {
                self.current_doc.frontmatter.status = Some(status);
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
                doc.frontmatter.status = Some(status);
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

    pub(super) fn move_note_in_board(
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

    pub(super) fn shift_note_in_board(&mut self, rel_path: &str, delta: isize) {
        let Some(cur_status) = self
            .notes
            .iter()
            .find(|n| n.path == rel_path)
            .and_then(|n| n.status)
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


    pub(super) fn open_or_create_daily_note(&mut self, date: SimpleDate) {
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
        match create_daily_note_with_template(
            &self.config.vault_dir,
            date,
            self.config.language,
            Some(&self.config.daily_template),
        ) {
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

    pub(super) fn save_external_conflict_copy(&mut self) {
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

}
