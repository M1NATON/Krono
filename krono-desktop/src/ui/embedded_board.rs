use crate::i18n::{strings, Lang};
use crate::ui::components::{
    focus_once, hairline_h, reset_focus_gate, secondary_button, styled_text_input,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui,
};
use krono_core::collection::{CollectionDoc, ColumnDef, ColumnType, RecordItem};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum EmbeddedBoardAction {
    OpenFull(String),
    AddCard {
        board_path: String,
        option_id: String,
        title: String,
    },
    MoveCard {
        board_path: String,
        record_id: String,
        target_option_id: String,
    },
    ReorderCard {
        board_path: String,
        record_id: String,
        delta: isize,
    },
    DeleteCard {
        board_path: String,
        record_id: String,
    },
}

#[derive(Clone, Default)]
struct EmbeddedBoardDraftState {
    adding_opt: Option<String>,
    draft_title: String,
}

fn find_group_column(doc: &CollectionDoc) -> Option<&ColumnDef> {
    if let Some(gid) = &doc.group_by_column {
        if let Some(col) = doc
            .columns
            .iter()
            .find(|c| c.id == *gid && c.col_type == ColumnType::Select)
        {
            return Some(col);
        }
    }
    doc.columns.iter().find(|c| c.col_type == ColumnType::Select)
}

