use crate::config::CustomSlashSnippet;
use crate::i18n::{strings, Lang};
use crate::ui::components::icon_button;
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::markdown_assist::{
    apply_slash_replacement, apply_wikilink_replacement, apply_wrap_or_insert,
    detect_assist_trigger, filter_slash_commands, filter_wikilink_candidates,
    handle_auto_list_on_enter, handle_auto_pair, render_assist_popup, AssistPopupAction,
    AssistTrigger,
};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{
    font_body, font_bold, font_medium, font_mono, font_semibold, font_serif_bold,
    font_serif_semibold, ThemePalette,
};
use crate::ui::{EditorPrefs, SettingsTab, UiAction};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, FontId, Rect, Rounding, Sense, Stroke, TextFormat, Ui,
};
use egui::text::LayoutJob;
use krono_core::{daily::SimpleDate, note::NoteMeta};
use std::collections::HashSet;

#[derive(Clone, Copy)]
enum InlineFmt {
    Heading1,
    Heading2,
    Heading3,
    Bold,
    Italic,
    Code,
    CodeBlock,
    Task,
    List,
    Quote,
    Table,
    WikiLink,
    TodayDate,
}

#[allow(clippy::too_many_arguments)]
pub fn render_markdown_editor(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    today: SimpleDate,
    body: &mut String,
    editor_id: egui::Id,
    prefs: EditorPrefs,
    custom_snippets: &[CustomSlashSnippet],
    all_notes: &[NoteMeta],
    resolved_titles: &HashSet<String>,
    focus_body: bool,
) -> (bool, Option<UiAction>) {
    let s = strings(lang);
    let mut changed = false;
    let mut extra_action: Option<UiAction> = None;

    let popup_open_id = editor_id.with("assist_open");
    let popup_sel_id = editor_id.with("assist_sel");
    let dismissed_at_id = editor_id.with("assist_dismissed");
    let prev_body_id = editor_id.with("prev_body");

    let was_popup_open = ui.data(|d| d.get_temp::<bool>(popup_open_id)).unwrap_or(false);
    let had_focus = ui.ctx().memory(|m| m.has_focus(editor_id));

    let (mut pop_up, mut pop_down, mut pop_enter, mut pop_tab, mut pop_esc) =
        (false, false, false, false, false);
    if was_popup_open && had_focus {
        ui.input_mut(|i| {
            pop_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
            pop_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
            pop_enter = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
            pop_tab = i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            pop_esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        });
    }

    let mut shortcut_fmt: Option<InlineFmt> = None;
    if had_focus {
        ui.input_mut(|i| {
            if i.modifiers.command && i.modifiers.shift && i.key_pressed(egui::Key::D) {
                shortcut_fmt = Some(InlineFmt::TodayDate);
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::B) {
                shortcut_fmt = Some(InlineFmt::Bold);
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::I) {
                shortcut_fmt = Some(InlineFmt::Italic);
            } else if i.consume_key(egui::Modifiers::COMMAND, egui::Key::E) {
                shortcut_fmt = Some(InlineFmt::Code);
            }
        });
    }

    // Optional formatting toolbar
    let mut pending_fmt: Option<InlineFmt> = shortcut_fmt;
    if prefs.md_show_toolbar {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 4.0);
            let items: &[(Icon, &str, InlineFmt)] = &[
                (Icon::Heading1, "H1", InlineFmt::Heading1),
                (Icon::Heading2, "H2", InlineFmt::Heading2),
                (Icon::Heading3, "H3", InlineFmt::Heading3),
                (Icon::Bold, "Bold (Ctrl/Cmd+B)", InlineFmt::Bold),
                (Icon::Italic, "Italic (Ctrl/Cmd+I)", InlineFmt::Italic),
                (Icon::Code, "Code (Ctrl/Cmd+E)", InlineFmt::Code),
                (Icon::Task, "Checklist", InlineFmt::Task),
                (Icon::List, "List", InlineFmt::List),
                (Icon::Quote, "Quote", InlineFmt::Quote),
                (Icon::FileCode, "Code Block", InlineFmt::CodeBlock),
                (Icon::Table, "Table", InlineFmt::Table),
            ];
            for &(icon, tip, fmt) in items {
                if toolbar_icon_chip(ui, p, icon, None, tip, false).clicked() {
                    pending_fmt = Some(fmt);
                }
            }
            ui.add_space(4.0);
            if toolbar_icon_chip(
                ui,
                p,
                Icon::Link,
                Some(s.md_toolbar_link),
                "[[Wikilink]]",
                true,
            )
            .clicked()
            {
                pending_fmt = Some(InlineFmt::WikiLink);
            }
            if toolbar_icon_chip(
                ui,
                p,
                Icon::Image,
                None,
                s.toolbar_insert_image,
                false,
            )
            .clicked()
            {
                extra_action = Some(UiAction::PickImageForEditor);
            }
            if toolbar_icon_chip(
                ui,
                p,
                Icon::CalendarDays,
                None,
                s.toolbar_insert_date,
                false,
            )
            .clicked()
            {
                pending_fmt = Some(InlineFmt::TodayDate);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, p, Icon::CircleHelp, s.md_toolbar_help, false).clicked() {
                    extra_action = Some(UiAction::OpenSettings(SettingsTab::MarkdownHelp));
                }
            });
        });
        ui.add_space(8.0);
    }

    if let Some(fmt) = pending_fmt {
        apply_toolbar_format(ui, editor_id, body, fmt, lang, today);
        changed = true;
    }

    if focus_body {
        let mut state = egui::TextEdit::load_state(ui.ctx(), editor_id).unwrap_or_default();
        if state.cursor.char_range().is_none() {
            let end_c = body.chars().count();
            state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(end_c),
            )));
            state.store(ui.ctx(), editor_id);
        }
        ui.memory_mut(|m| m.request_focus(editor_id));
    }

    let font_size = prefs.font_size;
    let heading_serif = prefs.heading_serif;
    let palette = *p;
    let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
        let job = build_editor_layout_job(text, wrap_width, font_size, heading_serif, &palette);
        ui.fonts(|f| f.layout_job(job))
    };

    let prev_body = ui
        .data(|d| d.get_temp::<String>(prev_body_id))
        .unwrap_or_else(|| body.clone());

    let mut output = egui::TextEdit::multiline(body)
        .id(editor_id)
        .frame(false)
        .desired_width(f32::INFINITY)
        .desired_rows(20)
        .margin(egui::Margin::symmetric(0.0, 4.0))
        .layouter(&mut layouter)
        .show(ui);

    if output.response.changed() {
        changed = true;
        if !(was_popup_open && (pop_enter || pop_tab)) {
            let cur_idx = output
                .state
                .cursor
                .char_range()
                .map(|r| r.primary.index)
                .unwrap_or_else(|| body.chars().count());
            let mut next_cursor: Option<usize> = None;
            if prefs.md_auto_lists {
                next_cursor = handle_auto_list_on_enter(&prev_body, body, cur_idx);
            }
            if next_cursor.is_none() && prefs.md_auto_pairs {
                next_cursor = handle_auto_pair(&prev_body, body, cur_idx);
            }
            if let Some(nc) = next_cursor {
                let cc = egui::text::CCursor::new(nc);
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                output.state.clone().store(ui.ctx(), editor_id);
            }
        }
    }

    // Quiet empty space below the last line: clicking places caret at end of note
    let (tail_rect, tail_resp) =
        ui.allocate_exact_size(vec2(ui.available_width().max(60.0), 110.0), Sense::click());
    if tail_resp.on_hover_cursor(CursorIcon::Text).clicked()
        && !ui.ctx().input(|i| i.pointer.is_decidedly_dragging())
    {
        let end_c = body.chars().count();
        let cc = egui::text::CCursor::new(end_c);
        output
            .state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(cc)));
        output.state.clone().store(ui.ctx(), editor_id);
        ui.memory_mut(|m| m.request_focus(editor_id));
    }
    let _ = tail_rect;

    ui.data_mut(|d| d.insert_temp(prev_body_id, body.clone()));

    // Floating Slash / Wikilink Assist Popup near Caret
    let mut next_popup_open = false;
    let is_focused = output.response.has_focus() || ui.ctx().memory(|m| m.has_focus(editor_id));
    if is_focused && (prefs.md_slash_menu || prefs.md_wikilink_autocomplete) {
        let cursor_char = output
            .state
            .cursor
            .char_range()
            .map(|r| r.primary.index)
            .unwrap_or(0);

        if let Some(trigger) = detect_assist_trigger(
            body,
            cursor_char,
            prefs.md_slash_trigger.as_char(),
            prefs.md_slash_menu,
            prefs.md_wikilink_autocomplete,
        ) {
            let trig_start = match &trigger {
                AssistTrigger::Slash { start_char, .. } => *start_char,
                AssistTrigger::WikiLink { start_char, .. } => *start_char,
            };
            let dismissed_at = ui.data(|d| d.get_temp::<Option<usize>>(dismissed_at_id)).flatten();
            if dismissed_at != Some(trig_start) {
                ui.data_mut(|d| d.insert_temp::<Option<usize>>(dismissed_at_id, None));

                let slash_items = match &trigger {
                    AssistTrigger::Slash { query, .. } => filter_slash_commands(
                        lang,
                        prefs.md_slash_trigger.as_char(),
                        custom_snippets,
                        query,
                    ),
                    AssistTrigger::WikiLink { .. } => Vec::new(),
                };
                let note_items = match &trigger {
                    AssistTrigger::WikiLink { query, .. } => {
                        filter_wikilink_candidates(all_notes, query)
                    }
                    AssistTrigger::Slash { .. } => Vec::new(),
                };

                let has_candidates = !slash_items.is_empty() || !note_items.is_empty();
                if has_candidates {
                    let caret_cursor = output
                        .galley
                        .from_ccursor(egui::text::CCursor::new(cursor_char));
                    let local_rect = output.galley.pos_from_cursor(&caret_cursor);
                    let anchor = output.galley_pos
                        + vec2(local_rect.left(), local_rect.bottom().max(local_rect.top() + font_size));

                    let mut sel_idx = ui.data(|d| d.get_temp::<usize>(popup_sel_id)).unwrap_or(0);
                    let act = render_assist_popup(
                        ui.ctx(),
                        p,
                        lang,
                        anchor,
                        (font_size * 1.6).round(),
                        &trigger,
                        &slash_items,
                        &note_items,
                        &mut sel_idx,
                        pop_enter,
                        pop_tab,
                        pop_esc,
                        pop_up,
                        pop_down,
                    );
                    ui.data_mut(|d| d.insert_temp(popup_sel_id, sel_idx));

                    match act {
                        Some(AssistPopupAction::AcceptSlash(cmd)) => {
                            let new_c = apply_slash_replacement(
                                body,
                                trig_start,
                                cursor_char,
                                &cmd.insert_text,
                                cmd.cursor_offset_chars,
                            );
                            let cc = egui::text::CCursor::new(new_c);
                            output
                                .state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                            output.state.clone().store(ui.ctx(), editor_id);
                            ui.memory_mut(|m| m.request_focus(editor_id));
                            changed = true;
                            if cmd.id == "image" {
                                extra_action = Some(UiAction::PickImageForEditor);
                            } else if cmd.id == "kanban" {
                                extra_action = Some(UiAction::CreateLinkedBoardForCurrentNote);
                            }
                        }
                        Some(AssistPopupAction::AcceptWikiLink { target_title }) => {
                            let has_close = matches!(
                                trigger,
                                AssistTrigger::WikiLink {
                                    has_closing_brackets: true,
                                    ..
                                }
                            );
                            let new_c = apply_wikilink_replacement(
                                body,
                                trig_start,
                                cursor_char,
                                has_close,
                                &target_title,
                            );
                            let cc = egui::text::CCursor::new(new_c);
                            output
                                .state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                            output.state.clone().store(ui.ctx(), editor_id);
                            ui.memory_mut(|m| m.request_focus(editor_id));
                            changed = true;
                        }
                        Some(AssistPopupAction::Dismiss) => {
                            ui.data_mut(|d| {
                                d.insert_temp::<Option<usize>>(dismissed_at_id, Some(trig_start))
                            });
                        }
                        None => {
                            next_popup_open = true;
                        }
                    }
                }
            }
        } else {
            ui.data_mut(|d| d.insert_temp::<Option<usize>>(dismissed_at_id, None));
        }
    }

    ui.data_mut(|d| d.insert_temp(popup_open_id, next_popup_open));
    let _ = resolved_titles;

    (changed, extra_action)
}

