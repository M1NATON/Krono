#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupLang {
    Ru,
    En,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SmartPasteOutcome {
    WrapLink(String),
    SuggestWikilink { note_title: String },
    Plain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtractedNotePlan {
    pub title: String,
    pub body: String,
    pub replacement_link: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineKind {
    Blank,
    Heading,
    Quote,
    Table,
    Rule,
    Paragraph,
}

pub fn smart_markdown_cleanup(input: &str, lang: CleanupLang) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.trim().is_empty() {
        return String::new();
    }

    let mut out_lines: Vec<(String, bool)> = Vec::new(); // (line, is_paragraph_joinable)
    let mut in_fence: Option<(char, usize)> = None;
    let mut prev_was_blank = true;

    for raw_line in normalized.split('\n') {
        if let Some((f_ch, f_len)) = in_fence {
            out_lines.push((raw_line.to_string(), false));
            if is_closing_fence(raw_line, f_ch, f_len) {
                in_fence = None;
            }
            prev_was_blank = false;
            continue;
        }

        if let Some((f_ch, f_len)) = detect_opening_fence(raw_line) {
            out_lines.push((raw_line.trim_end().to_string(), false));
            in_fence = Some((f_ch, f_len));
            prev_was_blank = false;
            continue;
        }

        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if !prev_was_blank && !out_lines.is_empty() {
                out_lines.push((String::new(), false));
                prev_was_blank = true;
            }
            continue;
        }
        prev_was_blank = false;

        if let Some(list_line) = normalize_list_line(raw_line) {
            let formatted = apply_prose_typography(&list_line, lang);
            out_lines.push((formatted, false));
            continue;
        }

        let kind = classify_line(trimmed);
        match kind {
            LineKind::Blank => {}
            LineKind::Heading | LineKind::Quote | LineKind::Table | LineKind::Rule => {
                let clean = if kind == LineKind::Table || kind == LineKind::Rule {
                    trimmed.to_string()
                } else {
                    apply_prose_typography(trimmed, lang)
                };
                out_lines.push((clean, false));
            }
            LineKind::Paragraph => {
                let has_hard_break = raw_line.ends_with("  ") || raw_line.ends_with('\\');
                let clean_part = if has_hard_break {
                    format!("{}  ", apply_prose_typography(trimmed, lang))
                } else {
                    apply_prose_typography(trimmed, lang)
                };

                if let Some((prev_line, true)) = out_lines.last_mut() {
                    prev_line.push(' ');
                    prev_line.push_str(&clean_part);
                    // Re-apply typography on joined paragraph so quotes spanning lines pair cleanly.
                    *prev_line = apply_prose_typography(prev_line, lang);
                    if has_hard_break {
                        out_lines.last_mut().expect("non-empty").1 = false;
                    }
                } else {
                    out_lines.push((clean_part, !has_hard_break));
                }
            }
        }
    }

    while out_lines.last().is_some_and(|(l, _)| l.is_empty()) {
        out_lines.pop();
    }

    out_lines
        .into_iter()
        .map(|(l, _)| l)
        .collect::<Vec<_>>()
        .join("\n")
}

fn detect_opening_fence(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start();
    let ch = t.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let count = t.chars().take_while(|&c| c == ch).count();
    if count >= 3 {
        if ch == '`' && t[count..].contains('`') {
            return None;
        }
        Some((ch, count))
    } else {
        None
    }
}

fn is_closing_fence(line: &str, f_ch: char, f_len: usize) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    let count = t.chars().take_while(|&c| c == f_ch).count();
    count >= f_len && t.chars().all(|c| c == f_ch)
}

fn classify_line(trimmed: &str) -> LineKind {
    if trimmed.is_empty() {
        return LineKind::Blank;
    }
    if is_atx_heading(trimmed) {
        return LineKind::Heading;
    }
    if trimmed.starts_with('>') {
        return LineKind::Quote;
    }
    if is_horizontal_rule(trimmed) {
        return LineKind::Rule;
    }
    if is_table_line(trimmed) {
        return LineKind::Table;
    }
    LineKind::Paragraph
}

