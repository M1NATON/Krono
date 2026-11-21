use super::{today_local, KronoApp};
use crate::i18n::{format_relative_time, strings};
use crate::storage::vault::{create_vault_folder, list_custom_templates};
use crate::ui::{
    board_view::render_board_view,
    canvas_view::render_canvas_view,
    collection_view::render_collection_view,
    command_palette::render_command_palette,
    components::{render_toasts, ToastKind},
    dialogs::{
        render_create_item_modal, render_create_subfolder_modal, render_delete_modal,
        render_rename_modal,
    },
    editor_view::render_editor_view,
    graph_view::render_graph_view,
    image_cache::render_image_lightbox,
    inspector::{render_inspector, render_inspector_drawer},
    mobile_nav::{render_mobile_bottom_bar, render_mobile_notes_list},
    motion::{anim_bool_eased, anim_since},
    scratchpad::render_scratchpad_modal,
    settings_view::{render_settings_view, UpdateUiState},
    sidebar::{render_sidebar, render_sidebar_drawer},
    table_view::render_table_view,
    topbar::{render_recent_tabs_bar, render_topbar},
    vault_map_view::render_vault_map_view,
    CalendarUiState, EditorDocState, EditorPrefs, LayoutClass, PagesTreeUiState, SettingsTab,
    SyncUiState, TopbarDocContext, UiAction, ViewMode,
};
use eframe::egui::{self, Margin};
use krono_core::path::VaultFileKind;
use std::time::Instant;