fn toolbar_icon_chip(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Icon,
    label_opt: Option<&str>,
    tooltip: &str,
    accented: bool,
) -> egui::Response {
    let fg_rest = if accented { p.accent_text } else { p.text_muted };
    let label_galley = label_opt.map(|l| {
        ui.painter()
            .layout_no_wrap(l.to_string(), font_medium(11.5), fg_rest)
    });
    let w = match &label_galley {
        Some(g) => g.size().x + 30.0,
        None => 26.0,
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);

    let bg = if accented {
        lerp_color(p.accent_soft, p.bg_hover, hov)
    } else {
        with_alpha(p.bg_hover, hov)
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, Rounding::same(5.0), bg);
    }

    let fg = if accented {
        p.accent_text
    } else {
        lerp_color(p.text_muted, p.text_primary, hov)
    };

    if let Some(g) = label_galley {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(rect.left() + 12.0, rect.center().y), vec2(12.5, 12.5)),
            icon,
            fg,
        );
        ui.painter().galley(
            pos2(rect.left() + 22.0, rect.center().y - g.size().y * 0.5),
            g,
            fg,
        );
    } else {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(13.5, 13.5)),
            icon,
            fg,
        );
    }

    resp.on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

fn apply_toolbar_format(
    ui: &mut Ui,
    editor_id: egui::Id,
    body: &mut String,
    fmt: InlineFmt,
    lang: Lang,
    today: SimpleDate,
) {
    let table_tpl = match lang {
        Lang::Ru => "| Колонка 1 | Колонка 2 |\n| --- | --- |\n|  |  |",
        Lang::En => "| Column 1 | Column 2 |\n| --- | --- |\n|  |  |",
    };
    let date_str = today.format_iso();
    let (prefix, suffix, line_prefix): (&str, &str, bool) = match fmt {
        InlineFmt::Heading1 => ("# ", "", true),
        InlineFmt::Heading2 => ("## ", "", true),
        InlineFmt::Heading3 => ("### ", "", true),
        InlineFmt::Bold => ("**", "**", false),
        InlineFmt::Italic => ("*", "*", false),
        InlineFmt::Code => ("`", "`", false),
        InlineFmt::CodeBlock => ("```\n", "\n```", true),
        InlineFmt::Task => ("- [ ] ", "", true),
        InlineFmt::List => ("- ", "", true),
        InlineFmt::Quote => ("> ", "", true),
        InlineFmt::Table => (table_tpl, "", true),
        InlineFmt::WikiLink => ("[[", "]]", false),
        InlineFmt::TodayDate => (date_str.as_str(), "", false),
    };

    let mut state = egui::TextEdit::load_state(ui.ctx(), editor_id).unwrap_or_default();
    let end_c = body.chars().count();
    let (sel_start, sel_end) = state
        .cursor
        .char_range()
        .map(|r| (r.primary.index, r.secondary.index))
        .unwrap_or((end_c, end_c));

    let (new_a, new_b) = apply_wrap_or_insert(body, sel_start, sel_end, prefix, suffix, line_prefix);
    let range = egui::text::CCursorRange::two(
        egui::text::CCursor::new(new_a),
        egui::text::CCursor::new(new_b),
    );
    state.cursor.set_char_range(Some(range));
    state.store(ui.ctx(), editor_id);
    ui.memory_mut(|m| m.request_focus(editor_id));
}

