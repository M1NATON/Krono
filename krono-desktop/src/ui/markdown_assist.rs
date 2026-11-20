use crate::config::CustomSlashSnippet;
use crate::i18n::Lang;
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_medium, font_mono, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Margin, Pos2, Rect, Rounding, Sense, Stroke,
};
use krono_core::note::NoteMeta;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssistTrigger {
    Slash {
        start_char: usize,
        query: String,
    },
    WikiLink {
        start_char: usize,
        query: String,
        has_closing_brackets: bool,
    },
}

#[derive(Clone, Debug)]
pub struct SlashCommand {
    #[allow(dead_code)]
    pub id: String,
    pub trigger: String,
    pub label: String,
    pub hint: String,
    pub keywords: Vec<String>,
    pub insert_text: String,
    pub cursor_offset_chars: usize,
    pub icon: Icon,
    pub is_custom: bool,
}

#[derive(Clone, Debug)]
pub enum AssistPopupAction {
    AcceptSlash(SlashCommand),
    AcceptWikiLink { target_title: String },
    Dismiss,
}

pub fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub fn detect_assist_trigger(
    body: &str,
    cursor_char: usize,
    trigger_ch: char,
    enable_slash: bool,
    enable_wikilink: bool,
) -> Option<AssistTrigger> {
    let cursor_byte = char_to_byte(body, cursor_char);
    let before = &body[..cursor_byte];
    let line_start_byte = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_before = &before[line_start_byte..];
    let line_start_char = body[..line_start_byte].chars().count();

    if enable_wikilink {
        if let Some(open_rel) = line_before.rfind("[[") {
            let after_open = &line_before[open_rel + 2..];
            if !after_open.contains("]]") && !after_open.contains('\n') {
                let start_char = line_start_char + line_before[..open_rel].chars().count();
                let has_closing_brackets = body[cursor_byte..].starts_with("]]");
                return Some(AssistTrigger::WikiLink {
                    start_char,
                    query: after_open.to_string(),
                    has_closing_brackets,
                });
            }
        }
    }

    if enable_slash {
        if let Some(trig_rel) = line_before.rfind(trigger_ch) {
            let prefix_ok = trig_rel == 0
                || line_before[..trig_rel]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace);
            let after_trig = &line_before[trig_rel + trigger_ch.len_utf8()..];
            let suffix_ok =
                !after_trig.chars().any(char::is_whitespace) && after_trig.chars().count() <= 28;
            if prefix_ok && suffix_ok {
                let start_char = line_start_char + line_before[..trig_rel].chars().count();
                return Some(AssistTrigger::Slash {
                    start_char,
                    query: after_trig.to_string(),
                });
            }
        }
    }

    None
}

