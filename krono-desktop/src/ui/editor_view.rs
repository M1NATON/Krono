use crate::config::CustomSlashSnippet;
use crate::i18n::{format_relative_time, strings, Lang};
use crate::spellcheck::SpellChecker;
use crate::ui::components::{
    focus_once, icon_button, optional_status_pill_picker, primary_button, reset_focus_gate,
    secondary_button, tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::embedded_board::render_embedded_board_widget;
use crate::ui::image_cache::ImageTextureCache;
use crate::ui::markdown_editor::render_markdown_editor;
use crate::ui::markdown_preview::{
    render_markdown_preview, resolve_embedded_board, EmbeddedNoteData, PreviewAction,
};
use crate::ui::theme::{
    font_body, font_bold, font_medium, font_mono, font_serif_bold, ThemePalette,
};
use crate::ui::{EditorDocState, EditorMode, EditorPrefs, LayoutClass, SidebarModule, UiAction};
use eframe::egui::{
    self, pos2, vec2, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui,
};
use krono_core::{
    collection::CollectionDoc,
    daily::SimpleDate,
    markdown::{count_words, extract_embedded_boards},
    note::{NoteMeta, NoteStatus},
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[allow(clippy::too_many_arguments)]
pub fn render_editor_view(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    doc: EditorDocState<'_>,
    new_tag_input: &mut String,
    adding_tag: &mut bool,
    editor_mode: EditorMode,
    prefs: EditorPrefs,
    custom_snippets: &[CustomSlashSnippet],
    all_notes: &[NoteMeta],
    focus_title: &mut bool,
    focus_body: &mut bool,
    entrance_t: f32,
    resolved_titles: &HashSet<String>,
    copied_code_hash: Option<u64>,
    vault_dir: &Path,
    image_cache: &mut ImageTextureCache,
    embedded_boards: &HashMap<String, CollectionDoc>,
    available_boards: &[(String, String)],
    embedded_notes: &HashMap<String, EmbeddedNoteData>,
    spellchecker: &mut SpellChecker,
    today: SimpleDate,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();

    let Some(rel_path) = doc.selected_path else {
        render_empty_state(ui, p, lang, prefs.heading_serif, &mut actions);
        return actions;
    };

    let is_today_daily = krono_core::daily::parse_daily_note_path(rel_path) == Some(today);
    let effective_mode = if editor_mode == EditorMode::Split
        && (!layout.is_desktop() || ui.available_width() < 640.0)
    {
        EditorMode::Edit
    } else {
        editor_mode
    };

    if effective_mode == EditorMode::Split {
        egui::ScrollArea::vertical()
            .id_salt("editor_split_scroll")
            .drag_to_scroll(false)
            .show(ui, |ui| {
                ui.add_space(6.0 + (1.0 - entrance_t) * 6.0);
                if doc.external_conflict {
                    render_external_conflict_banner(ui, p, lang, &mut actions);
                    ui.add_space(10.0);
                }
                if crate::app::english::is_english_lesson_path(rel_path) {
                    render_english_lesson_nav_bar(ui, p, lang, rel_path, all_notes, &mut actions);
                    ui.add_space(10.0);
                }
                render_document_header(
                    ui,
                    p,
                    lang,
                    layout,
                    today,
                    is_today_daily,
                    doc.title,
                    doc.status,
                    doc.tags,
                    doc.pinned,
                    doc.extra_lines,
                    doc.modified,
                    new_tag_input,
                    adding_tag,
                    prefs.heading_serif,
                    focus_title,
                    focus_body,
                    &mut actions,
                );
                ui.add_space(14.0);

                let avail_w = ui.available_width().max(400.0);
                let col_w = ((avail_w - 28.0) * 0.5).max(180.0);

                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        let ed_id = egui::Id::new("krono_md_editor_split");
                        let fb = *focus_body;
                        *focus_body = false;
                        let (changed, extra) = render_markdown_editor(
                            ui,
                            p,
                            lang,
                            today,
                            doc.body,
                            ed_id,
                            prefs,
                            custom_snippets,
                            all_notes,
                            resolved_titles,
                            fb,
                            spellchecker,
                        );
                        if changed {
                            actions.push(UiAction::MarkDirty);
                        }
                        if let Some(act) = extra {
                            actions.push(act);
                        }
                    });

                    ui.add_space(12.0);
                    let sep_h = ui.available_height().max(460.0);
                    let (sep_rect, _) = ui.allocate_exact_size(vec2(1.0, sep_h), Sense::hover());
                    ui.painter().line_segment(
                        [
                            pos2(sep_rect.center().x, sep_rect.top()),
                            pos2(sep_rect.center().x, sep_rect.bottom()),
                        ],
                        Stroke::new(1.0_f32, p.border_hairline),
                    );
                    ui.add_space(12.0);

                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        ui.add_space(32.0);
                        if let Some(act) = render_markdown_preview(
                            ui,
                            p,
                            lang,
                            today,
                            doc.body,
                            prefs.font_size,
                            prefs.heading_serif,
                            resolved_titles,
                            copied_code_hash,
                            vault_dir,
                            image_cache,
                            embedded_boards,
                            available_boards,
                            embedded_notes,
                            Some(rel_path),
                        ) {
                            handle_preview_action(act, &mut actions);
                        }
                    });
                });

                if prefs.show_footer {
                    ui.add_space(28.0);
                    render_editor_footer(ui, p, lang, layout, rel_path, doc.body, effective_mode);
                }
            });
        return actions;
    }

    // Centered single-column page (configured reading measure)
    egui::ScrollArea::vertical()
        .id_salt("editor_main_scroll")
        .drag_to_scroll(false)
        .show(ui, |ui| {
            let avail = ui.available_width();
            let edge = if layout.is_mobile() { 4.0 } else { 24.0 };
            let max_w = prefs.max_width.min((avail - edge).max(260.0));
            let side_pad = ((avail - max_w) * 0.5).max(edge * 0.5);

            ui.horizontal(|ui| {
                ui.add_space(side_pad);
                ui.vertical(|ui| {
                    ui.set_width(max_w);
                    ui.set_max_width(max_w);
                    ui.add_space((if layout.is_mobile() { 8.0 } else { 14.0 }) + (1.0 - entrance_t) * 6.0);

                    if doc.external_conflict {
                        render_external_conflict_banner(ui, p, lang, &mut actions);
                        ui.add_space(10.0);
                    }

                    if crate::app::english::is_english_lesson_path(rel_path) {
                        render_english_lesson_nav_bar(ui, p, lang, rel_path, all_notes, &mut actions);
                        ui.add_space(10.0);
                    }

                    render_document_header(
                        ui,
                        p,
                        lang,
                        layout,
                        today,
                        is_today_daily,
                        doc.title,
                        doc.status,
                        doc.tags,
                        doc.pinned,
                        doc.extra_lines,
                        doc.modified,
                        new_tag_input,
                        adding_tag,
                        prefs.heading_serif,
                        focus_title,
                        focus_body,
                        &mut actions,
                    );

                    ui.add_space(16.0);
                    match effective_mode {
                        EditorMode::Edit => {
                            let ed_id = egui::Id::new("krono_md_editor_main");
                            let fb = *focus_body;
                            *focus_body = false;
                            let (changed, extra) = render_markdown_editor(
                                ui,
                                p,
                                lang,
                                today,
                                doc.body,
                                ed_id,
                                prefs,
                                custom_snippets,
                                all_notes,
                                resolved_titles,
                                fb,
                                spellchecker,
                            );
                            if changed {
                                actions.push(UiAction::MarkDirty);
                            }
                            if let Some(act) = extra {
                                actions.push(act);
                            }
                            let board_refs = extract_embedded_boards(doc.body);
                            if !board_refs.is_empty() {
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new(s.note_linked_boards_title)
                                        .font(font_medium(12.0))
                                        .color(p.text_muted),
                                );
                                ui.add_space(6.0);
                                for (b_idx, target) in board_refs.iter().enumerate() {
                                    if let Some((b_path, b_doc)) =
                                        resolve_embedded_board(target, embedded_boards)
                                    {
                                        let mut b_acts = Vec::new();
                                        let salt = format!("ed_{b_idx}");
                                        render_embedded_board_widget(
                                            ui, p, lang, b_path, b_doc, &salt, &mut b_acts,
                                        );
                                        for ba in b_acts {
                                            actions.push(UiAction::EmbeddedBoard(ba));
                                        }
                                        ui.add_space(8.0);
                                    }
                                }
                            }
                        }
                        EditorMode::Read => {
                            if let Some(act) = render_markdown_preview(
                                ui,
                                p,
                                lang,
                                today,
                                doc.body,
                                prefs.font_size,
                                prefs.heading_serif,
                                resolved_titles,
                                copied_code_hash,
                                vault_dir,
                                image_cache,
                                embedded_boards,
                                available_boards,
                                embedded_notes,
                                Some(rel_path),
                            ) {
                                handle_preview_action(act, &mut actions);
                            }
                        }
                        EditorMode::Split => {}
                    }

                    if prefs.show_footer {
                        ui.add_space(if layout.is_mobile() { 28.0 } else { 40.0 });
                        render_editor_footer(ui, p, lang, layout, rel_path, doc.body, effective_mode);
                    }
                    ui.add_space(24.0);
                });
            });
        });

    let _ = s;
    actions
}

