use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl SimpleDate {
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(1..=12).contains(&month) {
            return None;
        }
        let max_d = days_in_month(year, month);
        if day < 1 || day > max_d {
            return None;
        }
        Some(Self { year, month, day })
    }

    pub fn format_iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn parse_iso(raw: &str) -> Option<Self> {
        let mut parts = raw.trim().split('-');
        let y: i32 = parts.next()?.parse().ok()?;
        let m: u32 = parts.next()?.parse().ok()?;
        let d: u32 = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Self::from_ymd(y, m, d)
    }

    /// Weekday index where 0 = Monday .. 6 = Sunday.
    pub fn weekday_mon0(self) -> u32 {
        let days = days_from_civil(self.year, self.month, self.day);
        (days + 3).rem_euclid(7) as u32
    }

    pub fn prev_day(self) -> Self {
        let days = days_from_civil(self.year, self.month, self.day) - 1;
        civil_from_days(days)
    }

    pub fn next_day(self) -> Self {
        let days = days_from_civil(self.year, self.month, self.day) + 1;
        civil_from_days(days)
    }
}

pub fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

pub fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month <= 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
    }
}

pub fn next_month(year: i32, month: u32) -> (i32, u32) {
    if month >= 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    }
}

pub fn daily_note_rel_path(date: SimpleDate) -> String {
    format!(
        "daily/{:04}/{:02}/{}.md",
        date.year,
        date.month,
        date.format_iso()
    )
}

pub fn daily_legacy_rel_path(date: SimpleDate) -> String {
    format!("daily/{}.md", date.format_iso())
}

pub fn daily_year_hub_rel_path(year: i32) -> String {
    format!("daily/{year:04}/{year:04}.md")
}

pub fn daily_month_hub_rel_path(year: i32, month: u32) -> String {
    format!("daily/{year:04}/{month:02}/{year:04}-{month:02}.md")
}