pub fn builtin_slash_commands(lang: Lang, trigger_ch: char) -> Vec<SlashCommand> {
    let table_ins = match lang {
        Lang::Ru => "| Колонка 1 | Колонка 2 |\n| --- | --- |\n|  |  |",
        Lang::En => "| Column 1 | Column 2 |\n| --- | --- |\n|  |  |",
    };
    let ext_link = match lang {
        Lang::Ru => "[текст](https://)",
        Lang::En => "[label](https://)",
    };

    let items: Vec<(&str, &str, &str, &str, &[&str], &str, usize, Icon)> = vec![
        (
            "h1",
            "h1",
            match lang {
                Lang::Ru => "Заголовок H1",
                Lang::En => "Heading 1",
            },
            "# ",
            &["h1", "heading", "title", "заг", "заголовок", "1"],
            "# ",
            2,
            Icon::Heading1,
        ),
        (
            "h2",
            "h2",
            match lang {
                Lang::Ru => "Подзаголовок H2",
                Lang::En => "Heading 2",
            },
            "## ",
            &["h2", "heading2", "section", "заг2", "раздел", "подзаг", "2"],
            "## ",
            3,
            Icon::Heading2,
        ),
        (
            "h3",
            "h3",
            match lang {
                Lang::Ru => "Заголовок H3",
                Lang::En => "Heading 3",
            },
            "### ",
            &["h3", "heading3", "sub", "заг3", "3"],
            "### ",
            4,
            Icon::Heading3,
        ),
        (
            "todo",
            "todo",
            match lang {
                Lang::Ru => "Чек-лист",
                Lang::En => "To-do list",
            },
            "- [ ] ",
            &["todo", "task", "check", "чек", "задач", "[]"],
            "- [ ] ",
            6,
            Icon::Task,
        ),
        (
            "list",
            "list",
            match lang {
                Lang::Ru => "Маркированный список",
                Lang::En => "Bulleted list",
            },
            "- ",
            &["list", "bullet", "ul", "список", "марк", "пункт"],
            "- ",
            2,
            Icon::List,
        ),
        (
            "num",
            "num",
            match lang {
                Lang::Ru => "Нумерованный список",
                Lang::En => "Numbered list",
            },
            "1. ",
            &["num", "ol", "ordered", "1.", "нум", "поряд"],
            "1. ",
            3,
            Icon::ListOrdered,
        ),
        (
            "quote",
            "quote",
            match lang {
                Lang::Ru => "Цитата",
                Lang::En => "Quote",
            },
            "> ",
            &["quote", "blockquote", "цит", "цитата"],
            "> ",
            2,
            Icon::Quote,
        ),
        (
            "code",
            "code",
            match lang {
                Lang::Ru => "Блок кода",
                Lang::En => "Code block",
            },
            "```",
            &["code", "fence", "snippet", "код", "блок"],
            "```\n\n```",
            4,
            Icon::FileCode,
        ),
        (
            "table",
            "table",
            match lang {
                Lang::Ru => "Таблица Markdown",
                Lang::En => "Table",
            },
            "| | |",
            &["table", "таб", "таблица"],
            table_ins,
            2,
            Icon::Table,
        ),
        (
            "divider",
            "hr",
            match lang {
                Lang::Ru => "Разделитель",
                Lang::En => "Divider",
            },
            "---",
            &["hr", "divider", "line", "разд", "линия", "---"],
            "---\n",
            4,
            Icon::Minus,
        ),
        (
            "wikilink",
            "link",
            match lang {
                Lang::Ru => "Ссылка на заметку",
                Lang::En => "Wikilink [[…]]",
            },
            "[[]]",
            &["link", "wiki", "ссыл", "ссылка", "замет"],
            "[[]]",
            2,
            Icon::Link,
        ),
        (
            "extlink",
            "url",
            match lang {
                Lang::Ru => "Внешняя ссылка",
                Lang::En => "External link",
            },
            "[](url)",
            &["url", "http", "web", "внеш", "адрес"],
            ext_link,
            1,
            Icon::Globe,
        ),
        (
            "image",
            "image",
            match lang {
                Lang::Ru => "Вставить картинку",
                Lang::En => "Insert image",
            },
            "![](assets/…)",
            &["image", "img", "pic", "photo", "карт", "фото", "изобр"],
            "",
            0,
            Icon::Image,
        ),
        (
            "kanban",
            "kanban",
            match lang {
                Lang::Ru => "Доска задач (Kanban)",
                Lang::En => "Task Board (Kanban)",
            },
            "![[…board.json]]",
            &["kanban", "board", "task", "доск", "канбан", "задач"],
            "",
            0,
            Icon::Board,
        ),
    ];

    let _ = trigger_ch;
    items
        .into_iter()
        .map(
            |(id, trigger, label, hint, kws, insert_text, offset, icon)| SlashCommand {
                id: id.to_string(),
                trigger: trigger.to_string(),
                label: label.to_string(),
                hint: hint.to_string(),
                keywords: kws.iter().map(|s| (*s).to_string()).collect(),
                insert_text: insert_text.to_string(),
                cursor_offset_chars: offset,
                icon,
                is_custom: false,
            },
        )
        .collect()
}