fn render_english_lesson_nav_bar(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    rel_path: &str,
    all_notes: &[NoteMeta],
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let mut lessons: Vec<&str> = all_notes
        .iter()
        .filter(|n| crate::app::english::is_english_lesson_path(&n.path))
        .map(|n| n.path.as_str())
        .collect();
    lessons.sort();
    let cur_idx = lessons.iter().position(|p| *p == rel_path);
    let prev_path = cur_idx
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| lessons.get(i))
        .map(|s| (*s).to_string());
    let next_path = cur_idx
        .and_then(|i| lessons.get(i + 1))
        .map(|s| (*s).to_string());

    egui::Frame::none()
        .fill(p.bg_sidebar)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(10.0, 6.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                ui.add_enabled_ui(prev_path.is_some(), |ui| {
                    if secondary_button(ui, p, None, s.eng_prev_lesson).clicked() {
                        if let Some(prev) = prev_path.clone() {
                            actions.push(UiAction::OpenNote(prev));
                        }
                    }
                });
                if secondary_button(ui, p, Some(Icon::Canvas), s.eng_map_btn).clicked() {
                    actions.push(UiAction::OpenSidebarModule(SidebarModule::English));
                }
                if secondary_button(ui, p, Some(Icon::Table), s.eng_vocab_btn).clicked() {
                    actions.push(UiAction::OpenEnglishVocabulary);
                }
                ui.add_enabled_ui(next_path.is_some(), |ui| {
                    if secondary_button(ui, p, None, s.eng_next_lesson).clicked() {
                        if let Some(next) = next_path.clone() {
                            actions.push(UiAction::OpenNote(next));
                        }
                    }
                });
                if primary_button(ui, p, Some(Icon::Plus), s.eng_new_lesson_btn, false).clicked() {
                    actions.push(UiAction::CreateNextEnglishLesson { open_note: true });
                }
            });
        });
}

