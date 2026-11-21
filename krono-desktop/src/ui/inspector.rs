use crate::app::history::HistoryState;
use crate::i18n::{strings, Lang};
use crate::storage::index::BacklinkHit;
use crate::ui::components::{draw_outer_left_border, hairline_h, icon_button, section_heading};
use crate::ui::graph_view::{render_local_graph, LocalGraphData};
use crate::ui::history_view::{render_history_panel, InspectorTab};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::UiAction;
use eframe::egui::{self, pos2, vec2, Color32, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui};
use krono_core::markdown::{count_words, Heading};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct OutgoingLinkInfo {
    pub target: String,
    pub resolved_path: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_inspector(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    anim_t: f32,
    selected_path: Option<&str>,
    note_body: &str,
    headings: &[Heading],
    backlinks: &[BacklinkHit],
    outgoing: &[OutgoingLinkInfo],
    local_graph: &LocalGraphData,
    local_graph_depth: &mut usize,
    current_full_md: &str,
    history: &mut HistoryState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let Some(path) = selected_path else {
        return actions;
    };

    let target_w = 252.0;
    let current_w = (target_w * anim_t).max(1.0);

    let panel_resp = egui::SidePanel::right("krono_inspector")
        .show_separator_line(false)
        .exact_width(current_w)
        .resizable(false)
        .frame(
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .inner_margin(Margin::symmetric(14.0, 14.0)),
        )
        .show(ctx, |ui| {
            if anim_t < 0.35 {
                return;
            }
            ui.set_clip_rect(ui.max_rect().expand(4.0));
            render_inspector_contents(
                ctx,
                ui,
                p,
                lang,
                path,
                note_body,
                headings,
                backlinks,
                outgoing,
                local_graph,
                local_graph_depth,
                current_full_md,
                history,
                &mut actions,
            );
        });

    draw_outer_left_border(ctx, panel_resp.response.rect, p);
    actions
}

#[allow(clippy::too_many_arguments)]
pub fn render_inspector_drawer(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    anim_t: f32,
    selected_path: Option<&str>,
    note_body: &str,
    headings: &[Heading],
    backlinks: &[BacklinkHit],
    outgoing: &[OutgoingLinkInfo],
    local_graph: &LocalGraphData,
    local_graph_depth: &mut usize,
    current_full_md: &str,
    history: &mut HistoryState,
) -> (Vec<UiAction>, bool) {
    let mut actions = Vec::new();
    let mut dismiss = false;
    let Some(path) = selected_path else {
        return (actions, false);
    };

    let s = strings(lang);
    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };

    egui::Area::new(egui::Id::new("krono_inspector_drawer_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, (max_alpha * anim_t).round() as u8),
            );
            if resp.clicked() {
                dismiss = true;
            }
        });

    let drawer_w = 284.0_f32.min((screen.width() - 36.0).max(220.0));
    let x = screen.right() - drawer_w * anim_t;
    let mut sh = p.shadow_floating;
    sh.color = with_alpha(sh.color, anim_t);

    let area_resp = egui::Area::new(egui::Id::new("krono_inspector_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(x, screen.top()))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .shadow(sh)
                .inner_margin(Margin::symmetric(14.0, 12.0))
                .show(ui, |ui| {
                    let inner_w = (drawer_w - 28.0).max(160.0);
                    let inner_h = (screen.height() - 24.0).max(200.0);
                    ui.set_min_size(vec2(inner_w, inner_h));
                    ui.set_max_size(vec2(inner_w, inner_h));

                    if anim_t >= 0.3 {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(s.inspector_title)
                                    .font(font_semibold(14.5))
                                    .color(p.text_primary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(
                                        ui,
                                        p,
                                        Icon::Close,
                                        s.tooltip_close,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        dismiss = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        hairline_h(ui, p);
                        ui.add_space(10.0);

                        render_inspector_contents(
                            ctx,
                            ui,
                            p,
                            lang,
                            path,
                            note_body,
                            headings,
                            backlinks,
                            outgoing,
                            local_graph,
                            local_graph_depth,
                            current_full_md,
                            history,
                            &mut actions,
                        );
                    }
                });
        });

    let dr = area_resp.response.rect;
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("krono_inspector_drawer_border"),
    ))
    .line_segment(
        [pos2(dr.left().round(), dr.top()), pos2(dr.left().round(), dr.bottom())],
        Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)),
    );

    if actions.iter().any(|a| {
        matches!(
            a,
            UiAction::OpenNote(_)
                | UiAction::OpenOrCreateWikilink(_)
                | UiAction::OpenCollection(_)
                | UiAction::OpenCanvas(_)
                | UiAction::OpenGraphForNode(_)
        )
    }) {
        dismiss = true;
    }

    (actions, dismiss)
}

