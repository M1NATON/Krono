use crate::i18n::{strings, Lang, Strings};
use crate::storage::index::VaultTaskRow;
use crate::ui::components::{hairline_h, icon_button, secondary_button, styled_text_input, tag_chip};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, font_serif_bold, ThemePalette};
use crate::ui::{LayoutClass, UiAction, ViewMode};
use eframe::egui::{self, pos2, vec2, Align, Color32, CursorIcon, Layout, Margin, Rect, Rounding, Sense, Stroke, Ui};
use krono_core::{
    daily::SimpleDate,
    markdown::TaskPriority,
    tasks::TaskRecurrence,
};

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TasksTab {
    #[default]
    Active,
    Archive,
}

#[derive(Default)]
pub struct TasksViewState {
    pub query: String,
    pub show_no_date: bool,
    pub quick_add_draft: String,
    pub tab: TasksTab,
    pub completed_today_collapsed: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn render_tasks_view(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    tasks: &[VaultTaskRow],
    state: &mut TasksViewState,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    let is_mobile = layout.is_mobile();

    if !is_mobile {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(s.tasks_title)
                    .font(font_serif_bold(24.0))
                    .color(p.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if secondary_button(ui, p, Some(Icon::CalendarDays), s.tasks_carry_over_btn).clicked() {
                    actions.push(UiAction::CarryOverDailyTasks);
                }
                ui.add_space(6.0);
                if secondary_button(ui, p, Some(Icon::Notes), s.weekly_review_btn).clicked() {
                    actions.push(UiAction::GenerateWeeklyReview);
                }
            });
        });
        ui.add_space(12.0);
    }

    // Quick-add bar -> Today's Diary
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let input_w = (ui.available_width() - 96.0).max(140.0).min(460.0);
        let resp = ui.add(
            egui::TextEdit::singleline(&mut state.quick_add_draft)
                .hint_text(s.tasks_quick_add_placeholder)
                .desired_width(input_w)
                .font(font_body(13.0)),
        );
        let enter_submit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        let btn_submit = crate::ui::components::primary_button(
            ui,
            p,
            Some(Icon::Plus),
            s.tasks_quick_add_btn,
            false,
        )
        .clicked();
        if (enter_submit || btn_submit) && !state.quick_add_draft.trim().is_empty() {
            let txt = std::mem::take(&mut state.quick_add_draft);
            actions.push(UiAction::QuickAddDailyTask(txt));
        }
    });
    ui.add_space(10.0);

    // Filter toolbar
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        let search_w = if is_mobile { ui.available_width() } else { 240.0 };
        styled_text_input(ui, p, &mut state.query, s.search_placeholder, false, search_w);

        tab_pill(ui, p, s.tasks_tab_active, state.tab == TasksTab::Active, || {
            state.tab = TasksTab::Active;
        });
        tab_pill(ui, p, s.tasks_tab_archive, state.tab == TasksTab::Archive, || {
            state.tab = TasksTab::Archive;
        });

        if state.tab == TasksTab::Active {
            filter_toggle_pill(ui, p, s.tasks_show_no_date, &mut state.show_no_date);
        }

        if is_mobile {
            if secondary_button(ui, p, Some(Icon::CalendarDays), s.tasks_carry_over_btn).clicked() {
                actions.push(UiAction::CarryOverDailyTasks);
            }
        }
    });
    ui.add_space(12.0);

    let q_lower = state.query.trim().to_lowercase();
    let matches_query = |t: &VaultTaskRow| -> bool {
        if q_lower.is_empty() {
            return true;
        }
        t.clean_text.to_lowercase().contains(&q_lower)
            || t.note_title.to_lowercase().contains(&q_lower)
            || t.path.to_lowercase().contains(&q_lower)
            || t.tags.iter().any(|tg| tg.to_lowercase().contains(&q_lower))
    };

    if state.tab == TasksTab::Archive {
        let mut by_date: BTreeMap<Option<SimpleDate>, Vec<&VaultTaskRow>> = BTreeMap::new();
        for t in tasks {
            if t.done && matches_query(t) {
                by_date.entry(t.completed_at).or_default().push(t);
            }
        }
        egui::ScrollArea::vertical()
            .id_salt("tasks_archive_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let max_w = ui.available_width().min(860.0);
                ui.set_max_width(max_w);
                if by_date.is_empty() {
                    ui.add_space(48.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(s.tasks_archive_empty)
                                .font(font_body(14.0))
                                .color(p.text_muted),
                        );
                    });
                    return;
                }
                let mut groups: Vec<(Option<SimpleDate>, Vec<&VaultTaskRow>)> =
                    by_date.into_iter().collect();
                groups.sort_by(|a, b| match (a.0, b.0) {
                    (Some(da), Some(db)) => db.cmp(&da),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                });
                for (date_opt, mut list) in groups {
                    list.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.task_idx.cmp(&b.task_idx)));
                    let heading = match date_opt {
                        Some(d) if d == today => format!("{} · {}", s.tasks_today, d.format_iso()),
                        Some(d) => d.format_iso(),
                        None => s.tasks_completed_no_date.to_string(),
                    };
                    render_task_section(ui, p, lang, s, &heading, p.success, &list, today, &mut actions);
                    ui.add_space(14.0);
                }
            });
        return actions;
    }

    let mut overdue: Vec<&VaultTaskRow> = Vec::new();
    let mut due_today: Vec<&VaultTaskRow> = Vec::new();
    let mut upcoming: Vec<&VaultTaskRow> = Vec::new();
    let mut no_date: Vec<&VaultTaskRow> = Vec::new();
    let mut completed_today: Vec<&VaultTaskRow> = Vec::new();

    for t in tasks {
        if !matches_query(t) {
            continue;
        }
        if t.done {
            if t.completed_at == Some(today) {
                completed_today.push(t);
            }
            continue;
        }
        match t.due_date {
            Some(d) if d < today => overdue.push(t),
            Some(d) if d == today => due_today.push(t),
            Some(_) => upcoming.push(t),
            None => {
                if state.show_no_date {
                    no_date.push(t);
                }
            }
        }
    }

    let sort_group = |list: &mut Vec<&VaultTaskRow>| {
        list.sort_by(|a, b| {
            a.due_date
                .cmp(&b.due_date)
                .then_with(|| prio_rank(a.priority).cmp(&prio_rank(b.priority)))
                .then_with(|| a.path.cmp(&b.path))
                .then_with(|| a.task_idx.cmp(&b.task_idx))
        });
    };
    sort_group(&mut overdue);
    sort_group(&mut due_today);
    sort_group(&mut upcoming);
    sort_group(&mut no_date);
    sort_group(&mut completed_today);

    egui::ScrollArea::vertical()
        .id_salt("tasks_view_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let max_w = ui.available_width().min(860.0);
            ui.set_max_width(max_w);

            let any_visible = !overdue.is_empty()
                || !due_today.is_empty()
                || !upcoming.is_empty()
                || !no_date.is_empty()
                || !completed_today.is_empty();

            if !any_visible {
                ui.add_space(48.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(s.tasks_empty)
                            .font(font_body(14.0))
                            .color(p.text_muted),
                    );
                });
                return;
            }

            if !overdue.is_empty() {
                render_task_section(ui, p, lang, s, s.tasks_overdue, p.danger, &overdue, today, &mut actions);
                ui.add_space(16.0);
            }
            if !due_today.is_empty() {
                render_task_section(ui, p, lang, s, s.tasks_today, p.accent_text, &due_today, today, &mut actions);
                ui.add_space(16.0);
            }
            if !upcoming.is_empty() {
                render_task_section(ui, p, lang, s, s.tasks_upcoming, p.text_secondary, &upcoming, today, &mut actions);
                ui.add_space(16.0);
            }
            if !no_date.is_empty() {
                render_task_section(ui, p, lang, s, s.tasks_no_date, p.text_muted, &no_date, today, &mut actions);
                ui.add_space(16.0);
            }
            if !completed_today.is_empty() {
                let hdr_resp = ui.horizontal(|ui| {
                    let chev = if state.completed_today_collapsed { "▸" } else { "▾" };
                    let lbl = format!("{chev} {}", s.tasks_completed_today);
                    let r = ui.add(
                        egui::Label::new(
                            egui::RichText::new(lbl)
                                .font(font_semibold(14.5))
                                .color(p.success),
                        )
                        .sense(Sense::click()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!("{}", completed_today.len()))
                            .font(font_mono(12.0))
                            .color(p.text_muted),
                    );
                    r
                });
                if hdr_resp.inner.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    state.completed_today_collapsed = !state.completed_today_collapsed;
                }
                if !state.completed_today_collapsed {
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_hairline))
                        .rounding(Rounding::same(10.0))
                        .inner_margin(Margin::symmetric(12.0, 6.0))
                        .show(ui, |ui| {
                            for (idx, task) in completed_today.iter().enumerate() {
                                if idx > 0 {
                                    hairline_h(ui, p);
                                }
                                render_task_row(ui, p, lang, s, task, today, &mut actions);
                            }
                        });
                }
            }
        });

    actions
}

