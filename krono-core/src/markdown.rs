use pulldown_cmark::{Event, Options, Parser, TagEnd};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiLink {
    pub target: String,
    pub alias: Option<String>,
    pub context: String,
}

pub(crate) fn is_fence_toggle(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// Extracts `[[Target]]` and `[[Target|Alias]]` wikilinks outside fenced code blocks.
pub fn extract_wikilinks(body: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let mut active_fence: Option<char> = None;

    for raw_line in body.lines() {
        let trimmed = raw_line.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if active_fence == Some(ch) {
                active_fence = None;
            } else if active_fence.is_none() {
                active_fence = Some(ch);
            }
            continue;
        }
        if active_fence.is_some() {
            continue;
        }

        let context = raw_line.trim().to_string();
        let mut rest = raw_line;
        while let Some(start) = rest.find("[[") {
            let after = &rest[start + 2..];
            if let Some(end) = after.find("]]") {
                let inner = after[..end].trim();
                if !inner.is_empty() && !inner.contains('\n') {
                    let (target, alias) = match inner.split_once('|') {
                        Some((t, a)) => (t.trim(), Some(a.trim().to_string())),
                        None => (inner, None),
                    };
                    if !target.is_empty() {
                        links.push(WikiLink {
                            target: target.to_string(),
                            alias: alias.filter(|a| !a.is_empty()),
                            context: context.clone(),
                        });
                    }
                }
                rest = &after[end + 2..];
            } else {
                break;
            }
        }
    }

    links
}

/// Extracts deduplicated `![[...board.json]]` embedded board targets outside fenced code blocks.
pub fn extract_embedded_boards(body: &str) -> Vec<String> {
    let mut boards = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut active_fence: Option<char> = None;

    for raw_line in body.lines() {
        let trimmed = raw_line.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if active_fence == Some(ch) {
                active_fence = None;
            } else if active_fence.is_none() {
                active_fence = Some(ch);
            }
            continue;
        }
        if active_fence.is_some() {
            continue;
        }

        let mut rest = raw_line;
        while let Some(start) = rest.find("![[") {
            let after = &rest[start + 3..];
            if let Some(end) = after.find("]]") {
                let inner = after[..end].trim();
                let target = inner
                    .split_once('|')
                    .map(|(t, _)| t.trim())
                    .unwrap_or(inner);
                if !target.is_empty()
                    && !target.contains('\n')
                    && target.to_lowercase().ends_with(".board.json")
                    && seen.insert(target.to_lowercase())
                {
                    boards.push(target.to_string());
                }
                rest = &after[end + 2..];
            } else {
                break;
            }
        }
    }

    boards
}

/// Extracts H1–H3 Markdown headings outside fenced code blocks for the Outline inspector.
pub fn extract_headings(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut active_fence: Option<char> = None;

    for (line_idx, raw_line) in body.lines().enumerate() {
        let trimmed = raw_line.trim();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if active_fence == Some(ch) {
                active_fence = None;
            } else if active_fence.is_none() {
                active_fence = Some(ch);
            }
            continue;
        }
        if active_fence.is_some() {
            continue;
        }

        let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
        if (1..=3).contains(&hashes) {
            let after = &trimmed[hashes..];
            if after.starts_with(' ') || after.starts_with('\t') {
                let text = after.trim().trim_end_matches('#').trim();
                if !text.is_empty() {
                    out.push(Heading {
                        level: hashes as u8,
                        text: text.to_string(),
                        line: line_idx,
                    });
                }
            }
        }
    }

    out
}

/// Strips Markdown formatting via pulldown-cmark to produce a clean card/search excerpt.
pub fn plain_excerpt(body: &str, max_chars: usize) -> String {
    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(body, options);

    let mut plain = String::new();
    for event in parser {
        match event {
            Event::Text(t) | Event::Code(t) => {
                plain.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => plain.push(' '),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::BlockQuote(_)
                | TagEnd::CodeBlock,
            ) => plain.push(' '),
            _ => {}
        }
        if plain.chars().count() > max_chars + 64 {
            break;
        }
    }

    let cleaned_wikilinks = strip_wikilink_markup(&plain);
    let collapsed: String = cleaned_wikilinks
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let count = collapsed.chars().count();
    if count <= max_chars {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated.trim_end())
    }
}