fn render_external_conflict_banner(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    egui::Frame::none()
        .fill(p.warning_soft)
        .stroke(Stroke::new(1.0_f32, p.warning))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 9.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(15.0, 16.0), Sense::hover());
                draw_icon(ui.painter(), ir, Icon::Alert, p.warning);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(s.ext_conflict_msg)
                            .font(font_medium(12.5))
                            .color(p.text_primary),
                    )
                    .wrap(),
                );
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                if secondary_button(ui, p, Some(Icon::RotateCcw), s.ext_conflict_reload).clicked() {
                    actions.push(UiAction::ExternalReloadFromDisk);
                }
                if secondary_button(ui, p, None, s.ext_conflict_keep).clicked() {
                    actions.push(UiAction::ExternalKeepMine);
                }
                if primary_button(ui, p, Some(Icon::Copy), s.ext_conflict_copy, false).clicked() {
                    actions.push(UiAction::ExternalSaveConflictCopy);
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn render_document_header(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    is_today_daily: bool,
    note_title: &mut String,
    note_status: &mut Option<NoteStatus>,
    note_tags: &mut Vec<String>,
    note_pinned: &mut bool,
    note_extra_lines: &mut Vec<(String, String)>,
    note_modified: i64,
    new_tag_input: &mut String,
    adding_tag: &mut bool,
    heading_serif: bool,
    focus_title: &mut bool,
    focus_body: &mut bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let title_font = if heading_serif {
        font_serif_bold(if layout.is_mobile() { 24.0 } else { 30.0 })
    } else {
        font_bold(if layout.is_mobile() { 23.0 } else { 29.0 })
    };

    let title_resp = ui.add(
        egui::TextEdit::singleline(note_title)
            .id(egui::Id::new("krono_doc_title_input"))
            .frame(false)
            .font(title_font)
            .text_color(p.text_primary)
            .hint_text(s.untitled_hint)
            .desired_width(f32::INFINITY),
    );
    if *focus_title {
        title_resp.request_focus();
        *focus_title = false;
    }
    let enter_on_title = title_resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if enter_on_title {
        *focus_body = true;
    }
    if title_resp.changed() {
        actions.push(UiAction::MarkDirty);
    }
    if enter_on_title || title_resp.lost_focus() {
        actions.push(UiAction::CommitCurrentNoteTitle);
    }

    ui.add_space(10.0);

    // Compact Properties Row (separated from prose by breathing room, not a heavy divider)
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);

        if let Some(new_status) = optional_status_pill_picker(
            ui,
            p,
            lang,
            "editor_status_pill",
            *note_status,
        ) {
            if new_status != *note_status {
                *note_status = new_status;
                actions.push(UiAction::MarkDirty);
                actions.push(UiAction::SaveCurrentNote);
            }
        }

        if icon_button(ui, p, Icon::Pin, s.tooltip_pin, *note_pinned).clicked() {
            *note_pinned = !*note_pinned;
            actions.push(UiAction::MarkDirty);
            actions.push(UiAction::SaveCurrentNote);
        }

        if let Some(date_idx) = note_extra_lines
            .iter()
            .position(|(k, _)| k.trim().eq_ignore_ascii_case("date"))
        {
            let date_val = note_extra_lines[date_idx].1.clone();
            let g = ui.painter().layout_no_wrap(
                date_val,
                font_mono(11.5),
                p.text_secondary,
            );
            let (rect, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 34.0, 22.0), Sense::click());
            ui.painter()
                .rect_filled(rect, Rounding::same(11.0), p.bg_elevated);
            ui.painter().rect_stroke(
                rect,
                Rounding::same(11.0),
                Stroke::new(1.0_f32, p.border_hairline),
            );
            draw_icon(
                ui.painter(),
                Rect::from_center_size(pos2(rect.left() + 11.0, rect.center().y), vec2(11.0, 11.0)),
                Icon::CalendarDays,
                p.text_muted,
            );
            ui.painter().galley(
                pos2(rect.left() + 20.0, rect.center().y - g.size().y * 0.5),
                g,
                p.text_secondary,
            );
            let del_r = Rect::from_center_size(
                pos2(rect.right() - 10.0, rect.center().y),
                vec2(10.0, 10.0),
            );
            let del_hov = resp
                .hover_pos()
                .is_some_and(|pt| del_r.expand(4.0).contains(pt));
            draw_icon(
                ui.painter(),
                del_r,
                Icon::Close,
                if del_hov { p.danger } else { p.text_muted },
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && del_hov {
                note_extra_lines.remove(date_idx);
                actions.push(UiAction::MarkDirty);
                actions.push(UiAction::SaveCurrentNote);
            }
        } else {
            let g = ui.painter().layout_no_wrap(
                s.insert_date_btn.to_string(),
                font_medium(11.5),
                p.text_muted,
            );
            let (rect, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 12.0, 21.0), Sense::click());
            if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, Rounding::same(5.0), p.bg_hover);
            } else {
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(5.0),
                    Stroke::new(1.0_f32, p.border_hairline),
                );
            }
            ui.painter().galley(
                pos2(
                    rect.center().x - g.size().x * 0.5,
                    rect.center().y - g.size().y * 0.5,
                ),
                g,
                if resp.hovered() {
                    p.text_primary
                } else {
                    p.text_muted
                },
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                note_extra_lines.push(("date".to_string(), today.format_iso()));
                actions.push(UiAction::MarkDirty);
                actions.push(UiAction::SaveCurrentNote);
            }
        }

        if is_today_daily {
            if secondary_button(ui, p, Some(Icon::CalendarDays), s.tasks_carry_over_btn).clicked() {
                actions.push(UiAction::CarryOverDailyTasks);
            }
        }

        let mut remove_idx: Option<usize> = None;
        for (idx, tag) in note_tags.iter().enumerate() {
            let (resp, rm) = tag_chip(ui, p, tag, true);
            if rm {
                remove_idx = Some(idx);
            } else if resp.clicked() {
                actions.push(UiAction::SetTagFilter(Some(tag.clone())));
            }
        }
        if let Some(idx) = remove_idx {
            note_tags.remove(idx);
            actions.push(UiAction::MarkDirty);
        }

        let tag_gate_id = egui::Id::new("editor_add_tag_foc_gate");
        if *adding_tag {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.accent))
                .rounding(Rounding::same(5.0))
                .inner_margin(Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(new_tag_input)
                            .hint_text(s.add_tag_hint)
                            .frame(false)
                            .font(font_body(11.5))
                            .desired_width(88.0),
                    );
                    focus_once(ui.ctx(), tag_gate_id, true, resp.id);
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                    if enter {
                        let clean = new_tag_input.trim().trim_start_matches('#').trim();
                        if !clean.is_empty()
                            && !note_tags.iter().any(|t| t.eq_ignore_ascii_case(clean))
                        {
                            note_tags.push(clean.to_string());
                            actions.push(UiAction::MarkDirty);
                        }
                        new_tag_input.clear();
                        *adding_tag = false;
                    } else if esc || resp.lost_focus() {
                        new_tag_input.clear();
                        *adding_tag = false;
                    }
                });
        } else {
            reset_focus_gate(ui.ctx(), tag_gate_id);
            let g = ui.painter().layout_no_wrap(
                s.add_tag_btn.to_string(),
                font_medium(11.5),
                p.text_muted,
            );
            let (rect, resp) =
                ui.allocate_exact_size(vec2(g.size().x + 12.0, 21.0), Sense::click());
            if resp.hovered() {
                ui.painter()
                    .rect_filled(rect, Rounding::same(5.0), p.bg_hover);
            } else {
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(5.0),
                    Stroke::new(1.0_f32, p.border_hairline),
                );
            }
            ui.painter().galley(
                pos2(
                    rect.center().x - g.size().x * 0.5,
                    rect.center().y - g.size().y * 0.5,
                ),
                g,
                if resp.hovered() {
                    p.text_primary
                } else {
                    p.text_muted
                },
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                *adding_tag = true;
            }
        }

        let bg_g = ui.painter().layout_no_wrap(
            s.note_add_board_btn.to_string(),
            font_medium(11.5),
            p.accent_text,
        );
        let (brect, bresp) =
            ui.allocate_exact_size(vec2(bg_g.size().x + 14.0, 21.0), Sense::click());
        ui.painter().rect_filled(
            brect,
            Rounding::same(5.0),
            if bresp.hovered() {
                p.bg_hover
            } else {
                p.accent_soft
            },
        );
        ui.painter().galley(
            pos2(
                brect.center().x - bg_g.size().x * 0.5,
                brect.center().y - bg_g.size().y * 0.5,
            ),
            bg_g,
            p.accent_text,
        );
        if bresp
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(s.note_add_board_tooltip)
            .clicked()
        {
            actions.push(UiAction::CreateLinkedBoardForCurrentNote);
        }

        if !layout.is_mobile() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{}: {}",
                        s.prop_modified,
                        format_relative_time(lang, note_modified)
                    ))
                    .font(font_mono(11.0))
                    .color(p.text_muted),
                );
            });
        }
    });
    if layout.is_mobile() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{}: {}",
                s.prop_modified,
                format_relative_time(lang, note_modified)
            ))
            .font(font_mono(11.0))
            .color(p.text_muted),
        );
    }
}

