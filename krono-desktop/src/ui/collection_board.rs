use crate::i18n::{strings, Lang};
use crate::ui::collection_view::{render_color_token_picker, CollectionEditorState};
use crate::ui::components::{
    focus_once, icon_button, primary_button, reset_focus_gate, secondary_button, styled_text_input,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{LayoutClass, UiAction};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, DragAndDrop, LayerId, Margin, Order, PopupCloseBehavior,
    Rect, Rounding, Sense, Stroke, Ui,
};
use krono_core::{
    collection::{
        record_matches_smart_filter, strip_link_target, ColorToken, ColumnDef, ColumnType,
        RecordItem,
    },
    daily::SimpleDate,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
struct CustomDragCard {
    record_id: String,
    from_opt_id: String,
    height: f32,
}

#[derive(Clone, Debug)]
struct ColumnMoveOp {
    record_id: String,
    target_opt_id: String,
    insert_before_id: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_custom_board(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let is_ru = lang == Lang::Ru;

    let Some(group_col_id) = state.doc.effective_group_column_id().map(str::to_string) else {
        render_missing_select_state(ui, p, lang, state, actions);
        return;
    };

    let Some(group_col) = state.doc.columns.iter().find(|c| c.id == group_col_id).cloned() else {
        render_missing_select_state(ui, p, lang, state, actions);
        return;
    };

    let q_lower = state.filter_query.trim().to_lowercase();
    let other_select_cols: Vec<ColumnDef> = state
        .doc
        .columns
        .iter()
        .filter(|c| c.col_type == ColumnType::Select && c.id != group_col_id)
        .cloned()
        .collect();
    let date_col_id: Option<String> = state
        .doc
        .columns
        .iter()
        .find(|c| c.col_type == ColumnType::Date)
        .map(|c| c.id.clone());
    let note_link_cols: Vec<String> = state
        .doc
        .columns
        .iter()
        .filter(|c| c.col_type == ColumnType::NoteLink)
        .map(|c| c.id.clone())
        .collect();

    // Group visible records by group option ID (defaulting unmatched records to first option).
    let fallback_opt = group_col
        .options
        .first()
        .map(|o| o.id.clone())
        .unwrap_or_default();
    let mut grouped: HashMap<String, Vec<RecordItem>> = HashMap::new();
    for opt in &group_col.options {
        grouped.insert(opt.id.clone(), Vec::new());
    }
    let sel_filter = state
        .select_filter
        .as_ref()
        .map(|(c, o)| (c.as_str(), o.as_str()));
    for rec in &state.doc.records {
        if !record_matches_smart_filter(
            rec,
            &state.doc.columns,
            &q_lower,
            state.date_filter,
            state.tag_filter.as_deref(),
            sel_filter,
            today,
        ) {
            continue;
        }
        let raw_val = rec
            .values
            .get(&group_col_id)
            .map(|v| v.as_text())
            .unwrap_or_default();
        let target_opt = if group_col.options.iter().any(|o| o.id == raw_val) {
            raw_val
        } else {
            fallback_opt.clone()
        };
        if !target_opt.is_empty() {
            grouped.entry(target_opt).or_default().push(rec.clone());
        }
    }
    for list in grouped.values_mut() {
        list.sort_by_key(|r| (r.order, r.created_at));
    }

    let tabbed_mode = layout.is_mobile() || ui.available_width() < 600.0;
    if tabbed_mode {
        render_mobile_custom_board(
            ui,
            p,
            lang,
            &group_col,
            &grouped,
            &other_select_cols,
            date_col_id.as_deref(),
            &note_link_cols,
            state,
            actions,
        );
        return;
    }

    let avail_h = ui.available_height().max(320.0);
    let active_drag = DragAndDrop::payload::<CustomDragCard>(ui.ctx());
    let mut pending_move: Option<ColumnMoveOp> = None;
    let mut pending_reorder: Option<(String, isize)> = None;
    let mut rename_opt: Option<(String, String)> = None;
    let mut recolor_opt: Option<(String, ColorToken)> = None;
    let mut move_opt_dir: Option<(String, isize)> = None;
    let mut delete_opt_id: Option<String> = None;
    let mut clicked_note_link: Option<String> = None;

    egui::ScrollArea::horizontal()
        .id_salt("custom_board_cols_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let col_w = 238.0_f32;
                let ptr_pos = ui.ctx().pointer_interact_pos();

                for (opt_idx, opt) in group_col.options.iter().enumerate() {
                    let (pill_bg, dot_col, text_col) = p.token_colors(opt.color);
                    let cards = grouped.get(&opt.id).map(Vec::as_slice).unwrap_or(&[]);
                    let inner_w = col_w - 22.0;

                    let prev_col_rect: Option<Rect> = ui.data(|d| {
                        d.get_temp(ui.make_persistent_id(("cboard_col_rect", &opt.id)))
                    });
                    let prev_card_rects: Vec<(String, f32, f32)> = ui
                        .data(|d| d.get_temp(ui.make_persistent_id(("cboard_card_rects", &opt.id))))
                        .unwrap_or_default();
                    let is_adding_top = state.quick_add_col_opt.as_deref() == Some(opt.id.as_str())
                        && state.quick_add_at_top;

                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.set_max_width(col_w);

                        let mut col_frame = egui::Frame::none()
                            .fill(p.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, p.border_hairline))
                            .rounding(Rounding::same(10.0))
                            .inner_margin(Margin::same(11.0))
                            .begin(ui);

                        let mut hovered_insert_before: Option<String> = None;
                        let mut new_card_rects: Vec<(String, f32, f32)> =
                            Vec::with_capacity(cards.len());
                        {
                            let cui = &mut col_frame.content_ui;
                            cui.set_width(inner_w);
                            cui.set_max_width(inner_w);
                            cui.set_min_height((avail_h - 30.0).max(260.0));

                            // Column header: colored pill + count + quick-add '+' + config popover
                            cui.horizontal(|ui| {
                                egui::Frame::none()
                                    .fill(pill_bg)
                                    .rounding(Rounding::same(10.0))
                                    .inner_margin(Margin::symmetric(9.0, 3.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (dr, _) = ui.allocate_exact_size(
                                                vec2(7.0, 12.0),
                                                Sense::hover(),
                                            );
                                            ui.painter().circle_filled(dr.center(), 3.2, dot_col);
                                            ui.label(
                                                egui::RichText::new(&opt.label)
                                                    .font(font_medium(12.0))
                                                    .color(text_col),
                                            );
                                        });
                                    });
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(cards.len().to_string())
                                        .font(font_mono(11.5))
                                        .color(p.text_muted),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let pop_id =
                                            ui.make_persistent_id(("stage_cfg_pop", &opt.id));
                                        let gear = icon_button(
                                            ui,
                                            p,
                                            Icon::Edit,
                                            s.col_schema_title,
                                            ui.memory(|m| m.is_popup_open(pop_id)),
                                        );
                                        if gear.clicked() {
                                            ui.memory_mut(|m| m.toggle_popup(pop_id));
                                        }
                                        egui::popup_below_widget(
                                            ui,
                                            pop_id,
                                            &gear,
                                            PopupCloseBehavior::CloseOnClickOutside,
                                            |ui| {
                                                ui.set_min_width(214.0);
                                                ui.set_max_width(236.0);
                                                let mut lbl = opt.label.clone();
                                                ui.label(
                                                    egui::RichText::new(s.col_field_name)
                                                        .font(font_medium(11.5))
                                                        .color(p.text_secondary),
                                                );
                                                ui.add_space(4.0);
                                                let r = styled_text_input(
                                                    ui,
                                                    p,
                                                    &mut lbl,
                                                    s.col_option_hint,
                                                    false,
                                                    f32::INFINITY,
                                                );
                                                if r.changed() {
                                                    rename_opt = Some((opt.id.clone(), lbl));
                                                }
                                                ui.add_space(8.0);
                                                let mut ctok = opt.color;
                                                if render_color_token_picker(ui, p, &mut ctok) {
                                                    recolor_opt = Some((opt.id.clone(), ctok));
                                                }
                                                ui.add_space(8.0);
                                                ui.horizontal(|ui| {
                                                    if opt_idx > 0
                                                        && icon_button(
                                                            ui,
                                                            p,
                                                            Icon::ChevronLeft,
                                                            "<-",
                                                            false,
                                                        )
                                                        .clicked()
                                                    {
                                                        move_opt_dir = Some((opt.id.clone(), -1));
                                                    }
                                                    if opt_idx + 1 < group_col.options.len()
                                                        && icon_button(
                                                            ui,
                                                            p,
                                                            Icon::ChevronRight,
                                                            "->",
                                                            false,
                                                        )
                                                        .clicked()
                                                    {
                                                        move_opt_dir = Some((opt.id.clone(), 1));
                                                    }
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(egui::Align::Center),
                                                        |ui| {
                                                            if group_col.options.len() > 1
                                                                && icon_button(
                                                                    ui,
                                                                    p,
                                                                    Icon::Trash,
                                                                    s.delete_btn,
                                                                    false,
                                                                )
                                                                .clicked()
                                                            {
                                                                delete_opt_id =
                                                                    Some(opt.id.clone());
                                                                ui.memory_mut(|m| m.close_popup());
                                                            }
                                                        },
                                                    );
                                                });
                                            },
                                        );

                                        if icon_button(
                                            ui,
                                            p,
                                            Icon::Plus,
                                            s.col_new_card,
                                            is_adding_top,
                                        )
                                        .clicked()
                                        {
                                            if is_adding_top {
                                                state.quick_add_col_opt = None;
                                                state.quick_add_at_top = false;
                                                state.quick_add_title.clear();
                                            } else {
                                                state.quick_add_col_opt = Some(opt.id.clone());
                                                state.quick_add_at_top = true;
                                                state.quick_add_title.clear();
                                            }
                                        }
                                    },
                                );
                            });

                            cui.add_space(8.0);
                            if is_adding_top {
                                render_column_quick_add(
                                    cui,
                                    p,
                                    lang,
                                    &group_col_id,
                                    &opt.id,
                                    inner_w,
                                    "top",
                                    state,
                                    actions,
                                );
                                cui.add_space(8.0);
                            } else {
                                reset_focus_gate(
                                    cui.ctx(),
                                    egui::Id::new(("col_qa_foc", &opt.id, "top")),
                                );
                            }

                            let hovering_this_col = active_drag.is_some()
                                && ptr_pos
                                    .is_some_and(|pt| prev_col_rect.is_some_and(|r| r.contains(pt)));
                            let drag_h = active_drag.as_ref().map(|d| d.height).unwrap_or(72.0);
                            let drag_rec_id = active_drag.as_ref().map(|d| d.record_id.as_str());
                            let others: Vec<&RecordItem> = cards
                                .iter()
                                .filter(|r| Some(r.id.as_str()) != drag_rec_id)
                                .collect();
                            let target_slot_idx: Option<usize> = if hovering_this_col {
                                let pt = ptr_pos.unwrap();
                                let mut count = 0usize;
                                for rec in &others {
                                    if let Some((_, cy, _)) =
                                        prev_card_rects.iter().find(|(id, _, _)| id == &rec.id)
                                    {
                                        if *cy < pt.y {
                                            count += 1;
                                        }
                                    }
                                }
                                hovered_insert_before = others.get(count).map(|r| r.id.clone());
                                Some(count)
                            } else {
                                None
                            };

                            // Vertical card list with animated drag-reorder slots and FLIP settle
                            let scroll_max_h = (avail_h - 142.0).max(160.0);
                            egui::ScrollArea::vertical()
                                .id_salt(("custom_col_scroll", &opt.id))
                                .auto_shrink([false, true])
                                .max_height(scroll_max_h)
                                .show(cui, |ui| {
                                    ui.set_width(inner_w);
                                    let scroll_origin_y = ui.cursor().top();
                                    let scroll_clip = ui.clip_rect();

                                    // Floating ghost for the currently dragged card (originating from this column)
                                    for rec in cards {
                                        let drag_id = ui.make_persistent_id(("c_card", &rec.id));
                                        if ui.ctx().is_being_dragged(drag_id) {
                                            let cur_h = prev_card_rects
                                                .iter()
                                                .find(|(id, _, _)| id == &rec.id)
                                                .map(|(_, _, h)| *h)
                                                .unwrap_or(72.0);
                                            DragAndDrop::set_payload(
                                                ui.ctx(),
                                                CustomDragCard {
                                                    record_id: rec.id.clone(),
                                                    from_opt_id: opt.id.clone(),
                                                    height: cur_h,
                                                },
                                            );
                                            let layer_id = LayerId::new(Order::Tooltip, drag_id);
                                            let mut dummy_link = None;
                                            let mut dummy_reord = None;
                                            let ir = ui.with_layer_id(layer_id, |ui| {
                                                ui.set_width(inner_w);
                                                paint_custom_card(
                                                    ui,
                                                    p,
                                                    rec,
                                                    &other_select_cols,
                                                    date_col_id.as_deref(),
                                                    &note_link_cols,
                                                    inner_w,
                                                    1.0,
                                                    true,
                                                    None,
                                                    &mut dummy_link,
                                                    &mut dummy_reord,
                                                )
                                            });
                                            if let Some(ptr) = ptr_pos {
                                                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                                                let delta = ptr - ir.response.rect.center();
                                                ui.ctx().transform_layer_shapes(
                                                    layer_id,
                                                    egui::emath::TSTransform::from_translation(
                                                        delta,
                                                    ),
                                                );
                                                let ghost_top_y =
                                                    ptr.y - ir.response.rect.height() * 0.5;
                                                ui.data_mut(|d| {
                                                    d.insert_temp(
                                                        ui.make_persistent_id((
                                                            "cboard_ghost_top_y",
                                                            &rec.id,
                                                        )),
                                                        ghost_top_y,
                                                    );
                                                });
                                            }
                                        }
                                    }

                                    let others_len = others.len();
                                    for slot_idx in 0..=others_len {
                                        let gap_id =
                                            ui.make_persistent_id(("cboard_slot_gap", &opt.id, slot_idx));
                                        let gap_t = if active_drag.is_some() {
                                            anim_bool_eased(
                                                ui.ctx(),
                                                gap_id,
                                                hovering_this_col
                                                    && target_slot_idx == Some(slot_idx),
                                                0.14,
                                            )
                                        } else {
                                            let _ = ui.ctx().animate_bool_with_time(
                                                gap_id, false, 0.0,
                                            );
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
                                                Stroke::new(
                                                    2.0_f32,
                                                    with_alpha(p.accent, gap_t * 0.7),
                                                ),
                                            );
                                            ui.add_space(8.0 * gap_t);
                                        }

                                        if slot_idx < others_len {
                                            let rec = others[slot_idx];
                                            let drag_id =
                                                ui.make_persistent_id(("c_card", &rec.id));
                                            let card_top = ui.next_widget_position();
                                            let est_h = prev_card_rects
                                                .iter()
                                                .find(|(id, _, _)| id == &rec.id)
                                                .map(|(_, _, h)| *h)
                                                .unwrap_or(80.0);
                                            let est_rect = Rect::from_min_size(
                                                card_top,
                                                vec2(inner_w, est_h),
                                            );
                                            let is_hov = active_drag.is_none()
                                                && ui.rect_contains_pointer(est_rect);
                                            let hov_t = anim_bool_eased(
                                                ui.ctx(),
                                                drag_id.with("hov"),
                                                is_hov,
                                                0.11,
                                            );

                                            // FLIP settle animation after drop or up/down shift
                                            let local_y = card_top.y - scroll_origin_y;
                                            let settle_id = ui.make_persistent_id((
                                                "cboard_settle_y",
                                                &rec.id,
                                            ));
                                            let last_col: Option<String> = ui.data(|d| {
                                                d.get_temp(ui.make_persistent_id((
                                                    "cboard_last_col",
                                                    &rec.id,
                                                )))
                                            });
                                            let ghost_screen_y: Option<f32> = ui.data(|d| {
                                                d.get_temp(ui.make_persistent_id((
                                                    "cboard_ghost_top_y",
                                                    &rec.id,
                                                )))
                                            });
                                            let anim_local_y = if active_drag.is_none() {
                                                if let Some(gsy) = ghost_screen_y {
                                                    ui.data_mut(|d| {
                                                        d.remove::<f32>(ui.make_persistent_id((
                                                            "cboard_ghost_top_y",
                                                            &rec.id,
                                                        )));
                                                    });
                                                    let seed = (gsy - scroll_origin_y).clamp(
                                                        local_y - 380.0,
                                                        local_y + 380.0,
                                                    );
                                                    ui.ctx().animate_value_with_time(
                                                        settle_id, seed, 0.0,
                                                    );
                                                    ui.ctx().animate_value_with_time(
                                                        settle_id, local_y, 0.18,
                                                    )
                                                } else if last_col.as_deref()
                                                    == Some(opt.id.as_str())
                                                {
                                                    ui.ctx().animate_value_with_time(
                                                        settle_id, local_y, 0.18,
                                                    )
                                                } else {
                                                    ui.ctx().animate_value_with_time(
                                                        settle_id, local_y, 0.0,
                                                    )
                                                }
                                            } else {
                                                ui.ctx().animate_value_with_time(
                                                    settle_id, local_y, 0.0,
                                                )
                                            };
                                            ui.data_mut(|d| {
                                                d.insert_temp(
                                                    ui.make_persistent_id((
                                                        "cboard_last_col",
                                                        &rec.id,
                                                    )),
                                                    opt.id.clone(),
                                                );
                                            });
                                            let settle_dy = if active_drag.is_none() {
                                                (anim_local_y - local_y).clamp(-380.0, 380.0)
                                            } else {
                                                0.0
                                            };

                                            let reorder_caps = if active_drag.is_none()
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

                                            let mut link_hit = None;
                                            let mut reorder_hit = None;
                                            let inner_resp = if settle_dy.abs() > 0.6 {
                                                ui.ctx().request_repaint();
                                                let settle_layer = LayerId::new(
                                                    Order::Middle,
                                                    drag_id.with("settle"),
                                                );
                                                let ir = ui.with_layer_id(settle_layer, |ui| {
                                                    ui.set_clip_rect(scroll_clip);
                                                    paint_custom_card(
                                                        ui,
                                                        p,
                                                        rec,
                                                        &other_select_cols,
                                                        date_col_id.as_deref(),
                                                        &note_link_cols,
                                                        inner_w,
                                                        hov_t,
                                                        false,
                                                        reorder_caps,
                                                        &mut link_hit,
                                                        &mut reorder_hit,
                                                    )
                                                });
                                                ui.ctx().transform_layer_shapes(
                                                    settle_layer,
                                                    egui::emath::TSTransform::from_translation(
                                                        vec2(0.0, settle_dy),
                                                    ),
                                                );
                                                ir.inner
                                            } else {
                                                paint_custom_card(
                                                    ui,
                                                    p,
                                                    rec,
                                                    &other_select_cols,
                                                    date_col_id.as_deref(),
                                                    &note_link_cols,
                                                    inner_w,
                                                    hov_t,
                                                    false,
                                                    reorder_caps,
                                                    &mut link_hit,
                                                    &mut reorder_hit,
                                                )
                                            };

                                            new_card_rects.push((
                                                rec.id.clone(),
                                                inner_resp.rect.center().y,
                                                inner_resp.rect.height(),
                                            ));

                                            if let Some(target) = link_hit {
                                                clicked_note_link = Some(target);
                                            } else if let Some(delta) = reorder_hit {
                                                pending_reorder = Some((rec.id.clone(), delta));
                                            } else {
                                                let card_resp = ui.interact(
                                                    inner_resp.rect,
                                                    drag_id,
                                                    Sense::click_and_drag(),
                                                );
                                                if card_resp.hovered() {
                                                    card_resp
                                                        .clone()
                                                        .on_hover_cursor(CursorIcon::Grab);
                                                }
                                                if card_resp.clicked() {
                                                    state.editing_record_id = Some(rec.id.clone());
                                                }
                                            }

                                            ui.add_space(8.0);
                                        }
                                    }

                                    if others_len == 0
                                        && active_drag.is_some()
                                        && !hovering_this_col
                                    {
                                        let (hr, _) = ui.allocate_exact_size(
                                            vec2(inner_w, 52.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().rect_stroke(
                                            hr,
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
                                                hr.center().x - g.size().x * 0.5,
                                                hr.center().y - g.size().y * 0.5,
                                            ),
                                            g,
                                            p.accent_text,
                                        );
                                        ui.add_space(6.0);
                                    }

                                    if !is_adding_top {
                                        render_column_quick_add(
                                            ui,
                                            p,
                                            lang,
                                            &group_col_id,
                                            &opt.id,
                                            inner_w,
                                            "inline",
                                            state,
                                            actions,
                                        );
                                    }
                                });
                        }

                        let col_resp = col_frame.allocate_space(ui);
                        ui.data_mut(|d| {
                            d.insert_temp(
                                ui.make_persistent_id(("cboard_col_rect", &opt.id)),
                                col_resp.rect,
                            );
                            d.insert_temp(
                                ui.make_persistent_id(("cboard_card_rects", &opt.id)),
                                new_card_rects,
                            );
                        });
                        let can_drop = active_drag.is_some() && col_resp.contains_pointer();
                        let drop_t = anim_bool_eased(
                            ui.ctx(),
                            ui.make_persistent_id(("custom_col_drop", &opt.id)),
                            can_drop,
                            0.12,
                        );
                        if drop_t > 0.01 {
                            col_frame.frame.stroke = Stroke::new(
                                1.0 + drop_t * 0.6,
                                lerp_color(p.border_hairline, p.accent, drop_t),
                            );
                            col_frame.frame.fill =
                                lerp_color(p.bg_sidebar, p.bg_elevated, drop_t);
                        }
                        col_frame.paint(ui);

                        if let Some(dropped) = col_resp.dnd_release_payload::<CustomDragCard>() {
                            pending_move = Some(ColumnMoveOp {
                                record_id: dropped.record_id.clone(),
                                target_opt_id: opt.id.clone(),
                                insert_before_id: hovered_insert_before,
                            });
                            let _ = dropped.from_opt_id;
                        }
                    });
                }

                // Trailing "+ Колонка" adder card
                ui.vertical(|ui| {
                    render_add_stage_column(ui, p, lang, &group_col_id, state, actions);
                });
            });
        });

    if let Some(target) = clicked_note_link {
        actions.push(UiAction::OpenOrCreateWikilink(target));
    }
    if let Some((rec_id, delta)) = pending_reorder {
        if state.doc.shift_record_in_group(&rec_id, &group_col_id, delta) {
            actions.push(UiAction::MarkCollectionDirty);
        }
    }
    if let Some(mv) = pending_move {
        state.doc.move_record_to_group(
            &mv.record_id,
            &group_col_id,
            &mv.target_opt_id,
            mv.insert_before_id.as_deref(),
        );
        actions.push(UiAction::MarkCollectionDirty);
    }
    if let Some((opt_id, new_lbl)) = rename_opt {
        if let Some(col) = state.doc.columns.iter_mut().find(|c| c.id == group_col_id) {
            if let Some(o) = col.options.iter_mut().find(|o| o.id == opt_id) {
                o.label = new_lbl;
                actions.push(UiAction::MarkCollectionDirty);
            }
        }
    }
    if let Some((opt_id, color)) = recolor_opt {
        if let Some(col) = state.doc.columns.iter_mut().find(|c| c.id == group_col_id) {
            if let Some(o) = col.options.iter_mut().find(|o| o.id == opt_id) {
                o.color = color;
                actions.push(UiAction::MarkCollectionDirty);
            }
        }
    }
    if let Some((opt_id, delta)) = move_opt_dir {
        state.doc.move_select_option(&group_col_id, &opt_id, delta);
        actions.push(UiAction::MarkCollectionDirty);
    }
    if let Some(opt_id) = delete_opt_id {
        state.doc.remove_select_option(&group_col_id, &opt_id);
        actions.push(UiAction::MarkCollectionDirty);
    }
    let _ = is_ru;
}

