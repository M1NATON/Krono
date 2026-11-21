use crate::i18n::{format_relative_time, strings, Lang, Strings};
use crate::ui::components::{
    hairline_h, icon_button, optional_status_pill_picker, primary_button, secondary_button,
    tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_serif_bold, ThemePalette};
use crate::ui::{LayoutClass, TableColumnWidths, TableSort, UiAction, ViewMode};
use eframe::egui::{
    self, pos2, vec2, Align, CursorIcon, Layout, Margin, Rect, Rounding, Sense, Stroke, Ui,
};
use krono_core::{
    collection::CollectionViewKind,
    note::{NoteMeta, NoteStatus},
};

#[allow(clippy::too_many_arguments)]
pub fn render_table_view(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    notes: &[NoteMeta],
    filter_query: &mut String,
    status_filter: &mut Option<NoteStatus>,
    active_tag: Option<&str>,
    sort_mode: &mut TableSort,
    col_widths: &mut TableColumnWidths,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    let card_mode = layout.is_mobile() || ui.available_width() < 560.0;

    if !layout.is_mobile() {
        // Header row (desktop & compact; mobile uses topbar + search bar)
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(s.table_title)
                    .font(font_serif_bold(24.0))
                    .color(p.text_primary),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("({})", notes.len()))
                    .font(font_mono(13.0))
                    .color(p.text_muted),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if primary_button(ui, p, Some(Icon::Plus), s.new_note, false).clicked() {
                    actions.push(UiAction::CreateNote {
                        title: default_new_note_title(lang),
                        status: *status_filter,
                        tags: Vec::new(),
                    });
                }
                if secondary_button(ui, p, Some(Icon::Table), s.new_custom_table_btn).clicked() {
                    actions.push(UiAction::CreateCollection {
                        title: if lang == Lang::Ru {
                            "Новая таблица".into()
                        } else {
                            "New Table".into()
                        },
                        kind: CollectionViewKind::Table,
                    });
                }
            });
        });
        ui.add_space(12.0);
    }

    if card_mode {
        render_mobile_toolbar(
            ui,
            p,
            s,
            notes.len(),
            filter_query,
            status_filter,
            active_tag,
            sort_mode,
            &mut actions,
        );
    } else {
        render_desktop_toolbar(
            ui,
            p,
            s,
            layout,
            filter_query,
            status_filter,
            active_tag,
            sort_mode,
            &mut actions,
        );
    }

    ui.add_space(if card_mode { 10.0 } else { 12.0 });

    let filtered = filter_and_sort_notes(notes, filter_query, *status_filter, active_tag, *sort_mode);

    if card_mode {
        render_mobile_cards(ui, p, lang, s, &filtered, &mut actions);
    } else {
        render_desktop_table(ui, p, lang, s, &filtered, sort_mode, col_widths, &mut actions);
    }

    actions
}

fn default_new_note_title(lang: Lang) -> String {
    if lang == Lang::Ru {
        "Новая заметка".into()
    } else {
        "New Note".into()
    }
}

