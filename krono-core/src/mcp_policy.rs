use std::collections::HashSet;

use crate::path::VaultFileKind;

/// Domain-level access policy for the local `stdio` MCP server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct McpPolicy {
    allowed_dirs: Vec<String>,
    allow_all: bool,
    allow_write: bool,
}

impl McpPolicy {
    /// Builds a validated MCP access policy.
    /// When `raw_allowed_dirs` is empty and `allow_all` is `false`, the policy is fail-closed.
    pub fn new(
        raw_allowed_dirs: &[String],
        allow_all: bool,
        allow_write: bool,
    ) -> Result<Self, String> {
        let mut allowed_dirs = Vec::new();
        let mut seen = HashSet::new();
        for raw in raw_allowed_dirs {
            let norm = normalize_rel_dir(raw)?;
            if seen.insert(norm.clone()) {
                allowed_dirs.push(norm);
            }
        }
        Ok(Self {
            allowed_dirs,
            allow_all,
            allow_write,
        })
    }

    pub fn allowed_dirs(&self) -> &[String] {
        &self.allowed_dirs
    }

    pub fn allow_all(&self) -> bool {
        self.allow_all
    }

    pub fn allow_write(&self) -> bool {
        self.allow_write
    }

    pub fn is_fail_closed(&self) -> bool {
        !self.allow_all && self.allowed_dirs.is_empty()
    }

    /// Validates a folder argument against the allowed directory policy.
    pub fn is_rel_dir_allowed(&self, raw_dir: &str) -> Result<String, String> {
        let norm = normalize_rel_dir(raw_dir)?;
        if self.is_fail_closed() {
            return Err("access denied: server is fail-closed (pass --allow-dir or --allow-all)".to_string());
        }
        if self.allow_all {
            return Ok(norm);
        }
        let allowed = self
            .allowed_dirs
            .iter()
            .any(|dir| norm == *dir || path_in_dir(&norm, dir));
        if allowed {
            Ok(norm)
        } else {
            Err(format!("directory '{norm}' is outside --allow-dir"))
        }
    }

    /// Returns `true` iff `rel_path` is a valid non-encrypted `.md` note inside allowed dirs.
    pub fn is_rel_path_allowed(&self, rel_path: &str) -> bool {
        let Ok(normalized) = validate_mcp_note_path(rel_path) else {
            return false;
        };
        if self.is_fail_closed() {
            return false;
        }
        if self.allow_all {
            return true;
        }
        self.allowed_dirs
            .iter()
            .any(|dir| path_in_dir(&normalized, dir))
    }

    pub fn check_read_path(&self, raw_path: &str) -> Result<String, String> {
        let normalized = validate_mcp_note_path(raw_path)?;
        if self.is_fail_closed() {
            return Err("access denied: no --allow-dir or --allow-all configured".to_string());
        }
        if !self.allow_all
            && !self
                .allowed_dirs
                .iter()
                .any(|dir| path_in_dir(&normalized, dir))
        {
            return Err(format!("path '{normalized}' is outside --allow-dir"));
        }
        Ok(normalized)
    }

    pub fn check_write_path(&self, raw_path: &str) -> Result<String, String> {
        if !self.allow_write {
            return Err("write access disabled; start server with --allow-write".to_string());
        }
        self.check_read_path(raw_path)
    }
}

fn path_in_dir(rel_path: &str, dir: &str) -> bool {
    rel_path.starts_with(dir) && rel_path.as_bytes().get(dir.len()) == Some(&b'/')
}

/// Normalizes a relative directory path, rejecting traversal, absolute paths, drive letters, and hidden segments.
pub fn normalize_rel_dir(raw: &str) -> Result<String, String> {
    let replaced = raw.trim().replace('\\', "/");
    let trimmed = replaced.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("empty directory path".to_string());
    }
    if trimmed.starts_with('/') || trimmed.contains('\0') {
        return Err(format!("invalid leading slash or null byte: {raw}"));
    }
    if trimmed.as_bytes().get(1) == Some(&b':') {
        return Err(format!("absolute drive path not allowed: {raw}"));
    }

    let mut segments = Vec::new();
    for seg in trimmed.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') {
            return Err(format!("disallowed directory segment '{seg}' in '{raw}'"));
        }
        segments.push(seg);
    }
    Ok(segments.join("/"))
}

