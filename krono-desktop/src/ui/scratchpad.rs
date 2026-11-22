use crate::i18n::{strings, Lang};
use crate::ui::components::{
    hairline_h, icon_button, keycap_badge, secondary_button,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::theme::{font_body, font_medium, font_semibold, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Margin, Rounding, Sense, Stroke, ViewportBuilder, ViewportClass,
    ViewportCommand, ViewportId,
};

pub const SCRATCHPAD_DEFAULT_SIZE: egui::Vec2 = vec2(520.0, 260.0);

pub fn scratchpad_viewport_id() -> ViewportId {
    ViewportId::from_hash_of("krono_scratchpad")
}

pub fn compute_scratchpad_initial_pos(
    monitor_size: Option<egui::Vec2>,
    window_size: egui::Vec2,
) -> Option<egui::Pos2> {
    monitor_size.map(|mon| {
        pos2(
            ((mon.x - window_size.x) * 0.5).max(16.0),
            ((mon.y - window_size.y) * 0.30).max(32.0),
        )
    })
}

#[derive(Default)]
pub struct ScratchpadState {
    pub open: bool,
    pub text: String,
    pub loaded_from_disk: bool,
    pub request_focus: bool,
    pub initial_pos: Option<egui::Pos2>,
    pub draft_dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScratchpadAction {
    SendToDaily,
    SaveAsNote,
    CopyAndClear,
    Clear,
    Close,
}

pub fn render_scratchpad_viewport(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut ScratchpadState,
    hotkey_label: &str,
) -> Option<ScratchpadAction> {
    if !state.open {
        return None;
    }

    let s = strings(lang);
    let mut action: Option<ScratchpadAction> = None;

    let mut builder = ViewportBuilder::default()
        .with_title(s.scratchpad_title)
        .with_always_on_top()
        .with_decorations(false)
        .with_taskbar(false)
        .with_visible(true)
        .with_resizable(true)
        .with_inner_size(SCRATCHPAD_DEFAULT_SIZE)
        .with_min_inner_size(vec2(380.0, 220.0))
        .with_active(true);

    if let Some(pos) = state.initial_pos {
        builder = builder.with_position(pos);
    }

    ctx.show_viewport_immediate(scratchpad_viewport_id(), builder, |vp_ctx, class| {
        if vp_ctx.input(|i| i.viewport().close_requested()) {
            vp_ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            action = Some(ScratchpadAction::Close);
            return;
        }

        if vp_ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = Some(ScratchpadAction::Close);
            return;
        }

        if !state.text.trim().is_empty()
            && vp_ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter))
        {
            action = Some(ScratchpadAction::SendToDaily);
            return;
        }

        if state.request_focus {
            vp_ctx.send_viewport_cmd(ViewportCommand::Visible(true));
            vp_ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            vp_ctx.send_viewport_cmd(ViewportCommand::Focus);
        }

        if class == ViewportClass::Embedded {
            egui::Window::new(s.scratchpad_title)
                .id(egui::Id::new("scratchpad_embedded_window"))
                .title_bar(false)
                .collapsible(false)
                .resizable(true)
                .default_size(SCRATCHPAD_DEFAULT_SIZE)
                .frame(
                    egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .rounding(Rounding::same(12.0_f32))
                        .inner_margin(Margin::same(14.0)),
                )
                .show(vp_ctx, |ui| {
                    render_scratchpad_contents(
                        ui,
                        vp_ctx,
                        p,
                        lang,
                        state,
                        &mut action,
                        false,
                        hotkey_label,
                    );
                });
        } else {
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_strong))
                        .rounding(Rounding::same(8.0_f32))
                        .inner_margin(Margin::same(14.0)),
                )
                .show(vp_ctx, |ui| {
                    render_scratchpad_contents(
                        ui,
                        vp_ctx,
                        p,
                        lang,
                        state,
                        &mut action,
                        true,
                        hotkey_label,
                    );
                });
        }
    });

    action
}