fn strip_wikilink_markup(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = &after[..end];
            let display = match inner.split_once('|') {
                Some((_, alias)) => alias.trim(),
                None => inner.trim(),
            };
            out.push_str(display);
            rest = &after[end + 2..];
        } else {
            out.push_str("[[");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Produces up to `max_lines` clean preview lines from Markdown/frontmatter text,
/// stripping YAML frontmatter, `#` headings, fenced code blocks, and inline formatting.
pub fn card_preview_excerpt(
    raw: &str,
    max_lines: usize,
    max_chars_per_line: usize,
) -> String {
    if max_lines == 0 || max_chars_per_line == 0 {
        return String::new();
    }

    let body = strip_leading_yaml_frontmatter(raw);
    let mut lines_out: Vec<String> = Vec::with_capacity(max_lines);
    let mut in_fence: Option<char> = None;

    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            continue;
        }
        if in_fence.is_some() || trimmed.is_empty() {
            continue;
        }
        // Skip ATX headings (# .. ######)
        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|&c| c == '#').count();
            if (1..=6).contains(&hashes) {
                let after = trimmed[hashes..].chars().next();
                if after.is_none() || after.is_some_and(char::is_whitespace) {
                    continue;
                }
            }
        }
        // Skip horizontal rules (---, ***, ___)
        if trimmed.len() >= 3
            && (trimmed.chars().all(|c| c == '-' || c.is_whitespace())
                && trimmed.chars().filter(|&c| c == '-').count() >= 3
                || trimmed.chars().all(|c| c == '*' || c.is_whitespace())
                    && trimmed.chars().filter(|&c| c == '*').count() >= 3
                || trimmed.chars().all(|c| c == '_' || c.is_whitespace())
                    && trimmed.chars().filter(|&c| c == '_').count() >= 3)
        {
            continue;
        }

        let cleaned = clean_preview_line(trimmed);
        if cleaned.is_empty() {
            continue;
        }
        let clipped = if cleaned.chars().count() > max_chars_per_line {
            let head: String = cleaned
                .chars()
                .take(max_chars_per_line.saturating_sub(1))
                .collect();
            format!("{}…", head.trim_end())
        } else {
            cleaned
        };
        lines_out.push(clipped);
        if lines_out.len() >= max_lines {
            break;
        }
    }

    lines_out.join("\n")
}

fn strip_leading_yaml_frontmatter(raw: &str) -> &str {
    let trimmed_bom = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = trimmed_bom.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return trimmed_bom;
    };
    if first.trim() != "---" {
        return trimmed_bom;
    }
    let mut offset = first.len();
    for line in lines {
        offset += line.len();
        let t = line.trim();
        if t == "---" || t == "..." {
            return &trimmed_bom[offset..];
        }
    }
    trimmed_bom
}

fn clean_preview_line(line: &str) -> String {
    let mut s = line.trim();
    while let Some(rest) = s.strip_prefix('>') {
        s = rest.trim_start();
    }
    for prefix in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "* [X] ", "- ", "* "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim_start();
            break;
        }
    }
    let no_wiki = strip_wikilink_markup(s);
    let no_links = strip_md_inline_links(&no_wiki);
    let mut out = String::with_capacity(no_links.len());
    for ch in no_links.chars() {
        if matches!(ch, '`' | '~') {
            continue;
        }
        out.push(ch);
    }
    let out = out.replace("**", "").replace("__", "");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_md_inline_links(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(lb) = rest.find('[') {
        out.push_str(&rest[..lb]);
        let after_lb = &rest[lb + 1..];
        if let Some(rb) = after_lb.find("](") {
            let label = &after_lb[..rb];
            let after_paren = &after_lb[rb + 2..];
            if let Some(rp) = after_paren.find(')') {
                out.push_str(label);
                rest = &after_paren[rp + 1..];
                continue;
            }
        }
        out.push('[');
        rest = after_lb;
    }
    out.push_str(rest);
    out
}

pub fn count_words(body: &str) -> (usize, usize) {
    let words = body.split_whitespace().count();
    let chars = body.chars().count();
    (words, chars)
}