fn render_missing_select_state(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    ui.add_space(48.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(s.col_no_select_title)
                .font(font_semibold(17.0))
                .color(p.text_primary),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(s.col_no_select_desc)
                .font(font_body(13.0))
                .color(p.text_muted),
        );
        ui.add_space(16.0);
        if primary_button(ui, p, Some(Icon::Plus), s.col_add_status_col, false).clicked() {
            state.doc.ensure_group_column(lang == Lang::Ru);
            actions.push(UiAction::MarkCollectionDirty);
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_custom_board(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    group_col: &ColumnDef,
    grouped: &HashMap<String, Vec<RecordItem>>,
    other_select_cols: &[ColumnDef],
    date_col_id: Option<&str>,
    note_link_cols: &[String],
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    if !group_col
        .options
        .iter()
        .any(|o| o.id == state.mobile_active_opt_id)
    {
        state.mobile_active_opt_id = group_col
            .options
            .first()
            .map(|o| o.id.clone())
            .unwrap_or_default();
    }

    egui::ScrollArea::horizontal()
        .id_salt("mob_cboard_tabs")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for opt in &group_col.options {
                    let active = state.mobile_active_opt_id == opt.id;
                    let (pill_bg, dot_col, text_col) = p.token_colors(opt.color);
                    let count = grouped.get(&opt.id).map(Vec::len).unwrap_or(0);
                    let label = format!("{}  {}", opt.label, count);
                    let g = ui.painter().layout_no_wrap(
                        label,
                        font_medium(12.0),
                        if active { text_col } else { p.text_secondary },
                    );
                    let w = g.size().x + 26.0;
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        Rounding::same(8.0),
                        if active { pill_bg } else { p.bg_elevated },
                    );
                    ui.painter().rect_stroke(
                        rect,
                        Rounding::same(8.0),
                        Stroke::new(
                            1.0_f32,
                            if active { dot_col } else { p.border_hairline },
                        ),
                    );
                    ui.painter()
                        .circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.0, dot_col);
                    ui.painter().galley(
                        pos2(rect.left() + 19.0, rect.center().y - g.size().y * 0.5),
                        g,
                        if active { text_col } else { p.text_secondary },
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        state.mobile_active_opt_id = opt.id.clone();
                    }
                }
            });
        });

    ui.add_space(10.0);
    let active_opt = state.mobile_active_opt_id.clone();
    let cards = grouped.get(&active_opt).map(Vec::as_slice).unwrap_or(&[]);

    egui::ScrollArea::vertical()
        .id_salt(("mob_cboard_cards", &active_opt))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let full_w = ui.available_width().max(220.0);
            if cards.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(s.col_no_cards)
                            .font(font_body(13.0))
                            .color(p.text_muted),
                    );
                });
                ui.add_space(14.0);
            }
            let cards_len = cards.len();
            let mut mob_reorder: Option<(String, isize)> = None;
            for (cidx, rec) in cards.iter().enumerate() {
                let mut dummy_link = None;
                let mut reord_hit = None;
                let rcaps = if cards_len > 1 {
                    Some((cidx > 0, cidx + 1 < cards_len, 1.0))
                } else {
                    None
                };
                let resp = paint_custom_card(
                    ui,
                    p,
                    rec,
                    other_select_cols,
                    date_col_id,
                    note_link_cols,
                    full_w,
                    0.0,
                    false,
                    rcaps,
                    &mut dummy_link,
                    &mut reord_hit,
                );
                if let Some(target) = dummy_link {
                    actions.push(UiAction::OpenOrCreateWikilink(target));
                } else if let Some(delta) = reord_hit {
                    mob_reorder = Some((rec.id.clone(), delta));
                } else if resp.interact(Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    state.editing_record_id = Some(rec.id.clone());
                }
                ui.add_space(8.0);
            }
            if let Some((rid, delta)) = mob_reorder {
                if state.doc.shift_record_in_group(&rid, &group_col.id, delta) {
                    actions.push(UiAction::MarkCollectionDirty);
                }
            }
            if !active_opt.is_empty() {
                render_column_quick_add(
                    ui,
                    p,
                    lang,
                    &group_col.id,
                    &active_opt,
                    full_w,
                    "mob",
                    state,
                    actions,
                );
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn paint_custom_card(
    ui: &mut Ui,
    p: &ThemePalette,
    rec: &RecordItem,
    other_select_cols: &[ColumnDef],
    date_col_id: Option<&str>,
    note_link_cols: &[String],
    width: f32,
    hover_t: f32,
    floating: bool,
    reorder_caps: Option<(bool, bool, f32)>,
    clicked_link_out: &mut Option<String>,
    reorder_out: &mut Option<isize>,
) -> egui::Response {
    let bg = if floating {
        p.bg_elevated
    } else {
        lerp_color(p.bg_card, p.bg_elevated, hover_t)
    };
    let border = if floating {
        p.accent
    } else {
        lerp_color(p.border_hairline, p.border_subtle, hover_t)
    };

    let mut frame = egui::Frame::none()
        .fill(bg)
        .stroke(Stroke::new(if floating { 1.25_f32 } else { 1.0_f32 }, border))
        .rounding(Rounding::same(8.0))
        .shadow(if floating { p.shadow_floating } else { p.shadow_elevated })
        .inner_margin(Margin::same(11.0))
        .begin(ui);

    {
        let cui = &mut frame.content_ui;
        let content_w = (width - 22.0).max(80.0);
        cui.set_width(content_w);
        let top_right_x = cui.cursor().left() + content_w;
        let top_y = cui.cursor().top();
        cui.add(
            egui::Label::new(
                egui::RichText::new(if rec.title.trim().is_empty() {
                    "Untitled"
                } else {
                    rec.title.trim()
                })
                .font(font_medium(13.5))
                .color(p.text_primary),
            )
            .wrap(),
        );

        let desc_line = rec.description.lines().next().unwrap_or("").trim();
        if !desc_line.is_empty() {
            cui.add_space(4.0);
            cui.add(
                egui::Label::new(
                    egui::RichText::new(desc_line)
                        .font(font_body(11.5))
                        .color(p.text_muted),
                )
                .truncate(),
            );
        }

        // Metadata pills row: non-group selects, date, checklist count, tags, NoteLink
        let has_meta = !other_select_cols.is_empty()
            || !rec.checklist.is_empty()
            || !rec.tags.is_empty()
            || date_col_id.is_some_and(|id| rec.values.get(id).is_some_and(|v| !v.is_empty()))
            || note_link_cols
                .iter()
                .any(|id| rec.values.get(id).is_some_and(|v| !v.is_empty()));

        if has_meta {
            cui.add_space(7.0);
            cui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(5.0, 4.0);

                for col in other_select_cols {
                    if let Some(val) = rec.values.get(&col.id) {
                        let oid = val.as_text();
                        if let Some(opt) = col.options.iter().find(|o| o.id == oid) {
                            let (pbg, dot, ptxt) = p.token_colors(opt.color);
                            draw_mini_badge(ui, pbg, ptxt, Some(dot), &opt.label);
                        }
                    }
                }

                if let Some(did) = date_col_id {
                    if let Some(val) = rec.values.get(did) {
                        let d = val.as_text();
                        if !d.trim().is_empty() {
                            draw_mini_badge(ui, p.bg_hover, p.text_secondary, None, d.trim());
                        }
                    }
                }

                if !rec.checklist.is_empty() {
                    let done = rec.checklist.iter().filter(|c| c.done).count();
                    let label = format!("{done}/{}", rec.checklist.len());
                    let all_done = done == rec.checklist.len();
                    let (bg_c, fg_c) = if all_done {
                        (p.success_soft, p.success)
                    } else {
                        (p.bg_hover, p.text_secondary)
                    };
                    draw_mini_badge(ui, bg_c, fg_c, None, &label);
                }

                for lid in note_link_cols {
                    if let Some(val) = rec.values.get(lid) {
                        let raw = val.as_text();
                        let target = strip_link_target(&raw);
                        if !target.is_empty() {
                            let g = ui.painter().layout_no_wrap(
                                target.to_string(),
                                font_medium(11.0),
                                p.accent_text,
                            );
                            let bw = (g.size().x + 22.0).min(150.0);
                            let (r, resp) =
                                ui.allocate_exact_size(vec2(bw, 19.0), Sense::click());
                            ui.painter().rect_filled(r, Rounding::same(4.5), p.accent_soft);
                            draw_icon(
                                ui.painter(),
                                Rect::from_center_size(
                                    pos2(r.left() + 9.0, r.center().y),
                                    vec2(10.5, 10.5),
                                ),
                                Icon::Link,
                                p.accent_text,
                            );
                            ui.painter().galley(
                                pos2(r.left() + 17.0, r.center().y - g.size().y * 0.5),
                                g,
                                p.accent_text,
                            );
                            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                *clicked_link_out = Some(target.to_string());
                            }
                        }
                    }
                }

                for tag in rec.tags.iter().take(2) {
                    let (tb, tf) = p.tag_colors(tag);
                    draw_mini_badge(ui, tb, tf, None, &format!("#{tag}"));
                }
            });
        }

        if let Some((can_up, can_down, alpha)) = reorder_caps {
            if !floating && (can_up || can_down) {
                let mut rx = top_right_x + 2.0;
                let pill_y = top_y - 2.0;
                let btn_w = 18.0_f32;
                let pill_w = if can_up && can_down { 38.0 } else { 18.0 };
                let backdrop = Rect::from_min_size(pos2(rx - pill_w - 2.0, pill_y - 2.0), vec2(pill_w + 4.0, 22.0));
                cui.painter().rect_filled(backdrop, Rounding::same(6.0), with_alpha(p.bg_elevated, 0.92 * alpha));
                cui.painter().rect_stroke(backdrop, Rounding::same(6.0), Stroke::new(1.0_f32, with_alpha(p.border_subtle, 0.75 * alpha)));

                for &(enabled, delta, lbl) in &[(can_down, 1isize, "↓"), (can_up, -1isize, "↑")] {
                    if !enabled {
                        continue;
                    }
                    rx -= btn_w;
                    let br = Rect::from_min_size(pos2(rx, pill_y), vec2(btn_w, 18.0));
                    let bresp = cui.interact(br, cui.make_persistent_id(("c_reord", &rec.id, delta)), Sense::click());
                    if bresp.hovered() {
                        cui.painter().rect_filled(br, Rounding::same(4.5), p.accent_soft);
                    }
                    let fg = if bresp.hovered() { p.accent_text } else { with_alpha(p.text_secondary, alpha) };
                    let g = cui.painter().layout_no_wrap(lbl.to_string(), font_mono(11.0), fg);
                    cui.painter().galley(pos2(br.center().x - g.size().x * 0.5, br.center().y - g.size().y * 0.5), g, fg);
                    if bresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        *reorder_out = Some(delta);
                    }
                    rx -= 2.0;
                }
            }
        }
    }

    frame.end(ui)
}