fn is_atx_heading(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return false;
    }
    trimmed[hashes..].is_empty() || trimmed[hashes..].starts_with(' ') || trimmed[hashes..].starts_with('\t')
}

fn is_horizontal_rule(trimmed: &str) -> bool {
    let no_space: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if no_space.len() < 3 {
        return false;
    }
    let first = no_space.chars().next().unwrap_or(' ');
    (first == '-' || first == '*' || first == '_') && no_space.chars().all(|c| c == first)
}

fn is_table_line(trimmed: &str) -> bool {
    trimmed.starts_with('|') && trimmed.len() > 1 && trimmed.contains('|')
}

fn normalize_list_line(raw_line: &str) -> Option<String> {
    let mut indent_spaces = 0usize;
    let mut byte_idx = 0usize;
    for (idx, ch) in raw_line.char_indices() {
        match ch {
            ' ' => {
                indent_spaces += 1;
                byte_idx = idx + 1;
            }
            '\t' => {
                indent_spaces += 2;
                byte_idx = idx + 1;
            }
            _ => break,
        }
    }
    let rest = &raw_line[byte_idx..];
    if rest.is_empty() || is_horizontal_rule(rest.trim()) {
        return None;
    }

    let norm_indent = if indent_spaces == 0 {
        0
    } else {
        ((indent_spaces + 1) / 2) * 2
    };
    let indent_str = " ".repeat(norm_indent);

    // Unordered bullets: -, *, +, •, ◦, ▪, —, –
    for bullet in ["- ", "* ", "+ ", "• ", "◦ ", "▪ ", "— ", "– "] {
        if let Some(after) = rest.strip_prefix(bullet) {
            return Some(format!("{indent_str}- {}", after.trim_start()));
        }
    }
    // Bullet char immediately followed by tab
    for bullet_ch in ['-', '*', '+', '•', '◦', '▪'] {
        if let Some(after) = rest.strip_prefix(bullet_ch) {
            if let Some(after_tab) = after.strip_prefix('\t') {
                return Some(format!("{indent_str}- {}", after_tab.trim_start()));
            }
        }
    }

    // Ordered list: `1. text` or `1) text`
    let digits_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits_len > 0 && digits_len <= 6 {
        let after_digits = &rest[digits_len..];
        if let Some(after_sep) = after_digits
            .strip_prefix(". ")
            .or_else(|| after_digits.strip_prefix(") "))
            .or_else(|| after_digits.strip_prefix(".\t"))
            .or_else(|| after_digits.strip_prefix(")\t"))
        {
            let num = &rest[..digits_len];
            return Some(format!("{indent_str}{num}. {}", after_sep.trim_start()));
        }
    }

    None
}

fn apply_prose_typography(line: &str, lang: CleanupLang) -> String {
    // Preserve leading indentation exactly.
    let lead_len = line
        .char_indices()
        .find(|&(_, c)| c != ' ')
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let lead = &line[..lead_len];
    let content = line[lead_len..].trim_end();

    // Tokenize into protected spans (inline code, wikilinks, markdown links, URLs) vs prose.
    let segments = split_protected_segments(content);

    // Count straight double quotes across prose segments so we only pair even counts.
    let total_quotes: usize = segments
        .iter()
        .filter(|(is_prot, _)| !is_prot)
        .map(|(_, seg)| seg.chars().filter(|&c| c == '"').count())
        .sum();
    let max_paired_quotes = total_quotes - (total_quotes % 2);
    let mut quote_idx = 0usize;

    let mut out = String::with_capacity(line.len() + 8);
    out.push_str(lead);
    for (is_prot, seg) in segments {
        if is_prot {
            out.push_str(seg);
        } else {
            out.push_str(&transform_prose_segment(
                seg,
                lang,
                &mut quote_idx,
                max_paired_quotes,
            ));
        }
    }
    out
}