fn plural_words_label(lang: Lang, count: usize) -> &'static str {
    match lang {
        Lang::En => {
            if count == 1 {
                "word"
            } else {
                "words"
            }
        }
        Lang::Ru => {
            let m100 = count % 100;
            let m10 = count % 10;
            if (11..=14).contains(&m100) {
                "слов"
            } else if m10 == 1 {
                "слово"
            } else if (2..=4).contains(&m10) {
                "слова"
            } else {
                "слов"
            }
        }
    }
}

fn render_editor_footer(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    rel_path: &str,
    body: &str,
    editor_mode: EditorMode,
) {
    let s = strings(lang);
    let (words, _) = count_words(body);
    let mins = ((words as f32) / 200.0).ceil().max(1.0) as usize;
    let summary = if layout.is_mobile() {
        format!("{words} {} · ~{mins} {}", plural_words_label(lang, words), s.min_unit)
    } else {
        format!(
            "{words} {} · ~{mins} {} · {rel_path}",
            plural_words_label(lang, words),
            s.min_unit
        )
    };
    if layout.is_mobile() {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(summary)
                    .font(font_mono(11.0))
                    .color(p.text_muted),
            );
            if editor_mode == EditorMode::Read {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(s.read_mode_hint)
                        .font(font_body(11.0))
                        .color(p.text_muted),
                );
            }
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(summary)
                    .font(font_mono(11.0))
                    .color(p.text_muted),
            );
            if editor_mode == EditorMode::Read {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(s.read_mode_hint)
                            .font(font_body(11.5))
                            .color(p.text_muted),
                    );
                });
            }
        });
    }
}

