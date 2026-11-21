use crate::i18n::{strings, Lang};
use crate::ui::components::{
    compact_sync_icon_button, context_menu_row, draw_outer_bottom_border, ghost_sync_button,
    hairline_h, icon_button, paint_badge_dot, segmented_control,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_body, font_medium, ThemePalette};
use crate::ui::{
    EditorMode, LayoutClass, NavTarget, RecentTabItem, SettingsTab, SyncUiState, TopbarDocContext,
    UiAction, ViewMode,
};
use eframe::egui::{self, pos2, vec2, Align, CursorIcon, Layout, Margin, Rect, Rounding, Sense};
use krono_core::{collection::CollectionViewKind, path::VaultFileKind};

#[allow(clippy::too_many_arguments)]
pub fn render_topbar(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    mobile_showing_list: bool,
    sidebar_open: bool,
    inspector_open: bool,
    view_mode: ViewMode,
    editor_mode: &mut EditorMode,
    doc: TopbarDocContext<'_>,
    sync: SyncUiState<'_>,
    can_back: bool,
    can_forward: bool,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    let is_mobile = layout.is_mobile();
    let height = if is_mobile { 46.0 } else { 42.0 };
    let h_margin = if is_mobile { 10.0 } else { 16.0 };

    let panel_resp = egui::TopBottomPanel::top("krono_topbar")
        .show_separator_line(false)
        .exact_height(height)
        .frame(
            egui::Frame::none()
                .fill(p.bg_canvas)
                .inner_margin(Margin::symmetric(h_margin, 6.0)),
        )
        .show(ctx, |ui| {
            if is_mobile {
                render_mobile_topbar(
                    ctx,
                    ui,
                    p,
                    lang,
                    s,
                    mobile_showing_list,
                    inspector_open,
                    view_mode,
                    editor_mode,
                    doc,
                    sync,
                    &mut actions,
                );
                return;
            }

            let screen_w = ctx.screen_rect().width();
            let compact = layout == LayoutClass::Compact;
            let show_sidebar_btn = !sidebar_open || compact;
            let show_folder_crumb = !compact && screen_w >= 760.0;
            let active_dirty = match view_mode {
                ViewMode::Collection => doc.collection_dirty && doc.active_collection_path.is_some(),
                ViewMode::Canvas => doc.canvas_dirty && doc.active_canvas_path.is_some(),
                ViewMode::Editor => doc.note_dirty && doc.selected_note_path.is_some(),
                _ => false,
            };
            let show_unsaved_pill = active_dirty && screen_w >= 760.0;

            let right_reserve = match view_mode {
                ViewMode::Editor => {
                    let base = 28.0 + 110.0 + 24.0 + 64.0;
                    let save_w = if active_dirty { 82.0 } else { 0.0 };
                    let mode_w = if doc.selected_note_path.is_some() {
                        if layout.is_desktop() { 228.0 } else { 148.0 }
                    } else {
                        0.0
                    };
                    base + save_w + mode_w
                }
                ViewMode::Collection => {
                    let base = 110.0 + 24.0 + 32.0;
                    let save_w = if active_dirty { 82.0 } else { 0.0 };
                    base + save_w + 158.0
                }
                ViewMode::Canvas => {
                    let base = 110.0 + 24.0;
                    let save_w = if active_dirty { 82.0 } else { 0.0 };
                    base + save_w
                }
                _ => 126.0,
            };
            let left_max_w = (ui.available_width() - right_reserve).max(90.0);

            ui.horizontal_centered(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(left_max_w, 30.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.set_max_width(left_max_w);
                        if show_sidebar_btn {
                            if icon_button(
                                ui,
                                p,
                                Icon::SidebarToggle,
                                s.tooltip_expand_sidebar,
                                false,
                            )
                            .clicked()
                            {
                                actions.push(UiAction::ToggleSidebar);
                            }
                            ui.add_space(4.0);
                        }

                        if !compact {
                            if nav_arrow_button(
                                ui,
                                p,
                                Icon::ChevronLeft,
                                s.nav_back_tip,
                                can_back,
                            )
                            .clicked()
                            {
                                actions.push(UiAction::NavBack);
                            }
                            if nav_arrow_button(
                                ui,
                                p,
                                Icon::ChevronRight,
                                s.nav_forward_tip,
                                can_forward,
                            )
                            .clicked()
                            {
                                actions.push(UiAction::NavForward);
                            }
                            ui.add_space(6.0);
                        }

                        match view_mode {
                            ViewMode::Editor => {
                                if let Some(path) = doc.selected_note_path {
                                    if show_folder_crumb {
                                        render_folder_crumb(ui, p, path);
                                    }
                                    let title_show = if doc.note_title.trim().is_empty() {
                                        s.untitled_hint
                                    } else {
                                        doc.note_title.trim()
                                    };
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(title_show)
                                                .font(font_medium(13.0))
                                                .color(p.text_primary),
                                        )
                                        .truncate(),
                                    );
                                    if show_unsaved_pill {
                                        ui.add_space(4.0);
                                        render_unsaved_pill(ui, p, s.unsaved_label);
                                    }
                                } else {
                                    ui.label(
                                        egui::RichText::new(s.nav_notes)
                                            .font(font_medium(13.0))
                                            .color(p.text_secondary),
                                    );
                                }
                            }
                            ViewMode::Collection => {
                                if let Some(path) = doc.active_collection_path {
                                    if show_folder_crumb {
                                        render_folder_crumb(ui, p, path);
                                    }
                                    let cicon = match doc.collection_view {
                                        CollectionViewKind::Board => Icon::Board,
                                        CollectionViewKind::Table => Icon::Table,
                                        CollectionViewKind::Gallery => Icon::Image,
                                    };
                                    let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    draw_icon(ui.painter(), ir, cicon, p.accent_text);
                                    ui.add_space(4.0);
                                    let title_show = if doc.collection_title.trim().is_empty() {
                                        s.untitled_hint
                                    } else {
                                        doc.collection_title.trim()
                                    };
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(title_show)
                                                .font(font_medium(13.0))
                                                .color(p.text_primary),
                                        )
                                        .truncate(),
                                    );
                                    if show_unsaved_pill {
                                        ui.add_space(4.0);
                                        render_unsaved_pill(ui, p, s.unsaved_label);
                                    }
                                }
                            }
                            ViewMode::Table => {
                                ui.label(
                                    egui::RichText::new(s.table_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                            ViewMode::Board => {
                                ui.label(
                                    egui::RichText::new(s.board_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                            ViewMode::Graph => {
                                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                draw_icon(ui.painter(), ir, Icon::Graph, p.accent_text);
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(s.graph_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                            ViewMode::Overview => {
                                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                draw_icon(ui.painter(), ir, Icon::LayoutGrid, p.accent_text);
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(s.overview_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                            ViewMode::Tasks => {
                                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                draw_icon(ui.painter(), ir, Icon::Task, p.accent_text);
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(s.tasks_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                            ViewMode::Canvas => {
                                if let Some(path) = doc.active_canvas_path {
                                    if show_folder_crumb {
                                        render_folder_crumb(ui, p, path);
                                    }
                                    let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    draw_icon(ui.painter(), ir, Icon::Canvas, p.accent_text);
                                    ui.add_space(4.0);
                                    let title_show = if doc.canvas_title.trim().is_empty() {
                                        s.untitled_hint
                                    } else {
                                        doc.canvas_title.trim()
                                    };
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(title_show)
                                                .font(font_medium(13.0))
                                                .color(p.text_primary),
                                        )
                                        .truncate(),
                                    );
                                    if show_unsaved_pill {
                                        ui.add_space(4.0);
                                        render_unsaved_pill(ui, p, s.unsaved_label);
                                    }
                                }
                            }
                            ViewMode::Settings => {
                                if back_pill_button(ctx, ui, p, s.back_btn).clicked() {
                                    actions.push(UiAction::CloseSettings);
                                }
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new("/")
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(s.settings_title)
                                        .font(font_medium(13.5))
                                        .color(p.text_primary),
                                );
                            }
                        }
                    },
                );

                // Right: Inspector Toggle + Sync + Save + View Mode control
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if view_mode == ViewMode::Editor {
                        if icon_button(
                            ui,
                            p,
                            Icon::InspectorToggle,
                            s.tooltip_toggle_inspector,
                            inspector_open,
                        )
                        .clicked()
                        {
                            actions.push(UiAction::ToggleInspector);
                        }
                        if icon_button(ui, p, Icon::Maximize, s.action_toggle_zen, false).clicked()
                        {
                            actions.push(UiAction::ToggleZenMode);
                        }
                    }
                    if matches!(view_mode, ViewMode::Editor | ViewMode::Collection) {
                        if icon_button(ui, p, Icon::Split, s.action_toggle_split, false).clicked()
                        {
                            actions.push(UiAction::ToggleSplitRight);
                        }
                    }

                    let sync_label_owned;
                    let sync_label = if sync.busy {
                        s.syncing_label
                    } else if sync.configured && sync.pending_changes > 0 {
                        sync_label_owned = format!("{} · {}", s.sync_btn, sync.pending_changes);
                        &sync_label_owned
                    } else {
                        s.sync_btn
                    };
                    if ghost_sync_button(
                        ui,
                        p,
                        sync_label,
                        sync.busy,
                        sync.angle,
                        sync.configured,
                    )
                    .clicked()
                    {
                        if sync.configured {
                            actions.push(UiAction::SyncNow);
                        } else {
                            actions.push(UiAction::OpenSettings(SettingsTab::GitHub));
                        }
                    }

                    if active_dirty {
                        if compact_save_button(ui, p, s.save_btn).clicked() {
                            actions.push(match view_mode {
                                ViewMode::Collection => UiAction::SaveCurrentCollection,
                                ViewMode::Canvas => UiAction::SaveCurrentCanvas,
                                _ => UiAction::SaveCurrentNote,
                            });
                        }
                    }

                    if view_mode == ViewMode::Editor && doc.selected_note_path.is_some() {
                        let mut mode = *editor_mode;
                        if !layout.is_desktop() && mode == EditorMode::Split {
                            mode = EditorMode::Edit;
                        }
                        let changed = if layout.is_desktop() {
                            segmented_control(
                                ui,
                                p,
                                "topbar_editor_mode",
                                &mut mode,
                                &[
                                    (EditorMode::Edit, Some(Icon::Edit), s.mode_edit),
                                    (EditorMode::Split, Some(Icon::Split), s.mode_split),
                                    (EditorMode::Read, Some(Icon::Eye), s.mode_read),
                                ],
                            )
                        } else {
                            segmented_control(
                                ui,
                                p,
                                "topbar_editor_mode_compact",
                                &mut mode,
                                &[
                                    (EditorMode::Edit, Some(Icon::Edit), s.mode_edit),
                                    (EditorMode::Read, Some(Icon::Eye), s.mode_read),
                                ],
                            )
                        };
                        if changed {
                            *editor_mode = mode;
                            actions.push(UiAction::SetEditorMode(mode));
                        }
                    } else if view_mode == ViewMode::Collection && doc.active_collection_path.is_some() {
                        let mut cview = doc.collection_view;
                        if segmented_control(
                            ui,
                            p,
                            "topbar_col_view_mode",
                            &mut cview,
                            &[
                                (CollectionViewKind::Board, Some(Icon::Board), s.col_view_board),
                                (CollectionViewKind::Table, Some(Icon::Table), s.col_view_table),
                                (CollectionViewKind::Gallery, Some(Icon::Image), s.col_view_gallery),
                            ],
                        ) {
                            actions.push(UiAction::SetCollectionView(cview));
                        }
                    }
                });
            });
        });

    draw_outer_bottom_border(ctx, panel_resp.response.rect, p);
    actions
}

fn render_folder_crumb(ui: &mut egui::Ui, p: &ThemePalette, path: &str) {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > 1 {
        let folder = parts[..parts.len() - 1].join(" / ");
        ui.label(
            egui::RichText::new(format!("{folder} /"))
                .font(font_body(12.5))
                .color(p.text_muted),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_topbar(
    _ctx: &egui::Context,
    ui: &mut egui::Ui,
    p: &ThemePalette,
    _lang: Lang,
    s: &crate::i18n::Strings,
    mobile_showing_list: bool,
    inspector_open: bool,
    view_mode: ViewMode,
    editor_mode: &mut EditorMode,
    doc: TopbarDocContext<'_>,
    sync: SyncUiState<'_>,
    actions: &mut Vec<UiAction>,
) {
    let sync_tip = if sync.configured {
        if sync.busy {
            s.syncing_label
        } else {
            s.sync_btn
        }
    } else {
        s.settings_tab_github
    };

    let render_mob_sync = |ui: &mut egui::Ui, actions: &mut Vec<UiAction>| {
        let resp = compact_sync_icon_button(ui, p, sync_tip, sync.busy, sync.angle, sync.configured);
        if sync.configured && !sync.busy && sync.pending_changes > 0 {
            paint_badge_dot(ui.painter(), pos2(resp.rect.right() - 4.0, resp.rect.top() + 5.0), p.warning);
        }
        if resp.clicked() {
            if sync.configured {
                actions.push(UiAction::SyncNow);
            } else {
                actions.push(UiAction::OpenSettings(SettingsTab::GitHub));
            }
        }
    };

    let right_reserve = match view_mode {
        ViewMode::Editor if mobile_showing_list => 104.0,
        ViewMode::Editor => {
            let mut r = 68.0;
            if doc.selected_note_path.is_some() {
                r += 66.0;
                if doc.note_dirty {
                    r += 72.0;
                }
            }
            r
        }
        ViewMode::Collection => {
            let mut r = 104.0;
            if doc.collection_dirty {
                r += 72.0;
            }
            r
        }
        ViewMode::Canvas => {
            if doc.canvas_dirty {
                112.0
            } else {
                40.0
            }
        }
        ViewMode::Table | ViewMode::Board => 104.0,
        ViewMode::Tasks | ViewMode::Graph | ViewMode::Overview | ViewMode::Settings => 40.0,
    };
    let left_w = (ui.available_width() - right_reserve).max(64.0);

    ui.horizontal_centered(|ui| {
        ui.allocate_ui_with_layout(
            vec2(left_w, 34.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_max_width(left_w);
                ui.spacing_mut().item_spacing.x = 6.0;
                match view_mode {
                    ViewMode::Editor if mobile_showing_list => {
                        ui.label(
                            egui::RichText::new(s.nav_notes)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Editor => {
                        if icon_button(ui, p, Icon::ArrowLeft, s.back_btn, false).clicked() {
                            actions.push(UiAction::MobileShowNotesList);
                        }
                        let title_show = if doc.selected_note_path.is_some() {
                            if doc.note_title.trim().is_empty() {
                                s.untitled_hint
                            } else {
                                doc.note_title.trim()
                            }
                        } else {
                            s.nav_notes
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title_show)
                                    .font(font_medium(14.5))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                        if doc.note_dirty && doc.selected_note_path.is_some() {
                            let (dot_r, _) = ui.allocate_exact_size(vec2(8.0, 12.0), Sense::hover());
                            paint_badge_dot(ui.painter(), dot_r.center(), p.warning);
                        }
                    }
                    ViewMode::Collection => {
                        if icon_button(ui, p, Icon::ArrowLeft, s.back_btn, false).clicked() {
                            actions.push(UiAction::MobileShowNotesList);
                        }
                        let title_show = if doc.collection_title.trim().is_empty() {
                            s.untitled_hint
                        } else {
                            doc.collection_title.trim()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title_show)
                                    .font(font_medium(14.5))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                        if doc.collection_dirty {
                            let (dot_r, _) = ui.allocate_exact_size(vec2(8.0, 12.0), Sense::hover());
                            paint_badge_dot(ui.painter(), dot_r.center(), p.warning);
                        }
                    }
                    ViewMode::Table => {
                        ui.label(
                            egui::RichText::new(s.table_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Board => {
                        ui.label(
                            egui::RichText::new(s.board_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Graph => {
                        ui.label(
                            egui::RichText::new(s.graph_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Overview => {
                        ui.label(
                            egui::RichText::new(s.overview_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Tasks => {
                        ui.label(
                            egui::RichText::new(s.tasks_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                    ViewMode::Canvas => {
                        if icon_button(ui, p, Icon::ArrowLeft, s.back_btn, false).clicked() {
                            actions.push(UiAction::MobileShowNotesList);
                        }
                        let title_show = if doc.canvas_title.trim().is_empty() {
                            s.untitled_hint
                        } else {
                            doc.canvas_title.trim()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title_show)
                                    .font(font_medium(14.5))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                        if doc.canvas_dirty {
                            let (dot_r, _) = ui.allocate_exact_size(vec2(8.0, 12.0), Sense::hover());
                            paint_badge_dot(ui.painter(), dot_r.center(), p.warning);
                        }
                    }
                    ViewMode::Settings => {
                        if icon_button(ui, p, Icon::ArrowLeft, s.back_btn, false).clicked() {
                            actions.push(UiAction::CloseSettings);
                        }
                        ui.label(
                            egui::RichText::new(s.settings_title)
                                .font(font_medium(15.5))
                                .color(p.text_primary),
                        );
                    }
                }
            },
        );

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            match view_mode {
                ViewMode::Editor if mobile_showing_list => {
                    render_mob_sync(ui, actions);
                    if icon_button(ui, p, Icon::Search, s.tooltip_search, false).clicked() {
                        actions.push(UiAction::OpenPalette);
                    }
                    if icon_button(ui, p, Icon::Plus, s.tooltip_new_note, false).clicked() {
                        actions.push(UiAction::OpenCreateModal(None));
                    }
                }
                ViewMode::Editor => {
                    if icon_button(
                        ui,
                        p,
                        Icon::InspectorToggle,
                        s.tooltip_toggle_inspector,
                        inspector_open,
                    )
                    .clicked()
                    {
                        actions.push(UiAction::ToggleInspector);
                    }
                    render_mob_sync(ui, actions);
                    if doc.note_dirty && doc.selected_note_path.is_some() {
                        if compact_save_button(ui, p, s.save_btn).clicked() {
                            actions.push(UiAction::SaveCurrentNote);
                        }
                    }
                    if doc.selected_note_path.is_some() {
                        let mut mode = *editor_mode;
                        if mode == EditorMode::Split {
                            mode = EditorMode::Edit;
                        }
                        if segmented_control(
                            ui,
                            p,
                            "topbar_mobile_editor_mode",
                            &mut mode,
                            &[
                                (EditorMode::Edit, Some(Icon::Edit), ""),
                                (EditorMode::Read, Some(Icon::Eye), ""),
                            ],
                        ) {
                            *editor_mode = mode;
                            actions.push(UiAction::SetEditorMode(mode));
                        }
                    }
                }
                ViewMode::Collection => {
                    render_mob_sync(ui, actions);
                    if doc.collection_dirty {
                        if compact_save_button(ui, p, s.save_btn).clicked() {
                            actions.push(UiAction::SaveCurrentCollection);
                        }
                    }
                    let mut cview = doc.collection_view;
                    if segmented_control(
                        ui,
                        p,
                        "topbar_mobile_col_view",
                        &mut cview,
                        &[
                            (CollectionViewKind::Board, Some(Icon::Board), ""),
                            (CollectionViewKind::Table, Some(Icon::Table), ""),
                        ],
                    ) {
                        actions.push(UiAction::SetCollectionView(cview));
                    }
                }
                ViewMode::Table | ViewMode::Board => {
                    render_mob_sync(ui, actions);
                    if icon_button(ui, p, Icon::Search, s.tooltip_search, false).clicked() {
                        actions.push(UiAction::OpenPalette);
                    }
                    if icon_button(ui, p, Icon::Plus, s.tooltip_new_note, false).clicked() {
                        actions.push(UiAction::OpenCreateModal(None));
                    }
                }
                ViewMode::Tasks | ViewMode::Graph | ViewMode::Overview | ViewMode::Settings => {
                    render_mob_sync(ui, actions);
                }
                ViewMode::Canvas => {
                    render_mob_sync(ui, actions);
                    if doc.canvas_dirty {
                        if compact_save_button(ui, p, s.save_btn).clicked() {
                            actions.push(UiAction::SaveCurrentCanvas);
                        }
                    }
                }
            }
        });
    });
}

fn render_unsaved_pill(ui: &mut egui::Ui, p: &ThemePalette, label: &str) {
    egui::Frame::none()
        .fill(p.warning_soft)
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(8.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let (dot_r, _) = ui.allocate_exact_size(vec2(7.0, 12.0), Sense::hover());
                paint_badge_dot(ui.painter(), dot_r.center(), p.warning);
                ui.label(
                    egui::RichText::new(label)
                        .font(font_body(11.0))
                        .color(p.warning),
                );
            });
        });
}

fn compact_save_button(ui: &mut egui::Ui, p: &ThemePalette, label: &str) -> egui::Response {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.0), p.accent_text);
    let (brect, bresp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 24.0), Sense::click());
    ui.painter().rect_filled(
        brect,
        Rounding::same(5.0),
        if bresp.hovered() {
            p.accent_hover
        } else {
            p.accent_soft
        },
    );
    ui.painter().galley(
        pos2(
            brect.center().x - g.size().x * 0.5,
            brect.center().y - g.size().y * 0.5,
        ),
        g,
        if bresp.hovered() {
            egui::Color32::WHITE
        } else {
            p.accent_text
        },
    );
    bresp.on_hover_cursor(CursorIcon::PointingHand)
}

fn back_pill_button(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    p: &ThemePalette,
    label: &str,
) -> egui::Response {
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.5), p.text_secondary);
    let (brect, bresp) = ui.allocate_exact_size(vec2(g.size().x + 28.0, 26.0), Sense::click());
    let hov = anim_bool_eased(ctx, bresp.id.with("hov"), bresp.hovered(), 0.10);
    if hov > 0.01 {
        ui.painter()
            .rect_filled(brect, Rounding::same(6.0), with_alpha(p.bg_hover, hov));
    }
    let fg = if bresp.hovered() {
        p.text_primary
    } else {
        p.text_secondary
    };
    draw_icon(
        ui.painter(),
        Rect::from_center_size(pos2(brect.left() + 12.0, brect.center().y), vec2(13.5, 13.5)),
        Icon::ArrowLeft,
        fg,
    );
    ui.painter().galley(
        pos2(brect.left() + 23.0, brect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    bresp.on_hover_cursor(CursorIcon::PointingHand)
}

fn nav_arrow_button(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    icon: Icon,
    tooltip: &str,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), sense);
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, Rounding::same(6.0), p.bg_hover);
    }
    let col = if !enabled {
        with_alpha(p.text_muted, 0.42)
    } else if resp.hovered() {
        p.text_primary
    } else {
        p.text_secondary
    };
    draw_icon(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(13.0, 13.0)),
        icon,
        col,
    );
    let r = if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    };
    r.on_hover_text(tooltip)
}

#[derive(Clone, Debug)]
struct TabDragState {
    src_idx: usize,
    pinned: bool,
    start_pos: egui::Pos2,
    grab_offset_x: f32,
    tab_w: f32,
    target_in_group: usize,
    active: bool,
}

pub fn render_recent_tabs_bar(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    recent_tabs: &[RecentTabItem],
    view_mode: ViewMode,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let has_pinned = recent_tabs.iter().any(|t| t.pinned);
    if layout.is_mobile()
        || view_mode == ViewMode::Settings
        || (recent_tabs.len() < 2 && !has_pinned)
    {
        return actions;
    }

    let s = strings(lang);
    let drag_id = egui::Id::new("krono_tab_drag");
    let mut drag_state = ctx.data(|d| d.get_temp::<TabDragState>(drag_id));
    let mut tab_rects: Vec<(usize, bool, Rect)> = Vec::with_capacity(recent_tabs.len());

    // Precompute smooth horizontal shift for each tab when drag is active
    let active_drag = drag_state.as_ref().filter(|st| st.active).cloned();
    let src_group_pos = active_drag.as_ref().map(|st| {
        recent_tabs
            .iter()
            .take(st.src_idx)
            .filter(|t| t.pinned == st.pinned)
            .count()
    });

    let panel_resp = egui::TopBottomPanel::top("krono_recent_tabs")
        .show_separator_line(false)
        .exact_height(31.0)
        .frame(
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .inner_margin(Margin::symmetric(12.0, 3.0)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("recent_tabs_hscroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        let mut group_idx_counter = [0usize, 0usize]; // [unpinned, pinned]
                        for (idx, tab) in recent_tabs.iter().enumerate() {
                            let g_slot = usize::from(tab.pinned);
                            let my_g_idx = group_idx_counter[g_slot];
                            group_idx_counter[g_slot] += 1;

                            let icon = match tab.kind {
                                VaultFileKind::Board => Icon::Board,
                                VaultFileKind::Table => Icon::Table,
                                VaultFileKind::Canvas => Icon::Canvas,
                                _ => Icon::Document,
                            };
                            let fg = if tab.active { p.text_primary } else { p.text_secondary };
                            let max_chars = if tab.pinned { 14 } else { 24 };
                            let label: String = tab.title.chars().take(max_chars).collect();
                            let title_show = if tab.title.chars().count() > max_chars {
                                format!("{label}…")
                            } else {
                                label
                            };
                            let g = ui.painter().layout_no_wrap(
                                title_show,
                                font_medium(11.5),
                                fg,
                            );
                            let dot_w = if tab.dirty { 9.0 } else { 0.0 };
                            let pin_w = if tab.pinned { 13.0 } else { 0.0 };
                            let right_pad = if tab.pinned { 10.0 } else { 22.0 };
                            let min_w = if tab.pinned { 56.0 } else { 76.0 };
                            let pill_w = (22.0 + pin_w + dot_w + g.size().x + right_pad).max(min_w);
                            let (base_rect, resp) =
                                ui.allocate_exact_size(vec2(pill_w, 23.0), Sense::click_and_drag());
                            tab_rects.push((idx, tab.pinned, base_rect));

                            if resp.drag_started() {
                                if let Some(pos) = ctx.pointer_interact_pos() {
                                    drag_state = Some(TabDragState {
                                        src_idx: idx,
                                        pinned: tab.pinned,
                                        start_pos: pos,
                                        grab_offset_x: (pos.x - base_rect.left()).clamp(8.0, pill_w - 8.0),
                                        tab_w: pill_w,
                                        target_in_group: my_g_idx,
                                        active: false,
                                    });
                                }
                            }

                            let is_dragging_this = active_drag
                                .as_ref()
                                .is_some_and(|st| st.src_idx == idx);

                            // Smooth slot shift animation like board_dnd
                            let target_shift_x = if let (Some(st), Some(src_g)) = (&active_drag, src_group_pos) {
                                if tab.pinned == st.pinned && !is_dragging_this {
                                    let shift_amt = st.tab_w + 5.0;
                                    if st.target_in_group > src_g && my_g_idx > src_g && my_g_idx <= st.target_in_group {
                                        -shift_amt
                                    } else if st.target_in_group < src_g && my_g_idx >= st.target_in_group && my_g_idx < src_g {
                                        shift_amt
                                    } else {
                                        0.0
                                    }
                                } else {
                                    0.0
                                }
                            } else {
                                0.0
                            };
                            let shift_x = crate::ui::motion::anim_value_smooth(
                                ctx,
                                ui.make_persistent_id(("rtab_shift_x", idx)),
                                target_shift_x,
                                0.11,
                            );
                            let rect = base_rect.translate(vec2(shift_x, 0.0));

                            if !is_dragging_this {
                                let hov = anim_bool_eased(
                                    ctx,
                                    ui.make_persistent_id(("rtab_hov", idx)),
                                    resp.hovered(),
                                    0.09,
                                );
                                let bg = if tab.active {
                                    p.bg_elevated
                                } else if hov > 0.01 {
                                    with_alpha(p.bg_hover, hov)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                if bg.a() > 0 {
                                    ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
                                }
                                if tab.active {
                                    ui.painter().rect_stroke(
                                        rect,
                                        Rounding::same(6.0),
                                        egui::Stroke::new(1.0_f32, p.border_subtle),
                                    );
                                }
                                draw_icon(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        pos2(rect.left() + 11.0, rect.center().y),
                                        vec2(12.0, 12.0),
                                    ),
                                    icon,
                                    if tab.active { p.accent_text } else { p.text_muted },
                                );
                                let mut text_x = rect.left() + 21.0;
                                if tab.pinned {
                                    draw_icon(
                                        ui.painter(),
                                        Rect::from_center_size(
                                            pos2(text_x + 4.0, rect.center().y),
                                            vec2(10.0, 10.0),
                                        ),
                                        Icon::Pin,
                                        p.accent_text,
                                    );
                                    text_x += 13.0;
                                }
                                if tab.dirty {
                                    paint_badge_dot(
                                        ui.painter(),
                                        pos2(text_x + 3.0, rect.center().y),
                                        p.warning,
                                    );
                                    text_x += 9.0;
                                }
                                ui.painter().galley(
                                    pos2(text_x, rect.center().y - g.size().y * 0.5),
                                    g,
                                    fg,
                                );

                                let close_r = Rect::from_center_size(
                                    pos2(rect.right() - 10.0, rect.center().y),
                                    vec2(15.0, 15.0),
                                );
                                let ptr = ui.ctx().pointer_hover_pos();
                                let close_hov = !tab.pinned && ptr.is_some_and(|mp| close_r.contains(mp));
                                if !tab.pinned && (tab.active || resp.hovered() || close_hov) {
                                    if close_hov {
                                        ui.painter().rect_filled(
                                            close_r,
                                            Rounding::same(4.0),
                                            p.bg_hover,
                                        );
                                    }
                                    draw_icon(
                                        ui.painter(),
                                        Rect::from_center_size(close_r.center(), vec2(9.5, 9.5)),
                                        Icon::Close,
                                        if close_hov { p.text_primary } else { p.text_muted },
                                    );
                                }

                                if resp.clicked() {
                                    if close_hov {
                                        actions.push(UiAction::CloseRecentTab(tab.target.clone()));
                                    } else {
                                        actions.push(UiAction::OpenNavTarget(tab.target.clone()));
                                    }
                                } else if !tab.pinned && resp.middle_clicked() {
                                    actions.push(UiAction::CloseRecentTab(tab.target.clone()));
                                }
                            }

                            let tab_target_ctx = tab.target.clone();
                            let is_pinned = tab.pinned;
                            let can_split = matches!(
                                tab.target,
                                NavTarget::Note(_) | NavTarget::Collection(_)
                            );
                            resp.context_menu(|ui| {
                                ui.set_min_width(160.0);
                                let pin_lbl = if is_pinned {
                                    s.tab_ctx_unpin
                                } else {
                                    s.tab_ctx_pin
                                };
                                let pin_icon = if is_pinned { Icon::PinOff } else { Icon::Pin };
                                if context_menu_row(ui, p, pin_icon, pin_lbl, false).clicked() {
                                    actions.push(UiAction::TogglePinTab(tab_target_ctx.clone()));
                                    ui.close_menu();
                                }
                                if can_split
                                    && context_menu_row(
                                        ui,
                                        p,
                                        Icon::Split,
                                        s.tab_ctx_open_right,
                                        false,
                                    )
                                    .clicked()
                                {
                                    actions.push(UiAction::OpenInSplit(tab_target_ctx.clone()));
                                    ui.close_menu();
                                }
                                if !is_pinned {
                                    ui.add_space(3.0);
                                    hairline_h(ui, p);
                                    ui.add_space(3.0);
                                    if context_menu_row(
                                        ui,
                                        p,
                                        Icon::Close,
                                        s.tab_ctx_close,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::CloseRecentTab(tab_target_ctx));
                                        ui.close_menu();
                                    }
                                }
                            });

                            let _ = resp.on_hover_cursor(if is_dragging_this {
                                CursorIcon::Grabbing
                            } else {
                                CursorIcon::PointingHand
                            });
                        }
                    });
                });
        });

    // Drag-reorder with 5px threshold, hysteresis, and lifted floating ghost pill
    if let Some(mut st) = drag_state {
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let ptr_pos = ctx.pointer_interact_pos().or_else(|| ctx.pointer_hover_pos());
        if primary_down {
            if let Some(pos) = ptr_pos {
                if !st.active && (pos - st.start_pos).length() >= 5.0 {
                    st.active = true;
                }
                if st.active {
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                    ctx.request_repaint();

                    let group: Vec<(usize, Rect)> = tab_rects
                        .iter()
                        .filter(|&&(_, pin, _)| pin == st.pinned)
                        .map(|&(i, _, r)| (i, r))
                        .collect();

                    if !group.is_empty() {
                        // Compute target insertion index with 8px hysteresis
                        let mut new_target = 0usize;
                        for (g_idx, &(_, r)) in group.iter().enumerate() {
                            let thresh = if g_idx < st.target_in_group {
                                r.center().x - 8.0
                            } else {
                                r.center().x + 8.0
                            };
                            if pos.x >= thresh {
                                new_target = g_idx + 1;
                            }
                        }
                        st.target_in_group = new_target.min(group.len().saturating_sub(1));

                        // Draw target slot placeholder in the exact vacated gap
                        if let Some(src_g) = src_group_pos {
                            let target_ph_x = if st.target_in_group > src_g {
                                group[st.target_in_group].1.right() - st.tab_w
                            } else {
                                group[st.target_in_group].1.left()
                            };
                            let ph_x = crate::ui::motion::anim_value_smooth(
                                ctx,
                                egui::Id::new("rtab_ph_x"),
                                target_ph_x,
                                0.11,
                            );
                            let ph_r = Rect::from_min_size(
                                pos2(ph_x, panel_resp.response.rect.top() + 4.0),
                                vec2(st.tab_w, 23.0),
                            );
                            let ph_p = ctx.layer_painter(egui::LayerId::new(
                                egui::Order::Foreground,
                                egui::Id::new("krono_tab_ph_layer"),
                            ));
                            ph_p.rect_filled(
                                ph_r,
                                Rounding::same(6.0),
                                with_alpha(p.accent, 0.10),
                            );
                            ph_p.rect_stroke(
                                ph_r,
                                Rounding::same(6.0),
                                egui::Stroke::new(1.0_f32, with_alpha(p.accent, 0.45)),
                            );
                        }

                        // Draw floating lifted ghost pill following cursor horizontally
                        if let Some(tab) = recent_tabs.get(st.src_idx) {
                            let bar_r = panel_resp.response.rect;
                            let ghost_x = (pos.x - st.grab_offset_x)
                                .clamp(bar_r.left() + 6.0, (bar_r.right() - st.tab_w - 6.0).max(bar_r.left() + 6.0));
                            let ghost_y = bar_r.top() + 2.0; // lifted 2px
                            let ghost_r = Rect::from_min_size(pos2(ghost_x, ghost_y), vec2(st.tab_w, 23.0));
                            let gp = ctx.layer_painter(egui::LayerId::new(
                                egui::Order::Tooltip,
                                egui::Id::new("krono_tab_drag_ghost"),
                            ));
                            // Soft shadow under lifted pill
                            gp.rect_filled(
                                ghost_r.translate(vec2(0.0, 3.0)),
                                Rounding::same(7.0),
                                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 55),
                            );
                            gp.rect_filled(ghost_r, Rounding::same(6.0), p.bg_elevated);
                            gp.rect_stroke(
                                ghost_r,
                                Rounding::same(6.0),
                                egui::Stroke::new(1.2_f32, p.accent),
                            );

                            let icon = match tab.kind {
                                VaultFileKind::Board => Icon::Board,
                                VaultFileKind::Table => Icon::Table,
                                VaultFileKind::Canvas => Icon::Canvas,
                                _ => Icon::Document,
                            };
                            draw_icon(
                                &gp,
                                Rect::from_center_size(
                                    pos2(ghost_r.left() + 11.0, ghost_r.center().y),
                                    vec2(12.0, 12.0),
                                ),
                                icon,
                                p.accent_text,
                            );
                            let max_chars = if tab.pinned { 14 } else { 24 };
                            let label: String = tab.title.chars().take(max_chars).collect();
                            let title_show = if tab.title.chars().count() > max_chars {
                                format!("{label}…")
                            } else {
                                label
                            };
                            let gg = ctx.fonts(|f| {
                                f.layout_no_wrap(title_show, font_medium(11.5), p.text_primary)
                            });
                            let text_x = ghost_r.left() + if tab.pinned { 34.0 } else { 21.0 };
                            gp.galley(
                                pos2(text_x, ghost_r.center().y - gg.size().y * 0.5),
                                gg,
                                p.text_primary,
                            );
                        }
                    }
                }
            }
            ctx.data_mut(|d| d.insert_temp(drag_id, st));
        } else {
            if st.active {
                let group: Vec<(usize, Rect)> = tab_rects
                    .iter()
                    .filter(|&&(_, pin, _)| pin == st.pinned)
                    .map(|&(i, _, r)| (i, r))
                    .collect();
                if let Some(&(global_to_index, _)) = group.get(st.target_in_group) {
                    if let Some(item) = recent_tabs.get(st.src_idx) {
                        actions.push(UiAction::ReorderRecentTab {
                            target: item.target.clone(),
                            to_index: global_to_index,
                        });
                    }
                }
            }
            ctx.data_mut(|d| d.remove::<TabDragState>(drag_id));
        }
    }

    draw_outer_bottom_border(ctx, panel_resp.response.rect, p);
    actions
}