fn split_protected_segments(s: &str) -> Vec<(bool, &str)> {
    let mut res = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut prose_start = 0usize;

    while i < bytes.len() {
        // Inline code: `...`
        if bytes[i] == b'`' {
            if let Some(rel_end) = s[i + 1..].find('`') {
                if i > prose_start {
                    res.push((false, &s[prose_start..i]));
                }
                let end = i + 1 + rel_end + 1;
                res.push((true, &s[i..end]));
                i = end;
                prose_start = i;
                continue;
            }
        }
        // Wikilink / embed: [[...]] or ![[...]]
        if s[i..].starts_with("[[") {
            if let Some(rel_end) = s[i + 2..].find("]]") {
                if i > prose_start {
                    res.push((false, &s[prose_start..i]));
                }
                let end = i + 2 + rel_end + 2;
                res.push((true, &s[i..end]));
                i = end;
                prose_start = i;
                continue;
            }
        }
        // Markdown link/image target: ](...)
        if s[i..].starts_with("](") {
            if let Some(rel_end) = s[i + 2..].find(')') {
                if i > prose_start {
                    res.push((false, &s[prose_start..i]));
                }
                let end = i + 2 + rel_end + 1;
                res.push((true, &s[i..end]));
                i = end;
                prose_start = i;
                continue;
            }
        }
        // Raw URL: http:// or https://
        if s[i..].starts_with("http://") || s[i..].starts_with("https://") {
            if i > prose_start {
                res.push((false, &s[prose_start..i]));
            }
            let end = s[i..]
                .find(|c: char| c.is_whitespace())
                .map(|rel| i + rel)
                .unwrap_or(s.len());
            res.push((true, &s[i..end]));
            i = end;
            prose_start = i;
            continue;
        }

        let ch_len = s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        i += ch_len;
    }

    if prose_start < s.len() {
        res.push((false, &s[prose_start..]));
    }
    res
}

