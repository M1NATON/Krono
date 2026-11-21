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
    SidebarModulesState, SyncUiState, TopbarDocContext, UiAction, ViewMode,
};
use eframe::egui::{self, Margin};
use krono_core::path::VaultFileKind;
use std::time::Instant;

impl KronoApp {
    pub(super) fn render_workspace_frame(&mut self, ctx: &egui::Context) {
        self.poll_tmdb_events();
        self.spellchecker.poll_ready();
        let titlebar_sub = match self.view_mode {
            ViewMode::Editor => self.current_doc.frontmatter.title.clone(),
            ViewMode::Collection => self.active_collection.doc.title.clone(),
            ViewMode::Canvas => self.active_canvas.doc.title.clone(),
            _ => self
                .config
                .vault_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
        };
        crate::ui::titlebar::render_custom_titlebar(
            ctx,
            &self.palette,
            self.config.language,
            &titlebar_sub,
            self.config.custom_titlebar,
        );

        let embedded_notes = if self.view_mode == ViewMode::Editor
            && self.editor_mode != crate::ui::EditorMode::Edit
        {
            self.collect_embedded_notes_for_body(&self.current_doc.body.clone())
        } else if self.view_mode == ViewMode::Canvas {
            let md_paths: Vec<String> = self
                .active_canvas
                .doc
                .nodes
                .iter()
                .filter(|n| n.kind == krono_core::canvas::CanvasNodeKind::File)
                .filter_map(|n| n.file.clone())
                .filter(|fp| {
                    krono_core::path::VaultFileKind::from_path(fp)
                        == Some(krono_core::path::VaultFileKind::Markdown)
                })
                .collect();
            self.collect_embedded_notes_for_paths(&md_paths)
        } else {
            std::collections::HashMap::new()
        };
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

        let mut actions = Vec::new();

        // 1. Desktop Left Sidebar (smoothly animated width)
        if !self.zen_mode && layout.is_desktop() && desktop_sidebar_t > 0.02 {
            let mut sb_w = self.config.sidebar_width;
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
                tree_tab: &mut self.sidebar_tree_tab,
                show_system_folders: &mut self.config.sidebar_show_system_folders,
            };
            let mut cal_ui = CalendarUiState {
                open: self.config.calendar_open,
                year: &mut self.calendar_year,
                month: &mut self.calendar_month,
                today,
                daily_dates: &self.daily_dates,
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
                &self.config.pinned_folders,
                &self.config.folder_order,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
                due_tasks_badge,
                SidebarModulesState {
                    show_block: self.config.show_sidebar_modules,
                    show_media_library: self.config.show_module_media_library,
                    show_english: self.config.show_module_english,
                    study_language: self.config.study_language.clone(),
                    nav_tasks: self.config.nav_show_tasks,
                    nav_graph: self.config.nav_show_graph,
                    nav_overview: self.config.nav_show_overview,
                    custom_titlebar: self.config.custom_titlebar,
                },
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
        if !self.zen_mode {
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
                lang,
                layout,
                &recent_items,
                self.view_mode,
            ));
        }

        let prev_lg_depth = self.local_graph_depth;

