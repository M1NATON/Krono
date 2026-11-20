use crate::i18n::{status_label, strings, Lang, Strings};
use crate::ui::components::{
    focus_once, icon_button, primary_button, reset_focus_gate, secondary_button, status_pill_picker,
    styled_text_input, tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, lerp_color, with_alpha};
use crate::ui::theme::{
    font_body, font_medium, font_mono, font_semibold, font_serif_bold, ThemePalette,
};
use crate::ui::{LayoutClass, UiAction, ViewMode};
use eframe::egui::{
    self, pos2, vec2, Align, CursorIcon, DragAndDrop, LayerId, Layout, Margin, Order, Rounding,
    Sense, Stroke, Ui,
};
use krono_core::{
    collection::CollectionViewKind,
    note::{NoteMeta, NoteStatus},
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct DragCardPayload {
    path: String,
    height: f32,
}

#[allow(clippy::too_many_arguments)]
pub fn render_board_view(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    mobile_tab: &mut NoteStatus,
    notes: &[NoteMeta],
    active_tag: Option<&str>,
    quick_add_status: &mut Option<NoteStatus>,
    quick_add_title: &mut String,
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
            if let Some(tag) = active_tag {
                ui.add_space(10.0);
                let (resp, rm) = tag_chip(ui, p, tag, true);
                if rm || resp.clicked() {
                    actions.push(UiAction::SetTagFilter(None));
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
            });
        });
        ui.add_space(14.0);
    } else if let Some(tag) = active_tag {
        ui.horizontal(|ui| {
            let (resp, rm) = tag_chip(ui, p, tag, true);
            if rm || resp.clicked() {
                actions.push(UiAction::SetTagFilter(None));
            }
        });
        ui.add_space(8.0);
    }

    let mut grouped: HashMap<NoteStatus, Vec<&NoteMeta>> = HashMap::new();
    for st in NoteStatus::ALL {
        grouped.insert(st, Vec::new());
    }
    for n in notes {
        if let Some(tag) = active_tag {
            if !n.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                continue;
            }
        }
        grouped.entry(n.status).or_default().push(n);
    }
    for list in grouped.values_mut() {
        list.sort_by(|a, b| {
            let ar = if a.board_order > 0 { 0 } else { 1 };
            let br = if b.board_order > 0 { 0 } else { 1 };
            ar.cmp(&br)
                .then_with(|| a.board_order.cmp(&b.board_order))
                .then_with(|| b.modified.cmp(&a.modified))
        });
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
            &mut actions,
        );
        return actions;
    }

    let avail_h = ui.available_height().max(320.0);
    let active_drag = DragAndDrop::payload::<DragCardPayload>(ui.ctx());
    let qa_top_id = egui::Id::new("board_qa_at_top_st");
    let mut qa_at_top = ui.ctx().data(|d| d.get_temp::<bool>(qa_top_id)).unwrap_or(false);
    if quick_add_status.is_none() {
        qa_at_top = false;
    }
    let ptr_pos = ui.ctx().pointer_interact_pos();

    ui.columns(3, |cols| {
        for (idx, status) in NoteStatus::ALL.into_iter().enumerate() {
            let col_ui = &mut cols[idx];
            let col_total_w = col_ui.available_width().max(180.0);
            let inner_w = (col_total_w - 24.0).max(140.0);
            let (pill_bg, dot_col, text_col) = p.status_colors(status);
            let items = grouped.get(&status).map(Vec::as_slice).unwrap_or(&[]);
            let is_top_open = *quick_add_status == Some(status) && qa_at_top;

            let prev_col_rect: Option<egui::Rect> = col_ui.data(|d| {
                d.get_temp(col_ui.make_persistent_id(("kboard_col_rect", status.sort_rank())))
            });
            let prev_card_rects: Vec<(String, f32, f32)> = col_ui
                .data(|d| {
                    d.get_temp(col_ui.make_persistent_id(("kboard_card_rects", status.sort_rank())))
                })
                .unwrap_or_default();

            let mut col_frame = egui::Frame::none()
                .fill(p.bg_sidebar)
                .stroke(Stroke::new(1.0_f32, p.border_hairline))
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::same(12.0))
                .begin(col_ui);

            let mut hovered_insert_before: Option<String> = None;
            let mut new_card_rects: Vec<(String, f32, f32)> = Vec::with_capacity(items.len());
            {
                let cui = &mut col_frame.content_ui;
                cui.set_width(inner_w);
                cui.set_max_width(inner_w);
                cui.set_min_height((avail_h - 28.0).max(260.0));

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
                        if icon_button(ui, p, Icon::Plus, s.add_card_btn, is_top_open).clicked() {
                            if is_top_open {
                                *quick_add_status = None;
                                qa_at_top = false;
                                quick_add_title.clear();
                            } else {
                                *quick_add_status = Some(status);
                                qa_at_top = true;
                                quick_add_title.clear();
                            }
                        }
                    });
                });

                cui.add_space(8.0);
                if is_top_open {
                    render_quick_add_control(
                        cui,
                        p,
                        s,
                        status,
                        inner_w,
                        "top",
                        quick_add_status,
                        &mut qa_at_top,
                        quick_add_title,
                        &mut actions,
                    );
                    cui.add_space(8.0);
                } else {
                    reset_focus_gate(
                        cui.ctx(),
                        egui::Id::new(("board_qa_foc", status, "top")),
                    );
                }

                let hovering_this_col = active_drag.is_some()
                    && ptr_pos.is_some_and(|pt| prev_col_rect.is_some_and(|r| r.contains(pt)));
                let drag_h = active_drag.as_ref().map(|d| d.height).unwrap_or(76.0);
                let drag_path = active_drag.as_ref().map(|d| d.path.as_str());
                let others: Vec<&NoteMeta> = items
                    .iter()
                    .copied()
                    .filter(|it| Some(it.path.as_str()) != drag_path)
                    .collect();
                let target_slot_idx: Option<usize> = if hovering_this_col {
                    let pt = ptr_pos.unwrap();
                    let mut count = 0usize;
                    for it in &others {
                        if let Some((_, cy, _)) =
                            prev_card_rects.iter().find(|(id, _, _)| id == &it.path)
                        {
                            if *cy < pt.y {
                                count += 1;
                            }
                        }
                    }
                    hovered_insert_before = others.get(count).map(|it| it.path.clone());
                    Some(count)
                } else {
                    None
                };

                egui::ScrollArea::vertical()
                    .id_salt(("kanban_col", status.sort_rank()))
                    .auto_shrink([false, true])
                    .max_height((avail_h - 140.0).max(160.0))
                    .show(cui, |ui| {
                        ui.set_width(inner_w);
                        let scroll_origin_y = ui.cursor().top();
                        let scroll_clip = ui.clip_rect();

                        // Floating tooltip ghost for dragged card originating from this column
                        for item in items {
                            let drag_id = ui.make_persistent_id(("kanban_card", &item.path));
                            if ui.ctx().is_being_dragged(drag_id) {
                                let cur_h = prev_card_rects
                                    .iter()
                                    .find(|(id, _, _)| id == &item.path)
                                    .map(|(_, _, h)| *h)
                                    .unwrap_or(76.0);
                                DragAndDrop::set_payload(
                                    ui.ctx(),
                                    DragCardPayload {
                                        path: item.path.clone(),
                                        height: cur_h,
                                    },
                                );
                                let layer_id = LayerId::new(Order::Tooltip, drag_id);
                                let mut dummy_reord = None;
                                let ir = ui.with_layer_id(layer_id, |ui| {
                                    ui.set_width(inner_w);
                                    paint_card_contents(
                                        ui,
                                        p,
                                        item,
                                        inner_w,
                                        1.0,
                                        true,
                                        None,
                                        &mut dummy_reord,
                                    )
                                });
                                if let Some(ptr) = ptr_pos {
                                    ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                                    let delta = ptr - ir.response.rect.center();
                                    ui.ctx().transform_layer_shapes(
                                        layer_id,
                                        egui::emath::TSTransform::from_translation(delta),
                                    );
                                    let ghost_top_y = ptr.y - ir.response.rect.height() * 0.5;
                                    ui.data_mut(|d| {
                                        d.insert_temp(
                                            ui.make_persistent_id(("kboard_ghost_top_y", &item.path)),
                                            ghost_top_y,
                                        );
                                    });
                                }
                            }
                        }

                        let others_len = others.len();
                        for slot_idx in 0..=others_len {
                            let gap_id =
                                ui.make_persistent_id(("kboard_slot_gap", status.sort_rank(), slot_idx));
                            let gap_t = if active_drag.is_some() {
                                anim_bool_eased(
                                    ui.ctx(),
                                    gap_id,
                                    hovering_this_col && target_slot_idx == Some(slot_idx),
                                    0.14,
                                )
                            } else {
                                let _ = ui.ctx().animate_bool_with_time(gap_id, false, 0.0);
                                0.0
                            };
                            if gap_t > 0.01 {
                                let (ph, _) = ui.allocate_exact_size(
                                    vec2(inner_w, (drag_h * gap_t).max(6.0)),
                                    Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    ph,
                                    Rounding::same(8.0),
                                    with_alpha(p.accent_soft, gap_t * 0.85),
                                );
                                ui.painter().rect_stroke(
                                    ph,
                                    Rounding::same(8.0),
                                    Stroke::new(1.4_f32, with_alpha(p.accent, gap_t)),
                                );
                                ui.painter().line_segment(
                                    [
                                        pos2(ph.left() + 14.0, ph.center().y),
                                        pos2(ph.right() - 14.0, ph.center().y),
                                    ],
                                    Stroke::new(2.0_f32, with_alpha(p.accent, gap_t * 0.7)),
                                );
                                ui.add_space(8.0 * gap_t);
                            }

                            if slot_idx < others_len {
                                let item = others[slot_idx];
                                let drag_id = ui.make_persistent_id(("kanban_card", &item.path));
                                let card_top = ui.next_widget_position();
                                let est_h = prev_card_rects
                                    .iter()
                                    .find(|(id, _, _)| id == &item.path)
                                    .map(|(_, _, h)| *h)
                                    .unwrap_or(76.0);
                                let est_rect =
                                    egui::Rect::from_min_size(card_top, vec2(inner_w, est_h));
                                let is_hov =
                                    active_drag.is_none() && ui.rect_contains_pointer(est_rect);
                                let hov_t = anim_bool_eased(
                                    ui.ctx(),
                                    drag_id.with("hov"),
                                    is_hov,
                                    0.11,
                                );

                                let local_y = card_top.y - scroll_origin_y;
                                let settle_id =
                                    ui.make_persistent_id(("kboard_settle_y", &item.path));
                                let last_rank: Option<u8> = ui.data(|d| {
                                    d.get_temp(ui.make_persistent_id((
                                        "kboard_last_col",
                                        &item.path,
                                    )))
                                });
                                let ghost_screen_y: Option<f32> = ui.data(|d| {
                                    d.get_temp(ui.make_persistent_id((
                                        "kboard_ghost_top_y",
                                        &item.path,
                                    )))
                                });
                                let anim_local_y = if active_drag.is_none() {
                                    if let Some(gsy) = ghost_screen_y {
                                        ui.data_mut(|d| {
                                            d.remove::<f32>(ui.make_persistent_id((
                                                "kboard_ghost_top_y",
                                                &item.path,
                                            )));
                                        });
                                        let seed = (gsy - scroll_origin_y)
                                            .clamp(local_y - 380.0, local_y + 380.0);
                                        ui.ctx().animate_value_with_time(settle_id, seed, 0.0);
                                        ui.ctx().animate_value_with_time(settle_id, local_y, 0.18)
                                    } else if last_rank == Some(status.sort_rank()) {
                                        ui.ctx().animate_value_with_time(settle_id, local_y, 0.18)
                                    } else {
                                        ui.ctx().animate_value_with_time(settle_id, local_y, 0.0)
                                    }
                                } else {
                                    ui.ctx().animate_value_with_time(settle_id, local_y, 0.0)
                                };
                                ui.data_mut(|d| {
                                    d.insert_temp(
                                        ui.make_persistent_id(("kboard_last_col", &item.path)),
                                        status.sort_rank(),
                                    );
                                });
                                let settle_dy = if active_drag.is_none() {
                                    (anim_local_y - local_y).clamp(-380.0, 380.0)
                                } else {
                                    0.0
                                };

                                let reorder_caps =
                                    if active_drag.is_none() && others_len > 1 && hov_t > 0.18 {
                                        Some((
                                            slot_idx > 0,
                                            slot_idx + 1 < others_len,
                                            hov_t,
                                        ))
                                    } else {
                                        None
                                    };
                                let mut reord_hit = None;
                                let inner_resp = if settle_dy.abs() > 0.6 {
                                    ui.ctx().request_repaint();
                                    let settle_layer =
                                        LayerId::new(Order::Middle, drag_id.with("settle"));
                                    let ir = ui.with_layer_id(settle_layer, |ui| {
                                        ui.set_clip_rect(scroll_clip);
                                        paint_card_contents(
                                            ui,
                                            p,
                                            item,
                                            inner_w,
                                            hov_t,
                                            false,
                                            reorder_caps,
                                            &mut reord_hit,
                                        )
                                    });
                                    ui.ctx().transform_layer_shapes(
                                        settle_layer,
                                        egui::emath::TSTransform::from_translation(vec2(
                                            0.0, settle_dy,
                                        )),
                                    );
                                    ir.inner
                                } else {
                                    paint_card_contents(
                                        ui,
                                        p,
                                        item,
                                        inner_w,
                                        hov_t,
                                        false,
                                        reorder_caps,
                                        &mut reord_hit,
                                    )
                                };
                                new_card_rects.push((
                                    item.path.clone(),
                                    inner_resp.rect.center().y,
                                    inner_resp.rect.height(),
                                ));

                                if let Some(delta) = reord_hit {
                                    actions.push(UiAction::ShiftNoteInBoard {
                                        path: item.path.clone(),
                                        delta,
                                    });
                                } else {
                                    let card_resp = ui.interact(
                                        inner_resp.rect,
                                        drag_id,
                                        Sense::click_and_drag(),
                                    );
                                    if card_resp.hovered() {
                                        card_resp.clone().on_hover_cursor(CursorIcon::Grab);
                                    }
                                    if card_resp.clicked() {
                                        actions.push(UiAction::OpenNote(item.path.clone()));
                                        actions.push(UiAction::SetViewMode(ViewMode::Editor));
                                    }
                                }
                                ui.add_space(8.0);
                            }
                        }

                        if others_len == 0 && active_drag.is_some() && !hovering_this_col {
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
                                s,
                                status,
                                inner_w,
                                "inline",
                                quick_add_status,
                                &mut qa_at_top,
                                quick_add_title,
                                &mut actions,
                            );
                        }
                    });
            }

            let col_resp = col_frame.allocate_space(col_ui);
            col_ui.data_mut(|d| {
                d.insert_temp(
                    col_ui.make_persistent_id(("kboard_col_rect", status.sort_rank())),
                    col_resp.rect,
                );
                d.insert_temp(
                    col_ui.make_persistent_id(("kboard_card_rects", status.sort_rank())),
                    new_card_rects,
                );
            });
            let can_drop = active_drag.is_some() && col_resp.contains_pointer();
            let drop_t = anim_bool_eased(
                col_ui.ctx(),
                col_ui.make_persistent_id(("kanban_drop", status.sort_rank())),
                can_drop,
                0.12,
            );
            if drop_t > 0.01 {
                col_frame.frame.stroke = Stroke::new(
                    1.0 + drop_t * 0.5,
                    lerp_color(p.border_hairline, p.accent, drop_t),
                );
                col_frame.frame.fill = lerp_color(p.bg_sidebar, p.bg_elevated, drop_t);
            }
            col_frame.paint(col_ui);

            if let Some(dropped) = col_resp.dnd_release_payload::<DragCardPayload>() {
                actions.push(UiAction::MoveNoteInBoard {
                    path: dropped.path.clone(),
                    status,
                    insert_before: hovered_insert_before,
                });
            }
        }
    });

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
                s,
                active_status,
                card_w,
                "mob",
                quick_add_status,
                &mut mob_top,
                quick_add_title,
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
                    item.status,
                ) {
                    if new_st != item.status {
                        actions.push(UiAction::UpdateNoteStatus {
                            path: item.path.clone(),
                            status: new_st,
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
    s: &Strings,
    status: NoteStatus,
    inner_w: f32,
    slot_tag: &str,
    quick_add_status: &mut Option<NoteStatus>,
    qa_at_top: &mut bool,
    quick_add_title: &mut String,
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
            let enter = fui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = fui.input(|i| i.key_pressed(egui::Key::Escape));
            fui.add_space(7.0);
            fui.horizontal(|ui| {
                if (primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked() || enter)
                    && !quick_add_title.trim().is_empty()
                {
                    let t = quick_add_title.trim().to_string();
                    actions.push(UiAction::CreateNote { title: t, status });
                    quick_add_title.clear();
                    *quick_add_status = None;
                    *qa_at_top = false;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() || esc {
                        quick_add_title.clear();
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
        let (add_rect, add_resp) = ui.allocate_exact_size(vec2(inner_w, 30.0), Sense::click());
        let hov = anim_bool_eased(
            ui.ctx(),
            add_resp.id.with("hov"),
            add_resp.hovered(),
            0.10,
        );
        if hov > 0.01 {
            ui.painter().rect_filled(
                add_rect,
                Rounding::same(6.0),
                lerp_color(p.bg_sidebar, p.bg_hover, hov),
            );
        }
        let fg = lerp_color(p.text_muted, p.text_primary, hov);
        let g = ui
            .painter()
            .layout_no_wrap(s.add_card_btn.to_string(), font_medium(12.0), fg);
        ui.painter().galley(
            pos2(
                add_rect.left() + 8.0,
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
    let fill = lerp_color(p.bg_card, p.bg_elevated, hover_t);
    let border = lerp_color(
        p.border_hairline,
        if floating { p.accent } else { p.border_strong },
        hover_t,
    );
    let mut frame = egui::Frame::none()
        .fill(fill)
        .stroke(Stroke::new(if floating { 1.3_f32 } else { 1.0_f32 }, border))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(12.0));
    if floating {
        frame = frame.shadow(p.shadow_floating);
    } else if hover_t > 0.02 {
        let mut hov_sh = p.shadow_elevated;
        hov_sh.color = with_alpha(hov_sh.color, hover_t * 0.7);
        frame = frame.shadow(hov_sh);
    }

    let content_w = (card_w - 24.0).max(80.0);

    frame
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
        .response
}
