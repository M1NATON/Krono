use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::collections::HashSet;

use crate::path::is_image_ext;

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Expands `![[note]]` transclusions recursively up to `max_depth`, skipping image embeds and cycles.
pub fn resolve_transclusions<F>(markdown: &str, max_depth: usize, mut resolve_note: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    let mut visited = HashSet::new();
    resolve_transclusions_inner(markdown, max_depth, &mut visited, &mut resolve_note)
}

fn resolve_transclusions_inner<F>(
    markdown: &str,
    depth_left: usize,
    visited: &mut HashSet<String>,
    resolve_note: &mut F,
) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find("![[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 3..];
        let Some(end) = after.find("]]") else {
            out.push_str("![[");
            rest = after;
            continue;
        };
        let inner = after[..end].trim();
        let target = inner.split('|').next().unwrap_or(inner).trim();
        let target_key = target.to_lowercase();

        if is_image_ext(target) {
            out.push_str(&format!("![]({target})"));
        } else if depth_left > 0 && !target_key.is_empty() && !visited.contains(&target_key) {
            if let Some(child_md) = resolve_note(target) {
                visited.insert(target_key.clone());
                let expanded =
                    resolve_transclusions_inner(&child_md, depth_left - 1, visited, resolve_note);
                visited.remove(&target_key);
                out.push_str(&expanded);
            } else {
                out.push_str(&format!("![[{inner}]]"));
            }
        } else {
            out.push_str(&format!("![[{inner}]]"));
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Converts a Markdown note to a self-contained HTML5 document with embedded CSS and base64 images.
pub fn export_note_to_html<F, I>(
    title: &str,
    body_md: &str,
    resolve_note: F,
    mut resolve_image: I,
) -> String
where
    F: FnMut(&str) -> Option<String>,
    I: FnMut(&str) -> Option<(String, Vec<u8>)>,
{
    let expanded = resolve_transclusions(body_md, 3, resolve_note);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(&expanded, opts);
    let mut html_body = String::new();
    let mut events: Vec<Event<'_>> = Vec::new();

    for ev in parser {
        match ev {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title: img_title,
                id,
            }) => {
                let raw_path = dest_url.as_ref();
                let new_url = if let Some((mime, bytes)) = resolve_image(raw_path) {
                    format!("data:{mime};base64,{}", BASE64.encode(bytes))
                } else {
                    raw_path.to_string()
                };
                events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: new_url.into(),
                    title: img_title,
                    id,
                }));
            }
            other => events.push(other),
        }
    }
    pulldown_cmark::html::push_html(&mut html_body, events.into_iter());

    let esc_title = escape_html(title);
    format!(
        r#"<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{esc_title}</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "IBM Plex Sans", Roboto, sans-serif;
    line-height: 1.65;
    max-width: 820px;
    margin: 40px auto;
    padding: 0 24px;
    color: #1d2025;
    background: #ffffff;
  }}
  h1, h2, h3, h4 {{ line-height: 1.3; margin-top: 1.6em; margin-bottom: 0.5em; font-weight: 600; }}
  h1.doc-title {{ margin-top: 0; font-size: 2.1em; border-bottom: 1px solid #e5e7eb; padding-bottom: 0.35em; }}
  p {{ margin: 0.8em 0; }}
  a {{ color: #2563eb; text-decoration: none; }}
  a:hover {{ text-decoration: underline; }}
  blockquote {{
    margin: 1em 0; padding: 0.4em 1em;
    border-left: 3px solid #3b82f6; background: #f8fafc; color: #475569;
  }}
  code {{
    font-family: "JetBrains Mono", "Cascadia Code", Consolas, monospace;
    font-size: 0.9em; background: #f1f5f9; padding: 0.15em 0.35em; border-radius: 4px;
  }}
  pre {{
    background: #0f172a; color: #e2e8f0; padding: 14px 16px;
    border-radius: 8px; overflow-x: auto;
  }}
  pre code {{ background: none; padding: 0; color: inherit; }}
  table {{ border-collapse: collapse; width: 100%; margin: 1.2em 0; }}
  th, td {{ border: 1px solid #d1d5db; padding: 8px 12px; text-align: left; }}
  th {{ background: #f3f4f6; font-weight: 600; }}
  img {{ max-width: 100%; height: auto; border-radius: 6px; margin: 0.8em 0; }}
  hr {{ border: none; border-top: 1px solid #e5e7eb; margin: 1.8em 0; }}
  @media print {{ body {{ margin: 0; max-width: none; }} }}
</style>
</head>
<body>
<h1 class="doc-title">{esc_title}</h1>
{html_body}
</body>
</html>
"#
    )
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    ListItem {
        ordered: bool,
        index: usize,
        checked: Option<bool>,
        text: String,
    },
    Quote(String),
    CodeBlock { lang: String, code: String },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Image { path: String, alt: String },
    Rule,
}

/// Parses Markdown into structured blocks suitable for offline PDF rendering.
pub fn parse_markdown_to_export_blocks(body_md: &str) -> Vec<ExportBlock> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(body_md, opts);
    let mut blocks = Vec::new();

    let mut heading_lvl: Option<u8> = None;
    let mut in_paragraph = false;
    let mut in_quote = false;
    let mut in_code: Option<String> = None;
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut list_counters: Vec<usize> = Vec::new();
    let mut in_item = false;
    let mut item_checked: Option<bool> = None;

    let mut in_table = false;
    let mut in_table_head = false;
    let mut table_headers: Vec<String> = Vec::new();
    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut in_cell = false;

    let mut text_buf = String::new();
    let mut img_path: Option<String> = None;

    for ev in parser {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_lvl = Some(match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                });
                text_buf.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(lvl) = heading_lvl.take() {
                    let t = text_buf.trim().to_string();
                    if !t.is_empty() {
                        blocks.push(ExportBlock::Heading { level: lvl, text: t });
                    }
                }
                text_buf.clear();
            }
            Event::Start(Tag::Paragraph) => {
                if !in_item && !in_quote {
                    in_paragraph = true;
                    text_buf.clear();
                }
            }
            Event::End(TagEnd::Paragraph) => {
                if in_paragraph && !in_item && !in_quote {
                    in_paragraph = false;
                    let t = text_buf.trim().to_string();
                    if !t.is_empty() {
                        blocks.push(ExportBlock::Paragraph(t));
                    }
                    text_buf.clear();
                }
            }
            Event::Start(Tag::BlockQuote(_)) => {
                in_quote = true;
                text_buf.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                in_quote = false;
                let t = text_buf.trim().to_string();
                if !t.is_empty() {
                    blocks.push(ExportBlock::Quote(t));
                }
                text_buf.clear();
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                in_code = Some(lang);
                text_buf.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(lang) = in_code.take() {
                    blocks.push(ExportBlock::CodeBlock {
                        lang,
                        code: text_buf.trim_end_matches('\n').to_string(),
                    });
                }
                text_buf.clear();
            }
            Event::Start(Tag::List(start)) => {
                list_stack.push(start);
                list_counters.push(start.unwrap_or(1) as usize);
            }
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                list_counters.pop();
            }
            Event::Start(Tag::Item) => {
                in_item = true;
                item_checked = None;
                text_buf.clear();
            }
            Event::End(TagEnd::Item) => {
                in_item = false;
                let ordered = list_stack.last().and_then(|x| *x).is_some();
                let idx = list_counters.last().copied().unwrap_or(1);
                if let Some(last) = list_counters.last_mut() {
                    *last += 1;
                }
                let t = text_buf.trim().to_string();
                if !t.is_empty() {
                    blocks.push(ExportBlock::ListItem {
                        ordered,
                        index: idx,
                        checked: item_checked.take(),
                        text: t,
                    });
                }
                text_buf.clear();
            }
            Event::TaskListMarker(checked) => {
                item_checked = Some(checked);
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_headers.clear();
                table_rows.clear();
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                blocks.push(ExportBlock::Table {
                    headers: std::mem::take(&mut table_headers),
                    rows: std::mem::take(&mut table_rows),
                });
            }
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
                current_row.clear();
            }
            Event::End(TagEnd::TableHead) => {
                in_table_head = false;
                table_headers = std::mem::take(&mut current_row);
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                if !in_table_head {
                    table_rows.push(std::mem::take(&mut current_row));
                }
            }
            Event::Start(Tag::TableCell) => {
                in_cell = true;
                text_buf.clear();
            }
            Event::End(TagEnd::TableCell) => {
                in_cell = false;
                current_row.push(text_buf.trim().to_string());
                text_buf.clear();
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                img_path = Some(dest_url.to_string());
                if !in_paragraph && !in_item && !in_cell {
                    text_buf.clear();
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some(path) = img_path.take() {
                    blocks.push(ExportBlock::Image {
                        path,
                        alt: text_buf.trim().to_string(),
                    });
                    text_buf.clear();
                }
            }
            Event::Rule => {
                blocks.push(ExportBlock::Rule);
            }
            Event::Text(t) | Event::Code(t) => {
                text_buf.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => {
                if in_code.is_some() {
                    text_buf.push('\n');
                } else {
                    text_buf.push(' ');
                }
            }
            _ => {}
        }
    }

    if in_table && (!table_headers.is_empty() || !table_rows.is_empty()) {
        blocks.push(ExportBlock::Table {
            headers: table_headers,
            rows: table_rows,
        });
    }
    blocks
}

/// UTF-8 / Cyrillic-safe word wrapper by character count.
pub fn wrap_text_chars(text: &str, max_chars: usize) -> Vec<String> {
    let limit = max_chars.max(8);
    let mut lines = Vec::new();
    for raw_line in text.split('\n') {
        let trimmed = raw_line.trim_end();
        if trimmed.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut cur_len = 0usize;
        for word in trimmed.split_whitespace() {
            let w_len = word.chars().count();
            if cur_len == 0 {
                if w_len <= limit {
                    current.push_str(word);
                    cur_len = w_len;
                } else {
                    let mut chunk = String::new();
                    let mut c_len = 0usize;
                    for ch in word.chars() {
                        if c_len >= limit {
                            lines.push(chunk);
                            chunk = String::new();
                            c_len = 0;
                        }
                        chunk.push(ch);
                        c_len += 1;
                    }
                    current = chunk;
                    cur_len = c_len;
                }
            } else if cur_len + 1 + w_len <= limit {
                current.push(' ');
                current.push_str(word);
                cur_len += 1 + w_len;
            } else {
                lines.push(current);
                current = String::new();
                if w_len <= limit {
                    current.push_str(word);
                    cur_len = w_len;
                } else {
                    let mut chunk = String::new();
                    let mut c_len = 0usize;
                    for ch in word.chars() {
                        if c_len >= limit {
                            lines.push(chunk);
                            chunk = String::new();
                            c_len = 0;
                        }
                        chunk.push(ch);
                        c_len += 1;
                    }
                    current = chunk;
                    cur_len = c_len;
                }
            }
        }
        if !current.is_empty() {
            lines.push(current);
        }
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_export_embeds_css_tables_images_and_transclusions() {
        let md = "## Раздел\n\n![[Child]]\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] Готово\n\n![pic](img.png)\n";
        let html = export_note_to_html(
            "Тест <Заметка>",
            md,
            |target| {
                if target == "Child" {
                    Some("Вложенный текст".into())
                } else {
                    None
                }
            },
            |path| {
                if path == "img.png" {
                    Some(("image/png".into(), vec![1, 2, 3, 4]))
                } else {
                    None
                }
            },
        );
        assert!(html.contains("<title>Тест &lt;Заметка&gt;</title>"));
        assert!(html.contains("<style>"));
        assert!(html.contains("Вложенный текст"));
        assert!(html.contains("<table>"));
        assert!(html.contains("checked"));
        assert!(html.contains("data:image/png;base64,AQIDBA=="));
    }

    #[test]
    fn transclusion_cycle_guard_terminates() {
        let md = "![[SelfNote]]";
        let out = resolve_transclusions(md, 3, |_| Some("loop ![[SelfNote]]".into()));
        assert!(out.contains("loop"));
    }

    #[test]
    fn wrap_text_chars_handles_long_cyrillic_safely() {
        let s = "ОченьДлинноеКириллическоеСловоБезПробелов и обычный текст для переноса";
        let wrapped = wrap_text_chars(s, 14);
        assert!(wrapped.len() >= 4);
        for line in &wrapped {
            assert!(line.chars().count() <= 14);
        }
    }

    #[test]
    fn parses_blocks_for_pdf() {
        let md = "# Заголовок\n\nТекст\n\n- [ ] Задача\n\n| К1 | К2 |\n|---|---|\n| А | Б |\n";
        let blocks = parse_markdown_to_export_blocks(md);
        assert!(matches!(blocks[0], ExportBlock::Heading { level: 1, .. }));
        assert!(matches!(blocks[1], ExportBlock::Paragraph(_)));
        assert!(matches!(
            blocks[2],
            ExportBlock::ListItem {
                checked: Some(false),
                ..
            }
        ));
        assert!(matches!(blocks[3], ExportBlock::Table { .. }));
    }
}
