use super::{today_local, KronoApp};
use crate::i18n::strings;
use crate::notifications::send_task_notification;
use crate::storage::vault::{create_note_with_tags, read_note, save_note};
use crate::ui::{components::ToastKind, ViewMode};
use krono_core::{
    daily::{daily_note_rel_path, parse_daily_note_path, SimpleDate},
    path::vault_file_stem,
    tasks::{
        append_carried_tasks, append_wikilink_to_task, extract_open_tasks_for_carryover,
        extract_tasks, toggle_task_in_markdown,
    },
};
use std::collections::HashSet;
use std::time::Instant;

impl KronoApp {
    pub(super) fn quick_add_daily_task(&mut self, raw_text: &str) {
        let trimmed = raw_text.trim();
        if trimmed.is_empty() {
            return;
        }
        let today = today_local();
        let rel_path = daily_note_rel_path(today);
        let task_line = if trimmed.contains('@') || trimmed.contains('📅') {
            format!("- [ ] {trimmed}")
        } else {
            format!("- [ ] {trimmed} @{}", today.format_iso())
        };

        if self.selected_path.as_deref() == Some(rel_path.as_str()) {
            if !self.current_doc.body.is_empty() && !self.current_doc.body.ends_with('\n') {
                self.current_doc.body.push('\n');
            }
            self.current_doc.body.push_str(&task_line);
            self.current_doc.body.push('\n');
            self.dirty = true;
            self.last_edit_at = Some(Instant::now());
            self.save_current_note_inner(false);
            return;
        }

        if let Ok((mut doc, _, _)) = read_note(&self.config.vault_dir, &rel_path) {
            if !doc.body.is_empty() && !doc.body.ends_with('\n') {
                doc.body.push('\n');
            }
            doc.body.push_str(&task_line);
            doc.body.push('\n');
            if let Ok((modified, size)) = save_note(&self.config.vault_dir, &rel_path, &doc) {
                self.last_local_save_at = Some(Instant::now());
                let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                self.reload_index_caches();
            }
        } else {
            let tmpl = self.config.daily_template.clone();
            let tmpl_opt = (!tmpl.trim().is_empty()).then_some(tmpl.as_str());
            if let Ok(created) = crate::storage::vault::create_daily_note_with_template(
                &self.config.vault_dir,
                today,
                self.config.language,
                tmpl_opt,
            ) {
                for (hp, hdoc, hm, hs) in created.hubs {
                    let _ = self.index.upsert_note(&hp, &hdoc, hm, hs);
                }
                let mut doc = created.day_doc;
                if !doc.body.is_empty() && !doc.body.ends_with('\n') {
                    doc.body.push('\n');
                }
                doc.body.push_str(&task_line);
                doc.body.push('\n');
                if let Ok((modified, size)) = save_note(&self.config.vault_dir, &rel_path, &doc) {
                    self.last_local_save_at = Some(Instant::now());
                    let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                    self.reload_index_caches();
                }
            }
        }
    }

    pub(super) fn due_tasks_badge_count(&self, today: SimpleDate) -> usize {
        self.vault_tasks
            .iter()
            .filter(|t| !t.done && t.due_date.is_some_and(|d| d <= today))
            .count()
    }

