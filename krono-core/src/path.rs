use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VaultFileKind {
    Markdown,
    Board,
    Table,
    Canvas,
    Asset,
}

pub const IMAGE_EXTENSIONS: [&str; 5] = [".png", ".jpg", ".jpeg", ".webp", ".gif"];

pub fn is_image_ext(path: &str) -> bool {
    let lower = path.trim().to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

impl VaultFileKind {
    pub fn from_path(path: &str) -> Option<Self> {
        let lower = path.trim().to_lowercase();
        if lower.ends_with(".board.json") {
            Some(Self::Board)
        } else if lower.ends_with(".table.json") {
            Some(Self::Table)
        } else if lower.ends_with(".canvas") {
            Some(Self::Canvas)
        } else if lower.ends_with(".md") {
            Some(Self::Markdown)
        } else if is_image_ext(&lower) {
            Some(Self::Asset)
        } else {
            None
        }
    }

    pub fn is_binary(self) -> bool {
        matches!(self, Self::Asset)
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => ".md",
            Self::Board => ".board.json",
            Self::Table => ".table.json",
            Self::Canvas => ".canvas",
            Self::Asset => ".png",
        }
    }
}

/// Strips `.board.json`, `.table.json`, `.canvas`, `.md`, or image extensions from a path/filename stem.
pub fn vault_file_stem(path: &str) -> &str {
    let norm = path.trim().rsplit('/').next().unwrap_or(path);
    let lower = norm.to_lowercase();
    for ext in [
        ".board.json",
        ".table.json",
        ".canvas",
        ".md",
        ".jpeg",
        ".webp",
        ".png",
        ".jpg",
        ".gif",
    ] {
        if lower.ends_with(ext) && norm.len() >= ext.len() {
            return &norm[..norm.len() - ext.len()];
        }
    }
    norm
}

pub fn is_vault_file(path: &str) -> bool {
    VaultFileKind::from_path(path).is_some()
}

/// Validates and normalizes any vault-relative content file (`.md`, `.board.json`,
/// `.table.json`, `.canvas`), rejecting traversal, hidden `.krono`/`.git`/.trash dirs, and drive prefixes.
pub fn validate_vault_path(raw: &str) -> Result<(String, VaultFileKind), String> {
    let replaced = raw.trim().replace('\\', "/");
    if replaced.is_empty() {
        return Err("empty path".to_string());
    }
    if replaced.starts_with('/') || replaced.contains('\0') {
        return Err(format!("invalid leading slash or null byte: {raw}"));
    }
    // Reject Windows drive prefixes like `C:...`
    if replaced.as_bytes().get(1) == Some(&b':') {
        return Err(format!("absolute drive path not allowed: {raw}"));
    }

    let mut clean_segments = Vec::new();
    for seg in replaced.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') {
            return Err(format!("disallowed path segment '{seg}' in '{raw}'"));
        }
        clean_segments.push(seg);
    }

    let normalized = clean_segments.join("/");
    let kind = VaultFileKind::from_path(&normalized)
        .ok_or_else(|| format!("unsupported vault file type: {raw}"))?;
    Ok((normalized, kind))
}

/// Validates and normalizes a vault-relative markdown path (`/` separated).
pub fn validate_rel_path(raw: &str) -> Result<String, String> {
    let (normalized, kind) = validate_vault_path(raw)?;
    if kind != VaultFileKind::Markdown {
        return Err(format!("only .md notes are allowed: {raw}"));
    }
    Ok(normalized)
}

/// Returns groups of paths that differ only by ASCII/Unicode case (dangerous on Windows/macOS).
pub fn detect_case_collisions<'a>(
    paths: impl IntoIterator<Item = &'a str>,
) -> Vec<(String, Vec<String>)> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in paths {
        let norm = p.replace('\\', "/");
        let list = map.entry(norm.to_lowercase()).or_default();
        if !list.iter().any(|existing| existing == &norm) {
            list.push(norm);
        }
    }
    map.into_iter()
        .filter(|(_, variants)| variants.len() >= 2)
        .collect()
}

