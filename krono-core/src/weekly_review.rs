use crate::daily::SimpleDate;
use crate::tasks::extract_tasks;
use std::collections::{BTreeMap, HashSet};

fn extract_inline_tags(body: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_fence = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        for token in line.split_whitespace() {
            if let Some(rest) = token.strip_prefix('#') {
                let clean: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect();
                if !clean.is_empty() && !clean.chars().all(|c| c.is_ascii_digit()) {
                    tags.push(clean);
                }
            }
        }
    }
    tags
}

#[derive(Clone, Debug)]
pub struct DailyReviewInput {
    pub date: SimpleDate,
    pub rel_path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeeklyReviewResult {
    pub title: String,
    pub rel_path: String,
    pub tags: Vec<String>,
    pub body: String,
    pub completed_count: usize,
    pub open_count: usize,
    pub days_covered: usize,
}

/// Generates a 7-day weekly summary (`end_date - 6 days ..= end_date`) from daily notes:
/// completed tasks, open/carried-over tasks (deduplicated keeping the latest day), and top tags.
pub fn generate_weekly_review(
    end_date: SimpleDate,
    entries: &[DailyReviewInput],
    is_ru: bool,
) -> WeeklyReviewResult {
    let start_date = end_date.add_days(-6);
    let mut window: Vec<&DailyReviewInput> = entries
        .iter()
        .filter(|e| e.date >= start_date && e.date <= end_date)
        .collect();
    window.sort_by_key(|e| e.date);

    let mut completed: Vec<(String, String)> = Vec::new();
    let mut open_map: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut open_order: Vec<String> = Vec::new();
    let mut tag_counts: BTreeMap<String, usize> = BTreeMap::new();

    for entry in &window {
        let mut day_tags: HashSet<String> = HashSet::new();
        for t in &entry.tags {
            let norm = t.trim().trim_start_matches('#').to_lowercase();
            if !norm.is_empty() && norm != "daily" && norm != "weekly-review" {
                day_tags.insert(norm);
            }
        }
        for t in extract_inline_tags(&entry.body) {
            let norm = t.trim().trim_start_matches('#').to_lowercase();
            if !norm.is_empty() && norm != "daily" && norm != "weekly-review" {
                day_tags.insert(norm);
            }
        }
        for tag in day_tags {
            *tag_counts.entry(tag).or_insert(0) += 1;
        }

        for task in extract_tasks(&entry.body, entry.date) {
            let clean = task.clean_text.trim();
            if clean.is_empty() {
                continue;
            }
            if task.done {
                completed.push((clean.to_string(), entry.title.clone()));
            } else {
                let key = clean.to_lowercase();
                if !open_map.contains_key(&key) {
                    open_order.push(key.clone());
                }
                open_map.insert(key, (clean.to_string(), entry.title.clone()));
            }
        }
    }

    let open: Vec<(String, String)> = open_order
        .into_iter()
        .filter_map(|k| open_map.remove(&k))
        .collect();

    let mut top_tags: Vec<(String, usize)> = tag_counts.into_iter().collect();
    top_tags.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_tags.truncate(10);

    let start_iso = start_date.format_iso();
    let end_iso = end_date.format_iso();
    let title = if is_ru {
        format!("Недельный обзор · {start_iso} — {end_iso}")
    } else {
        format!("Weekly Review · {start_iso} — {end_iso}")
    };
    let rel_path = format!("weekly/{end_iso}_weekly-review.md");

    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));

    if is_ru {
        out.push_str(&format!(
            "> Период: **{start_iso}** — **{end_iso}** · Дней с записями: **{}/7** · Выполнено: **{}** · В работе: **{}**\n\n",
            window.len(),
            completed.len(),
            open.len()
        ));
    } else {
        out.push_str(&format!(
            "> Period: **{start_iso}** — **{end_iso}** · Daily notes: **{}/7** · Completed: **{}** · Open: **{}**\n\n",
            window.len(),
            completed.len(),
            open.len()
        ));
    }

    if !window.is_empty() {
        let links: Vec<String> = window.iter().map(|e| format!("[[{}]]", e.title)).collect();
        if is_ru {
            out.push_str(&format!("**Дневники за период:** {}\n\n", links.join(" · ")));
        } else {
            out.push_str(&format!("**Daily notes:** {}\n\n", links.join(" · ")));
        }
    }

    out.push_str(if is_ru {
        "## Выполненные задачи\n"
    } else {
        "## Completed Tasks\n"
    });
    if completed.is_empty() {
        out.push_str(if is_ru {
            "_За последние 7 дней выполненных задач не найдено._\n\n"
        } else {
            "_No completed tasks found in the last 7 days._\n\n"
        });
    } else {
        for (txt, src_title) in &completed {
            out.push_str(&format!("- [x] {txt} _([[{src_title}]])_\n"));
        }
        out.push('\n');
    }

    out.push_str(if is_ru {
        "## Незакрытые задачи\n"
    } else {
        "## Open / Carried-over Tasks\n"
    });
    if open.is_empty() {
        out.push_str(if is_ru {
            "_Все задачи за неделю закрыты._\n\n"
        } else {
            "_All tasks for the week are closed._\n\n"
        });
    } else {
        for (txt, src_title) in &open {
            out.push_str(&format!("- [ ] {txt} _([[{src_title}]])_\n"));
        }
        out.push('\n');
    }

    out.push_str(if is_ru {
        "## Топ тегов недели\n"
    } else {
        "## Top Tags\n"
    });
    if top_tags.is_empty() {
        out.push_str(if is_ru {
            "_Теги не найдены._\n\n"
        } else {
            "_No tags found._\n\n"
        });
    } else {
        for (tag, count) in &top_tags {
            out.push_str(&format!("- `#{tag}` — {count}\n"));
        }
        out.push('\n');
    }

    if is_ru {
        out.push_str("## Итоги и фокус на следующую неделю\n- Главный результат:\n- Фокус №1:\n");
    } else {
        out.push_str("## Reflection & Next Week Focus\n- Key win:\n- Top priority next week:\n");
    }

    WeeklyReviewResult {
        title,
        rel_path,
        tags: vec!["weekly-review".to_string()],
        body: out,
        completed_count: completed.len(),
        open_count: open.len(),
        days_covered: window.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> SimpleDate {
        SimpleDate::from_ymd(y, m, day).unwrap()
    }

    #[test]
    fn aggregates_seven_days_dedups_open_tasks_and_ranks_tags() {
        let end = d(2026, 11, 21);
        let entries = vec![
            // Outside 7-day window (2026-11-14 < 2026-11-15)
            DailyReviewInput {
                date: d(2026, 11, 14),
                rel_path: "daily/2026/11/2026-11-14.md".into(),
                title: "2026-11-14".into(),
                tags: vec!["old".into()],
                body: "- [x] Old task #old\n".into(),
            },
            DailyReviewInput {
                date: d(2026, 11, 16),
                rel_path: "daily/2026/11/2026-11-16.md".into(),
                title: "16 ноя".into(),
                tags: vec!["daily".into(), "rust".into()],
                body: "- [x] Finish parser #krono\n- [ ] Write docs #rust\n".into(),
            },
            DailyReviewInput {
                date: d(2026, 11, 21),
                rel_path: "daily/2026/11/2026-11-21.md".into(),
                title: "21 ноя".into(),
                tags: vec!["daily".into(), "rust".into()],
                body: "- [ ] Write docs #rust\n- [x] Ship beta\n".into(),
            },
        ];

        let res = generate_weekly_review(end, &entries, true);
        assert_eq!(res.rel_path, "weekly/2026-11-21_weekly-review.md");
        assert_eq!(res.days_covered, 2);
        assert_eq!(res.completed_count, 2);
        assert_eq!(res.open_count, 1);
        assert!(res.body.contains("Finish parser"));
        assert!(!res.body.contains("Old task"));
        assert!(res.body.contains("Write docs #rust _([[21 ноя]])_"));
        assert!(res.body.contains("`#rust` — 2"));
        assert!(!res.body.contains("`#daily`"));
    }

    #[test]
    fn handles_empty_entries_cleanly_in_english() {
        let res = generate_weekly_review(d(2026, 11, 21), &[], false);
        assert_eq!(res.days_covered, 0);
        assert_eq!(res.completed_count, 0);
        assert_eq!(res.open_count, 0);
        assert!(res.body.contains("## Completed Tasks"));
        assert!(res.body.contains("No completed tasks found"));
    }
}