pub fn build_editor_layout_job(
    text: &str,
    wrap_width: f32,
    font_size: f32,
    heading_serif: bool,
    p: &ThemePalette,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    job.break_on_newline = true;

    let body_sz = font_size.clamp(14.0, 18.0);
    let base_lh = Some((body_sz * 1.6).round());
    let mono_sz = (body_sz - 1.5).max(12.0);
    let code_lh = Some((mono_sz * 1.55).round());

    let base_fmt = TextFormat {
        font_id: font_body(body_sz),
        color: p.text_primary,
        line_height: base_lh,
        ..Default::default()
    };

    let mut in_fence: Option<char> = None;

    for segment in text.split_inclusive('\n') {
        let (line, has_nl) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (segment, "")
        };

        let trimmed = line.trim_start();
        let fence_ch = if trimmed.starts_with("```") {
            Some('`')
        } else if trimmed.starts_with("~~~") {
            Some('~')
        } else {
            None
        };

        if let Some(ch) = fence_ch {
            let mut fmt = TextFormat {
                font_id: font_mono(mono_sz),
                color: p.accent_text,
                background: with_alpha(p.bg_code, 0.55),
                line_height: code_lh,
                ..Default::default()
            };
            job.append(line, 0.0, fmt.clone());
            if !has_nl.is_empty() {
                fmt.background = Color32::TRANSPARENT;
                job.append(has_nl, 0.0, fmt);
            }
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            continue;
        }

        if in_fence.is_some() {
            let mut fmt = TextFormat {
                font_id: font_mono(mono_sz),
                color: p.text_secondary,
                background: with_alpha(p.bg_code, 0.55),
                line_height: code_lh,
                ..Default::default()
            };
            job.append(line, 0.0, fmt.clone());
            if !has_nl.is_empty() {
                fmt.background = Color32::TRANSPARENT;
                job.append(has_nl, 0.0, fmt);
            }
            continue;
        }

        // Heading lines (Serif or Sans per user preference)
        if let Some((prefix_len, h_size, kind)) = detect_heading(line, body_sz) {
            let marker = &line[..prefix_len];
            let content = &line[prefix_len..];
            let h_font = match (kind, heading_serif) {
                (1, true) => font_serif_bold(h_size),
                (1, false) => font_bold(h_size),
                (2, true) => font_serif_semibold(h_size),
                (2, false) => font_semibold(h_size),
                _ => font_semibold(h_size),
            };
            let lh = h_size * 1.45;
            job.append(
                marker,
                0.0,
                TextFormat {
                    font_id: font_mono(h_size * 0.68),
                    color: p.text_muted,
                    line_height: Some(lh),
                    ..Default::default()
                },
            );
            job.append(
                content,
                0.0,
                TextFormat {
                    font_id: h_font.clone(),
                    color: p.text_primary,
                    line_height: Some(lh),
                    ..Default::default()
                },
            );
            if !has_nl.is_empty() {
                job.append(
                    has_nl,
                    0.0,
                    TextFormat {
                        font_id: h_font,
                        color: p.text_primary,
                        line_height: Some(lh),
                        ..Default::default()
                    },
                );
            }
            continue;
        }

        // Blockquotes
        if trimmed.starts_with("> ") || trimmed == ">" {
            let indent_len = line.len() - trimmed.len();
            if indent_len > 0 {
                job.append(&line[..indent_len], 0.0, base_fmt.clone());
            }
            let marker_len = if trimmed.starts_with("> ") { 2 } else { 1 };
            job.append(
                &trimmed[..marker_len],
                0.0,
                TextFormat {
                    font_id: font_bold(body_sz),
                    color: p.accent_text,
                    line_height: base_lh,
                    ..Default::default()
                },
            );
            let quote_fmt = TextFormat {
                font_id: font_body(body_sz),
                color: p.text_secondary,
                italics: true,
                line_height: base_lh,
                ..Default::default()
            };
            append_inline_markdown(&mut job, &trimmed[marker_len..], &quote_fmt, p);
            if !has_nl.is_empty() {
                job.append(has_nl, 0.0, base_fmt.clone());
            }
            continue;
        }

        // Checklists (`- [ ] ` / `- [x] `)
        if let Some((prefix, rest, done)) = detect_task_line(line) {
            job.append(
                prefix,
                0.0,
                TextFormat {
                    font_id: font_mono(mono_sz),
                    color: if done { p.success } else { p.accent_text },
                    line_height: base_lh,
                    ..Default::default()
                },
            );
            let mut task_fmt = base_fmt.clone();
            if done {
                task_fmt.color = p.text_muted;
                task_fmt.strikethrough = Stroke::new(1.0_f32, p.text_muted);
            }
            append_inline_markdown(&mut job, rest, &task_fmt, p);
            if !has_nl.is_empty() {
                job.append(has_nl, 0.0, base_fmt.clone());
            }
            continue;
        }

        // Bullets (`- ` / `* `)
        if let Some((prefix, rest)) = detect_bullet_line(line) {
            job.append(
                prefix,
                0.0,
                TextFormat {
                    font_id: font_bold(body_sz),
                    color: p.accent_text,
                    line_height: base_lh,
                    ..Default::default()
                },
            );
            append_inline_markdown(&mut job, rest, &base_fmt, p);
            if !has_nl.is_empty() {
                job.append(has_nl, 0.0, base_fmt.clone());
            }
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" {
            job.append(
                line,
                0.0,
                TextFormat {
                    font_id: font_mono(mono_sz),
                    color: p.text_muted,
                    line_height: base_lh,
                    ..Default::default()
                },
            );
            if !has_nl.is_empty() {
                job.append(has_nl, 0.0, base_fmt.clone());
            }
            continue;
        }

        append_inline_markdown(&mut job, line, &base_fmt, p);
        if !has_nl.is_empty() {
            job.append(has_nl, 0.0, base_fmt.clone());
        }
    }

    if text.is_empty() {
        job.append("", 0.0, base_fmt);
    }
    job
}