fn render_empty_state(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    heading_serif: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let avail = ui.available_size();
    ui.allocate_ui_with_layout(
        avail,
        egui::Layout::centered_and_justified(egui::Direction::TopDown),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((avail.y * 0.25).max(20.0));
                let (lrect, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                draw_icon(
                    ui.painter(),
                    Rect::from_center_size(lrect.center(), vec2(38.0, 38.0)),
                    Icon::Logo,
                    p.text_secondary,
                );
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(s.empty_title)
                        .font(if heading_serif {
                            font_serif_bold(20.0)
                        } else {
                            font_bold(20.0)
                        })
                        .color(p.text_primary),
                );
                ui.add_space(6.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(s.empty_subtitle)
                            .font(font_body(13.5))
                            .color(p.text_secondary),
                    )
                    .wrap(),
                );
                ui.add_space(16.0);
                if primary_button(ui, p, Some(Icon::Plus), s.new_note, false).clicked() {
                    actions.push(UiAction::CreateNote {
                        title: if lang == Lang::Ru {
                            "Новая заметка".into()
                        } else {
                            "New Note".into()
                        },
                        status: None,
                        tags: Vec::new(),
                    });
                }
            });
        },
    );
}

fn handle_preview_action(act: PreviewAction, actions: &mut Vec<UiAction>) {
    match act {
        PreviewAction::NavigateWikilink(target) => {
            actions.push(UiAction::OpenOrCreateWikilink(target));
        }
        PreviewAction::ToggleTask(idx) => {
            actions.push(UiAction::ToggleTask(idx));
        }
        PreviewAction::CopyCode(code) => {
            actions.push(UiAction::CopyText(code));
        }
        PreviewAction::OpenImage(path) => {
            actions.push(UiAction::OpenImageLightbox(path));
        }
        PreviewAction::EmbeddedBoard(eb) => {
            actions.push(UiAction::EmbeddedBoard(eb));
        }
        PreviewAction::SendTaskToBoard {
            task_idx,
            board_path,
            move_task,
        } => {
            actions.push(UiAction::SendTaskToBoard {
                task_idx,
                board_path,
                move_task,
            });
        }
    }
}
