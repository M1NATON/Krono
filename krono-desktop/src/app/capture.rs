use super::{today_local, KronoApp};
use crate::config::write_file_atomic;
use crate::i18n::{strings, Lang};
use crate::storage::vault::{
    create_daily_note_with_template, create_note_with_tags, ensure_daily_hubs_for_date, read_note,
    save_note,
};
use crate::tray::TrayCommand;
use crate::ui::components::ToastKind;
use crate::ui::scratchpad::{
    compute_scratchpad_initial_pos, scratchpad_viewport_id, ScratchpadAction,
    SCRATCHPAD_DEFAULT_SIZE,
};
use crate::ui::ViewMode;
use chrono::Local;
use eframe::egui::{self, ViewportCommand};
use krono_core::capture::{append_timestamped_entry, split_scratchpad_note};
use krono_core::daily::{daily_legacy_rel_path, daily_note_rel_path, parse_daily_note_path};
use krono_core::note::{Frontmatter, NoteDocument};
use krono_core::weekly_review::{generate_weekly_review as build_weekly_review, DailyReviewInput};
use std::collections::HashSet;
use std::path::PathBuf;

impl KronoApp {
    fn scratchpad_draft_path(&self) -> PathBuf {
        self.config.vault_dir.join(".krono").join("scratchpad.tmp")
    }

