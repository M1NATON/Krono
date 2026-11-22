use crate::i18n::{status_label, strings, Lang, Strings};
use crate::ui::components::{
    focus_once, icon_button, normalize_tag_input, primary_button, reset_focus_gate,
    secondary_button, status_pill_picker, styled_text_input, tag_autocomplete_input, tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, lerp_color, with_alpha};
use crate::ui::theme::{
    font_body, font_medium, font_mono, font_semibold, font_serif_bold, ThemePalette,
};
use crate::ui::board_dnd::{BoardDndState, ColumnGeometry};
use crate::ui::{
    note_matches_tag_filters, BoardSort, LayoutClass, TagFilterMode, UiAction, ViewMode,
};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, CursorIcon, Layout, Margin, Order, Rect, Rounding, Sense,
    Stroke, Ui,
};
use krono_core::{
    collection::CollectionViewKind,
    note::{NoteMeta, NoteStatus},
};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub fn render_board_view(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    mobile_tab: &mut NoteStatus,
    notes: &[NoteMeta],
    active_tags: &[String],
    tag_mode: TagFilterMode,
    quick_add_status: &mut Option<NoteStatus>,
    quick_add_title: &mut String,
    quick_add_tags: &mut Vec<String>,
    quick_add_tag_input: &mut String,
    available_tags: &[String],
    board_sort: &mut BoardSort,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    let tabbed_mode = layout.is_mobile() || ui.available_width() < 620.0;

    if !layout.is_mobile() {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(s.board_title)
                    .font(font_serif_bold(24.0))
                    .color(p.text_primary),
            );
            for tag in active_tags {
                ui.add_space(6.0);
                let (resp, rm) = tag_chip(ui, p, tag, true);
                if rm || resp.clicked() {
                    actions.push(UiAction::ToggleTagFilter(tag.clone()));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                if secondary_button(ui, p, Some(Icon::Board), s.new_custom_board_btn).clicked() {
                    actions.push(UiAction::CreateCollection {
                        title: if lang == Lang::Ru {
                            "Новая доска".into()
                        } else {
                            "New Board".into()
                        },
                        kind: CollectionViewKind::Board,
                    });
                }

                let sort_lbl = match *board_sort {
                    BoardSort::Manual => if lang == Lang::Ru { "Порядок: Ручной" } else { "Order: Manual" },
                    BoardSort::ModifiedDesc => if lang == Lang::Ru { "Дата ↓" } else { "Date ↓" },
                    BoardSort::ModifiedAsc => if lang == Lang::Ru { "Дата ↑" } else { "Date ↑" },
                    BoardSort::TitleAsc => if lang == Lang::Ru { "Название ↑" } else { "Title ↑" },
                    BoardSort::TitleDesc => if lang == Lang::Ru { "Название ↓" } else { "Title ↓" },
                };
                if secondary_button(ui, p, Some(Icon::Filter), sort_lbl).clicked() {
                    *board_sort = match *board_sort {
                        BoardSort::Manual => BoardSort::ModifiedDesc,
                        BoardSort::ModifiedDesc => BoardSort::ModifiedAsc,
                        BoardSort::ModifiedAsc => BoardSort::TitleAsc,
                        BoardSort::TitleAsc => BoardSort::TitleDesc,
                        BoardSort::TitleDesc => BoardSort::Manual,
                    };
                }
            });
        });
        ui.add_space(14.0);
    } else if !active_tags.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for tag in active_tags {
                let (resp, rm) = tag_chip(ui, p, tag, true);
                if rm || resp.clicked() {
                    actions.push(UiAction::ToggleTagFilter(tag.clone()));
                }
            }
        });
        ui.add_space(8.0);
    }

    let mut grouped: HashMap<NoteStatus, Vec<&NoteMeta>> = HashMap::new();
    for st in NoteStatus::ALL {
        grouped.insert(st, Vec::new());
    }
    for n in notes {
        if !note_matches_tag_filters(&n.tags, active_tags, tag_mode) {
            continue;
        }
        if let Some(st) = n.status {
            grouped.entry(st).or_default().push(n);
        }
    }
    let is_sorted = *board_sort != BoardSort::Manual;
    for list in grouped.values_mut() {
        match *board_sort {
            BoardSort::Manual => list.sort_by(|a, b| {
                let ar = if a.board_order > 0 { 0 } else { 1 };
                let br = if b.board_order > 0 { 0 } else { 1 };
                ar.cmp(&br)
                    .then_with(|| a.board_order.cmp(&b.board_order))
                    .then_with(|| b.modified.cmp(&a.modified))
            }),
            BoardSort::ModifiedDesc => list.sort_by(|a, b| b.modified.cmp(&a.modified)),
            BoardSort::ModifiedAsc => list.sort_by(|a, b| a.modified.cmp(&b.modified)),
            BoardSort::TitleAsc => {
                list.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }
            BoardSort::TitleDesc => {
                list.sort_by(|a, b| b.title.to_lowercase().cmp(&a.title.to_lowercase()))
            }
        }
    }

    if tabbed_mode {
        render_mobile_tabbed_board(
            ui,
            p,
            lang,
            s,
            mobile_tab,
            &grouped,
            quick_add_status,
            quick_add_title,
            quick_add_tags,
            quick_add_tag_input,
            available_tags,
            &mut actions,
        );
        return actions;
    }

    let board_bottom = ui.max_rect().bottom();
    let dnd_id = ui.make_persistent_id("notes_kanban_dnd");
    let mut dnd = BoardDndState::load(ui.ctx(), dnd_id);
    if let Some(drop_ev) = dnd.begin_frame(ui.ctx(), is_sorted) {
        actions.push(UiAction::MoveNoteInBoard {
            path: drop_ev.card_id,
            status: NoteStatus::from_str_lossy(&drop_ev.to_col),
            insert_before: drop_ev.insert_before_id,
        });
    }

    let qa_top_id = egui::Id::new("board_qa_at_top_st");
    let mut qa_at_top = ui.ctx().data(|d| d.get_temp::<bool>(qa_top_id)).unwrap_or(false);
    if quick_add_status.is_none() {
        qa_at_top = false;
    }
    let ptr_pos = ui.ctx().pointer_interact_pos();
    let mut new_columns: HashMap<String, ColumnGeometry> = HashMap::new();
    let prev_selectable = ui.style().interaction.selectable_labels;
    if dnd.is_dragging() {
        ui.style_mut().interaction.selectable_labels = false;
    }

    ui.columns(3, |cols| {
        for (idx, status) in NoteStatus::ALL.into_iter().enumerate() {
            let col_ui = &mut cols[idx];
            let col_total_w = col_ui.available_width().max(240.0);
            let inner_w = (col_total_w - 20.0).max(180.0);
            let (pill_bg, dot_col, text_col) = p.status_colors(status);
            let col_id = status.as_yaml_str().to_string();
            let items = grouped.get(&status).map(Vec::as_slice).unwrap_or(&[]);
            let is_top_open = *quick_add_status == Some(status) && qa_at_top;

            let prev_col_rect = dnd.columns.get(&col_id).map(|c| c.drop_rect);
            let col_ptr_inside = prev_col_rect
                .is_some_and(|r| ptr_pos.is_some_and(|pt| r.contains(pt)));
            let col_hov_t = anim_bool_eased(
                col_ui.ctx(),
                col_ui.make_persistent_id(("kboard_col_hov", status.sort_rank())),
                col_ptr_inside || is_top_open,
                0.12,
            );

            let mut col_frame = egui::Frame::none()
                .fill(Color32::TRANSPARENT)
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(12.0))
                .inner_margin(Margin::symmetric(10.0, 8.0))
                .begin(col_ui);

            let mut col_base_cards: Vec<(String, f32, f32)> = Vec::with_capacity(items.len());
            let col_scroll_rect;
            let col_scroll_offset_y;
            {
                let cui = &mut col_frame.content_ui;
                cui.set_width(inner_w);
                cui.set_max_width(inner_w);

                // Column Header
                cui.horizontal(|ui| {
                    egui::Frame::none()
                        .fill(pill_bg)
                        .rounding(Rounding::same(10.0))
                        .inner_margin(Margin::symmetric(9.0, 3.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let (dr, _) =
                                    ui.allocate_exact_size(vec2(7.0, 12.0), Sense::hover());
                                ui.painter().circle_filled(dr.center(), 3.2, dot_col);
                                ui.label(
                                    egui::RichText::new(status_label(lang, status))
                                        .font(font_medium(12.0))
                                        .color(text_col),
                                );
                            });
                        });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(items.len().to_string())
                            .font(font_mono(11.5))
                            .color(p.text_muted),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let hdr_alpha = col_hov_t;
                        if muted_board_icon_btn(
                            ui,
                            p,
                            Icon::Plus,
                            s.add_card_btn,
                            is_top_open,
                            hdr_alpha,
                        )
                        .clicked() {
                            if is_top_open {
                                *quick_add_status = None;
                                qa_at_top = false;
                                quick_add_title.clear();
                                quick_add_tags.clear();
                                quick_add_tag_input.clear();
                            } else {
                                *quick_add_status = Some(status);
                                qa_at_top = true;
                                quick_add_title.clear();
                                quick_add_tags.clear();
                                quick_add_tag_input.clear();
                            }
                        }
                    });
                });

                cui.add_space(6.0);
                let (hr, _) = cui.allocate_exact_size(vec2(inner_w, 1.0), Sense::hover());
                cui.painter().line_segment(
                    [hr.left_center(), hr.right_center()],
                    Stroke::new(1.0_f32, with_alpha(p.border_hairline, 0.75)),
                );
                cui.add_space(8.0);
                if is_top_open {
                    render_quick_add_control(
                        cui,
                        p,
                        lang,
                        s,
                        status,
                        inner_w,
                        "top",
                        quick_add_status,
                        &mut qa_at_top,
                        quick_add_title,
                        quick_add_tags,
                        quick_add_tag_input,
                        available_tags,
                        &mut actions,
                    );
                    cui.add_space(8.0);
                } else {
                    reset_focus_gate(
                        cui.ctx(),
                        egui::Id::new(("board_qa_foc", status, "top")),
                    );
                }

                let mut v_sa = egui::ScrollArea::vertical()
                    .id_salt(("kanban_col", status.sort_rank()))
                    .drag_to_scroll(!dnd.is_dragging())
                    .auto_shrink([false, true])
                    .max_height((board_bottom - cui.cursor().top() - 8.0).max(160.0));
                if let Some(next_y) = dnd.vertical_autoscroll_offset(cui.ctx(), &col_id) {
                    v_sa = v_sa.vertical_scroll_offset(next_y);
                }
                let dragged_id = dnd.dragged_card_id().map(str::to_string);
                let hidden_id = dnd.hidden_card_id().map(str::to_string);
                let drag_h = dnd.dragged_card_height();
                let dnd_active = dnd.is_active();
                let dnd_dragging = dnd.is_dragging();

                let v_out = v_sa.show(cui, |ui| {
                        ui.set_width(inner_w);
                        let scroll_origin_y = ui.cursor().top();
                        let others: Vec<(usize, &NoteMeta)> = items
                            .iter()
                            .copied()
                            .enumerate()
                            .filter(|(_, it)| dragged_id.as_deref() != Some(it.path.as_str()))
                            .collect();

                        let mut base_y = scroll_origin_y;
                        for (_, it) in &others {
                            let h = dnd.card_heights.get(&it.path).copied().unwrap_or(76.0);
                            col_base_cards.push((it.path.clone(), base_y, h));
                            base_y += h + 8.0;
                        }

                        let others_len = others.len();
                        for slot_idx in 0..=others_len {
                            let gap_t = dnd.slot_gap_factor(&col_id, slot_idx);
                            if gap_t > 0.01 {
                                let (ph, _) = ui.allocate_exact_size(
                                    vec2(inner_w, (drag_h * gap_t).max(4.0)),
                                    Sense::hover(),
                                );
                                if dnd.is_active_target_slot(&col_id, slot_idx) {
                                    ui.painter().rect_filled(
                                        ph,
                                        Rounding::same(10.0),
                                        with_alpha(p.accent, gap_t * 0.08),
                                    );
                                    let bar_y = ph.center().y;
                                    ui.painter().line_segment(
                                        [
                                            pos2(ph.left() + 12.0, bar_y),
                                            pos2(ph.right() - 12.0, bar_y),
                                        ],
                                        Stroke::new(2.0_f32, with_alpha(p.accent, gap_t * 0.92)),
                                    );
                                    ui.painter().circle_filled(
                                        pos2(ph.left() + 12.0, bar_y),
                                        3.5 * gap_t,
                                        with_alpha(p.accent, gap_t),
                                    );
                                    ui.painter().circle_filled(
                                        pos2(ph.right() - 12.0, bar_y),
                                        3.5 * gap_t,
                                        with_alpha(p.accent, gap_t),
                                    );
                                } else {
                                    ui.painter().rect_filled(
                                        ph,
                                        Rounding::same(10.0),
                                        with_alpha(p.bg_elevated, gap_t * 0.24),
                                    );
                                    draw_dashed_board_rect(
                                        ui.painter(),
                                        ph,
                                        10.0,
                                        Stroke::new(1.25_f32, with_alpha(p.border_strong, gap_t * 0.52)),
                                        6.0,
                                        4.0,
                                    );
                                }
                                ui.add_space(10.0 * gap_t);
                            }

                            if slot_idx < others_len {
                                let (orig_idx, item) = others[slot_idx];
                                let drag_id = ui.make_persistent_id(("kanban_card", &item.path));
                                if hidden_id.as_deref() == Some(item.path.as_str()) {
                                    let h = dnd.card_heights.get(&item.path).copied().unwrap_or(drag_h);
                                    let (r, _) = ui.allocate_exact_size(vec2(inner_w, h), Sense::hover());
                                    dnd.resolve_settling_rect(&item.path, r);
                                    ui.add_space(8.0);
                                    continue;
                                }

                                let card_top = ui.next_widget_position();
                                let est_h = dnd.card_heights.get(&item.path).copied().unwrap_or(76.0);
                                let est_rect =
                                    egui::Rect::from_min_size(card_top, vec2(inner_w, est_h));
                                let is_hov = !dnd_active && ui.rect_contains_pointer(est_rect);
                                let hov_t = anim_bool_eased(
                                    ui.ctx(),
                                    drag_id.with("hov"),
                                    is_hov,
                                    0.11,
                                );

                                let reorder_caps = if !is_sorted
                                    && !dnd_active
                                    && others_len > 1
                                    && hov_t > 0.18
                                {
                                    Some((
                                        slot_idx > 0,
                                        slot_idx + 1 < others_len,
                                        hov_t,
                                    ))
                                } else {
                                    None
                                };
                                let mut reord_hit = None;
                                let inner_resp = paint_card_contents(
                                    ui,
                                    p,
                                    item,
                                    inner_w,
                                    hov_t,
                                    false,
                                    reorder_caps,
                                    &mut reord_hit,
                                );
                                dnd.card_heights
                                    .insert(item.path.clone(), inner_resp.rect.height());

                                if let Some(delta) = reord_hit {
                                    actions.push(UiAction::ShiftNoteInBoard {
                                        path: item.path.clone(),
                                        delta,
                                    });
                                } else if !dnd_dragging {
                                    let card_resp = ui.interact(
                                        inner_resp.rect,
                                        drag_id,
                                        Sense::click_and_drag(),
                                    );
                                    if card_resp.hovered() {
                                        card_resp.clone().on_hover_cursor(CursorIcon::Grab);
                                    }
                                    if card_resp.is_pointer_button_down_on()
                                        && ui.input(|i| i.pointer.primary_pressed())
                                    {
                                        if let Some(press_pos) = ptr_pos {
                                            dnd.try_begin_pending(
                                                &item.path,
                                                &col_id,
                                                orig_idx,
                                                inner_resp.rect,
                                                press_pos,
                                            );
                                        }
                                    }
                                    if card_resp.clicked() {
                                        actions.push(UiAction::OpenNote(item.path.clone()));
                                        actions.push(UiAction::SetViewMode(ViewMode::Editor));
                                    }
                                }
                                ui.add_space(8.0);
                            }
                        }

                        if others_len == 0 && dnd_dragging && !dnd.is_target_col(&col_id) {
                            let (hint_rect, _) =
                                ui.allocate_exact_size(vec2(inner_w, 56.0), Sense::hover());
                            ui.painter().rect_stroke(
                                hint_rect,
                                Rounding::same(8.0),
                                Stroke::new(1.0_f32, with_alpha(p.accent, 0.65)),
                            );
                            let g = ui.painter().layout_no_wrap(
                                s.drop_here_hint.to_string(),
                                font_body(12.0),
                                p.accent_text,
                            );
                            ui.painter().galley(
                                pos2(
                                    hint_rect.center().x - g.size().x * 0.5,
                                    hint_rect.center().y - g.size().y * 0.5,
                                ),
                                g,
                                p.accent_text,
                            );
                            ui.add_space(6.0);
                        }

                        if !is_top_open {
                            ui.add_space(2.0);
                            render_quick_add_control(
                                ui,
                                p,
                                lang,
                                s,
                                status,
                                inner_w,
                                "inline",
                                quick_add_status,
                                &mut qa_at_top,
                                quick_add_title,
                                quick_add_tags,
                                quick_add_tag_input,
                                available_tags,
                                &mut actions,
                            );
                        }
                        ui.add_space(10.0);
                    });
                col_scroll_rect = v_out.inner_rect;
                col_scroll_offset_y = v_out.state.offset.y;
            }

            let col_resp = col_frame.allocate_space(col_ui);
            let drop_zone_rect = Rect::from_min_max(
                col_resp.rect.min,
                pos2(
                    col_resp.rect.right(),
                    board_bottom.max(col_resp.rect.bottom()),
                ),
            );
            new_columns.insert(
                col_id.clone(),
                ColumnGeometry {
                    col_id: col_id.clone(),
                    drop_rect: drop_zone_rect,
                    scroll_rect: col_scroll_rect,
                    scroll_offset_y: col_scroll_offset_y,
                    base_cards: col_base_cards,
                },
            );
            let can_drop = dnd.is_dragging()
                && dnd.is_target_col(&col_id)
                && !dnd.is_same_col_sorted_locked(&col_id, is_sorted);
            let drop_t = anim_bool_eased(
                col_ui.ctx(),
                col_ui.make_persistent_id(("kanban_drop", status.sort_rank())),
                can_drop,
                0.12,
            );
            if drop_t > 0.01 {
                col_frame.frame.stroke =
                    Stroke::new(1.25_f32, with_alpha(dot_col, 0.55 * drop_t));
                col_frame.frame.fill = with_alpha(dot_col, 0.07 * drop_t);
            }
            col_frame.paint(col_ui);
        }
    });

    ui.style_mut().interaction.selectable_labels = prev_selectable;
    dnd.columns = new_columns;
    if let Some((ghost_id, ghost_min, ghost_size)) = dnd.active_ghost() {
        if let Some(item) = notes.iter().find(|n| n.path == ghost_id) {
            let mut dummy_reord = None;
            egui::Area::new(ui.make_persistent_id(("kboard_ghost_area", ghost_id)))
                .order(Order::Tooltip)
                .interactable(false)
                .fixed_pos(ghost_min)
                .show(ui.ctx(), |ui| {
                    ui.set_width(ghost_size.x);
                    paint_card_contents(
                        ui,
                        p,
                        item,
                        ghost_size.x,
                        1.0,
                        true,
                        None,
                        &mut dummy_reord,
                    );
                });
        }
    }
    dnd.store(ui.ctx(), dnd_id);

    ui.ctx().data_mut(|d| d.insert_temp(qa_top_id, qa_at_top));
    actions
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_tabbed_board(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    mobile_tab: &mut NoteStatus,
    grouped: &HashMap<NoteStatus, Vec<&NoteMeta>>,
    quick_add_status: &mut Option<NoteStatus>,
    quick_add_title: &mut String,
    quick_add_tags: &mut Vec<String>,
    quick_add_tag_input: &mut String,
    available_tags: &[String],
    actions: &mut Vec<UiAction>,
) {
    let avail_w = ui.available_width().max(260.0);
    let statuses = NoteStatus::ALL;
    let active_idx = statuses
        .iter()
        .position(|st| st == mobile_tab)
        .unwrap_or(0);
    let anim_idx = anim_value_smooth(
        ui.ctx(),
        ui.make_persistent_id("board_mobile_tab_glide"),
        active_idx as f32,
        0.18,
    );

    // 3-tab status switcher with gliding pill indicator
    let (bar_rect, _) = ui.allocate_exact_size(vec2(avail_w, 34.0), Sense::hover());
    ui.painter()
        .rect_filled(bar_rect, Rounding::same(8.0), p.bg_sidebar);
    ui.painter().rect_stroke(
        bar_rect,
        Rounding::same(8.0),
        Stroke::new(1.0_f32, p.border_hairline),
    );

    let pad = 3.0;
    let seg_w = ((bar_rect.width() - pad * 2.0) / 3.0).max(40.0);
    let pill_rect = egui::Rect::from_min_size(
        pos2(bar_rect.left() + pad + anim_idx * seg_w, bar_rect.top() + pad),
        vec2(seg_w, bar_rect.height() - pad * 2.0),
    );
    ui.painter()
        .rect_filled(pill_rect, Rounding::same(6.0), p.bg_elevated);
    ui.painter().rect_stroke(
        pill_rect,
        Rounding::same(6.0),
        Stroke::new(1.0_f32, p.border_subtle),
    );

    for (idx, status) in statuses.into_iter().enumerate() {
        let tab_rect = egui::Rect::from_min_size(
            pos2(bar_rect.left() + pad + idx as f32 * seg_w, bar_rect.top() + pad),
            vec2(seg_w, bar_rect.height() - pad * 2.0),
        );
        let resp = ui
            .interact(
                tab_rect,
                ui.make_persistent_id(("board_mob_tab", status.sort_rank())),
                Sense::click(),
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        let active = *mobile_tab == status;
        let count = grouped.get(&status).map(Vec::len).unwrap_or(0);
        let (_, dot_col, _) = p.status_colors(status);
        let text_col = if active {
            p.text_primary
        } else if resp.hovered() {
            p.text_primary
        } else {
            p.text_secondary
        };

        let label_text = format!("{} ({count})", status_label(lang, status));
        let mut g = ui
            .painter()
            .layout_no_wrap(label_text.clone(), font_medium(11.5), text_col);
        if g.size().x + 18.0 > tab_rect.width() {
            g = ui.painter().layout_no_wrap(
                format!("{count}"),
                font_medium(11.5),
                text_col,
            );
        }
        let total_w = 7.0 + 5.0 + g.size().x;
        let start_x = tab_rect.center().x - total_w * 0.5;
        ui.painter()
            .circle_filled(pos2(start_x + 3.5, tab_rect.center().y), 3.0, dot_col);
        ui.painter().galley(
            pos2(start_x + 12.0, tab_rect.center().y - g.size().y * 0.5),
            g,
            text_col,
        );

        if resp.clicked() {
            *mobile_tab = status;
        }
    }

    ui.add_space(10.0);

    let active_status = *mobile_tab;
    let items = grouped
        .get(&active_status)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    egui::ScrollArea::vertical()
        .id_salt(("kanban_mobile_scroll", active_status.sort_rank()))
        .show(ui, |ui| {
            let card_w = ui.available_width().max(220.0);
            if items.is_empty() {
                egui::Frame::none()
                    .fill(p.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(10.0))
                    .inner_margin(Margin::symmetric(16.0, 24.0))
                    .show(ui, |ui| {
                        ui.set_width((card_w - 32.0).max(160.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                egui::RichText::new(s.palette_no_results)
                                    .font(font_body(13.0))
                                    .color(p.text_muted),
                            );
                        });
                    });
                ui.add_space(10.0);
            }

            let count = items.len();
            for (idx, item) in items.into_iter().enumerate() {
                render_mobile_board_card(
                    ui,
                    p,
                    lang,
                    item,
                    idx > 0,
                    idx + 1 < count,
                    card_w,
                    actions,
                );
                ui.add_space(8.0);
            }

            let mut mob_top = false;
            render_quick_add_control(
                ui,
                p,
                lang,
                s,
                active_status,
                card_w,
                "mob",
                quick_add_status,
                &mut mob_top,
                quick_add_title,
                quick_add_tags,
                quick_add_tag_input,
                available_tags,
                actions,
            );
            ui.add_space(12.0);
        });
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_board_card(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    item: &NoteMeta,
    can_up: bool,
    can_down: bool,
    card_w: f32,
    actions: &mut Vec<UiAction>,
) {
    let content_w = (card_w - 24.0).max(160.0);
    egui::Frame::none()
        .fill(p.bg_card)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            ui.set_width(content_w);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 18.0), Sense::hover());
                draw_icon(
                    ui.painter(),
                    egui::Rect::from_center_size(ir.center(), vec2(13.0, 13.0)),
                    Icon::Document,
                    p.text_muted,
                );
                let resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(&item.title)
                            .font(font_semibold(14.0))
                            .color(p.text_primary),
                    )
                    .wrap()
                    .sense(Sense::click()),
                );
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    actions.push(UiAction::OpenNote(item.path.clone()));
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                }
            });

            if !item.excerpt.is_empty() {
                ui.add_space(4.0);
                let ex = ui.add(
                    egui::Label::new(
                        egui::RichText::new(&item.excerpt)
                            .font(font_body(12.0))
                            .color(p.text_secondary),
                    )
                    .wrap()
                    .sense(Sense::click()),
                );
                if ex.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    actions.push(UiAction::OpenNote(item.path.clone()));
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                }
            }

            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                if let Some(new_st) = status_pill_picker(
                    ui,
                    p,
                    lang,
                    ("board_mob_st", &item.path),
                    item.status.unwrap_or(NoteStatus::Todo),
                ) {
                    if Some(new_st) != item.status {
                        actions.push(UiAction::UpdateNoteStatus {
                            path: item.path.clone(),
                            status: Some(new_st),
                        });
                    }
                }
                for tag in item.tags.iter().take(3) {
                    let (resp, _) = tag_chip(ui, p, tag, false);
                    if resp.clicked() {
                        actions.push(UiAction::SetTagFilter(Some(tag.clone())));
                    }
                }
                if can_up && mobile_shift_btn(ui, p, "↑").clicked() {
                    actions.push(UiAction::ShiftNoteInBoard {
                        path: item.path.clone(),
                        delta: -1,
                    });
                }
                if can_down && mobile_shift_btn(ui, p, "↓").clicked() {
                    actions.push(UiAction::ShiftNoteInBoard {
                        path: item.path.clone(),
                        delta: 1,
                    });
                }
            });
        });
}

