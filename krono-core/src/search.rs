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
}