fn tab_pill(ui: &mut Ui, p: &ThemePalette, label: &str, active: bool, on_click: impl FnOnce()) {
    let fill = if active { p.accent_soft } else { p.bg_elevated };
    let border = if active { p.accent } else { p.border_hairline };
    let text_col = if active { p.accent_text } else { p.text_secondary };
    let g = ui.painter().layout_no_wrap(label.to_string(), font_medium(12.0), text_col);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 28.0), Sense::click());
    ui.painter().rect_filled(rect, Rounding::same(7.0), fill);
    ui.painter().rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0_f32, border));
    ui.painter().galley(
        pos2(rect.left() + 10.0, rect.center().y - g.size().y * 0.5),
        g,
        text_col,
    );
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        on_click();
    }
}

fn prio_rank(p: Option<TaskPriority>) -> u8 {
    match p {
        Some(TaskPriority::High) => 0,
        Some(TaskPriority::Medium) => 1,
        Some(TaskPriority::Low) => 2,
        None => 3,
    }
}

fn filter_toggle_pill(ui: &mut Ui, p: &ThemePalette, label: &str, active: &mut bool) {
    let fill = if *active { p.accent_soft } else { p.bg_elevated };
    let border = if *active { p.accent } else { p.border_hairline };
    let text_col = if *active { p.accent_text } else { p.text_secondary };
    let g = ui.painter().layout_no_wrap(label.to_string(), font_medium(12.0), text_col);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 28.0), Sense::click());
    ui.painter().rect_filled(rect, Rounding::same(7.0), fill);
    ui.painter().rect_stroke(rect, Rounding::same(7.0), Stroke::new(1.0_f32, border));
    ui.painter().galley(
        pos2(rect.left() + 10.0, rect.center().y - g.size().y * 0.5),
        g,
        text_col,
    );
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        *active = !*active;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_task_section(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    title: &str,
    accent: Color32,
    items: &[&VaultTaskRow],
    today: SimpleDate,
    actions: &mut Vec<UiAction>,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .font(font_semibold(14.5))
                .color(accent),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(format!("{}", items.len()))
                .font(font_mono(12.0))
                .color(p.text_muted),
        );
    });
    ui.add_space(8.0);

    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(12.0, 6.0))
        .show(ui, |ui| {
            for (idx, task) in items.iter().enumerate() {
                if idx > 0 {
                    hairline_h(ui, p);
                }
                render_task_row(ui, p, lang, s, task, today, actions);
            }
        });
}