/// Parses both nested (`daily/YYYY/MM/YYYY-MM-DD.md`) and legacy (`daily/YYYY-MM-DD.md`) daily note paths.
pub fn parse_daily_note_path(rel_path: &str) -> Option<SimpleDate> {
    let norm = rel_path.trim().replace('\\', "/");
    let rest = norm.strip_prefix("daily/")?.strip_suffix(".md")?;
    let parts: Vec<&str> = rest.split('/').collect();
    match parts.as_slice() {
        [file_stem] => SimpleDate::parse_iso(file_stem),
        [year_dir, month_dir, file_stem] => {
            let d = SimpleDate::parse_iso(file_stem)?;
            let yd: i32 = year_dir.parse().ok()?;
            let md: u32 = month_dir.parse().ok()?;
            if yd == d.year && md == d.month {
                Some(d)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Returns true for Year (`daily/YYYY/YYYY.md`) and Month (`daily/YYYY/MM/YYYY-MM.md`) Diary hub notes.
pub fn is_daily_hub_path(rel_path: &str) -> bool {
    let norm = rel_path.trim().replace('\\', "/");
    let Some(rest) = norm.strip_prefix("daily/").and_then(|s| s.strip_suffix(".md")) else {
        return false;
    };
    let parts: Vec<&str> = rest.split('/').collect();
    match parts.as_slice() {
        [y, stem] => y.len() == 4 && y == stem && y.parse::<i32>().is_ok(),
        [y, m, stem] => {
            y.len() == 4
                && m.len() == 2
                && *stem == format!("{y}-{m}")
                && y.parse::<i32>().is_ok()
                && m.parse::<u32>().is_ok_and(|mv| (1..=12).contains(&mv))
        }
        _ => false,
    }
}

/// Builds a 7-column calendar grid (`None` for leading/trailing empty cells).
pub fn month_grid(year: i32, month: u32, week_starts_monday: bool) -> Vec<Option<SimpleDate>> {
    let Some(first) = SimpleDate::from_ymd(year, month, 1) else {
        return Vec::new();
    };
    let mon0 = first.weekday_mon0();
    let leading = if week_starts_monday {
        mon0 as usize
    } else {
        ((mon0 + 1) % 7) as usize
    };
    let total_days = days_in_month(year, month) as usize;
    let total_cells = (leading + total_days).div_ceil(7) * 7;

    let mut grid = Vec::with_capacity(total_cells);
    for _ in 0..leading {
        grid.push(None);
    }
    for d in 1..=total_days as u32 {
        grid.push(SimpleDate::from_ymd(year, month, d));
    }
    while grid.len() < total_cells {
        grid.push(None);
    }
    grid
}

/// Counts current consecutive daily-note streak ending today (or yesterday if today isn't written yet).
pub fn compute_daily_streak(dates: &HashSet<SimpleDate>, today: SimpleDate) -> usize {
    let mut cursor = if dates.contains(&today) {
        today
    } else if dates.contains(&today.prev_day()) {
        today.prev_day()
    } else {
        return 0;
    };

    let mut streak = 0usize;
    while dates.contains(&cursor) {
        streak += 1;
        cursor = cursor.prev_day();
    }
    streak
}

pub fn count_entries_in_month(dates: &HashSet<SimpleDate>, year: i32, month: u32) -> usize {
    dates
        .iter()
        .filter(|d| d.year == year && d.month == month)
        .count()
}

/// Howard Hinnant's civil-to-days algorithm (days since 1970-01-01).
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m_i = i64::from(m);
    let doy = (153 * (m_i + if m_i > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> SimpleDate {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    SimpleDate {
        year,
        month: m,
        day: d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_leap_years_and_weekdays() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2026));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2026, 2), 28);

        assert_eq!(SimpleDate::from_ymd(2026, 11, 19).unwrap().weekday_mon0(), 3); // Thu
        assert_eq!(SimpleDate::from_ymd(2024, 2, 29).unwrap().weekday_mon0(), 3); // Thu
        assert_eq!(SimpleDate::from_ymd(2000, 1, 1).unwrap().weekday_mon0(), 5); // Sat
        assert_eq!(SimpleDate::from_ymd(2026, 12, 31).unwrap().next_day(), SimpleDate::from_ymd(2027, 1, 1).unwrap());
        assert_eq!(SimpleDate::from_ymd(2026, 1, 1).unwrap().prev_day(), SimpleDate::from_ymd(2025, 12, 31).unwrap());
    }

    #[test]
    fn builds_month_grids_and_parses_paths() {
        // 2026-11-01 is Sunday (mon0 = 6)
        let grid_mon = month_grid(2026, 11, true);
        assert_eq!(grid_mon.len(), 42);
        assert!(grid_mon[..6].iter().all(Option::is_none));
        assert_eq!(grid_mon[6], SimpleDate::from_ymd(2026, 11, 1));

        let grid_sun = month_grid(2026, 11, false);
        assert_eq!(grid_sun.len(), 35);
        assert_eq!(grid_sun[0], SimpleDate::from_ymd(2026, 11, 1));

        let d = SimpleDate::from_ymd(2026, 11, 19).unwrap();
        let rel = daily_note_rel_path(d);
        assert_eq!(rel, "daily/2026/11/2026-11-19.md");
        assert_eq!(parse_daily_note_path(&rel), Some(d));
        assert_eq!(
            parse_daily_note_path(&daily_legacy_rel_path(d)),
            Some(d)
        );
        let yh = daily_year_hub_rel_path(2026);
        let mh = daily_month_hub_rel_path(2026, 11);
        assert_eq!(yh, "daily/2026/2026.md");
        assert_eq!(mh, "daily/2026/11/2026-11.md");
        assert!(is_daily_hub_path(&yh));
        assert!(is_daily_hub_path(&mh));
        assert_eq!(parse_daily_note_path(&yh), None);
        assert_eq!(parse_daily_note_path(&mh), None);
        assert_eq!(parse_daily_note_path("welcome.md"), None);
    }

    #[test]
    fn computes_daily_streaks() {
        let d19 = SimpleDate::from_ymd(2026, 11, 19).unwrap();
        let d18 = d19.prev_day();
        let d17 = d18.prev_day();
        let d15 = SimpleDate::from_ymd(2026, 11, 15).unwrap();

        let mut set = HashSet::from([d18, d17, d15]);
        // Today missing yet, yesterday present -> active streak of 2
        assert_eq!(compute_daily_streak(&set, d19), 2);
        set.insert(d19);
        assert_eq!(compute_daily_streak(&set, d19), 3);
        assert_eq!(count_entries_in_month(&set, 2026, 11), 4);

        let d21 = SimpleDate::from_ymd(2026, 11, 21).unwrap();
        assert_eq!(compute_daily_streak(&set, d21), 0);
    }
}
