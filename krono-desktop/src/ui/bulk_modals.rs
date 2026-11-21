use crate::i18n::{strings, Lang};
use crate::ui::components::{hairline_h, primary_button, secondary_button};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::theme::{font_body, font_medium, font_mono, ThemePalette};
use eframe::egui::{
    self, vec2, Align, Align2, Color32, CursorIcon, Layout, Margin, Rounding, Sense, Stroke,
};
use krono_core::bulk_edit::FileReplacePreview;
use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub struct GlobalReplaceState {
    pub open: bool,
    pub find: String,
    pub replace: String,
    pub case_sensitive: bool,
    pub previews: Vec<FileReplacePreview>,
    pub excluded_paths: HashSet<String>,
    pub scanned: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TagRenameState {
    pub open: bool,
    pub old_tag: String,
    pub new_tag: String,
    pub previews: Vec<FileReplacePreview>,
    pub excluded_paths: HashSet<String>,
    pub scanned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkModalOutcome {
    None,
    Rescan,
    Apply,
}

fn bulk_modal_shell(
    ctx: &egui::Context,
    p: &ThemePalette,
    id_salt: &str,
    max_w: f32,
    open: &mut bool,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return;
    }

    let screen = ctx.screen_rect();
    let alpha = if p.is_dark { 145 } else { 85 };
    egui::Area::new(egui::Id::new((id_salt, "backdrop")))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
            );
            if resp.clicked() {
                *open = false;
            }
        });

    egui::Area::new(egui::Id::new((id_salt, "panel")))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_width(max_w.min((screen.width() - 32.0).max(300.0)));
                    add_contents(ui);
                });
        });
}

pub fn render_global_replace_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut GlobalReplaceState,
) -> BulkModalOutcome {
    if !state.open {
        return BulkModalOutcome::None;
    }
    let s = strings(lang);
    let mut outcome = BulkModalOutcome::None;
    let is_ru = lang == Lang::Ru;
    let mut keep_open = state.open;

    bulk_modal_shell(ctx, p, "global_replace_modal", 640.0, &mut keep_open, |ui| {
        ui.horizontal(|ui| {
            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            draw_icon(ui.painter(), ir, Icon::Search, p.accent_text);
            ui.label(
                egui::RichText::new(s.bulk_replace_title)
                    .font(font_medium(15.0))
                    .color(p.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new("×").frame(false)).clicked() {
                    outcome = BulkModalOutcome::None;
                    state.open = false;
                }
            });
        });
        ui.add_space(10.0);

        let mut inputs_changed = false;
        ui.horizontal(|ui| {
            ui.set_width(ui.available_width());
            let half_w = ((ui.available_width() - 96.0) * 0.5).max(120.0);
            let r1 = ui.add(
                egui::TextEdit::singleline(&mut state.find)
                    .hint_text(s.bulk_find_hint)
                    .font(font_body(13.0))
                    .desired_width(half_w),
            );
            let r2 = ui.add(
                egui::TextEdit::singleline(&mut state.replace)
                    .hint_text(s.bulk_replace_hint)
                    .font(font_body(13.0))
                    .desired_width(half_w),
            );
            if r1.changed() || r2.changed() {
                inputs_changed = true;
            }
            if secondary_button(ui, p, None, s.bulk_scan_btn).clicked()
                || ((r1.lost_focus() || r2.lost_focus())
                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                outcome = BulkModalOutcome::Rescan;
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut state.case_sensitive, s.bulk_case_sensitive)
                .changed()
            {
                inputs_changed = true;
            }
            if !state.previews.is_empty() {
                ui.add_space(12.0);
                if ui.small_button(s.bulk_select_all).clicked() {
                    state.excluded_paths.clear();
                }
                if ui.small_button(s.bulk_select_none).clicked() {
                    for prev in &state.previews {
                        state.excluded_paths.insert(prev.path.clone());
                    }
                }
            }
        });

        if inputs_changed && !state.find.is_empty() {
            outcome = BulkModalOutcome::Rescan;
        }

        ui.add_space(8.0);
        hairline_h(ui, p);
        ui.add_space(8.0);

        render_preview_list(
            ui,
            p,
            is_ru,
            &state.previews,
            &mut state.excluded_paths,
            state.scanned,
            s.bulk_no_matches,
        );

        ui.add_space(10.0);
        hairline_h(ui, p);
        ui.add_space(8.0);

        let selected_files = state
            .previews
            .iter()
            .filter(|prev| !state.excluded_paths.contains(&prev.path))
            .count();
        let selected_hits: usize = state
            .previews
            .iter()
            .filter(|prev| !state.excluded_paths.contains(&prev.path))
            .map(|prev| prev.match_count)
            .sum();

        ui.horizontal(|ui| {
            let summary = if is_ru {
                format!("Выбрано файлов: {selected_files} · совпадений: {selected_hits}")
            } else {
                format!("Selected files: {selected_files} · matches: {selected_hits}")
            };
            ui.label(
                egui::RichText::new(summary)
                    .font(font_body(12.0))
                    .color(p.text_secondary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(selected_files > 0 && !state.find.is_empty(), |ui| {
                    if primary_button(ui, p, None, s.bulk_apply_btn, false).clicked() {
                        outcome = BulkModalOutcome::Apply;
                    }
                });
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    state.open = false;
                }
            });
        });
    });

    if !keep_open {
        state.open = false;
    }
    outcome
}