#[allow(clippy::too_many_arguments)]
fn render_inspector_contents(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    path: &str,
    note_body: &str,
    headings: &[Heading],
    backlinks: &[BacklinkHit],
    outgoing: &[OutgoingLinkInfo],
    local_graph: &LocalGraphData,
    local_graph_depth: &mut usize,
    current_full_md: &str,
    history: &mut HistoryState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);

    // Tab switcher: Info | History
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (tab, label) in [
            (InspectorTab::Info, s.inspector_tab_info),
            (InspectorTab::History, s.inspector_tab_history),
        ] {
            let active = history.active_tab == tab;
            let fill = if active { p.accent_soft } else { p.bg_elevated };
            let fg = if active { p.accent_text } else { p.text_secondary };
            let stroke = if active {
                Stroke::new(1.0_f32, p.accent)
            } else {
                Stroke::new(1.0_f32, p.border_hairline)
            };
            let resp = ui
                .add(
                    egui::Button::new(egui::RichText::new(label).font(font_medium(12.0)).color(fg))
                        .fill(fill)
                        .stroke(stroke)
                        .rounding(Rounding::same(6.0))
                        .min_size(vec2(64.0, 26.0)),
                )
                .on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                history.active_tab = tab;
                if tab == InspectorTab::History
                    && history.loaded_for_path.as_deref() != Some(path)
                    && !history.loading_commits
                {
                    actions.push(UiAction::LoadNoteHistory(path.to_string()));
                }
            }
        }
    });
    ui.add_space(10.0);

    if history.active_tab == InspectorTab::History {
        egui::ScrollArea::vertical()
            .id_salt("inspector_history_scroll")
            .show(ui, |ui| {
                render_history_panel(ui, p, lang, path, current_full_md, history, actions);
            });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("inspector_scroll")
        .show(ui, |ui| {
            // 1. Outline
            section_heading(ui, p, s.inspector_outline);
            ui.add_space(6.0);
            if headings.is_empty() {
                ui.label(
                    egui::RichText::new(s.inspector_no_headings)
                        .font(font_body(12.0))
                        .color(p.text_muted),
                );
            } else {
                for h in headings {
                    let indent = ((h.level.saturating_sub(1)) as f32) * 12.0;
                    ui.horizontal(|ui| {
                        ui.add_space(indent);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&h.text)
                                    .font(if h.level == 1 {
                                        font_medium(12.5)
                                    } else {
                                        font_body(12.0)
                                    })
                                    .color(if h.level == 1 {
                                        p.text_primary
                                    } else {
                                        p.text_secondary
                                    }),
                            )
                            .wrap(),
                        );
                    });
                }
            }

            ui.add_space(16.0);

            // 2. Local Mini-Graph
            render_local_graph(ui, p, lang, local_graph, local_graph_depth, actions);

            ui.add_space(16.0);

            // 3. Backlinks (O(1) cached from SQLite)
            ui.horizontal(|ui| {
                section_heading(ui, p, s.inspector_backlinks);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(backlinks.len().to_string())
                            .font(font_mono(11.0))
                            .color(p.text_muted),
                    );
                });
            });
            ui.add_space(6.0);

            if backlinks.is_empty() {
                ui.label(
                    egui::RichText::new(s.inspector_no_backlinks)
                        .font(font_body(12.0))
                        .color(p.text_muted),
                );
            } else {
                for bl in backlinks {
                    let frame = egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_hairline))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(Margin::same(8.0));
                    let inner = frame.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let (ir, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
                            draw_icon(
                                ui.painter(),
                                Rect::from_center_size(ir.center(), vec2(11.0, 11.0)),
                                Icon::Link,
                                p.accent_text,
                            );
                            ui.label(
                                egui::RichText::new(&bl.source_title)
                                    .font(font_medium(12.5))
                                    .color(p.accent_text),
                            );
                        });
                        if !bl.context.is_empty() {
                            ui.add_space(2.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&bl.context)
                                        .font(font_body(11.5))
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        }
                    });
                    if ui
                        .interact(
                            inner.response.rect,
                            ui.id().with(("bl", &bl.source_path)),
                            Sense::click(),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        actions.push(UiAction::OpenNote(bl.source_path.clone()));
                    }
                    ui.add_space(6.0);
                }
            }

            ui.add_space(18.0);

            // 3. Outgoing Wikilinks
            section_heading(ui, p, s.inspector_outgoing);
            ui.add_space(6.0);
            if outgoing.is_empty() {
                ui.label(
                    egui::RichText::new(s.inspector_no_outgoing)
                        .font(font_body(12.0))
                        .color(p.text_muted),
                );
            } else {
                let mut seen = HashSet::new();
                for out in outgoing {
                    if !seen.insert(out.target.to_lowercase()) {
                        continue;
                    }
                    let resolved = out.resolved_path.is_some();
                    let w = ui.available_width();
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 25.0), Sense::click());
                    let hov = anim_bool_eased(ctx, resp.id.with("hov"), resp.hovered(), 0.10);
                    if hov > 0.01 {
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(5.0),
                            with_alpha(p.bg_hover, hov),
                        );
                    }
                    let ic = if resolved { Icon::OutLink } else { Icon::Plus };
                    let col = if resolved {
                        p.accent_text
                    } else {
                        p.text_secondary
                    };
                    draw_icon(
                        ui.painter(),
                        Rect::from_center_size(
                            pos2(rect.left() + 10.0, rect.center().y),
                            vec2(12.0, 12.0),
                        ),
                        ic,
                        col,
                    );
                    let g = ui
                        .painter()
                        .layout_no_wrap(out.target.clone(), font_medium(12.5), col);
                    ui.painter().galley(
                        pos2(rect.left() + 24.0, rect.center().y - g.size().y * 0.5),
                        g,
                        col,
                    );
                    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
                    let resp = if !resolved {
                        resp.on_hover_text(s.tooltip_create_linked)
                    } else {
                        resp
                    };
                    if resp.clicked() {
                        actions.push(UiAction::OpenOrCreateWikilink(out.target.clone()));
                    }
                }
            }

            ui.add_space(18.0);

            // 4. Document Stats (monospace tabular values)
            section_heading(ui, p, s.inspector_stats);
            ui.add_space(6.0);
            let (words, chars) = count_words(note_body);
            let mins = ((words as f32) / 200.0).ceil().max(1.0) as usize;
            egui::Grid::new("doc_stats_grid")
                .num_columns(2)
                .spacing([14.0, 5.0])
                .show(ui, |ui| {
                    stat_row(ui, p, s.stats_words, &words.to_string());
                    stat_row(ui, p, s.stats_chars, &chars.to_string());
                    stat_row(
                        ui,
                        p,
                        s.stats_read_time,
                        &format!("~{mins} {}", s.min_unit),
                    );
                    stat_row(ui, p, s.stats_file, path);
                });
        });
}

fn stat_row(ui: &mut Ui, p: &ThemePalette, label: &str, value: &str) {
    ui.label(
        egui::RichText::new(label)
            .font(font_body(12.0))
            .color(p.text_muted),
    );
    ui.add(
        egui::Label::new(
            egui::RichText::new(value)
                .font(font_mono(11.5))
                .color(p.text_secondary),
        )
        .wrap(),
    );
    ui.end_row();
}