pub fn filter_slash_commands(
    lang: Lang,
    trigger_ch: char,
    custom_snippets: &[CustomSlashSnippet],
    query: &str,
) -> Vec<SlashCommand> {
    let mut all = builtin_slash_commands(lang, trigger_ch);
    for (idx, snip) in custom_snippets.iter().enumerate() {
        let trig = snip.trigger.trim().trim_start_matches('/').trim_start_matches('\\');
        if trig.is_empty() || snip.content.is_empty() {
            continue;
        }
        let lbl = if snip.label.trim().is_empty() {
            trig.to_string()
        } else {
            snip.label.trim().to_string()
        };
        let char_len = snip.content.chars().count();
        all.push(SlashCommand {
            id: format!("custom_{idx}"),
            trigger: trig.to_string(),
            label: lbl.clone(),
            hint: format!("{trigger_ch}{trig}"),
            keywords: vec![trig.to_lowercase(), lbl.to_lowercase()],
            insert_text: snip.content.clone(),
            cursor_offset_chars: char_len,
            icon: Icon::Sparkles,
            is_custom: true,
        });
    }

    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return all;
    }

    all.into_iter()
        .filter(|cmd| {
            cmd.trigger.to_lowercase().contains(&q)
                || cmd.label.to_lowercase().contains(&q)
                || cmd.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        })
        .collect()
}

pub fn filter_wikilink_candidates<'a>(notes: &'a [NoteMeta], query: &str) -> Vec<&'a NoteMeta> {
    let q = query.trim().to_lowercase();
    let mut out: Vec<&NoteMeta> = notes
        .iter()
        .filter(|n| {
            q.is_empty()
                || n.title.to_lowercase().contains(&q)
                || n.path.to_lowercase().contains(&q)
        })
        .collect();
    out.truncate(8);
    out
}

pub fn apply_slash_replacement(
    body: &mut String,
    start_char: usize,
    cursor_char: usize,
    insert_text: &str,
    cursor_offset_chars: usize,
) -> usize {
    let sb = char_to_byte(body, start_char);
    let eb = char_to_byte(body, cursor_char.max(start_char));
    body.replace_range(sb..eb, insert_text);
    start_char + cursor_offset_chars.min(insert_text.chars().count())
}

pub fn apply_wikilink_replacement(
    body: &mut String,
    start_char: usize,
    cursor_char: usize,
    has_closing_brackets: bool,
    target_title: &str,
) -> usize {
    let sb = char_to_byte(body, start_char);
    let end_char = cursor_char.max(start_char) + if has_closing_brackets { 2 } else { 0 };
    let eb = char_to_byte(body, end_char);
    let replacement = format!("[[{target_title}]]");
    let len_chars = replacement.chars().count();
    body.replace_range(sb..eb, &replacement);
    start_char + len_chars
}

/// Wraps a selected range with `(prefix, suffix)` or inserts at `sel_start` when collapsed.
pub fn apply_wrap_or_insert(
    body: &mut String,
    sel_start: usize,
    sel_end: usize,
    prefix: &str,
    suffix: &str,
    line_prefix: bool,
) -> (usize, usize) {
    let (a, b) = if sel_start <= sel_end {
        (sel_start, sel_end)
    } else {
        (sel_end, sel_start)
    };
    let sb = char_to_byte(body, a);
    let eb = char_to_byte(body, b);

    if a < b {
        let selected = body[sb..eb].to_string();
        let replacement = format!("{prefix}{selected}{suffix}");
        body.replace_range(sb..eb, &replacement);
        let p_chars = prefix.chars().count();
        let s_chars = selected.chars().count();
        return (a + p_chars, a + p_chars + s_chars);
    }

    let mut actual_prefix = prefix.to_string();
    let mut extra_nl = 0usize;
    if line_prefix && sb > 0 && !body[..sb].ends_with('\n') {
        actual_prefix.insert(0, '\n');
        extra_nl = 1;
    }
    let replacement = format!("{actual_prefix}{suffix}");
    body.insert_str(sb, &replacement);
    let new_caret = a + extra_nl + prefix.chars().count();
    (new_caret, new_caret)
}