fn render_task_row(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    s: &Strings,
    task: &VaultTaskRow,
    today: SimpleDate,
    actions: &mut Vec<UiAction>,
) {
    let row_id = ui.make_persistent_id(("vault_task_row", &task.path, task.task_idx));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        // Checkbox
        let (cb_r, cb_resp) = ui.allocate_exact_size(vec2(20.0, 26.0), Sense::click());
        let box_r = Rect::from_center_size(cb_r.center(), vec2(16.0, 16.0));
        let hov = anim_bool_eased(ui.ctx(), row_id.with("cb"), cb_resp.hovered(), 0.10);
        let fill = if task.done {
            p.accent
        } else if hov > 0.01 {
            with_alpha(p.accent_soft, hov)
        } else {
            p.bg_input
        };
        let stroke_col = if task.done { p.accent } else { p.border_subtle };
        ui.painter().rect_filled(box_r, Rounding::same(4.5), fill);
        ui.painter().rect_stroke(box_r, Rounding::same(4.5), Stroke::new(1.2_f32, stroke_col));
        if task.done {
            draw_icon(
                ui.painter(),
                Rect::from_center_size(box_r.center(), vec2(11.0, 11.0)),
                Icon::Check,
                Color32::WHITE,
            );
        }
        let cb_resp = cb_resp.on_hover_cursor(CursorIcon::PointingHand);
        let cb_resp = if task.done {
            cb_resp.on_hover_text(s.tasks_restore_hint)
        } else {
            cb_resp
        };
        if cb_resp.clicked() {
            actions.push(UiAction::ToggleVaultTask {
                path: task.path.clone(),
                task_idx: task.task_idx,
            });
        }

        // Main text + metadata
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.add_space(4.0);
            let mut rt = egui::RichText::new(if task.clean_text.is_empty() {
                &task.raw_text
            } else {
                &task.clean_text
            })
            .font(font_body(13.5))
            .color(if task.done { p.text_muted } else { p.text_primary });
            if task.done {
                rt = rt.strikethrough();
            }
            ui.add(egui::Label::new(rt).wrap());

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);

                if let Some(prio) = task.priority {
                    let (lbl, col) = match prio {
                        TaskPriority::High => ("!high", p.danger),
                        TaskPriority::Medium => ("!med", p.warning),
                        TaskPriority::Low => ("!low", p.text_muted),
                    };
                    mini_pill(ui, with_alpha(col, 0.14), col, lbl);
                }

                if let Some(due) = task.due_date {
                    let (bg, fg) = if !task.done && due < today {
                        (with_alpha(p.danger, 0.15), p.danger)
                    } else if due == today {
                        (p.accent_soft, p.accent_text)
                    } else {
                        (p.bg_hover, p.text_secondary)
                    };
                    mini_pill(ui, bg, fg, &due.format_iso());
                }

                if let Some(rec) = task.recurrence {
                    let lbl = match (lang, rec) {
                        (Lang::Ru, TaskRecurrence::Daily) => "↻ каждый день",
                        (Lang::Ru, TaskRecurrence::Weekly) => "↻ каждую неделю",
                        (Lang::Ru, TaskRecurrence::Monthly) => "↻ каждый месяц",
                        (Lang::En, TaskRecurrence::Daily) => "↻ daily",
                        (Lang::En, TaskRecurrence::Weekly) => "↻ weekly",
                        (Lang::En, TaskRecurrence::Monthly) => "↻ monthly",
                    };
                    mini_pill(ui, p.bg_hover, p.text_secondary, lbl);
                }

                for tag in task.tags.iter().take(3) {
                    let _ = tag_chip(ui, p, tag, false);
                }

                // Source note button
                let note_lbl = format!("↗ {}", task.note_title);
                let g = ui.painter().layout_no_wrap(note_lbl, font_medium(11.5), p.text_muted);
                let (nr, nresp) = ui.allocate_exact_size(vec2(g.size().x + 8.0, 18.0), Sense::click());
                ui.painter().galley(
                    pos2(nr.left() + 4.0, nr.center().y - g.size().y * 0.5),
                    g,
                    if nresp.hovered() { p.accent_text } else { p.text_muted },
                );
                if nresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    actions.push(UiAction::OpenNote(task.path.clone()));
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                }
            });
            ui.add_space(4.0);
        });

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icon_button(ui, p, Icon::FilePlus, s.tasks_convert_to_note, false).clicked() {
                actions.push(UiAction::ConvertTaskToNote {
                    source_path: task.path.clone(),
                    task_idx: task.task_idx,
                });
            }
        });
    });
}

fn mini_pill(ui: &mut Ui, bg: Color32, fg: Color32, text: &str) {
    let g = ui.painter().layout_no_wrap(text.to_string(), font_mono(11.0), fg);
    let (r, _) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 18.0), Sense::hover());
    ui.painter().rect_filled(r, Rounding::same(5.0), bg);
    ui.painter().galley(pos2(r.left() + 6.0, r.center().y - g.size().y * 0.5), g, fg);
}
