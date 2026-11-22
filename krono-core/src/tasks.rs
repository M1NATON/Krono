use crate::daily::SimpleDate;
use crate::markdown::{is_fence_toggle, match_task_line_prefix, parse_inline_task_meta, TaskPriority};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskRecurrence {
    Daily,
    Weekly,
    Monthly,
}

impl TaskRecurrence {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    pub fn from_db_str(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }

    pub fn next_date(self, base: SimpleDate) -> SimpleDate {
        match self {
            Self::Daily => base.add_days(1),
            Self::Weekly => base.add_days(7),
            Self::Monthly => base.add_months(1),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedTask {
    pub line_idx: usize,
    pub task_idx: usize,
    pub raw_text: String,
    pub clean_text: String,
    pub done: bool,
    pub due_date: Option<SimpleDate>,
    pub completed_at: Option<SimpleDate>,
    pub priority: Option<TaskPriority>,
    pub recurrence: Option<TaskRecurrence>,
    pub tags: Vec<String>,
}

/// Extracts `✅ YYYY-MM-DD` / `✅YYYY-MM-DD` / `done:YYYY-MM-DD` completion markers.
pub fn extract_completed_date(raw: &str) -> (String, Option<SimpleDate>) {
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(tokens.len());
    let mut found: Option<SimpleDate> = None;
    let mut i = 0usize;

    while i < tokens.len() {
        let tok = tokens[i];
        if tok == "✅" {
            if let Some(next) = tokens.get(i + 1) {
                if let Some(d) = SimpleDate::parse_iso(next) {
                    if found.is_none() {
                        found = Some(d);
                    }
                    i += 2;
                    continue;
                }
            }
        } else if let Some(rest) = tok.strip_prefix('✅') {
            if let Some(d) = SimpleDate::parse_iso(rest) {
                if found.is_none() {
                    found = Some(d);
                }
                i += 1;
                continue;
            }
        } else if tok.len() > 5 && tok.get(..5).is_some_and(|p| p.eq_ignore_ascii_case("done:")) {
            if let Some(d) = SimpleDate::parse_iso(&tok[5..]) {
                if found.is_none() {
                    found = Some(d);
                }
                i += 1;
                continue;
            }
        }
        kept.push(tok);
        i += 1;
    }

    (kept.join(" "), found)
}

/// Extracts all Markdown checklist tasks outside fenced code blocks.
pub fn extract_tasks(body: &str, reference_date: SimpleDate) -> Vec<ParsedTask> {
    let mut out = Vec::new();
    let mut in_fence: Option<char> = None;
    let mut task_idx = 0usize;

    for (line_idx, segment) in body.split_inclusive('\n').enumerate() {
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
        if let Some((prefix, rest)) = match_task_line_prefix(trimmed) {
            let done = prefix.contains('x') || prefix.contains('X');
            let raw_text = rest.trim_end_matches(['\r', '\n']).trim().to_string();
            let (without_done_date, completed_at) = extract_completed_date(&raw_text);
            let meta = parse_inline_task_meta(&without_done_date, reference_date);
            let due_date = meta
                .due_date
                .as_deref()
                .and_then(SimpleDate::parse_iso);
            out.push(ParsedTask {
                line_idx,
                task_idx,
                raw_text,
                clean_text: meta.clean_text,
                done,
                due_date,
                completed_at: if done { completed_at } else { None },
                priority: meta.priority,
                recurrence: meta.recurrence,
                tags: meta.tags,
            });
            task_idx += 1;
        }
    }

    out
}

/// Atomically toggles `[ ]` <-> `[x]` on the `target_task_idx`-th task outside fenced code blocks.
/// If a recurring task transitions `[ ]` -> `[x]`, keeps `[x]` in place for history and inserts
/// the next `[ ]` occurrence directly underneath with the updated date and preserved indentation.
pub fn toggle_task_in_markdown(
    body: &str,
    target_task_idx: usize,
    today: SimpleDate,
) -> Option<String> {
    let mut out = String::with_capacity(body.len() + 64);
    let mut in_fence: Option<char> = None;
    let mut current_idx = 0usize;
    let mut toggled = false;

    let default_eol = if body.contains("\r\n") { "\r\n" } else { "\n" };

    for segment in body.split_inclusive('\n') {
        let (line_no_eol, eol) = if let Some(stripped) = segment.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (segment, "")
        };

        let trimmed = line_no_eol.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            out.push_str(segment);
            continue;
        }

        if in_fence.is_none() && !toggled {
            if let Some((prefix, rest)) = match_task_line_prefix(trimmed) {
                if current_idx == target_task_idx {
                    let indent_len = line_no_eol.len() - trimmed.len();
                    let indent = &line_no_eol[..indent_len];
                    let bullet = if prefix.starts_with('*') { '*' } else { '-' };
                    let was_done = prefix.contains('x') || prefix.contains('X');
                    let next_mark = if was_done { ' ' } else { 'x' };

                    out.push_str(indent);
                    out.push(bullet);
                    out.push_str(" [");
                    out.push(next_mark);
                    out.push_str("] ");
                    if was_done {
                        let (cleaned_rest, _) = extract_completed_date(rest);
                        out.push_str(&cleaned_rest);
                    } else {
                        let trimmed_rest = rest.trim_end();
                        out.push_str(trimmed_rest);
                        if extract_completed_date(trimmed_rest).1.is_none() {
                            if !trimmed_rest.is_empty() {
                                out.push(' ');
                            }
                            out.push_str("✅ ");
                            out.push_str(&today.format_iso());
                        }
                    }

                    if !was_done {
                        let meta = parse_inline_task_meta(rest, today);
                        if let Some(rec) = meta.recurrence {
                            let base_date = meta
                                .due_date
                                .as_deref()
                                .and_then(SimpleDate::parse_iso)
                                .unwrap_or(today);
                            let next_due = rec.next_date(base_date);
                            let (rest_no_done, _) = extract_completed_date(rest);
                            let next_rest = replace_or_append_due_date(&rest_no_done, next_due);
                            let sep = if eol.is_empty() { default_eol } else { eol };
                            out.push_str(sep);
                            out.push_str(indent);
                            out.push(bullet);
                            out.push_str(" [ ] ");
                            out.push_str(&next_rest);
                        }
                    }

                    out.push_str(eol);
                    toggled = true;
                    continue;
                }
                current_idx += 1;
            }
        }

        out.push_str(segment);
    }

    if toggled {
        Some(out)
    } else {
        None
    }
}

fn replace_or_append_due_date(raw_rest: &str, next_due: SimpleDate) -> String {
    let next_token = format!("@{}", next_due.format_iso());
    let mut replaced = false;
    let mut parts: Vec<String> = Vec::new();

    for token in raw_rest.split_whitespace() {
        let lower = token.to_lowercase();
        let trimmed_punct = lower.trim_end_matches([',', '.', ';', ':']);
        let trailing_punct = &token[trimmed_punct.len().min(token.len())..];

        let is_date_token = matches!(
            trimmed_punct,
            "@today" | "@сегодня" | "@tomorrow" | "@завтра"
        ) || token
            .strip_prefix('@')
            .or_else(|| token.strip_prefix('📅'))
            .is_some_and(|d| SimpleDate::parse_iso(d.trim_end_matches([',', '.', ';', ':'])).is_some());

        if is_date_token {
            if !replaced {
                parts.push(format!("{next_token}{trailing_punct}"));
                replaced = true;
            }
        } else {
            parts.push(token.to_string());
        }
    }

    if !replaced {
        parts.push(next_token);
    }
    parts.join(" ")
}

/// Extracts raw open task lines (`- [ ] ...`) outside fenced code blocks for daily carryover.
pub fn extract_open_tasks_for_carryover(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence: Option<char> = None;

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
        if let Some((prefix, rest)) = match_task_line_prefix(trimmed) {
            if !(prefix.contains('x') || prefix.contains('X')) {
                let clean = rest.trim_end_matches(['\r', '\n']).trim();
                if !clean.is_empty() {
                    out.push(format!("- [ ] {clean}"));
                }
            }
        }
    }
    out
}