fn mobile_shift_btn(ui: &mut Ui, p: &ThemePalette, glyph: &str) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(22.0, 20.0), Sense::click());
    let bg = if resp.hovered() { p.bg_hover } else { p.bg_elevated };
    ui.painter().rect_filled(rect, Rounding::same(5.0), bg);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(5.0),
        Stroke::new(1.0_f32, p.border_hairline),
    );
    let fg = if resp.hovered() { p.text_primary } else { p.text_secondary };
    let g = ui.painter().layout_no_wrap(glyph.to_string(), font_mono(11.0), fg);
    ui.painter().galley(
        pos2(rect.center().x - g.size().x * 0.5, rect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

#[allow(clippy::too_many_arguments)]
fn render_quick_add_control(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    status: NoteStatus,
    inner_w: f32,
    slot_tag: &str,
    quick_add_status: &mut Option<NoteStatus>,
    qa_at_top: &mut bool,
    quick_add_title: &mut String,
    quick_add_tags: &mut Vec<String>,
    quick_add_tag_input: &mut String,
    available_tags: &[String],
    actions: &mut Vec<UiAction>,
) {
    let qa_gate_id = egui::Id::new(("board_qa_foc", status, slot_tag));
    if *quick_add_status == Some(status) {
        let mut composer = egui::Frame::none()
            .fill(p.bg_card)
            .stroke(Stroke::new(1.2_f32, p.accent))
            .rounding(Rounding::same(9.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::same(10.0))
            .begin(ui);
        {
            let fui = &mut composer.content_ui;
            fui.set_width((inner_w - 20.0).max(80.0));
            let resp = styled_text_input(
                fui,
                p,
                quick_add_title,
                s.new_note_placeholder,
                false,
                f32::INFINITY,
            );
            focus_once(fui.ctx(), qa_gate_id, true, resp.id);
            let title_enter = resp.has_focus() && fui.input(|i| i.key_pressed(egui::Key::Enter));

            fui.add_space(6.0);
            if !quick_add_tags.is_empty() {
                fui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    let mut remove_idx: Option<usize> = None;
                    for (ti, tag) in quick_add_tags.iter().enumerate() {
                        let (_, rm) = tag_chip(ui, p, tag, true);
                        if rm {
                            remove_idx = Some(ti);
                        }
                    }
                    if let Some(ti) = remove_idx {
                        quick_add_tags.remove(ti);
                    }
                });
                fui.add_space(4.0);
            }

            if let Some(new_tag) = tag_autocomplete_input(
                fui,
                p,
                lang,
                ("board_qa_tag", status, slot_tag),
                quick_add_tag_input,
                s.add_tag_hint,
                f32::INFINITY,
                available_tags,
                quick_add_tags,
            ) {
                let clean = normalize_tag_input(&new_tag);
                if !clean.is_empty()
                    && !quick_add_tags
                        .iter()
                        .any(|t| normalize_tag_input(t) == clean)
                {
                    quick_add_tags.push(clean);
                }
            }

            let esc = fui.input(|i| i.key_pressed(egui::Key::Escape));
            fui.add_space(7.0);
            fui.horizontal(|ui| {
                if (primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked()
                    || title_enter)
                    && !quick_add_title.trim().is_empty()
                {
                    let t = quick_add_title.trim().to_string();
                    let tags = std::mem::take(quick_add_tags);
                    actions.push(UiAction::CreateNote {
                        title: t,
                        status: Some(status),
                        tags,
                    });
                    quick_add_title.clear();
                    quick_add_tags.clear();
                    quick_add_tag_input.clear();
                    *quick_add_status = None;
                    *qa_at_top = false;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() || esc {
                        quick_add_title.clear();
                        quick_add_tags.clear();
                        quick_add_tag_input.clear();
                        *quick_add_status = None;
                        *qa_at_top = false;
                    }
                });
            });
        }
        let resp = composer.end(ui);
        if slot_tag == "inline" && resp.hovered() {
            resp.scroll_to_me(Some(egui::Align::Max));
        }
    } else {
        reset_focus_gate(ui.ctx(), qa_gate_id);
        let (add_rect, add_resp) = ui.allocate_exact_size(vec2(inner_w, 32.0), Sense::click());
        let hov = anim_bool_eased(
            ui.ctx(),
            add_resp.id.with("hov"),
            add_resp.hovered(),
            0.10,
        );
        if hov > 0.01 {
            ui.painter().rect_filled(
                add_rect,
                Rounding::same(8.0),
                with_alpha(p.bg_elevated, 0.75 * hov),
            );
            ui.painter().rect_stroke(
                add_rect,
                Rounding::same(8.0),
                Stroke::new(1.0_f32, with_alpha(p.border_subtle, hov)),
            );
        }
        let fg = lerp_color(p.text_muted, p.text_primary, hov);
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(add_rect.left() + 14.0, add_rect.center().y), vec2(12.0, 12.0)),
            Icon::Plus,
            fg,
        );
        let g = ui
            .painter()
            .layout_no_wrap(s.add_card_btn.to_string(), font_medium(12.0), fg);
        ui.painter().galley(
            pos2(
                add_rect.left() + 26.0,
                add_rect.center().y - g.size().y * 0.5,
            ),
            g,
            fg,
        );
        if add_resp
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *quick_add_status = Some(status);
            *qa_at_top = false;
            quick_add_title.clear();
            quick_add_tags.clear();
            quick_add_tag_input.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_card_contents(
    ui: &mut Ui,
    p: &ThemePalette,
    item: &NoteMeta,
    card_w: f32,
    hover_t: f32,
    floating: bool,
    reorder_caps: Option<(bool, bool, f32)>,
    reorder_out: &mut Option<isize>,
) -> egui::Response {
    let fill = if floating {
        p.bg_elevated
    } else {
        lerp_color(p.bg_card, p.bg_elevated, hover_t * 0.55)
    };
    let border = if floating {
        p.accent
    } else {
        lerp_color(p.border_subtle, with_alpha(p.accent, 0.78), hover_t)
    };
    let rest_shadow = egui::Shadow {
        offset: p.shadow_elevated.offset,
        blur: p.shadow_elevated.blur,
        spread: p.shadow_elevated.spread,
        color: with_alpha(p.shadow_elevated.color, 0.55 + 0.45 * hover_t),
    };
    let frame = egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(if floating { 1.5_f32 } else { 1.0_f32 }, border))
        .rounding(Rounding::same(10.0))
        .shadow(if floating { p.shadow_floating } else { rest_shadow })
        .inner_margin(Margin::symmetric(14.0, 12.0));

    let content_w = (card_w - 28.0).max(80.0);

    let resp = frame
        .show(ui, |ui| {
            ui.set_width(content_w);
            ui.set_max_width(content_w);
            let top_right_x = ui.cursor().left() + content_w;
            let top_y = ui.cursor().top();

            // Title row with explicit wrap width so text never collapses vertically
            let mut title_job = egui::text::LayoutJob::single_section(
                item.title.clone(),
                egui::TextFormat {
                    font_id: font_semibold(13.5),
                    color: p.text_primary,
                    line_height: Some(18.0),
                    ..Default::default()
                },
            );
            title_job.wrap = egui::text::TextWrapping {
                max_width: (content_w - 20.0).max(60.0),
                max_rows: 2,
                break_anywhere: false,
                overflow_character: Some('…'),
            };
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let (ir, _) = ui.allocate_exact_size(vec2(13.0, 17.0), Sense::hover());
                draw_icon(
                    ui.painter(),
                    egui::Rect::from_center_size(ir.center(), vec2(12.5, 12.5)),
                    Icon::Document,
                    p.text_muted,
                );
                ui.label(title_job);
            });

            if !item.excerpt.is_empty() {
                ui.add_space(5.0);
                let mut job = egui::text::LayoutJob::single_section(
                    item.excerpt.clone(),
                    egui::TextFormat {
                        font_id: font_body(12.0),
                        color: p.text_secondary,
                        line_height: Some(17.0),
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: content_w,
                    max_rows: 2,
                    break_anywhere: false,
                    overflow_character: Some('…'),
                };
                ui.label(job);
            }

            if !item.tags.is_empty() {
                ui.add_space(7.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(5.0, 4.0);
                    for tag in item.tags.iter().take(3) {
                        let _ = tag_chip(ui, p, tag, false);
                    }
                });
            }

            if let Some((can_up, can_down, alpha)) = reorder_caps {
                if !floating && (can_up || can_down) {
                    let mut rx = top_right_x + 2.0;
                    let pill_y = top_y - 2.0;
                    let btn_w = 18.0_f32;
                    let pill_w = if can_up && can_down { 38.0 } else { 18.0 };
                    let backdrop = egui::Rect::from_min_size(
                        pos2(rx - pill_w - 2.0, pill_y - 2.0),
                        vec2(pill_w + 4.0, 22.0),
                    );
                    ui.painter().rect_filled(
                        backdrop,
                        Rounding::same(6.0),
                        with_alpha(p.bg_elevated, 0.92 * alpha),
                    );
                    ui.painter().rect_stroke(
                        backdrop,
                        Rounding::same(6.0),
                        Stroke::new(1.0_f32, with_alpha(p.border_subtle, 0.75 * alpha)),
                    );

                    for &(enabled, delta, lbl) in &[(can_down, 1isize, "↓"), (can_up, -1isize, "↑")] {
                        if !enabled {
                            continue;
                        }
                        rx -= btn_w;
                        let br = egui::Rect::from_min_size(pos2(rx, pill_y), vec2(btn_w, 18.0));
                        let bresp = ui.interact(
                            br,
                            ui.make_persistent_id(("kb_reord", &item.path, delta)),
                            Sense::click(),
                        );
                        if bresp.hovered() {
                            ui.painter().rect_filled(br, Rounding::same(4.5), p.accent_soft);
                        }
                        let fg = if bresp.hovered() {
                            p.accent_text
                        } else {
                            with_alpha(p.text_secondary, alpha)
                        };
                        let g = ui.painter().layout_no_wrap(lbl.to_string(), font_mono(11.0), fg);
                        ui.painter().galley(
                            pos2(br.center().x - g.size().x * 0.5, br.center().y - g.size().y * 0.5),
                            g,
                            fg,
                        );
                        if bresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            *reorder_out = Some(delta);
                        }
                        rx -= 2.0;
                    }
                }
            }
        })
        .response;
    if p.is_dark {
        let sheen_alpha = if floating { 28 } else { (14.0 + 12.0 * hover_t) as u8 };
        let r = resp.rect;
        ui.painter().line_segment(
            [pos2(r.left() + 10.0, r.top() + 1.0), pos2(r.right() - 10.0, r.top() + 1.0)],
            Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, sheen_alpha)),
        );
    }
    resp
}

