use crate::i18n::{strings, Lang};
use crate::storage::index::CollectionMeta;
use crate::ui::calendar_widget::render_diary_calendar;
use crate::ui::components::{draw_outer_top_border, hairline_h, tag_chip};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, lerp_color, with_alpha};
use crate::ui::sidebar::{render_pages_and_tags_tree, render_search_trigger};
use crate::ui::theme::{font_body, font_medium, ThemePalette};
use crate::ui::{CalendarUiState, PagesTreeUiState, SettingsTab, UiAction, ViewMode};
use eframe::egui::{self, pos2, vec2, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui};
use krono_core::note::NoteMeta;
use std::collections::HashSet;

pub fn render_mobile_bottom_bar(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    view_mode: ViewMode,
    mobile_showing_list: bool,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();

    let panel_resp = egui::TopBottomPanel::bottom("krono_mobile_bottom_bar")
        .show_separator_line(false)
        .exact_height(56.0)
        .frame(
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .inner_margin(Margin::symmetric(8.0, 4.0)),
        )
        .show(ctx, |ui| {
            let tabs = [
                (ViewMode::Editor, Icon::Notes, s.nav_notes),
                (ViewMode::Table, Icon::Table, s.nav_table),
                (ViewMode::Board, Icon::Board, s.nav_board),
                (ViewMode::Graph, Icon::Graph, s.nav_graph),
                (ViewMode::Settings, Icon::Settings, s.settings_title),
            ];

            let avail_w = ui.available_width().max(240.0);
            let tab_w = avail_w / (tabs.len() as f32);
            let bar_h = ui.available_height().max(44.0);

            struct TabSlot {
                mode: ViewMode,
                icon: Icon,
                label: &'static str,
                rect: Rect,
                resp: egui::Response,
                active: bool,
            }

            let mut slots = Vec::with_capacity(tabs.len());
            let mut active_center_x: Option<f32> = None;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (mode, icon, label) in tabs {
                    let (rect, resp) = ui.allocate_exact_size(vec2(tab_w, bar_h), Sense::click());
                    let active = view_mode == mode
                        || (mode == ViewMode::Editor
                            && matches!(view_mode, ViewMode::Collection | ViewMode::Canvas));
                    if active {
                        active_center_x = Some(rect.center().x);
                    }
                    slots.push(TabSlot {
                        mode,
                        icon,
                        label,
                        rect,
                        resp,
                        active,
                    });
                }
            });

            if let Some(target_cx) = active_center_x {
                let cx = anim_value_smooth(
                    ctx,
                    egui::Id::new("mobile_tab_ind_cx"),
                    target_cx,
                    0.16,
                );
                let top_y = slots.first().map(|sl| sl.rect.top()).unwrap_or(0.0);
                let ind_r = Rect::from_center_size(pos2(cx, top_y + 1.5), vec2(28.0, 2.5));
                ui.painter()
                    .rect_filled(ind_r, Rounding::same(1.5), p.accent);
            }

            for slot in slots {
                let hov = anim_bool_eased(
                    ctx,
                    ui.make_persistent_id(("mob_tab", slot.label)),
                    slot.resp.hovered(),
                    0.10,
                );
                let sel_t = anim_bool_eased(
                    ctx,
                    ui.make_persistent_id(("mob_tab_sel", slot.label)),
                    slot.active,
                    0.15,
                );

                let icon_center = pos2(slot.rect.center().x, slot.rect.top() + 17.0);
                if sel_t > 0.01 {
                    let pill_r = Rect::from_center_size(icon_center, vec2(38.0, 23.0));
                    ui.painter().rect_filled(
                        pill_r,
                        Rounding::same(11.5),
                        with_alpha(p.accent_soft, sel_t),
                    );
                }

                let fg = lerp_color(
                    lerp_color(p.text_muted, p.text_secondary, hov),
                    p.accent_text,
                    sel_t,
                );
                draw_icon(
                    ui.painter(),
                    Rect::from_center_size(icon_center, vec2(15.0, 15.0)),
                    slot.icon,
                    fg,
                );

                let g = ui.painter().layout_no_wrap(
                    slot.label.to_string(),
                    if slot.active {
                        font_medium(10.0)
                    } else {
                        font_body(10.0)
                    },
                    fg,
                );
                ui.painter().galley(
                    pos2(slot.rect.center().x - g.size().x * 0.5, slot.rect.top() + 31.5),
                    g,
                    fg,
                );

                if slot.resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    match slot.mode {
                        ViewMode::Settings => {
                            if view_mode != ViewMode::Settings {
                                actions.push(UiAction::OpenSettings(SettingsTab::Appearance));
                            }
                        }
                        ViewMode::Editor => {
                            if matches!(
                                view_mode,
                                ViewMode::Editor | ViewMode::Collection | ViewMode::Canvas
                            ) {
                                if !mobile_showing_list {
                                    actions.push(UiAction::MobileShowNotesList);
                                }
                            } else {
                                actions.push(UiAction::ReturnToNotesTab);
                            }
                        }
                        other => {
                            actions.push(UiAction::SetViewMode(other));
                        }
                    }
                }
            }
        });

    draw_outer_top_border(ctx, panel_resp.response.rect, p);
    actions
}