/// Appends non-duplicate open tasks from a previous daily note into `today_body`.
/// Returns `(updated_body, added_count)`.
pub fn append_carried_tasks(
    today_body: &str,
    open_tasks: &[String],
    _source_date: SimpleDate,
) -> (String, usize) {
    let mut existing: HashSet<String> = HashSet::new();
    let mut in_fence: Option<char> = None;
    for segment in today_body.split_inclusive('\n') {
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
            existing.insert(rest.trim().to_lowercase());
        }
    }

    let mut to_add: Vec<String> = Vec::new();
    for line in open_tasks {
        let trimmed = line.trim();
        let rest_key = match_task_line_prefix(trimmed)
            .map(|(_, r)| r.trim().to_lowercase())
            .unwrap_or_else(|| trimmed.to_lowercase());
        if !rest_key.is_empty() && existing.insert(rest_key) {
            to_add.push(trimmed.to_string());
        }
    }

    if to_add.is_empty() {
        return (today_body.to_string(), 0);
    }

    let eol = if today_body.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out = today_body.trim_end_matches(['\r', '\n']).to_string();
    if !out.is_empty() {
        out.push_str(eol);
        out.push_str(eol);
    }
    let added = to_add.len();
    out.push_str(&to_add.join(eol));
    out.push_str(eol);
    (out, added)
}