/// Rewrites `[[target]]` and `[[target|alias]]` wikilinks matching any `old_targets`
/// (case-insensitively) to `new_target`, skipping fenced code blocks and inline code spans.
pub fn rewrite_wikilinks(
    body: &str,
    old_targets: &[&str],
    new_target: &str,
) -> (String, usize) {
    let clean_new = new_target.trim();
    let normalized_olds: Vec<String> = old_targets
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    if normalized_olds.is_empty() || clean_new.is_empty() {
        return (body.to_string(), 0);
    }

    let mut out = String::with_capacity(body.len());
    let mut total_count = 0usize;
    let mut active_fence: Option<char> = None;

    for segment in body.split_inclusive('\n') {
        let (line, nl) = if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (segment, "")
        };

        let trimmed = line.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if active_fence == Some(ch) {
                active_fence = None;
            } else if active_fence.is_none() {
                active_fence = Some(ch);
            }
            out.push_str(line);
            out.push_str(nl);
            continue;
        }
        if active_fence.is_some() {
            out.push_str(line);
            out.push_str(nl);
            continue;
        }

        let (rewritten, count) = rewrite_line_outside_code(line, &normalized_olds, clean_new);
        total_count += count;
        out.push_str(&rewritten);
        out.push_str(nl);
    }

    (out, total_count)
}

fn rewrite_line_outside_code(
    line: &str,
    normalized_olds: &[String],
    new_target: &str,
) -> (String, usize) {
    let mut out = String::with_capacity(line.len());
    let mut count = 0usize;
    let mut i = 0usize;
    let bytes = line.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'`' {
            let mut ticks = 0usize;
            while i + ticks < bytes.len() && bytes[i + ticks] == b'`' {
                ticks += 1;
            }
            let closer = "`".repeat(ticks);
            if let Some(rel_end) = line[i + ticks..].find(&closer) {
                let end = i + ticks + rel_end + ticks;
                out.push_str(&line[i..end]);
                i = end;
                continue;
            }
            out.push_str(&line[i..i + ticks]);
            i += ticks;
            continue;
        }

        if line[i..].starts_with("[[") {
            let after = &line[i + 2..];
            if let Some(end_rel) = after.find("]]") {
                let inner = &after[..end_rel];
                let (raw_target, alias_opt) = match inner.split_once('|') {
                    Some((t, a)) => (t.trim(), Some(a)),
                    None => (inner.trim(), None),
                };
                if !raw_target.is_empty()
                    && normalized_olds
                        .iter()
                        .any(|o| raw_target.to_lowercase() == *o)
                {
                    out.push_str("[[");
                    out.push_str(new_target);
                    if let Some(alias) = alias_opt {
                        out.push('|');
                        out.push_str(alias.trim());
                    }
                    out.push_str("]]");
                    count += 1;
                    i += 2 + end_rel + 2;
                    continue;
                }
            }
        }

        let ch_len = line[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&line[i..i + ch_len]);
        i += ch_len;
    }

    (out, count)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParsedTaskMeta {
    pub clean_text: String,
    pub due_date: Option<String>,
    pub priority: Option<TaskPriority>,
    pub recurrence: Option<crate::tasks::TaskRecurrence>,
    pub tags: Vec<String>,
}