/// Continues `- `, `* `, `- [ ] `, and `N. ` lists on Enter; clears empty list markers on second Enter.
pub fn handle_auto_list_on_enter(
    prev_body: &str,
    new_body: &mut String,
    cursor_char: usize,
) -> Option<usize> {
    if cursor_char == 0 {
        return None;
    }
    let prev_chars = prev_body.chars().count();
    let new_chars = new_body.chars().count();
    if new_chars != prev_chars + 1 {
        return None;
    }

    let nl_byte = char_to_byte(new_body, cursor_char - 1);
    if !new_body[nl_byte..].starts_with('\n') {
        return None;
    }
    let after_byte = nl_byte + 1;
    if &new_body[..nl_byte] != &prev_body[..char_to_byte(prev_body, cursor_char - 1)]
        || &new_body[after_byte..] != &prev_body[char_to_byte(prev_body, cursor_char - 1)..]
    {
        return None;
    }

    let before_nl = &new_body[..nl_byte];
    let line_start_byte = before_nl.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let prev_line = &before_nl[line_start_byte..];
    let indent_len = prev_line.len() - prev_line.trim_start().len();
    let indent = &prev_line[..indent_len];
    let trimmed = &prev_line[indent_len..];

    let (continuation, content) = if let Some(rest) = trimmed
        .strip_prefix("- [ ] ")
        .or_else(|| trimmed.strip_prefix("- [x] "))
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        (format!("{indent}- [ ] "), rest)
    } else if let Some(rest) = trimmed.strip_prefix("- ") {
        (format!("{indent}- "), rest)
    } else if let Some(rest) = trimmed.strip_prefix("* ") {
        (format!("{indent}* "), rest)
    } else if let Some((num_str, rest)) = trimmed.split_once(". ") {
        if let Ok(n) = num_str.parse::<usize>() {
            (format!("{indent}{}. ", n + 1), rest)
        } else {
            return None;
        }
    } else {
        return None;
    };

    if content.trim().is_empty() {
        // Exit list by replacing the empty marker + newly inserted newline cleanly
        let line_start_char = new_body[..line_start_byte].chars().count();
        new_body.replace_range(line_start_byte..after_byte, "");
        return Some(line_start_char);
    }

    new_body.insert_str(after_byte, &continuation);
    Some(cursor_char + continuation.chars().count())
}

