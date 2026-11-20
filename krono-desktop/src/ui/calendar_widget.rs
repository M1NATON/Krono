use crate::i18n::{month_name, strings, Lang};
use crate::ui::components::icon_button;
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, ThemePalette};
use crate::ui::UiAction;
use eframe::egui::{self, pos2, vec2, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui};
use krono_core::daily::{
    compute_daily_streak, count_entries_in_month, month_grid, next_month, parse_daily_note_path,
    prev_month, SimpleDate,
};
use std::collections::HashSet;

#[allow(clippy::too_many_arguments)]
pub fn render_diary_calendar(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    open: bool,
    view_year: &mut i32,
    view_month: &mut u32,
    today: SimpleDate,
    selected_path: Option<&str>,
    daily_dates: &HashSet<SimpleDate>,
    compact_touch: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let selected_daily = selected_path.and_then(parse_daily_note_path);
    let open_t = anim_bool_eased(ctx, egui::Id::new("diary_cal_open"), open, 0.12);

    // Header Row: Chevron + Icon + Title + Today Button
    let header_w = ui.available_width().max(120.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let today_lbl = s.diary_today_btn;
        let tg = ui
            .painter()
            .layout_no_wrap(today_lbl.to_string(), font_medium(11.0), p.accent_text);
        let today_btn_w = tg.size().x + 14.0;
        let toggle_w = (header_w - today_btn_w - 6.0).max(70.0);

        let (hrect, hresp) = ui.allocate_exact_size(vec2(toggle_w, 24.0), Sense::click());
        let hov = anim_bool_eased(ctx, hresp.id.with("hov"), hresp.hovered(), 0.10);
        if hov > 0.01 {
            ui.painter()
                .rect_filled(hrect, Rounding::same(5.0), with_alpha(p.bg_hover, hov));
        }

        let chev_c = pos2(hrect.left() + 8.0, hrect.center().y);
        let angle = open_t * std::f32::consts::FRAC_PI_2;
        let (sin_a, cos_a) = angle.sin_cos();
        let rot = |lx: f32, ly: f32| {
            pos2(
                chev_c.x + lx * cos_a - ly * sin_a,
                chev_c.y + lx * sin_a + ly * cos_a,
            )
        };
        ui.painter().add(egui::Shape::line(
            vec![rot(-1.8, -3.2), rot(1.8, 0.0), rot(-1.8, 3.2)],
            Stroke::new(1.3_f32, lerp_color(p.text_muted, p.text_secondary, hov)),
        ));

        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(hrect.left() + 22.0, hrect.center().y), vec2(13.0, 13.0)),
            Icon::CalendarDays,
            if selected_daily.is_some() { p.accent_text } else { p.text_secondary },
        );

        let title_col = lerp_color(p.text_secondary, p.text_primary, hov);
        let g = ui
            .painter()
            .layout_no_wrap(s.diary_section.to_string(), font_medium(12.0), title_col);
        ui.painter().galley(
            pos2(hrect.left() + 33.0, hrect.center().y - g.size().y * 0.5),
            g,
            title_col,
        );
        if hresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            actions.push(UiAction::ToggleCalendar);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (trect, tresp) = ui.allocate_exact_size(vec2(today_btn_w, 22.0), Sense::click());
            let thov = anim_bool_eased(ctx, tresp.id.with("hov"), tresp.hovered(), 0.10);
            ui.painter().rect_filled(
                trect,
                Rounding::same(5.0),
                lerp_color(p.accent_soft, p.bg_selected, thov),
            );
            ui.painter().galley(
                pos2(
                    trect.center().x - tg.size().x * 0.5,
                    trect.center().y - tg.size().y * 0.5,
                ),
                tg,
                p.accent_text,
            );
            if tresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                *view_year = today.year;
                *view_month = today.month;
                actions.push(UiAction::OpenTodayNote);
            }
        });
    });

    if open_t <= 0.005 {
        return;
    }

    let cal_h_id = ui.make_persistent_id(("diary_cal_h", compact_touch));
    if open_t >= 0.995 {
        ui.add_space(4.0);
        let fr = render_calendar_frame(
            ctx,
            ui,
            p,
            lang,
            view_year,
            view_month,
            today,
            selected_daily,
            daily_dates,
            compact_touch,
            actions,
        );
        ui.data_mut(|d| d.insert_temp(cal_h_id, fr.response.rect.height() + 4.0));
    } else {
        let default_h = if compact_touch { 252.0 } else { 216.0 };
        let full_h = ui.data(|d| d.get_temp::<f32>(cal_h_id)).unwrap_or(default_h);
        let visible_h = (full_h * open_t).round().max(0.0);
        if visible_h > 0.5 {
            let w = ui.available_width().max(150.0);
            let (clip_rect, _) = ui.allocate_exact_size(vec2(w, visible_h), Sense::hover());
            let content_rect =
                Rect::from_min_size(clip_rect.min, vec2(w, full_h.max(visible_h) + 8.0));
            let mut child_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            child_ui.set_clip_rect(ui.clip_rect().intersect(clip_rect));
            child_ui.multiply_opacity(open_t);
            child_ui.add_space(4.0);
            let fr = render_calendar_frame(
                ctx,
                &mut child_ui,
                p,
                lang,
                view_year,
                view_month,
                today,
                selected_daily,
                daily_dates,
                compact_touch,
                actions,
            );
            ui.data_mut(|d| d.insert_temp(cal_h_id, fr.response.rect.height() + 4.0));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_calendar_frame(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    view_year: &mut i32,
    view_month: &mut u32,
    today: SimpleDate,
    selected_daily: Option<SimpleDate>,
    daily_dates: &HashSet<SimpleDate>,
    compact_touch: bool,
    actions: &mut Vec<UiAction>,
) -> egui::InnerResponse<()> {
    let s = strings(lang);
    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(8.0, 7.0))
        .show(ui, |ui| {
            let inner_w = ui.available_width().max(150.0);
            ui.set_width(inner_w);

            // Month navigation header
            ui.horizontal(|ui| {
                if icon_button(ui, p, Icon::ChevronLeft, "<", false).clicked() {
                    let (py, pm) = prev_month(*view_year, *view_month);
                    *view_year = py;
                    *view_month = pm;
                }
                let center_w = (inner_w - 60.0).max(60.0);
                ui.allocate_ui_with_layout(
                    vec2(center_w, 24.0),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                month_name(lang, *view_month),
                                *view_year
                            ))
                            .font(font_medium(12.0))
                            .color(p.text_primary),
                        );
                    },
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, p, Icon::ChevronRight, ">", false).clicked() {
                        let (ny, nm) = next_month(*view_year, *view_month);
                        *view_year = ny;
                        *view_month = nm;
                    }
                });
            });

            ui.add_space(4.0);

            let cell_w = inner_w / 7.0;
            let week_starts_monday = lang == Lang::Ru;
            let weekdays: [&str; 7] = if week_starts_monday {
                ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"]
            } else {
                ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
            };

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for wd in weekdays {
                    let (wr, _) = ui.allocate_exact_size(vec2(cell_w, 16.0), Sense::hover());
                    let wg = ui
                        .painter()
                        .layout_no_wrap(wd.to_string(), font_body(10.0), p.text_muted);
                    ui.painter().galley(
                        pos2(wr.center().x - wg.size().x * 0.5, wr.center().y - wg.size().y * 0.5),
                        wg,
                        p.text_muted,
                    );
                }
            });

            let grid = month_grid(*view_year, *view_month, week_starts_monday);
            let cell_h = if compact_touch { 30.0 } else { 24.0 };
            for week in grid.chunks(7) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for slot in week {
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(cell_w, cell_h), Sense::click());
                        let Some(date) = *slot else {
                            continue;
                        };
                        let is_selected = selected_daily == Some(date);
                        let is_today = date == today;
                        let has_entry = daily_dates.contains(&date);

                        let hov = anim_bool_eased(
                            ctx,
                            ui.make_persistent_id(("cal_day", date)),
                            resp.hovered(),
                            0.08,
                        );
                        let pill_rect = rect.shrink2(vec2(2.0, 1.5));
                        if is_selected {
                            ui.painter()
                                .rect_filled(pill_rect, Rounding::same(5.0), p.accent_soft);
                            ui.painter().rect_stroke(
                                pill_rect,
                                Rounding::same(5.0),
                                Stroke::new(1.0_f32, p.accent),
                            );
                        } else if hov > 0.01 {
                            ui.painter().rect_filled(
                                pill_rect,
                                Rounding::same(5.0),
                                with_alpha(p.bg_hover, hov),
                            );
                        }
                        if is_today && !is_selected {
                            ui.painter().rect_stroke(
                                pill_rect,
                                Rounding::same(5.0),
                                Stroke::new(1.0_f32, p.accent),
                            );
                        }

                        let num_col = if is_selected {
                            p.accent_text
                        } else if is_today {
                            p.text_primary
                        } else if has_entry {
                            p.text_primary
                        } else {
                            p.text_secondary
                        };
                        let ng = ui.painter().layout_no_wrap(
                            date.day.to_string(),
                            if is_today || is_selected {
                                font_medium(11.0)
                            } else {
                                font_body(11.0)
                            },
                            num_col,
                        );
                        let num_y = if has_entry {
                            rect.center().y - ng.size().y * 0.5 - 2.0
                        } else {
                            rect.center().y - ng.size().y * 0.5
                        };
                        ui.painter()
                            .galley(pos2(rect.center().x - ng.size().x * 0.5, num_y), ng, num_col);

                        if has_entry {
                            ui.painter().circle_filled(
                                pos2(rect.center().x, pill_rect.bottom() - 3.5),
                                1.8,
                                if is_selected { p.accent_text } else { p.accent },
                            );
                        }

                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            actions.push(UiAction::OpenOrCreateDailyNote(date));
                        }
                    }
                });
            }

            let month_count = count_entries_in_month(daily_dates, *view_year, *view_month);
            let streak = compute_daily_streak(daily_dates, today);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{month_count} {}", s.diary_month_count))
                        .font(font_mono(10.0))
                        .color(p.text_muted),
                );
                if streak > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        ui.label(
                            egui::RichText::new(format!("{streak} {}", s.diary_streak_days))
                                .font(font_mono(10.0))
                                .color(p.accent_text),
                        );
                        let (fr, _) = ui.allocate_exact_size(vec2(11.0, 12.0), Sense::hover());
                        draw_icon(ui.painter(), fr, Icon::Flame, p.accent_text);
                    });
                }
            });
        })
}