pub fn render_tag_rename_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut TagRenameState,
    existing_tags: &[String],
) -> BulkModalOutcome {
    if !state.open {
        return BulkModalOutcome::None;
    }
    let s = strings(lang);
    let mut outcome = BulkModalOutcome::None;
    let is_ru = lang == Lang::Ru;
    let mut keep_open = state.open;

    bulk_modal_shell(ctx, p, "tag_rename_modal", 600.0, &mut keep_open, |ui| {
        ui.horizontal(|ui| {
            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            draw_icon(ui.painter(), ir, Icon::Sparkles, p.accent_text);
            ui.label(
                egui::RichText::new(s.tag_rename_title)
                    .font(font_medium(15.0))
                    .color(p.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.add(egui::Button::new("×").frame(false)).clicked() {
                    state.open = false;
                }
            });
        });
        ui.add_space(10.0);

        let mut changed = false;
        ui.horizontal(|ui| {
            let field_w = ((ui.available_width() - 115.0) * 0.5).max(110.0);
            let r1 = ui.add(
                egui::TextEdit::singleline(&mut state.old_tag)
                    .hint_text(s.tag_rename_old_hint)
                    .font(font_body(13.0))
                    .desired_width(field_w),
            );
            ui.label(egui::RichText::new("→").color(p.text_muted));
            let r2 = ui.add(
                egui::TextEdit::singleline(&mut state.new_tag)
                    .hint_text(s.tag_rename_new_hint)
                    .font(font_body(13.0))
                    .desired_width(field_w),
            );
            if r1.changed() || r2.changed() {
                changed = true;
            }
            if secondary_button(ui, p, None, s.bulk_scan_btn).clicked() {
                outcome = BulkModalOutcome::Rescan;
            }
        });

        if !existing_tags.is_empty() && state.old_tag.trim().is_empty() {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for tag in existing_tags.iter().take(12) {
                    let chip = ui.add(
                        egui::Button::new(
                            egui::RichText::new(format!("#{tag}"))
                                .font(font_mono(11.0))
                                .color(p.text_secondary),
                        )
                        .fill(p.bg_hover)
                        .rounding(Rounding::same(5.0)),
                    );
                    if chip.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        state.old_tag = tag.clone();
                        changed = true;
                    }
                }
            });
        }

        if changed && !state.old_tag.trim().is_empty() && !state.new_tag.trim().is_empty() {
            outcome = BulkModalOutcome::Rescan;
        }

        ui.add_space(8.0);
        hairline_h(ui, p);
        ui.add_space(8.0);

        render_preview_list(
            ui,
            p,
            is_ru,
            &state.previews,
            &mut state.excluded_paths,
            state.scanned,
            s.bulk_no_matches,
        );

        ui.add_space(10.0);
        hairline_h(ui, p);
        ui.add_space(8.0);

        let selected_files = state
            .previews
            .iter()
            .filter(|prev| !state.excluded_paths.contains(&prev.path))
            .count();

        ui.horizontal(|ui| {
            let summary = if is_ru {
                format!("Файлов к обновлению: {selected_files}")
            } else {
                format!("Files to update: {selected_files}")
            };
            ui.label(
                egui::RichText::new(summary)
                    .font(font_body(12.0))
                    .color(p.text_secondary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_enabled_ui(selected_files > 0, |ui| {
                    if primary_button(ui, p, None, s.tag_rename_apply_btn, false).clicked() {
                        outcome = BulkModalOutcome::Apply;
                    }
                });
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    state.open = false;
                }
            });
        });
    });

    if !keep_open {
        state.open = false;
    }
    outcome
}

