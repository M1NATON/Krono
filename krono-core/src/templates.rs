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

/// Built-in language lesson note template with New Vocabulary, Grammar Rule, and Practice blocks.
pub fn language_lesson_template(
    title: &str,
    study_language: &str,
    is_ru: bool,
) -> (Vec<String>, String) {
    let lang_tag: String = study_language
        .trim()
        .to_lowercase()
        .replace(' ', "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let lang_tag = if lang_tag.is_empty() {
        "language".to_string()
    } else {
        lang_tag
    };
    let tags = vec![lang_tag, "lesson".to_string()];
    let body = if is_ru {
        format!(
            "# {title}\n\n## Новые слова\n| Слово | Транскрипция | Перевод | Пример |\n| --- | --- | --- | --- |\n| | | | |\n\n## Грамматика\n**Правило:** \n\n**Свои примеры:**\n- \n- \n\n## Практика\n- [ ] Составить 5 предложений с новыми словами\n- [ ] Проговорить вслух или написать короткий абзац\n\n> Заметки:\n> \n"
        )
    } else {
        format!(
            "# {title}\n\n## New Vocabulary\n| Word | Transcription | Translation | Example |\n| --- | --- | --- | --- |\n| | | | |\n\n## Grammar Rule\n**Rule:** \n\n**Own examples:**\n- \n- \n\n## Speaking / Writing Practice\n- [ ] Write 5 sentences using the new words\n- [ ] Speak aloud or draft a short paragraph\n\n> Practice:\n> \n"
        )
    };
    (tags, body)
}

/// Built-in English Lesson note template (delegates to `language_lesson_template`).
pub fn english_lesson_template(title: &str, is_ru: bool) -> (Vec<String>, String) {
    language_lesson_template(title, "english", is_ru)
}

/// Built-in Media Review note template with genre tags, `[[Director]]` link, rating, and mini-review.
pub fn media_review_template(
    title: &str,
    year: &str,
    director: &str,
    genres: &[String],
    rating: &str,
    mini_review: &str,
    is_ru: bool,
) -> (Vec<String>, String) {
    let mut tags: Vec<String> = Vec::new();
    for raw in genres {
        let cleaned = raw
            .trim()
            .trim_start_matches('#')
            .to_lowercase()
            .replace(' ', "-");
        if !cleaned.is_empty() && !tags.iter().any(|t| t == &cleaned) {
            tags.push(cleaned);
        }
    }
    if !tags.iter().any(|t| t == "review") {
        tags.push("review".to_string());
    }

    let heading = if year.trim().is_empty() {
        format!("# {title}")
    } else {
        format!("# {title} ({})", year.trim())
    };
    let director_line = if director.trim().is_empty() {
        String::new()
    } else if is_ru {
        format!("- **Режиссёр / Создатель:** [[{}]]\n", director.trim())
    } else {
        format!("- **Director / Creator:** [[{}]]\n", director.trim())
    };
    let rating_line = if rating.trim().is_empty() {
        String::new()
    } else if is_ru {
        format!("- **Оценка:** ★ {}/10\n", rating.trim())
    } else {
        format!("- **Rating:** ★ {}/10\n", rating.trim())
    };
    let summary = if mini_review.trim().is_empty() {
        String::new()
    } else {
        format!("\n> {}\n", mini_review.trim().replace("\r\n", "\n"))
    };

    let body = if is_ru {
        format!(
            "{heading}\n\n{director_line}{rating_line}{summary}\n## Впечатления\n\n\n## Детали и цитаты\n- \n\n## Итог\n\n"
        )
    } else {
        format!(
            "{heading}\n\n{director_line}{rating_line}{summary}\n## Impressions\n\n\n## Details & Quotes\n- \n\n## Verdict\n\n"
        )
    };
    (tags, body.replace("\r\n", "\n"))
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
        let (tags_ru, body_ru) = english_lesson_template("Урок 1", true);
        assert_eq!(tags_ru, vec!["english", "lesson"]);
        assert!(body_ru.contains("## Новые слова"));
        assert!(body_ru.contains("## Грамматика"));
        assert!(body_ru.contains("## Практика"));

        let (tags_en, body_en) = english_lesson_template("Lesson 1", false);
        assert_eq!(tags_en, vec!["english", "lesson"]);
        assert!(body_en.contains("## New Vocabulary"));
        assert!(body_en.contains("## Grammar Rule"));
        assert!(body_en.contains("## Speaking / Writing Practice"));
    }

    #[test]
    fn builds_language_lesson_template_with_custom_language() {
        let (tags, body) = language_lesson_template("Урок 1", "Spanish", true);
        assert_eq!(tags, vec!["spanish", "lesson"]);
        assert!(body.contains("## Новые слова"));
        assert!(!body.contains("ubiquitous"));
    }

    #[test]
    fn builds_media_review_template_with_tags_and_director_wikilink() {
        let (tags, body) = media_review_template(
            "Interstellar",
            "2014",
            "Christopher Nolan",
            &["Sci-Fi".into(), "Drama".into()],
            "9",
            "Мощный саундтрек и визуал.",
            true,
        );
        assert_eq!(tags, vec!["sci-fi", "drama", "review"]);
        assert!(body.contains("# Interstellar (2014)"));
        assert!(body.contains("[[Christopher Nolan]]"));
        assert!(body.contains("★ 9/10"));
        assert!(body.contains("> Мощный саундтрек и визуал."));
        assert!(body.contains("## Впечатления"));
    }
}