impl KronoApp {
    pub(super) fn render_workspace_frame(&mut self, ctx: &egui::Context) {
        let layout = LayoutClass::from_width(ctx.screen_rect().width());
        if layout != self.last_layout_class {
            self.drawer_sidebar_open = false;
            self.drawer_inspector_open = false;
            self.last_layout_class = layout;
        }

        let desktop_sidebar_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_sidebar_anim"),
            layout.is_desktop() && self.config.sidebar_open,
            0.18,
        );
        let drawer_sidebar_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_sidebar_drawer_anim"),
            layout == LayoutClass::Compact && self.drawer_sidebar_open,
            0.18,
        );
        let desktop_inspector_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_inspector_anim"),
            layout.is_desktop() && self.view_mode == ViewMode::Editor && self.config.inspector_open,
            0.18,
        );
        let drawer_inspector_t = anim_bool_eased(
            ctx,
            egui::Id::new("krono_inspector_drawer_anim"),
            !layout.is_desktop()
                && self.view_mode == ViewMode::Editor
                && self.drawer_inspector_open,
            0.18,
        );

        let palette = self.palette;
        let lang = self.config.language;
        let s = strings(lang);
        let sync_busy = self.sync_worker.is_busy();
        let sync_configured = self.config.is_github_configured(self.has_saved_github_token);
        let sync_status_text = if sync_busy {
            s.syncing_label.to_string()
        } else if !sync_configured {
            s.sync_not_configured.to_string()
        } else if let Some(ts) = self.last_sync_ts {
            format_relative_time(lang, ts)
        } else {
            s.synced_label.to_string()
        };

        let sync_ui = SyncUiState {
            busy: sync_busy,
            angle: self.sync_angle,
            configured: sync_configured,
            pending_changes: self.pending_sync_changes,
            status_text: &sync_status_text,
        };
        let can_back = self.can_nav_back();
        let can_forward = self.can_nav_forward();
        let recent_items = self.build_recent_tab_items();
        let vault_dir_snap = self.config.vault_dir.clone();
        let vault_counts = self.vault_counts();
        let today = today_local();
        let due_tasks_badge = self.due_tasks_badge_count(today);

        let mut cal_ui = CalendarUiState {
            open: self.config.calendar_open,
            year: &mut self.calendar_year,
            month: &mut self.calendar_month,
            today,
            daily_dates: &self.daily_dates,
        };

        let mut actions = Vec::new();

        // 1. Desktop Left Sidebar (smoothly animated width)
        if layout.is_desktop() && desktop_sidebar_t > 0.02 {
            let mut sb_w = self.config.sidebar_width;
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
            };
            actions.extend(render_sidebar(
                ctx,
                &palette,
                lang,
                &mut sb_w,
                desktop_sidebar_t,
                &self.notes,
                &self.collections,
                &self.vault_folders,
                &self.tag_counts,
                self.selected_path.as_deref(),
                self.active_collection.path.as_deref(),
                self.active_canvas.path.as_deref(),
                self.active_tag_filter.as_deref(),
                &self.expanded_folders,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
                due_tasks_badge,
            ));
            if desktop_sidebar_t > 0.99 && (sb_w - self.config.sidebar_width).abs() > 1.0 {
                self.config.sidebar_width = sb_w;
            }
        }

        // 2. Mobile Bottom Tab Bar (declared BEFORE CentralPanel so it reserves bottom inset)
        if layout.is_mobile() {
            actions.extend(render_mobile_bottom_bar(
                ctx,
                &palette,
                lang,
                self.view_mode,
                self.mobile_showing_list,
            ));
        }

        // 3. Top Bar
        let eff_sidebar_open = if layout.is_desktop() {
            self.config.sidebar_open
        } else {
            self.drawer_sidebar_open
        };
        let eff_inspector_open = if layout.is_desktop() {
            self.config.inspector_open
        } else {
            self.drawer_inspector_open
        };
        let topbar_doc = TopbarDocContext {
            selected_note_path: self.selected_path.as_deref(),
            note_title: &self.current_doc.frontmatter.title,
            note_dirty: self.dirty,
            active_collection_path: self.active_collection.path.as_deref(),
            collection_title: &self.active_collection.doc.title,
            collection_dirty: self.active_collection.dirty,
            collection_view: self.active_collection.active_view,
            active_canvas_path: self.active_canvas.path.as_deref(),
            canvas_title: &self.active_canvas.doc.title,
            canvas_dirty: self.active_canvas.dirty,
        };
        actions.extend(render_topbar(
            ctx,
            &palette,
            lang,
            layout,
            self.mobile_showing_list,
            eff_sidebar_open,
            eff_inspector_open,
            self.view_mode,
            &mut self.editor_mode,
            topbar_doc,
            sync_ui,
            can_back,
            can_forward,
        ));

        actions.extend(render_recent_tabs_bar(
            ctx,
            &palette,
            layout,
            &recent_items,
            self.view_mode,
        ));

        let prev_lg_depth = self.local_graph_depth;

        // 4. Desktop Right Inspector (smoothly animated panel, hidden by default)
        if layout.is_desktop() && desktop_inspector_t > 0.02 {
            actions.extend(render_inspector(
                ctx,
                &palette,
                lang,
                desktop_inspector_t,
                self.selected_path.as_deref(),
                &self.current_doc.body,
                &self.cached_headings,
                &self.cached_backlinks,
                &self.cached_outgoing,
                &self.local_graph_data,
                &mut self.local_graph_depth,
            ));
        }

        // 5. Central Panel
        let inner_pad = match layout {
            LayoutClass::Mobile => match self.view_mode {
                ViewMode::Editor if self.mobile_showing_list => Margin::symmetric(12.0, 10.0),
                ViewMode::Editor => Margin::symmetric(14.0, 8.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Board | ViewMode::Collection => Margin {
                    left: 12.0,
                    right: 12.0,
                    top: 10.0,
                    bottom: 4.0,
                },
                ViewMode::Table | ViewMode::Tasks => Margin::symmetric(12.0, 10.0),
                ViewMode::Settings => Margin::symmetric(10.0, 10.0),
            },
            LayoutClass::Compact => match self.view_mode {
                ViewMode::Editor => Margin::symmetric(16.0, 12.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Board | ViewMode::Collection => Margin {
                    left: 16.0,
                    right: 16.0,
                    top: 14.0,
                    bottom: 6.0,
                },
                ViewMode::Table | ViewMode::Tasks => Margin::same(16.0),
                ViewMode::Settings => Margin::symmetric(16.0, 14.0),
            },
            LayoutClass::Desktop => match self.view_mode {
                ViewMode::Editor => Margin::symmetric(20.0, 14.0),
                ViewMode::Graph | ViewMode::Canvas | ViewMode::Overview => Margin::same(0.0),
                ViewMode::Board | ViewMode::Collection => Margin {
                    left: 22.0,
                    right: 22.0,
                    top: 18.0,
                    bottom: 6.0,
                },
                ViewMode::Table | ViewMode::Tasks => Margin::same(22.0),
                ViewMode::Settings => Margin::symmetric(24.0, 20.0),
            },
        };
        let prefs = EditorPrefs {
            font_size: self.config.editor_font_size,
            heading_serif: self.config.heading_serif,
            max_width: self.config.editor_width.max_px(),
            show_footer: self.config.show_editor_footer,
            md_slash_menu: self.config.md_slash_menu,
            md_wikilink_autocomplete: self.config.md_wikilink_autocomplete,
            md_auto_lists: self.config.md_auto_lists,
            md_auto_pairs: self.config.md_auto_pairs,
            md_show_toolbar: self.config.md_show_toolbar,
            md_slash_trigger: self.config.md_slash_trigger,
        };
        let last_sync_info = self.last_sync_ts.map(|ts| {
            let rel = format_relative_time(lang, ts);
            match &self.last_sync_commit {
                Some(sha) if sha.len() >= 7 => format!("{rel} ({})", &sha[..7]),
                _ => rel,
            }
        });

        let view_t = anim_since(ctx, self.view_anim_at, 0.15);
        let note_t = anim_since(ctx, self.note_anim_at, 0.15);
        let tab_anim_t = anim_since(ctx, self.settings_tab_anim_at, 0.15);

        let today = today_local();
        let available_boards: Vec<(String, String)> = self
            .collections
            .iter()
            .filter(|c| c.kind == VaultFileKind::Board)
            .map(|c| (c.path.clone(), c.title.clone()))
            .collect();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette.bg_canvas)
                    .inner_margin(inner_pad),
            )
            .show(ctx, |ui| match self.view_mode {
                ViewMode::Editor => {
                    if layout.is_mobile() && self.mobile_showing_list {
                        let mut pages_ui = PagesTreeUiState {
                            filter: &mut self.pages_filter,
                            sort: &mut self.pages_sort,
                            filter_open: &mut self.pages_filter_open,
                        };
                        actions.extend(render_mobile_notes_list(
                            ctx,
                            ui,
                            &palette,
                            lang,
                            &self.notes,
                            &self.collections,
                            &self.vault_folders,
                            &self.tag_counts,
                            self.selected_path.as_deref(),
                            self.active_collection.path.as_deref(),
                            self.active_canvas.path.as_deref(),
                            self.active_tag_filter.as_deref(),
                            &self.expanded_folders,
                            &mut pages_ui,
                            &mut cal_ui,
                        ));
                    } else {
                        let doc_state = EditorDocState {
                            selected_path: self.selected_path.as_deref(),
                            title: &mut self.current_doc.frontmatter.title,
                            status: &mut self.current_doc.frontmatter.status,
                            tags: &mut self.current_doc.frontmatter.tags,
                            pinned: &mut self.current_doc.frontmatter.pinned,
                            extra_lines: &mut self.current_doc.frontmatter.extra_lines,
                            body: &mut self.current_doc.body,
                            modified: self.current_modified,
                            external_conflict: self.external_conflict_open_note,
                        };
                        actions.extend(render_editor_view(
                            ui,
                            &palette,
                            lang,
                            layout,
                            doc_state,
                            &mut self.new_tag_input,
                            &mut self.adding_tag,
                            self.editor_mode,
                            prefs,
                            &self.config.custom_slash_snippets,
                            &self.notes,
                            &mut self.focus_title_next,
                            &mut self.focus_body_next,
                            note_t.min(view_t),
                            &self.resolved_titles,
                            self.copied_code_hash,
                            &self.config.vault_dir,
                            &mut self.image_cache,
                            &self.embedded_boards,
                            today,
                            &available_boards,
                        ));
                    }
                }
                ViewMode::Tasks => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(crate::ui::tasks_view::render_tasks_view(
                        ui,
                        &palette,
                        lang,
                        layout,
                        today,
                        &self.vault_tasks,
                        &mut self.tasks_view_state,
                    ));
                }
                ViewMode::Table => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(render_table_view(
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.notes,
                        &mut self.table_filter_query,
                        &mut self.table_status_filter,
                        self.active_tag_filter.as_deref(),
                        &mut self.table_sort,
                        &mut self.table_col_widths,
                    ));
                }
                ViewMode::Board => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    let mut board_tags: Vec<String> =
                        self.tag_counts.iter().map(|(t, _)| t.clone()).collect();
                    board_tags.sort_by_key(|a| a.to_lowercase());
                    actions.extend(render_board_view(
                        ui,
                        &palette,
                        lang,
                        layout,
                        &mut self.mobile_board_tab,
                        &self.notes,
                        self.active_tag_filter.as_deref(),
                        &mut self.quick_add_board_status,
                        &mut self.quick_add_board_title,
                        &mut self.quick_add_board_tags,
                        &mut self.quick_add_board_tag_input,
                        &board_tags,
                        &mut self.board_sort,
                    ));
                }
                ViewMode::Graph => {
                    actions.extend(render_graph_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.notes,
                        &self.collections,
                        &self.all_links,
                        &mut self.graph_state,
                    ));
                }
                ViewMode::Overview => {
                    actions.extend(render_vault_map_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &self.config.vault_dir,
                        &mut self.vault_map,
                        &self.notes,
                        &self.collections,
                        &self.all_links,
                    ));
                }
                ViewMode::Collection => {
                    if view_t < 0.99 {
                        ui.add_space((1.0 - view_t) * 6.0);
                    }
                    actions.extend(render_collection_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        today,
                        &mut self.active_collection,
                        &self.notes,
                    ));
                }
                ViewMode::Canvas => {
                    actions.extend(render_canvas_view(
                        ctx,
                        ui,
                        &palette,
                        lang,
                        layout,
                        &mut self.active_canvas,
                        &self.notes,
                        &self.collections,
                        &self.config.vault_dir,
                        &mut self.image_cache,
                    ));
                }
                ViewMode::Settings => {
                    let prev_tab = self.settings_tab;
                    actions.extend(render_settings_view(
                        ui,
                        &palette,
                        &mut self.config,
                        &mut self.settings_tab,
                        tab_anim_t,
                        &mut self.vault_draft,
                        self.has_saved_github_token,
                        &mut self.token_replace_mode,
                        &mut self.token_draft,
                        &self.test_feedback,
                        last_sync_info.as_deref(),
                        sync_busy,
                        vault_counts,
                        &mut self.snippet_draft_trigger,
                        &mut self.snippet_draft_label,
                        &mut self.snippet_draft_content,
                        &self.asset_audit,
                        &mut self.asset_audit_only_unused,
                        &vault_dir_snap,
                        &mut self.image_cache,
                        UpdateUiState {
                            checking: self.update_worker.is_checking(),
                            installing: self.update_worker.is_installing(),
                            available: self.available_update.as_ref(),
                            status_msg: self.update_status_msg.as_deref(),
                        },
                    ));
                    if self.settings_tab != prev_tab {
                        self.settings_tab_anim_at = Some(Instant::now());
                        if self.settings_tab == SettingsTab::Storage {
                            actions.push(UiAction::RefreshAssetAudit);
                        }
                    }
                }
            });

        // 6. Overlay Drawers for Compact & Mobile viewports (AFTER CentralPanel)
        if layout == LayoutClass::Compact && drawer_sidebar_t > 0.02 {
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
            };
            let (d_actions, dismissed) = render_sidebar_drawer(
                ctx,
                &palette,
                lang,
                drawer_sidebar_t,
                &self.notes,
                &self.collections,
                &self.vault_folders,
                &self.tag_counts,
                self.selected_path.as_deref(),
                self.active_collection.path.as_deref(),
                self.active_canvas.path.as_deref(),
                self.active_tag_filter.as_deref(),
                &self.expanded_folders,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
                due_tasks_badge,
            );
            actions.extend(d_actions);
            if dismissed {
                self.drawer_sidebar_open = false;
            }
        }

        if !layout.is_desktop()
            && self.view_mode == ViewMode::Editor
            && drawer_inspector_t > 0.02
        {
            let (i_actions, dismissed) = render_inspector_drawer(
                ctx,
                &palette,
                lang,
                drawer_inspector_t,
                self.selected_path.as_deref(),
                &self.current_doc.body,
                &self.cached_headings,
                &self.cached_backlinks,
                &self.cached_outgoing,
                &self.local_graph_data,
                &mut self.local_graph_depth,
            );
            actions.extend(i_actions);
            if dismissed {
                self.drawer_inspector_open = false;
            }
        }

        if self.local_graph_depth != prev_lg_depth {
            self.refresh_document_derivations();
        }

        // 5. Command Palette Modal
        let (q_changed, pal_actions) = render_command_palette(
            ctx,
            &palette,
            lang,
            &mut self.palette_open,
            &mut self.palette_query,
            &mut self.palette_selected,
            &self.palette_hits,
        );
        if q_changed {
            self.refresh_palette_hits();
        }
        actions.extend(pal_actions);

        // 6. Action Dialogs (Create Modal, Rename & Delete)
        let folders_for_modal = if self.create_modal.open {
            self.existing_folders_list()
        } else {
            Vec::new()
        };
        let custom_templates = if self.create_modal.open {
            list_custom_templates(&self.config.vault_dir)
        } else {
            Vec::new()
        };
        if let Some(act) = render_create_item_modal(
            ctx,
            &palette,
            lang,
            &mut self.create_modal,
            &folders_for_modal,
            &custom_templates,
        ) {
            actions.push(act);
        }

        if let Some((parent, name)) = render_create_subfolder_modal(
            ctx,
            &palette,
            lang,
            &mut self.create_subfolder_parent,
            &mut self.create_subfolder_draft,
        ) {
            self.watcher.mark_internal_change();
            match create_vault_folder(&self.config.vault_dir, &parent, &name) {
                Ok(rel) => {
                    self.expand_parents(&format!("{rel}/_"));
                    self.reload_index_caches();
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
        }

        if let Some((old_path, new_title)) = render_rename_modal(
            ctx,
            &palette,
            lang,
            &mut self.rename_target,
            &mut self.rename_draft,
        ) {
            self.perform_rename(&old_path, &new_title);
        }

        if let Some(del_path) =
            render_delete_modal(ctx, &palette, lang, &mut self.delete_target)
        {
            self.perform_delete(&del_path);
        }

        // 6b. Fullscreen Image Lightbox
        render_image_lightbox(
            ctx,
            &palette,
            lang,
            &self.config.vault_dir,
            &mut self.image_cache,
            &mut self.lightbox_image,
        );

        // 6c. Quick Capture Scratchpad
        if let Some(sp_action) =
            render_scratchpad_modal(ctx, &palette, lang, &mut self.scratchpad)
        {
            self.handle_scratchpad_action(ctx, sp_action);
        }

        // 7. Animated Toasts
        render_toasts(ctx, &palette, &mut self.toasts);

        if !actions.is_empty() {
            self.handle_actions(ctx, actions);
        }
    }
}
