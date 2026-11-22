use std::ops::Range;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedSearchQuery {
    pub tags: Vec<String>,
    pub statuses: Vec<String>,
    pub paths: Vec<String>,
    pub terms: Vec<String>,
}

impl ParsedSearchQuery {
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
            && self.statuses.is_empty()
            && self.paths.is_empty()
            && self.terms.is_empty()
    }

    pub fn has_filters(&self) -> bool {
        !self.tags.is_empty() || !self.statuses.is_empty() || !self.paths.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnippetPart {
    pub text: String,
    pub highlighted: bool,
}

pub fn normalize_status_filter(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "inbox" | "входящие" => "inbox".to_string(),
        "todo" | "к выполнению" | "сделать" => "todo".to_string(),
        "doing" | "in_progress" | "in-progress" | "inprogress" | "в работе" => {
            "in_progress".to_string()
        }
        "done" | "готово" => "done".to_string(),
        "idea" | "идея" => "idea".to_string(),
        _ => lower,
    }
}

fn tokenize_query(raw: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch.is_whitespace() && !in_quotes {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

pub fn parse_search_query(raw: &str) -> ParsedSearchQuery {
    let mut out = ParsedSearchQuery::default();

    for token in tokenize_query(raw) {
        let lower = token.to_lowercase();
        if let Some(rest) = lower.strip_prefix("tag:") {
            let val = rest.trim().trim_start_matches('#').to_string();
            if !val.is_empty() && !out.tags.contains(&val) {
                out.tags.push(val);
            }
        } else if let Some(rest) = lower.strip_prefix("status:") {
            let val = normalize_status_filter(rest);
            if !val.is_empty() && !out.statuses.contains(&val) {
                out.statuses.push(val);
            }
        } else if let Some(rest) = lower.strip_prefix("path:") {
            let val = rest.trim().replace('\\', "/").trim_matches('/').to_string();
            if !val.is_empty() && !out.paths.contains(&val) {
                out.paths.push(val);
            }
        } else if let Some(rest) = lower.strip_prefix('#') {
            let val = rest.trim().to_string();
            if !val.is_empty() && !out.tags.contains(&val) {
                out.tags.push(val);
            }
        } else {
            let clean = token.trim();
            if !clean.is_empty() {
                out.terms.push(clean.to_string());
            }
        }
    }

    out
}

/// UTF-8 / Cyrillic-safe case-insensitive snippet highlighting.
/// Maps each lowercased char back to the original string's byte range so slicing
/// never splits a multi-byte UTF-8 codepoint even if `to_lowercase()` expands chars.
pub fn highlight_snippet(snippet: &str, terms: &[String]) -> Vec<SnippetPart> {
    if snippet.is_empty() {
        return Vec::new();
    }

    let clean_terms: Vec<String> = terms
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();

    if clean_terms.is_empty() {
        return vec![SnippetPart {
            text: snippet.to_string(),
            highlighted: false,
        }];
    }

    let mut lower_buf = String::with_capacity(snippet.len());
    // For every byte in `lower_buf`, store the original byte range `[start, end)` in `snippet`.
    let mut byte_map: Vec<(usize, usize)> = Vec::with_capacity(snippet.len());

    for (orig_start, ch) in snippet.char_indices() {
        let orig_end = orig_start + ch.len_utf8();
        for lch in ch.to_lowercase() {
            let mut enc = [0u8; 4];
            let s = lch.encode_utf8(&mut enc);
            lower_buf.push_str(s);
            for _ in 0..s.len() {
                byte_map.push((orig_start, orig_end));
            }
        }
    }

    let mut raw_ranges: Vec<Range<usize>> = Vec::new();
    for term in &clean_terms {
        let mut search_from = 0usize;
        while search_from < lower_buf.len() {
            if let Some(rel) = lower_buf[search_from..].find(term.as_str()) {
                let l_start = search_from + rel;
                let l_end = l_start + term.len();
                if l_start < byte_map.len() && l_end > l_start && l_end <= byte_map.len() {
                    let orig_start = byte_map[l_start].0;
                    let orig_end = byte_map[l_end - 1].1;
                    if orig_start < orig_end {
                        raw_ranges.push(orig_start..orig_end);
                    }
                }
                search_from = l_end.max(search_from + 1);
            } else {
                break;
            }
        }
    }

    if raw_ranges.is_empty() {
        return vec![SnippetPart {
            text: snippet.to_string(),
            highlighted: false,
        }];
    }

    raw_ranges.sort_by_key(|r| (r.start, r.end));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(raw_ranges.len());
    for r in raw_ranges {
        if let Some(last) = merged.last_mut() {
            if r.start <= last.end {
                if r.end > last.end {
                    last.end = r.end;
                }
                continue;
            }
        }
        merged.push(r);
    }

    let mut parts = Vec::new();
    let mut cursor = 0usize;
    for r in merged {
        if r.start > cursor {
            parts.push(SnippetPart {
                text: snippet[cursor..r.start].to_string(),
                highlighted: false,
            });
        }
        parts.push(SnippetPart {
            text: snippet[r.start..r.end].to_string(),
            highlighted: true,
        });
        cursor = r.end;
    }
    if cursor < snippet.len() {
        parts.push(SnippetPart {
            text: snippet[cursor..].to_string(),
            highlighted: false,
        });
    }
    parts
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagSuggestion {
    pub tag: String,
    pub count: usize,
    pub match_tier: u8,
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut h_chars = haystack.chars();
    for nc in needle.chars() {
        if !h_chars.any(|hc| hc == nc) {
            return false;
        }
    }
    true
}

fn has_segment_boundary_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut start = 0usize;
    while let Some(rel) = haystack[start..].find(needle) {
        let idx = start + rel;
        if idx > 0 {
            if let Some(prev) = haystack[..idx].chars().next_back() {
                if matches!(prev, '-' | '_' | '/') {
                    return true;
                }
            }
        }
        start = idx + needle.chars().next().map_or(1, char::len_utf8);
    }
    false
}

/// Ranks vault tags against a prefix/query with full Unicode/Cyrillic support.
/// Tiers: 0 = exact, 1 = prefix, 2 = segment-boundary or substring, 3 = subsequence/fuzzy.
pub fn suggest_tags(
    prefix: &str,
    all_tags: &[(String, usize)],
    limit: usize,
) -> Vec<TagSuggestion> {
    if limit == 0 {
        return Vec::new();
    }
    let q = prefix.trim().trim_start_matches('#').to_lowercase();
    if q.is_empty() {
        let mut items: Vec<(String, usize, String)> = all_tags
            .iter()
            .map(|(tag, count)| (tag.clone(), *count, tag.to_lowercase()))
            .collect();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));
        items.truncate(limit);
        return items
            .into_iter()
            .map(|(tag, count, _)| TagSuggestion {
                tag,
                count,
                match_tier: 0,
            })
            .collect();
    }

    let mut ranked: Vec<(u8, bool, usize, String, String)> = Vec::new();
    for (tag, count) in all_tags {
        let t_low = tag.to_lowercase();
        let (tier, boundary_hit) = if t_low == q {
            (0u8, true)
        } else if t_low.starts_with(&q) {
            (1u8, true)
        } else if t_low.contains(&q) {
            (2u8, has_segment_boundary_match(&t_low, &q))
        } else if is_subsequence(&q, &t_low) {
            (3u8, false)
        } else {
            continue;
        };
        ranked.push((tier, !boundary_hit, *count, t_low, tag.clone()));
    }

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| b.2.cmp(&a.2))
            .then_with(|| a.3.cmp(&b.3))
    });
    ranked.truncate(limit);
    ranked
        .into_iter()
        .map(|(match_tier, _, count, _, tag)| TagSuggestion {
            tag,
            count,
            match_tier,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagTokenPrefix {
    TagColon,
    Hash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveTagToken {
    pub byte_start: usize,
    pub byte_end: usize,
    pub prefix: TagTokenPrefix,
    pub query: String,
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub fn find_active_tag_token(input: &str, cursor_char_idx: usize) -> Option<ActiveTagToken> {
    if input.is_empty() {
        return None;
    }
    let cursor_byte = char_to_byte_idx(input, cursor_char_idx);
    let byte_start = input[..cursor_byte]
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(i, ch)| i + ch.len_utf8())
        .unwrap_or(0);
    let byte_end = input[cursor_byte..]
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(i, _)| cursor_byte + i)
        .unwrap_or(input.len());

    let tok = &input[byte_start..byte_end];
    if tok.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("tag:")) {
        Some(ActiveTagToken {
            byte_start,
            byte_end,
            prefix: TagTokenPrefix::TagColon,
            query: tok[4..].trim_start_matches('#').to_string(),
        })
    } else if let Some(rest) = tok.strip_prefix('#') {
        Some(ActiveTagToken {
            byte_start,
            byte_end,
            prefix: TagTokenPrefix::Hash,
            query: rest.to_string(),
        })
    } else {
        None
    }
}

pub fn replace_active_tag_token(
    input: &str,
    tok: &ActiveTagToken,
    chosen_tag: &str,
) -> (String, usize) {
    let prefix = match tok.prefix {
        TagTokenPrefix::TagColon => "tag:",
        TagTokenPrefix::Hash => "#",
    };
    let clean_tag = chosen_tag.trim().trim_start_matches('#');
    let replacement = format!("{prefix}{clean_tag}");
    let after = &input[tok.byte_end..];
    let add_space = after.is_empty();
    let mut out = String::with_capacity(input.len() + replacement.len() + 1);
    out.push_str(&input[..tok.byte_start]);
    out.push_str(&replacement);
    if add_space {
        out.push(' ');
    }
    let cursor_bytes = out.len();
    out.push_str(after);
    let new_cursor_char = out[..cursor_bytes].chars().count();
    (out, new_cursor_char)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_operators_quotes_and_terms() {
        let q = parse_search_query(r#"tag:#Work status:doing path:"proj/a b" дизайн архитектура"#);
        assert_eq!(q.tags, vec!["work"]);
        assert_eq!(q.statuses, vec!["in_progress"]);
        assert_eq!(q.paths, vec!["proj/a b"]);
        assert_eq!(q.terms, vec!["дизайн", "архитектура"]);
        assert!(q.has_filters());
        assert!(!q.is_empty());
    }

    #[test]
    fn handles_empty_operators_and_unclosed_quotes() {
        let q = parse_search_query(r#"tag: status: path:"unclosed folder"#);
        assert!(q.tags.is_empty());
        assert!(q.statuses.is_empty());
        assert_eq!(q.paths, vec!["unclosed folder"]);
        assert!(q.terms.is_empty());
    }

    #[test]
    fn highlights_cyrillic_and_merged_spans_safely() {
        let parts = highlight_snippet(
            "Быстрый Нативный редактор ЗАМЕТОК",
            &["натив".to_string(), "заметок".to_string()],
        );
        assert_eq!(
            parts,
            vec![
                SnippetPart {
                    text: "Быстрый ".into(),
                    highlighted: false,
                },
                SnippetPart {
                    text: "Натив".into(),
                    highlighted: true,
                },
                SnippetPart {
                    text: "ный редактор ".into(),
                    highlighted: false,
                },
                SnippetPart {
                    text: "ЗАМЕТОК".into(),
                    highlighted: true,
                },
            ]
        );
    }

    #[test]
    fn highlights_unicode_case_expansion_without_panic() {
        // 'İ' (U+0130) lowercases to "i\u{307}" (different UTF-8 byte length)
        let parts = highlight_snippet("İstanbul note", &["note".to_string()]);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].text, "İstanbul ");
        assert!(!parts[0].highlighted);
        assert_eq!(parts[1].text, "note");
        assert!(parts[1].highlighted);
    }

    #[test]
    fn parse_search_query_supports_hash_shorthand() {
        let q = parse_search_query("#Кино tag:Работа заметка");
        assert_eq!(q.tags, vec!["кино", "работа"]);
        assert_eq!(q.terms, vec!["заметка"]);
    }

    #[test]
    fn suggest_tags_ranks_cyrillic_latin_empty_boundary_and_fuzzy() {
        let tags = vec![
            ("кинотека".to_string(), 5),
            ("Кино".to_string(), 12),
            ("архив-кино".to_string(), 9),
            ("микино".to_string(), 20),
            ("weekly-review".to_string(), 14),
            ("work".to_string(), 30),
            ("review-weekly".to_string(), 4),
        ];

        // Empty prefix -> sorted by count DESC
        let top = suggest_tags("", &tags, 3);
        assert_eq!(
            top.iter().map(|t| t.tag.as_str()).collect::<Vec<_>>(),
            vec!["work", "микино", "weekly-review"]
        );

        // Cyrillic mixed-case: exact -> prefix -> segment boundary -> substring
        let cyr = suggest_tags("КИНО", &tags, 10);
        assert_eq!(
            cyr.iter().map(|t| (t.tag.as_str(), t.match_tier)).collect::<Vec<_>>(),
            vec![
                ("Кино", 0),
                ("кинотека", 1),
                ("архив-кино", 2),
                ("микино", 2),
            ]
        );

        // Fuzzy subsequence: wr -> weekly-review (work has higher count so check weekly-review ahead of review-weekly)
        let fuzzy = suggest_tags("wrev", &tags, 5);
        assert_eq!(fuzzy[0].tag, "weekly-review");
        assert_eq!(fuzzy[0].match_tier, 3);
        let fuzzy_wr = suggest_tags("wr", &[("weekly-review".to_string(), 14), ("review-weekly".to_string(), 20)], 5);
        assert_eq!(fuzzy_wr.len(), 1);
        assert_eq!(fuzzy_wr[0].tag, "weekly-review");
    }

    #[test]
    fn active_tag_token_detects_and_replaces_preserving_rest_of_query() {
        let input = "tag:кр фикс";
        let tok = find_active_tag_token(input, 6).expect("token at cursor");
        assert_eq!(tok.query, "кр");
        let (replaced, cursor) = replace_active_tag_token(input, &tok, "krono");
        assert_eq!(replaced, "tag:krono фикс");
        assert_eq!(cursor, "tag:krono".chars().count());

        let tail = "задача #нед";
        let tok2 = find_active_tag_token(tail, tail.chars().count()).expect("hash token");
        assert_eq!(tok2.query, "нед");
        let (replaced2, cursor2) = replace_active_tag_token(tail, &tok2, "неделя");
        assert_eq!(replaced2, "задача #неделя ");
        assert_eq!(cursor2, replaced2.chars().count());
    }
}