/// Appends ` [[link_target]]` to the `target_task_idx`-th task line if not already linked.
pub fn append_wikilink_to_task(
    body: &str,
    target_task_idx: usize,
    link_target: &str,
) -> Option<String> {
    let clean_target = link_target.trim();
    if clean_target.is_empty() {
        return None;
    }
    let needle = format!("[[{clean_target}]]");
    let mut out = String::with_capacity(body.len() + needle.len() + 2);
    let mut in_fence: Option<char> = None;
    let mut current_idx = 0usize;
    let mut updated = false;

    for segment in body.split_inclusive('\n') {
        let (line_no_eol, eol) = if let Some(stripped) = segment.strip_suffix("\r\n") {
            (stripped, "\r\n")
        } else if let Some(stripped) = segment.strip_suffix('\n') {
            (stripped, "\n")
        } else {
            (segment, "")
        };
        let trimmed = line_no_eol.trim_start();
        if let Some(ch) = is_fence_toggle(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            out.push_str(segment);
            continue;
        }
        if in_fence.is_none() && !updated && match_task_line_prefix(trimmed).is_some() {
            if current_idx == target_task_idx {
                out.push_str(line_no_eol.trim_end());
                if !line_no_eol.to_lowercase().contains(&needle.to_lowercase()) {
                    out.push(' ');
                    out.push_str(&needle);
                }
                out.push_str(eol);
                updated = true;
                continue;
            }
            current_idx += 1;
        }
        out.push_str(segment);
    }

    if updated {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tasks_with_metadata_and_skips_fences() {
        let today = SimpleDate::from_ymd(2026, 11, 21).unwrap();
        let md = "# Day\n```md\n- [ ] Not a task @today\n```\n- [ ] Pay rent @2026-11-25 !high @every month #finance\n  * [X] Done item @today !low\n";
        let tasks = extract_tasks(md, today);
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].task_idx, 0);
        assert!(!tasks[0].done);
        assert_eq!(tasks[0].due_date, SimpleDate::from_ymd(2026, 11, 25));
        assert_eq!(tasks[0].priority, Some(TaskPriority::High));
        assert_eq!(tasks[0].recurrence, Some(TaskRecurrence::Monthly));
        assert_eq!(tasks[0].tags, vec!["finance".to_string()]);
        assert_eq!(tasks[0].clean_text, "Pay rent #finance");

        assert_eq!(tasks[1].task_idx, 1);
        assert!(tasks[1].done);
        assert_eq!(tasks[1].due_date, Some(today));
        assert_eq!(tasks[1].priority, Some(TaskPriority::Low));
    }

    #[test]
    fn toggles_task_atomically_and_inserts_recurrence_below() {
        let today = SimpleDate::from_ymd(2026, 11, 21).unwrap();
        let md = "Intro\r\n  - [ ] Standup @2026-11-21 @every week !med #team\r\nOutro\r\n";
        let toggled = toggle_task_in_markdown(md, 0, today).unwrap();
        assert_eq!(
            toggled,
            "Intro\r\n  - [x] Standup @2026-11-21 @every week !med #team ✅ 2026-11-21\r\n  - [ ] Standup @2026-11-28 @every week !med #team\r\nOutro\r\n"
        );

        // Toggle back to open does not create duplicate recurrence and removes completion marker
        let untoggled = toggle_task_in_markdown(&toggled, 0, today).unwrap();
        assert_eq!(
            untoggled,
            "Intro\r\n  - [ ] Standup @2026-11-21 @every week !med #team\r\n  - [ ] Standup @2026-11-28 @every week !med #team\r\nOutro\r\n"
        );
    }

    #[test]
    fn toggle_task_records_and_clears_completed_at_date() {
        let today = SimpleDate::from_ymd(2026, 11, 22).unwrap();
        let md = "- [ ] Подготовить релиз #krono\n";
        let done_md = toggle_task_in_markdown(md, 0, today).unwrap();
        assert_eq!(done_md, "- [x] Подготовить релиз #krono ✅ 2026-11-22\n");

        let parsed = extract_tasks(&done_md, today);
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].done);
        assert_eq!(parsed[0].completed_at, Some(today));
        assert_eq!(parsed[0].clean_text, "Подготовить релиз #krono");

        let reopened_md = toggle_task_in_markdown(&done_md, 0, today).unwrap();
        assert_eq!(reopened_md, "- [ ] Подготовить релиз #krono\n");
        let parsed_open = extract_tasks(&reopened_md, today);
        assert!(!parsed_open[0].done);
        assert_eq!(parsed_open[0].completed_at, None);
    }

    #[test]
    fn carries_over_open_tasks_without_duplicates() {
        let prev = SimpleDate::from_ymd(2026, 11, 20).unwrap();
        let prev_body = "- [x] Done\n- [ ] Call client !high\n- [ ] Write docs\n";
        let open = extract_open_tasks_for_carryover(prev_body);
        assert_eq!(open.len(), 2);

        let today_body = "# Today\n\n- [ ] Write docs\n";
        let (merged, added) = append_carried_tasks(today_body, &open, prev);
        assert_eq!(added, 1);
        assert!(merged.contains("- [ ] Call client !high"));
        assert_eq!(merged.matches("Write docs").count(), 1);
    }

    #[test]
    fn appends_wikilink_to_task_line() {
        let body = "- [ ] Design API !high\n- [ ] Next\n";
        let updated = append_wikilink_to_task(body, 0, "Design API").unwrap();
        assert_eq!(updated, "- [ ] Design API !high [[Design API]]\n- [ ] Next\n");
    }
}
