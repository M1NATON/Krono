use crate::config::write_file_atomic;
use crate::i18n::{format_daily_month_title, format_daily_title, format_daily_year_title, Lang};
use chrono::Datelike;
use krono_core::{
    canvas::CanvasDoc,
    collection::{CollectionDoc, CollectionViewKind},
    daily::{
        daily_month_hub_rel_path, daily_note_rel_path, daily_year_hub_rel_path,
        parse_daily_note_path, SimpleDate,
    },
    markdown::rewrite_wikilinks,
    note::{normalize_tags, Frontmatter, NoteDocument, NoteStatus},
    path::{
        slugify_title, unique_note_path, unique_vault_path, validate_rel_path,
        validate_vault_path, vault_file_stem, VaultFileKind,
    },
    templates::{english_lesson_template, render_template, sanitize_template_id, TemplateVars},
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, Debug)]
pub struct DiskFileInfo {
    pub rel_path: String,
    pub full_path: PathBuf,
    pub modified: i64,
    pub size: u64,
    pub kind: VaultFileKind,
}

fn is_hidden(entry: &DirEntry) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Returns `(mtime_ms, byte_len)` using millisecond precision so rapid edits within 1s are caught.
pub fn file_mtime_and_size(path: &Path) -> (i64, u64) {
    match fs::metadata(path) {
        Ok(meta) => {
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            (mtime_ms, meta.len())
        }
        Err(_) => (0, 0),
    }
}

pub fn list_vault_files(vault_dir: &Path) -> Vec<DiskFileInfo> {
    let mut out = Vec::new();
    let walker = WalkDir::new(vault_dir)
        .into_iter()
        .filter_entry(|e| !is_hidden(e));

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(stripped) = entry.path().strip_prefix(vault_dir) else {
            continue;
        };
        let raw_rel = stripped.to_string_lossy().replace('\\', "/");
        if let Ok((rel_path, kind)) = validate_vault_path(&raw_rel) {
            let (modified, size) = file_mtime_and_size(entry.path());
            out.push(DiskFileInfo {
                rel_path,
                full_path: entry.path().to_path_buf(),
                modified,
                size,
                kind,
            });
        }
    }
    out
}

pub fn list_markdown_files(vault_dir: &Path) -> Vec<DiskFileInfo> {
    list_vault_files(vault_dir)
        .into_iter()
        .filter(|f| f.kind == VaultFileKind::Markdown)
        .collect()
}

pub fn list_collection_files(vault_dir: &Path) -> Vec<DiskFileInfo> {
    list_vault_files(vault_dir)
        .into_iter()
        .filter(|f| matches!(f.kind, VaultFileKind::Board | VaultFileKind::Table))
        .collect()
}

pub fn list_canvas_files(vault_dir: &Path) -> Vec<DiskFileInfo> {
    list_vault_files(vault_dir)
        .into_iter()
        .filter(|f| f.kind == VaultFileKind::Canvas)
        .collect()
}

pub fn full_vault_path(vault_dir: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let (clean, _) = validate_vault_path(rel_path)?;
    let mut full = vault_dir.to_path_buf();
    for seg in clean.split('/') {
        full.push(seg);
    }
    Ok(full)
}

pub fn full_note_path(vault_dir: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let clean = validate_rel_path(rel_path)?;
    let mut full = vault_dir.to_path_buf();
    for seg in clean.split('/') {
        full.push(seg);
    }
    Ok(full)
}

pub fn write_note_bytes_atomic(
    vault_dir: &Path,
    rel_path: &str,
    bytes: &[u8],
) -> Result<(i64, u64), String> {
    let full = full_vault_path(vault_dir, rel_path)?;
    write_file_atomic(&full, bytes)?;
    Ok(file_mtime_and_size(&full))
}

pub fn read_note(vault_dir: &Path, rel_path: &str) -> Result<(NoteDocument, i64, u64), String> {
    let full = full_note_path(vault_dir, rel_path)?;
    let raw = fs::read_to_string(&full).map_err(|e| format!("read '{rel_path}': {e}"))?;
    let stem = full
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());
    let doc = NoteDocument::parse(&raw, &stem);
    let (modified, size) = file_mtime_and_size(&full);
    Ok((doc, modified, size))
}

pub fn save_note(
    vault_dir: &Path,
    rel_path: &str,
    doc: &NoteDocument,
) -> Result<(i64, u64), String> {
    let serialized = doc.serialize();
    write_note_bytes_atomic(vault_dir, rel_path, serialized.as_bytes())
}

/// Parses optional `folder/sub/Title` input into `(Option<folder_rel>, title)`.
fn split_folder_and_title(raw_input: &str) -> (Option<String>, String) {
    let norm = raw_input.trim().replace('\\', "/");
    if let Some((folder_part, title_part)) = norm.rsplit_once('/') {
        let clean_segs: Vec<String> = folder_part
            .split('/')
            .map(slugify_title)
            .filter(|s| !s.is_empty() && s != "untitled")
            .collect();
        let title = title_part.trim();
        let clean_title = if title.is_empty() {
            "Untitled".to_string()
        } else {
            title.to_string()
        };
        let folder = if clean_segs.is_empty() {
            None
        } else {
            Some(clean_segs.join("/"))
        };
        (folder, clean_title)
    } else {
        let title = if norm.is_empty() {
            "Untitled".to_string()
        } else {
            norm
        };
        (None, title)
    }
}