    fn load_scratchpad_draft_if_needed(&mut self) {
        if self.scratchpad.loaded_from_disk {
            return;
        }
        self.scratchpad.loaded_from_disk = true;
        let path = self.scratchpad_draft_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if self.scratchpad.text.is_empty() && !content.is_empty() {
                self.scratchpad.text = content;
            }
        }
    }

    pub(super) fn persist_scratchpad_draft(&mut self) {
        let path = self.scratchpad_draft_path();
        if self.scratchpad.text.trim().is_empty() {
            let _ = std::fs::remove_file(&path);
        } else {
            let _ = write_file_atomic(&path, self.scratchpad.text.as_bytes());
        }
        self.scratchpad.draft_dirty = false;
    }

    fn clear_scratchpad_draft(&mut self) {
        self.scratchpad.text.clear();
        self.scratchpad.draft_dirty = false;
        let _ = std::fs::remove_file(self.scratchpad_draft_path());
    }

    pub(super) fn wake_and_focus_window(&self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        ctx.request_repaint();
    }

    pub(super) fn open_scratchpad(&mut self, ctx: &egui::Context) {
        self.load_scratchpad_draft_if_needed();
        if !self.scratchpad.open {
            self.scratchpad.initial_pos = compute_scratchpad_initial_pos(
                ctx.input(|i| i.viewport().monitor_size),
                SCRATCHPAD_DEFAULT_SIZE,
            );
        }
        self.scratchpad.open = true;
        self.scratchpad.request_focus = true;
        let vp_id = scratchpad_viewport_id();
        ctx.send_viewport_cmd_to(vp_id, ViewportCommand::Visible(true));
        ctx.send_viewport_cmd_to(vp_id, ViewportCommand::Focus);
        ctx.request_repaint_of(vp_id);
        ctx.request_repaint();
    }

    pub(super) fn close_scratchpad(&mut self, ctx: &egui::Context) {
        self.persist_scratchpad_draft();
        self.scratchpad.open = false;
        self.scratchpad.request_focus = false;
        ctx.request_repaint();
    }

    pub(super) fn toggle_scratchpad(&mut self, ctx: &egui::Context) {
        if self.scratchpad.open {
            self.close_scratchpad(ctx);
        } else {
            self.open_scratchpad(ctx);
        }
    }

    pub(super) fn poll_tray_commands(&mut self, ctx: &egui::Context) {
        let cmds = self.tray.poll();
        for cmd in cmds {
            match cmd {
                TrayCommand::OpenScratchpad => {
                    self.open_scratchpad(ctx);
                }
                TrayCommand::ToggleScratchpad => {
                    self.toggle_scratchpad(ctx);
                }
                TrayCommand::OpenTodayDaily => {
                    let today = today_local();
                    self.calendar_year = today.year;
                    self.calendar_month = today.month;
                    self.open_or_create_daily_note(today);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                    self.wake_and_focus_window(ctx);
                }
                TrayCommand::ShowMainWindow => {
                    self.wake_and_focus_window(ctx);
                }
                TrayCommand::Quit => {
                    self.flush_unsaved_documents();
                    self.persist_scratchpad_draft();
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
    }

    pub(super) fn handle_scratchpad_action(
        &mut self,
        ctx: &egui::Context,
        action: ScratchpadAction,
    ) {
        let lang = self.config.language;
        let s = strings(lang);
        match action {
            ScratchpadAction::Close => {
                self.close_scratchpad(ctx);
            }
            ScratchpadAction::Clear => {
                self.clear_scratchpad_draft();
                self.scratchpad.request_focus = true;
            }
            ScratchpadAction::CopyAndClear => {
                let text = self.scratchpad.text.clone();
                if !text.trim().is_empty() {
                    if let Ok(mut cb) = arboard::Clipboard::new() {
                        let _ = cb.set_text(text.clone());
                    } else {
                        ctx.copy_text(text);
                    }
                    self.clear_scratchpad_draft();
                    self.scratchpad.open = false;
                    ctx.request_repaint();
                    self.push_toast(s.copied_btn.to_string(), ToastKind::Success);
                }
            }
            ScratchpadAction::SendToDaily => {
                let raw = self.scratchpad.text.clone();
                if raw.trim().is_empty() {
                    return;
                }
                let today = today_local();
                let nested_rel = daily_note_rel_path(today);
                let legacy_rel = daily_legacy_rel_path(today);
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

                self.watcher.mark_internal_change();
                let res: Result<String, String> = (|| {
                    let (rel_path, mut doc) = if let Some(rel) = existing_rel {
                        let (doc, _, _) = read_note(&self.config.vault_dir, &rel)?;
                        let _ = ensure_daily_hubs_for_date(
                            &self.config.vault_dir,
                            today,
                            lang,
                            &doc.frontmatter.title,
                        );
                        (rel, doc)
                    } else {
                        let created = create_daily_note_with_template(
                            &self.config.vault_dir,
                            today,
                            lang,
                            Some(&self.config.daily_template),
                        )?;
                        for (hp, hdoc, hm, hsz) in created.hubs {
                            let _ = self.index.upsert_note(&hp, &hdoc, hm, hsz);
                        }
                        (created.day_path, created.day_doc)
                    };

                    let hh_mm = Local::now().format("%H:%M").to_string();
                    doc.body = append_timestamped_entry(&doc.body, &hh_mm, &raw);
                    let (modified, size) = save_note(&self.config.vault_dir, &rel_path, &doc)?;
                    let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                    Ok(rel_path)
                })();

                match res {
                    Ok(rel_path) => {
                        self.reload_index_caches();
                        if self.selected_path.as_deref() == Some(rel_path.as_str()) {
                            self.dirty = false;
                            self.open_note(&rel_path);
                        }
                        self.clear_scratchpad_draft();
                        self.scratchpad.open = false;
                        ctx.request_repaint();
                        self.push_toast(s.scratchpad_sent_daily_toast.to_string(), ToastKind::Success);
                    }
                    Err(err) => {
                        self.push_toast(err, ToastKind::Error);
                    }
                }
            }
            ScratchpadAction::SaveAsNote => {
                let raw = self.scratchpad.text.clone();
                if raw.trim().is_empty() {
                    return;
                }
                let fallback = match lang {
                    Lang::Ru => "Быстрая заметка",
                    Lang::En => "Quick Note",
                };
                let (title, body) = split_scratchpad_note(&raw, fallback);
                let existing: HashSet<String> =
                    self.notes.iter().map(|n| n.path.clone()).collect();
                self.watcher.mark_internal_change();
                match create_note_with_tags(
                    &self.config.vault_dir,
                    &title,
                    None,
                    Vec::new(),
                    &body,
                    &existing,
                ) {
                    Ok((rel, doc, modified, size)) => {
                        let _ = self.index.upsert_note(&rel, &doc, modified, size);
                        self.reload_index_caches();
                        self.open_note(&rel);
                        self.view_mode = ViewMode::Editor;
                        self.clear_scratchpad_draft();
                        self.scratchpad.open = false;
                        ctx.request_repaint();
                        self.push_toast(s.scratchpad_saved_note_toast.to_string(), ToastKind::Success);
                    }
                    Err(err) => {
                        self.push_toast(err, ToastKind::Error);
                    }
                }
            }
        }
    }

    pub(super) fn generate_weekly_review(&mut self) {
        let lang = self.config.language;
        let s = strings(lang);
        let today = today_local();
        let start = today.add_days(-6);

        let mut inputs: Vec<DailyReviewInput> = Vec::new();
        for note in &self.notes {
            let Some(d) = parse_daily_note_path(&note.path) else {
                continue;
            };
            if d < start || d > today {
                continue;
            }
            if let Ok((doc, _, _)) = read_note(&self.config.vault_dir, &note.path) {
                inputs.push(DailyReviewInput {
                    date: d,
                    rel_path: note.path.clone(),
                    title: if doc.frontmatter.title.trim().is_empty() {
                        note.title.clone()
                    } else {
                        doc.frontmatter.title
                    },
                    tags: doc.frontmatter.tags,
                    body: doc.body,
                });
            }
        }

        let result = build_weekly_review(today, &inputs, lang == Lang::Ru);
        let mut fm = Frontmatter::new(result.title.clone());
        fm.tags = result.tags;
        let doc = NoteDocument {
            frontmatter: fm,
            body: result.body,
        };

        self.watcher.mark_internal_change();
        match save_note(&self.config.vault_dir, &result.rel_path, &doc) {
            Ok((modified, size)) => {
                let _ = self.index.upsert_note(&result.rel_path, &doc, modified, size);
                self.reload_index_caches();
                self.open_note(&result.rel_path);
                self.view_mode = ViewMode::Editor;
                self.push_toast(s.weekly_review_created_toast.to_string(), ToastKind::Success);
            }
            Err(err) => {
                self.push_toast(err, ToastKind::Error);
            }
        }
    }
}