/// Converts an arbitrary title (Latin or Cyrillic) into a clean filename slug.
pub fn slugify_title(title: &str) -> String {
    let trimmed_input = title.trim();
    let lower = trimmed_input.to_lowercase();
    let stripped = [
        ".board.json",
        ".table.json",
        ".canvas",
        ".md",
        ".jpeg",
        ".webp",
        ".png",
        ".jpg",
        ".gif",
    ]
        .iter()
        .find_map(|ext| {
            if lower.ends_with(ext) && trimmed_input.len() >= ext.len() {
                Some(&trimmed_input[..trimmed_input.len() - ext.len()])
            } else {
                None
            }
        })
        .unwrap_or(trimmed_input);

    let mut slug = String::new();
    let mut prev_dash = false;

    for ch in stripped.chars() {
        if ch.is_alphanumeric() {
            for lc in ch.to_lowercase() {
                slug.push(lc);
            }
            prev_dash = false;
        } else if (ch == '-' || ch == '_' || ch.is_whitespace()) && !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }

    let mut trimmed: String = slug
        .trim_matches('-')
        .chars()
        .take(80)
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if trimmed.is_empty() {
        return "untitled".to_string();
    }
    let is_win_reserved = matches!(
        trimmed.as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    );
    if is_win_reserved {
        trimmed.push_str("-note");
    }
    trimmed
}

/// Generates a guaranteed-unique relative path with custom extension (`<slug><ext>`, `<slug>-2<ext>`, ...).
pub fn unique_vault_path(
    folder: Option<&str>,
    title: &str,
    ext: &str,
    existing: &HashSet<String>,
) -> String {
    let slug = slugify_title(title);
    let clean_ext = if ext.starts_with('.') {
        ext.to_string()
    } else {
        format!(".{ext}")
    };
    let prefix = folder
        .map(|f| f.trim().trim_matches('/').replace('\\', "/"))
        .filter(|f| !f.is_empty());

    let make_candidate = |suffix: Option<usize>| -> String {
        let filename = match suffix {
            None => format!("{slug}{clean_ext}"),
            Some(n) => format!("{slug}-{n}{clean_ext}"),
        };
        match &prefix {
            Some(dir) => format!("{dir}/{filename}"),
            None => filename,
        }
    };

    let first = make_candidate(None);
    let first_lower = first.to_lowercase();
    if !existing.iter().any(|p| p.to_lowercase() == first_lower) {
        return first;
    }
    let mut counter = 2usize;
    loop {
        let cand = make_candidate(Some(counter));
        let cand_lower = cand.to_lowercase();
        if !existing.iter().any(|p| p.to_lowercase() == cand_lower) {
            return cand;
        }
        counter += 1;
    }
}

/// Generates a guaranteed-unique relative path (`<slug>.md`, `<slug>-2.md`, ...).
pub fn unique_note_path(
    folder: Option<&str>,
    title: &str,
    existing: &HashSet<String>,
) -> String {
    unique_vault_path(folder, title, ".md", existing)
}

