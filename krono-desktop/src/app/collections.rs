use super::{today_local, KronoApp};
use crate::i18n::{strings, Lang};
use crate::storage::vault::{
    create_collection, create_collection_preset, create_note_with_tags, read_collection,
    save_collection,
};
use crate::sync::{
    secrets::load_tmdb_token,
    tmdb::{
        spawn_discover_by_genres, spawn_fetch_details, spawn_fetch_recommendations,
        spawn_search_multi, TmdbEvent, TmdbMediaDetails, TmdbMediaKind, TmdbSearchItem,
    },
};
use crate::ui::{
    components::ToastKind, embedded_board::EmbeddedBoardAction, NavTarget, ViewMode,
};
use krono_core::{
    collection::{CollectionViewKind, ColumnDef, ColumnType, FieldValue},
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
            Ok((mut doc, mut modified, _, _)) => {
                if doc.localize_media_library_in_place(self.config.language == Lang::Ru) {
                    self.watcher.mark_internal_change();
                    if let Ok((m, sz)) = save_collection(&self.config.vault_dir, rel_path, &doc) {
                        modified = m;
                        let kind =
                            VaultFileKind::from_path(rel_path).unwrap_or(VaultFileKind::Table);
                        let _ = self.index.upsert_collection(rel_path, kind, &doc, m, sz);
                        self.reload_index_caches();
                    }
                }
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

    pub(super) fn convert_record_to_note(&mut self, record_id: &str) {
        let Some(col_path) = self.active_collection.path.clone() else {
            return;
        };
        let Some(rec_snap) = self
            .active_collection
            .doc
            .records
            .iter()
            .find(|r| r.id == record_id)
            .cloned()
        else {
            return;
        };

        let s = strings(self.config.language);
        let fallback_title = if self.config.language == Lang::Ru {
            "Новая заметка"
        } else {
            "New Note"
        };
        let title = if rec_snap.title.trim().is_empty() {
            fallback_title.to_string()
        } else {
            rec_snap.title.trim().to_string()
        };
        let folder_prefix = col_path
            .rsplit_once('/')
            .map(|(dir, _)| dir)
            .filter(|dir| !dir.starts_with("daily"));
        let raw_title_or_path = match folder_prefix {
            Some(dir) if !dir.is_empty() => format!("{dir}/{title}"),
            _ => title.clone(),
        };

        let mut body = String::new();
        let desc = rec_snap.description.trim();
        if !desc.is_empty() {
            body.push_str(desc);
            body.push_str("\n\n");
        }
        if !rec_snap.checklist.is_empty() {
            for item in &rec_snap.checklist {
                let mark = if item.done { 'x' } else { ' ' };
                body.push_str(&format!("- [{}] {}\n", mark, item.text.trim()));
            }
        }

        let existing: std::collections::HashSet<String> =
            self.notes.iter().map(|n| n.path.clone()).collect();
        match create_note_with_tags(
            &self.config.vault_dir,
            &raw_title_or_path,
            None,
            rec_snap.tags.clone(),
            &body,
            &existing,
        ) {
            Ok((new_rel_path, new_doc, modified, size)) => {
                self.last_local_save_at = Some(Instant::now());
                let _ = self.index.upsert_note(&new_rel_path, &new_doc, modified, size);

                // Ensure collection has a NoteLink column and populate it for this record
                let note_link_col_id = if let Some(col) = self
                    .active_collection
                    .doc
                    .columns
                    .iter()
                    .find(|c| c.col_type == ColumnType::NoteLink)
                {
                    col.id.clone()
                } else {
                    let id = "note_link".to_string();
                    self.active_collection.doc.columns.push(ColumnDef {
                        id: id.clone(),
                        name: if self.config.language == Lang::Ru {
                            "Заметка".to_string()
                        } else {
                            "Note".to_string()
                        },
                        col_type: ColumnType::NoteLink,
                        width: 170.0,
                        options: Vec::new(),
                    });
                    id
                };

                if let Some(rec) = self
                    .active_collection
                    .doc
                    .records
                    .iter_mut()
                    .find(|r| r.id == record_id)
                {
                    rec.values.insert(
                        note_link_col_id,
                        FieldValue::Text(new_doc.frontmatter.title.clone()),
                    );
                }
                self.active_collection.dirty = true;
                self.active_collection.editing_record_id = None;
                self.save_current_collection();

                self.reload_index_caches();
                self.open_note(&new_rel_path);
                self.view_mode = ViewMode::Editor;
                self.previous_view_mode = ViewMode::Editor;
                self.last_notes_tab_mode = ViewMode::Editor;
                self.push_toast(s.tasks_converted_toast, ToastKind::Success);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn create_media_review_note(&mut self, record_id: &str) {
        let Some(col_path) = self.active_collection.path.clone() else {
            return;
        };
        let Some(rec_snap) = self
            .active_collection
            .doc
            .records
            .iter()
            .find(|r| r.id == record_id)
            .cloned()
        else {
            return;
        };

        let is_ru = self.config.language == Lang::Ru;
        let title = if rec_snap.title.trim().is_empty() {
            if is_ru { "Рецензия" } else { "Review" }.to_string()
        } else {
            rec_snap.title.trim().to_string()
        };
        let year = rec_snap
            .values
            .get("year")
            .map(FieldValue::as_text)
            .unwrap_or_default();
        let director = rec_snap
            .values
            .get("director")
            .map(FieldValue::as_text)
            .unwrap_or_default();
        let genres_raw = rec_snap
            .values
            .get("genres")
            .map(FieldValue::as_text)
            .unwrap_or_default();
        let mut genres: Vec<String> = genres_raw
            .split([',', ';', '/'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        for t in &rec_snap.tags {
            if !genres.iter().any(|g| g.eq_ignore_ascii_case(t)) {
                genres.push(t.clone());
            }
        }
        let rating = rec_snap
            .values
            .get("rating")
            .map(FieldValue::as_text)
            .unwrap_or_default();
        let mini_review = rec_snap
            .values
            .get("review")
            .map(FieldValue::as_text)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| rec_snap.description.clone());

        let (tags, body) = krono_core::templates::media_review_template(
            &title,
            &year,
            &director,
            &genres,
            &rating,
            &mini_review,
            is_ru,
        );

        let folder_prefix = col_path
            .rsplit_once('/')
            .map(|(dir, _)| dir)
            .filter(|dir| !dir.starts_with("daily"));
        let note_title = if is_ru {
            format!("{title} — рецензия")
        } else {
            format!("{title} — Review")
        };
        let target_folder = match folder_prefix {
            Some(dir) if !dir.is_empty() => {
                if dir.ends_with("/Рецензии")
                    || dir.ends_with("/Reviews")
                    || dir == "Рецензии"
                    || dir == "Reviews"
                {
                    dir.to_string()
                } else if is_ru {
                    format!("{dir}/Рецензии")
                } else {
                    format!("{dir}/Reviews")
                }
            }
            _ => {
                if is_ru {
                    "Кинотека/Рецензии".to_string()
                } else {
                    "Media/Reviews".to_string()
                }
            }
        };
        let raw_title_or_path = format!("{target_folder}/{note_title}");
        let existing: std::collections::HashSet<String> =
            self.notes.iter().map(|n| n.path.clone()).collect();
        match create_note_with_tags(
            &self.config.vault_dir,
            &raw_title_or_path,
            None,
            tags,
            &body,
            &existing,
        ) {
            Ok((new_rel_path, new_doc, modified, size)) => {
                self.last_local_save_at = Some(Instant::now());
                let _ = self.index.upsert_note(&new_rel_path, &new_doc, modified, size);

                let note_link_col_id = if let Some(col) = self
                    .active_collection
                    .doc
                    .columns
                    .iter()
                    .find(|c| c.col_type == ColumnType::NoteLink)
                {
                    col.id.clone()
                } else {
                    let id = "note_link".to_string();
                    self.active_collection.doc.columns.push(ColumnDef {
                        id: id.clone(),
                        name: if is_ru { "Заметка" } else { "Note" }.to_string(),
                        col_type: ColumnType::NoteLink,
                        width: 170.0,
                        options: Vec::new(),
                    });
                    id
                };

                if let Some(rec) = self
                    .active_collection
                    .doc
                    .records
                    .iter_mut()
                    .find(|r| r.id == record_id)
                {
                    rec.values.insert(
                        note_link_col_id,
                        FieldValue::Text(new_rel_path.clone()),
                    );
                }
                self.active_collection.dirty = true;
                self.active_collection.editing_record_id = None;
                self.save_current_collection();

                self.reload_index_caches();
                self.open_note(&new_rel_path);
                self.view_mode = ViewMode::Editor;
                self.previous_view_mode = ViewMode::Editor;
                self.last_notes_tab_mode = ViewMode::Editor;
                let s = strings(self.config.language);
                self.push_toast(s.tasks_converted_toast, ToastKind::Success);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn poll_tmdb_events(&mut self) {
        while let Ok(ev) = self.tmdb_rx.try_recv() {
            match ev {
                TmdbEvent::SearchDone { query, result } => {
                    if query != self.active_collection.tmdb_ui.last_dispatched_query
                        && !self.active_collection.tmdb_ui.last_dispatched_query.is_empty()
                    {
                        continue;
                    }
                    self.active_collection.tmdb_ui.search_busy = false;
                    match result {
                        Ok(items) => {
                            self.active_collection.tmdb_ui.search_error = None;
                            self.active_collection.tmdb_ui.search_results = items;
                        }
                        Err(e) => {
                            self.active_collection.tmdb_ui.search_error = Some(e);
                        }
                    }
                }
                TmdbEvent::DetailsDone {
                    target_status,
                    fallback,
                    result,
                } => {
                    let details = match result {
                        Ok(d) => d,
                        Err(_) => TmdbMediaDetails {
                            item: fallback,
                            genres: Vec::new(),
                            director: String::new(),
                        },
                    };
                    self.insert_tmdb_media_record(details, &target_status);
                }
                TmdbEvent::RecommendationsDone(result) => {
                    self.active_collection.tmdb_ui.recs_busy = false;
                    match result {
                        Ok(items) => {
                            self.active_collection.tmdb_ui.recs_error = None;
                            self.active_collection.tmdb_ui.recs_results = items;
                        }
                        Err(e) => {
                            self.active_collection.tmdb_ui.recs_error = Some(e);
                        }
                    }
                }
            }
        }
    }

    pub(super) fn start_tmdb_search(&mut self, ctx: &eframe::egui::Context, query: String) {
        let Ok(Some(token)) = load_tmdb_token() else {
            self.has_saved_tmdb_token = false;
            return;
        };
        self.active_collection.tmdb_ui.search_busy = true;
        self.active_collection.tmdb_ui.search_error = None;
        spawn_search_multi(
            ctx.clone(),
            self.tmdb_tx.clone(),
            token.to_string(),
            query,
            self.config.language,
            self.config.effective_tmdb_language(),
        );
    }

    pub(super) fn start_tmdb_recommendations(&mut self, ctx: &eframe::egui::Context) {
        self.active_collection.tmdb_ui.recs_open = true;
        self.active_collection.tmdb_ui.recs_error = None;
        if !self.has_saved_tmdb_token {
            return;
        }
        let Ok(Some(token)) = load_tmdb_token() else {
            self.has_saved_tmdb_token = false;
            return;
        };

        let existing_ids: std::collections::HashSet<String> = self
            .active_collection
            .doc
            .records
            .iter()
            .filter_map(|r| r.values.get("tmdb_id"))
            .map(|v| v.as_text().trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();

        let mut seeds: Vec<(f64, TmdbMediaKind, String)> = self
            .active_collection
            .doc
            .records
            .iter()
            .filter_map(|r| {
                let rating = r.values.get("rating").and_then(FieldValue::as_number)?;
                if rating < 8.0 {
                    return None;
                }
                let tmdb_id = r
                    .values
                    .get("tmdb_id")
                    .map(FieldValue::as_text)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())?;
                let kind = r
                    .values
                    .get("media_type")
                    .map(FieldValue::as_text)
                    .and_then(|s| TmdbMediaKind::from_select_opt(&s))
                    .unwrap_or(TmdbMediaKind::Movie);
                Some((rating, kind, tmdb_id))
            })
            .collect();
        seeds.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let seed_pairs: Vec<(TmdbMediaKind, String)> =
            seeds.into_iter().take(5).map(|(_, k, id)| (k, id)).collect();

        if seed_pairs.is_empty() {
            let s = strings(self.config.language);
            self.active_collection.tmdb_ui.recs_busy = false;
            self.active_collection.tmdb_ui.recs_results.clear();
            self.active_collection.tmdb_ui.recs_error = Some(s.tmdb_recs_need_high_rating.to_string());
            return;
        }

        self.active_collection.tmdb_ui.recs_busy = true;
        spawn_fetch_recommendations(
            ctx.clone(),
            self.tmdb_tx.clone(),
            token.to_string(),
            seed_pairs,
            existing_ids,
            self.config.language,
            self.config.effective_tmdb_language(),
        );
    }

    pub(super) fn start_tmdb_add_media(
        &mut self,
        ctx: &eframe::egui::Context,
        item: TmdbSearchItem,
        status_opt: String,
    ) {
        if let Ok(Some(token)) = load_tmdb_token() {
            spawn_fetch_details(
                ctx.clone(),
                self.tmdb_tx.clone(),
                token.to_string(),
                item,
                status_opt,
                self.config.language,
                self.config.effective_tmdb_language(),
            );
        } else {
            self.insert_tmdb_media_record(
                TmdbMediaDetails {
                    item,
                    genres: Vec::new(),
                    director: String::new(),
                },
                &status_opt,
            );
        }
    }

    fn insert_tmdb_media_record(&mut self, details: TmdbMediaDetails, status_opt: &str) {
        let item = details.item;
        let rec_id = self.active_collection.doc.add_record(
            &item.title,
            Some(("status", status_opt)),
            chrono::Utc::now().timestamp(),
        );
        if let Some(rec) = self
            .active_collection
            .doc
            .records
            .iter_mut()
            .find(|r| r.id == rec_id)
        {
            rec.values.insert(
                "media_type".into(),
                FieldValue::Text(item.kind.as_select_opt().into()),
            );
            if let Some(year) = item.year {
                rec.values.insert("year".into(), FieldValue::Number(year as f64));
            }
            if !details.genres.is_empty() {
                let is_ru = self.config.language == Lang::Ru;
                let mut norm_genres: Vec<String> = Vec::new();
                for g in &details.genres {
                    let norm = krono_core::collection::normalize_genre_label(g, is_ru);
                    if !norm.is_empty()
                        && !norm_genres
                            .iter()
                            .any(|x| x.to_lowercase() == norm.to_lowercase())
                    {
                        norm_genres.push(norm);
                    }
                }
                if !norm_genres.is_empty() {
                    rec.values.insert(
                        "genres".into(),
                        FieldValue::Text(norm_genres.join(", ")),
                    );
                }
            }
            if !details.director.trim().is_empty() {
                rec.values.insert(
                    "director".into(),
                    FieldValue::Text(details.director.trim().to_string()),
                );
            }
            rec.values
                .insert("tmdb_id".into(), FieldValue::Text(item.tmdb_id));
            if let Some(pp) = item.poster_path {
                rec.values.insert("poster_path".into(), FieldValue::Text(pp));
            }
            if !item.overview.trim().is_empty() {
                rec.description = item.overview;
            }
        }
        self.active_collection.dirty = true;
        self.save_current_collection();
        let s = strings(self.config.language);
        self.push_toast(format!("{}: {}", s.tmdb_added_toast, item.title), ToastKind::Success);
    }

    pub(super) fn start_tmdb_similar(
        &mut self,
        ctx: &eframe::egui::Context,
        kind: TmdbMediaKind,
        tmdb_id: String,
    ) {
        self.active_collection.tmdb_ui.recs_open = true;
        self.active_collection.tmdb_ui.recs_error = None;
        let Ok(Some(token)) = load_tmdb_token() else {
            self.has_saved_tmdb_token = false;
            return;
        };
        let existing_ids: std::collections::HashSet<String> = self
            .active_collection
            .doc
            .records
            .iter()
            .filter_map(|r| r.values.get("tmdb_id"))
            .map(|v| v.as_text().trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();
        self.active_collection.tmdb_ui.recs_busy = true;
        spawn_fetch_recommendations(
            ctx.clone(),
            self.tmdb_tx.clone(),
            token.to_string(),
            vec![(kind, tmdb_id)],
            existing_ids,
            self.config.language,
            self.config.effective_tmdb_language(),
        );
    }

    pub(super) fn start_tmdb_by_genres(
        &mut self,
        ctx: &eframe::egui::Context,
        kind: TmdbMediaKind,
        genre_keys: Vec<u32>,
    ) {
        self.active_collection.tmdb_ui.recs_open = true;
        self.active_collection.tmdb_ui.recs_error = None;
        let Ok(Some(token)) = load_tmdb_token() else {
            self.has_saved_tmdb_token = false;
            return;
        };
        let existing_ids: std::collections::HashSet<String> = self
            .active_collection
            .doc
            .records
            .iter()
            .filter_map(|r| r.values.get("tmdb_id"))
            .map(|v| v.as_text().trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();
        self.active_collection.tmdb_ui.recs_busy = true;
        spawn_discover_by_genres(
            ctx.clone(),
            self.tmdb_tx.clone(),
            token.to_string(),
            kind,
            genre_keys,
            existing_ids,
            self.config.language,
            self.config.effective_tmdb_language(),
        );
    }

    pub(super) fn open_sidebar_module(&mut self, module: crate::ui::SidebarModule) {
        let is_ru = self.config.language == Lang::Ru;
        match module {
            crate::ui::SidebarModule::MediaLibrary => {
                let found = self.collections.iter().find_map(|c| {
                    let t = c.title.to_lowercase();
                    let p = c.path.to_lowercase();
                    if t.contains("кинотек") || t.contains("media library") || p.contains("media") || p.contains("кинотек") {
                        return Some(c.path.clone());
                    }
                    None
                });
                if let Some(path) = found {
                    self.open_collection(&path);
                } else {
                    let raw_path = if is_ru {
                        "Кинотека/Кинотека"
                    } else {
                        "Media/Media Library"
                    };
                    let existing = self.existing_paths_set();
                    self.watcher.mark_internal_change();
                    match create_collection_preset(
                        &self.config.vault_dir,
                        raw_path,
                        CollectionViewKind::Gallery,
                        "media_library",
                        is_ru,
                        &existing,
                    ) {
                        Ok((rel_path, fkind, doc, modified, size)) => {
                            let _ = self
                                .index
                                .upsert_collection(&rel_path, fkind, &doc, modified, size);
                            self.reload_index_caches();
                            self.open_collection(&rel_path);
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
            }
            crate::ui::SidebarModule::English => {
                self.open_or_create_english_workspace();
            }
        }
    }
}