pub fn note_preset_body(preset_id: &str, title: &str, lang: Lang) -> (Option<NoteStatus>, Vec<String>, String) {
    match (preset_id, lang) {
        ("meeting", Lang::Ru) => (
            None,
            vec!["meeting".into()],
            format!(
                "# {title}\n\n**Участники:** \n\n## Повестка\n- \n\n## Решения и договорённости\n- \n\n## Задачи (Action Items)\n- [ ] \n"
            ),
        ),
        ("meeting", Lang::En) => (
            None,
            vec!["meeting".into()],
            format!(
                "# {title}\n\n**Attendees:** \n\n## Agenda\n- \n\n## Key Decisions\n- \n\n## Action Items\n- [ ] \n"
            ),
        ),
        ("concept", Lang::Ru) => (
            None,
            vec!["idea".into()],
            format!(
                "# {title}\n\n## Главная мысль\n> \n\n## Ключевые тезисы\n- \n\n## Вопросы и связи\nСм. также: [[]]\n"
            ),
        ),
        ("concept", Lang::En) => (
            None,
            vec!["idea".into()],
            format!(
                "# {title}\n\n## Core Idea\n> \n\n## Key Points\n- \n\n## Questions & Links\nRelated: [[]]\n"
            ),
        ),
        ("script", Lang::Ru) => (
            None,
            vec!["draft".into()],
            format!(
                "# {title}\n\n## Хук (Первые 15 секунд)\n\n## Структура\n1. Вступление\n2. Основная часть\n3. Вывод и CTA\n\n## Черновик текста\n\n\n## Чек-лист перед публикацией\n- [ ] Проверить тезисы\n- [ ] Обложка и заголовок\n"
            ),
        ),
        ("script", Lang::En) => (
            None,
            vec!["draft".into()],
            format!(
                "# {title}\n\n## Opening Hook\n\n## Outline\n1. Intro\n2. Deep Dive\n3. Summary & CTA\n\n## Draft\n\n\n## Pre-publish Checklist\n- [ ] Fact check\n- [ ] Title & Thumbnail\n"
            ),
        ),
        ("english_lesson", _) => {
            let (tags, body) = english_lesson_template(title, lang == Lang::Ru);
            (None, tags, body)
        }
        _ => (None, Vec::new(), String::new()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomTemplateInfo {
    pub id: String,
    pub name: String,
    pub rel_path: String,
}

fn templates_dir(vault_dir: &Path) -> PathBuf {
    vault_dir.join(".krono").join("templates")
}

pub fn list_custom_templates(vault_dir: &Path) -> Vec<CustomTemplateInfo> {
    let dir = templates_dir(vault_dir);
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(fname) = file_name.to_str() else {
            continue;
        };
        let Some(id) = sanitize_template_id(fname) else {
            continue;
        };
        let name = id.replace(['_', '-'], " ");
        out.push(CustomTemplateInfo {
            rel_path: format!(".krono/templates/{fname}"),
            name,
            id,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn read_custom_template(vault_dir: &Path, id: &str) -> Result<String, String> {
    let clean_id = sanitize_template_id(&format!("{id}.md"))
        .ok_or_else(|| "invalid template id".to_string())?;
    let path = templates_dir(vault_dir).join(format!("{clean_id}.md"));
    fs::read_to_string(&path).map_err(|e| format!("read template {clean_id}: {e}"))
}

fn weekday_label(lang: Lang, date: SimpleDate) -> &'static str {
    match (lang, date.weekday_mon0()) {
        (Lang::Ru, 0) => "Понедельник",
        (Lang::Ru, 1) => "Вторник",
        (Lang::Ru, 2) => "Среда",
        (Lang::Ru, 3) => "Четверг",
        (Lang::Ru, 4) => "Пятница",
        (Lang::Ru, 5) => "Суббота",
        (Lang::Ru, _) => "Воскресенье",
        (Lang::En, 0) => "Monday",
        (Lang::En, 1) => "Tuesday",
        (Lang::En, 2) => "Wednesday",
        (Lang::En, 3) => "Thursday",
        (Lang::En, 4) => "Friday",
        (Lang::En, 5) => "Saturday",
        (Lang::En, _) => "Sunday",
    }
}

#[cfg(test)]
pub fn create_note(
    vault_dir: &Path,
    raw_title_or_path: &str,
    status: Option<NoteStatus>,
    initial_body: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    create_note_with_tags(
        vault_dir,
        raw_title_or_path,
        status,
        Vec::new(),
        initial_body,
        existing_paths,
    )
}

pub fn create_note_with_tags(
    vault_dir: &Path,
    raw_title_or_path: &str,
    status: Option<NoteStatus>,
    tags: Vec<String>,
    initial_body: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    let (folder, title) = split_folder_and_title(raw_title_or_path);
    let rel_path = unique_note_path(folder.as_deref(), &title, existing_paths);
    let mut fm = Frontmatter::new(title);
    fm.status = status;
    fm.tags = normalize_tags(tags);
    let doc = NoteDocument {
        frontmatter: fm,
        body: initial_body.to_string(),
    };
    let (modified, size) = save_note(vault_dir, &rel_path, &doc)?;
    Ok((rel_path, doc, modified, size))
}

pub fn create_note_with_preset(
    vault_dir: &Path,
    raw_title_or_path: &str,
    preset_id: &str,
    lang: Lang,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    let (_, title) = split_folder_and_title(raw_title_or_path);
    if let Some(custom_id) = preset_id.strip_prefix("custom:") {
        let raw = read_custom_template(vault_dir, custom_id)?;
        let now = chrono::Local::now().date_naive();
        let today = SimpleDate::from_ymd(now.year(), now.month(), now.day()).unwrap_or(SimpleDate {
            year: 2026,
            month: 1,
            day: 1,
        });
        let date_str = today.format_iso();
        let month_str = format_daily_month_title(lang, today.year, today.month);
        let rendered = render_template(
            &raw,
            &TemplateVars {
                title: &title,
                date: &date_str,
                month: &month_str,
                weekday: weekday_label(lang, today),
            },
        );
        let parsed = NoteDocument::parse(&rendered, &title);
        return create_note_with_tags(
            vault_dir,
            raw_title_or_path,
            parsed.frontmatter.status,
            parsed.frontmatter.tags,
            &parsed.body,
            existing_paths,
        );
    }
    let (status, tags, body) = note_preset_body(preset_id, &title, lang);
    create_note_with_tags(vault_dir, raw_title_or_path, status, tags, &body, existing_paths)
}

pub fn create_language_lesson_note(
    vault_dir: &Path,
    raw_title_or_path: &str,
    study_language: &str,
    lang: Lang,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    let (_, title) = split_folder_and_title(raw_title_or_path);
    let (tags, body) =
        krono_core::templates::language_lesson_template(&title, study_language, lang == Lang::Ru);
    create_note_with_tags(vault_dir, raw_title_or_path, None, tags, &body, existing_paths)
}

pub fn rename_note(
    vault_dir: &Path,
    old_rel_path: &str,
    new_title: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    let clean_old = validate_rel_path(old_rel_path)?;
    let (mut doc, _, _) = read_note(vault_dir, &clean_old)?;
    let clean_title = new_title.trim();
    if clean_title.is_empty() {
        return Err("title cannot be empty".to_string());
    }

    let folder = clean_old
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string());

    let mut filtered_existing = existing_paths.clone();
    filtered_existing.remove(&clean_old);

    let new_rel_path = unique_note_path(folder.as_deref(), clean_title, &filtered_existing);
    doc.frontmatter.title = clean_title.to_string();

    if new_rel_path == clean_old {
        let (modified, size) = save_note(vault_dir, &clean_old, &doc)?;
        return Ok((clean_old, doc, modified, size));
    }

    let (modified, size) = save_note(vault_dir, &new_rel_path, &doc)?;
    let old_full = full_note_path(vault_dir, &clean_old)?;
    if old_full.exists() {
        let _ = fs::remove_file(&old_full);
    }
    Ok((new_rel_path, doc, modified, size))
}

pub fn duplicate_note(
    vault_dir: &Path,
    rel_path: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, NoteDocument, i64, u64), String> {
    let (mut doc, _, _) = read_note(vault_dir, rel_path)?;
    let folder = rel_path.rsplit_once('/').map(|(d, _)| d.to_string());
    let copy_title = format!("{} (Copy)", doc.frontmatter.title);
    let new_rel = unique_note_path(folder.as_deref(), &copy_title, existing_paths);
    doc.frontmatter.title = copy_title;
    let (modified, size) = save_note(vault_dir, &new_rel, &doc)?;
    Ok((new_rel, doc, modified, size))
}

/// Soft-deletes a vault note by moving it to `.trash/<timestamp>_<name>` and pruning empty folders.
pub fn delete_note(vault_dir: &Path, rel_path: &str) -> Result<(), String> {
    let (clean, _) = validate_vault_path(rel_path)?;
    let full = {
        let mut p = vault_dir.to_path_buf();
        for seg in clean.split('/') {
            p.push(seg);
        }
        p
    };
    if full.exists() {
        let trash_dir = vault_dir.join(".trash");
        let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S-%3f");
        let flat = clean.replace('/', "__");
        let trash_dest = trash_dir.join(format!("{ts}_{flat}"));
        if fs::create_dir_all(&trash_dir).is_ok() && fs::rename(&full, &trash_dest).is_err() {
            if fs::copy(&full, &trash_dest).is_ok() {
                fs::remove_file(&full).map_err(|e| format!("delete '{rel_path}': {e}"))?;
            } else {
                fs::remove_file(&full).map_err(|e| format!("delete '{rel_path}': {e}"))?;
            }
        }
    }
    Ok(())
}

/// Explicitly deletes a vault folder (moving any contained vault files to `.trash` first).
pub fn delete_vault_folder(
    vault_dir: &Path,
    rel_folder: &str,
) -> Result<Vec<(String, VaultFileKind)>, String> {
    let clean = rel_folder.trim().replace('\\', "/");
    let clean = clean.trim_matches('/').to_string();
    if clean.is_empty()
        || clean.contains('\0')
        || clean.as_bytes().get(1) == Some(&b':')
        || clean
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.'))
    {
        return Err(format!("invalid folder path: {rel_folder}"));
    }
    if clean.eq_ignore_ascii_case("daily") {
        return Err("cannot delete daily root folder".to_string());
    }

    let mut full_dir = vault_dir.to_path_buf();
    for seg in clean.split('/') {
        full_dir.push(seg);
    }
    if !full_dir.is_dir() {
        return Err(format!("folder not found: {clean}"));
    }

    let prefix = format!("{clean}/");
    let files: Vec<(String, VaultFileKind)> = list_vault_files(vault_dir)
        .into_iter()
        .filter(|f| f.rel_path.starts_with(&prefix))
        .map(|f| (f.rel_path, f.kind))
        .collect();

    for (rel_path, _) in &files {
        delete_note(vault_dir, rel_path)?;
    }
    fs::remove_dir_all(&full_dir).map_err(|e| format!("delete folder '{clean}': {e}"))?;
    Ok(files)
}

pub fn ensure_starter_vault(vault_dir: &Path, lang: Lang) -> Result<(), String> {
    fs::create_dir_all(vault_dir).map_err(|e| format!("create vault dir: {e}"))?;
    if !list_markdown_files(vault_dir).is_empty() {
        return Ok(());
    }

    let notes: &[(&str, &str)] = match lang {
        Lang::Ru => &[
            (
                "welcome.md",
                "---\ntitle: Добро пожаловать в Krono\nstatus: In Progress\ntags: krono, pkm, guide\n---\n\n# Добро пожаловать в Krono\n\nНативное рабочее пространство для заметок и задач без Electron и WebView — на OpenGL, SQLite FTS5 и прямом хранении в Markdown.\n\n## Что нового в v0.2\n- [x] Быстрый нативный интерфейс **Obsidian Ink** и **Nordic Paper**\n- [x] Прямая синхронизация с приватным репозиторием GitHub\n- [ ] Организовать личную базу знаний через `[[ссылки]]`\n\n> Нажмите `Ctrl+K` (или `Cmd+K` на macOS) для мгновенного поиска по всем заметкам и командам.\n\nСвязанные заметки: [[Architecture]] и [[projects/roadmap|Дорожная карта]].\n",
            ),
            (
                "architecture.md",
                "---\ntitle: Architecture\nstatus: Done\ntags: rust, sqlite, github\n---\n\n# Архитектура Krono\n\nВсе заметки хранятся в обычных `.md` файлах с YAML-свойствами, а кэш поиска и граф связей живут в локальной базе `.krono/index.db`.\n\n## Синхронизация через GitHub\nKrono использует **Git Data API** (Blobs + Trees + Commits) для 3-way merge без необходимости устанавливать `git` или арендовать VPS.\n\n```rust\nlet blob_sha = krono_core::hash::git_blob_sha1(content.as_bytes());\n```\n\nНазад к [[Добро пожаловать в Krono]].\n",
            ),
            (
                "projects/roadmap.md",
                "---\ntitle: Дорожная карта\nstatus: Todo\ntags: planning, release\n---\n\n# План развития\n\n- [x] Переписать UI на кастомных векторных компонентах\n- [x] Добавить переключение тем Dark / Light и языков RU / EN\n- [ ] Настроить приватный репозиторий GitHub в настройках\n\nПодробности устройства описаны в [[Architecture]].\n",
            ),
        ],
        Lang::En => &[
            (
                "welcome.md",
                "---\ntitle: Welcome to Krono\nstatus: In Progress\ntags: krono, pkm, guide\n---\n\n# Welcome to Krono\n\nA fast native workspace for notes and tasks without Electron or WebView — powered by OpenGL, SQLite FTS5, and clean Markdown files.\n\n## Highlights in v0.2\n- [x] Native **Obsidian Ink** and **Nordic Paper** themes\n- [x] Direct 3-way sync with your private GitHub repository\n- [ ] Connect ideas using `[[wikilinks]]`\n\n> Press `Ctrl+K` (or `Cmd+K` on macOS) to open the Command Palette.\n\nExplore [[Architecture]] and [[projects/roadmap|Roadmap]].\n",
            ),
            (
                "architecture.md",
                "---\ntitle: Architecture\nstatus: Done\ntags: rust, sqlite, github\n---\n\n# Krono Architecture\n\nNotes live as plain `.md` files with lossless YAML frontmatter, while full-text search and link graphs are cached locally in `.krono/index.db`.\n\n## Private GitHub Sync\nKrono uses the **Git Data API** (Blobs + Trees + Commits) for deterministic 3-way merging with zero VPS or local `git` CLI dependencies.\n\n```rust\nlet blob_sha = krono_core::hash::git_blob_sha1(content.as_bytes());\n```\n\nBack to [[Welcome to Krono]].\n",
            ),
            (
                "projects/roadmap.md",
                "---\ntitle: Roadmap\nstatus: Todo\ntags: planning, release\n---\n\n# Product Roadmap\n\n- [x] Custom vector UI system and bundled typography\n- [x] Instant Dark / Light themes and RU / EN localization\n- [ ] Connect your private GitHub repository in Settings\n\nSee technical notes in [[Architecture]].\n",
            ),
        ],
    };

    for (rel, body) in notes {
        let _ = write_note_bytes_atomic(vault_dir, rel, body.as_bytes())?;
    }
    Ok(())
}

/// Rewrites incoming `[[wikilinks]]` across all markdown files in the vault except `skip_rel_path`.
pub fn rewrite_vault_wikilinks(
    vault_dir: &Path,
    skip_rel_path: &str,
    old_targets: &[&str],
    new_target: &str,
) -> Result<(Vec<(String, NoteDocument, i64, u64)>, usize), String> {
    let mut updated_notes = Vec::new();
    let mut total_rewrites = 0usize;

    for info in list_markdown_files(vault_dir) {
        if info.rel_path == skip_rel_path {
            continue;
        }
        let Ok((mut doc, _, _)) = read_note(vault_dir, &info.rel_path) else {
            continue;
        };
        let (new_body, count) = rewrite_wikilinks(&doc.body, old_targets, new_target);
        if count > 0 {
            doc.body = new_body;
            let (modified, size) = save_note(vault_dir, &info.rel_path, &doc)?;
            total_rewrites += count;
            updated_notes.push((info.rel_path, doc, modified, size));
        }
    }

    Ok((updated_notes, total_rewrites))
}

pub struct DailyCreationResult {
    pub day_path: String,
    pub day_doc: NoteDocument,
    pub day_modified: i64,
    pub day_size: u64,
    pub hubs: Vec<(String, NoteDocument, i64, u64)>,
}

/// Ensures `daily/YYYY/YYYY.md` and `daily/YYYY/MM/YYYY-MM.md` hubs exist and link Year <-> Month <-> Day.
pub fn ensure_daily_hubs_for_date(
    vault_dir: &Path,
    date: SimpleDate,
    lang: Lang,
    day_title: &str,
) -> Result<Vec<(String, NoteDocument, i64, u64)>, String> {
    let mut touched = Vec::new();
    let year_path = daily_year_hub_rel_path(date.year);
    let month_path = daily_month_hub_rel_path(date.year, date.month);
    let year_title = format_daily_year_title(lang, date.year);
    let month_title = format_daily_month_title(lang, date.year, date.month);

    // 1. Year hub -> links each Month
    let month_bullet = format!("- [[{month_title}]]");
    let (mut year_doc, year_existed) = match read_note(vault_dir, &year_path) {
        Ok((doc, _, _)) => (doc, true),
        Err(_) => {
            let mut fm = Frontmatter::new(year_title.clone());
            fm.status = None;
            fm.tags = vec!["daily".into(), "year".into()];
            let heading = match lang {
                Lang::Ru => format!("# {year_title}\n\n## Месяцы\n{month_bullet}\n"),
                Lang::En => format!("# {year_title}\n\n## Months\n{month_bullet}\n"),
            };
            (NoteDocument { frontmatter: fm, body: heading }, false)
        }
    };
    let mut year_changed = !year_existed;
    if year_existed && !year_doc.body.contains(&format!("[[{month_title}]]")) {
        if !year_doc.body.ends_with('\n') && !year_doc.body.is_empty() {
            year_doc.body.push('\n');
        }
        year_doc.body.push_str(&month_bullet);
        year_doc.body.push('\n');
        year_changed = true;
    }
    if year_changed {
        let (m, sz) = save_note(vault_dir, &year_path, &year_doc)?;
        touched.push((year_path, year_doc, m, sz));
    }

    // 2. Month hub -> links Year and every Day in that Month
    let day_bullet = format!("- [[{day_title}]]");
    let (mut month_doc, month_existed) = match read_note(vault_dir, &month_path) {
        Ok((doc, _, _)) => (doc, true),
        Err(_) => {
            let mut fm = Frontmatter::new(month_title.clone());
            fm.status = None;
            fm.tags = vec!["daily".into(), "month".into()];
            let body = match lang {
                Lang::Ru => format!(
                    "# {month_title}\n\nГод: [[{year_title}]]\n\n## Дни месяца\n{day_bullet}\n"
                ),
                Lang::En => format!(
                    "# {month_title}\n\nYear: [[{year_title}]]\n\n## Days\n{day_bullet}\n"
                ),
            };
            (NoteDocument { frontmatter: fm, body }, false)
        }
    };
    let mut month_changed = !month_existed;
    if month_existed && !month_doc.body.contains(&format!("[[{day_title}]]")) {
        if !month_doc.body.ends_with('\n') && !month_doc.body.is_empty() {
            month_doc.body.push('\n');
        }
        month_doc.body.push_str(&day_bullet);
        month_doc.body.push('\n');
        month_changed = true;
    }
    if month_changed {
        let (m, sz) = save_note(vault_dir, &month_path, &month_doc)?;
        touched.push((month_path, month_doc, m, sz));
    }

    Ok(touched)
}

#[cfg(test)]
pub fn create_daily_note(
    vault_dir: &Path,
    date: SimpleDate,
    lang: Lang,
) -> Result<DailyCreationResult, String> {
    create_daily_note_with_template(vault_dir, date, lang, None)
}

pub fn create_daily_note_with_template(
    vault_dir: &Path,
    date: SimpleDate,
    lang: Lang,
    template_id: Option<&str>,
) -> Result<DailyCreationResult, String> {
    let rel_path = daily_note_rel_path(date);
    let title = format_daily_title(lang, date);
    let month_title = format_daily_month_title(lang, date.year, date.month);

    let mut fm = Frontmatter::new(title.clone());
    fm.status = None;
    fm.tags = vec!["daily".to_string()];

    let custom_rendered = template_id
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .and_then(|id| read_custom_template(vault_dir, id).ok())
        .map(|raw| {
            let date_str = date.format_iso();
            render_template(
                &raw,
                &TemplateVars {
                    title: &title,
                    date: &date_str,
                    month: &month_title,
                    weekday: weekday_label(lang, date),
                },
            )
        });

    let body = if let Some(rendered) = custom_rendered {
        let parsed = NoteDocument::parse(&rendered, &title);
        if !parsed.frontmatter.tags.is_empty() {
            let mut tags = vec!["daily".to_string()];
            tags.extend(parsed.frontmatter.tags);
            fm.tags = normalize_tags(tags);
        }
        parsed.body
    } else {
        match lang {
            Lang::Ru => format!(
                "# {title}\n\nМесяц: [[{month_title}]]\n\n## План на день\n- [ ] \n\n## Заметки\n\n\n## Итоги дня\n- Что получилось хорошо:\n- Над чем поработать завтра:\n"
            ),
            Lang::En => format!(
                "# {title}\n\nMonth: [[{month_title}]]\n\n## Focus Today\n- [ ] \n\n## Notes\n\n\n## Daily Reflection\n- Wins:\n- Tomorrow's priority:\n"
            ),
        }
    };

    let doc = NoteDocument {
        frontmatter: fm,
        body,
    };
    let (modified, size) = save_note(vault_dir, &rel_path, &doc)?;
    let hubs = ensure_daily_hubs_for_date(vault_dir, date, lang, &title)?;
    Ok(DailyCreationResult {
        day_path: rel_path,
        day_doc: doc,
        day_modified: modified,
        day_size: size,
        hubs,
    })
}

pub fn read_collection(
    vault_dir: &Path,
    rel_path: &str,
) -> Result<(CollectionDoc, i64, u64, VaultFileKind), String> {
    let (clean, kind) = validate_vault_path(rel_path)?;
    if !matches!(kind, VaultFileKind::Board | VaultFileKind::Table) {
        return Err(format!("not a collection file: {rel_path}"));
    }
    let full = full_vault_path(vault_dir, &clean)?;
    let raw = fs::read_to_string(&full).map_err(|e| format!("read '{clean}': {e}"))?;
    let stem = vault_file_stem(&clean);
    let fallback_view = match kind {
        VaultFileKind::Table => CollectionViewKind::Table,
        _ => CollectionViewKind::Board,
    };
    let doc = CollectionDoc::parse(&raw, stem, fallback_view)?;
    let (modified, size) = file_mtime_and_size(&full);
    Ok((doc, modified, size, kind))
}

pub fn save_collection(
    vault_dir: &Path,
    rel_path: &str,
    doc: &CollectionDoc,
) -> Result<(i64, u64), String> {
    let (_, kind) = validate_vault_path(rel_path)?;
    if !matches!(kind, VaultFileKind::Board | VaultFileKind::Table) {
        return Err(format!("not a collection file: {rel_path}"));
    }
    let serialized = doc.serialize_pretty()?;
    write_note_bytes_atomic(vault_dir, rel_path, serialized.as_bytes())
}

pub fn create_collection(
    vault_dir: &Path,
    raw_title_or_path: &str,
    view_kind: CollectionViewKind,
    is_ru: bool,
    existing_paths: &HashSet<String>,
) -> Result<(String, VaultFileKind, CollectionDoc, i64, u64), String> {
    let default_preset = match view_kind {
        CollectionViewKind::Board => "kanban",
        CollectionViewKind::Table | CollectionViewKind::Gallery => "general",
    };
    create_collection_preset(
        vault_dir,
        raw_title_or_path,
        view_kind,
        default_preset,
        is_ru,
        existing_paths,
    )
}

pub fn create_collection_preset(
    vault_dir: &Path,
    raw_title_or_path: &str,
    view_kind: CollectionViewKind,
    preset_id: &str,
    is_ru: bool,
    existing_paths: &HashSet<String>,
) -> Result<(String, VaultFileKind, CollectionDoc, i64, u64), String> {
    let (folder, title) = split_folder_and_title(raw_title_or_path);
    let kind = match view_kind {
        CollectionViewKind::Board => VaultFileKind::Board,
        CollectionViewKind::Table | CollectionViewKind::Gallery => VaultFileKind::Table,
    };
    let rel_path = unique_vault_path(folder.as_deref(), &title, kind.extension(), existing_paths);
    let doc = match view_kind {
        CollectionViewKind::Board => CollectionDoc::new_board_preset(&title, preset_id, is_ru),
        CollectionViewKind::Table | CollectionViewKind::Gallery => {
            CollectionDoc::new_table_preset(&title, preset_id, is_ru)
        }
    };
    let (modified, size) = save_collection(vault_dir, &rel_path, &doc)?;
    Ok((rel_path, kind, doc, modified, size))
}

pub fn rename_collection(
    vault_dir: &Path,
    old_rel_path: &str,
    new_title: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, CollectionDoc, i64, u64, VaultFileKind), String> {
    let (clean_old, kind) = validate_vault_path(old_rel_path)?;
    if !matches!(kind, VaultFileKind::Board | VaultFileKind::Table) {
        return Err(format!("not a collection file: {old_rel_path}"));
    }
    let (mut doc, _, _, _) = read_collection(vault_dir, &clean_old)?;
    let clean_title = new_title.trim();
    if clean_title.is_empty() {
        return Err("title cannot be empty".to_string());
    }
    let folder = clean_old.rsplit_once('/').map(|(d, _)| d.to_string());
    let mut filtered_existing = existing_paths.clone();
    filtered_existing.remove(&clean_old);

    let new_rel_path =
        unique_vault_path(folder.as_deref(), clean_title, kind.extension(), &filtered_existing);
    doc.title = clean_title.to_string();

    if new_rel_path == clean_old {
        let (modified, size) = save_collection(vault_dir, &clean_old, &doc)?;
        return Ok((clean_old, doc, modified, size, kind));
    }

    let (modified, size) = save_collection(vault_dir, &new_rel_path, &doc)?;
    let old_full = full_vault_path(vault_dir, &clean_old)?;
    if old_full.exists() {
        let _ = fs::remove_file(&old_full);
    }
    Ok((new_rel_path, doc, modified, size, kind))
}

pub fn duplicate_collection(
    vault_dir: &Path,
    rel_path: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, CollectionDoc, i64, u64, VaultFileKind), String> {
    let (clean, kind) = validate_vault_path(rel_path)?;
    let (mut doc, _, _, _) = read_collection(vault_dir, &clean)?;
    let folder = clean.rsplit_once('/').map(|(d, _)| d.to_string());
    let copy_title = format!("{} (Copy)", doc.title);
    let new_rel =
        unique_vault_path(folder.as_deref(), &copy_title, kind.extension(), existing_paths);
    doc.title = copy_title;
    let (modified, size) = save_collection(vault_dir, &new_rel, &doc)?;
    Ok((new_rel, doc, modified, size, kind))
}

pub fn rewrite_vault_collection_links(
    vault_dir: &Path,
    old_targets: &[&str],
    new_target: &str,
) -> Result<(Vec<(String, VaultFileKind, CollectionDoc, i64, u64)>, usize), String> {
    let mut updated = Vec::new();
    let mut total_rewrites = 0usize;

    for info in list_collection_files(vault_dir) {
        let Ok((mut doc, _, _, kind)) = read_collection(vault_dir, &info.rel_path) else {
            continue;
        };
        let count = doc.rewrite_note_links(old_targets, new_target);
        if count > 0 {
            let (modified, size) = save_collection(vault_dir, &info.rel_path, &doc)?;
            total_rewrites += count;
            updated.push((info.rel_path, kind, doc, modified, size));
        }
    }

    Ok((updated, total_rewrites))
}

pub fn read_canvas(vault_dir: &Path, rel_path: &str) -> Result<(CanvasDoc, i64, u64), String> {
    let (clean, kind) = validate_vault_path(rel_path)?;
    if kind != VaultFileKind::Canvas {
        return Err(format!("not a canvas file: {rel_path}"));
    }
    let full = full_vault_path(vault_dir, &clean)?;
    let raw = fs::read_to_string(&full).map_err(|e| format!("read '{clean}': {e}"))?;
    let stem = vault_file_stem(&clean);
    let doc = CanvasDoc::parse(&raw, stem)?;
    let (modified, size) = file_mtime_and_size(&full);
    Ok((doc, modified, size))
}

pub fn save_canvas(vault_dir: &Path, rel_path: &str, doc: &CanvasDoc) -> Result<(i64, u64), String> {
    let (_, kind) = validate_vault_path(rel_path)?;
    if kind != VaultFileKind::Canvas {
        return Err(format!("not a canvas file: {rel_path}"));
    }
    let serialized = doc.serialize_pretty()?;
    write_note_bytes_atomic(vault_dir, rel_path, serialized.as_bytes())
}

pub fn create_canvas_preset(
    vault_dir: &Path,
    raw_title_or_path: &str,
    preset_id: &str,
    is_ru: bool,
    existing_paths: &HashSet<String>,
) -> Result<(String, CanvasDoc, i64, u64), String> {
    let (folder, title) = split_folder_and_title(raw_title_or_path);
    let rel_path = unique_vault_path(
        folder.as_deref(),
        &title,
        VaultFileKind::Canvas.extension(),
        existing_paths,
    );
    let doc = CanvasDoc::new_preset(&title, preset_id, is_ru);
    let (modified, size) = save_canvas(vault_dir, &rel_path, &doc)?;
    Ok((rel_path, doc, modified, size))
}

pub fn rename_canvas(
    vault_dir: &Path,
    old_rel_path: &str,
    new_title: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, CanvasDoc, i64, u64), String> {
    let (clean_old, kind) = validate_vault_path(old_rel_path)?;
    if kind != VaultFileKind::Canvas {
        return Err(format!("not a canvas file: {old_rel_path}"));
    }
    let (mut doc, _, _) = read_canvas(vault_dir, &clean_old)?;
    let clean_title = new_title.trim();
    if clean_title.is_empty() {
        return Err("title cannot be empty".to_string());
    }
    let folder = clean_old.rsplit_once('/').map(|(d, _)| d.to_string());
    let mut filtered_existing = existing_paths.clone();
    filtered_existing.remove(&clean_old);

    let new_rel_path = unique_vault_path(
        folder.as_deref(),
        clean_title,
        VaultFileKind::Canvas.extension(),
        &filtered_existing,
    );
    doc.title = clean_title.to_string();

    if new_rel_path == clean_old {
        let (modified, size) = save_canvas(vault_dir, &clean_old, &doc)?;
        return Ok((clean_old, doc, modified, size));
    }

    let (modified, size) = save_canvas(vault_dir, &new_rel_path, &doc)?;
    let old_full = full_vault_path(vault_dir, &clean_old)?;
    if old_full.exists() {
        let _ = fs::remove_file(&old_full);
    }
    Ok((new_rel_path, doc, modified, size))
}

pub fn duplicate_canvas(
    vault_dir: &Path,
    rel_path: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, CanvasDoc, i64, u64), String> {
    let (clean, kind) = validate_vault_path(rel_path)?;
    if kind != VaultFileKind::Canvas {
        return Err(format!("not a canvas file: {rel_path}"));
    }
    let (mut doc, _, _) = read_canvas(vault_dir, &clean)?;
    let folder = clean.rsplit_once('/').map(|(d, _)| d.to_string());
    let copy_title = format!("{} (Copy)", doc.title);
    let new_rel = unique_vault_path(
        folder.as_deref(),
        &copy_title,
        VaultFileKind::Canvas.extension(),
        existing_paths,
    );
    doc.title = copy_title;
    let (modified, size) = save_canvas(vault_dir, &new_rel, &doc)?;
    Ok((new_rel, doc, modified, size))
}

pub fn rewrite_vault_canvas_links(
    vault_dir: &Path,
    old_path: &str,
    new_path: &str,
    old_targets: &[&str],
    new_target: &str,
) -> Result<(Vec<(String, CanvasDoc, i64, u64)>, usize), String> {
    let mut updated = Vec::new();
    let mut total_rewrites = 0usize;

    for info in list_canvas_files(vault_dir) {
        let Ok((mut doc, _, _)) = read_canvas(vault_dir, &info.rel_path) else {
            continue;
        };
        let count = doc.rewrite_note_links(old_path, new_path, old_targets, new_target);
        if count > 0 {
            let (modified, size) = save_canvas(vault_dir, &info.rel_path, &doc)?;
            total_rewrites += count;
            updated.push((info.rel_path, doc, modified, size));
        }
    }

    Ok((updated, total_rewrites))
}

fn normalize_image_ext(ext: &str) -> &'static str {
    match ext.trim().trim_start_matches('.').to_lowercase().as_str() {
        "jpg" => ".jpg",
        "jpeg" => ".jpeg",
        "webp" => ".webp",
        "gif" => ".gif",
        _ => ".png",
    }
}

pub fn save_image_bytes_to_assets(
    vault_dir: &Path,
    stem_hint: &str,
    ext: &str,
    bytes: &[u8],
    existing_paths: &HashSet<String>,
) -> Result<(String, i64, u64), String> {
    if bytes.is_empty() {
        return Err("image file is empty".to_string());
    }
    let clean_ext = normalize_image_ext(ext);
    let clean_stem = {
        let s = slugify_title(stem_hint);
        if s == "untitled" {
            "image".to_string()
        } else {
            s
        }
    };
    let rel_path = unique_vault_path(Some("assets"), &clean_stem, clean_ext, existing_paths);
    let (modified, size) = write_note_bytes_atomic(vault_dir, &rel_path, bytes)?;
    Ok((rel_path, modified, size))
}

pub fn import_image_file_to_assets(
    vault_dir: &Path,
    source_path: &Path,
    existing_paths: &HashSet<String>,
) -> Result<(String, Vec<u8>, i64, u64), String> {
    let ext = source_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err(format!("unsupported image format: {ext}"));
    }
    let stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let bytes = fs::read(source_path).map_err(|e| format!("read image: {e}"))?;
    let (rel_path, modified, size) =
        save_image_bytes_to_assets(vault_dir, stem, &ext, &bytes, existing_paths)?;
    Ok((rel_path, bytes, modified, size))
}

fn prune_empty_parent_dirs(vault_dir: &Path, full_path: &Path) {
    let mut cur = full_path.parent();
    while let Some(dir) = cur {
        if dir == vault_dir {
            break;
        }
        if fs::read_dir(dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
        {
            let _ = fs::remove_dir(dir);
            cur = dir.parent();
        } else {
            break;
        }
    }
}

/// Migrates flat legacy daily notes (`daily/YYYY-MM-DD.md` -> `daily/YYYY/MM/YYYY-MM-DD.md`
/// with Year/Month hub links) and separates diary boards (`daily/**/*.board.json` outside
/// `daily/boards/` -> `daily/boards/<file>`), rewriting internal vault links cleanly.
pub fn migrate_diary_hierarchy(vault_dir: &Path, lang: Lang) -> Result<usize, String> {
    let mut migrated = 0usize;

    // 1. Move flat daily/YYYY-MM-DD.md into daily/YYYY/MM/YYYY-MM-DD.md
    let md_files = list_markdown_files(vault_dir);
    for info in md_files {
        if !info.rel_path.starts_with("daily/") {
            continue;
        }
        let Some(date) = parse_daily_note_path(&info.rel_path) else {
            continue;
        };
        let target_path = daily_note_rel_path(date);
        if info.rel_path == target_path {
            continue;
        }
        let Ok(target_full) = full_vault_path(vault_dir, &target_path) else {
            continue;
        };
        if target_full.exists() {
            continue;
        }

        let Ok((mut doc, _, _)) = read_note(vault_dir, &info.rel_path) else {
            continue;
        };
        let month_title = format_daily_month_title(lang, date.year, date.month);
        let month_tag = format!("[[{month_title}]]");
        if !doc.body.contains(&month_tag) {
            let month_lbl = match lang {
                Lang::Ru => format!("Месяц: {month_tag}"),
                Lang::En => format!("Month: {month_tag}"),
            };
            if let Some(first_nl) = doc.body.find('\n') {
                let first_line = doc.body[..first_nl].trim_start();
                if first_line.starts_with('#') {
                    doc.body.insert_str(first_nl + 1, &format!("\n{month_lbl}\n"));
                } else {
                    doc.body = format!("{month_lbl}\n\n{}", doc.body);
                }
            } else if doc.body.is_empty() {
                doc.body = format!("{month_lbl}\n");
            } else {
                doc.body.push_str(&format!("\n\n{month_lbl}\n"));
            }
        }
        let day_title = if doc.frontmatter.title.trim().is_empty() {
            format_daily_title(lang, date)
        } else {
            doc.frontmatter.title.trim().to_string()
        };

        let _ = save_note(vault_dir, &target_path, &doc)?;
        if let Ok(old_full) = full_vault_path(vault_dir, &info.rel_path) {
            let _ = fs::remove_file(&old_full);
            prune_empty_parent_dirs(vault_dir, &old_full);
        }
        let _ = ensure_daily_hubs_for_date(vault_dir, date, lang, &day_title);

        let old_no_ext = info.rel_path.strip_suffix(".md").unwrap_or(&info.rel_path);
        let old_targets = [info.rel_path.as_str(), old_no_ext];
        let _ = rewrite_vault_wikilinks(vault_dir, &target_path, &old_targets, &target_path);
        let _ = rewrite_vault_collection_links(vault_dir, &old_targets, &target_path);
        let _ = rewrite_vault_canvas_links(
            vault_dir,
            &info.rel_path,
            &target_path,
            &old_targets,
            &target_path,
        );
        migrated += 1;
    }

    // 2. Organize diary Kanban boards under daily/boards/YYYY/MM/ (or daily/boards/ if undated)
    let board_candidates: Vec<(DiskFileInfo, CollectionDoc, String)> = list_collection_files(vault_dir)
        .into_iter()
        .filter(|c| c.kind == VaultFileKind::Board && c.rel_path.starts_with("daily/"))
        .filter_map(|c| {
            let Ok((doc, _, _, _)) = read_collection(vault_dir, &c.rel_path) else {
                return None;
            };
            let target_dir = match infer_diary_board_ym(&c.rel_path, &doc) {
                Some((y, m)) => format!("daily/boards/{y:04}/{m:02}"),
                None => "daily/boards".to_string(),
            };
            let cur_dir = c.rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            if cur_dir == target_dir {
                None
            } else {
                Some((c, doc, target_dir))
            }
        })
        .collect();

    if !board_candidates.is_empty() {
        let mut existing: HashSet<String> = list_vault_files(vault_dir)
            .into_iter()
            .map(|f| f.rel_path)
            .collect();

        for (info, doc, target_dir) in board_candidates {
            let file_name = info
                .rel_path
                .rsplit('/')
                .next()
                .unwrap_or(&info.rel_path);
            let direct_target = format!("{target_dir}/{file_name}");
            let new_path = if !existing.contains(&direct_target)
                && !full_vault_path(vault_dir, &direct_target)
                    .map(|p| p.exists())
                    .unwrap_or(false)
            {
                direct_target
            } else {
                let raw_stem = file_name
                    .strip_suffix(VaultFileKind::Board.extension())
                    .unwrap_or(&doc.title);
                unique_vault_path(
                    Some(&target_dir),
                    raw_stem,
                    VaultFileKind::Board.extension(),
                    &existing,
                )
            };

            let _ = save_collection(vault_dir, &new_path, &doc)?;
            if let Ok(old_full) = full_vault_path(vault_dir, &info.rel_path) {
                let _ = fs::remove_file(&old_full);
                prune_empty_parent_dirs(vault_dir, &old_full);
            }
            existing.remove(&info.rel_path);
            existing.insert(new_path.clone());

            let old_no_ext = info
                .rel_path
                .strip_suffix(VaultFileKind::Board.extension())
                .unwrap_or(&info.rel_path);
            let old_targets = [info.rel_path.as_str(), old_no_ext];
            let _ = rewrite_vault_wikilinks(vault_dir, "", &old_targets, &new_path);
            let _ = rewrite_vault_canvas_links(
                vault_dir,
                &info.rel_path,
                &new_path,
                &old_targets,
                &new_path,
            );
            migrated += 1;
        }
    }

    Ok(migrated)
}

pub fn infer_diary_board_ym(board_path: &str, doc: &CollectionDoc) -> Option<(i32, u32)> {
    // 1. Check any linked daily note paths on the board records / descriptions
    for (target, _) in doc.extract_note_links() {
        let clean = target.trim().replace('\\', "/");
        if let Some(d) = parse_daily_note_path(&clean) {
            return Some((d.year, d.month));
        }
        // Check Month hub `daily/YYYY/MM/YYYY-MM.md` or `daily/YYYY/MM`
        let segs: Vec<&str> = clean.split('/').collect();
        if segs.len() >= 3 && segs[0].eq_ignore_ascii_case("daily") {
            if let (Ok(y), Ok(m)) = (segs[1].parse::<i32>(), segs[2].parse::<u32>()) {
                if (1970..=2200).contains(&y) && (1..=12).contains(&m) {
                    return Some((y, m));
                }
            }
        }
    }

    // 2. Check existing `daily/boards/YYYY/MM/...` path prefix
    let norm_board = board_path.trim().replace('\\', "/");
    let p_segs: Vec<&str> = norm_board.split('/').collect();
    if p_segs.len() >= 5
        && p_segs[0].eq_ignore_ascii_case("daily")
        && p_segs[1].eq_ignore_ascii_case("boards")
    {
        if let (Ok(y), Ok(m)) = (p_segs[2].parse::<i32>(), p_segs[3].parse::<u32>()) {
            if (1970..=2200).contains(&y) && (1..=12).contains(&m) {
                return Some((y, m));
            }
        }
    }

    // 3. Parse month + year from title or filename (e.g. "20 ноября 2026 — Задачи" or "2026-11-20")
    let combined = format!("{} {}", doc.title, board_path).to_lowercase();
    for tok in combined.split(|c: char| !c.is_ascii_digit() && c != '-') {
        if let Some(d) = SimpleDate::parse_iso(tok) {
            return Some((d.year, d.month));
        }
    }
    let year = combined
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| s.len() == 4)
        .filter_map(|s| s.parse::<i32>().ok())
        .find(|&y| (2000..=2150).contains(&y))?;

    let months: &[(&[&str], u32)] = &[
        (&["январ", "january", "jan"], 1),
        (&["феврал", "february", "feb"], 2),
        (&["март", "march", "mar"], 3),
        (&["апрел", "april", "apr"], 4),
        (&["мая", "май", "may"], 5),
        (&["июн", "june", "jun"], 6),
        (&["июл", "july", "jul"], 7),
        (&["август", "august", "aug"], 8),
        (&["сентябр", "september", "sep"], 9),
        (&["октябр", "october", "oct"], 10),
        (&["ноябр", "november", "nov"], 11),
        (&["декабр", "december", "dec"], 12),
    ];
    for (stems, m) in months {
        if stems.iter().any(|st| combined.contains(st)) {
            return Some((year, *m));
        }
    }
    None
}

#[derive(Clone, Debug)]
pub struct AssetAuditEntry {
    pub rel_path: String,
    pub size: u64,
    pub ref_count: usize,
    pub referenced_by: Vec<String>,
}

/// Scans all image assets in the vault and counts how many Notes, Collections, or Canvases
/// reference each file, returning unused orphans first, then sorting by byte size descending.
pub fn audit_vault_assets(vault_dir: &Path) -> Vec<AssetAuditEntry> {
    let all_files = list_vault_files(vault_dir);
    let assets: Vec<DiskFileInfo> = all_files
        .iter()
        .filter(|f| f.kind == VaultFileKind::Asset)
        .cloned()
        .collect();
    if assets.is_empty() {
        return Vec::new();
    }

    let doc_contents: Vec<(String, String)> = all_files
        .iter()
        .filter(|f| !f.kind.is_binary())
        .filter_map(|f| {
            fs::read_to_string(&f.full_path)
                .ok()
                .map(|raw| (f.rel_path.clone(), raw.replace('\\', "/").to_lowercase()))
        })
        .collect();

    let mut entries = Vec::with_capacity(assets.len());
    for asset in assets {
        let rel_lower = asset.rel_path.replace('\\', "/").to_lowercase();
        let pct_lower = rel_lower.replace(' ', "%20");
        let file_name = rel_lower.rsplit('/').next().unwrap_or(&rel_lower);
        let wiki_exact = format!("[[{file_name}]]");
        let wiki_alias = format!("[[{file_name}|");
        let mut refs = Vec::new();

        for (doc_path, hay) in &doc_contents {
            if hay.contains(&rel_lower)
                || hay.contains(&pct_lower)
                || hay.contains(&wiki_exact)
                || hay.contains(&wiki_alias)
            {
                refs.push(doc_path.clone());
            }
        }

        entries.push(AssetAuditEntry {
            rel_path: asset.rel_path,
            size: asset.size,
            ref_count: refs.len(),
            referenced_by: refs,
        });
    }

    entries.sort_by(|a, b| {
        (a.ref_count > 0)
            .cmp(&(b.ref_count > 0))
            .then_with(|| b.size.cmp(&a.size))
            .then_with(|| a.rel_path.cmp(&b.rel_path))
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_vault_wikilinks_on_rename() {
        let dir = std::env::temp_dir().join(format!("krono_vault_rw_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let _ = write_note_bytes_atomic(
            &dir,
            "ref.md",
            b"---\ntitle: Ref\nstatus: Todo\ntags:\n---\n\nSee [[Old Name]] and [[Old Name|Alias]].\n```\n[[Old Name]]\n```\n",
        )
        .unwrap();

        let (updated, count) =
            rewrite_vault_wikilinks(&dir, "other.md", &["Old Name", "old_name"], "New Name")
                .unwrap();
        assert_eq!(count, 2);
        assert_eq!(updated.len(), 1);
        assert!(updated[0].1.body.contains("See [[New Name]] and [[New Name|Alias]]."));
        assert!(updated[0].1.body.contains("```\n[[Old Name]]\n```"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn soft_deletes_into_hidden_trash_and_prunes_empty_dirs() {
        let dir = std::env::temp_dir().join(format!("krono_vault_trash_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let _ = write_note_bytes_atomic(&dir, "sub/temp.md", b"hello").unwrap();
        assert!(dir.join("sub/temp.md").exists());

        delete_note(&dir, "sub/temp.md").unwrap();
        assert!(!dir.join("sub/temp.md").exists());
        assert!(dir.join("sub").exists());
        delete_vault_folder(&dir, "sub").unwrap();
        assert!(!dir.join("sub").exists());
        let trash_entries: Vec<_> = fs::read_dir(dir.join(".trash"))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(trash_entries.len(), 1);
        // Hidden .trash must never appear in active vault scans
        assert!(list_vault_files(&dir).is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_renames_duplicates_and_trashes_collections() {
        let dir =
            std::env::temp_dir().join(format!("krono_vault_col_crud_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let existing = HashSet::new();
        let (path, kind, mut doc, _, _) = create_collection(
            &dir,
            "youtube/Контент план",
            CollectionViewKind::Board,
            true,
            &existing,
        )
        .unwrap();
        assert_eq!(kind, VaultFileKind::Board);
        assert_eq!(path, "youtube/контент-план.board.json");

        let rec_id = doc.add_record("Видео 1", Some(("stage", "opt_idea")), 10);
        doc.records[0].description = "Детали в [[Old Note]].".into();
        let _ = save_collection(&dir, &path, &doc).unwrap();
        let _ = rec_id;

        let (updated, count) =
            rewrite_vault_collection_links(&dir, &["Old Note"], "New Note").unwrap();
        assert_eq!(count, 1);
        assert_eq!(updated.len(), 1);
        assert!(updated[0].2.records[0].description.contains("[[New Note]]"));

        let (new_path, _, _, _, rkind) =
            rename_collection(&dir, &path, "План 2026", &existing).unwrap();
        assert_eq!(rkind, VaultFileKind::Board);
        assert_eq!(new_path, "youtube/план-2026.board.json");

        let (dup_path, dup_doc, _, _, dkind) =
            duplicate_collection(&dir, &new_path, &existing).unwrap();
        assert_eq!(dkind, VaultFileKind::Board);
        assert!(dup_doc.title.contains("(Copy)"));
        assert!(dir.join(&dup_path).exists());

        delete_note(&dir, &new_path).unwrap();
        assert!(!dir.join(&new_path).exists());
        assert_eq!(list_collection_files(&dir).len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_renames_duplicates_and_trashes_canvases() {
        let dir = std::env::temp_dir().join(format!("krono_vault_canvas_crud_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let existing = HashSet::new();
        let (path, mut doc, _, _) =
            create_canvas_preset(&dir, "youtube/Сценарий ролика", "storyboard", true, &existing)
                .unwrap();
        assert_eq!(path, "youtube/сценарий-ролика.canvas");

        doc.add_file_node(120.0, 40.0, "old.md", None);
        let _ = save_canvas(&dir, &path, &doc).unwrap();

        let (updated, count) =
            rewrite_vault_canvas_links(&dir, "old.md", "new.md", &["Old"], "New").unwrap();
        assert_eq!(count, 1);
        assert_eq!(updated.len(), 1);

        let (new_path, _, _, _) = rename_canvas(&dir, &path, "Финальный холст", &existing).unwrap();
        assert_eq!(new_path, "youtube/финальный-холст.canvas");

        let (dup_path, dup_doc, _, _) = duplicate_canvas(&dir, &new_path, &existing).unwrap();
        assert!(dup_doc.title.contains("(Copy)"));
        assert!(dir.join(&dup_path).exists());

        delete_note(&dir, &new_path).unwrap();
        assert_eq!(list_canvas_files(&dir).len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_nested_daily_notes_with_year_and_month_hubs() {
        let dir = std::env::temp_dir().join(format!("krono_vault_daily_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let d1 = SimpleDate::from_ymd(2026, 11, 19).unwrap();
        let d2 = SimpleDate::from_ymd(2026, 11, 20).unwrap();
        let res1 = create_daily_note(&dir, d1, Lang::Ru).unwrap();
        assert_eq!(res1.day_path, "daily/2026/11/2026-11-19.md");
        assert!(res1.day_doc.body.contains("[[Ноябрь 2026]]"));
        assert_eq!(res1.hubs.len(), 2);

        let res2 = create_daily_note(&dir, d2, Lang::Ru).unwrap();
        assert_eq!(res2.day_path, "daily/2026/11/2026-11-20.md");
        let (month_doc, _, _) = read_note(&dir, "daily/2026/11/2026-11.md").unwrap();
        assert!(month_doc.body.contains("[[Дневник 2026]]"));
        assert!(month_doc.body.contains("[[19 ноября 2026]]"));
        assert!(month_doc.body.contains("[[20 ноября 2026]]"));
        let (year_doc, _, _) = read_note(&dir, "daily/2026/2026.md").unwrap();
        assert!(year_doc.body.contains("[[Ноябрь 2026]]"));
        assert_eq!(year_doc.body.matches("[[Ноябрь 2026]]").count(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn saves_and_deduplicates_image_assets() {
        let dir = std::env::temp_dir().join(format!("krono_vault_assets_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut existing = HashSet::new();
        let (p1, _, sz1) =
            save_image_bytes_to_assets(&dir, "Скрин экрана", "PNG", b"\x89PNG\r\n\x1a\n123", &existing)
                .unwrap();
        assert_eq!(p1, "assets/скрин-экрана.png");
        assert_eq!(sz1, 11);
        existing.insert(p1.clone());

        let (p2, _, _) =
            save_image_bytes_to_assets(&dir, "Скрин экрана", ".png", b"\x89PNG\r\n\x1a\n456", &existing)
                .unwrap();
        assert_eq!(p2, "assets/скрин-экрана-2.png");
        assert!(dir.join(&p1).exists());
        assert!(dir.join(&p2).exists());
        assert_eq!(list_vault_files(&dir).len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_daily_notes_and_diary_boards() {
        let dir = std::env::temp_dir().join(format!("krono_vault_mig_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let _ = write_note_bytes_atomic(
            &dir,
            "daily/2026-11-20.md",
            b"---\ntitle: 20 \xd0\xbd\xd0\xbe\xd1\x8f\xd0\xb1\xd1\x80\xd1\x8f 2026\nstatus: Todo\ntags:\n  - daily\n---\n\n# 20 \xd0\xbd\xd0\xbe\xd1\x8f\xd0\xb1\xd1\x80\xd1\x8f 2026\n\n## \xd0\x97\xd0\xb0\xd0\xb4\xd0\xb0\xd1\x87\xd0\xb8\n![[daily/tasks.board.json]]\n",
        )
        .unwrap();

        let mut board = CollectionDoc::new_board_preset("Задачи", "kanban", true);
        let nl_id = board
            .columns
            .iter()
            .find(|c| c.col_type == krono_core::collection::ColumnType::NoteLink)
            .unwrap()
            .id
            .clone();
        let rec_id = board.add_record("Первая задача", None, 100);
        board
            .records
            .iter_mut()
            .find(|r| r.id == rec_id)
            .unwrap()
            .values
            .insert(
                nl_id.clone(),
                krono_core::collection::FieldValue::Text("daily/2026-11-20.md".into()),
            );
        save_collection(&dir, "daily/tasks.board.json", &board).unwrap();

        let count = migrate_diary_hierarchy(&dir, Lang::Ru).unwrap();
        assert_eq!(count, 2);

        assert!(!dir.join("daily/2026-11-20.md").exists());
        assert!(dir.join("daily/2026/11/2026-11-20.md").exists());
        assert!(dir.join("daily/2026/2026.md").exists());
        assert!(dir.join("daily/2026/11/2026-11.md").exists());
        assert!(!dir.join("daily/tasks.board.json").exists());
        assert!(dir.join("daily/boards/2026/11/tasks.board.json").exists());

        let (day_doc, _, _) = read_note(&dir, "daily/2026/11/2026-11-20.md").unwrap();
        assert!(day_doc.body.contains("[[Ноябрь 2026]]"));
        assert!(day_doc
            .body
            .contains("![[daily/boards/2026/11/tasks.board.json]]"));

        let (migrated_board, _, _, _) =
            read_collection(&dir, "daily/boards/2026/11/tasks.board.json").unwrap();
        assert_eq!(
            migrated_board.records[0].values.get(&nl_id).unwrap().as_text(),
            "daily/2026/11/2026-11-20.md"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn audits_used_and_unused_vault_assets() {
        let dir = std::env::temp_dir().join(format!("krono_vault_audit_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let existing = HashSet::new();
        let (used_p, _, _) =
            save_image_bytes_to_assets(&dir, "used", "png", b"\x89PNG\r\n\x1a\n111", &existing)
                .unwrap();
        let (unused_p, _, _) = save_image_bytes_to_assets(
            &dir,
            "orphan",
            "png",
            b"\x89PNG\r\n\x1a\n222333",
            &existing,
        )
        .unwrap();
        write_note_bytes_atomic(
            &dir,
            "note.md",
            format!("---\ntitle: Note\n---\n\n![Demo]({used_p})\n").as_bytes(),
        )
        .unwrap();

        let audit = audit_vault_assets(&dir);
        assert_eq!(audit.len(), 2);
        // Unused orphans come first
        assert_eq!(audit[0].rel_path, unused_p);
        assert_eq!(audit[0].ref_count, 0);
        assert_eq!(audit[1].rel_path, used_p);
        assert_eq!(audit[1].ref_count, 1);
        assert_eq!(audit[1].referenced_by, vec!["note.md".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn creates_and_lists_vault_folders() {
        let dir = std::env::temp_dir().join(format!("krono_vault_folders_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let f1 = create_vault_folder(&dir, "", "Проекты").unwrap();
        assert_eq!(f1, "проекты");
        let f2 = create_vault_folder(&dir, &f1, "Идеи 2026").unwrap();
        assert_eq!(f2, "проекты/идеи-2026");

        let listed = list_vault_folders(&dir);
        assert!(listed.contains(&"проекты".to_string()));
        assert!(listed.contains(&"проекты/идеи-2026".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }
}

pub fn list_vault_folders(vault_dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in WalkDir::new(vault_dir)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
            if e.depth() == 1 && (name.eq_ignore_ascii_case("assets") || name.eq_ignore_ascii_case("daily")) {
                return false;
            }
            true
        })
        .flatten()
    {
        if !entry.file_type().is_dir() {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(vault_dir) {
            let norm = rel.to_string_lossy().replace('\\', "/");
            if !norm.is_empty() && !norm.split('/').any(|s| s.is_empty() || s.starts_with('.')) {
                out.push(norm);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

pub fn create_vault_folder(
    vault_dir: &Path,
    parent: &str,
    raw_name: &str,
) -> Result<String, String> {
    let seg = slugify_title(raw_name);
    if seg == "untitled" && raw_name.trim().is_empty() {
        return Err("folder name cannot be empty".to_string());
    }
    let parent_clean = parent.trim().trim_matches('/').replace('\\', "/");
    if parent_clean.split('/').any(|s| s == ".." || s.starts_with('.')) {
        return Err("invalid parent folder".to_string());
    }
    let rel = if parent_clean.is_empty() {
        seg
    } else {
        format!("{parent_clean}/{seg}")
    };
    let mut full = vault_dir.to_path_buf();
    for part in rel.split('/') {
        full.push(part);
    }
    fs::create_dir_all(&full).map_err(|e| format!("create folder '{rel}': {e}"))?;
    Ok(rel)
}

/// Moves a vault file (`.md`, `.md.enc`, `.board.json`, `.table.json`, `.canvas`)
/// into `target_folder` (empty string = vault root) with atomic write and collision deduplication.
pub fn move_vault_file_to_folder(
    vault_dir: &Path,
    old_rel_path: &str,
    target_folder: &str,
    existing_paths: &HashSet<String>,
) -> Result<(String, VaultFileKind, i64, u64), String> {
    let (clean_old, kind) = validate_vault_path(old_rel_path)?;
    if matches!(kind, VaultFileKind::Asset) {
        return Err("moving binary assets is not supported".to_string());
    }
    let clean_target = target_folder.trim().replace('\\', "/");
    let clean_target = clean_target.trim_matches('/').to_string();
    if !clean_target.is_empty() {
        if clean_target.contains('\0') || clean_target.as_bytes().get(1) == Some(&b':') {
            return Err(format!("invalid target folder: {target_folder}"));
        }
        for seg in clean_target.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') {
                return Err(format!("disallowed target folder segment '{seg}'"));
            }
        }
    }

    let old_parent = clean_old.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let old_full = full_vault_path(vault_dir, &clean_old)?;
    if !old_full.is_file() {
        return Err(format!("source file does not exist: {clean_old}"));
    }
    if old_parent == clean_target {
        let meta = fs::metadata(&old_full).map_err(|e| format!("stat '{clean_old}': {e}"))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        return Ok((clean_old, kind, modified, meta.len()));
    }

    let file_name = clean_old.rsplit('/').next().unwrap_or(&clean_old);
    let candidate = if clean_target.is_empty() {
        file_name.to_string()
    } else {
        format!("{clean_target}/{file_name}")
    };

    let mut filtered_existing = existing_paths.clone();
    filtered_existing.remove(&clean_old);
    let has_collision = filtered_existing
        .iter()
        .any(|p| p.eq_ignore_ascii_case(&candidate));
    let new_rel_path = if has_collision {
        unique_vault_path(
            (!clean_target.is_empty()).then_some(clean_target.as_str()),
            vault_file_stem(&clean_old),
            kind.extension(),
            &filtered_existing,
        )
    } else {
        candidate
    };

    let bytes = fs::read(&old_full).map_err(|e| format!("read '{clean_old}': {e}"))?;
    let new_full = full_vault_path(vault_dir, &new_rel_path)?;
    write_file_atomic(&new_full, &bytes)?;
    let meta = fs::metadata(&new_full).map_err(|e| format!("stat '{new_rel_path}': {e}"))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let size = meta.len();
    if new_rel_path != clean_old {
        let _ = fs::remove_file(&old_full);
    }
    Ok((new_rel_path, kind, modified, size))
}

#[cfg(test)]
mod move_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static MOVE_TEST_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn moves_notes_boards_and_encrypted_files_between_folders() {
        let id = MOVE_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "krono_move_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("work")).unwrap();
        fs::write(dir.join("note.md"), "# Note\n").unwrap();
        fs::write(dir.join("board.board.json"), "{}").unwrap();
        fs::write(dir.join("secret.md.enc"), "ENC").unwrap();
        fs::write(dir.join("work/note.md"), "# Existing\n").unwrap();

        let existing: HashSet<String> = [
            "note.md".to_string(),
            "board.board.json".to_string(),
            "secret.md.enc".to_string(),
            "work/note.md".to_string(),
        ]
        .into_iter()
        .collect();

        let (moved_note, kind_n, _, _) =
            move_vault_file_to_folder(&dir, "note.md", "work", &existing).unwrap();
        assert_eq!(kind_n, VaultFileKind::Markdown);
        assert_eq!(moved_note, "work/note-2.md");
        assert!(!dir.join("note.md").exists());
        assert!(dir.join("work/note-2.md").exists());

        let (moved_board, kind_b, _, _) =
            move_vault_file_to_folder(&dir, "board.board.json", "work", &existing).unwrap();
        assert_eq!(kind_b, VaultFileKind::Board);
        assert_eq!(moved_board, "work/board.board.json");

        let (moved_enc, kind_e, _, _) =
            move_vault_file_to_folder(&dir, "secret.md.enc", "work", &existing).unwrap();
        assert_eq!(kind_e, VaultFileKind::EncryptedMarkdown);
        assert_eq!(moved_enc, "work/secret.md.enc");

        let mut existing_after = existing.clone();
        existing_after.remove("board.board.json");
        existing_after.insert("work/board.board.json".to_string());
        let (back_root, _, _, _) =
            move_vault_file_to_folder(&dir, "work/board.board.json", "", &existing_after).unwrap();
        assert_eq!(back_root, "board.board.json");
        assert!(dir.join("board.board.json").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