fn transform_prose_segment(
    seg: &str,
    lang: CleanupLang,
    quote_idx: &mut usize,
    max_paired_quotes: usize,
) -> String {
    // 1. Collapse multiple spaces into one.
    let mut collapsed = String::with_capacity(seg.len());
    let mut prev_space = false;
    for ch in seg.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                collapsed.push(' ');
                prev_space = true;
            }
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }

    // 2. Replace spaced dashes with em-dash ` — `.
    let dashed = collapsed
        .replace(" -- ", " — ")
        .replace(" - ", " — ")
        .replace(" – ", " — ");

    // 3. Replace paired straight quotes with typographic quotes.
    let (open_q, close_q) = match lang {
        CleanupLang::Ru => ('«', '»'),
        CleanupLang::En => ('“', '”'),
    };
    let mut out = String::with_capacity(dashed.len() + 4);
    for ch in dashed.chars() {
        if ch == '"' && *quote_idx < max_paired_quotes {
            if *quote_idx % 2 == 0 {
                out.push(open_q);
            } else {
                out.push(close_q);
            }
            *quote_idx += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn html_to_clean_markdown(raw: &str, lang: CleanupLang) -> String {
    let fragment = extract_cf_html_fragment(raw);
    if !looks_like_html(fragment) {
        return smart_markdown_cleanup(fragment, lang);
    }

    let md = convert_html_fragment_to_md(fragment);
    smart_markdown_cleanup(&md, lang)
}

fn extract_cf_html_fragment(raw: &str) -> &str {
    if let Some(start) = raw.find("<!--StartFragment-->") {
        let after = &raw[start + "<!--StartFragment-->".len()..];
        if let Some(end) = after.find("<!--EndFragment-->") {
            return &after[..end];
        }
        return after;
    }
    if let Some(b_start) = raw.to_ascii_lowercase().find("<body") {
        if let Some(gt) = raw[b_start..].find('>') {
            let after = &raw[b_start + gt + 1..];
            if let Some(b_end) = after.to_ascii_lowercase().find("</body>") {
                return &after[..b_end];
            }
            return after;
        }
    }
    raw
}

fn looks_like_html(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    [
        "<p", "<div", "<h1", "<h2", "<h3", "<h4", "<h5", "<h6", "<a ", "<strong", "<b", "<em",
        "<i", "<code", "<pre", "<ul", "<ol", "<li", "<br", "<blockquote", "<table", "<tr",
        "<td", "<th", "<span",
    ]
    .iter()
    .any(|tag| low.contains(tag))
}

fn convert_html_fragment_to_md(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut i = 0usize;
    let mut skip_depth = 0usize;
    let mut in_pre = false;
    let mut in_blockquote = false;
    let mut list_stack: Vec<(bool, usize)> = Vec::new(); // (is_ordered, counter)
    let mut link_href: Option<String> = None;
    let mut table_row_cells: Vec<String> = Vec::new();
    let mut table_rows: Vec<(bool, Vec<String>)> = Vec::new(); // (is_header, cells)
    let mut in_table = false;
    let mut in_cell = false;
    let mut cell_is_header = false;
    let mut cell_buf = String::new();

    while i < html.len() {
        if html[i..].starts_with("<!--") {
            if let Some(end) = html[i + 4..].find("-->") {
                i = i + 4 + end + 3;
                continue;
            }
        }

        if html.as_bytes()[i] == b'<' {
            if let Some(rel_gt) = html[i + 1..].find('>') {
                let raw_tag = &html[i + 1..i + 1 + rel_gt];
                i = i + 1 + rel_gt + 1;
                let tag_trim = raw_tag.trim();
                let is_closing = tag_trim.starts_with('/');
                let is_self_closing = tag_trim.ends_with('/');
                let name_part = tag_trim
                    .trim_start_matches('/')
                    .trim_end_matches('/')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();

                if matches!(name_part.as_str(), "script" | "style" | "head") {
                    if !is_closing && !is_self_closing {
                        skip_depth += 1;
                    } else if is_closing && skip_depth > 0 {
                        skip_depth -= 1;
                    }
                    continue;
                }
                if skip_depth > 0 {
                    continue;
                }

                let target = if in_cell { &mut cell_buf } else { &mut out };
                match name_part.as_str() {
                    "br" => {
                        target.push('\n');
                    }
                    "hr" => {
                        ensure_blank_line(target);
                        target.push_str("---\n\n");
                    }
                    "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let lvl = name_part[1..].parse::<usize>().unwrap_or(1);
                        if !is_closing {
                            ensure_blank_line(target);
                            for _ in 0..lvl {
                                target.push('#');
                            }
                            target.push(' ');
                        } else {
                            target.push_str("\n\n");
                        }
                    }
                    "p" | "div" | "section" | "article" => {
                        if !is_closing {
                            ensure_newline(target);
                        } else {
                            target.push_str("\n\n");
                        }
                    }
                    "strong" | "b" => target.push_str("**"),
                    "em" | "i" => target.push('*'),
                    "pre" => {
                        if !is_closing {
                            ensure_blank_line(target);
                            target.push_str("```\n");
                            in_pre = true;
                        } else {
                            if !target.ends_with('\n') {
                                target.push('\n');
                            }
                            target.push_str("```\n\n");
                            in_pre = false;
                        }
                    }
                    "code" => {
                        if !in_pre {
                            target.push('`');
                        }
                    }
                    "blockquote" => {
                        if !is_closing {
                            ensure_blank_line(target);
                            target.push_str("> ");
                            in_blockquote = true;
                        } else {
                            target.push_str("\n\n");
                            in_blockquote = false;
                        }
                    }
                    "ul" => {
                        if !is_closing {
                            if list_stack.is_empty() {
                                ensure_blank_line(target);
                            } else {
                                ensure_newline(target);
                            }
                            list_stack.push((false, 0));
                        } else {
                            list_stack.pop();
                            if list_stack.is_empty() {
                                target.push('\n');
                            }
                        }
                    }
                    "ol" => {
                        if !is_closing {
                            if list_stack.is_empty() {
                                ensure_blank_line(target);
                            } else {
                                ensure_newline(target);
                            }
                            list_stack.push((true, 0));
                        } else {
                            list_stack.pop();
                            if list_stack.is_empty() {
                                target.push('\n');
                            }
                        }
                    }
                    "li" => {
                        if !is_closing {
                            ensure_newline(target);
                            let depth = list_stack.len().saturating_sub(1);
                            for _ in 0..depth {
                                target.push_str("  ");
                            }
                            if let Some((ordered, cnt)) = list_stack.last_mut() {
                                *cnt += 1;
                                if *ordered {
                                    target.push_str(&format!("{cnt}. "));
                                } else {
                                    target.push_str("- ");
                                }
                            } else {
                                target.push_str("- ");
                            }
                        } else {
                            ensure_newline(target);
                        }
                    }
                    "a" => {
                        if !is_closing {
                            let href = extract_attr(raw_tag, "href").unwrap_or_default();
                            if !href.is_empty() && !href.starts_with("javascript:") {
                                link_href = Some(href);
                                target.push('[');
                            } else {
                                link_href = None;
                            }
                        } else if let Some(href) = link_href.take() {
                            target.push_str(&format!("]({href})"));
                        }
                    }
                    "table" => {
                        if !is_closing {
                            in_table = true;
                            table_rows.clear();
                        } else {
                            in_table = false;
                            flush_html_table(&mut out, &table_rows);
                            table_rows.clear();
                        }
                    }
                    "tr" => {
                        if in_table {
                            if !is_closing {
                                table_row_cells.clear();
                                cell_is_header = false;
                            } else if !table_row_cells.is_empty() {
                                table_rows.push((cell_is_header, std::mem::take(&mut table_row_cells)));
                            }
                        }
                    }
                    "th" | "td" => {
                        if in_table {
                            if !is_closing {
                                in_cell = true;
                                cell_is_header = cell_is_header || name_part == "th";
                                cell_buf.clear();
                            } else {
                                in_cell = false;
                                table_row_cells.push(cell_buf.trim().replace('\n', " "));
                                cell_buf.clear();
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
        }

        let next_lt = html[i..].find('<').map(|rel| i + rel).unwrap_or(html.len());
        if skip_depth == 0 {
            let chunk = &html[i..next_lt];
            let decoded = decode_html_entities(chunk);
            let target = if in_cell { &mut cell_buf } else { &mut out };
            if in_pre {
                target.push_str(&decoded);
            } else {
                let collapsed = collapse_html_whitespace(&decoded);
                if in_blockquote && collapsed.contains('\n') {
                    target.push_str(&collapsed.replace('\n', "\n> "));
                } else {
                    target.push_str(&collapsed);
                }
            }
        }
        i = next_lt;
    }

    out
}

fn ensure_newline(s: &mut String) {
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
}

fn ensure_blank_line(s: &mut String) {
    if s.is_empty() {
        return;
    }
    if !s.ends_with('\n') {
        s.push_str("\n\n");
    } else if !s.ends_with("\n\n") {
        s.push('\n');
    }
}

fn flush_html_table(out: &mut String, rows: &[(bool, Vec<String>)]) {
    if rows.is_empty() {
        return;
    }
    ensure_blank_line(out);
    let cols = rows.iter().map(|(_, r)| r.len()).max().unwrap_or(1).max(1);
    for (idx, (_, cells)) in rows.iter().enumerate() {
        out.push('|');
        for c in 0..cols {
            let val = cells.get(c).map(String::as_str).unwrap_or("");
            out.push(' ');
            out.push_str(&val.replace('|', "\\|"));
            out.push_str(" |");
        }
        out.push('\n');
        if idx == 0 {
            out.push('|');
            for _ in 0..cols {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    out.push('\n');
}

fn collapse_html_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch == '\n' || ch == '\r' || ch == '\t' || ch == ' ' {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out
}

fn extract_attr(tag: &str, attr_name: &str) -> Option<String> {
    let low = tag.to_ascii_lowercase();
    let needle = format!("{attr_name}=");
    let idx = low.find(&needle)?;
    let rest = tag[idx + needle.len()..].trim_start();
    if let Some(q) = rest.chars().next().filter(|&c| c == '"' || c == '\'') {
        let inner = &rest[q.len_utf8()..];
        let end = inner.find(q)?;
        Some(decode_html_entities(&inner[..end]))
    } else {
        let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
        Some(decode_html_entities(&rest[..end]))
    }
}

fn decode_html_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < s.len() {
        if s.as_bytes()[i] == b'&' {
            if let Some(semi) = s[i + 1..].find(';').filter(|&d| d <= 10) {
                let ent = &s[i + 1..i + 1 + semi];
                let decoded = match ent {
                    "amp" => Some("&".to_string()),
                    "lt" => Some("<".to_string()),
                    "gt" => Some(">".to_string()),
                    "quot" => Some("\"".to_string()),
                    "apos" | "#39" => Some("'".to_string()),
                    "nbsp" => Some(" ".to_string()),
                    "mdash" => Some("—".to_string()),
                    "ndash" => Some("–".to_string()),
                    "laquo" => Some("«".to_string()),
                    "raquo" => Some("»".to_string()),
                    _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                        u32::from_str_radix(&ent[2..], 16)
                            .ok()
                            .and_then(char::from_u32)
                            .map(|c| c.to_string())
                    }
                    _ if ent.starts_with('#') => ent[1..]
                        .parse::<u32>()
                        .ok()
                        .and_then(char::from_u32)
                        .map(|c| c.to_string()),
                    _ => None,
                };
                if let Some(ch) = decoded {
                    out.push_str(&ch);
                    i += semi + 2;
                    continue;
                }
            }
        }
        let ch = s[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

pub fn classify_smart_paste(
    selected_text: &str,
    clipboard_text: &str,
    existing_titles: &[String],
) -> SmartPasteOutcome {
    let trimmed_clip = clipboard_text.trim();
    if trimmed_clip.is_empty() {
        return SmartPasteOutcome::Plain;
    }

    if !selected_text.trim().is_empty()
        && (trimmed_clip.starts_with("http://") || trimmed_clip.starts_with("https://"))
        && !trimmed_clip.chars().any(char::is_whitespace)
    {
        return SmartPasteOutcome::WrapLink(format!("[{selected_text}]({trimmed_clip})"));
    }

    if selected_text.is_empty()
        && !(trimmed_clip.starts_with("[[") && trimmed_clip.ends_with("]]"))
        && !trimmed_clip.contains('\n')
    {
        let clip_low = trimmed_clip.to_lowercase();
        if let Some(found) = existing_titles.iter().find(|t| t.trim().to_lowercase() == clip_low) {
            return SmartPasteOutcome::SuggestWikilink {
                note_title: found.trim().to_string(),
            };
        }
    }

    SmartPasteOutcome::Plain
}

pub fn plan_extract_selection(selection: &str, fallback_title: &str) -> Option<ExtractedNotePlan> {
    let trimmed = selection.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut lines = trimmed.lines();
    let first_non_empty = lines.find(|l| !l.trim().is_empty()).unwrap_or("");
    let raw_first = first_non_empty.trim();
    let is_heading = raw_first.starts_with('#');

    let mut cleaned = raw_first.trim_start_matches('#').trim();
    for (open, close) in [("**", "**"), ("__", "__"), ("[[", "]]"), ("*", "*"), ("_", "_")] {
        if cleaned.starts_with(open) && cleaned.ends_with(close) && cleaned.len() > open.len() + close.len() {
            cleaned = cleaned[open.len()..cleaned.len() - close.len()].trim();
        }
    }
    let cleaned_title: String = cleaned.chars().take(80).collect::<String>().trim().to_string();
    let title = if cleaned_title.is_empty() {
        fallback_title.to_string()
    } else {
        cleaned_title
    };

    let body = if is_heading {
        let rest: Vec<&str> = trimmed.lines().skip(1).collect();
        let rest_joined = rest.join("\n").trim().to_string();
        if rest_joined.is_empty() {
            trimmed.to_string()
        } else {
            rest_joined
        }
    } else {
        trimmed.to_string()
    };

    Some(ExtractedNotePlan {
        replacement_link: format!("[[{title}]]"),
        title,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_joins_broken_paragraphs_and_preserves_structures() {
        let src = "Первая строка\nвторая строка   с  пробелами.\n\n• Пункт один\n  * Вложенный пункт\n1) Нумерованный\n\n> Цитата\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\n```rust\nlet x =  1;\nlet s = \"raw\";\n```";
        let out = smart_markdown_cleanup(src, CleanupLang::Ru);
        assert!(out.contains("Первая строка вторая строка с пробелами."));
        assert!(out.contains("- Пункт один\n  - Вложенный пункт\n1. Нумерованный"));
        assert!(out.contains("> Цитата"));
        assert!(out.contains("| A | B |"));
        assert!(out.contains("let x =  1;\nlet s = \"raw\";"));
    }

    #[test]
    fn cleanup_applies_ru_and_en_typography_safely() {
        let src = "Он сказал \"привет\" - и открыл `let a = \"x\";` по https://example.com/a-b";
        let ru = smart_markdown_cleanup(src, CleanupLang::Ru);
        assert_eq!(
            ru,
            "Он сказал «привет» — и открыл `let a = \"x\";` по https://example.com/a-b"
        );
        let en = smart_markdown_cleanup("She said \"hello\" - then left.", CleanupLang::En);
        assert_eq!(en, "She said “hello” — then left.");
    }

    #[test]
    fn html_to_clean_markdown_handles_cf_html_and_lists() {
        let raw = "Version:0.9\r\n<!--StartFragment--><h2>Заголовок</h2><p>Текст со <a href=\"https://krono.dev\">ссылкой</a> и <strong>жирным</strong>.</p><ul><li>Раз</li><li>Два</li></ul><!--EndFragment-->";
        let md = html_to_clean_markdown(raw, CleanupLang::Ru);
        assert!(md.contains("## Заголовок"));
        assert!(md.contains("Текст со [ссылкой](https://krono.dev) и **жирным**."));
        assert!(md.contains("- Раз\n- Два"));
    }

    #[test]
    fn classify_smart_paste_wraps_url_and_suggests_wikilink() {
        let titles = vec!["Архитектура Krono".to_string()];
        assert_eq!(
            classify_smart_paste("документация", "https://example.com", &titles),
            SmartPasteOutcome::WrapLink("[документация](https://example.com)".to_string())
        );
        assert_eq!(
            classify_smart_paste("", "архитектура krono", &titles),
            SmartPasteOutcome::SuggestWikilink {
                note_title: "Архитектура Krono".to_string()
            }
        );
        assert_eq!(classify_smart_paste("", "обычный текст", &titles), SmartPasteOutcome::Plain);
    }

    #[test]
    fn plan_extract_selection_strips_heading_and_rejects_empty() {
        assert!(plan_extract_selection("   \n ", "Note").is_none());
        let plan = plan_extract_selection("## Новый раздел\n\nТекст абзаца", "Fallback").unwrap();
        assert_eq!(plan.title, "Новый раздел");
        assert_eq!(plan.body, "Текст абзаца");
        assert_eq!(plan.replacement_link, "[[Новый раздел]]");
    }
}
