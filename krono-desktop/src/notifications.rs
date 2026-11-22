use crate::i18n::Lang;
use crate::storage::index::VaultTaskRow;
use krono_core::daily::SimpleDate;
use std::collections::HashSet;
use std::time::Instant;

pub fn send_task_notification(summary: &str, body: &str) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        notify_rust::Notification::new()
            .appname("Krono")
            .summary(summary)
            .body(body)
            .show()
            .map(|_| ())
            .map_err(|e| format!("notification error: {e}"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (summary, body);
        Ok(())
    }
}

#[derive(Default)]
pub struct TaskNotificationTracker {
    notified_day: Option<SimpleDate>,
    notified_keys: HashSet<String>,
    last_check: Option<Instant>,
}

impl TaskNotificationTracker {
    pub fn next_due_batch(
        &mut self,
        tasks: &[VaultTaskRow],
        today: SimpleDate,
        lang: Lang,
    ) -> Option<(String, String)> {
        if let Some(last) = self.last_check {
            if last.elapsed().as_secs() < 60 {
                return None;
            }
        }
        self.last_check = Some(Instant::now());

        if self.notified_day != Some(today) {
            self.notified_day = Some(today);
            self.notified_keys.clear();
        }

        let mut fresh: Vec<&VaultTaskRow> = Vec::new();
        for t in tasks {
            if t.done {
                continue;
            }
            let Some(due) = t.due_date else {
                continue;
            };
            if due > today {
                continue;
            }
            let key = format!("{}:{}:{}", t.path, t.task_idx, due.format_iso());
            if self.notified_keys.insert(key) {
                fresh.push(t);
            }
        }

        if fresh.is_empty() {
            return None;
        }

        let summary = if lang == Lang::Ru {
            format!("Krono: задачи на сегодня ({})", fresh.len())
        } else {
            format!("Krono: due tasks ({})", fresh.len())
        };
        let body = if fresh.len() == 1 {
            fresh[0].clean_text.clone()
        } else {
            let preview: Vec<String> = fresh
                .iter()
                .take(3)
                .map(|t| format!("• {}", t.clean_text))
                .collect();
            preview.join("\n")
        };
        Some((summary, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, idx: usize, text: &str, due: Option<SimpleDate>, done: bool) -> VaultTaskRow {
        VaultTaskRow {
            path: path.to_string(),
            note_title: path.to_string(),
            line_idx: idx,
            task_idx: idx,
            raw_text: text.to_string(),
            clean_text: text.to_string(),
            done,
            due_date: due,
            completed_at: None,
            priority: None,
            recurrence: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn deduplicates_notifications_within_same_day() {
        let today = SimpleDate::from_ymd(2026, 11, 21).unwrap();
        let tasks = vec![
            row("a.md", 0, "Due today", Some(today), false),
            row("b.md", 0, "Future", Some(today.next_day()), false),
            row("c.md", 0, "Already done", Some(today), true),
        ];
        let mut tracker = TaskNotificationTracker::default();
        let first = tracker.next_due_batch(&tasks, today, Lang::Ru);
        assert!(first.is_some());
        // Simulate 60s elapsed without changing day -> no duplicate notification
        tracker.last_check = None;
        assert!(tracker.next_due_batch(&tasks, today, Lang::Ru).is_none());
    }
}
