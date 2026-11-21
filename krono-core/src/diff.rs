#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffOp {
    Equal,
    Insert,
    Delete,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub op: DiffOp,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DiffStats {
    pub added: usize,
    pub removed: usize,
}

fn split_normalized_lines(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let norm = s.replace("\r\n", "\n");
    norm.split('\n').map(|l| l.to_string()).collect()
}

/// Computes a line-by-line LCS diff between `old_text` and `new_text`.
/// Normalizes `\r\n` to `\n`. Falls back to prefix/suffix trim if `n * m` is huge.
pub fn diff_lines(old_text: &str, new_text: &str) -> (Vec<DiffLine>, DiffStats) {
    let old = split_normalized_lines(old_text);
    let new = split_normalized_lines(new_text);

    let mut prefix = 0usize;
    while prefix < old.len() && prefix < new.len() && old[prefix] == new[prefix] {
        prefix += 1;
    }

    let mut suffix = 0usize;
    while suffix + prefix < old.len()
        && suffix + prefix < new.len()
        && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];

    let mut out = Vec::new();
    let mut stats = DiffStats::default();

    for (i, line) in old[..prefix].iter().enumerate() {
        out.push(DiffLine {
            op: DiffOp::Equal,
            old_line: Some(i + 1),
            new_line: Some(i + 1),
            text: line.clone(),
        });
    }

    let n = old_mid.len();
    let m = new_mid.len();

    if n == 0 && m == 0 {
        // Nothing changed in middle.
    } else if n.saturating_mul(m) > 2_000_000 {
        for (idx, line) in old_mid.iter().enumerate() {
            stats.removed += 1;
            out.push(DiffLine {
                op: DiffOp::Delete,
                old_line: Some(prefix + idx + 1),
                new_line: None,
                text: line.clone(),
            });
        }
        for (idx, line) in new_mid.iter().enumerate() {
            stats.added += 1;
            out.push(DiffLine {
                op: DiffOp::Insert,
                old_line: None,
                new_line: Some(prefix + idx + 1),
                text: line.clone(),
            });
        }
    } else {
        let cols = m + 1;
        let mut dp = vec![0u32; (n + 1) * cols];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                dp[i * cols + j] = if old_mid[i] == new_mid[j] {
                    dp[(i + 1) * cols + (j + 1)] + 1
                } else {
                    dp[(i + 1) * cols + j].max(dp[i * cols + (j + 1)])
                };
            }
        }

        let mut i = 0usize;
        let mut j = 0usize;
        while i < n || j < m {
            if i < n && j < m && old_mid[i] == new_mid[j] {
                out.push(DiffLine {
                    op: DiffOp::Equal,
                    old_line: Some(prefix + i + 1),
                    new_line: Some(prefix + j + 1),
                    text: old_mid[i].clone(),
                });
                i += 1;
                j += 1;
            } else if i < n && (j == m || dp[(i + 1) * cols + j] >= dp[i * cols + (j + 1)]) {
                stats.removed += 1;
                out.push(DiffLine {
                    op: DiffOp::Delete,
                    old_line: Some(prefix + i + 1),
                    new_line: None,
                    text: old_mid[i].clone(),
                });
                i += 1;
            } else {
                stats.added += 1;
                out.push(DiffLine {
                    op: DiffOp::Insert,
                    old_line: None,
                    new_line: Some(prefix + j + 1),
                    text: new_mid[j].clone(),
                });
                j += 1;
            }
        }
    }

    let old_tail_start = old.len() - suffix;
    let new_tail_start = new.len() - suffix;
    for k in 0..suffix {
        out.push(DiffLine {
            op: DiffOp::Equal,
            old_line: Some(old_tail_start + k + 1),
            new_line: Some(new_tail_start + k + 1),
            text: old[old_tail_start + k].clone(),
        });
    }

    (out, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_produce_no_changes() {
        let (lines, stats) = diff_lines("Привет\nМир", "Привет\r\nМир");
        assert_eq!(stats, DiffStats { added: 0, removed: 0 });
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.op == DiffOp::Equal));
    }

    #[test]
    fn empty_old_and_empty_new() {
        let (lines_add, stats_add) = diff_lines("", "Первая\nВторая");
        assert_eq!(stats_add, DiffStats { added: 2, removed: 0 });
        assert!(lines_add.iter().all(|l| l.op == DiffOp::Insert));

        let (lines_del, stats_del) = diff_lines("Первая\nВторая", "");
        assert_eq!(stats_del, DiffStats { added: 0, removed: 2 });
        assert!(lines_del.iter().all(|l| l.op == DiffOp::Delete));
    }

    #[test]
    fn cyrillic_additions_deletions_and_edits() {
        let old = "Заголовок\nСтарая строка\nОбщая строка";
        let new = "Заголовок\nНовая строка\nДобавлена ещё\nОбщая строка";
        let (lines, stats) = diff_lines(old, new);
        assert_eq!(stats, DiffStats { added: 2, removed: 1 });
        assert_eq!(lines[0].op, DiffOp::Equal);
        assert_eq!(lines[0].text, "Заголовок");
        assert!(lines.iter().any(|l| l.op == DiffOp::Delete && l.text == "Старая строка"));
        assert!(lines.iter().any(|l| l.op == DiffOp::Insert && l.text == "Новая строка"));
        assert!(lines.iter().any(|l| l.op == DiffOp::Insert && l.text == "Добавлена ещё"));
        assert_eq!(lines.last().unwrap().op, DiffOp::Equal);
    }
}
