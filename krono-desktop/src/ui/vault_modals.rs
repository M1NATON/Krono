use crate::app::vault_crypto::{CryptoModalKind, VaultCryptoState};
use crate::i18n::{strings, Lang};
use crate::ui::components::{primary_button, secondary_button};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::with_alpha;
use crate::ui::theme::{font_body, font_medium, font_semibold, font_serif_bold, ThemePalette};
use crate::ui::UiAction;
use eframe::egui::{self, vec2, Align2, Color32, Margin, Rounding, Sense, Stroke, Ui};

pub fn render_locked_note_overlay(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    path: &str,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    ui.vertical_centered(|ui| {
        ui.add_space(72.0);
        let (ir, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
        draw_icon(ui.painter(), ir, Icon::Lock, p.accent_text);
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new(s.vault_locked_title)
                .font(font_serif_bold(22.0))
                .color(p.text_primary),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(s.vault_locked_subtitle)
                .font(font_body(13.5))
                .color(p.text_muted),
        );
        ui.add_space(18.0);
        if primary_button(ui, p, Some(Icon::Unlock), s.vault_unlock_btn, false).clicked() {
            actions.push(UiAction::OpenUnlockModal(path.to_string()));
        }
    });
}

/// Renders the Master Password / Create Encrypted / Encrypt Existing modal.
/// Returns `true` if the user pressed Submit (Enter or button).
pub fn render_vault_crypto_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut VaultCryptoState,
) -> bool {
    let Some(kind) = state.modal.clone() else {
        return false;
    };
    let s = strings(lang);
    let mut submit = false;
    let mut cancel = false;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }

    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("vault_crypto_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, if p.is_dark { 150 } else { 85 }),
            );
            if resp.clicked() {
                cancel = true;
            }
        });

    let title_str = match &kind {
        CryptoModalKind::Unlock { .. } => s.vault_modal_unlock_title,
        CryptoModalKind::CreateEncrypted => s.vault_modal_create_title,
        CryptoModalKind::EncryptExisting { .. } => s.vault_modal_encrypt_title,
    };

    egui::Area::new(egui::Id::new("vault_crypto_modal"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(20.0))
                .show(ui, |ui| {
                    ui.set_width(360.0_f32.min((screen.width() - 32.0).max(260.0)));
                    ui.horizontal(|ui| {
                        let (ir, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                        draw_icon(ui.painter(), ir, Icon::Lock, p.accent_text);
                        ui.label(
                            egui::RichText::new(title_str)
                                .font(font_semibold(16.0))
                                .color(p.text_primary),
                        );
                    });
                    ui.add_space(10.0);

                    if matches!(kind, CryptoModalKind::CreateEncrypted) {
                        ui.label(
                            egui::RichText::new(s.vault_note_title_label)
                                .font(font_medium(12.5))
                                .color(p.text_secondary),
                        );
                        ui.add_space(4.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut state.title_draft)
                                .desired_width(f32::INFINITY)
                                .font(font_body(13.5)),
                        );
                        ui.add_space(10.0);
                    }

                    ui.label(
                        egui::RichText::new(s.vault_master_pw_label)
                            .font(font_medium(12.5))
                            .color(p.text_secondary),
                    );
                    ui.add_space(4.0);
                    let pw_resp = ui.add(
                        egui::TextEdit::singleline(&mut *state.password_draft)
                            .password(true)
                            .desired_width(f32::INFINITY)
                            .font(font_body(13.5)),
                    );
                    if matches!(kind, CryptoModalKind::Unlock { .. })
                        && pw_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        submit = true;
                    }

                    if matches!(
                        kind,
                        CryptoModalKind::CreateEncrypted | CryptoModalKind::EncryptExisting { .. }
                    ) {
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(s.vault_confirm_pw_label)
                                .font(font_medium(12.5))
                                .color(p.text_secondary),
                        );
                        ui.add_space(4.0);
                        let c_resp = ui.add(
                            egui::TextEdit::singleline(&mut *state.confirm_draft)
                                .password(true)
                                .desired_width(f32::INFINITY)
                                .font(font_body(13.5)),
                        );
                        if c_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            submit = true;
                        }
                    }

                    if let Some(err) = &state.modal_error {
                        ui.add_space(8.0);
                        egui::Frame::none()
                            .fill(with_alpha(p.danger, 0.14))
                            .rounding(Rounding::same(6.0))
                            .inner_margin(Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(
                                    egui::RichText::new(err)
                                        .font(font_body(12.0))
                                        .color(p.danger),
                                );
                            });
                    }

                    ui.add_space(16.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if primary_button(ui, p, Some(Icon::Check), s.vault_modal_submit, false)
                            .clicked()
                        {
                            submit = true;
                        }
                        if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                            cancel = true;
                        }
                    });
                });
        });

    if cancel {
        state.clear_modal();
        return false;
    }
    submit
}