/// Auto-closes single typed `` ` ``, `[`, and `*` safely without breaking fenced code blocks.
pub fn handle_auto_pair(
    prev_body: &str,
    new_body: &mut String,
    cursor_char: usize,
) -> Option<usize> {
    if cursor_char == 0 {
        return None;
    }
    let prev_chars = prev_body.chars().count();
    let new_chars = new_body.chars().count();
    if new_chars != prev_chars + 1 {
        return None;
    }

    let ch_byte = char_to_byte(new_body, cursor_char - 1);
    let typed = new_body[ch_byte..].chars().next()?;
    let next_byte = ch_byte + typed.len_utf8();

    if &new_body[..ch_byte] != &prev_body[..char_to_byte(prev_body, cursor_char - 1)]
        || &new_body[next_byte..] != &prev_body[char_to_byte(prev_body, cursor_char - 1)..]
    {
        return None;
    }

    // Never auto-pair inside fenced code blocks
    let mut in_fence = false;
    for l in new_body[..ch_byte].lines() {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
        }
    }
    if in_fence {
        return None;
    }

    let prev_ch = new_body[..ch_byte].chars().next_back();
    let next_ch = new_body[next_byte..].chars().next();
    let at_word_end = next_ch.is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ']' | '.' | ',' | ':' | ';'));

    match typed {
        '[' => {
            if prev_ch == Some('[') && next_ch == Some(']') {
                new_body.insert(next_byte, ']');
                return Some(cursor_char);
            }
            if at_word_end {
                new_body.insert(next_byte, ']');
                return Some(cursor_char);
            }
            None
        }
        '`' => {
            // Do not auto-close ``` fenced blocks
            if prev_ch == Some('`') || next_ch == Some('`') {
                return None;
            }
            if at_word_end {
                new_body.insert(next_byte, '`');
                return Some(cursor_char);
            }
            None
        }
        '*' => {
            if prev_ch == Some('*') && next_ch == Some('*') {
                new_body.insert(next_byte, '*');
                return Some(cursor_char);
            }
            if at_word_end && prev_ch.is_none_or(|c| c.is_whitespace() || c == '(') {
                new_body.insert(next_byte, '*');
                return Some(cursor_char);
            }
            None
        }
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_assist_popup(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    anchor_pos: Pos2,
    line_height: f32,
    trigger: &AssistTrigger,
    slash_items: &[SlashCommand],
    note_items: &[&NoteMeta],
    selected_idx: &mut usize,
    enter_pressed: bool,
    tab_pressed: bool,
    esc_pressed: bool,
    up_pressed: bool,
    down_pressed: bool,
) -> Option<AssistPopupAction> {
    if esc_pressed {
        return Some(AssistPopupAction::Dismiss);
    }

    let count = match trigger {
        AssistTrigger::Slash { .. } => slash_items.len(),
        AssistTrigger::WikiLink { .. } => note_items.len(),
    };
    if count == 0 {
        return None;
    }

    if *selected_idx >= count {
        *selected_idx = 0;
    }
    if down_pressed {
        *selected_idx = (*selected_idx + 1) % count;
    }
    if up_pressed {
        *selected_idx = (*selected_idx + count - 1) % count;
    }

    let screen = ctx.screen_rect();
    let popup_w = 290.0_f32.min((screen.width() - 24.0).max(220.0));
    let visible_rows = count.min(7);
    let est_h = (visible_rows as f32) * 34.0 + 30.0;

    let mut pos_x = anchor_pos.x.clamp(screen.left() + 12.0, (screen.right() - popup_w - 12.0).max(screen.left() + 12.0));
    let below_y = anchor_pos.y + 4.0;
    let mut pos_y = if below_y + est_h <= screen.bottom() - 16.0 {
        below_y
    } else {
        (anchor_pos.y - line_height - est_h - 6.0).max(screen.top() + 12.0)
    };
    pos_x = pos_x.round();
    pos_y = pos_y.round();

    let mut result: Option<AssistPopupAction> = None;
    let accept_now = enter_pressed || tab_pressed;
    let kb_moved = up_pressed || down_pressed;
    let pointer_moved = ctx.input(|i| i.pointer.delta().length_sq() > 0.25);

    let header_title = match trigger {
        AssistTrigger::Slash { .. } => {
            if lang == Lang::Ru { "Вставить блок" } else { "Insert block" }
        }
        AssistTrigger::WikiLink { .. } => {
            if lang == Lang::Ru { "Связать с заметкой" } else { "Link to note" }
        }
    };

    egui::Area::new(egui::Id::new("krono_md_assist_popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(pos_x, pos_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(10.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(6.0))
                .show(ui, |ui| {
                    let inner_w = popup_w - 12.0;
                    ui.set_width(inner_w);
                    ui.set_max_width(inner_w);

                    ui.horizontal(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(header_title)
                                .font(font_medium(10.5))
                                .color(p.text_muted),
                        );
                    });
                    ui.add_space(4.0);

                    egui::ScrollArea::vertical()
                        .id_salt("md_assist_scroll")
                        .max_height(246.0)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 2.0;
                            match trigger {
                                AssistTrigger::Slash { .. } => {
                                    for (idx, cmd) in slash_items.iter().enumerate() {
                                        let is_sel = idx == *selected_idx;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            vec2(inner_w, 32.0),
                                            Sense::click(),
                                        );
                                        if kb_moved && is_sel {
                                            resp.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        paint_popup_row_bg(ctx, ui, p, rect, &resp, is_sel, idx);
                                        draw_icon(
                                            ui.painter(),
                                            Rect::from_center_size(
                                                pos2(rect.left() + 16.0, rect.center().y),
                                                vec2(14.0, 14.0),
                                            ),
                                            cmd.icon,
                                            if is_sel || cmd.is_custom {
                                                p.accent_text
                                            } else {
                                                p.text_secondary
                                            },
                                        );
                                        let hint_g = ui.painter().layout_no_wrap(
                                            cmd.hint.clone(),
                                            font_mono(10.5),
                                            p.text_muted,
                                        );
                                        let label_max_w = (inner_w - hint_g.size().x - 42.0).max(80.0);
                                        let mut job = egui::text::LayoutJob::single_section(
                                            cmd.label.clone(),
                                            egui::TextFormat {
                                                font_id: font_medium(12.5),
                                                color: p.text_primary,
                                                ..Default::default()
                                            },
                                        );
                                        job.wrap = egui::text::TextWrapping {
                                            max_width: label_max_w,
                                            max_rows: 1,
                                            break_anywhere: false,
                                            overflow_character: Some('…'),
                                        };
                                        let lg = ui.fonts(|f| f.layout_job(job));
                                        ui.painter().galley(
                                            pos2(rect.left() + 31.0, rect.center().y - lg.size().y * 0.5),
                                            lg,
                                            p.text_primary,
                                        );
                                        ui.painter().galley(
                                            pos2(rect.right() - 8.0 - hint_g.size().x, rect.center().y - hint_g.size().y * 0.5),
                                            hint_g,
                                            p.text_muted,
                                        );
                                        if resp.hovered() && pointer_moved && !kb_moved {
                                            *selected_idx = idx;
                                            resp.scroll_to_me(None);
                                        }
                                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                            || (accept_now && is_sel)
                                        {
                                            result = Some(AssistPopupAction::AcceptSlash(cmd.clone()));
                                        }
                                    }
                                }
                                AssistTrigger::WikiLink { .. } => {
                                    for (idx, note) in note_items.iter().enumerate() {
                                        let is_sel = idx == *selected_idx;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            vec2(inner_w, 32.0),
                                            Sense::click(),
                                        );
                                        if kb_moved && is_sel {
                                            resp.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        paint_popup_row_bg(ctx, ui, p, rect, &resp, is_sel, idx);
                                        draw_icon(
                                            ui.painter(),
                                            Rect::from_center_size(
                                                pos2(rect.left() + 16.0, rect.center().y),
                                                vec2(14.0, 14.0),
                                            ),
                                            Icon::Document,
                                            if is_sel { p.accent_text } else { p.text_secondary },
                                        );
                                        let mut pjob = egui::text::LayoutJob::single_section(
                                            note.path.clone(),
                                            egui::TextFormat {
                                                font_id: font_mono(10.0),
                                                color: p.text_muted,
                                                ..Default::default()
                                            },
                                        );
                                        pjob.wrap = egui::text::TextWrapping {
                                            max_width: (inner_w * 0.38).max(48.0),
                                            max_rows: 1,
                                            break_anywhere: false,
                                            overflow_character: Some('…'),
                                        };
                                        let path_g = ui.fonts(|f| f.layout_job(pjob));
                                        let title_max_w =
                                            (inner_w - path_g.size().x - 44.0).max(70.0);
                                        let mut job = egui::text::LayoutJob::single_section(
                                            note.title.clone(),
                                            egui::TextFormat {
                                                font_id: font_medium(12.5),
                                                color: p.text_primary,
                                                ..Default::default()
                                            },
                                        );
                                        job.wrap = egui::text::TextWrapping {
                                            max_width: title_max_w,
                                            max_rows: 1,
                                            break_anywhere: false,
                                            overflow_character: Some('…'),
                                        };
                                        let tg = ui.fonts(|f| f.layout_job(job));
                                        ui.painter().galley(
                                            pos2(rect.left() + 31.0, rect.center().y - tg.size().y * 0.5),
                                            tg,
                                            p.text_primary,
                                        );
                                        ui.painter().galley(
                                            pos2(
                                                rect.right() - 8.0 - path_g.size().x,
                                                rect.center().y - path_g.size().y * 0.5,
                                            ),
                                            path_g,
                                            p.text_muted,
                                        );
                                        if resp.hovered() && pointer_moved && !kb_moved {
                                            *selected_idx = idx;
                                            resp.scroll_to_me(None);
                                        }
                                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                            || (accept_now && is_sel)
                                        {
                                            result = Some(AssistPopupAction::AcceptWikiLink {
                                                target_title: note.title.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        });
                });
        });

    result
}

fn paint_popup_row_bg(
    ctx: &egui::Context,
    ui: &mut egui::Ui,
    p: &ThemePalette,
    rect: Rect,
    resp: &egui::Response,
    selected: bool,
    idx: usize,
) {
    let hov = anim_bool_eased(ctx, ui.make_persistent_id(("assist_row", idx)), resp.hovered(), 0.08);
    let bg = if selected {
        p.bg_selected
    } else if hov > 0.01 {
        with_alpha(p.bg_hover, hov)
    } else {
        Color32::TRANSPARENT
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
    }
    if selected {
        ui.painter().rect_filled(
            Rect::from_center_size(pos2(rect.left() + 3.0, rect.center().y), vec2(2.5, 16.0)),
            Rounding::same(1.5),
            p.accent,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_slash_and_wikilink_triggers_safely() {
        assert_eq!(
            detect_assist_trigger("Hello /заг", 10, '/', true, true),
            Some(AssistTrigger::Slash {
                start_char: 6,
                query: "заг".into(),
            })
        );
        // URL slashes must never trigger
        assert_eq!(
            detect_assist_trigger("See https://github.com/org", 22, '/', true, true),
            None
        );
        // Wikilink detection with auto-closed ]]
        let body = "Open [[Арх]] next";
        assert_eq!(
            detect_assist_trigger(body, 10, '/', true, true),
            Some(AssistTrigger::WikiLink {
                start_char: 5,
                query: "Арх".into(),
                has_closing_brackets: true,
            })
        );
        // Already closed [[Done]] after cursor shouldn't trigger
        assert_eq!(detect_assist_trigger("[[Done]] hi", 11, '/', true, true), None);
    }

    #[test]
    fn filters_and_applies_slash_and_wikilink_replacements() {
        let ru_hits = filter_slash_commands(Lang::Ru, '/', &[], "чек");
        assert!(ru_hits.iter().any(|c| c.id == "todo"));

        let custom = vec![CustomSlashSnippet {
            trigger: "видео".into(),
            label: "Сценарий".into(),
            content: "## Хук\n".into(),
        }];
        let custom_hits = filter_slash_commands(Lang::Ru, '/', &custom, "вид");
        assert_eq!(custom_hits.len(), 1);
        assert!(custom_hits[0].is_custom);

        let mut text = "Hi /todo".to_string();
        let caret = apply_slash_replacement(&mut text, 3, 8, "- [ ] ", 6);
        assert_eq!(text, "Hi - [ ] ");
        assert_eq!(caret, 9);

        let mut wtext = "Link [[Арх]]!".to_string();
        let wcaret = apply_wikilink_replacement(&mut wtext, 5, 10, true, "Архитектура");
        assert_eq!(wtext, "Link [[Архитектура]]!");
        assert_eq!(wcaret, 20);
    }

    #[test]
    fn wraps_selection_and_continues_lists_and_pairs() {
        let mut s = "Привет мир".to_string();
        let (sa, sb) = apply_wrap_or_insert(&mut s, 7, 10, "**", "**", false);
        assert_eq!(s, "Привет **мир**");
        assert_eq!((sa, sb), (9, 12));

        // Task list continuation & empty exit
        let prev = "- [x] Первый пункт";
        let mut entered = format!("{prev}\n");
        let entered_len = entered.chars().count();
        let next_pos = handle_auto_list_on_enter(prev, &mut entered, entered_len).unwrap();
        assert_eq!(entered, "- [x] Первый пункт\n- [ ] ");
        assert_eq!(next_pos, entered.chars().count());

        let prev_empty = "- [ ] ";
        let mut exited = format!("{prev_empty}\n");
        let exited_len = exited.chars().count();
        let exit_pos = handle_auto_list_on_enter(prev_empty, &mut exited, exited_len).unwrap();
        assert_eq!(exited, "");
        assert_eq!(exit_pos, 0);

        // Numbered list increments
        let prev_num = "2. Step two";
        let mut num_ed = format!("{prev_num}\n");
        let num_len = num_ed.chars().count();
        let _ = handle_auto_list_on_enter(prev_num, &mut num_ed, num_len).unwrap();
        assert_eq!(num_ed, "2. Step two\n3. ");

        // Auto-pair: [ -> [], then second [ -> [[]], backtick, code fence ignored
        let mut p1 = "See [".to_string();
        assert_eq!(handle_auto_pair("See ", &mut p1, 5), Some(5));
        assert_eq!(p1, "See []");
        let mut p2 = "See [[]".to_string();
        assert_eq!(handle_auto_pair("See []", &mut p2, 6), Some(6));
        assert_eq!(p2, "See [[]]");
    }
}