fn filter_and_sort_notes<'a>(
    notes: &'a [NoteMeta],
    filter_query: &str,
    status_filter: Option<NoteStatus>,
    active_tag: Option<&str>,
    sort_mode: TableSort,
) -> Vec<&'a NoteMeta> {
    let q_lower = filter_query.trim().to_lowercase();
    let mut filtered: Vec<&NoteMeta> = notes
        .iter()
        .filter(|n| {
            if let Some(st) = status_filter {
                if n.status != Some(st) {
                    return false;
                }
            }
            if let Some(tag) = active_tag {
                if !n.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                    return false;
                }
            }
            if !q_lower.is_empty() {
                let in_title = n.title.to_lowercase().contains(&q_lower);
                let in_tags = n.tags.iter().any(|t| t.to_lowercase().contains(&q_lower));
                let in_excerpt = n.excerpt.to_lowercase().contains(&q_lower);
                let in_path = n.path.to_lowercase().contains(&q_lower);
                if !in_title && !in_tags && !in_excerpt && !in_path {
                    return false;
                }
            }
            true
        })
        .collect();

    match sort_mode {
        TableSort::ModifiedDesc => filtered.sort_by(|a, b| b.modified.cmp(&a.modified)),
        TableSort::ModifiedAsc => filtered.sort_by(|a, b| a.modified.cmp(&b.modified)),
        TableSort::TitleAsc => {
            filtered.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }
        TableSort::TitleDesc => {
            filtered.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()))
        }
        TableSort::StatusAsc => filtered.sort_by(|a, b| {
            a.status
                .map_or(3, NoteStatus::sort_rank)
                .cmp(&b.status.map_or(3, NoteStatus::sort_rank))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        TableSort::StatusDesc => filtered.sort_by(|a, b| {
            b.status
                .map_or(0, |st| 3 - st.sort_rank())
                .cmp(&a.status.map_or(0, |st| 3 - st.sort_rank()))
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        TableSort::TagsAsc => filtered.sort_by(|a, b| {
            let ta = a.tags.first().map(|t| t.to_lowercase()).unwrap_or_default();
            let tb = b.tags.first().map(|t| t.to_lowercase()).unwrap_or_default();
            match (ta.is_empty(), tb.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => ta.cmp(&tb),
            }
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
        TableSort::TagsDesc => filtered.sort_by(|a, b| {
            let ta = a.tags.first().map(|t| t.to_lowercase()).unwrap_or_default();
            let tb = b.tags.first().map(|t| t.to_lowercase()).unwrap_or_default();
            match (ta.is_empty(), tb.is_empty()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => tb.cmp(&ta),
            }
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        }),
    }
    filtered
}

fn render_search_box(
    ui: &mut Ui,
    p: &ThemePalette,
    hint: &str,
    filter_query: &mut String,
    desired_w: f32,
) {
    egui::Frame::none()
        .fill(p.bg_input)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
                draw_icon(
                    ui.painter(),
                    Rect::from_center_size(ir.center(), vec2(12.5, 12.5)),
                    Icon::Search,
                    p.text_muted,
                );
                ui.add(
                    egui::TextEdit::singleline(filter_query)
                        .hint_text(hint)
                        .frame(false)
                        .font(font_body(12.5))
                        .desired_width(desired_w),
                );
            });
        });
}

fn render_status_chips(
    ui: &mut Ui,
    p: &ThemePalette,
    s: &Strings,
    status_filter: &mut Option<NoteStatus>,
    active_tag: Option<&str>,
    actions: &mut Vec<UiAction>,
) {
    let status_opts: &[(Option<NoteStatus>, &str)] = &[
        (None, s.status_all),
        (Some(NoteStatus::Todo), s.status_todo),
        (Some(NoteStatus::InProgress), s.status_in_progress),
        (Some(NoteStatus::Done), s.status_done),
    ];
    for &(opt, lbl) in status_opts {
        let active = *status_filter == opt;
        let fg_rest = if active { p.text_primary } else { p.text_secondary };
        let g = ui
            .painter()
            .layout_no_wrap(lbl.to_string(), font_medium(12.0), fg_rest);
        let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 26.0), Sense::click());
        let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
        let sel_t = anim_bool_eased(ui.ctx(), resp.id.with("sel"), active, 0.14);
        let rest_bg = lerp_color(p.bg_elevated, p.bg_hover, hov);
        let bg = lerp_color(rest_bg, p.bg_selected, sel_t);
        ui.painter().rect_filled(rect, Rounding::same(13.0), bg);
        ui.painter().rect_stroke(
            rect,
            Rounding::same(13.0),
            Stroke::new(
                1.0_f32,
                lerp_color(p.border_hairline, p.border_strong, sel_t.max(hov * 0.6)),
            ),
        );
        ui.painter().galley(
            pos2(
                rect.center().x - g.size().x * 0.5,
                rect.center().y - g.size().y * 0.5,
            ),
            g,
            lerp_color(fg_rest, p.text_primary, sel_t.max(hov)),
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *status_filter = opt;
        }
    }

    if let Some(tag) = active_tag {
        let (resp, rm) = tag_chip(ui, p, tag, true);
        if rm || resp.clicked() {
            actions.push(UiAction::SetTagFilter(None));
        }
    }
}

