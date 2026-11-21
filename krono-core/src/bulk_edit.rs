use crate::collection::CollectionDoc;
use crate::note::normalize_tags;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinePreview {
    pub line_no: usize,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileReplacePreview {
    pub path: String,
    pub match_count: usize,
    pub lines: Vec<LinePreview>,
}

const MAX_LINE_PREVIEWS: usize = 50;

pub fn normalize_tag_name(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('#')
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

pub fn apply_find_replace(
    content: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> (String, usize) {
    if query.is_empty() {
        return (content.to_string(), 0);
    }

    if case_sensitive {
        let mut count = 0usize;
        let mut out = String::with_capacity(content.len());
        let mut cursor = 0usize;
        while let Some(rel) = content[cursor..].find(query) {
            let start = cursor + rel;
            out.push_str(&content[cursor..start]);
            out.push_str(replacement);
            cursor = start + query.len();
            count += 1;
        }
        if count == 0 {
            return (content.to_string(), 0);
        }
        out.push_str(&content[cursor..]);
        return (out, count);
    }

    let query_lower = query.to_lowercase();
    if query_lower.is_empty() {
        return (content.to_string(), 0);
    }

    let mut lower_buf = String::with_capacity(content.len());
    let mut byte_map: Vec<(usize, usize)> = Vec::with_capacity(content.len());
    for (orig_start, ch) in content.char_indices() {
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

    let mut out = String::with_capacity(content.len());
    let mut count = 0usize;
    let mut lower_cursor = 0usize;
    let mut orig_cursor = 0usize;

    while lower_cursor < lower_buf.len() {
        let Some(rel) = lower_buf[lower_cursor..].find(query_lower.as_str()) else {
            break;
        };
        let l_start = lower_cursor + rel;
        let l_end = l_start + query_lower.len();
        if l_start >= byte_map.len() || l_end > byte_map.len() || l_end <= l_start {
            break;
        }
        let orig_start = byte_map[l_start].0;
        let orig_end = byte_map[l_end - 1].1;
        if orig_start < orig_cursor || orig_end <= orig_start {
            lower_cursor = l_start + 1;
            continue;
        }
        out.push_str(&content[orig_cursor..orig_start]);
        out.push_str(replacement);
        orig_cursor = orig_end;
        lower_cursor = l_end;
        count += 1;
    }

    if count == 0 {
        return (content.to_string(), 0);
    }
    out.push_str(&content[orig_cursor..]);
    (out, count)
}

pub fn preview_find_replace(
    path: &str,
    content: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
) -> Option<FileReplacePreview> {
    if query.is_empty() {
        return None;
    }
    let mut match_count = 0usize;
    let mut lines = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let (after, count) = apply_find_replace(line, query, replacement, case_sensitive);
        if count > 0 {
            match_count += count;
            if lines.len() < MAX_LINE_PREVIEWS {
                lines.push(LinePreview {
                    line_no: idx + 1,
                    before: line.to_string(),
                    after,
                });
            }
        }
    }
    if match_count == 0 {
        None
    } else {
        Some(FileReplacePreview {
            path: path.to_string(),
            match_count,
            lines,
        })
    }
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-' || c == '/'
}

fn is_valid_tag_prefix_prev(prev: Option<char>) -> bool {
    match prev {
        None => true,
        Some(c) => !c.is_alphanumeric() && c != '_' && c != '/' && c != '&',
    }
}

fn replace_inline_tags_outside_code(line: &str, old_lower: &str, new_tag: &str) -> (String, usize) {
    let mut out = String::with_capacity(line.len());
    let mut count = 0usize;
    let mut in_inline_code = false;
    let mut chars = line.char_indices().peekable();
    let mut prev_char: Option<char> = None;

    while let Some((idx, ch)) = chars.next() {
        if ch == '`' {
            in_inline_code = !in_inline_code;
            out.push(ch);
            prev_char = Some(ch);
            continue;
        }
        if !in_inline_code && ch == '#' && is_valid_tag_prefix_prev(prev_char) {
            let rest = &line[idx + 1..];
            let mut end_rel = 0usize;
            for (r_idx, r_ch) in rest.char_indices() {
                if is_tag_char(r_ch) {
                    end_rel = r_idx + r_ch.len_utf8();
                } else {
                    break;
                }
            }
            if end_rel > 0 {
                let tag_slice = &rest[..end_rel];
                if tag_slice.to_lowercase() == old_lower {
                    out.push('#');
                    out.push_str(new_tag);
                    count += 1;
                    // Advance iterator past tag_slice
                    let target_byte = idx + 1 + end_rel;
                    while let Some(&(next_idx, _)) = chars.peek() {
                        if next_idx < target_byte {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    prev_char = tag_slice.chars().last();
                    continue;
                }
            }
        }
        out.push(ch);
        prev_char = Some(ch);
    }

    (out, count)
}

fn collect_frontmatter_existing_tags(fm_lines: &[&str]) -> HashSet<String> {
    let mut tags = HashSet::new();
    let mut in_tags_list = false;
    for line in fm_lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("tags:") {
            in_tags_list = true;
            let rest = rest.trim().trim_start_matches('[').trim_end_matches(']');
            for item in rest.split(',') {
                let norm = normalize_tag_name(item.trim().trim_matches('"').trim_matches('\''));
                if !norm.is_empty() {
                    tags.insert(norm.to_lowercase());
                }
            }
            if !rest.is_empty() {
                in_tags_list = false;
            }
            continue;
        }
        if in_tags_list {
            if let Some(item) = trimmed.strip_prefix("- ") {
                let norm = normalize_tag_name(item.trim().trim_matches('"').trim_matches('\''));
                if !norm.is_empty() {
                    tags.insert(norm.to_lowercase());
                }
            } else if !trimmed.is_empty() {
                in_tags_list = false;
            }
        }
    }
    tags
}

pub fn rename_tag_in_markdown(content: &str, old_tag: &str, new_tag: &str) -> (String, usize) {
    let old_norm = normalize_tag_name(old_tag);
    let new_norm = normalize_tag_name(new_tag);
    if old_norm.is_empty() || new_norm.is_empty() || old_norm.eq_ignore_ascii_case(&new_norm) {
        return (content.to_string(), 0);
    }

    let old_lower = old_norm.to_lowercase();
    let new_lower = new_norm.to_lowercase();
    let has_trailing_nl = content.ends_with('\n');
    let lines: Vec<&str> = content.split('\n').collect();
    let mut out_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut total_changes = 0usize;

    // Detect YAML frontmatter starting at line 0
    let fm_end = if lines.first().map(|l| l.trim()) == Some("---") {
        lines
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, l)| l.trim() == "---")
            .map(|(i, _)| i)
    } else {
        None
    };

    let mut body_start = 0usize;
    if let Some(end_idx) = fm_end {
        let fm_slice = &lines[1..end_idx];
        let existing_fm_tags = collect_frontmatter_existing_tags(fm_slice);
        let new_already_in_fm = existing_fm_tags.contains(&new_lower);

        out_lines.push(lines[0].to_string());
        let mut in_tags_list = false;
        for &line in fm_slice {
            let trimmed = line.trim();
            if let Some((prefix, rest)) = line.split_once("tags:") {
                if prefix.trim().is_empty() {
                    let rest_trim = rest.trim();
                    if rest_trim.is_empty() {
                        in_tags_list = true;
                        out_lines.push(line.to_string());
                    } else {
                        in_tags_list = false;
                        let bracketed = rest_trim.starts_with('[') && rest_trim.ends_with(']');
                        let inner = rest_trim.trim_start_matches('[').trim_end_matches(']');
                        let mut changed = false;
                        let mut seen = HashSet::new();
                        let mut items = Vec::new();
                        for part in inner.split(',') {
                            let raw_item = part.trim().trim_matches('"').trim_matches('\'');
                            let norm = normalize_tag_name(raw_item);
                            if norm.is_empty() {
                                continue;
                            }
                            let final_tag = if norm.to_lowercase() == old_lower {
                                changed = true;
                                total_changes += 1;
                                new_norm.clone()
                            } else {
                                norm
                            };
                            if seen.insert(final_tag.to_lowercase()) {
                                items.push(final_tag);
                            }
                        }
                        if changed {
                            if bracketed {
                                out_lines.push(format!("{prefix}tags: [{}]", items.join(", ")));
                            } else {
                                out_lines.push(format!("{prefix}tags: {}", items.join(", ")));
                            }
                        } else {
                            out_lines.push(line.to_string());
                        }
                    }
                    continue;
                }
            }

            if in_tags_list {
                if let Some((indent, item_part)) = line.split_once("- ") {
                    if indent.trim().is_empty() {
                        let raw_item = item_part.trim().trim_matches('"').trim_matches('\'');
                        let norm = normalize_tag_name(raw_item);
                        if norm.to_lowercase() == old_lower {
                            total_changes += 1;
                            if !new_already_in_fm {
                                out_lines.push(format!("{indent}- {new_norm}"));
                            }
                            // If new tag already exists in frontmatter, drop duplicate item.
                            continue;
                        }
                        out_lines.push(line.to_string());
                        continue;
                    }
                }
                if !trimmed.is_empty() {
                    in_tags_list = false;
                }
            }
            out_lines.push(line.to_string());
        }
        out_lines.push(lines[end_idx].to_string());
        body_start = end_idx + 1;
    }

    let mut in_fence = false;
    let mut fence_marker = "";
    for &line in &lines[body_start..] {
        let trimmed = line.trim_start();
        if !in_fence {
            if trimmed.starts_with("```") {
                in_fence = true;
                fence_marker = "```";
                out_lines.push(line.to_string());
                continue;
            } else if trimmed.starts_with("~~~") {
                in_fence = true;
                fence_marker = "~~~";
                out_lines.push(line.to_string());
                continue;
            }
        } else if trimmed.starts_with(fence_marker) {
            in_fence = false;
            out_lines.push(line.to_string());
            continue;
        }

        if in_fence {
            out_lines.push(line.to_string());
            continue;
        }

        let (replaced, c) = replace_inline_tags_outside_code(line, &old_lower, &new_norm);
        total_changes += c;
        out_lines.push(replaced);
    }

    if total_changes == 0 {
        return (content.to_string(), 0);
    }

    let mut joined = out_lines.join("\n");
    if has_trailing_nl && !joined.ends_with('\n') {
        joined.push('\n');
    }
    (joined, total_changes)
}

pub fn preview_tag_rename_markdown(
    path: &str,
    content: &str,
    old_tag: &str,
    new_tag: &str,
) -> Option<FileReplacePreview> {
    let (updated, count) = rename_tag_in_markdown(content, old_tag, new_tag);
    if count == 0 {
        return None;
    }
    let mut lines = Vec::new();
    for (idx, (b, a)) in content.lines().zip(updated.lines()).enumerate() {
        if b != a {
            lines.push(LinePreview {
                line_no: idx + 1,
                before: b.to_string(),
                after: a.to_string(),
            });
            if lines.len() >= MAX_LINE_PREVIEWS {
                break;
            }
        }
    }
    Some(FileReplacePreview {
        path: path.to_string(),
        match_count: count,
        lines,
    })
}

pub fn rename_tag_in_collection(doc: &mut CollectionDoc, old_tag: &str, new_tag: &str) -> usize {
    let old_norm = normalize_tag_name(old_tag);
    let new_norm = normalize_tag_name(new_tag);
    if old_norm.is_empty() || new_norm.is_empty() || old_norm.eq_ignore_ascii_case(&new_norm) {
        return 0;
    }
    let old_lower = old_norm.to_lowercase();
    let mut changed = 0usize;
    for rec in &mut doc.records {
        let mut rec_changed = false;
        for tag in &mut rec.tags {
            if tag.trim().trim_start_matches('#').to_lowercase() == old_lower {
                *tag = new_norm.clone();
                rec_changed = true;
                changed += 1;
            }
        }
        if rec_changed {
            rec.tags = normalize_tags(std::mem::take(&mut rec.tags));
        }
    }
    changed
}

pub fn preview_tag_rename_collection(
    path: &str,
    doc: &CollectionDoc,
    old_tag: &str,
    new_tag: &str,
) -> Option<FileReplacePreview> {
    let old_norm = normalize_tag_name(old_tag);
    let new_norm = normalize_tag_name(new_tag);
    if old_norm.is_empty() || new_norm.is_empty() || old_norm.eq_ignore_ascii_case(&new_norm) {
        return None;
    }
    let old_lower = old_norm.to_lowercase();
    let mut count = 0usize;
    let mut lines = Vec::new();
    for (idx, rec) in doc.records.iter().enumerate() {
        let hits = rec
            .tags
            .iter()
            .filter(|t| t.trim().trim_start_matches('#').to_lowercase() == old_lower)
            .count();
        if hits > 0 {
            count += hits;
            if lines.len() < MAX_LINE_PREVIEWS {
                let mut after_tags = rec.tags.clone();
                for t in &mut after_tags {
                    if t.trim().trim_start_matches('#').to_lowercase() == old_lower {
                        *t = new_norm.clone();
                    }
                }
                after_tags = normalize_tags(after_tags);
                lines.push(LinePreview {
                    line_no: idx + 1,
                    before: format!("{}: [{}]", rec.title, rec.tags.join(", ")),
                    after: format!("{}: [{}]", rec.title, after_tags.join(", ")),
                });
            }
        }
    }
    if count == 0 {
        None
    } else {
        Some(FileReplacePreview {
            path: path.to_string(),
            match_count: count,
            lines,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::RecordItem;

    #[test]
    fn find_replace_case_insensitive_cyrillic_and_utf8_safe() {
        let src = "Привет Мир, тихой заметки! İstanbul мир";
        let (out, count) = apply_find_replace(src, "мир", "свет", false);
        assert_eq!(count, 2);
        assert_eq!(out, "Привет свет, тихой заметки! İstanbul свет");

        let (out2, count2) = apply_find_replace("Мир и мирный путь", "мир", "дом", false);
        assert_eq!(count2, 2);
        assert_eq!(out2, "дом и домный путь");
    }

    #[test]
    fn find_replace_preview_tracks_lines_and_counts() {
        let prev = preview_find_replace("a.md", "one\ntwo one\nthree", "one", "1", true).unwrap();
        assert_eq!(prev.match_count, 2);
        assert_eq!(prev.lines.len(), 2);
        assert_eq!(prev.lines[0].line_no, 1);
        assert_eq!(prev.lines[1].after, "two 1");
    }

    #[test]
    fn tag_rename_respects_boundaries_frontmatter_and_code_blocks() {
        let md = "\
---
title: Test
tags: [work, extra]
---

План #work и #workout, а также `inline #work` и `#work`.

```rust
let s = \"#work\";
```

Ещё одна строка #Work.";

        let (updated, count) = rename_tag_in_markdown(md, "#work", "job");
        assert_eq!(count, 3);
        assert!(updated.contains("tags: [job, extra]"));
        assert!(updated.contains("План #job и #workout"));
        assert!(updated.contains("`inline #work`"));
        assert!(updated.contains("let s = \"#work\";"));
        assert!(updated.contains("Ещё одна строка #job."));
    }

    #[test]
    fn tag_rename_deduplicates_multiline_frontmatter() {
        let md = "\
---
title: Dup
tags:
  - work
  - job
---

Body #work";
        let (updated, count) = rename_tag_in_markdown(md, "work", "job");
        assert_eq!(count, 2);
        assert!(!updated.contains("  - work"));
        assert_eq!(updated.matches("  - job").count(), 1);
        assert!(updated.contains("Body #job"));
    }

    #[test]
    fn tag_rename_in_collection_updates_and_dedups() {
        let mut doc = CollectionDoc::default();
        doc.records.push(RecordItem {
            id: "1".into(),
            title: "Task".into(),
            tags: vec!["work".into(), "job".into()],
            ..Default::default()
        });
        let changed = rename_tag_in_collection(&mut doc, "work", "job");
        assert_eq!(changed, 1);
        assert_eq!(doc.records[0].tags, vec!["job"]);
    }
}
