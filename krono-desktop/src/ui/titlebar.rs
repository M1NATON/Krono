use crate::i18n::{strings, Lang};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::theme::{font_body, font_medium, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Align, Color32, CursorIcon, Layout, Margin, Rect, ResizeDirection,
    Rounding, Sense, Stroke,
};

pub fn render_custom_titlebar(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    subtitle: &str,
    custom_titlebar: bool,
) {
    if !custom_titlebar {
        return;
    }

    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));

    // 6px resize hit-test zones around edges/corners, strictly disabled when maximized
    // so top-right Close button triggers without being swallowed.
    if !is_maximized {
        const EDGE: f32 = 6.0;
        const CORNER: f32 = 10.0;
        let screen = ctx.screen_rect();
        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            let left = pos.x <= screen.left() + EDGE;
            let right = pos.x >= screen.right() - EDGE;
            let top = pos.y <= screen.top() + EDGE;
            let bottom = pos.y >= screen.bottom() - EDGE;

            let c_left = pos.x <= screen.left() + CORNER;
            let c_right = pos.x >= screen.right() - CORNER;
            let c_top = pos.y <= screen.top() + CORNER;
            let c_bottom = pos.y >= screen.bottom() - CORNER;

            let dir_and_cursor = if (left && c_top) || (top && c_left) {
                Some((ResizeDirection::NorthWest, CursorIcon::ResizeNwSe))
            } else if (right && c_top) || (top && c_right) {
                Some((ResizeDirection::NorthEast, CursorIcon::ResizeNeSw))
            } else if (left && c_bottom) || (bottom && c_left) {
                Some((ResizeDirection::SouthWest, CursorIcon::ResizeNeSw))
            } else if (right && c_bottom) || (bottom && c_right) {
                Some((ResizeDirection::SouthEast, CursorIcon::ResizeNwSe))
            } else if left {
                Some((ResizeDirection::West, CursorIcon::ResizeHorizontal))
            } else if right {
                Some((ResizeDirection::East, CursorIcon::ResizeHorizontal))
            } else if top {
                Some((ResizeDirection::North, CursorIcon::ResizeVertical))
            } else if bottom {
                Some((ResizeDirection::South, CursorIcon::ResizeVertical))
            } else {
                None
            };

            if let Some((dir, cursor)) = dir_and_cursor {
                if !ctx.is_using_pointer() {
                    ctx.set_cursor_icon(cursor);
                    if ctx.input(|i| i.pointer.primary_pressed()) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    }
                }
            }
        }
    }

    let s = strings(lang);
    let bar_resp = egui::TopBottomPanel::top("krono_custom_titlebar")
        .show_separator_line(false)
        .exact_height(32.0)
        .frame(
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .inner_margin(Margin {
                    left: 10.0,
                    right: 0.0,
                    top: 0.0,
                    bottom: 0.0,
                }),
        )
        .show(ctx, |ui| {
            let full_r = ui.max_rect();
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                let controls_w = 44.0 * 3.0;
                let drag_w = (full_r.width() - controls_w - 10.0).max(80.0);

                let (drag_rect, drag_resp) =
                    ui.allocate_exact_size(vec2(drag_w, 32.0), Sense::click_and_drag());

                let icon_r = Rect::from_center_size(
                    pos2(drag_rect.left() + 8.0, drag_rect.center().y),
                    vec2(14.0, 14.0),
                );
                draw_icon(ui.painter(), icon_r, Icon::Logo, p.accent_text);

                let title_g = ui.painter().layout_no_wrap(
                    "Krono".to_string(),
                    font_medium(12.0),
                    p.text_primary,
                );
                let title_x = drag_rect.left() + 22.0;
                ui.painter().galley(
                    pos2(title_x, drag_rect.center().y - title_g.size().y * 0.5),
                    title_g.clone(),
                    p.text_primary,
                );

                let sub = subtitle.trim();
                if !sub.is_empty() && drag_w > 190.0 {
                    let sub_clip: String = sub.chars().take(48).collect();
                    let sub_text = if sub.chars().count() > 48 {
                        format!("  ·  {sub_clip}…")
                    } else {
                        format!("  ·  {sub_clip}")
                    };
                    let sub_g = ui.painter().layout_no_wrap(
                        sub_text,
                        font_body(11.5),
                        p.text_muted,
                    );
                    let sx = title_x + title_g.size().x;
                    if sx + sub_g.size().x + 8.0 < drag_rect.right() {
                        ui.painter().galley(
                            pos2(sx, drag_rect.center().y - sub_g.size().y * 0.5),
                            sub_g,
                            p.text_muted,
                        );
                    }
                }

                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                } else if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    // Close
                    let (close_r, close_resp) =
                        ui.allocate_exact_size(vec2(44.0, 32.0), Sense::click());
                    if close_resp.hovered() {
                        ui.painter().rect_filled(
                            close_r,
                            Rounding::ZERO,
                            Color32::from_rgb(232, 17, 35),
                        );
                    }
                    let cc = close_r.center();
                    let x_col = if close_resp.hovered() {
                        Color32::WHITE
                    } else {
                        p.text_secondary
                    };
                    ui.painter().line_segment(
                        [pos2(cc.x - 4.5, cc.y - 4.5), pos2(cc.x + 4.5, cc.y + 4.5)],
                        Stroke::new(1.3_f32, x_col),
                    );
                    ui.painter().line_segment(
                        [pos2(cc.x + 4.5, cc.y - 4.5), pos2(cc.x - 4.5, cc.y + 4.5)],
                        Stroke::new(1.3_f32, x_col),
                    );
                    if close_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(s.titlebar_close)
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    // Maximize / Restore
                    let (max_r, max_resp) =
                        ui.allocate_exact_size(vec2(44.0, 32.0), Sense::click());
                    if max_resp.hovered() {
                        ui.painter()
                            .rect_filled(max_r, Rounding::ZERO, p.bg_hover);
                    }
                    let mc = max_r.center();
                    let m_col = if max_resp.hovered() {
                        p.text_primary
                    } else {
                        p.text_secondary
                    };
                    if is_maximized {
                        let front = Rect::from_center_size(pos2(mc.x - 1.2, mc.y + 1.2), vec2(7.5, 7.5));
                        ui.painter()
                            .rect_stroke(front, Rounding::ZERO, Stroke::new(1.2_f32, m_col));
                        ui.painter().line_segment(
                            [pos2(mc.x - 2.0, mc.y - 4.2), pos2(mc.x + 4.5, mc.y - 4.2)],
                            Stroke::new(1.2_f32, m_col),
                        );
                        ui.painter().line_segment(
                            [pos2(mc.x + 4.5, mc.y - 4.2), pos2(mc.x + 4.5, mc.y + 2.2)],
                            Stroke::new(1.2_f32, m_col),
                        );
                    } else {
                        let box_r = Rect::from_center_size(mc, vec2(9.0, 9.0));
                        ui.painter()
                            .rect_stroke(box_r, Rounding::ZERO, Stroke::new(1.2_f32, m_col));
                    }
                    let max_tip = if is_maximized {
                        s.titlebar_restore
                    } else {
                        s.titlebar_maximize
                    };
                    if max_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(max_tip)
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }

                    // Minimize
                    let (min_r, min_resp) =
                        ui.allocate_exact_size(vec2(44.0, 32.0), Sense::click());
                    if min_resp.hovered() {
                        ui.painter()
                            .rect_filled(min_r, Rounding::ZERO, p.bg_hover);
                    }
                    let nc = min_r.center();
                    let n_col = if min_resp.hovered() {
                        p.text_primary
                    } else {
                        p.text_secondary
                    };
                    ui.painter().line_segment(
                        [pos2(nc.x - 4.5, nc.y + 0.5), pos2(nc.x + 4.5, nc.y + 0.5)],
                        Stroke::new(1.3_f32, n_col),
                    );
                    if min_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(s.titlebar_minimize)
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });

    let r = bar_resp.response.rect;
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("krono_titlebar_border"),
    ))
    .line_segment(
        [pos2(r.left(), r.bottom()), pos2(r.right(), r.bottom())],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}
