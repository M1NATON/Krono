use crate::i18n::{strings, Lang};
use crate::ui::components::icon_button;
use crate::ui::icons::Icon;
use crate::ui::theme::{font_mono, ThemePalette};
use eframe::egui::{self, pos2, vec2, CursorIcon, Pos2, Rect, Rounding, Stroke, Ui, Vec2};

pub const MIN_ZOOM: f32 = 0.18;
pub const MAX_ZOOM: f32 = 2.8;

#[derive(Clone, Debug)]
pub struct Viewport2D {
    pub pan: Vec2,
    pub zoom: f32,
    pub target_pan: Vec2,
    pub target_zoom: f32,
}

impl Default for Viewport2D {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
            target_pan: Vec2::ZERO,
            target_zoom: 1.0,
        }
    }
}

impl Viewport2D {
    pub fn world_to_screen(&self, canvas_rect: Rect, world: Pos2) -> Pos2 {
        let c = canvas_rect.center();
        pos2(
            c.x + (world.x - self.pan.x) * self.zoom,
            c.y + (world.y - self.pan.y) * self.zoom,
        )
    }

    pub fn screen_to_world(&self, canvas_rect: Rect, screen: Pos2) -> Pos2 {
        let c = canvas_rect.center();
        pos2(
            self.pan.x + (screen.x - c.x) / self.zoom,
            self.pan.y + (screen.y - c.y) / self.zoom,
        )
    }

    pub fn world_rect_to_screen(&self, canvas_rect: Rect, world_rect: Rect) -> Rect {
        Rect::from_min_max(
            self.world_to_screen(canvas_rect, world_rect.min),
            self.world_to_screen(canvas_rect, world_rect.max),
        )
    }

    pub fn zoom_at(&mut self, canvas_rect: Rect, anchor_screen: Pos2, factor: f32, instant: bool) {
        let c = canvas_rect.center();
        let cur_zoom = self.target_zoom.max(MIN_ZOOM);
        let world_anchor = pos2(
            self.target_pan.x + (anchor_screen.x - c.x) / cur_zoom,
            self.target_pan.y + (anchor_screen.y - c.y) / cur_zoom,
        );
        let new_zoom = (cur_zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let new_pan = vec2(
            world_anchor.x - (anchor_screen.x - c.x) / new_zoom,
            world_anchor.y - (anchor_screen.y - c.y) / new_zoom,
        );
        self.target_zoom = new_zoom;
        self.target_pan = new_pan;
        if instant {
            self.zoom = new_zoom;
            self.pan = new_pan;
        }
    }

    pub fn fit_bounds(&mut self, canvas_rect: Rect, world_bounds: Rect, padding_px: f32, instant: bool) {
        let avail_w = (canvas_rect.width() - padding_px * 2.0).max(80.0);
        let avail_h = (canvas_rect.height() - padding_px * 2.0).max(80.0);
        let bw = world_bounds.width().max(80.0);
        let bh = world_bounds.height().max(80.0);
        let zoom = (avail_w / bw).min(avail_h / bh).clamp(MIN_ZOOM, 1.35);
        let center = world_bounds.center();
        self.target_zoom = zoom;
        self.target_pan = center.to_vec2();
        if instant {
            self.zoom = zoom;
            self.pan = self.target_pan;
        }
    }

    pub fn step_animation(&mut self, ctx: &egui::Context) {
        let dt = ctx.input(|i| i.stable_dt.min(0.05));
        let alpha = 1.0 - (-dt * 16.0).exp();
        let zoom_diff = (self.target_zoom - self.zoom).abs();
        let pan_diff = (self.target_pan - self.pan).length();
        if zoom_diff > 0.0008 || pan_diff > 0.25 {
            self.zoom += (self.target_zoom - self.zoom) * alpha;
            self.pan += (self.target_pan - self.pan) * alpha;
            ctx.request_repaint();
        } else {
            self.zoom = self.target_zoom;
            self.pan = self.target_pan;
        }
    }

    pub fn handle_pan_zoom(
        &mut self,
        ui: &Ui,
        canvas_rect: Rect,
        bg_resp: &egui::Response,
        allow_left_drag_pan: bool,
        wheel_zooms_directly: bool,
    ) {
        self.handle_pan_zoom_ex(
            ui,
            canvas_rect,
            bg_resp,
            allow_left_drag_pan,
            wheel_zooms_directly,
            true,
        );
    }

    pub fn handle_pan_zoom_ex(
        &mut self,
        ui: &Ui,
        canvas_rect: Rect,
        bg_resp: &egui::Response,
        allow_left_drag_pan: bool,
        wheel_zooms_directly: bool,
        allow_wheel: bool,
    ) {
        let hovered = bg_resp.hovered()
            || ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|p| canvas_rect.contains(p));

        if hovered && allow_wheel {
            let (pinch, scroll_y, pointer_pos, space_held) = ui.ctx().input(|i| {
                (
                    i.zoom_delta(),
                    i.smooth_scroll_delta.y,
                    i.pointer.hover_pos(),
                    i.key_down(egui::Key::Space),
                )
            });

            if (pinch - 1.0).abs() > 0.001 {
                let anchor = pointer_pos.unwrap_or_else(|| canvas_rect.center());
                self.zoom_at(canvas_rect, anchor, pinch, true);
            } else if wheel_zooms_directly && scroll_y.abs() > 0.5 && !space_held {
                let anchor = pointer_pos.unwrap_or_else(|| canvas_rect.center());
                let factor = (1.0 + scroll_y * 0.0016).clamp(0.75, 1.35);
                self.zoom_at(canvas_rect, anchor, factor, false);
            } else if !wheel_zooms_directly && scroll_y.abs() > 0.5 {
                let pan_delta = ui.ctx().input(|i| i.smooth_scroll_delta);
                let step = pan_delta / self.zoom.max(MIN_ZOOM);
                self.pan -= step;
                self.target_pan = self.pan;
            }
        }

        let space_down = ui.ctx().input(|i| i.key_down(egui::Key::Space));
        let middle_drag = bg_resp.dragged_by(egui::PointerButton::Middle);
        let left_drag = bg_resp.dragged_by(egui::PointerButton::Primary)
            && (allow_left_drag_pan || space_down);
        if middle_drag || left_drag {
            let delta = bg_resp.drag_delta() / self.zoom.max(MIN_ZOOM);
            self.pan -= delta;
            self.target_pan = self.pan;
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        } else if bg_resp.hovered() && (allow_left_drag_pan || space_down) {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }
    }
}