fn render_sort_pills(ui: &mut Ui, p: &ThemePalette, s: &Strings, sort_mode: &mut TableSort) {
    ui.label(
        egui::RichText::new(s.sort_label)
            .font(font_body(12.0))
            .color(p.text_muted),
    );
    for (asc_mode, desc_mode, lbl) in [
        (TableSort::ModifiedAsc, TableSort::ModifiedDesc, s.sort_modified),
        (TableSort::TitleAsc, TableSort::TitleDesc, s.sort_title),
        (TableSort::StatusAsc, TableSort::StatusDesc, s.sort_status),
    ] {
        let active = *sort_mode == asc_mode || *sort_mode == desc_mode;
        let arrow = if *sort_mode == asc_mode {
            " ↑"
        } else if *sort_mode == desc_mode {
            " ↓"
        } else {
            ""
        };
        let full_lbl = format!("{lbl}{arrow}");
        let fg_rest = if active { p.text_primary } else { p.text_muted };
        let g = ui
            .painter()
            .layout_no_wrap(full_lbl, font_medium(12.0), fg_rest);
        let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 24.0), Sense::click());
        let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
        let sel_t = anim_bool_eased(ui.ctx(), resp.id.with("sel"), active, 0.14);
        let fill_t = sel_t.max(hov * 0.65);
        if fill_t > 0.01 {
            let bg = lerp_color(p.bg_hover, p.bg_selected, sel_t);
            ui.painter()
                .rect_filled(rect, Rounding::same(5.0), with_alpha(bg, fill_t));
        }
        ui.painter().galley(
            pos2(
                rect.center().x - g.size().x * 0.5,
                rect.center().y - g.size().y * 0.5,
            ),
            g,
            lerp_color(fg_rest, p.text_primary, sel_t.max(hov)),
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            *sort_mode = if *sort_mode == desc_mode {
                asc_mode
            } else {
                desc_mode
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_toolbar(
    ui: &mut Ui,
    p: &ThemePalette,
    s: &Strings,
    total_notes: usize,
    filter_query: &mut String,
    status_filter: &mut Option<NoteStatus>,
    active_tag: Option<&str>,
    sort_mode: &mut TableSort,
    actions: &mut Vec<UiAction>,
) {
    render_search_box(ui, p, s.filter_search_hint, filter_query, f32::INFINITY);
    ui.add_space(8.0);

    egui::ScrollArea::horizontal()
        .id_salt("tbl_mobile_status_scroll")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                render_status_chips(ui, p, s, status_filter, active_tag, actions);
            });
        });

    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
        render_sort_pills(ui, p, s, sort_mode);
        ui.label(
            egui::RichText::new(format!("({total_notes})"))
                .font(font_mono(11.5))
                .color(p.text_muted),
        );
    });
}

#[allow(clippy::too_many_arguments)]
fn render_desktop_toolbar(
    ui: &mut Ui,
    p: &ThemePalette,
    s: &Strings,
    layout: LayoutClass,
    filter_query: &mut String,
    status_filter: &mut Option<NoteStatus>,
    active_tag: Option<&str>,
    sort_mode: &mut TableSort,
    actions: &mut Vec<UiAction>,
) {
    let search_w = if layout == LayoutClass::Compact {
        168.0
    } else {
        220.0
    };
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        render_search_box(ui, p, s.filter_search_hint, filter_query, search_w);
        render_status_chips(ui, p, s, status_filter, active_tag, actions);
        if layout == LayoutClass::Compact {
            render_sort_pills(ui, p, s, sort_mode);
        } else {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for (mode, lbl) in [
                    (TableSort::StatusAsc, s.sort_status),
                    (TableSort::TitleAsc, s.sort_title),
                    (TableSort::ModifiedDesc, s.sort_modified),
                ] {
                    let active = *sort_mode == mode;
                    let fg_rest = if active { p.text_primary } else { p.text_muted };
                    let g = ui
                        .painter()
                        .layout_no_wrap(lbl.to_string(), font_medium(12.0), fg_rest);
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(g.size().x + 14.0, 24.0), Sense::click());
                    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
                    let sel_t = anim_bool_eased(ui.ctx(), resp.id.with("sel"), active, 0.14);
                    let fill_t = sel_t.max(hov * 0.65);
                    if fill_t > 0.01 {
                        let bg = lerp_color(p.bg_hover, p.bg_selected, sel_t);
                        ui.painter()
                            .rect_filled(rect, Rounding::same(5.0), with_alpha(bg, fill_t));
                    }
                    ui.painter().galley(
                        pos2(
                            rect.center().x - g.size().x * 0.5,
                            rect.center().y - g.size().y * 0.5,
                        ),
                        g,
                        lerp_color(fg_rest, p.text_primary, sel_t.max(hov)),
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        *sort_mode = mode;
                    }
                }
                ui.label(
                    egui::RichText::new(s.sort_label)
                        .font(font_body(12.0))
                        .color(p.text_muted),
                );
            });
        }
    });
}

