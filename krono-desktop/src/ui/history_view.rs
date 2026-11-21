use crate::app::history::HistoryState;
use crate::i18n::{strings, Lang};
use crate::ui::components::{hairline_h, icon_button, primary_button, secondary_button, section_heading};
use crate::ui::icons::Icon;
use crate::ui::motion::with_alpha;
use crate::ui::theme::{font_body, font_medium, font_mono, ThemePalette};
use crate::ui::UiAction;
use eframe::egui::{self, vec2, CursorIcon, Margin, Rounding, Sense, Stroke, Ui};
use krono_core::diff::{diff_lines, DiffOp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum InspectorTab {
    #[default]
    Info,
    History,
}

fn format_commit_date(iso: &str) -> String {
    // e.g. "2026-11-21T14:32:10Z" -> "2026-11-21 14:32"
    if iso.len() >= 16 {
        let d = &iso[..10];
        let t = &iso[11..16];
        format!("{d} {t}")
    } else {
        iso.to_string()
    }
}

pub fn render_history_panel(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    path: &str,
    current_full_md: &str,
    state: &mut HistoryState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);

    ui.horizontal(|ui| {
        section_heading(ui, p, s.history_commits_title);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, p, Icon::RotateCcw, s.history_refresh_tooltip, false).clicked() {
                actions.push(UiAction::LoadNoteHistory(path.to_string()));
            }
        });
    });
    ui.add_space(6.0);

    if state.loading_commits {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new(s.history_loading)
                    .font(font_body(12.0))
                    .color(p.text_muted),
            );
        });
        return;
    }

    if let Some(err) = &state.commits_error {
        ui.label(
            egui::RichText::new(err)
                .font(font_body(12.0))
                .color(p.danger),
        );
        return;
    }

    if state.commits.is_empty() {
        ui.label(
            egui::RichText::new(s.history_empty)
                .font(font_body(12.0))
                .color(p.text_muted),
        );
        return;
    }

    let selected_sha = state.selected_sha.clone();
    egui::ScrollArea::vertical()
        .id_salt("history_commits_scroll")
        .max_height(190.0)
        .show(ui, |ui| {
            for commit in &state.commits {
                let is_sel = selected_sha.as_deref() == Some(commit.sha.as_str());
                let fill = if is_sel {
                    p.accent_soft
                } else {
                    egui::Color32::TRANSPARENT
                };
                let stroke = if is_sel {
                    Stroke::new(1.0_f32, p.accent)
                } else {
                    Stroke::NONE
                };
                let frame_resp = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin::symmetric(8.0, 5.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&commit.short_sha)
                                    .font(font_mono(11.0))
                                    .color(p.accent_text),
                            );
                            ui.label(
                                egui::RichText::new(format_commit_date(&commit.date_iso))
                                    .font(font_mono(10.5))
                                    .color(p.text_muted),
                            );
                            if !commit.author.trim().is_empty() {
                                ui.label(
                                    egui::RichText::new(&commit.author)
                                        .font(font_body(10.5))
                                        .color(p.text_muted),
                                );
                            }
                        });
                        let msg = if commit.message.is_empty() {
                            "—"
                        } else {
                            commit.message.as_str()
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(msg)
                                    .font(font_medium(12.0))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                    });

                let click_resp = ui
                    .interact(
                        frame_resp.response.rect,
                        ui.make_persistent_id(("hist_commit", &commit.sha)),
                        Sense::click(),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                if click_resp.clicked() {
                    actions.push(UiAction::SelectHistoryVersion {
                        path: path.to_string(),
                        sha: commit.sha.clone(),
                    });
                }
                ui.add_space(3.0);
            }
        });

    ui.add_space(8.0);
    hairline_h(ui, p);
    ui.add_space(8.0);

    if state.loading_content {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(
                egui::RichText::new(s.history_loading_version)
                    .font(font_body(12.0))
                    .color(p.text_muted),
            );
        });
        return;
    }

    if let Some(err) = &state.content_error {
        ui.label(
            egui::RichText::new(err)
                .font(font_body(12.0))
                .color(p.danger),
        );
        return;
    }

    let Some(hist_content) = state.selected_content.clone() else {
        ui.label(
            egui::RichText::new(s.history_select_hint)
                .font(font_body(12.0))
                .color(p.text_muted),
        );
        return;
    };

    let (diff, stats) = diff_lines(current_full_md, &hist_content);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(s.history_diff_title)
                .font(font_medium(12.0))
                .color(p.text_primary),
        );
        ui.label(
            egui::RichText::new(format!("+{} / -{}", stats.added, stats.removed))
                .font(font_mono(11.0))
                .color(p.text_muted),
        );
    });
    ui.add_space(4.0);

    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::same(6.0))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("history_diff_scroll")
                .max_height(240.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &diff {
                        let (prefix, bg, fg) = match line.op {
                            DiffOp::Insert => (
                                "+",
                                with_alpha(p.success, 0.16),
                                p.text_primary,
                            ),
                            DiffOp::Delete => (
                                "-",
                                with_alpha(p.danger, 0.16),
                                p.text_primary,
                            ),
                            DiffOp::Equal => (
                                " ",
                                egui::Color32::TRANSPARENT,
                                p.text_muted,
                            ),
                        };
                        egui::Frame::none()
                            .fill(bg)
                            .inner_margin(Margin::symmetric(4.0, 1.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("{prefix} {}", line.text))
                                            .font(font_mono(11.0))
                                            .color(fg),
                                    )
                                    .wrap(),
                                );
                            });
                    }
                });
        });

    ui.add_space(8.0);
    let sel_sha = state.selected_sha.as_deref().unwrap_or("");
    let confirming = state.confirm_restore_sha.as_deref() == Some(sel_sha);
    if confirming {
        ui.label(
            egui::RichText::new(s.history_restore_confirm_hint)
                .font(font_body(11.5))
                .color(p.warning),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            if primary_button(
                ui,
                p,
                Some(Icon::Check),
                s.history_restore_confirm_btn,
                false,
            )
            .clicked()
            {
                state.confirm_restore_sha = None;
                actions.push(UiAction::RestoreHistoryVersion {
                    path: path.to_string(),
                    content: hist_content,
                });
            }
            if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                state.confirm_restore_sha = None;
            }
        });
    } else if secondary_button(ui, p, Some(Icon::RotateCcw), s.history_restore_btn).clicked() {
        state.confirm_restore_sha = Some(sel_sha.to_string());
    }
}
