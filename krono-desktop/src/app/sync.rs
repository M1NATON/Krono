use super::KronoApp;
use crate::i18n::{strings, Lang};
use crate::storage::vault::{
    import_image_file_to_assets, read_canvas, read_collection, read_note,
};
use crate::sync::{
    secrets::load_github_token,
    updater::{UpdateApplyOutcome, UpdateEvent},
    worker::SyncEvent,
};
use crate::ui::{
    components::ToastKind, image_cache::image_dimensions_from_bytes, SettingsTab, ViewMode,
};
use eframe::egui;
use std::sync::mpsc;

impl KronoApp {
    pub(super) fn trigger_sync(&mut self, ctx: &egui::Context) {
        self.flush_unsaved_documents();
        self.last_local_save_at = None;
        if !self.config.is_github_configured(self.has_saved_github_token) {
            if self.view_mode != ViewMode::Settings {
                self.previous_view_mode = self.view_mode;
            }
            self.view_mode = ViewMode::Settings;
            self.settings_tab = SettingsTab::GitHub;
            let s = strings(self.config.language);
            self.push_toast(s.sync_not_configured, ToastKind::Info);
            return;
        }
        let token = match load_github_token() {
            Ok(Some(t)) => t,
            Ok(None) => {
                self.has_saved_github_token = false;
                let s = strings(self.config.language);
                self.push_toast(s.gh_token_missing_toast, ToastKind::Info);
                self.view_mode = ViewMode::Settings;
                self.settings_tab = SettingsTab::GitHub;
                return;
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
                return;
            }
        };
        let base_hashes = self.index.load_sync_state().unwrap_or_default();
        self.sync_worker.start_sync(
            self.config.vault_dir.clone(),
            base_hashes,
            self.config.github_repo.clone(),
            self.config.github_branch.clone(),
            token,
            self.config.language,
            ctx.clone(),
        );
    }


    pub(super) fn poll_background_events(&mut self, ctx: &egui::Context) {
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
                                if doc != self.current_doc {
                                    self.current_doc = doc;
                                }
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

}