        // 4. Desktop Right Inspector (smoothly animated panel, hidden by default)
        if !self.zen_mode && layout.is_desktop() && desktop_inspector_t > 0.02 {
            let current_full_md = self.current_doc.serialize();
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
                &current_full_md,
                &mut self.history,
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
            spellcheck: self.config.editor_spellcheck,
            md_show_toolbar: self.config.md_show_toolbar && !self.zen_mode,
            md_slash_trigger: self.config.md_slash_trigger,
            zen_mode: self.zen_mode,
            pane_salt: "",
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

        if self.zen_mode {
            let (words, _) = krono_core::markdown::count_words(&self.current_doc.body);
            let goal = self.config.zen_daily_word_goal;
            egui::TopBottomPanel::bottom("krono_zen_footer")
                .show_separator_line(false)
                .exact_height(32.0)
                .frame(
                    egui::Frame::none()
                        .fill(palette.bg_canvas)
                        .inner_margin(Margin::symmetric(20.0, 6.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let w_label = if lang == crate::i18n::Lang::Ru {
                            format!("{words} слов")
                        } else {
                            format!("{words} words")
                        };
                        ui.label(
                            egui::RichText::new(w_label)
                                .font(crate::ui::theme::font_mono(11.5))
                                .color(palette.text_muted),
                        );
                        if goal > 0 {
                            ui.add_space(12.0);
                            let ratio = (words as f32 / goal as f32).clamp(0.0, 1.0);
                            let pct = ((words as f32 / goal as f32) * 100.0).round() as usize;
                            let (bar_r, _) =
                                ui.allocate_exact_size(egui::vec2(120.0, 6.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                bar_r,
                                egui::Rounding::same(3.0),
                                palette.bg_hover,
                            );
                            if ratio > 0.0 {
                                let fill_r = egui::Rect::from_min_size(
                                    bar_r.min,
                                    egui::vec2((bar_r.width() * ratio).max(4.0), bar_r.height()),
                                );
                                let bar_col = if words >= goal {
                                    palette.success
                                } else {
                                    palette.accent
                                };
                                ui.painter().rect_filled(
                                    fill_r,
                                    egui::Rounding::same(3.0),
                                    bar_col,
                                );
                            }
                            ui.add_space(8.0);
                            let s_i18n = crate::i18n::strings(lang);
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {words} / {goal} ({pct}%)",
                                    s_i18n.zen_goal_label
                                ))
                                .font(crate::ui::theme::font_mono(11.0))
                                .color(if words >= goal {
                                    palette.success
                                } else {
                                    palette.text_muted
                                }),
                            );
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let s_i18n = crate::i18n::strings(lang);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(s_i18n.zen_exit_btn)
                                            .font(crate::ui::theme::font_medium(11.5))
                                            .color(palette.text_secondary),
                                    )
                                    .fill(palette.bg_elevated)
                                    .rounding(egui::Rounding::same(6.0)),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                actions.push(UiAction::ToggleZenMode);
                            }
                        });
                    });
                });
        }

        let right_embedded_notes = match self.split.right {
            Some(super::split::SplitContent::Note(ref sn))
                if sn.editor_mode != crate::ui::EditorMode::Edit =>
            {
                self.collect_embedded_notes_for_body(&sn.doc.body.clone())
            }
            _ => std::collections::HashMap::new(),
        };

        let mut right_pane_actions: Vec<UiAction> = Vec::new();
        let split_active = !self.zen_mode
            && !layout.is_mobile()
            && self.split.right.is_some()
            && matches!(self.view_mode, ViewMode::Editor | ViewMode::Collection);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette.bg_canvas)
                    .inner_margin(if split_active {
                        Margin::same(6.0)
                    } else {
                        inner_pad
                    }),
            )
            .show(ctx, |ui| {
                let mut left_rect_opt: Option<egui::Rect> = None;
                let mut right_rect_opt: Option<egui::Rect> = None;
                if split_active {
                    let avail = ui.available_rect_before_wrap();
                    let ratio = self.split.ratio.clamp(0.25, 0.75);
                    let sep_w = 6.0;
                    let left_w = ((avail.width() - sep_w) * ratio).max(200.0);
                    let l_rect = egui::Rect::from_min_size(
                        avail.min,
                        egui::vec2(left_w, avail.height()),
                    );
                    let s_rect = egui::Rect::from_min_size(
                        egui::pos2(l_rect.right(), avail.top()),
                        egui::vec2(sep_w, avail.height()),
                    );
                    let r_rect = egui::Rect::from_min_max(
                        egui::pos2(s_rect.right(), avail.top()),
                        avail.max,
                    );

                    let sep_resp = ui
                        .interact(
                            s_rect,
                            egui::Id::new("krono_split_divider"),
                            egui::Sense::drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                    if sep_resp.dragged() {
                        if let Some(pos) = ctx.pointer_interact_pos() {
                            let usable = (avail.width() - sep_w).max(1.0);
                            self.split.ratio =
                                ((pos.x - avail.left()) / usable).clamp(0.25, 0.75);
                        }
                    }
                    let line_col = if sep_resp.hovered() || sep_resp.dragged() {
                        palette.accent
                    } else {
                        palette.border_hairline
                    };
                    let line_w = if sep_resp.hovered() || sep_resp.dragged() {
                        2.0_f32
                    } else {
                        1.0_f32
                    };
                    ui.painter().line_segment(
                        [
                            egui::pos2(s_rect.center().x, s_rect.top()),
                            egui::pos2(s_rect.center().x, s_rect.bottom()),
                        ],
                        egui::Stroke::new(line_w, line_col),
                    );

                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        if let Some(pos) = ctx.pointer_interact_pos() {
                            if l_rect.contains(pos) {
                                self.split.active_pane = super::split::SplitPane::Left;
                            } else if r_rect.contains(pos) {
                                self.split.active_pane = super::split::SplitPane::Right;
                            }
                        }
                    }
                    left_rect_opt = Some(l_rect);
                    right_rect_opt = Some(r_rect);
                }

                let is_left_active = self.split.active_pane == super::split::SplitPane::Left;
                let mut render_left_pane = |ui: &mut egui::Ui| match self.view_mode {
                ViewMode::Editor => {
                    if layout.is_mobile() && self.mobile_showing_list {
                        let mut pages_ui = PagesTreeUiState {
                            filter: &mut self.pages_filter,
                            sort: &mut self.pages_sort,
                            filter_open: &mut self.pages_filter_open,
                            tree_tab: &mut self.sidebar_tree_tab,
                            show_system_folders: &mut self.config.sidebar_show_system_folders,
                        };
                        let mut cal_ui = CalendarUiState {
                            open: self.config.calendar_open,
                            year: &mut self.calendar_year,
                            month: &mut self.calendar_month,
                            today,
                            daily_dates: &self.daily_dates,
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
                    } else if self.selected_path.is_none() {
                        let recent = self.build_empty_recent_entries();
                        actions.extend(crate::ui::empty_state::render_empty_state(
                            ui, &palette, lang, &recent,
                        ));
                    } else if self.is_current_note_locked() {
                        if let Some(ref lp) = self.selected_path {
                            crate::ui::vault_modals::render_locked_note_overlay(
                                ui,
                                &palette,
                                lang,
                                lp,
                                &mut actions,
                            );
                        }
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
                            &available_boards,
                            &embedded_notes,
                            &mut self.spellchecker,
                            today,
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
                    if self.active_collection.path.is_none() {
                        let recent = self.build_empty_recent_entries();
                        actions.extend(crate::ui::empty_state::render_empty_state(
                            ui, &palette, lang, &recent,
                        ));
                    } else {
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
                            &mut self.cover_cache,
                            self.has_saved_tmdb_token,
                        ));
                    }
                }
                ViewMode::Canvas => {
                    if self.active_canvas.path.is_none() {
                        let recent = self.build_empty_recent_entries();
                        actions.extend(crate::ui::empty_state::render_empty_state(
                            ui, &palette, lang, &recent,
                        ));
                    } else {
                        let canvas_md_ctx = crate::ui::canvas_view::CanvasMarkdownCtx {
                            today,
                            heading_serif: self.config.heading_serif,
                            resolved_titles: &self.resolved_titles,
                            copied_code_hash: self.copied_code_hash,
                            embedded_boards: &self.embedded_boards,
                            available_boards: &available_boards,
                            embedded_notes: &embedded_notes,
                        };
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
                            &canvas_md_ctx,
                        ));
                    }
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
                        self.has_saved_tmdb_token,
                        &mut self.tmdb_token_replace_mode,
                        &mut self.tmdb_token_draft,
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
            };

            if let (Some(l_rect), Some(r_rect)) = (left_rect_opt, right_rect_opt) {
                let mut left_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(l_rect.shrink(4.0))
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                left_ui.set_clip_rect(l_rect);
                render_left_pane(&mut left_ui);
                drop(render_left_pane);

                // Pane frame highlight (Left)
                let l_stroke = if is_left_active {
                    egui::Stroke::new(
                        1.0_f32,
                        crate::ui::motion::with_alpha(palette.accent, 0.45),
                    )
                } else {
                    egui::Stroke::new(1.0_f32, palette.border_hairline)
                };
                ui.painter()
                    .rect_stroke(l_rect, egui::Rounding::same(6.0), l_stroke);
                if is_left_active {
                    ui.painter().line_segment(
                        [
                            egui::pos2(l_rect.left() + 6.0, l_rect.top() + 1.0),
                            egui::pos2(l_rect.right() - 6.0, l_rect.top() + 1.0),
                        ],
                        egui::Stroke::new(2.0_f32, palette.accent),
                    );
                }

                // Right pane
                let right_prefs = EditorPrefs {
                    pane_salt: "right",
                    ..prefs
                };
                let mut right_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(r_rect.shrink(4.0))
                        .layout(egui::Layout::top_down(egui::Align::LEFT)),
                );
                right_ui.set_clip_rect(r_rect);
                let s_i18n = crate::i18n::strings(lang);
                right_ui.push_id("krono_split_right_pane", |ui| {
                    if let Some(ref mut right_content) = self.split.right {
                        // Slim 28px right pane header
                        ui.horizontal(|ui| {
                            ui.set_height(26.0);
                            let (r_icon, r_title, r_dirty) = match right_content {
                                super::split::SplitContent::Note(n) => (
                                    crate::ui::icons::Icon::Document,
                                    n.doc.frontmatter.title.clone(),
                                    n.dirty,
                                ),
                                super::split::SplitContent::Collection(c) => (
                                    match c.active_view {
                                        krono_core::collection::CollectionViewKind::Board => {
                                            crate::ui::icons::Icon::Board
                                        }
                                        krono_core::collection::CollectionViewKind::Table => {
                                            crate::ui::icons::Icon::Table
                                        }
                                        krono_core::collection::CollectionViewKind::Gallery => {
                                            crate::ui::icons::Icon::Image
                                        }
                                    },
                                    c.doc.title.clone(),
                                    c.dirty,
                                ),
                            };
                            let (ir, _) = ui.allocate_exact_size(
                                egui::vec2(14.0, 14.0),
                                egui::Sense::hover(),
                            );
                            crate::ui::icons::draw_icon(
                                ui.painter(),
                                ir,
                                r_icon,
                                palette.accent_text,
                            );
                            let show_title = if r_title.trim().is_empty() {
                                s_i18n.untitled_hint.to_string()
                            } else {
                                r_title.trim().to_string()
                            };
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(show_title)
                                        .font(crate::ui::theme::font_medium(12.5))
                                        .color(palette.text_primary),
                                )
                                .truncate(),
                            );
                            if r_dirty {
                                crate::ui::components::paint_badge_dot(
                                    ui.painter(),
                                    ui.cursor().left_center() + egui::vec2(4.0, 0.0),
                                    palette.warning,
                                );
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if crate::ui::components::icon_button(
                                        ui,
                                        &palette,
                                        crate::ui::icons::Icon::Close,
                                        s_i18n.split_close_btn,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        right_pane_actions.push(UiAction::CloseSplit);
                                    }
                                    if let super::split::SplitContent::Note(ref mut sn) =
                                        right_content
                                    {
                                        for (m, lbl) in [
                                            (crate::ui::EditorMode::Read, s_i18n.mode_read),
                                            (crate::ui::EditorMode::Edit, s_i18n.mode_edit),
                                        ] {
                                            let active = sn.editor_mode == m;
                                            if ui
                                                .selectable_label(
                                                    active,
                                                    egui::RichText::new(lbl)
                                                        .font(crate::ui::theme::font_medium(11.0)),
                                                )
                                                .clicked()
                                            {
                                                sn.editor_mode = m;
                                            }
                                        }
                                    }
                                },
                            );
                        });
                        crate::ui::components::hairline_h(ui, &palette);
                        ui.add_space(4.0);

                        match right_content {
                            super::split::SplitContent::Note(ref mut sn) => {
                                let doc_state = EditorDocState {
                                    selected_path: Some(sn.path.as_str()),
                                    title: &mut sn.doc.frontmatter.title,
                                    status: &mut sn.doc.frontmatter.status,
                                    tags: &mut sn.doc.frontmatter.tags,
                                    pinned: &mut sn.doc.frontmatter.pinned,
                                    extra_lines: &mut sn.doc.frontmatter.extra_lines,
                                    body: &mut sn.doc.body,
                                    modified: sn.modified,
                                    external_conflict: false,
                                };
                                right_pane_actions.extend(render_editor_view(
                                    ui,
                                    &palette,
                                    lang,
                                    LayoutClass::Compact,
                                    doc_state,
                                    &mut sn.new_tag_input,
                                    &mut sn.adding_tag,
                                    sn.editor_mode,
                                    right_prefs,
                                    &self.config.custom_slash_snippets,
                                    &self.notes,
                                    &mut sn.focus_title_next,
                                    &mut sn.focus_body_next,
                                    1.0,
                                    &self.resolved_titles,
                                    self.copied_code_hash,
                                    &self.config.vault_dir,
                                    &mut self.image_cache,
                                    &self.embedded_boards,
                                    &available_boards,
                                    &right_embedded_notes,
                                    &mut self.spellchecker,
                                    today,
                                ));
                            }
                            super::split::SplitContent::Collection(ref mut sc) => {
                                right_pane_actions.extend(render_collection_view(
                                    ctx,
                                    ui,
                                    &palette,
                                    lang,
                                    LayoutClass::Compact,
                                    today,
                                    sc,
                                    &self.notes,
                                    &mut self.cover_cache,
                                    self.has_saved_tmdb_token,
                                ));
                            }
                        }
                    }
                });

                // Pane frame highlight (Right)
                let r_stroke = if !is_left_active {
                    egui::Stroke::new(
                        1.0_f32,
                        crate::ui::motion::with_alpha(palette.accent, 0.45),
                    )
                } else {
                    egui::Stroke::new(1.0_f32, palette.border_hairline)
                };
                ui.painter()
                    .rect_stroke(r_rect, egui::Rounding::same(6.0), r_stroke);
                if !is_left_active {
                    ui.painter().line_segment(
                        [
                            egui::pos2(r_rect.left() + 6.0, r_rect.top() + 1.0),
                            egui::pos2(r_rect.right() - 6.0, r_rect.top() + 1.0),
                        ],
                        egui::Stroke::new(2.0_f32, palette.accent),
                    );
                }
            } else {
                render_left_pane(ui);
            }
        });

        if !right_pane_actions.is_empty() {
            self.handle_right_pane_actions(ctx, right_pane_actions);
        }

        // 6. Overlay Drawers for Compact & Mobile viewports (AFTER CentralPanel)
        if layout == LayoutClass::Compact && drawer_sidebar_t > 0.02 {
            let mut pages_ui = PagesTreeUiState {
                filter: &mut self.pages_filter,
                sort: &mut self.pages_sort,
                filter_open: &mut self.pages_filter_open,
                tree_tab: &mut self.sidebar_tree_tab,
                show_system_folders: &mut self.config.sidebar_show_system_folders,
            };
            let mut cal_ui = CalendarUiState {
                open: self.config.calendar_open,
                year: &mut self.calendar_year,
                month: &mut self.calendar_month,
                today,
                daily_dates: &self.daily_dates,
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
                &self.config.pinned_folders,
                &self.config.folder_order,
                self.view_mode,
                &mut pages_ui,
                &mut cal_ui,
                sync_ui,
                due_tasks_badge,
                SidebarModulesState {
                    show_block: self.config.show_sidebar_modules,
                    show_media_library: self.config.show_module_media_library,
                    show_english: self.config.show_module_english,
                    study_language: self.config.study_language.clone(),
                    nav_tasks: self.config.nav_show_tasks,
                    nav_graph: self.config.nav_show_graph,
                    nav_overview: self.config.nav_show_overview,
                    custom_titlebar: self.config.custom_titlebar,
                },
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
            let current_full_md = self.current_doc.serialize();
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
                &current_full_md,
                &mut self.history,
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

        // 6d. Stage 5.1 Bulk Modals (Global Replace & Tag Rename)
        match crate::ui::bulk_modals::render_global_replace_modal(
            ctx,
            &palette,
            lang,
            &mut self.global_replace,
        ) {
            crate::ui::bulk_modals::BulkModalOutcome::Rescan => self.scan_global_replace(),
            crate::ui::bulk_modals::BulkModalOutcome::Apply => self.apply_global_replace(),
            crate::ui::bulk_modals::BulkModalOutcome::None => {}
        }
        let existing_tags: Vec<String> = self.tag_counts.iter().map(|(t, _)| t.clone()).collect();
        match crate::ui::bulk_modals::render_tag_rename_modal(
            ctx,
            &palette,
            lang,
            &mut self.tag_rename,
            &existing_tags,
        ) {
            crate::ui::bulk_modals::BulkModalOutcome::Rescan => self.scan_tag_rename(),
            crate::ui::bulk_modals::BulkModalOutcome::Apply => self.apply_tag_rename(),
            crate::ui::bulk_modals::BulkModalOutcome::None => {}
        }

        if crate::ui::vault_modals::render_vault_crypto_modal(
            ctx,
            &palette,
            lang,
            &mut self.vault_crypto,
        ) {
            self.submit_crypto_modal();
        }

        // Compact Vault Status / Clipboard Countdown Pill
        let clip_secs = self.vault_crypto.clipboard_seconds_left();
        if self.vault_crypto.is_unlocked() || clip_secs.is_some() {
            egui::Area::new(egui::Id::new("vault_status_pill"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -14.0))
                .show(ctx, |ui| {
                    egui::Frame::none()
                        .fill(palette.bg_elevated)
                        .stroke(egui::Stroke::new(1.0_f32, palette.border_subtle))
                        .rounding(egui::Rounding::same(8.0))
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                if let Some(secs) = clip_secs {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}: {secs}s",
                                            strings(lang).clipboard_clears_in
                                        ))
                                        .font(crate::ui::theme::font_mono(11.5))
                                        .color(palette.warning),
                                    );
                                }
                                if self.vault_crypto.is_unlocked() {
                                    if crate::ui::components::secondary_button(
                                        ui,
                                        &palette,
                                        Some(crate::ui::icons::Icon::Lock),
                                        strings(lang).vault_lock_now_btn,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::LockVaultNow);
                                    }
                                }
                            });
                        });
                });
        }

        // 7. Animated Toasts
        render_toasts(ctx, &palette, &mut self.toasts);

        if !actions.is_empty() {
            self.handle_actions(ctx, actions);
        }
    }
}