pub fn draw_dot_grid(
    painter: &egui::Painter,
    canvas_rect: Rect,
    vp: &Viewport2D,
    spacing_world: f32,
    dot_color: egui::Color32,
) {
    let spacing_px = spacing_world * vp.zoom;
    let step_world = if spacing_px < 12.0 {
        spacing_world * 4.0
    } else if spacing_px < 20.0 {
        spacing_world * 2.0
    } else {
        spacing_world
    };
    let min_w = vp.screen_to_world(canvas_rect, canvas_rect.min);
    let max_w = vp.screen_to_world(canvas_rect, canvas_rect.max);

    let start_x = (min_w.x / step_world).floor() as i32;
    let end_x = (max_w.x / step_world).ceil() as i32;
    let start_y = (min_w.y / step_world).floor() as i32;
    let end_y = (max_w.y / step_world).ceil() as i32;

    if (end_x - start_x) * (end_y - start_y) > 4500 {
        return;
    }

    let r = (1.15 * vp.zoom.sqrt()).clamp(0.8, 1.6);
    for ix in start_x..=end_x {
        let wx = (ix as f32) * step_world;
        for iy in start_y..=end_y {
            let wy = (iy as f32) * step_world;
            let sp = vp.world_to_screen(canvas_rect, pos2(wx, wy));
            if canvas_rect.contains(sp) {
                painter.circle_filled(sp, r, dot_color);
            }
        }
    }
}

pub fn render_zoom_hud(
    ui: &mut Ui,
    canvas_rect: Rect,
    p: &ThemePalette,
    lang: Lang,
    vp: &mut Viewport2D,
    fit_bounds: Option<Rect>,
) {
    let s = strings(lang);
    let hud_w = if fit_bounds.is_some() { 144.0 } else { 112.0 };
    let hud_h = 36.0;
    let hud_rect = Rect::from_min_size(
        pos2(
            (canvas_rect.right() - hud_w - 14.0).max(canvas_rect.left() + 8.0),
            (canvas_rect.bottom() - hud_h - 14.0).max(canvas_rect.top() + 8.0),
        ),
        vec2(hud_w, hud_h),
    );

    let mut hud_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(hud_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    hud_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));

    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(8.0))
        .shadow(p.shadow_elevated)
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
        .show(&mut hud_ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if icon_button(ui, p, Icon::ZoomOut, "-", false).clicked() {
                    vp.zoom_at(canvas_rect, canvas_rect.center(), 1.0 / 1.2, false);
                }
                let pct = format!("{}%", (vp.target_zoom * 100.0).round() as i32);
                let pct_resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(pct)
                            .font(font_mono(11.5))
                            .color(p.text_secondary),
                    )
                    .sense(egui::Sense::click()),
                );
                if pct_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    vp.target_zoom = 1.0;
                }
                if icon_button(ui, p, Icon::ZoomIn, "+", false).clicked() {
                    vp.zoom_at(canvas_rect, canvas_rect.center(), 1.2, false);
                }
                if let Some(bounds) = fit_bounds {
                    if icon_button(ui, p, Icon::Maximize, s.canvas_fit_all, false).clicked() {
                        vp.fit_bounds(canvas_rect, bounds, 48.0, false);
                    }
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_screen_roundtrip_and_zoom_at_cursor_invariance() {
        let mut vp = Viewport2D::default();
        let rect = Rect::from_min_size(pos2(100.0, 50.0), vec2(800.0, 600.0));
        let cursor = pos2(380.0, 240.0);
        let world_before = vp.screen_to_world(rect, cursor);
        vp.zoom_at(rect, cursor, 1.5, true);
        let world_after = vp.screen_to_world(rect, cursor);

        assert!((world_before.x - world_after.x).abs() < 0.01);
        assert!((world_before.y - world_after.y).abs() < 0.01);

        let back = vp.world_to_screen(rect, world_after);
        assert!((back.x - cursor.x).abs() < 0.01);
        assert!((back.y - cursor.y).abs() < 0.01);
    }

    #[test]
    fn fit_bounds_centers_and_clamps_zoom() {
        let mut vp = Viewport2D::default();
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let bounds = Rect::from_min_max(pos2(-200.0, -100.0), pos2(600.0, 300.0));
        vp.fit_bounds(rect, bounds, 50.0, true);
        assert!((vp.pan.x - 200.0).abs() < 0.01);
        assert!((vp.pan.y - 100.0).abs() < 0.01);
        assert!(vp.zoom >= MIN_ZOOM && vp.zoom <= 1.35);
    }
}
