use crate::i18n::{strings, Lang};
use crate::ui::board_dnd::{BoardDndState, ColumnGeometry};
use crate::ui::collection_view::{
    render_color_token_picker, CollectionEditorState, CollectionSortDir,
};
use crate::ui::components::{
    focus_once, icon_button, normalize_tag_input, primary_button, reset_focus_gate,
    secondary_button, styled_text_input, tag_autocomplete_input, tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{LayoutClass, UiAction};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Margin, Order, PopupCloseBehavior, Rect, Rounding,
    Sense, Stroke, Ui,
};
use krono_core::{
    collection::{
        compare_records, record_matches_smart_filter, strip_link_target, ColorToken, ColumnDef,
        ColumnType, RecordItem, SortDirection,
    },
    daily::SimpleDate,
    markdown::card_preview_excerpt,
};
use std::collections::HashMap;

#[allow(clippy::too_many_arguments)]
pub fn render_custom_board(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    state: &mut CollectionEditorState,
    available_tags: &[String],
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

    let tabbed_mode = layout.is_mobile() || ui.available_width() < 600.0;
    let is_sorted = state.sort.is_some();
    let dnd_id = ui.make_persistent_id(("cboard_dnd", &group_col_id));
    let dnd = if !tabbed_mode {
        let mut st = BoardDndState::load(ui.ctx(), dnd_id);
        if let Some(drop_ev) = st.begin_frame(ui.ctx(), is_sorted) {
            state.doc.move_record_to_group(
                &drop_ev.card_id,
                &group_col_id,
                &drop_ev.to_col,
                drop_ev.insert_before_id.as_deref(),
            );
            actions.push(UiAction::MarkCollectionDirty);
        }
        Some(st)
    } else {
        None
    };

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
        ) || !crate::ui::collection_gallery::record_matches_media_category(
            rec,
            state.media_category_filter.as_deref(),
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
    let is_sorted = state.sort.is_some();
    for list in grouped.values_mut() {
        if let Some(ref sort) = state.sort {
            let dir = match sort.dir {
                CollectionSortDir::Asc => SortDirection::Asc,
                CollectionSortDir::Desc => SortDirection::Desc,
            };
            list.sort_by(|a, b| {
                compare_records(a, b, &sort.column_key, dir, &state.doc.columns)
            });
        } else {
            list.sort_by_key(|r| (r.order, r.created_at));
        }
    }

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
            available_tags,
            actions,
        );
        return;
    }

    let mut dnd = dnd.unwrap_or_default();
    let mut kb_moved = false;
    if state.editing_record_id.is_none()
        && !state.adding_board_stage
        && state.quick_add_col_opt.is_none()
        && !ui.ctx().wants_keyboard_input()
    {
        let col_ids: Vec<Vec<String>> = group_col
            .options
            .iter()
            .map(|opt| {
                grouped
                    .get(&opt.id)
                    .map(|list| list.iter().map(|r| r.id.clone()).collect())
                    .unwrap_or_default()
            })
            .collect();
        let non_empty_cols: Vec<usize> = col_ids
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.is_empty())
            .map(|(i, _)| i)
            .collect();
        if !non_empty_cols.is_empty() {
            let find_pos = |id: &str| -> Option<(usize, usize)> {
                for (ci, col) in col_ids.iter().enumerate() {
                    if let Some(ri) = col.iter().position(|x| x == id) {
                        return Some((ci, ri));
                    }
                }
                None
            };
            let cur_pos = state
                .kb_selected_record_id
                .as_deref()
                .and_then(find_pos);
            let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
            let up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
            let right = ui.input(|i| i.key_pressed(egui::Key::ArrowRight));
            let left = ui.input(|i| i.key_pressed(egui::Key::ArrowLeft));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            if down || up || right || left {
                let (mut ci, mut ri) = cur_pos.unwrap_or((non_empty_cols[0], 0));
                if down {
                    if cur_pos.is_some() && ri + 1 < col_ids[ci].len() {
                        ri += 1;
                    }
                } else if up {
                    if cur_pos.is_some() {
                        ri = ri.saturating_sub(1);
                    }
                } else if right {
                    if let Some(pos) = non_empty_cols.iter().position(|&c| c > ci) {
                        ci = non_empty_cols[pos];
                        ri = ri.min(col_ids[ci].len() - 1);
                    }
                } else if left {
                    if let Some(pos) = non_empty_cols.iter().rposition(|&c| c < ci) {
                        ci = non_empty_cols[pos];
                        ri = ri.min(col_ids[ci].len() - 1);
                    }
                }
                state.kb_selected_record_id = Some(col_ids[ci][ri].clone());
                kb_moved = true;
            } else if enter {
                if let Some(ref id) = state.kb_selected_record_id {
                    if find_pos(id).is_some() {
                        state.modal_new_tag.clear();
                        state.editing_record_id = Some(id.clone());
                    }
                }
            }
        }
    }
    let board_bottom = ui.max_rect().bottom();
    let mut pending_reorder: Option<(String, isize)> = None;
    let mut rename_opt: Option<(String, String)> = None;
    let mut recolor_opt: Option<(String, ColorToken)> = None;
    let mut move_opt_dir: Option<(String, isize)> = None;
    let mut delete_opt_id: Option<String> = None;
    let mut clicked_note_link: Option<String> = None;
    let mut convert_record_id: Option<String> = None;

    let ptr_pos = ui.ctx().pointer_interact_pos();
    let mut new_columns: HashMap<String, ColumnGeometry> = HashMap::new();
    let prev_selectable = ui.style().interaction.selectable_labels;
    if dnd.is_dragging() {
        ui.style_mut().interaction.selectable_labels = false;
    }

    let mut h_sa = egui::ScrollArea::horizontal()
        .id_salt("custom_board_cols_scroll")
        .drag_to_scroll(!dnd.is_dragging())
        .auto_shrink([false, false]);
    if let Some(next_x) = dnd.horizontal_autoscroll_offset(ui.ctx()) {
        h_sa = h_sa.horizontal_scroll_offset(next_x);
    }

    let h_out = h_sa.show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let col_w = 284.0_f32;

                for (opt_idx, opt) in group_col.options.iter().enumerate() {
                    let (pill_bg, dot_col, text_col) = p.token_colors(opt.color);
                    let cards = grouped.get(&opt.id).map(Vec::as_slice).unwrap_or(&[]);
                    let inner_w = col_w - 20.0;

                    let prev_col_rect = dnd.columns.get(&opt.id).map(|c| c.drop_rect);
                    let is_adding_top = state.quick_add_col_opt.as_deref() == Some(opt.id.as_str())
                        && state.quick_add_at_top;
                    let pop_id = ui.make_persistent_id(("stage_cfg_pop", &opt.id));
                    let pop_open = ui.memory(|m| m.is_popup_open(pop_id));
                    let col_ptr_inside = prev_col_rect
                        .is_some_and(|r| ptr_pos.is_some_and(|pt| r.contains(pt)));
                    let col_hov_t = anim_bool_eased(
                        ui.ctx(),
                        ui.make_persistent_id(("cboard_col_hov", &opt.id)),
                        col_ptr_inside || is_adding_top || pop_open,
                        0.12,
                    );

                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.set_max_width(col_w);

                        let mut col_frame = egui::Frame::none()
                            .fill(Color32::TRANSPARENT)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(12.0))
                            .inner_margin(Margin::symmetric(10.0, 8.0))
                            .begin(ui);

                        let mut col_base_cards: Vec<(String, f32, f32)> =
                            Vec::with_capacity(cards.len());
                        let col_scroll_rect;
                        let col_scroll_offset_y;
                        {
                            let cui = &mut col_frame.content_ui;
                            cui.set_width(inner_w);
                            cui.set_max_width(inner_w);

                            // Column header: colored pill + count + muted '+' & edit (fade in on column hover)
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
                                        let hdr_alpha = col_hov_t;
                                        let gear = muted_icon_btn(
                                            ui,
                                            p,
                                            Icon::Edit,
                                            s.col_schema_title,
                                            pop_open,
                                            hdr_alpha,
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

                                        let hdr_alpha = col_hov_t;
                                        if muted_icon_btn(
                                            ui,
                                            p,
                                            Icon::Plus,
                                            s.col_new_card,
                                            is_adding_top,
                                            hdr_alpha,
                                        )
                                        .clicked()
                                        {
                                            if is_adding_top {
                                                state.quick_add_col_opt = None;
                                                state.quick_add_at_top = false;
                                                state.quick_add_title.clear();
                                                state.quick_add_tags.clear();
                                                state.quick_add_tag_input.clear();
                                            } else {
                                                state.quick_add_col_opt = Some(opt.id.clone());
                                                state.quick_add_at_top = true;
                                                state.quick_add_title.clear();
                                                state.quick_add_tags.clear();
                                                state.quick_add_tag_input.clear();
                                            }
                                        }
                                    },
                                );
                            });

                            cui.add_space(6.0);
                            let (hr, _) = cui.allocate_exact_size(vec2(inner_w, 1.0), Sense::hover());
                            cui.painter().line_segment(
                                [hr.left_center(), hr.right_center()],
                                Stroke::new(1.0_f32, with_alpha(p.border_hairline, 0.75)),
                            );
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
                                    available_tags,
                                    actions,
                                );
                                cui.add_space(8.0);
                            } else {
                                reset_focus_gate(
                                    cui.ctx(),
                                    egui::Id::new(("col_qa_foc", &opt.id, "top")),
                                );
                            }

                            // Vertical card list with deterministic unshifted D&D hit-testing
                            let scroll_max_h = (board_bottom - cui.cursor().top() - 12.0).max(160.0);
                            let mut v_sa = egui::ScrollArea::vertical()
                                .id_salt(("custom_col_scroll", &opt.id))
                                .drag_to_scroll(!dnd.is_dragging())
                                .auto_shrink([false, true])
                                .max_height(scroll_max_h);
                            if let Some(next_y) = dnd.vertical_autoscroll_offset(cui.ctx(), &opt.id) {
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
                                    let others: Vec<(usize, &RecordItem)> = cards
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, r)| dragged_id.as_deref() != Some(r.id.as_str()))
                                        .collect();

                                    // Compute unshifted base card coords for stable hysteresis hit-testing
                                    let mut base_y = scroll_origin_y;
                                    for (_, rec) in &others {
                                        let h = dnd.card_heights.get(&rec.id).copied().unwrap_or(80.0);
                                        col_base_cards.push((rec.id.clone(), base_y, h));
                                        base_y += h + 10.0;
                                    }

                                    let others_len = others.len();
                                    for slot_idx in 0..=others_len {
                                        let gap_t = dnd.slot_gap_factor(&opt.id, slot_idx);
                                        if gap_t > 0.01 {
                                            let (ph, _) = ui.allocate_exact_size(
                                                vec2(inner_w, (drag_h * gap_t).max(4.0)),
                                                Sense::hover(),
                                            );
                                            if dnd.is_active_target_slot(&opt.id, slot_idx) {
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
                                                    Stroke::new(
                                                        2.0_f32,
                                                        with_alpha(p.accent, gap_t * 0.92),
                                                    ),
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
                                                draw_dashed_rect(
                                                    ui.painter(),
                                                    ph,
                                                    10.0,
                                                    Stroke::new(
                                                        1.25_f32,
                                                        with_alpha(p.border_strong, gap_t * 0.52),
                                                    ),
                                                    6.0,
                                                    4.0,
                                                );
                                            }
                                            ui.add_space(10.0 * gap_t);
                                        }

                                        if slot_idx < others_len {
                                            let (orig_idx, rec) = others[slot_idx];
                                            let drag_id = ui.make_persistent_id(("c_card", &rec.id));
                                            if hidden_id.as_deref() == Some(rec.id.as_str()) {
                                                let h = dnd
                                                    .card_heights
                                                    .get(&rec.id)
                                                    .copied()
                                                    .unwrap_or(drag_h);
                                                let (r, _) = ui.allocate_exact_size(
                                                    vec2(inner_w, h),
                                                    Sense::hover(),
                                                );
                                                dnd.resolve_settling_rect(&rec.id, r);
                                                ui.add_space(10.0);
                                                continue;
                                            }

                                            let card_top = ui.next_widget_position();
                                            let est_h = dnd
                                                .card_heights
                                                .get(&rec.id)
                                                .copied()
                                                .unwrap_or(80.0);
                                            let est_rect = Rect::from_min_size(
                                                card_top,
                                                vec2(inner_w, est_h),
                                            );
                                            let is_hov = !dnd_active
                                                && ui.rect_contains_pointer(est_rect);
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

                                            let mut link_hit = None;
                                            let mut reorder_hit = None;
                                            let mut convert_hit = false;
                                            let inner_resp = paint_custom_card(
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
                                                s.tasks_convert_to_note,
                                                &mut link_hit,
                                                &mut reorder_hit,
                                                &mut convert_hit,
                                            );
                                            dnd.card_heights
                                                .insert(rec.id.clone(), inner_resp.rect.height());
                                            if state.kb_selected_record_id.as_deref()
                                                == Some(rec.id.as_str())
                                                && state.editing_record_id.is_none()
                                            {
                                                ui.painter().rect_stroke(
                                                    inner_resp.rect,
                                                    Rounding::same(8.0),
                                                    Stroke::new(1.5_f32, p.accent),
                                                );
                                                if kb_moved {
                                                    inner_resp.scroll_to_me(Some(egui::Align::Center));
                                                }
                                            }

                                            if convert_hit {
                                                convert_record_id = Some(rec.id.clone());
                                            } else if let Some(target) = link_hit {
                                                clicked_note_link = Some(target);
                                            } else if let Some(delta) = reorder_hit {
                                                pending_reorder = Some((rec.id.clone(), delta));
                                            } else if !dnd_dragging {
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
                                                if card_resp.is_pointer_button_down_on()
                                                    && ui.input(|i| i.pointer.primary_pressed())
                                                {
                                                    if let Some(press_pos) = ptr_pos {
                                                        dnd.try_begin_pending(
                                                            &rec.id,
                                                            &opt.id,
                                                            orig_idx,
                                                            inner_resp.rect,
                                                            press_pos,
                                                        );
                                                    }
                                                }
                                                if card_resp.clicked() {
                                                    state.modal_new_tag.clear();
                                                    state.editing_record_id = Some(rec.id.clone());
                                                }
                                                card_resp.context_menu(|ui| {
                                                    if crate::ui::components::context_menu_row(
                                                        ui,
                                                        p,
                                                        Icon::FilePlus,
                                                        s.tasks_convert_to_note,
                                                        false,
                                                    )
                                                    .clicked()
                                                    {
                                                        convert_record_id = Some(rec.id.clone());
                                                        ui.close_menu();
                                                    }
                                                });
                                            }

                                            ui.add_space(10.0);
                                        }
                                    }

                                    if others_len == 0
                                        && dnd_dragging
                                        && !dnd.is_target_col(&opt.id)
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
                                            available_tags,
                                            actions,
                                        );
                                    }
                                    ui.add_space(10.0);
                                });
                            col_scroll_rect = v_out.inner_rect;
                            col_scroll_offset_y = v_out.state.offset.y;
                        }

                        let col_resp = col_frame.allocate_space(ui);
                        let drop_zone_rect = Rect::from_min_max(
                            col_resp.rect.min,
                            pos2(
                                col_resp.rect.right(),
                                board_bottom.max(col_resp.rect.bottom()),
                            ),
                        );
                        new_columns.insert(
                            opt.id.clone(),
                            ColumnGeometry {
                                col_id: opt.id.clone(),
                                drop_rect: drop_zone_rect,
                                scroll_rect: col_scroll_rect,
                                scroll_offset_y: col_scroll_offset_y,
                                base_cards: col_base_cards,
                            },
                        );

                        let can_drop = dnd.is_dragging()
                            && dnd.is_target_col(&opt.id)
                            && !dnd.is_same_col_sorted_locked(&opt.id, is_sorted);
                        let drop_t = anim_bool_eased(
                            ui.ctx(),
                            ui.make_persistent_id(("custom_col_drop", &opt.id)),
                            can_drop,
                            0.12,
                        );
                        if drop_t > 0.01 {
                            col_frame.frame.stroke = Stroke::new(
                                1.25_f32,
                                with_alpha(dot_col, 0.55 * drop_t),
                            );
                            col_frame.frame.fill = with_alpha(dot_col, 0.07 * drop_t);
                        }
                        col_frame.paint(ui);
                    });
                }

                // Trailing "+ Колонка" adder card
                ui.vertical(|ui| {
                    render_add_stage_column(ui, p, lang, &group_col_id, state, actions);
                });
            });
        });

    ui.style_mut().interaction.selectable_labels = prev_selectable;
    dnd.board_scroll_rect = Some(h_out.inner_rect);
    dnd.board_scroll_offset_x = h_out.state.offset.x;
    dnd.columns = new_columns;

    if let Some((ghost_id, ghost_min, ghost_size)) = dnd.active_ghost() {
        if let Some(rec) = state.doc.records.iter().find(|r| r.id == ghost_id) {
            let mut dummy_link = None;
            let mut dummy_reord = None;
            let mut dummy_conv = false;
            egui::Area::new(ui.make_persistent_id(("cboard_ghost_area", ghost_id)))
                .order(Order::Tooltip)
                .interactable(false)
                .fixed_pos(ghost_min)
                .show(ui.ctx(), |ui| {
                    ui.set_width(ghost_size.x);
                    paint_custom_card(
                        ui,
                        p,
                        rec,
                        &other_select_cols,
                        date_col_id.as_deref(),
                        &note_link_cols,
                        ghost_size.x,
                        1.0,
                        true,
                        None,
                        s.tasks_convert_to_note,
                        &mut dummy_link,
                        &mut dummy_reord,
                        &mut dummy_conv,
                    );
                });
        }
    }
    dnd.store(ui.ctx(), dnd_id);

    if let Some(target) = clicked_note_link {
        actions.push(UiAction::OpenOrCreateWikilink(target));
    }
    if let Some(rec_id) = convert_record_id {
        actions.push(UiAction::ConvertRecordToNote { record_id: rec_id });
    }
    if let Some((rec_id, delta)) = pending_reorder {
        if state.doc.shift_record_in_group(&rec_id, &group_col_id, delta) {
            actions.push(UiAction::MarkCollectionDirty);
        }
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
    available_tags: &[String],
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
                let mut conv_hit = false;
                let rcaps = if state.sort.is_none() && cards_len > 1 {
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
                    strings(lang).tasks_convert_to_note,
                    &mut dummy_link,
                    &mut reord_hit,
                    &mut conv_hit,
                );
                if conv_hit {
                    actions.push(UiAction::ConvertRecordToNote {
                        record_id: rec.id.clone(),
                    });
                } else if let Some(target) = dummy_link {
                    actions.push(UiAction::OpenOrCreateWikilink(target));
                } else if let Some(delta) = reord_hit {
                    mob_reorder = Some((rec.id.clone(), delta));
                } else if resp.interact(Sense::click()).on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    state.modal_new_tag.clear();
                    state.editing_record_id = Some(rec.id.clone());
                }
                ui.add_space(10.0);
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
                    available_tags,
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
    convert_label: &str,
    clicked_link_out: &mut Option<String>,
    reorder_out: &mut Option<isize>,
    convert_out: &mut bool,
) -> egui::Response {
    let bg = if floating {
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

    let mut frame = egui::Frame::none()
        .fill(bg)
        .stroke(Stroke::new(if floating { 1.5_f32 } else { 1.0_f32 }, border))
        .rounding(Rounding::same(10.0))
        .shadow(if floating { p.shadow_floating } else { rest_shadow })
        .inner_margin(Margin::symmetric(14.0, 12.0))
        .begin(ui);

    {
        let cui = &mut frame.content_ui;
        let content_w = (width - 28.0).max(80.0);
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
                .font(font_semibold(13.5))
                .color(p.text_primary),
            )
            .wrap(),
        );

        let preview = card_preview_excerpt(&rec.description, 2, 72);
        if !preview.is_empty() {
            cui.add_space(5.0);
            let mut job = egui::text::LayoutJob::single_section(
                preview,
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
            cui.label(job);
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
            cui.add_space(9.0);
            cui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 5.0);

                for col in other_select_cols {
                    if let Some(val) = rec.values.get(&col.id) {
                        let oid = val.as_text();
                        if let Some(opt) = col.options.iter().find(|o| o.id == oid) {
                            let (pbg, dot, ptxt) = p.token_colors(opt.color);
                            draw_mini_badge_bordered(ui, pbg, ptxt, Some(dot), &opt.label);
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

        if !floating && hover_t > 0.18 {
            let mut rx = top_right_x + 2.0;
            let pill_y = top_y - 2.0;
            let btn_w = 18.0_f32;
            let (has_up, has_down) = reorder_caps
                .map(|(u, d, _)| (u, d))
                .unwrap_or((false, false));
            let btn_count = 1 + usize::from(has_up) + usize::from(has_down);
            let pill_w = (btn_count as f32) * btn_w + ((btn_count.saturating_sub(1)) as f32) * 2.0;
            let backdrop = Rect::from_min_size(pos2(rx - pill_w - 2.0, pill_y - 2.0), vec2(pill_w + 4.0, 22.0));
            cui.painter().rect_filled(backdrop, Rounding::same(6.0), with_alpha(p.bg_elevated, 0.92 * hover_t));
            cui.painter().rect_stroke(backdrop, Rounding::same(6.0), Stroke::new(1.0_f32, with_alpha(p.border_subtle, 0.75 * hover_t)));

            // Convert to note quick button
            rx -= btn_w;
            let cr = Rect::from_min_size(pos2(rx, pill_y), vec2(btn_w, 18.0));
            let cresp = cui.interact(cr, cui.make_persistent_id(("c_conv", &rec.id)), Sense::click());
            if cresp.hovered() {
                cui.painter().rect_filled(cr, Rounding::same(4.5), p.accent_soft);
            }
            draw_icon(
                cui.painter(),
                Rect::from_center_size(cr.center(), vec2(11.0, 11.0)),
                Icon::FilePlus,
                if cresp.hovered() { p.accent_text } else { with_alpha(p.text_secondary, hover_t) },
            );
            if cresp.on_hover_cursor(CursorIcon::PointingHand).on_hover_text(convert_label).clicked() {
                *convert_out = true;
            }
            rx -= 2.0;

            for &(enabled, delta, lbl) in &[(has_down, 1isize, "↓"), (has_up, -1isize, "↑")] {
                if !enabled {
                    continue;
                }
                rx -= btn_w;
                let br = Rect::from_min_size(pos2(rx, pill_y), vec2(btn_w, 18.0));
                let bresp = cui.interact(br, cui.make_persistent_id(("c_reord", &rec.id, delta)), Sense::click());
                if bresp.hovered() {
                    cui.painter().rect_filled(br, Rounding::same(4.5), p.accent_soft);
                }
                let fg = if bresp.hovered() { p.accent_text } else { with_alpha(p.text_secondary, hover_t) };
                let g = cui.painter().layout_no_wrap(lbl.to_string(), font_mono(11.0), fg);
                cui.painter().galley(pos2(br.center().x - g.size().x * 0.5, br.center().y - g.size().y * 0.5), g, fg);
                if bresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    *reorder_out = Some(delta);
                }
                rx -= 2.0;
            }
        }
    }

    let resp = frame.end(ui);
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

fn muted_icon_btn(
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

fn draw_dashed_rect(
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

fn draw_mini_badge_bordered(
    ui: &mut Ui,
    _bg: Color32,
    fg: Color32,
    dot: Option<Color32>,
    text: &str,
) {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_medium(11.0), fg);
    let pad_l = if dot.is_some() { 17.0 } else { 7.0 };
    let w = (g.size().x + pad_l + 7.0).min(156.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    if let Some(dcol) = dot {
        ui.painter()
            .rect_filled(r, Rounding::same(6.0), with_alpha(dcol, 0.11));
        ui.painter().rect_stroke(
            r,
            Rounding::same(6.0),
            Stroke::new(1.0_f32, with_alpha(dcol, 0.34)),
        );
        ui.painter()
            .circle_filled(pos2(r.left() + 8.5, r.center().y), 2.6, dcol);
    }
    ui.painter().galley(
        pos2(r.left() + pad_l, r.center().y - g.size().y * 0.5),
        g,
        fg,
    );
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
    available_tags: &[String],
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
            let title_enter = resp.has_focus() && fui.input(|i| i.key_pressed(egui::Key::Enter));

            fui.add_space(6.0);
            if !state.quick_add_tags.is_empty() {
                fui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    let mut remove_idx: Option<usize> = None;
                    for (ti, tag) in state.quick_add_tags.iter().enumerate() {
                        let (_, rm) = tag_chip(ui, p, tag, true);
                        if rm {
                            remove_idx = Some(ti);
                        }
                    }
                    if let Some(ti) = remove_idx {
                        state.quick_add_tags.remove(ti);
                    }
                });
                fui.add_space(4.0);
            }

            if let Some(new_tag) = tag_autocomplete_input(
                fui,
                p,
                lang,
                ("col_qa_tag", opt_id, slot_tag),
                &mut state.quick_add_tag_input,
                s.add_tag_hint,
                f32::INFINITY,
                available_tags,
                &state.quick_add_tags,
            ) {
                let clean = normalize_tag_input(&new_tag);
                if !clean.is_empty()
                    && !state
                        .quick_add_tags
                        .iter()
                        .any(|t| normalize_tag_input(t) == clean)
                {
                    state.quick_add_tags.push(clean);
                }
            }

            let esc = fui.input(|i| i.key_pressed(egui::Key::Escape));
            fui.add_space(7.0);
            fui.horizontal(|ui| {
                if (primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked()
                    || title_enter)
                    && !state.quick_add_title.trim().is_empty()
                {
                    let title = state.quick_add_title.trim().to_string();
                    let new_id = state.doc.add_record(
                        &title,
                        Some((group_col_id, opt_id)),
                        chrono::Utc::now().timestamp(),
                    );
                    let tags = krono_core::note::normalize_tags(
                        std::mem::take(&mut state.quick_add_tags),
                    );
                    if let Some(rec) = state.doc.records.iter_mut().find(|r| r.id == new_id) {
                        rec.tags = tags;
                    }
                    state.quick_add_title.clear();
                    state.quick_add_tags.clear();
                    state.quick_add_tag_input.clear();
                    state.quick_add_col_opt = None;
                    state.quick_add_at_top = false;
                    actions.push(UiAction::MarkCollectionDirty);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() || esc {
                        state.quick_add_title.clear();
                        state.quick_add_tags.clear();
                        state.quick_add_tag_input.clear();
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
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
        let hov = anim_bool_eased(
            ui.ctx(),
            ui.make_persistent_id(("col_qa", opt_id, slot_tag)),
            resp.hovered(),
            0.10,
        );
        if hov > 0.01 {
            ui.painter().rect_filled(
                rect,
                Rounding::same(8.0),
                with_alpha(p.bg_elevated, 0.75 * hov),
            );
            ui.painter().rect_stroke(
                rect,
                Rounding::same(8.0),
                Stroke::new(1.0_f32, with_alpha(p.border_subtle, hov)),
            );
        }
        let fg = lerp_color(p.text_muted, p.text_primary, hov);
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(12.0, 12.0)),
            Icon::Plus,
            fg,
        );
        let g = ui
            .painter()
            .layout_no_wrap(s.col_new_card.to_string(), font_medium(12.0), fg);
        ui.painter().galley(
            pos2(rect.left() + 26.0, rect.center().y - g.size().y * 0.5),
            g,
            fg,
        );
        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            state.quick_add_col_opt = Some(opt_id.to_string());
            state.quick_add_at_top = false;
            state.quick_add_title.clear();
            state.quick_add_tags.clear();
            state.quick_add_tag_input.clear();
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