fn detect_heading(line: &str, base_size: f32) -> Option<(usize, f32, u8)> {
    let hashes = line.bytes().take_while(|&b| b == b'#').count();
    if !(1..=3).contains(&hashes) {
        return None;
    }
    if !line[hashes..].starts_with(' ') {
        return None;
    }
    let delta = base_size - 15.0;
    match hashes {
        1 => Some((2, 23.0 + delta, 1)),
        2 => Some((3, 19.0 + delta, 2)),
        3 => Some((4, 16.5 + delta, 3)),
        _ => None,
    }
}

fn detect_task_line(line: &str) -> Option<(&str, &str, bool)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    for pat in ["- [ ] ", "* [ ] "] {
        if trimmed.starts_with(pat) {
            return Some((&line[..indent + pat.len()], &trimmed[pat.len()..], false));
        }
    }
    for pat in ["- [x] ", "- [X] ", "* [x] ", "* [X] "] {
        if trimmed.starts_with(pat) {
            return Some((&line[..indent + pat.len()], &trimmed[pat.len()..], true));
        }
    }
    None
}

fn detect_bullet_line(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        return Some((&line[..indent + 2], &trimmed[2..]));
    }
    None
}

fn append_inline_markdown(
    job: &mut LayoutJob,
    line: &str,
    base: &TextFormat,
    p: &ThemePalette,
) {
    let mut i = 0usize;
    let mut plain_start = 0usize;

    while i < line.len() {
        let rest = &line[i..];

        if rest.starts_with("[[") {
            if let Some(end_rel) = rest[2..].find("]]") {
                if !rest[2..2 + end_rel].contains('\n') {
                    if i > plain_start {
                        job.append(&line[plain_start..i], 0.0, base.clone());
                    }
                    let span_len = 2 + end_rel + 2;
                    job.append(
                        &line[i..i + span_len],
                        0.0,
                        TextFormat {
                            font_id: font_medium(base.font_id.size),
                            color: p.accent_text,
                            background: with_alpha(p.accent_soft, 0.45),
                            line_height: base.line_height,
                            ..Default::default()
                        },
                    );
                    i += span_len;
                    plain_start = i;
                    continue;
                }
            }
        }

        if rest.starts_with('`') && !rest.starts_with("``") {
            if let Some(end_rel) = rest[1..].find('`') {
                if i > plain_start {
                    job.append(&line[plain_start..i], 0.0, base.clone());
                }
                let span_len = 1 + end_rel + 1;
                job.append(
                    &line[i..i + span_len],
                    0.0,
                    TextFormat {
                        font_id: FontId::monospace((base.font_id.size - 1.5).max(12.0)),
                        color: p.accent_text,
                        background: with_alpha(p.bg_code, 0.55),
                        line_height: base.line_height,
                        ..Default::default()
                    },
                );
                i += span_len;
                plain_start = i;
                continue;
            }
        }

        if rest.starts_with("**") {
            if let Some(end_rel) = rest[2..].find("**") {
                if end_rel > 0 {
                    if i > plain_start {
                        job.append(&line[plain_start..i], 0.0, base.clone());
                    }
                    let span_len = 2 + end_rel + 2;
                    let mut bold_fmt = base.clone();
                    bold_fmt.font_id = font_bold(base.font_id.size);
                    job.append(&line[i..i + 2], 0.0, muted_marker(base, p));
                    job.append(&line[i + 2..i + 2 + end_rel], 0.0, bold_fmt);
                    job.append(&line[i + 2 + end_rel..i + span_len], 0.0, muted_marker(base, p));
                    i += span_len;
                    plain_start = i;
                    continue;
                }
            }
        }

        if rest.starts_with('*') && !rest.starts_with("**") {
            if let Some(end_rel) = rest[1..].find('*') {
                if end_rel > 0 {
                    if i > plain_start {
                        job.append(&line[plain_start..i], 0.0, base.clone());
                    }
                    let span_len = 1 + end_rel + 1;
                    let mut it_fmt = base.clone();
                    it_fmt.italics = true;
                    job.append(&line[i..i + 1], 0.0, muted_marker(base, p));
                    job.append(&line[i + 1..i + 1 + end_rel], 0.0, it_fmt);
                    job.append(&line[i + 1 + end_rel..i + span_len], 0.0, muted_marker(base, p));
                    i += span_len;
                    plain_start = i;
                    continue;
                }
            }
        }

        let ch_len = rest.chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        i += ch_len;
    }

    if plain_start < line.len() {
        job.append(&line[plain_start..], 0.0, base.clone());
    }
}

fn muted_marker(base: &TextFormat, p: &ThemePalette) -> TextFormat {
    let mut f = base.clone();
    f.color = p.text_muted;
    f
}
