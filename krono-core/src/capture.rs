#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliCommand {
    None,
    Help,
    Add(String),
}

/// Parses CLI arguments after the executable name.
/// Unknown flags return `Ok(CliCommand::None)` so OS/launcher flags do not block GUI startup.
pub fn parse_cli_args<I, S>(args: I) -> Result<CliCommand, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut iter = args.into_iter();
    let Some(first) = iter.next() else {
        return Ok(CliCommand::None);
    };
    let cmd = first.as_ref().trim();
    match cmd {
        "-h" | "--help" | "help" => Ok(CliCommand::Help),
        "add" => {
            let parts: Vec<String> = iter
                .map(|s| s.as_ref().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let text = parts.join(" ");
            if text.is_empty() {
                Err("empty text for `krono add`".to_string())
            } else {
                Ok(CliCommand::Add(text))
            }
        }
        _ => Ok(CliCommand::None),
    }
}

/// Appends a timestamped entry (`- **HH:MM** — text`) into the `## Заметки` or `## Notes` section
/// of a daily note if present, or at the end of the document otherwise.
pub fn append_timestamped_entry(markdown: &str, hh_mm: &str, text: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return normalized;
    }

    let mut entry_lines = Vec::new();
    for (idx, line) in trimmed.lines().enumerate() {
        if idx == 0 {
            entry_lines.push(format!("- **{hh_mm}** — {line}"));
        } else {
            entry_lines.push(format!("  {line}"));
        }
    }
    let entry_block = entry_lines.join("\n");

    let lines: Vec<&str> = normalized.lines().collect();
    let notes_heading_idx = lines.iter().position(|line| {
        let t = line.trim();
        t.eq_ignore_ascii_case("## Заметки") || t.eq_ignore_ascii_case("## Notes")
    });

    if let Some(h_idx) = notes_heading_idx {
        let mut insert_at = lines.len();
        for (i, line) in lines.iter().enumerate().skip(h_idx + 1) {
            if line.trim_start().starts_with("## ") {
                insert_at = i;
                break;
            }
        }

        let mut before: Vec<&str> = lines[..insert_at].to_vec();
        while before.len() > h_idx + 1 && before.last().is_some_and(|l| l.trim().is_empty()) {
            before.pop();
        }

        let after = &lines[insert_at..];
        let mut out = Vec::with_capacity(lines.len() + entry_lines.len() + 2);
        out.extend(before.into_iter().map(str::to_string));
        out.extend(entry_lines);
        if !after.is_empty() {
            out.push(String::new());
            out.extend(after.iter().map(|s| (*s).to_string()));
        }
        let mut joined = out.join("\n");
        joined.push('\n');
        return joined;
    }

    let mut out = normalized.trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&entry_block);
    out.push('\n');
    out
}

/// Splits raw scratchpad text into `(title, body)` where the first non-empty line
/// (stripped of leading `#` and whitespace) becomes the note title.
pub fn split_scratchpad_note(raw: &str, fallback_title: &str) -> (String, String) {
    let normalized = raw.replace("\r\n", "\n");
    let mut lines = normalized.lines();
    let mut title = String::new();
    let mut rest_lines: Vec<&str> = Vec::new();

    for line in &mut lines {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            title = trimmed.trim_start_matches('#').trim().to_string();
            break;
        }
    }
    for line in lines {
        rest_lines.push(line);
    }

    if title.is_empty() {
        title = fallback_title.trim().to_string();
    }

    while rest_lines.first().is_some_and(|l| l.trim().is_empty()) {
        rest_lines.remove(0);
    }
    while rest_lines.last().is_some_and(|l| l.trim().is_empty()) {
        rest_lines.pop();
    }

    let body = if rest_lines.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{}\n", rest_lines.join("\n"))
    };
    (title, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cli_args_variants() {
        assert_eq!(parse_cli_args(Vec::<String>::new()), Ok(CliCommand::None));
        assert_eq!(parse_cli_args(["--help"]), Ok(CliCommand::Help));
        assert_eq!(
            parse_cli_args(["add", "купить молоко"]),
            Ok(CliCommand::Add("купить молоко".to_string()))
        );
        assert_eq!(
            parse_cli_args(["add", "buy", "some", "coffee"]),
            Ok(CliCommand::Add("buy some coffee".to_string()))
        );
        assert!(parse_cli_args(["add"]).is_err());
        assert!(parse_cli_args(["add", "   "]).is_err());
        assert_eq!(parse_cli_args(["--shot"]), Ok(CliCommand::None));
    }

    #[test]
    fn appends_timestamped_entry_into_ru_and_en_notes_section() {
        let ru = "# День\n\n## План на день\n- [ ] Дела\n\n## Заметки\n\n\n## Итоги дня\n- Ок\n";
        let updated = append_timestamped_entry(ru, "14:35", "Позвонить Ивану");
        assert!(updated.contains("## Заметки\n- **14:35** — Позвонить Ивану\n\n## Итоги дня"));

        let en = "# Day\n\n## Notes\n- **09:00** — Morning sync\n\n## Daily Reflection\n";
        let updated_en = append_timestamped_entry(en, "10:15", "Line 1\nLine 2");
        assert!(updated_en.contains(
            "## Notes\n- **09:00** — Morning sync\n- **10:15** — Line 1\n  Line 2\n\n## Daily Reflection"
        ));
    }

    #[test]
    fn appends_timestamped_entry_when_notes_section_missing() {
        let plain = "# Note\n\nHello";
        let updated = append_timestamped_entry(plain, "18:05", "Quick thought");
        assert_eq!(updated, "# Note\n\nHello\n- **18:05** — Quick thought\n");
    }

    #[test]
    fn splits_scratchpad_note_with_heading_and_fallback() {
        let (t1, b1) = split_scratchpad_note("# Идея проекта\n\nДетали 1\nДетали 2\n", "Заметка");
        assert_eq!(t1, "Идея проекта");
        assert_eq!(b1, "# Идея проекта\n\nДетали 1\nДетали 2\n");

        let (t2, b2) = split_scratchpad_note("Только одна строка", "Заметка");
        assert_eq!(t2, "Только одна строка");
        assert_eq!(b2, "# Только одна строка\n");

        let (t3, b3) = split_scratchpad_note("   \n  ", "Быстрая заметка");
        assert_eq!(t3, "Быстрая заметка");
        assert_eq!(b3, "# Быстрая заметка\n");
    }
}