/// Parses inline task metadata tokens like `@2026-11-25`, `@today`/`@сегодня`,
/// `@tomorrow`/`@завтра`, `📅2026-11-25`, `@every day|week|month`, `!high`/`!med`/`!low` and `#tag` from a task line.
pub fn parse_inline_task_meta(raw: &str, today: crate::daily::SimpleDate) -> ParsedTaskMeta {
    use crate::tasks::TaskRecurrence;

    let mut clean_parts: Vec<String> = Vec::new();
    let mut due_date: Option<String> = None;
    let mut priority: Option<TaskPriority> = None;
    let mut recurrence: Option<TaskRecurrence> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut seen_tags = std::collections::HashSet::new();

    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut i = 0usize;
    while i < tokens.len() {
        let token = tokens[i];
        let lower = token.to_lowercase();
        let trimmed_punct = lower.trim_end_matches([',', '.', ';', ':']);

        if (trimmed_punct == "@every" || trimmed_punct == "@каждый" || trimmed_punct == "@каждую")
            && i + 1 < tokens.len()
        {
            let next_lower = tokens[i + 1]
                .to_lowercase()
                .trim_end_matches([',', '.', ';', ':'])
                .to_string();
            let parsed_rec = match next_lower.as_str() {
                "day" | "день" => Some(TaskRecurrence::Daily),
                "week" | "неделю" | "неделя" => Some(TaskRecurrence::Weekly),
                "month" | "месяц" => Some(TaskRecurrence::Monthly),
                _ => None,
            };
            if let Some(rec) = parsed_rec {
                recurrence = Some(rec);
                i += 2;
                continue;
            }
        }
        if matches!(trimmed_punct, "@daily" | "@ежедневно") {
            recurrence = Some(TaskRecurrence::Daily);
            i += 1;
            continue;
        }
        if matches!(trimmed_punct, "@weekly" | "@еженедельно") {
            recurrence = Some(TaskRecurrence::Weekly);
            i += 1;
            continue;
        }
        if matches!(trimmed_punct, "@monthly" | "@ежемесячно") {
            recurrence = Some(TaskRecurrence::Monthly);
            i += 1;
            continue;
        }

        if trimmed_punct == "@today" || trimmed_punct == "@сегодня" {
            if due_date.is_none() {
                due_date = Some(today.format_iso());
            }
            i += 1;
            continue;
        }
        if trimmed_punct == "@tomorrow" || trimmed_punct == "@завтра" {
            if due_date.is_none() {
                due_date = Some(today.next_day().format_iso());
            }
            i += 1;
            continue;
        }
        if let Some(date_raw) = token
            .strip_prefix('@')
            .or_else(|| token.strip_prefix('📅'))
        {
            let clean_d = date_raw.trim_end_matches([',', '.', ';', ':']);
            if let Some(parsed) = crate::daily::SimpleDate::parse_iso(clean_d) {
                if due_date.is_none() {
                    due_date = Some(parsed.format_iso());
                }
                i += 1;
                continue;
            }
        }
        if matches!(
            trimmed_punct,
            "!high" | "!p1" | "!высокий" | "!срочно"
        ) {
            priority = Some(TaskPriority::High);
            i += 1;
            continue;
        }
        if matches!(
            trimmed_punct,
            "!med" | "!medium" | "!p2" | "!средний"
        ) {
            priority = Some(TaskPriority::Medium);
            i += 1;
            continue;
        }
        if matches!(trimmed_punct, "!low" | "!p3" | "!низкий") {
            priority = Some(TaskPriority::Low);
            i += 1;
            continue;
        }
        if let Some(raw_tag) = token.strip_prefix('#') {
            let clean_tag = raw_tag.trim_end_matches([',', '.', ';', ':']);
            if !clean_tag.is_empty() && !clean_tag.chars().all(|c| c == '#') {
                let key = clean_tag.to_lowercase();
                if seen_tags.insert(key) {
                    tags.push(clean_tag.to_string());
                }
            }
        }
        clean_parts.push(token.to_string());
        i += 1;
    }

    ParsedTaskMeta {
        clean_text: clean_parts.join(" "),
        due_date,
        priority,
        recurrence,
        tags,
    }
}

pub(crate) fn match_task_line_prefix(trimmed_start: &str) -> Option<(&'static str, &str)> {
    for prefix in [
        "- [ ] ",
        "- [x] ",
        "- [X] ",
        "* [ ] ",
        "* [x] ",
        "* [X] ",
    ] {
        if let Some(rest) = trimmed_start.strip_prefix(prefix) {
            return Some((prefix, rest));
        }
    }
    None
}

/// Extracts the clean text body of the `target_idx`-th Markdown task checkbox outside fenced code blocks.
pub fn extract_nth_task_line(body: &str, target_idx: usize) -> Option<String> {
    let mut in_fence: Option<char> = None;
    let mut current_idx = 0usize;

    for segment in body.split_inclusive('\n') {
        let trimmed = segment.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            continue;
        }
        if in_fence.is_some() {
            continue;
        }
        if let Some((_, rest)) = match_task_line_prefix(trimmed) {
            if current_idx == target_idx {
                return Some(rest.trim_end_matches(['\r', '\n']).trim().to_string());
            }
            current_idx += 1;
        }
    }
    None
}