/// Percent-encodes each path segment per RFC 3986 while preserving `/` separators.
pub fn encode_url_path(rel_path: &str) -> String {
    rel_path
        .split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode_segment(seg: &str) -> String {
    let mut out = String::with_capacity(seg.len());
    for &b in seg.as_bytes() {
        let is_unreserved =
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if is_unreserved {
            out.push(b as char);
        } else {
            use std::fmt::Write;
            let _ = write!(&mut out, "%{b:02X}");
        }
    }
    out
}

/// Turns `dir/note.md` (or `.board.json` / `.table.json` / `.canvas`) into
/// `dir/note.conflict-YYYYMMDD-HHMM.ext`.
pub fn conflict_path(rel_path: &str, timestamp: &str) -> String {
    let norm = rel_path.replace('\\', "/");
    let lower = norm.to_lowercase();
    for ext in [
        ".board.json",
        ".table.json",
        ".canvas",
        ".md",
        ".jpeg",
        ".webp",
        ".png",
        ".jpg",
        ".gif",
    ] {
        if lower.ends_with(ext) && norm.len() >= ext.len() {
            let split_at = norm.len() - ext.len();
            let stem = &norm[..split_at];
            let orig_ext = &norm[split_at..];
            return format!("{stem}.conflict-{timestamp}{orig_ext}");
        }
    }
    format!("{norm}.conflict-{timestamp}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_paths_strictly() {
        assert_eq!(validate_rel_path("notes\\test.md").unwrap(), "notes/test.md");
        assert!(validate_rel_path("../secret.md").is_err());
        assert!(validate_rel_path("/abs.md").is_err());
        assert!(validate_rel_path(".krono/index.md").is_err());
        assert!(validate_rel_path("notes/image.png").is_err());
        assert!(validate_rel_path("C:/bad.md").is_err());
    }

    #[test]
    fn deduplicates_note_paths_with_cyrillic_slugs() {
        let mut existing = HashSet::new();
        let p1 = unique_note_path(Some("projects"), "Идеи 2026", &existing);
        assert_eq!(p1, "projects/идеи-2026.md");
        existing.insert(p1);

        let p2 = unique_note_path(Some("projects"), "Идеи 2026", &existing);
        assert_eq!(p2, "projects/идеи-2026-2.md");
    }

    #[test]
    fn encodes_url_segments_safely() {
        assert_eq!(
            encode_url_path("заметки/hello world?.md"),
            "%D0%B7%D0%B0%D0%BC%D0%B5%D1%82%D0%BA%D0%B8/hello%20world%3F.md"
        );
    }

    #[test]
    fn formats_conflict_path() {
        assert_eq!(
            conflict_path("docs/plan.md", "20261119-1800"),
            "docs/plan.conflict-20261119-1800.md"
        );
        assert_eq!(
            conflict_path("boards/yt.board.json", "20261119-1800"),
            "boards/yt.conflict-20261119-1800.board.json"
        );
        assert_eq!(
            conflict_path("scripts/hook.canvas", "20261119-1800"),
            "scripts/hook.conflict-20261119-1800.canvas"
        );
    }

    #[test]
    fn validates_vault_file_kinds_and_case_collisions() {
        assert_eq!(
            validate_vault_path("boards\\yt.board.json").unwrap(),
            ("boards/yt.board.json".into(), VaultFileKind::Board)
        );
        assert_eq!(
            validate_vault_path("tables/tasks.table.json").unwrap(),
            ("tables/tasks.table.json".into(), VaultFileKind::Table)
        );
        assert_eq!(
            validate_vault_path("scripts/ep1.canvas").unwrap(),
            ("scripts/ep1.canvas".into(), VaultFileKind::Canvas)
        );
        for (img_path, expected_norm) in [
            ("assets\\shot.png", "assets/shot.png"),
            ("assets/photo.jpg", "assets/photo.jpg"),
            ("assets/frame.jpeg", "assets/frame.jpeg"),
            ("assets/pic.webp", "assets/pic.webp"),
            ("assets/anim.gif", "assets/anim.gif"),
        ] {
            let (norm, kind) = validate_vault_path(img_path).unwrap();
            assert_eq!(norm, expected_norm);
            assert_eq!(kind, VaultFileKind::Asset);
            assert!(kind.is_binary());
        }
        assert!(validate_vault_path("../evil.png").is_err());
        assert!(validate_vault_path(".hidden/a.png").is_err());
        assert!(is_vault_file("a.md"));
        assert!(is_vault_file("img.png"));
        assert!(!is_vault_file("archive.zip"));

        let cols = detect_case_collisions(["Note.md", "note.md", "other.md"]);
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].0, "note.md");
        assert_eq!(cols[0].1, vec!["Note.md", "note.md"]);
    }

    #[test]
    fn slugifies_and_deduplicates_collection_paths() {
        assert_eq!(vault_file_stem("boards/YouTube.board.json"), "YouTube");
        assert_eq!(vault_file_stem("задачи.table.json"), "задачи");
        assert_eq!(slugify_title("Контент план.board.json"), "контент-план");

        let mut existing = HashSet::new();
        let b1 = unique_vault_path(Some("youtube"), "Контент план", ".board.json", &existing);
        assert_eq!(b1, "youtube/контент-план.board.json");
        existing.insert(b1);

        let b2 = unique_vault_path(Some("youtube"), "Контент план", ".board.json", &existing);
        assert_eq!(b2, "youtube/контент-план-2.board.json");

        let t1 = unique_vault_path(None, "Подписки", ".table.json", &existing);
        assert_eq!(t1, "подписки.table.json");
    }

    #[test]
    fn slugify_handles_reserved_names_cyrillic_and_collisions() {
        assert_eq!(slugify_title("CON"), "con-note");
        assert_eq!(slugify_title("lpt1"), "lpt1-note");
        let long_title = "Очень длинный заголовок заметки ".repeat(6);
        assert!(slugify_title(&long_title).chars().count() <= 80);

        let mut existing = HashSet::new();
        existing.insert("папка/План.md".to_string());
        let p2 = unique_note_path(Some("папка"), "план", &existing);
        assert_eq!(p2, "папка/план-2.md");
        existing.insert(p2);
        let p3 = unique_note_path(Some("папка"), "План", &existing);
        assert_eq!(p3, "папка/план-3.md");
    }
}