fn draw_mini_badge(ui: &mut Ui, bg: Color32, fg: Color32, dot: Option<Color32>, text: &str) {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_medium(10.5), fg);
    let left_pad = if dot.is_some() { 16.0 } else { 7.0 };
    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + left_pad + 7.0, 19.0), Sense::hover());
    ui.painter().rect_filled(r, Rounding::same(4.5), bg);
    if let Some(dcol) = dot {
        ui.painter()
            .circle_filled(pos2(r.left() + 8.0, r.center().y), 2.8, dcol);
    }
    ui.painter().galley(
        pos2(r.left() + left_pad, r.center().y - g.size().y * 0.5),
        g,
        fg,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_column_quick_add(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    group_col_id: &str,
    opt_id: &str,
    width: f32,
    slot_tag: &str,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let is_open = state.quick_add_col_opt.as_deref() == Some(opt_id);
    let qa_gate_id = egui::Id::new(("col_qa_foc", opt_id, slot_tag));
    if is_open {
        let mut frame = egui::Frame::none()
            .fill(p.bg_card)
            .stroke(Stroke::new(1.2_f32, p.accent))
            .rounding(Rounding::same(9.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::same(10.0))
            .begin(ui);
        {
            let fui = &mut frame.content_ui;
            fui.set_width((width - 20.0).max(80.0));
            let resp = styled_text_input(
                fui,
                p,
                &mut state.quick_add_title,
                s.col_card_title_hint,
                false,
                f32::INFINITY,
            );
            focus_once(fui.ctx(), qa_gate_id, true, resp.id);
            let enter = fui.input(|i| i.key_pressed(egui::Key::Enter));
            let esc = fui.input(|i| i.key_pressed(egui::Key::Escape));
            fui.add_space(7.0);
            fui.horizontal(|ui| {
                if (primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked() || enter)
                    && !state.quick_add_title.trim().is_empty()
                {
                    let title = state.quick_add_title.trim().to_string();
                    state.doc.add_record(
                        &title,
                        Some((group_col_id, opt_id)),
                        chrono::Utc::now().timestamp(),
                    );
                    state.quick_add_title.clear();
                    state.quick_add_col_opt = None;
                    state.quick_add_at_top = false;
                    actions.push(UiAction::MarkCollectionDirty);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() || esc {
                        state.quick_add_title.clear();
                        state.quick_add_col_opt = None;
                        state.quick_add_at_top = false;
                    }
                });
            });
        }
        let resp = frame.end(ui);
        if slot_tag == "inline" && resp.hovered() {
            resp.scroll_to_me(Some(egui::Align::Max));
        }
    } else {
        reset_focus_gate(ui.ctx(), qa_gate_id);
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
        let hov = anim_bool_eased(
            ui.ctx(),
            ui.make_persistent_id(("col_qa", opt_id, slot_tag)),
            resp.hovered(),
            0.10,
        );
        if hov > 0.01 {
            ui.painter()
                .rect_filled(rect, Rounding::same(7.0), with_alpha(p.bg_hover, hov));
        }
        let g = ui.painter().layout_no_wrap(
            s.col_new_card.to_string(),
            font_medium(12.0),
            if resp.hovered() { p.text_primary } else { p.text_muted },
        );
        ui.painter().galley(
            pos2(rect.left() + 8.0, rect.center().y - g.size().y * 0.5),
            g,
            if resp.hovered() { p.text_primary } else { p.text_muted },
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            state.quick_add_col_opt = Some(opt_id.to_string());
            state.quick_add_at_top = false;
            state.quick_add_title.clear();
        }
    }
}

fn render_add_stage_column(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    group_col_id: &str,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let col_w = 236.0;
    let st_gate_id = egui::Id::new("col_add_stage_foc");
    if state.adding_board_stage {
        egui::Frame::none()
            .fill(p.bg_sidebar)
            .stroke(Stroke::new(1.0_f32, p.accent))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_width(col_w - 24.0);
                let resp = styled_text_input(
                    ui,
                    p,
                    &mut state.new_stage_name,
                    s.col_option_hint,
                    false,
                    f32::INFINITY,
                );
                focus_once(ui.ctx(), st_gate_id, true, resp.id);
                ui.add_space(8.0);
                render_color_token_picker(ui, p, &mut state.new_stage_color);
                ui.add_space(10.0);
                let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                ui.horizontal(|ui| {
                    if (primary_button(ui, p, None, s.col_add_option, false).clicked() || enter)
                        && !state.new_stage_name.trim().is_empty()
                    {
                        let name = state.new_stage_name.trim().to_string();
                        state
                            .doc
                            .add_select_option(group_col_id, &name, state.new_stage_color);
                        state.new_stage_name.clear();
                        state.adding_board_stage = false;
                        actions.push(UiAction::MarkCollectionDirty);
                    }
                    if secondary_button(ui, p, None, s.cancel_btn).clicked() || esc {
                        state.adding_board_stage = false;
                        state.new_stage_name.clear();
                    }
                });
            });
    } else {
        reset_focus_gate(ui.ctx(), st_gate_id);
        let (rect, resp) = ui.allocate_exact_size(vec2(148.0, 38.0), Sense::click());
        let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.11);
        ui.painter().rect_filled(
            rect,
            Rounding::same(9.0),
            lerp_color(p.bg_sidebar, p.bg_hover, hov),
        );
        ui.painter().rect_stroke(
            rect,
            Rounding::same(9.0),
            Stroke::new(1.0_f32, lerp_color(p.border_hairline, p.border_subtle, hov)),
        );
        let g = ui.painter().layout_no_wrap(
            s.col_add_column.to_string(),
            font_medium(12.5),
            p.text_secondary,
        );
        ui.painter().galley(
            pos2(
                rect.center().x - g.size().x * 0.5,
                rect.center().y - g.size().y * 0.5,
            ),
            g,
            p.text_secondary,
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            state.adding_board_stage = true;
            state.new_stage_name.clear();
        }
    }
}