/// Removes the `target_idx`-th Markdown task checkbox line outside fenced code blocks.
pub fn remove_nth_task_line(body: &str, target_idx: usize) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut in_fence: Option<char> = None;
    let mut current_idx = 0usize;
    let mut removed = false;

    for segment in body.split_inclusive('\n') {
        let trimmed = segment.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            out.push_str(segment);
            continue;
        }
        if in_fence.is_none() && !removed {
            if match_task_line_prefix(trimmed).is_some() {
                if current_idx == target_idx {
                    removed = true;
                    continue;
                }
                current_idx += 1;
            }
        }
        out.push_str(segment);
    }

    if removed {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_wikilinks_with_aliases_and_skips_code_blocks() {
        let body = "See [[Architecture]] and [[Roadmap|Q4 Plan]].\n```rust\nlet x = \"[[Ignored]]\";\n```\nBack to [[Welcome]].";
        let links = extract_wikilinks(body);
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].target, "Architecture");
        assert_eq!(links[0].alias, None);
        assert_eq!(links[1].target, "Roadmap");
        assert_eq!(links[1].alias.as_deref(), Some("Q4 Plan"));
        assert_eq!(links[2].target, "Welcome");
    }

    #[test]
    fn extracts_headings_and_skips_code_comments() {
        let body = "# Title\n```python\n# not a heading\n```\n## Section\n### Sub ###";
        let headings = extract_headings(body);
        assert_eq!(
            headings,
            vec![
                Heading {
                    level: 1,
                    text: "Title".into(),
                    line: 0
                },
                Heading {
                    level: 2,
                    text: "Section".into(),
                    line: 4
                },
                Heading {
                    level: 3,
                    text: "Sub".into(),
                    line: 5
                },
            ]
        );
    }

    #[test]
    fn produces_clean_excerpt() {
        let body = "# Hello\n\nThis is **bold** and [[Linked Note]] with `code`.";
        assert_eq!(
            plain_excerpt(body, 100),
            "Hello This is bold and Linked Note with code."
        );
    }

    #[test]
    fn rewrites_wikilinks_skipping_code_blocks_and_inline_code() {
        let body = "See [[Дорожная карта]] and [[дорожная карта|План]].\nKeep `[[Дорожная карта]]` in inline code.\n```md\n[[Дорожная карта]]\n```\nAlso [[old/road]] stays.";
        let (updated, count) =
            rewrite_wikilinks(body, &["Дорожная карта", "projects/roadmap"], "Карта v2");
        assert_eq!(count, 2);
        assert!(updated.contains("See [[Карта v2]] and [[Карта v2|План]]."));
        assert!(updated.contains("Keep `[[Дорожная карта]]` in inline code."));
        assert!(updated.contains("```md\n[[Дорожная карта]]\n```"));
        assert!(updated.contains("Also [[old/road]] stays."));
    }

    #[test]
    fn extracts_embedded_boards_ignoring_code_and_images() {
        let body = "Text\n![[assets/a.png]]\n![[projects/tasks.board.json]]\n```md\n![[ignored/test.board.json]]\n```\n![[projects/tasks.board.json|Sprint]]\n";
        let boards = extract_embedded_boards(body);
        assert_eq!(boards, vec!["projects/tasks.board.json".to_string()]);
        let links = extract_wikilinks(body);
        assert!(links.iter().any(|l| l.target == "projects/tasks.board.json"));
    }

    #[test]
    fn parses_inline_task_meta_and_extracts_or_removes_nth_task() {
        let today = crate::daily::SimpleDate::from_ymd(2026, 11, 20).unwrap();
        let meta = parse_inline_task_meta("Сверстать Canvas #ui @tomorrow !high", today);
        assert_eq!(meta.due_date.as_deref(), Some("2026-11-21"));
        assert_eq!(meta.priority, Some(TaskPriority::High));
        assert_eq!(meta.tags, vec!["ui".to_string()]);
        assert_eq!(meta.clean_text, "Сверстать Canvas #ui");

        let meta_explicit = parse_inline_task_meta("Релиз @2026-12-01 !med", today);
        assert_eq!(meta_explicit.due_date.as_deref(), Some("2026-12-01"));
        assert_eq!(meta_explicit.priority, Some(TaskPriority::Medium));
        assert_eq!(meta_explicit.clean_text, "Релиз");

        let body = "## Plan\n```md\n- [ ] Fake inside fence\n```\n- [ ] First task @today !low\n- [x] Second task\n";
        assert_eq!(
            extract_nth_task_line(body, 0).as_deref(),
            Some("First task @today !low")
        );
        assert_eq!(
            extract_nth_task_line(body, 1).as_deref(),
            Some("Second task")
        );
        let stripped = remove_nth_task_line(body, 0).unwrap();
        assert!(!stripped.contains("First task"));
        assert!(stripped.contains("Fake inside fence"));
        assert!(stripped.contains("- [x] Second task"));
    }

    #[test]
    fn card_preview_strips_frontmatter_headings_and_fences() {
        let md = "---\ntitle: Spec\ntags: ui\n---\n# Main Heading\n## Subheading\n```rust\nlet x = 1;\n```\nFirst **meaningful** line with [[Target|Alias]]\n- [ ] Second [link](https://example.com) line\nThird ignored line\n";
        assert_eq!(
            card_preview_excerpt(md, 2, 80),
            "First meaningful line with Alias\nSecond link line"
        );

        let only_headers = "---\ntitle: Empty\n---\n# Title\n## Section\n";
        assert_eq!(card_preview_excerpt(only_headers, 2, 80), "");
    }
}
