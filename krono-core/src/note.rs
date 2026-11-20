use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoteStatus {
    Todo,
    InProgress,
    Done,
}

impl Default for NoteStatus {
    fn default() -> Self {
        Self::Todo
    }
}

impl NoteStatus {
    pub const ALL: [NoteStatus; 3] = [NoteStatus::Todo, NoteStatus::InProgress, NoteStatus::Done];

    pub fn as_yaml_str(self) -> &'static str {
        match self {
            NoteStatus::Todo => "Todo",
            NoteStatus::InProgress => "In Progress",
            NoteStatus::Done => "Done",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        let norm = raw.trim().to_lowercase();
        match norm.as_str() {
            "in progress" | "in_progress" | "in-progress" | "progress" | "в работе" => {
                NoteStatus::InProgress
            }
            "done" | "completed" | "finished" | "готово" => NoteStatus::Done,
            _ => NoteStatus::Todo,
        }
    }

    pub fn sort_rank(self) -> u8 {
        match self {
            NoteStatus::Todo => 0,
            NoteStatus::InProgress => 1,
            NoteStatus::Done => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frontmatter {
    pub title: String,
    pub status: NoteStatus,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub board_order: Option<i64>,
    pub extra_lines: Vec<(String, String)>,
}

impl Frontmatter {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: NoteStatus::Todo,
            tags: Vec::new(),
            pinned: false,
            board_order: None,
            extra_lines: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteDocument {
    pub frontmatter: Frontmatter,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteMeta {
    pub path: String,
    pub title: String,
    pub status: NoteStatus,
    pub tags: Vec<String>,
    pub excerpt: String,
    pub modified: i64,
    pub word_count: usize,
    pub pinned: bool,
    pub board_order: i64,
}

fn strip_quotes(s: &str) -> &str {
    let trimmed = s.trim();
    if trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')))
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

/// Parses comma-separated or bracketed `[a, b]` YAML tag values into clean deduplicated tags.
pub fn parse_tags_value(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2 {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    };

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for part in inner.split(',') {
        let tag = strip_quotes(part).trim().trim_start_matches('#').trim();
        if !tag.is_empty() {
            let key = tag.to_lowercase();
            if seen.insert(key) {
                out.push(tag.to_string());
            }
        }
    }
    out
}

pub fn normalize_tags(tags: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for raw in tags {
        let tag = raw.trim().trim_start_matches('#').trim();
        if !tag.is_empty() {
            let key = tag.to_lowercase();
            if seen.insert(key) {
                out.push(tag.to_string());
            }
        }
    }
    out
}

fn yaml_scalar(val: &str) -> String {
    let trimmed = val.trim();
    if trimmed.contains('\n') || trimmed.contains('\r') {
        trimmed.replace(['\r', '\n'], " ")
    } else {
        trimmed.to_string()
    }
}

impl NoteDocument {
    /// Parses YAML frontmatter + Markdown body while normalizing CRLF -> LF
    /// and preserving all unknown frontmatter keys losslessly.
    pub fn parse(raw: &str, fallback_title: &str) -> Self {
        let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
        let mut fm = Frontmatter::new(fallback_title);
        let mut body = normalized.as_str();

        if let Some(after_open) = normalized.strip_prefix("---\n") {
            let mut close_offset = None;
            let mut offset = 0usize;
            for segment in after_open.split_inclusive('\n') {
                let line = segment.trim_end_matches('\n');
                if line.trim() == "---" {
                    close_offset = Some((offset, offset + segment.len()));
                    break;
                }
                offset += segment.len();
            }
            if close_offset.is_none() && after_open[offset..].trim() == "---" {
                close_offset = Some((offset, after_open.len()));
            }

            if let Some((yaml_end, body_start)) = close_offset {
                let yaml_block = &after_open[..yaml_end];
                body = after_open[body_start..].trim_start_matches('\n');

                let mut in_tags_list = false;
                for raw_line in yaml_block.lines() {
                    let trimmed = raw_line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if in_tags_list {
                        if let Some(item) = trimmed.strip_prefix("- ") {
                            let tag = strip_quotes(item).trim().trim_start_matches('#').trim();
                            if !tag.is_empty()
                                && !fm.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
                            {
                                fm.tags.push(tag.to_string());
                            }
                            continue;
                        } else {
                            in_tags_list = false;
                        }
                    }

                    if let Some((k, v)) = raw_line.split_once(':') {
                        let key_raw = k.trim();
                        let key_lower = key_raw.to_lowercase();
                        let val_raw = v.trim();
                        match key_lower.as_str() {
                            "title" => {
                                let t = strip_quotes(val_raw);
                                if !t.is_empty() {
                                    fm.title = t.to_string();
                                }
                            }
                            "status" => {
                                if !val_raw.is_empty() {
                                    fm.status = NoteStatus::from_str_lossy(strip_quotes(val_raw));
                                }
                            }
                            "tags" => {
                                if val_raw.is_empty() {
                                    in_tags_list = true;
                                } else {
                                    fm.tags = parse_tags_value(val_raw);
                                }
                            }
                            "pinned" => {
                                let v = strip_quotes(val_raw).trim().to_lowercase();
                                fm.pinned = matches!(v.as_str(), "true" | "1" | "yes");
                            }
                            "order" | "board_order" => {
                                if let Ok(ord) = strip_quotes(val_raw).trim().parse::<i64>() {
                                    fm.board_order = Some(ord);
                                }
                            }
                            _ => {
                                fm.extra_lines
                                    .push((key_raw.to_string(), val_raw.to_string()));
                            }
                        }
                    }
                }
            }
        }

        Self {
            frontmatter: fm,
            body: body.to_string(),
        }
    }

    pub fn serialize(&self) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("title: {}\n", yaml_scalar(&self.frontmatter.title)));
        out.push_str(&format!("status: {}\n", self.frontmatter.status.as_yaml_str()));
        out.push_str(&format!("tags: {}\n", self.frontmatter.tags.join(", ")));
        if self.frontmatter.pinned {
            out.push_str("pinned: true\n");
        }
        if let Some(ord) = self.frontmatter.board_order {
            out.push_str(&format!("order: {ord}\n"));
        }
        for (k, v) in &self.frontmatter.extra_lines {
            let key = k.trim();
            if !key.is_empty() {
                if v.is_empty() {
                    out.push_str(&format!("{key}:\n"));
                } else {
                    out.push_str(&format!("{key}: {}\n", yaml_scalar(v)));
                }
            }
        }
        out.push_str("---\n\n");
        let body_lf = self.body.replace("\r\n", "\n").replace('\r', "\n");
        out.push_str(&body_lf);
        if !body_lf.is_empty() && !body_lf.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lossless_frontmatter_roundtrip_and_crlf() {
        let raw = "---\r\ntitle: \"Roadmap 2026\"\r\nstatus: In Progress\r\ntags: [rust, ui, Rust]\r\nauthor: Alex\r\ncreated: 2026-01-10\r\n---\r\n\r\nHello **world**\r\nLine 2";
        let doc = NoteDocument::parse(raw, "fallback");
        assert_eq!(doc.frontmatter.title, "Roadmap 2026");
        assert_eq!(doc.frontmatter.status, NoteStatus::InProgress);
        assert_eq!(doc.frontmatter.tags, vec!["rust", "ui"]);
        assert_eq!(
            doc.frontmatter.extra_lines,
            vec![
                ("author".to_string(), "Alex".to_string()),
                ("created".to_string(), "2026-01-10".to_string()),
            ]
        );
        assert_eq!(doc.body, "Hello **world**\nLine 2");

        let serialized = doc.serialize();
        assert!(!serialized.contains('\r'));
        let reparsed = NoteDocument::parse(&serialized, "fallback");
        assert_eq!(reparsed.frontmatter, doc.frontmatter);
        assert_eq!(reparsed.body.trim_end(), doc.body.trim_end());
    }

    #[test]
    fn parses_yaml_multiline_tag_list_and_missing_frontmatter() {
        let raw = "---\ntitle: List Tags\ntags:\n  - alpha\n  - beta\n---\nBody text";
        let doc = NoteDocument::parse(raw, "fallback");
        assert_eq!(doc.frontmatter.tags, vec!["alpha", "beta"]);
        assert_eq!(doc.body, "Body text");

        let plain = "# Heading\nJust markdown.";
        let doc2 = NoteDocument::parse(plain, "My Fallback");
        assert_eq!(doc2.frontmatter.title, "My Fallback");
        assert_eq!(doc2.frontmatter.status, NoteStatus::Todo);
        assert_eq!(doc2.body, plain);
    }

    #[test]
    fn pinned_frontmatter_roundtrip() {
        let raw = "---\ntitle: Pinned Note\nstatus: Todo\ntags: daily\npinned: true\n---\nHello";
        let doc = NoteDocument::parse(raw, "fallback");
        assert!(doc.frontmatter.pinned);
        assert!(doc.frontmatter.extra_lines.is_empty());
        let out = doc.serialize();
        assert!(out.contains("pinned: true\n"));
        let reparsed = NoteDocument::parse(&out, "fallback");
        assert!(reparsed.frontmatter.pinned);

        let mut unpinned = doc;
        unpinned.frontmatter.pinned = false;
        assert!(!unpinned.serialize().contains("pinned:"));
    }

    #[test]
    fn board_order_frontmatter_roundtrip() {
        let raw = "---\ntitle: Board Task\nstatus: Todo\ntags: work\norder: 30\n---\nDetails";
        let doc = NoteDocument::parse(raw, "fallback");
        assert_eq!(doc.frontmatter.board_order, Some(30));
        assert!(doc.frontmatter.extra_lines.is_empty());
        let out = doc.serialize();
        assert!(out.contains("order: 30\n"));
        let reparsed = NoteDocument::parse(&out, "fallback");
        assert_eq!(reparsed.frontmatter.board_order, Some(30));

        let mut cleared = doc;
        cleared.frontmatter.board_order = None;
        assert!(!cleared.serialize().contains("order:"));
    }
}