#[allow(clippy::too_many_arguments)]
pub fn render_mobile_notes_list(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_folders: &[String],
    tag_counts: &[(String, usize)],
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    active_tag: Option<&str>,
    expanded_folders: &HashSet<String>,
    pages_ui: &mut PagesTreeUiState<'_>,
    cal: &mut CalendarUiState<'_>,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();

    render_search_trigger(ctx, ui, p, lang, false, &mut actions);

    if !tag_counts.is_empty() {
        ui.add_space(8.0);
        egui::ScrollArea::horizontal()
            .id_salt("mobile_tags_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if active_tag.is_some() {
                        let g = ui.painter().layout_no_wrap(
                            s.all_tags_clear.to_string(),
                            font_medium(11.5),
                            p.accent_text,
                        );
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(g.size().x + 14.0, 21.0), Sense::click());
                        ui.painter()
                            .rect_filled(rect, Rounding::same(5.0), p.accent_soft);
                        ui.painter().galley(
                            pos2(
                                rect.center().x - g.size().x * 0.5,
                                rect.center().y - g.size().y * 0.5,
                            ),
                            g,
                            p.accent_text,
                        );
                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            actions.push(UiAction::SetTagFilter(None));
                        }
                    }
                    for (tag, _) in tag_counts {
                        let selected = active_tag.is_some_and(|at| at.eq_ignore_ascii_case(tag));
                        let (resp, _) = tag_chip(ui, p, tag, false);
                        if selected {
                            ui.painter().rect_stroke(
                                resp.rect,
                                Rounding::same(5.0),
                                Stroke::new(1.5_f32, p.accent),
                            );
                        }
                        if resp.clicked() {
                            if selected {
                                actions.push(UiAction::SetTagFilter(None));
                            } else {
                                actions.push(UiAction::SetTagFilter(Some(tag.clone())));
                            }
                        }
                    }
                });
            });
    }

    ui.add_space(10.0);

    egui::ScrollArea::vertical()
        .id_salt("mobile_notes_scroll")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            render_diary_calendar(
                ctx,
                ui,
                p,
                lang,
                cal.open,
                cal.year,
                cal.month,
                cal.today,
                selected_path,
                cal.daily_dates,
                true,
                &mut actions,
            );
            ui.add_space(10.0);
            hairline_h(ui, p);
            ui.add_space(10.0);
            render_pages_and_tags_tree(
                ui,
                p,
                lang,
                notes,
                collections,
                vault_folders,
                tag_counts,
                selected_path,
                selected_collection_path,
                selected_canvas_path,
                active_tag,
                expanded_folders,
                &[],
                &[],
                ViewMode::Editor,
                false,
                true,
                pages_ui,
                &mut actions,
            );
            ui.add_space(16.0);
        });

    actions
}
