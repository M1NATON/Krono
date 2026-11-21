pub struct TemplateVars<'a> {
    pub title: &'a str,
    pub date: &'a str,
    pub month: &'a str,
    pub weekday: &'a str,
}

/// Substitutes `{{title}}`, `{{date}}`, `{{month}}`, `{{weekday}}` and normalizes CRLF to LF.
pub fn render_template(raw: &str, vars: &TemplateVars<'_>) -> String {
    let normalized = raw.replace("\r\n", "\n");
    normalized
        .replace("{{title}}", vars.title)
        .replace("{{date}}", vars.date)
        .replace("{{month}}", vars.month)
        .replace("{{weekday}}", vars.weekday)
}

/// Validates a custom template file name (must end in `.md`, no path separators or `..`).
/// Returns the sanitized stem ID.
pub fn sanitize_template_id(file_name: &str) -> Option<String> {
    let trimmed = file_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        return None;
    }
    let stem = trimmed.strip_suffix(".md")?;
    if stem.is_empty() || stem.starts_with('.') {
        return None;
    }
    Some(stem.to_string())
}

/// Built-in English Lesson note template with New Vocabulary, Grammar Rule, and Practice blocks.
pub fn english_lesson_template(title: &str, is_ru: bool) -> (Vec<String>, String) {
    let tags = vec!["english".to_string(), "lesson".to_string()];
    let body = if is_ru {
        format!(
            "# {title}\n\n## New Vocabulary\n| Word | Transcription | Translation | Example |\n| --- | --- | --- | --- |\n| ubiquitous | /juːˈbɪk.wɪ.təs/ | повсеместный | Smartphones have become ubiquitous. |\n| | | | |\n\n## Grammar Rule\n**Правило:** \n\n**Свои примеры:**\n- \n- \n\n## Speaking / Writing Practice\n- [ ] Составить 5 предложений с новыми словами\n- [ ] Проговорить вслух или написать короткий абзац\n\n> Практика:\n> \n"
        )
    } else {
        format!(
            "# {title}\n\n## New Vocabulary\n| Word | Transcription | Translation | Example |\n| --- | --- | --- | --- |\n| ubiquitous | /juːˈbɪk.wɪ.təs/ | everywhere | Smartphones have become ubiquitous. |\n| | | | |\n\n## Grammar Rule\n**Rule:** \n\n**Own examples:**\n- \n- \n\n## Speaking / Writing Practice\n- [ ] Write 5 sentences using the new words\n- [ ] Speak aloud or draft a short paragraph\n\n> Practice:\n> \n"
        )
    };
    (tags, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_template_placeholders_and_normalizes_crlf() {
        let raw = "# {{title}}\r\nДата: {{date}} ({{weekday}}), месяц: [[{{month}}]]\r\n";
        let vars = TemplateVars {
            title: "План",
            date: "2026-11-21",
            month: "Ноябрь 2026",
            weekday: "Суббота",
        };
        let out = render_template(raw, &vars);
        assert_eq!(
            out,
            "# План\nДата: 2026-11-21 (Суббота), месяц: [[Ноябрь 2026]]\n"
        );
    }

    #[test]
    fn sanitizes_template_ids() {
        assert_eq!(sanitize_template_id("standup.md"), Some("standup".into()));
        assert_eq!(sanitize_template_id("../evil.md"), None);
        assert_eq!(sanitize_template_id("sub/a.md"), None);
        assert_eq!(sanitize_template_id("notes.txt"), None);
        assert_eq!(sanitize_template_id(".hidden.md"), None);
    }

    #[test]
    fn builds_english_lesson_template() {
        let (tags, body) = english_lesson_template("Lesson 1", true);
        assert_eq!(tags, vec!["english", "lesson"]);
        assert!(body.contains("## New Vocabulary"));
        assert!(body.contains("## Grammar Rule"));
        assert!(body.contains("## Speaking / Writing Practice"));
    }
}
