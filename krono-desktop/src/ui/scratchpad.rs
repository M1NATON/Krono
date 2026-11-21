use crate::i18n::{strings, Lang};
use crate::ui::components::{
    hairline_h, icon_button, keycap_badge, secondary_button,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::anim_bool_eased;
use crate::ui::theme::{font_body, font_medium, font_semibold, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Color32, Margin, Rounding, Sense, Stroke,
};

#[derive(Default)]
pub struct ScratchpadState {
    pub open: bool,
    pub text: String,
    pub loaded_from_disk: bool,
    pub request_focus: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScratchpadAction {
    SendToDaily,
    SaveAsNote,
    CopyAndClear,
    Clear,
    Close,
}

pub fn render_scratchpad_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut ScratchpadState,
) -> Option<ScratchpadAction> {
    let anim_t = anim_bool_eased(ctx, egui::Id::new("scratchpad_modal_anim"), state.open, 0.14);
    if !state.open && anim_t < 0.01 {
        return None;
    }

    let s = strings(lang);
    let mut action: Option<ScratchpadAction> = None;

    if state.open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        return Some(ScratchpadAction::Close);
    }
    if state.open
        && !state.text.trim().is_empty()
        && ctx.input(|i| (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Enter))
    {
        return Some(ScratchpadAction::SendToDaily);
    }

    let screen = ctx.screen_rect();
    let backdrop_alpha = if p.is_dark { 150.0 } else { 90.0 };
    egui::Area::new(egui::Id::new("scratchpad_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, (backdrop_alpha * anim_t) as u8),
            );
            if resp.clicked() {
                action = Some(ScratchpadAction::Close);
            }
        });

    if !state.open {
        return action;
    }

    let width = (screen.width() - 40.0).clamp(320.0, 560.0);
    let top_y = ((screen.height() - 360.0) * 0.36).clamp(24.0, 120.0) - (1.0 - anim_t) * 8.0;
    let x = screen.center().x - width * 0.5;

    egui::Area::new(egui::Id::new("scratchpad_modal_card"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(x, top_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_strong))
                .rounding(Rounding::same(12.0_f32))
                .inner_margin(Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_width(width - 32.0);

                    ui.horizontal(|ui| {
                        let (icon_rect, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        draw_icon(ui.painter(), icon_rect, Icon::Sparkles, p.accent);
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(s.scratchpad_title)
                                .font(font_semibold(14.5))
                                .color(p.text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, p, Icon::Close, s.cancel_btn, false).clicked() {
                                action = Some(ScratchpadAction::Close);
                            }
                            ui.add_space(6.0);
                            keycap_badge(ui, p, "Ctrl+Shift+Space");
                        });
                    });

                    ui.add_space(10.0);
                    hairline_h(ui, p);
                    ui.add_space(10.0);

                    let editor_frame = egui::Frame::none()
                        .fill(p.bg_canvas)
                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                        .rounding(Rounding::same(8.0_f32))
                        .inner_margin(Margin::symmetric(12.0, 10.0));

                    editor_frame.show(ui, |ui| {
                        let edit = egui::TextEdit::multiline(&mut state.text)
                            .id(egui::Id::new("scratchpad_text_input"))
                            .desired_width(f32::INFINITY)
                            .desired_rows(7)
                            .frame(false)
                            .font(font_body(14.0))
                            .text_color(p.text_primary)
                            .hint_text(
                                egui::RichText::new(s.scratchpad_placeholder)
                                    .font(font_body(13.5))
                                    .color(p.text_muted),
                            );
                        let resp = ui.add(edit);
                        if state.request_focus {
                            resp.request_focus();
                            state.request_focus = false;
                        }
                    });

                    ui.add_space(12.0);
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
                            action = Some(ScratchpadAction::SendToDaily);
                        }

                        if secondary_button(ui, p, None, s.scratchpad_save_note).clicked() && has_text {
                            action = Some(ScratchpadAction::SaveAsNote);
                        }

                        if secondary_button(ui, p, None, s.scratchpad_copy_clear).clicked() && has_text {
                            action = Some(ScratchpadAction::CopyAndClear);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if secondary_button(ui, p, None, s.scratchpad_clear).clicked() && has_text {
                                action = Some(ScratchpadAction::Clear);
                            }
                        });
                    });
                });
        });

    action
}