fn render_mobile_cards(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    filtered: &[&NoteMeta],
    actions: &mut Vec<UiAction>,
) {
    egui::ScrollArea::vertical()
        .id_salt("table_cards_scroll")
        .show(ui, |ui| {
            if filtered.is_empty() {
                ui.add_space(28.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(s.palette_no_results)
                            .font(font_body(13.0))
                            .color(p.text_muted),
                    );
                });
                ui.add_space(28.0);
                return;
            }

            let card_w = ui.available_width();
            for note in filtered {
                egui::Frame::none()
                    .fill(p.bg_card)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_width((card_w - 24.0).max(200.0));
                        let title_w = (ui.available_width() - 36.0).max(120.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.allocate_ui_with_layout(
                                vec2(title_w, 24.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let (ir, _) =
                                        ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    draw_icon(ui.painter(), ir, Icon::Document, p.text_muted);
                                    let resp = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&note.title)
                                                .font(font_medium(14.0))
                                                .color(p.text_primary),
                                        )
                                        .truncate()
                                        .sense(Sense::click()),
                                    );
                                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        actions.push(UiAction::OpenNote(note.path.clone()));
                                        actions.push(UiAction::SetViewMode(ViewMode::Editor));
                                    }
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked() {
                                    actions.push(UiAction::RequestDelete(note.path.clone()));
                                }
                            });
                        });

                        if !note.excerpt.is_empty() {
                            ui.add_space(3.0);
                            let ex = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&note.excerpt)
                                        .font(font_body(12.0))
                                        .color(p.text_secondary),
                                )
                                .wrap()
                                .sense(Sense::click()),
                            );
                            if ex.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                actions.push(UiAction::OpenNote(note.path.clone()));
                                actions.push(UiAction::SetViewMode(ViewMode::Editor));
                            }
                        }

                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                            if let Some(new_st) = optional_status_pill_picker(
                                ui,
                                p,
                                lang,
                                ("tbl_card_st", &note.path),
                                note.status,
                            ) {
                                if new_st != note.status {
                                    actions.push(UiAction::UpdateNoteStatus {
                                        path: note.path.clone(),
                                        status: new_st,
                                    });
                                }
                            }
                            for t in note.tags.iter().take(2) {
                                let (resp, _) = tag_chip(ui, p, t, false);
                                if resp.clicked() {
                                    actions.push(UiAction::SetTagFilter(Some(t.clone())));
                                }
                            }
                            ui.label(
                                egui::RichText::new(format_relative_time(lang, note.modified))
                                    .font(font_mono(11.0))
                                    .color(p.text_muted),
                            );
                        });
                    });
                ui.add_space(8.0);
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn render_desktop_table(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    filtered: &[&NoteMeta],
    sort_mode: &mut TableSort,
    col_widths: &mut TableColumnWidths,
    actions: &mut Vec<UiAction>,
) {
    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(0.0))
        .show(ui, |ui| {
            let avail_w = (ui.available_width() - 28.0).max(460.0);
            let show_path = avail_w >= 680.0;
            let col_del = 32.0;
            let base_name = col_widths.name.clamp(120.0, 560.0);
            let col_status = col_widths.status.clamp(100.0, 320.0);
            let base_tags = col_widths.tags.clamp(90.0, 420.0);
            let base_path = if show_path {
                col_widths.path.clamp(90.0, 480.0)
            } else {
                0.0
            };
            let col_mod = col_widths.modified.clamp(84.0, 240.0);
            let base_sum = base_name + col_status + base_tags + base_path + col_mod + col_del + 16.0;
            let extra = (avail_w - base_sum).max(0.0);
            let (col_name, col_tags, col_path) = if show_path {
                (
                    base_name + extra * 0.50,
                    base_tags + extra * 0.25,
                    base_path + extra * 0.25,
                )
            } else {
                (base_name + extra * 0.65, base_tags + extra * 0.35, 0.0)
            };

            egui::Frame::none()
                .fill(p.bg_sidebar)
                .rounding(Rounding {
                    nw: 10.0,
                    ne: 10.0,
                    sw: 0.0,
                    se: 0.0,
                })
                .inner_margin(Margin::symmetric(14.0, 9.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let name_ind = match *sort_mode {
                            TableSort::TitleAsc => Some("↑"),
                            TableSort::TitleDesc => Some("↓"),
                            _ => None,
                        };
                        if col_header(ui, p, s.table_col_name, col_name - 6.0, name_ind) {
                            *sort_mode = if *sort_mode == TableSort::TitleAsc {
                                TableSort::TitleDesc
                            } else {
                                TableSort::TitleAsc
                            };
                        }
                        col_widths.name = (col_widths.name + resize_handle(ui, p, "ntbl_name"))
                            .clamp(120.0, 560.0);

                        let st_ind = match *sort_mode {
                            TableSort::StatusAsc => Some("↑"),
                            TableSort::StatusDesc => Some("↓"),
                            _ => None,
                        };
                        if col_header(ui, p, s.table_col_status, col_status - 6.0, st_ind) {
                            *sort_mode = if *sort_mode == TableSort::StatusAsc {
                                TableSort::StatusDesc
                            } else {
                                TableSort::StatusAsc
                            };
                        }
                        col_widths.status = (col_widths.status + resize_handle(ui, p, "ntbl_status"))
                            .clamp(100.0, 320.0);

                        let tg_ind = match *sort_mode {
                            TableSort::TagsAsc => Some("↑"),
                            TableSort::TagsDesc => Some("↓"),
                            _ => None,
                        };
                        if col_header(ui, p, s.table_col_tags, col_tags - 6.0, tg_ind) {
                            *sort_mode = if *sort_mode == TableSort::TagsAsc {
                                TableSort::TagsDesc
                            } else {
                                TableSort::TagsAsc
                            };
                        }
                        col_widths.tags = (col_widths.tags + resize_handle(ui, p, "ntbl_tags"))
                            .clamp(90.0, 420.0);

                        if show_path {
                            let _ = col_header(ui, p, s.table_col_path, col_path - 6.0, None);
                            col_widths.path = (col_widths.path + resize_handle(ui, p, "ntbl_path"))
                                .clamp(90.0, 480.0);
                        }

                        let mod_ind = match *sort_mode {
                            TableSort::ModifiedAsc => Some("↑"),
                            TableSort::ModifiedDesc => Some("↓"),
                            _ => None,
                        };
                        if col_header(ui, p, s.table_col_modified, col_mod - 6.0, mod_ind) {
                            *sort_mode = if *sort_mode == TableSort::ModifiedDesc {
                                TableSort::ModifiedAsc
                            } else {
                                TableSort::ModifiedDesc
                            };
                        }
                        col_widths.modified = (col_widths.modified + resize_handle(ui, p, "ntbl_mod"))
                            .clamp(84.0, 240.0);

                        let _ = ui.allocate_exact_size(vec2(col_del, 18.0), Sense::hover());
                    });
                });
            hairline_h(ui, p);

            egui::ScrollArea::vertical()
                .id_salt("table_rows_scroll")
                .show(ui, |ui| {
                    if filtered.is_empty() {
                        ui.add_space(28.0);
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(s.palette_no_results)
                                    .font(font_body(13.0))
                                    .color(p.text_muted),
                            );
                        });
                        ui.add_space(28.0);
                    }

                    for (idx, note) in filtered.iter().enumerate() {
                        if idx > 0 {
                            hairline_h(ui, p);
                        }
                        let row_id = ui.make_persistent_id(("tbl_row", &note.path));
                        let prev_hov = ui.rect_contains_pointer(Rect::from_min_size(
                            ui.next_widget_position(),
                            vec2(avail_w + 28.0, 38.0),
                        ));
                        let row_t = anim_bool_eased(ui.ctx(), row_id, prev_hov, 0.09);
                        let row_fill = with_alpha(p.bg_hover, row_t * 0.65);

                        egui::Frame::none()
                            .fill(row_fill)
                            .inner_margin(Margin::symmetric(14.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;

                                    in_cell(ui, col_name, 26.0, |ui| {
                                        let (ir, _) = ui
                                            .allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                        draw_icon(ui.painter(), ir, Icon::Document, p.text_muted);
                                        let link_resp = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&note.title)
                                                    .font(font_medium(13.5))
                                                    .color(p.text_primary),
                                            )
                                            .truncate()
                                            .sense(Sense::click()),
                                        );
                                        if link_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            actions.push(UiAction::OpenNote(note.path.clone()));
                                            actions.push(UiAction::SetViewMode(ViewMode::Editor));
                                        }
                                    });

                                    in_cell(ui, col_status, 26.0, |ui| {
                                        if let Some(new_st) = optional_status_pill_picker(
                                            ui,
                                            p,
                                            lang,
                                            ("tbl_status", &note.path),
                                            note.status,
                                        ) {
                                            if new_st != note.status {
                                                actions.push(UiAction::UpdateNoteStatus {
                                                    path: note.path.clone(),
                                                    status: new_st,
                                                });
                                            }
                                        }
                                    });

                                    in_cell(ui, col_tags, 26.0, |ui| {
                                        if note.tags.is_empty() {
                                            ui.label(
                                                egui::RichText::new("—")
                                                    .font(font_body(12.0))
                                                    .color(p.text_muted),
                                            );
                                        } else {
                                            for t in note.tags.iter().take(2) {
                                                let (resp, _) = tag_chip(ui, p, t, false);
                                                if resp.clicked() {
                                                    actions.push(UiAction::SetTagFilter(Some(
                                                        t.clone(),
                                                    )));
                                                }
                                            }
                                        }
                                    });

                                    if show_path {
                                        in_cell(ui, col_path, 26.0, |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(&note.path)
                                                        .font(font_mono(11.5))
                                                        .color(p.text_muted),
                                                )
                                                .truncate(),
                                            );
                                        });
                                    }

                                    in_cell(ui, col_mod, 26.0, |ui| {
                                        ui.label(
                                            egui::RichText::new(format_relative_time(
                                                lang,
                                                note.modified,
                                            ))
                                            .font(font_mono(11.5))
                                            .color(p.text_secondary),
                                        );
                                    });

                                    in_cell(ui, col_del, 26.0, |ui| {
                                        if icon_button(
                                            ui,
                                            p,
                                            Icon::Trash,
                                            s.delete_btn,
                                            false,
                                        )
                                        .clicked()
                                        {
                                            actions
                                                .push(UiAction::RequestDelete(note.path.clone()));
                                        }
                                    });
                                });
                            });
                    }
                });
        });
}