/// Validates a vault-relative note path for MCP access.
/// Rejects traversal, hidden dirs (`.krono`, `.git`), encrypted notes (`.md.enc`), and non-markdown files.
pub fn validate_mcp_note_path(raw: &str) -> Result<String, String> {
    let (normalized, kind) = crate::path::validate_vault_path(raw)?;
    match kind {
        VaultFileKind::Markdown => Ok(normalized),
        VaultFileKind::EncryptedMarkdown => {
            Err(format!("encrypted notes (.md.enc) are forbidden via MCP: {raw}"))
        }
        _ => Err(format!("only .md notes are accessible via MCP: {raw}")),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadSlice {
    pub content: String,
    pub total_lines: usize,
    pub returned_lines: usize,
    pub truncated: bool,
}

const DEFAULT_READ_MAX_LINES: usize = 400;
const HARD_READ_MAX_LINES: usize = 2000;

/// Slices Markdown text by line and byte limits on UTF-8 boundaries.
pub fn slice_markdown_for_read(
    raw: &str,
    max_lines: Option<usize>,
    max_bytes: usize,
) -> ReadSlice {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.is_empty() {
        return ReadSlice {
            content: String::new(),
            total_lines: 0,
            returned_lines: 0,
            truncated: false,
        };
    }

    let limit_lines = max_lines
        .unwrap_or(DEFAULT_READ_MAX_LINES)
        .clamp(1, HARD_READ_MAX_LINES);
    let byte_cap = max_bytes.max(1);

    let all_lines: Vec<&str> = normalized.lines().collect();
    let total_lines = all_lines.len();
    let mut out = String::new();
    let mut returned_lines = 0usize;
    let mut truncated = false;

    for (idx, line) in all_lines.iter().enumerate() {
        if idx >= limit_lines {
            truncated = true;
            break;
        }
        let sep = usize::from(!out.is_empty());
        if out.len() + sep + line.len() > byte_cap {
            if out.is_empty() {
                let mut end = byte_cap.min(line.len());
                while end > 0 && !line.is_char_boundary(end) {
                    end -= 1;
                }
                out.push_str(&line[..end]);
                returned_lines = 1;
            }
            truncated = true;
            break;
        }
        if sep == 1 {
            out.push('\n');
        }
        out.push_str(line);
        returned_lines += 1;
    }

    ReadSlice {
        content: out,
        total_lines,
        returned_lines,
        truncated,
    }
}

fn parse_atx_heading(line: &str) -> Option<(usize, &str)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    if title.is_empty() {
        None
    } else {
        Some((hashes, title))
    }
}

fn normalize_target_heading(raw: &str) -> Option<(usize, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes > 0 && trimmed[hashes..].starts_with(char::is_whitespace) {
        let title = trimmed[hashes..].trim().trim_end_matches('#').trim();
        if title.is_empty() {
            None
        } else {
            Some((hashes.clamp(1, 6), title.to_string()))
        }
    } else {
        let title = trimmed.trim_start_matches('#').trim();
        if title.is_empty() {
            None
        } else {
            Some((2, title.to_string()))
        }
    }
}

/// Appends text either at EOF or under the specified Markdown ATX heading (`## ...`), using LF.
pub fn append_to_markdown(existing: &str, addition: &str, heading: Option<&str>) -> String {
    let base = existing.replace("\r\n", "\n").replace('\r', "\n");
    let add = addition
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_matches('\n')
        .to_string();

    let target = heading.and_then(normalize_target_heading);
    let Some((req_level, req_title)) = target else {
        let mut out = base;
        if add.is_empty() {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            return out;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&add);
        out.push('\n');
        return out;
    };

    let mut lines: Vec<String> = if base.is_empty() {
        Vec::new()
    } else {
        base.lines().map(str::to_string).collect()
    };

    let matched = lines.iter().enumerate().find_map(|(idx, line)| {
        let (lvl, title) = parse_atx_heading(line)?;
        if title.eq_ignore_ascii_case(&req_title)
            || title.to_lowercase() == req_title.to_lowercase()
        {
            Some((idx, lvl))
        } else {
            None
        }
    });

    if let Some((start_idx, section_lvl)) = matched {
        let mut insert_idx = lines.len();
        for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
            if let Some((lvl, _)) = parse_atx_heading(line) {
                if lvl <= section_lvl {
                    insert_idx = idx;
                    break;
                }
            }
        }
        while insert_idx > start_idx + 1 && lines[insert_idx - 1].trim().is_empty() {
            insert_idx -= 1;
        }
        let mut new_lines: Vec<String> = Vec::new();
        if !add.is_empty() {
            new_lines.extend(add.lines().map(str::to_string));
        }
        let needs_blank_after = insert_idx < lines.len()
            && !new_lines.is_empty()
            && !lines[insert_idx].trim().is_empty();
        if needs_blank_after {
            new_lines.push(String::new());
        }
        lines.splice(insert_idx..insert_idx, new_lines);
        let mut out = lines.join("\n");
        if !out.is_empty() {
            out.push('\n');
        }
        return out;
    }

    let mut out = base;
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
    }
    out.push_str(&"#".repeat(req_level));
    out.push(' ');
    out.push_str(&req_title);
    out.push('\n');
    if !add.is_empty() {
        out.push_str(&add);
        out.push('\n');
    }
    out
}

