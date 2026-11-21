use crate::i18n::{strings, Lang};
use crate::ui::components::hairline_h;
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{CreateItemKind, NavTarget, UiAction};
use eframe::egui::{self, pos2, vec2, CursorIcon, Rect, Rounding, Sense, Stroke};

pub struct EmptyRecentEntry {
    pub title: String,
    pub folder_hint: String,
    pub icon: Icon,
    pub target: NavTarget,
}

pub fn render_empty_state(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    lang: Lang,
    recent: &[EmptyRecentEntry],
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let s = strings(lang);
    let avail = ui.available_size();
    let card_w = (avail.x - 32.0).clamp(240.0, 368.0);

    let quick_items: [(Icon, &str, Option<&str>, UiAction); 4] = [
        (
            Icon::Plus,
            s.empty_action_new_note,
            Some("Ctrl+N"),
            UiAction::OpenCreateModal(Some(CreateItemKind::Note)),
        ),
        (
            Icon::Search,
            s.empty_action_search,
            Some("Ctrl+K"),
            UiAction::OpenPalette,
        ),
        (
            Icon::CalendarDays,
            s.empty_action_today,
            None,
            UiAction::OpenTodayNote,
        ),
        (
            Icon::Canvas,
            s.empty_action_canvas,
            None,
            UiAction::OpenCreateModal(Some(CreateItemKind::Canvas)),
        ),
    ];

    let recent_n = recent.len().min(4);
    let est_h = 64.0
        + (quick_items.len() as f32) * 34.0
        + if recent_n > 0 {
            34.0 + (recent_n as f32) * 30.0
        } else {
            0.0
        };
    let top_pad = ((avail.y - est_h) * 0.42).max(20.0);

    ui.vertical_centered(|ui| {
        ui.add_space(top_pad);
        ui.allocate_ui_with_layout(
            vec2(card_w, est_h + 24.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                // Muted brand mark [K] Krono
                ui.horizontal(|ui| {
                    let badge_sz = 26.0;
                    let total_w = badge_sz + 8.0 + 48.0;
                    let lead = ((card_w - total_w) * 0.5).max(0.0);
                    ui.add_space(lead);

                    let (b_rect, _) =
                        ui.allocate_exact_size(vec2(badge_sz, badge_sz), Sense::hover());
                    ui.painter().rect_filled(
                        b_rect,
                        Rounding::same(7.0),
                        with_alpha(p.accent_soft, 0.55),
                    );
                    ui.painter().rect_stroke(
                        b_rect,
                        Rounding::same(7.0),
                        Stroke::new(1.0_f32, with_alpha(p.border_subtle, 0.9)),
                    );
                    let kg = ui.painter().layout_no_wrap(
                        "K".to_string(),
                        font_semibold(14.0),
                        with_alpha(p.accent_text, 0.85),
                    );
                    ui.painter().galley(
                        pos2(
                            b_rect.center().x - kg.size().x * 0.5,
                            b_rect.center().y - kg.size().y * 0.5,
                        ),
                        kg,
                        with_alpha(p.accent_text, 0.85),
                    );

                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Krono")
                            .font(font_semibold(17.5))
                            .color(with_alpha(p.text_secondary, 0.82)),
                    );
                });

                ui.add_space(22.0);

                // Quick actions
                for (idx, (icon, label, hotkey, action)) in quick_items.into_iter().enumerate() {
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(card_w, 32.0), Sense::click());
                    let hov = anim_bool_eased(
                        ui.ctx(),
                        ui.make_persistent_id(("empty_act", idx)),
                        resp.hovered(),
                        0.10,
                    );
                    if hov > 0.01 {
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(7.0),
                            with_alpha(p.bg_hover, hov),
                        );
                    }
                    draw_icon(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(rect.left() + 16.0, rect.center().y),
                            vec2(14.0, 14.0),
                        ),
                        icon,
                        lerp_color(p.text_muted, p.accent_text, hov),
                    );
                    let fg = lerp_color(p.text_secondary, p.text_primary, hov);
                    let lg = ui
                        .painter()
                        .layout_no_wrap(label.to_string(), font_body(13.0), fg);
                    ui.painter().galley(
                        pos2(rect.left() + 34.0, rect.center().y - lg.size().y * 0.5),
                        lg,
                        fg,
                    );
                    if let Some(hk) = hotkey {
                        let hk_col = lerp_color(
                            with_alpha(p.text_muted, 0.75),
                            p.text_secondary,
                            hov,
                        );
                        let hg = ui
                            .painter()
                            .layout_no_wrap(hk.to_string(), font_mono(11.0), hk_col);
                        ui.painter().galley(
                            pos2(
                                rect.right() - 10.0 - hg.size().x,
                                rect.center().y - hg.size().y * 0.5,
                            ),
                            hg,
                            hk_col,
                        );
                    }
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        actions.push(action);
                    }
                    ui.add_space(2.0);
                }

                // Recent items (omitted if empty)
                if !recent.is_empty() {
                    ui.add_space(14.0);
                    hairline_h(ui, p);
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(s.empty_recent_heading)
                                .font(font_medium(11.0))
                                .color(with_alpha(p.text_muted, 0.85)),
                        );
                    });
                    ui.add_space(4.0);

                    for (idx, item) in recent.iter().take(4).enumerate() {
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(card_w, 28.0), Sense::click());
                        let hov = anim_bool_eased(
                            ui.ctx(),
                            ui.make_persistent_id(("empty_rec", idx, &item.title)),
                            resp.hovered(),
                            0.10,
                        );
                        if hov > 0.01 {
                            ui.painter().rect_filled(
                                rect,
                                Rounding::same(6.0),
                                with_alpha(p.bg_hover, hov),
                            );
                        }
                        draw_icon(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(rect.left() + 16.0, rect.center().y),
                                vec2(13.0, 13.0),
                            ),
                            item.icon,
                            lerp_color(p.text_muted, p.text_secondary, hov),
                        );

                        let hint_w = if item.folder_hint.is_empty() {
                            0.0
                        } else {
                            let hg = ui.painter().layout_no_wrap(
                                item.folder_hint.clone(),
                                font_body(11.0),
                                with_alpha(p.text_muted, 0.75),
                            );
                            let hw = hg.size().x.min(card_w * 0.36);
                            ui.painter().galley(
                                pos2(
                                    rect.right() - 10.0 - hw,
                                    rect.center().y - hg.size().y * 0.5,
                                ),
                                hg,
                                with_alpha(p.text_muted, 0.75),
                            );
                            hw + 14.0
                        };

                        let title_max_w = (card_w - 42.0 - hint_w).max(60.0);
                        let fg = lerp_color(p.text_secondary, p.text_primary, hov);
                        let tg = ui.painter().layout_no_wrap(
                            item.title.clone(),
                            font_body(12.5),
                            fg,
                        );
                        let clip = ui.painter().clip_rect().intersect(Rect::from_min_max(
                            pos2(rect.left() + 34.0, rect.top()),
                            pos2(rect.left() + 34.0 + title_max_w, rect.bottom()),
                        ));
                        ui.painter().with_clip_rect(clip).galley(
                            pos2(rect.left() + 34.0, rect.center().y - tg.size().y * 0.5),
                            tg,
                            fg,
                        );

                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            actions.push(UiAction::OpenNavTarget(item.target.clone()));
                        }
                        ui.add_space(1.0);
                    }
                }
            },
        );
    });

    actions
}