pub fn render_embedded_board_widget(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    board_path: &str,
    doc: &CollectionDoc,
    id_salt: &str,
    actions: &mut Vec<EmbeddedBoardAction>,
) {
    let s = strings(lang);
    let state_id = ui.make_persistent_id(("emb_board_st", board_path, id_salt));
    let mut draft_st = ui
        .data(|d| d.get_temp::<EmbeddedBoardDraftState>(state_id))
        .unwrap_or_default();

    let Some(group_col) = find_group_column(doc) else {
        egui::Frame::none()
            .fill(p.bg_card)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&doc.title)
                            .font(font_semibold(13.5))
                            .color(p.text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if secondary_button(ui, p, Some(Icon::ExternalLink), s.embedded_board_open)
                            .clicked()
                        {
                            actions.push(EmbeddedBoardAction::OpenFull(board_path.to_string()));
                        }
                    });
                });
            });
        return;
    };

    let first_opt_id = group_col
        .options
        .first()
        .map(|o| o.id.clone())
        .unwrap_or_default();
    let mut grouped: HashMap<&str, Vec<&RecordItem>> = HashMap::new();
    for rec in &doc.records {
        let opt_val = rec
            .values
            .get(&group_col.id)
            .map(|v| v.as_text())
            .filter(|t| group_col.options.iter().any(|o| o.id == *t))
            .unwrap_or_else(|| first_opt_id.clone());
        if let Some(opt_ref) = group_col.options.iter().find(|o| o.id == opt_val) {
            grouped.entry(opt_ref.id.as_str()).or_default().push(rec);
        }
    }
    for list in grouped.values_mut() {
        list.sort_by_key(|r| (r.order, r.created_at));
    }

    let total_cards = doc.records.len();
    let done_cards = group_col
        .options
        .last()
        .and_then(|last| grouped.get(last.id.as_str()))
        .map(|v| v.len())
        .unwrap_or(0);

    egui::Frame::none()
        .fill(p.bg_card)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(12.0))
        .show(ui, |ui| {
            let full_w = ui.available_width().max(220.0);
            ui.set_width(full_w);

            // Header bar: Icon + Board Title + Progress Pill + Open Full button
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                draw_icon(ui.painter(), ir, Icon::Board, p.accent_text);
                ui.label(
                    egui::RichText::new(&doc.title)
                        .font(font_semibold(13.5))
                        .color(p.text_primary),
                );
                let prog_txt = format!("{done_cards}/{total_cards}");
                let pg = ui
                    .painter()
                    .layout_no_wrap(prog_txt, font_mono(10.5), p.text_secondary);
                let (pr, _) = ui.allocate_exact_size(vec2(pg.size().x + 12.0, 18.0), Sense::hover());
                ui.painter().rect_filled(pr, Rounding::same(9.0), p.bg_input);
                ui.painter().galley(
                    pos2(pr.center().x - pg.size().x * 0.5, pr.center().y - pg.size().y * 0.5),
                    pg,
                    p.text_secondary,
                );

                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if secondary_button(ui, p, Some(Icon::ExternalLink), s.embedded_board_open)
                        .clicked()
                    {
                        actions.push(EmbeddedBoardAction::OpenFull(board_path.to_string()));
                    }
                });
            });

            // Slim progress bar
            if total_cards > 0 {
                ui.add_space(6.0);
                let (bar_r, _) = ui.allocate_exact_size(vec2(full_w, 3.0), Sense::hover());
                ui.painter().rect_filled(bar_r, Rounding::same(1.5), p.bg_input);
                let ratio = (done_cards as f32 / total_cards as f32).clamp(0.0, 1.0);
                if ratio > 0.0 {
                    let fill_r = Rect::from_min_size(
                        bar_r.min,
                        vec2((bar_r.width() * ratio).max(4.0), bar_r.height()),
                    );
                    ui.painter().rect_filled(fill_r, Rounding::same(1.5), p.success);
                }
            }

            ui.add_space(8.0);
            hairline_h(ui, p);
            ui.add_space(8.0);

            let num_cols = group_col.options.len().max(1);
            let gap = 8.0_f32;
            let ideal_col_w = ((full_w - gap * (num_cols as f32 - 1.0)) / num_cols as f32)
                .clamp(168.0, 232.0);

            egui::ScrollArea::horizontal()
                .id_salt(("emb_board_cols", board_path, id_salt))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;

                        for (opt_idx, opt) in group_col.options.iter().enumerate() {
                            let (pill_bg, dot_col, _txt_col) = p.token_colors(opt.color);
                            let cards = grouped.get(opt.id.as_str()).map(Vec::as_slice).unwrap_or(&[]);

                            ui.vertical(|ui| {
                                ui.set_width(ideal_col_w);
                                ui.set_max_width(ideal_col_w);

                                egui::Frame::none()
                                    .fill(p.bg_sidebar)
                                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                                    .rounding(Rounding::same(8.0))
                                    .inner_margin(Margin::same(8.0))
                                    .show(ui, |ui| {
                                        let inner_w = (ideal_col_w - 16.0).max(120.0);
                                        ui.set_width(inner_w);

                                        // Column Header
                                        ui.horizontal(|ui| {
                                            let (dr, _) =
                                                ui.allocate_exact_size(vec2(8.0, 14.0), Sense::hover());
                                            ui.painter().circle_filled(dr.center(), 3.6, dot_col);
                                            ui.label(
                                                egui::RichText::new(&opt.label)
                                                    .font(font_medium(12.0))
                                                    .color(p.text_primary),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        egui::RichText::new(cards.len().to_string())
                                                            .font(font_mono(10.5))
                                                            .color(p.text_muted),
                                                    );
                                                },
                                            );
                                        });
                                        ui.add_space(6.0);

                                        // Cards
                                        let col_len = cards.len();
                                        for (card_idx, rec) in cards.iter().enumerate() {
                                            egui::Frame::none()
                                                .fill(p.bg_card)
                                                .stroke(Stroke::new(1.0_f32, p.border_hairline))
                                                .rounding(Rounding::same(7.0))
                                                .inner_margin(Margin::symmetric(8.0, 7.0))
                                                .show(ui, |ui| {
                                                    let cw = (inner_w - 16.0).max(96.0);
                                                    ui.set_width(cw);

                                                    ui.add(
                                                        egui::Label::new(
                                                            egui::RichText::new(&rec.title)
                                                                .font(font_medium(12.0))
                                                                .color(p.text_primary),
                                                        )
                                                        .wrap(),
                                                    );

                                                    ui.add_space(4.0);
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 4.0;
                                                        if !rec.checklist.is_empty() {
                                                            let chk_done = rec
                                                                .checklist
                                                                .iter()
                                                                .filter(|c| c.done)
                                                                .count();
                                                            ui.label(
                                                                egui::RichText::new(format!(
                                                                    "☑ {chk_done}/{}",
                                                                    rec.checklist.len()
                                                                ))
                                                                .font(font_mono(10.0))
                                                                .color(p.text_muted),
                                                            );
                                                        }

                                                        ui.with_layout(
                                                            egui::Layout::right_to_left(Align::Center),
                                                            |ui| {
                                                                ui.spacing_mut().item_spacing.x = 3.0;
                                                                if mini_pill_btn(ui, p, "×", true, pill_bg)
                                                                    .on_hover_text(s.delete_btn)
                                                                    .clicked()
                                                                {
                                                                    actions.push(
                                                                        EmbeddedBoardAction::DeleteCard {
                                                                            board_path: board_path.to_string(),
                                                                            record_id: rec.id.clone(),
                                                                        },
                                                                    );
                                                                }
                                                                if opt_idx + 1 < group_col.options.len() {
                                                                    let next_opt = &group_col.options[opt_idx + 1];
                                                                    if mini_pill_btn(ui, p, "→", false, pill_bg)
                                                                        .on_hover_text(&next_opt.label)
                                                                        .clicked()
                                                                    {
                                                                        actions.push(
                                                                            EmbeddedBoardAction::MoveCard {
                                                                                board_path: board_path.to_string(),
                                                                                record_id: rec.id.clone(),
                                                                                target_option_id: next_opt.id.clone(),
                                                                            },
                                                                        );
                                                                    }
                                                                }
                                                                if opt_idx > 0 {
                                                                    let prev_opt = &group_col.options[opt_idx - 1];
                                                                    if mini_pill_btn(ui, p, "←", false, pill_bg)
                                                                        .on_hover_text(&prev_opt.label)
                                                                        .clicked()
                                                                    {
                                                                        actions.push(
                                                                            EmbeddedBoardAction::MoveCard {
                                                                                board_path: board_path.to_string(),
                                                                                record_id: rec.id.clone(),
                                                                                target_option_id: prev_opt.id.clone(),
                                                                            },
                                                                        );
                                                                    }
                                                                }
                                                                if card_idx + 1 < col_len
                                                                    && mini_pill_btn(ui, p, "↓", false, pill_bg).clicked()
                                                                {
                                                                    actions.push(
                                                                        EmbeddedBoardAction::ReorderCard {
                                                                            board_path: board_path.to_string(),
                                                                            record_id: rec.id.clone(),
                                                                            delta: 1,
                                                                        },
                                                                    );
                                                                }
                                                                if card_idx > 0
                                                                    && mini_pill_btn(ui, p, "↑", false, pill_bg).clicked()
                                                                {
                                                                    actions.push(
                                                                        EmbeddedBoardAction::ReorderCard {
                                                                            board_path: board_path.to_string(),
                                                                            record_id: rec.id.clone(),
                                                                            delta: -1,
                                                                        },
                                                                    );
                                                                }
                                                            },
                                                        );
                                                    });
                                                });
                                            ui.add_space(5.0);
                                        }

                                        // Quick-add input per column
                                        let gate_id = ui.make_persistent_id((
                                            "emb_add_foc",
                                            board_path,
                                            &opt.id,
                                            id_salt,
                                        ));
                                        let is_adding =
                                            draft_st.adding_opt.as_deref() == Some(opt.id.as_str());
                                        if is_adding {
                                            let r = styled_text_input(
                                                ui,
                                                p,
                                                &mut draft_st.draft_title,
                                                s.embedded_board_add_task,
                                                false,
                                                inner_w,
                                            );
                                            focus_once(ui.ctx(), gate_id, true, r.id);
                                            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                            let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                            if (r.lost_focus() && enter)
                                                || (r.has_focus() && enter)
                                            {
                                                let t = draft_st.draft_title.trim().to_string();
                                                if !t.is_empty() {
                                                    actions.push(EmbeddedBoardAction::AddCard {
                                                        board_path: board_path.to_string(),
                                                        option_id: opt.id.clone(),
                                                        title: t,
                                                    });
                                                }
                                                draft_st.draft_title.clear();
                                                draft_st.adding_opt = None;
                                                reset_focus_gate(ui.ctx(), gate_id);
                                            } else if esc {
                                                draft_st.draft_title.clear();
                                                draft_st.adding_opt = None;
                                                reset_focus_gate(ui.ctx(), gate_id);
                                            }
                                        } else {
                                            reset_focus_gate(ui.ctx(), gate_id);
                                            let (ar, aresp) = ui.allocate_exact_size(
                                                vec2(inner_w, 24.0),
                                                Sense::click(),
                                            );
                                            let ahov = anim_bool_eased(
                                                ui.ctx(),
                                                aresp.id.with("hov"),
                                                aresp.hovered(),
                                                0.10,
                                            );
                                            if ahov > 0.01 {
                                                ui.painter().rect_filled(
                                                    ar,
                                                    Rounding::same(5.0),
                                                    with_alpha(p.bg_hover, ahov),
                                                );
                                            }
                                            let ag = ui.painter().layout_no_wrap(
                                                s.embedded_board_add_task.to_string(),
                                                font_body(11.5),
                                                if aresp.hovered() {
                                                    p.text_primary
                                                } else {
                                                    p.text_muted
                                                },
                                            );
                                            ui.painter().galley(
                                                pos2(
                                                    ar.left() + 6.0,
                                                    ar.center().y - ag.size().y * 0.5,
                                                ),
                                                ag,
                                                if aresp.hovered() {
                                                    p.text_primary
                                                } else {
                                                    p.text_muted
                                                },
                                            );
                                            if aresp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .clicked()
                                            {
                                                draft_st.adding_opt = Some(opt.id.clone());
                                                draft_st.draft_title.clear();
                                            }
                                        }
                                    });
                            });
                        }
                    });
                });
        });

    ui.data_mut(|d| d.insert_temp(state_id, draft_st));
}

fn mini_pill_btn(
    ui: &mut Ui,
    p: &ThemePalette,
    label: &str,
    danger: bool,
    _accent_hint: Color32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(19.0, 17.0), Sense::click());
    let bg = if resp.hovered() {
        if danger {
            p.danger_soft
        } else {
            p.bg_hover
        }
    } else {
        p.bg_input
    };
    let fg = if resp.hovered() {
        if danger {
            p.danger
        } else {
            p.text_primary
        }
    } else {
        p.text_muted
    };
    ui.painter().rect_filled(rect, Rounding::same(4.0), bg);
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_mono(10.5), fg);
    ui.painter().galley(
        pos2(rect.center().x - g.size().x * 0.5, rect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