fn in_cell(ui: &mut Ui, width: f32, height: f32, add_contents: impl FnOnce(&mut Ui)) {
    let (cell_rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let mut cell_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(cell_rect)
            .layout(Layout::left_to_right(Align::Center)),
    );
    cell_ui.set_clip_rect(cell_rect.intersect(ui.clip_rect()));
    add_contents(&mut cell_ui);
}

fn col_header(
    ui: &mut Ui,
    p: &ThemePalette,
    text: &str,
    width: f32,
    sort_indicator: Option<&str>,
) -> bool {
    let label = match sort_indicator {
        Some(ind) => format!("{text} {ind}"),
        None => text.to_string(),
    };
    let (cell_rect, resp) = ui.allocate_exact_size(vec2(width.max(40.0), 20.0), Sense::click());
    let g = ui.painter().layout_no_wrap(
        label,
        font_medium(12.0),
        if sort_indicator.is_some() || resp.hovered() {
            p.text_primary
        } else {
            p.text_muted
        },
    );
    ui.painter().galley(
        pos2(cell_rect.left() + 2.0, cell_rect.center().y - g.size().y * 0.5),
        g,
        p.text_muted,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
}

fn resize_handle(ui: &mut Ui, p: &ThemePalette, id_key: &str) -> f32 {
    let (rect, _) = ui.allocate_exact_size(vec2(6.0, 20.0), Sense::hover());
    let resp = ui.interact(rect, ui.make_persistent_id(id_key), Sense::drag());
    let active = resp.hovered() || resp.dragged();
    ui.painter().line_segment(
        [
            pos2(rect.center().x, rect.top() + 2.0),
            pos2(rect.center().x, rect.bottom() - 2.0),
        ],
        Stroke::new(if active { 2.0_f32 } else { 1.0_f32 }, if active { p.accent } else { p.border_hairline }),
    );
    if active {
        resp.clone().on_hover_cursor(CursorIcon::ResizeHorizontal);
    }
    if resp.dragged() {
        resp.drag_delta().x
    } else {
        0.0
    }
}