    pub(super) fn toggle_vault_task(&mut self, path: &str, task_idx: usize) {
        let today = today_local();
        let ref_date = parse_daily_note_path(path).unwrap_or(today);

        if self.selected_path.as_deref() == Some(path) {
            if let Some(updated) = toggle_task_in_markdown(&self.current_doc.body, task_idx, ref_date) {
                self.current_doc.body = updated;
                self.dirty = true;
                self.last_edit_at = Some(Instant::now());
                self.save_current_note_inner(false);
            }
            return;
        }

        match read_note(&self.config.vault_dir, path) {
            Ok((mut doc, _, _)) => {
                if let Some(updated) = toggle_task_in_markdown(&doc.body, task_idx, ref_date) {
                    doc.body = updated;
                    match save_note(&self.config.vault_dir, path, &doc) {
                        Ok((modified, size)) => {
                            self.last_local_save_at = Some(Instant::now());
                            if let Err(e) = self.index.upsert_note(path, &doc, modified, size) {
                                self.push_toast(e, ToastKind::Error);
                            } else {
                                self.reload_index_caches();
                            }
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn carry_over_daily_tasks(&mut self) {
        let today = today_local();
        let s = strings(self.config.language);

        let mut prev_dates: Vec<SimpleDate> = self
            .daily_dates
            .iter()
            .copied()
            .filter(|&d| d < today)
            .collect();
        prev_dates.sort();

        let Some(prev_date) = prev_dates.last().copied() else {
            self.push_toast(s.tasks_carry_over_none, ToastKind::Info);
            return;
        };

        let prev_path = self
            .notes
            .iter()
            .find(|n| parse_daily_note_path(&n.path) == Some(prev_date))
            .map(|n| n.path.clone())
            .unwrap_or_else(|| daily_note_rel_path(prev_date));

        let open_tasks = match read_note(&self.config.vault_dir, &prev_path) {
            Ok((prev_doc, _, _)) => extract_open_tasks_for_carryover(&prev_doc.body),
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
                return;
            }
        };

        if open_tasks.is_empty() {
            self.push_toast(s.tasks_carry_over_none, ToastKind::Info);
            return;
        }

        let is_on_today = self
            .selected_path
            .as_deref()
            .and_then(parse_daily_note_path)
            == Some(today);
        if !is_on_today {
            self.open_or_create_daily_note(today);
        }

        let (merged, added) = append_carried_tasks(&self.current_doc.body, &open_tasks, prev_date);
        if added == 0 {
            self.push_toast(s.tasks_carry_over_none, ToastKind::Info);
            return;
        }

        self.current_doc.body = merged;
        self.dirty = true;
        self.last_edit_at = Some(Instant::now());
        self.save_current_note_inner(false);
        self.push_toast(format!("{} {}", s.tasks_carry_over_done, added), ToastKind::Success);
    }

    pub(super) fn tick_task_notifications(&mut self) {
        if !self.config.task_notifications || self.first_run_open {
            return;
        }
        let today = today_local();
        if let Some((summary, body)) =
            self.task_notifier
                .next_due_batch(&self.vault_tasks, today, self.config.language)
        {
            let _ = send_task_notification(&summary, &body);
        }
    }

    pub(super) fn convert_task_to_note(&mut self, source_path: &str, task_idx: usize) {
        let today = today_local();
        let ref_date = parse_daily_note_path(source_path).unwrap_or(today);
        let s = strings(self.config.language);

        let (mut src_doc, is_open) = if self.selected_path.as_deref() == Some(source_path) {
            (self.current_doc.clone(), true)
        } else {
            match read_note(&self.config.vault_dir, source_path) {
                Ok((doc, _, _)) => (doc, false),
                Err(e) => {
                    self.push_toast(e, ToastKind::Error);
                    return;
                }
            }
        };

        let tasks = extract_tasks(&src_doc.body, ref_date);
        let Some(task) = tasks.into_iter().find(|t| t.task_idx == task_idx) else {
            return;
        };

        let title = if task.clean_text.trim().is_empty() {
            task.raw_text.trim().to_string()
        } else {
            task.clean_text.trim().to_string()
        };
        let folder_prefix = source_path
            .rsplit_once('/')
            .map(|(dir, _)| dir)
            .filter(|dir| !dir.starts_with("daily"));
        let raw_title_or_path = match folder_prefix {
            Some(dir) if !dir.is_empty() => format!("{dir}/{title}"),
            _ => title.clone(),
        };

        let existing: HashSet<String> = self.notes.iter().map(|n| n.path.clone()).collect();
        let mut initial_body = format!("# {title}\n\n");
        if let Some(due) = task.due_date {
            initial_body.push_str(&format!("Due: {}\n\n", due.format_iso()));
        }

        match create_note_with_tags(
            &self.config.vault_dir,
            &raw_title_or_path,
            None,
            task.tags.clone(),
            &initial_body,
            &existing,
        ) {
            Ok((new_rel_path, new_doc, modified, size)) => {
                self.last_local_save_at = Some(Instant::now());
                let _ = self.index.upsert_note(&new_rel_path, &new_doc, modified, size);

                let link_target = vault_file_stem(&new_rel_path);
                if let Some(updated_src) =
                    append_wikilink_to_task(&src_doc.body, task_idx, link_target)
                {
                    src_doc.body = updated_src;
                    if is_open {
                        self.current_doc.body = src_doc.body.clone();
                        self.dirty = true;
                        self.save_current_note_inner(false);
                    } else if let Ok((sm, ss)) =
                        save_note(&self.config.vault_dir, source_path, &src_doc)
                    {
                        self.last_local_save_at = Some(Instant::now());
                        let _ = self.index.upsert_note(source_path, &src_doc, sm, ss);
                    }
                }

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
}
