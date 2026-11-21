use super::{today_local, KronoApp};
use crate::i18n::{strings, Lang};
use crate::storage::vault::{
    create_collection, create_collection_preset, read_collection, save_collection,
};
use crate::ui::{
    components::ToastKind, embedded_board::EmbeddedBoardAction, NavTarget, ViewMode,
};
use krono_core::{
    collection::{CollectionViewKind, ColumnType, FieldValue},
    daily::parse_daily_note_path,
    markdown::{
        extract_embedded_boards, extract_nth_task_line, parse_inline_task_meta,
        remove_nth_task_line, TaskPriority,
    },
    path::{conflict_path, vault_file_stem, VaultFileKind},
};
use std::time::Instant;

impl KronoApp {
    pub(super) fn handle_send_task_to_board(&mut self, task_idx: usize, board_path: &str, move_task: bool) {
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

    pub(super) fn save_current_collection(&mut self) {
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

    pub(super) fn create_and_open_collection(&mut self, title: &str, view_kind: CollectionViewKind) {
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

    pub(super) fn save_collection_external_conflict_copy(&mut self) {
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


    pub(super) fn refresh_embedded_boards(&mut self) {
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


    pub(super) fn create_linked_board_for_current_note(&mut self) {
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

    pub(super) fn handle_embedded_board_action(&mut self, act: EmbeddedBoardAction) {
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

}