/// Extracts a compact UTF-8/Cyrillic-safe snippet around the first matching search term.
pub fn extract_search_snippet(body: &str, terms: &[String], max_chars: usize) -> String {
    let compact: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() || max_chars == 0 {
        return String::new();
    }

    let chars: Vec<char> = compact.chars().collect();
    if chars.len() <= max_chars {
        return compact;
    }

    let mut match_char_idx: Option<usize> = None;
    let lower_compact = compact.to_lowercase();
    for term in terms {
        let t = term.trim().to_lowercase();
        if t.is_empty() {
            continue;
        }
        if let Some(byte_pos) = lower_compact.find(&t) {
            let char_pos = lower_compact[..byte_pos].chars().count();
            match_char_idx = Some(match_char_idx.map_or(char_pos, |prev| prev.min(char_pos)));
        }
    }

    let start = match_char_idx
        .map(|idx| idx.saturating_sub(max_chars / 3))
        .unwrap_or(0)
        .min(chars.len().saturating_sub(max_chars));
    let end = (start + max_chars).min(chars.len());

    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.extend(&chars[start..end]);
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_closed_by_default_and_prefix_collision_blocked() {
        let closed = McpPolicy::new(&[], false, false).unwrap();
        assert!(closed.is_fail_closed());
        assert!(!closed.is_rel_path_allowed("notes/a.md"));
        assert!(closed.check_read_path("notes/a.md").is_err());

        let policy = McpPolicy::new(&["notes/".to_string(), "work\\sub".to_string()], false, false).unwrap();
        assert_eq!(policy.allowed_dirs(), &["notes", "work/sub"]);
        assert!(policy.is_rel_path_allowed("notes/a.md"));
        assert!(policy.is_rel_path_allowed("work/sub/plan.md"));
        assert!(!policy.is_rel_path_allowed("notes-secret/a.md"));
        assert!(!policy.is_rel_path_allowed("other/a.md"));
    }

    #[test]
    fn rejects_traversal_hidden_and_encrypted_notes() {
        let policy = McpPolicy::new(&[], true, true).unwrap();
        for bad in [
            "../secret.md",
            "notes/../../secret.md",
            "/etc/passwd.md",
            "C:\\vault\\a.md",
            "notes/secret.md.enc",
            ".krono/index.md",
            ".git/HEAD.md",
            "notes/.hidden/a.md",
            "notes/board.board.json",
        ] {
            assert!(!policy.is_rel_path_allowed(bad), "expected reject: {bad}");
            assert!(policy.check_read_path(bad).is_err(), "expected err: {bad}");
        }
        assert!(policy.check_read_path("notes/ok.md").is_ok());
    }

    #[test]
    fn read_only_blocks_write() {
        let ro = McpPolicy::new(&["notes".to_string()], false, false).unwrap();
        assert!(ro.check_read_path("notes/a.md").is_ok());
        assert!(ro.check_write_path("notes/a.md").is_err());

        let rw = McpPolicy::new(&["notes".to_string()], false, true).unwrap();
        assert!(rw.check_write_path("notes/a.md").is_ok());
    }

    #[test]
    fn slices_lines_and_cyrillic_bytes_safely() {
        let text = "Строка 1\nСтрока 2\nСтрока 3";
        let by_lines = slice_markdown_for_read(text, Some(2), 4096);
        assert_eq!(by_lines.content, "Строка 1\nСтрока 2");
        assert_eq!(by_lines.total_lines, 3);
        assert_eq!(by_lines.returned_lines, 2);
        assert!(by_lines.truncated);

        let by_bytes = slice_markdown_for_read("Привет мир", Some(10), 7);
        assert_eq!(by_bytes.content, "При");
        assert!(by_bytes.truncated);
    }

    #[test]
    fn appends_eof_and_under_heading() {
        assert_eq!(append_to_markdown("Hello", "World", None), "Hello\nWorld\n");

        let doc = "# Note\n\n## Задачи\n- [ ] Первая\n### Детали\nТекст\n\n## Итоги\nКонец\n";
        let updated = append_to_markdown(doc, "- [ ] Вторая", Some("## задачи"));
        assert_eq!(
            updated,
            "# Note\n\n## Задачи\n- [ ] Первая\n### Детали\nТекст\n- [ ] Вторая\n\n## Итоги\nКонец\n"
        );

        let created = append_to_markdown("# Note\n", "Новая запись", Some("Лог"));
        assert_eq!(created, "# Note\n\n## Лог\nНовая запись\n");
    }

    #[test]
    fn extracts_cyrillic_snippet() {
        let body = "Введение по архитектуре. ".repeat(5)
            + "Здесь описан локальный протокол MCP без сетевых портов и с песочницей.";
        let snippet = extract_search_snippet(&body, &["протокол".to_string()], 60);
        assert!(snippet.contains("протокол"));
        assert!(snippet.chars().count() <= 62);
    }
}