#[allow(clippy::too_many_arguments)]
fn render_scratchpad_contents(
    ui: &mut egui::Ui,
    vp_ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut ScratchpadState,
    action: &mut Option<ScratchpadAction>,
    native_drag: bool,
    hotkey_label: &str,
) {
    let s = strings(lang);

    ui.horizontal(|ui| {
        let header_h = 26.0;
        let drag_w = (ui.available_width() - 180.0).max(100.0);
        let (drag_rect, drag_resp) =
            ui.allocate_exact_size(vec2(drag_w, header_h), Sense::drag());

        let mut header_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(drag_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let (icon_rect, _) = header_ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        draw_icon(header_ui.painter(), icon_rect, Icon::Sparkles, p.accent);
        header_ui.add_space(6.0);
        header_ui.label(
            egui::RichText::new(s.scratchpad_title)
                .font(font_semibold(14.0))
                .color(p.text_primary),
        );

        if native_drag && drag_resp.drag_started_by(egui::PointerButton::Primary) {
            vp_ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() {
                *action = Some(ScratchpadAction::Close);
            }
            ui.add_space(6.0);
            let badge = if hotkey_label.trim().is_empty() {
                "Alt+N"
            } else {
                hotkey_label
            };
            keycap_badge(ui, p, badge);
        });
    });

    ui.add_space(8.0);
    hairline_h(ui, p);
    ui.add_space(8.0);

    let footer_reserved = 42.0;
    let editor_height = (ui.available_height() - footer_reserved).max(96.0);

    let editor_frame = egui::Frame::none()
        .fill(p.bg_canvas)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(8.0_f32))
        .inner_margin(Margin::symmetric(12.0, 10.0));

    editor_frame.show(ui, |ui| {
        ui.set_min_height(editor_height - 20.0);
        egui::ScrollArea::vertical()
            .max_height(editor_height - 20.0)
            .show(ui, |ui| {
                let edit = egui::TextEdit::multiline(&mut state.text)
                    .id(egui::Id::new("scratchpad_text_input"))
                    .desired_width(f32::INFINITY)
                    .desired_rows(6)
                    .frame(false)
                    .font(font_body(14.0))
                    .text_color(p.text_primary)
                    .hint_text(
                        egui::RichText::new(s.scratchpad_placeholder)
                            .font(font_body(13.5))
                            .color(p.text_muted),
                    );
                let resp = ui.add(edit);
                if resp.changed() {
                    state.draft_dirty = true;
                }
                if state.request_focus {
                    resp.request_focus();
                    state.request_focus = false;
                }
            });
    });

    ui.add_space(10.0);
    let has_text = !state.text.trim().is_empty();

    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                has_text,
                egui::Button::new(
                    egui::RichText::new(s.scratchpad_to_daily)
                        .font(font_medium(12.5))
                        .color(if has_text { p.accent_on_fill } else { p.text_muted }),
                )
                .fill(if has_text { p.accent } else { p.bg_input })
                .rounding(Rounding::same(7.0))
                .min_size(vec2(108.0, 30.0)),
            )
            .clicked()
        {
            *action = Some(ScratchpadAction::SendToDaily);
        }

        if secondary_button(ui, p, None, s.scratchpad_save_note).clicked() && has_text {
            *action = Some(ScratchpadAction::SaveAsNote);
        }

        if secondary_button(ui, p, None, s.scratchpad_copy_clear).clicked() && has_text {
            *action = Some(ScratchpadAction::CopyAndClear);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if secondary_button(ui, p, None, s.scratchpad_clear).clicked() && has_text {
                *action = Some(ScratchpadAction::Clear);
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_pos_centers_horizontally_in_upper_third() {
        let pos = compute_scratchpad_initial_pos(Some(vec2(1920.0, 1080.0)), SCRATCHPAD_DEFAULT_SIZE)
            .expect("monitor size provided");
        assert!((pos.x - 700.0).abs() < 0.01);
        assert!((pos.y - 246.0).abs() < 0.01);
    }

    #[test]
    fn initial_pos_clamps_on_small_monitor_and_handles_none() {
        let small = compute_scratchpad_initial_pos(Some(vec2(400.0, 240.0)), SCRATCHPAD_DEFAULT_SIZE)
            .expect("monitor size provided");
        assert_eq!(small, pos2(16.0, 32.0));
        assert!(compute_scratchpad_initial_pos(None, SCRATCHPAD_DEFAULT_SIZE).is_none());
    }
}