fn muted_board_icon_btn(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Icon,
    tooltip: &str,
    active: bool,
    vis_alpha: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
    if active || hov > 0.01 {
        let fill = if active {
            p.accent_soft
        } else {
            with_alpha(p.bg_hover, hov)
        };
        ui.painter().rect_filled(rect, Rounding::same(6.0), fill);
    }
    let alpha = vis_alpha.max(hov);
    if active || alpha >= 0.02 {
        let fg = if active {
            p.accent_text
        } else {
            with_alpha(lerp_color(p.text_muted, p.text_primary, hov), alpha)
        };
        draw_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(13.5, 13.5)),
            icon,
            fg,
        );
    }
    resp.on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

fn draw_dashed_board_rect(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let r = rect.shrink(0.5);
    let ins = (rounding * 0.45).min(6.0);
    let edges = [
        (pos2(r.left() + ins, r.top()), pos2(r.right() - ins, r.top())),
        (pos2(r.right(), r.top() + ins), pos2(r.right(), r.bottom() - ins)),
        (pos2(r.right() - ins, r.bottom()), pos2(r.left() + ins, r.bottom())),
        (pos2(r.left(), r.bottom() - ins), pos2(r.left(), r.top() + ins)),
    ];
    let step = dash_len + gap_len;
    for (a, b) in edges {
        let diff = b - a;
        let len = diff.length();
        if len < 1.0 {
            continue;
        }
        let dir = diff / len;
        let mut t = 0.0_f32;
        while t < len {
            let t_end = (t + dash_len).min(len);
            painter.line_segment([a + dir * t, a + dir * t_end], stroke);
            t += step;
        }
    }
}
