use super::{today_local, ImagePickTarget, KronoApp};
use crate::config::open_in_file_manager;
use crate::i18n::{strings, Lang};
use crate::sync::secrets::{delete_github_token, load_github_token, save_github_token};
use crate::ui::{
    components::ToastKind,
    markdown_preview::{hash_str, toggle_markdown_task},
    CreateItemKind, NavTarget, SettingsTab, UiAction, ViewMode,
};
use eframe::egui;
use krono_core::{
    markdown::{extract_embedded_boards, extract_headings},
    path::VaultFileKind,
};
use std::time::Instant;
use zeroize::Zeroize;

impl KronoApp {
    pub(super) fn handle_actions(&mut self, ctx: &egui::Context, actions: Vec<UiAction>) {
        for act in actions {
            match act {
                UiAction::OpenNote(path) => {
                    match VaultFileKind::from_path(&path) {
                        Some(VaultFileKind::Board | VaultFileKind::Table) => {
                            self.open_collection(&path);
                        }
                        Some(VaultFileKind::Canvas) => {
                            self.open_canvas(&path);
                        }
                        _ => {
                            self.open_note(&path);
                            if self.view_mode != ViewMode::Editor {
                                self.view_anim_at = Some(Instant::now());
                            }
                            self.view_mode = ViewMode::Editor;
                            self.previous_view_mode = ViewMode::Editor;
                            self.last_notes_tab_mode = ViewMode::Editor;
                        }
                    }
                    self.mobile_showing_list = false;
                    self.last_notes_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::CreateNote {
                    title,
                    status,
                    tags,
                } => {
                    self.create_and_open_note_with_tags(&title, status, tags, "");
                    self.focus_title_next = true;
                    self.mobile_showing_list = false;
                    self.last_notes_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                    self.last_notes_tab_mode = ViewMode::Editor;
                }
                UiAction::OpenOrCreateWikilink(target) => {
                    self.mobile_showing_list = false;
                    self.drawer_inspector_open = false;
                    if let Some(path) = self.find_note_by_wikilink(&target) {
                        self.open_note(&path);
                        self.view_mode = ViewMode::Editor;
                    } else {
                        self.create_and_open_note(&target, None, "");
                        let msg = match self.config.language {
                            Lang::Ru => format!("Создана заметка «{target}»"),
                            Lang::En => format!("Created note \"{target}\""),
                        };
                        self.push_toast(msg, ToastKind::Info);
                    }
                }
                UiAction::SaveCurrentNote => self.save_current_note_inner(true),
                UiAction::CommitCurrentNoteTitle => self.save_current_note_inner(true),
                UiAction::UpdateNoteStatus { path, status } => {
                    self.update_note_status(&path, status);
                }
                UiAction::MoveNoteInBoard {
                    path,
                    status,
                    insert_before,
                } => {
                    self.move_note_in_board(&path, status, insert_before.as_deref());
                }
                UiAction::ShiftNoteInBoard { path, delta } => {
                    self.shift_note_in_board(&path, delta);
                }
                UiAction::NavBack => {
                    if self.can_nav_back() {
                        self.nav_cursor -= 1;
                        if let Some(tgt) = self.nav_history.get(self.nav_cursor).cloned() {
                            self.navigate_to_target(tgt, false);
                        }
                    }
                }
                UiAction::NavForward => {
                    if self.can_nav_forward() {
                        self.nav_cursor += 1;
                        if let Some(tgt) = self.nav_history.get(self.nav_cursor).cloned() {
                            self.navigate_to_target(tgt, false);
                        }
                    }
                }
                UiAction::OpenNavTarget(target) => {
                    self.navigate_to_target(target, true);
                }
                UiAction::CloseRecentTab(target) => {
                    self.recent_tabs.retain(|t| t != &target);
                }
                UiAction::OpenCreateModalInFolder(folder) => {
                    self.create_modal.open_with(CreateItemKind::Note);
                    self.create_modal.folder = folder;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::CreateInFolder { folder, kind } => {
                    if !folder.is_empty() {
                        self.expand_parents(&format!("{folder}/_"));
                    }
                    self.create_modal.open_with(kind);
                    self.create_modal.folder = folder;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::StartCreateSubfolder(parent) => {
                    self.create_subfolder_parent = Some(parent);
                    self.create_subfolder_draft.clear();
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::SendTaskToBoard {
                    task_idx,
                    board_path,
                    move_task,
                } => {
                    self.handle_send_task_to_board(task_idx, &board_path, move_task);
                }
                UiAction::RefreshAssetAudit => {
                    self.refresh_asset_audit();
                }
                UiAction::DeleteUnusedAssets => {
                    self.delete_unused_assets();
                }
                UiAction::DeleteSingleAsset(rel) => {
                    self.delete_single_asset(&rel);
                }
                UiAction::StartRename(path) => {
                    let current_title = self
                        .notes
                        .iter()
                        .find(|n| n.path == path)
                        .map(|n| n.title.clone())
                        .or_else(|| {
                            self.collections
                                .iter()
                                .find(|c| c.path == path)
                                .map(|c| c.title.clone())
                        })
                        .unwrap_or_else(|| path.clone());
                    self.rename_draft = current_title;
                    self.rename_target = Some(path);
                }
                UiAction::DuplicateNote(path) => self.perform_duplicate(&path),
                UiAction::RequestDelete(path) => self.delete_target = Some(path),
                UiAction::SetViewMode(mode) => {
                    if self.view_mode != mode {
                        self.view_anim_at = Some(Instant::now());
                    }
                    if mode != ViewMode::Settings {
                        self.previous_view_mode = mode;
                    }
                    if matches!(mode, ViewMode::Editor | ViewMode::Collection | ViewMode::Canvas) {
                        self.last_notes_tab_mode = mode;
                    }
                    self.view_mode = mode;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    if matches!(
                        mode,
                        ViewMode::Table | ViewMode::Board | ViewMode::Graph | ViewMode::Overview
                    ) {
                        self.record_nav(NavTarget::View(mode));
                    }
                }
                UiAction::ReturnToNotesTab => {
                    let target = match self.last_notes_tab_mode {
                        ViewMode::Canvas if self.active_canvas.path.is_some() => ViewMode::Canvas,
                        ViewMode::Collection if self.active_collection.path.is_some() => {
                            ViewMode::Collection
                        }
                        _ => ViewMode::Editor,
                    };
                    if self.view_mode != target {
                        self.view_anim_at = Some(Instant::now());
                    }
                    self.view_mode = target;
                    self.previous_view_mode = target;
                    self.mobile_showing_list = self.last_notes_showing_list;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::SetEditorMode(mode) => {
                    self.editor_mode = mode;
                    self.note_anim_at = Some(Instant::now());
                }
                UiAction::MobileShowNotesList => {
                    self.view_mode = ViewMode::Editor;
                    self.previous_view_mode = ViewMode::Editor;
                    self.mobile_showing_list = true;
                    self.last_notes_tab_mode = ViewMode::Editor;
                    self.last_notes_showing_list = true;
                    self.drawer_inspector_open = false;
                }
                UiAction::OpenCreateModal(opt_kind) => {
                    self.create_modal.open_with(opt_kind.unwrap_or(CreateItemKind::Note));
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                }
                UiAction::CreateFromModal {
                    kind,
                    title,
                    folder,
                    preset_id,
                } => {
                    self.create_from_modal(kind, &title, &folder, &preset_id);
                }
                UiAction::ToggleSidebar => {
                    if self.last_layout_class.is_desktop() {
                        self.config.sidebar_open = !self.config.sidebar_open;
                        let _ = self.config.save();
                    } else {
                        self.drawer_sidebar_open = !self.drawer_sidebar_open;
                        if self.drawer_sidebar_open {
                            self.drawer_inspector_open = false;
                        }
                    }
                }
                UiAction::ToggleInspector => {
                    if self.last_layout_class.is_desktop() {
                        self.config.inspector_open = !self.config.inspector_open;
                        let _ = self.config.save();
                    } else {
                        self.drawer_inspector_open = !self.drawer_inspector_open;
                        if self.drawer_inspector_open {
                            self.drawer_sidebar_open = false;
                        }
                    }
                }
                UiAction::ToggleTheme => {
                    self.config.toggle_theme_polarity();
                    self.set_target_palette(ctx, false);
                    let _ = self.config.save();
                }
                UiAction::ToggleLang => {
                    self.config.language = self.config.language.toggle();
                    let _ = self.config.save();
                }
                UiAction::OpenPalette => {
                    self.palette_open = true;
                    self.palette_query.clear();
                    self.palette_selected = 0;
                    self.refresh_palette_hits();
                }
                UiAction::OpenSettings(tab) => {
                    if self.view_mode != ViewMode::Settings {
                        self.previous_view_mode = self.view_mode;
                        self.view_anim_at = Some(Instant::now());
                    }
                    if self.settings_tab != tab {
                        self.settings_tab_anim_at = Some(Instant::now());
                    }
                    self.view_mode = ViewMode::Settings;
                    self.settings_tab = tab;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    self.vault_draft = self.config.vault_dir.to_string_lossy().to_string();
                    if tab == SettingsTab::Storage {
                        self.refresh_asset_audit();
                    }
                }
                UiAction::CloseSettings => {
                    if self.view_mode == ViewMode::Settings {
                        self.view_anim_at = Some(Instant::now());
                        self.view_mode = if self.previous_view_mode == ViewMode::Settings {
                            ViewMode::Editor
                        } else {
                            self.previous_view_mode
                        };
                    }
                }
                UiAction::SyncNow => self.trigger_sync(ctx),
                UiAction::TestGitHubConnection => {
                    let _ = self.config.save();
                    match load_github_token() {
                        Ok(Some(token)) => {
                            self.sync_worker.start_test(
                                self.config.github_repo.clone(),
                                token,
                                self.config.language,
                                ctx.clone(),
                            );
                        }
                        Ok(None) => {
                            self.has_saved_github_token = false;
                            let s = strings(self.config.language);
                            self.test_feedback = Some((false, s.gh_token_missing_toast.to_string()));
                        }
                        Err(e) => {
                            self.test_feedback = Some((false, e));
                        }
                    }
                }
                UiAction::SaveGitHubToken => {
                    let trimmed = self.token_draft.trim().to_string();
                    if !trimmed.is_empty() {
                        match save_github_token(&trimmed) {
                            Ok(()) => {
                                self.token_draft.zeroize();
                                self.token_draft.clear();
                                self.has_saved_github_token = true;
                                self.token_replace_mode = false;
                                self.test_feedback = None;
                                self.reload_index_caches();
                                let s = strings(self.config.language);
                                self.push_toast(s.gh_token_saved_toast, ToastKind::Success);
                            }
                            Err(e) => {
                                self.push_toast(e, ToastKind::Error);
                            }
                        }
                    }
                }
                UiAction::DeleteGitHubToken => {
                    match delete_github_token() {
                        Ok(()) => {
                            self.token_draft.zeroize();
                            self.token_draft.clear();
                            self.has_saved_github_token = false;
                            self.token_replace_mode = false;
                            self.test_feedback = None;
                            self.reload_index_caches();
                            let s = strings(self.config.language);
                            self.push_toast(s.gh_token_deleted_toast, ToastKind::Info);
                        }
                        Err(e) => {
                            self.push_toast(e, ToastKind::Error);
                        }
                    }
                }
                UiAction::SetTagFilter(tag) => self.active_tag_filter = tag,
                UiAction::ToggleFolder(folder) => {
                    if !self.expanded_folders.remove(&folder) {
                        self.expanded_folders.insert(folder);
                    }
                }
                UiAction::MarkDirty => {
                    self.dirty = true;
                    self.last_edit_at = Some(Instant::now());
                    self.cached_headings = extract_headings(&self.current_doc.body);
                    let refs = extract_embedded_boards(&self.current_doc.body);
                    if refs.len() != self.embedded_boards.len() {
                        self.refresh_embedded_boards();
                    }
                }
                UiAction::ToggleTask(task_idx) => {
                    if let Some(updated) = toggle_markdown_task(&self.current_doc.body, task_idx) {
                        self.current_doc.body = updated;
                        self.dirty = true;
                        self.last_edit_at = Some(Instant::now());
                        self.save_current_note();
                    }
                }
                UiAction::CopyText(text) => {
                    self.copied_code_hash = Some(hash_str(&text));
                    self.copied_code_at = Some(Instant::now());
                    ctx.output_mut(|o| o.copied_text = text);
                }
                UiAction::ApplyVaultDir(dir) => self.switch_vault(&dir),
                UiAction::OpenVaultFolder => {
                    if let Err(e) = open_in_file_manager(&self.config.vault_dir) {
                        self.push_toast(e, ToastKind::Error);
                    }
                }
                UiAction::RebuildIndex => {
                    match self.index.rebuild_from_disk(&self.config.vault_dir) {
                        Ok(()) => {
                            self.reload_index_caches();
                            self.refresh_document_derivations();
                            self.refresh_embedded_boards();
                            let s = strings(self.config.language);
                            self.push_toast(s.toast_index_rebuilt, ToastKind::Success);
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
                UiAction::ConfigChanged => {
                    self.set_target_palette(ctx, false);
                    let _ = self.config.save();
                }
                UiAction::TogglePin(path) => {
                    self.toggle_pin(&path);
                }
                UiAction::OpenOrCreateDailyNote(date) => {
                    self.open_or_create_daily_note(date);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                }
                UiAction::OpenTodayNote => {
                    let today = today_local();
                    self.calendar_year = today.year;
                    self.calendar_month = today.month;
                    self.open_or_create_daily_note(today);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                    self.view_mode = ViewMode::Editor;
                }
                UiAction::ToggleCalendar => {
                    self.config.calendar_open = !self.config.calendar_open;
                    let _ = self.config.save();
                }
                UiAction::ExternalReloadFromDisk => {
                    if let Some(path) = self.selected_path.clone() {
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        self.open_note(&path);
                    }
                }
                UiAction::ExternalKeepMine => {
                    self.external_conflict_open_note = false;
                    self.save_current_note();
                }
                UiAction::ExternalSaveConflictCopy => {
                    self.save_external_conflict_copy();
                }
                UiAction::OpenCollection(path) => {
                    self.open_collection(&path);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::CreateCollection { title, kind } => {
                    self.create_and_open_collection(&title, kind);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::SetCollectionView(vkind) => {
                    self.active_collection.active_view = vkind;
                }
                UiAction::SaveCurrentCollection => {
                    self.save_current_collection();
                }
                UiAction::MarkCollectionDirty => {
                    self.active_collection.dirty = true;
                    self.active_collection.last_edit_at = Some(Instant::now());
                }
                UiAction::CollectionExternalReload => {
                    if let Some(p) = self.active_collection.path.clone() {
                        self.active_collection.dirty = false;
                        self.active_collection.external_conflict = false;
                        self.open_collection(&p);
                    }
                }
                UiAction::CollectionExternalKeepMine => {
                    self.active_collection.external_conflict = false;
                    self.save_current_collection();
                }
                UiAction::CollectionExternalSaveConflictCopy => {
                    self.save_collection_external_conflict_copy();
                }
                UiAction::OpenCanvas(path) => {
                    self.open_canvas(&path);
                    self.mobile_showing_list = false;
                    self.drawer_sidebar_open = false;
                }
                UiAction::SaveCurrentCanvas => {
                    self.save_current_canvas();
                }
                UiAction::MarkCanvasDirty => {
                    self.active_canvas.dirty = true;
                    self.active_canvas.last_edit_at = Some(Instant::now());
                }
                UiAction::CanvasExternalReload => {
                    if let Some(p) = self.active_canvas.path.clone() {
                        self.active_canvas.dirty = false;
                        self.active_canvas.external_conflict = false;
                        self.open_canvas(&p);
                    }
                }
                UiAction::CanvasExternalKeepMine => {
                    self.active_canvas.external_conflict = false;
                    self.save_current_canvas();
                }
                UiAction::CanvasExternalSaveConflictCopy => {
                    self.save_canvas_external_conflict_copy();
                }
                UiAction::ExportCanvasToMarkdown => {
                    self.export_active_canvas_to_markdown();
                }
                UiAction::OpenGraphForNode(path) => {
                    self.graph_state
                        .sync_if_needed(&self.notes, &self.collections, &self.all_links);
                    self.graph_state.focus_node(&path, ctx.screen_rect());
                    if self.view_mode != ViewMode::Graph {
                        self.view_anim_at = Some(Instant::now());
                    }
                    self.view_mode = ViewMode::Graph;
                    self.previous_view_mode = ViewMode::Graph;
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                    self.mobile_showing_list = false;
                }
                UiAction::PickImageForEditor => {
                    self.spawn_image_picker(ctx, ImagePickTarget::Editor);
                }
                UiAction::PickImageForCanvas => {
                    self.spawn_image_picker(ctx, ImagePickTarget::Canvas);
                }
                UiAction::PasteClipboardImage => {
                    self.try_paste_image_from_clipboard(ctx, true);
                }
                UiAction::OpenImageLightbox(path) => {
                    self.lightbox_image = Some(path);
                }
                UiAction::CreateLinkedBoardForCurrentNote => {
                    self.create_linked_board_for_current_note();
                }
                UiAction::EmbeddedBoard(eb) => {
                    self.handle_embedded_board_action(eb);
                }
                UiAction::CheckForUpdates => {
                    self.update_status_msg = None;
                    self.update_worker.start_check(
                        self.config.include_prerelease_updates,
                        false,
                        self.config.language,
                        ctx.clone(),
                    );
                }
                UiAction::InstallUpdate => {
                    if let Some(upd) = self.available_update.clone() {
                        self.flush_unsaved_documents();
                        self.update_worker.start_install(
                            upd,
                            self.config.language,
                            ctx.clone(),
                        );
                    }
                }
            }
        }
    }

}