fn render_preview_list(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    is_ru: bool,
    previews: &[FileReplacePreview],
    excluded_paths: &mut HashSet<String>,
    scanned: bool,
    empty_msg: &str,
) {
    egui::ScrollArea::vertical()
        .id_salt("bulk_preview_scroll")
        .max_height(280.0)
        .show(ui, |ui| {
            if previews.is_empty() {
                ui.add_space(16.0);
                ui.vertical_centered(|ui| {
                    let msg = if scanned {
                        empty_msg
                    } else if is_ru {
                        "Введите запрос и нажмите «Найти» для предпросмотра"
                    } else {
                        "Enter text and click Scan to preview changes"
                    };
                    ui.label(
                        egui::RichText::new(msg)
                            .font(font_body(12.5))
                            .color(p.text_muted),
                    );
                });
                ui.add_space(16.0);
                return;
            }

            for prev in previews {
                let mut included = !excluded_paths.contains(&prev.path);
                egui::Frame::none()
                    .fill(p.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(10.0, 7.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut included, "").changed() {
                                if included {
                                    excluded_paths.remove(&prev.path);
                                } else {
                                    excluded_paths.insert(prev.path.clone());
                                }
                            }
                            ui.label(
                                egui::RichText::new(&prev.path)
                                    .font(font_medium(12.5))
                                    .color(p.text_primary),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let badge = if is_ru {
                                    format!("{} совп.", prev.match_count)
                                } else {
                                    format!("{} hits", prev.match_count)
                                };
                                ui.label(
                                    egui::RichText::new(badge)
                                        .font(font_mono(11.0))
                                        .color(p.accent_text),
                                );
                            });
                        });
                        for lp in prev.lines.iter().take(4) {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("L{}:", lp.line_no))
                                        .font(font_mono(10.5))
                                        .color(p.text_muted),
                                );
                                let b_trim: String = lp.before.trim().chars().take(55).collect();
                                let a_trim: String = lp.after.trim().chars().take(55).collect();
                                ui.label(
                                    egui::RichText::new(b_trim)
                                        .font(font_mono(10.5))
                                        .color(Color32::from_rgb(210, 110, 110)),
                                );
                                ui.label(
                                    egui::RichText::new("→")
                                        .font(font_mono(10.5))
                                        .color(p.text_muted),
                                );
                                ui.label(
                                    egui::RichText::new(a_trim)
                                        .font(font_mono(10.5))
                                        .color(Color32::from_rgb(105, 195, 130)),
                                );
                            });
                        }
                        if prev.lines.len() > 4 {
                            let rest = prev.lines.len() - 4;
                            let more = if is_ru {
                                format!("+ ещё {rest} строк…")
                            } else {
                                format!("+ {rest} more lines…")
                            };
                            ui.label(
                                egui::RichText::new(more)
                                    .font(font_mono(10.0))
                                    .color(p.text_muted),
                            );
                        }
                    });
                ui.add_space(5.0);
            }
        });
}
