use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use krono_core::mcp_policy::{
    append_to_markdown, extract_search_snippet, slice_markdown_for_read, McpPolicy,
};
use krono_core::note::NoteDocument;
use krono_core::path::vault_file_stem;
use krono_core::search::{parse_search_query, ParsedSearchQuery};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Value};
use walkdir::WalkDir;

const MAX_READ_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_READ_RETURN_BYTES: usize = 64 * 1024;
const DEFAULT_SEARCH_LIMIT: usize = 15;
const HARD_SEARCH_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResultItem {
    pub path: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub snippet: String,
}

pub struct VaultSandbox {
    vault_root: PathBuf,
    canonical_vault: PathBuf,
    policy: McpPolicy,
}

impl VaultSandbox {
    pub fn new(vault_root: PathBuf, policy: McpPolicy) -> Result<Self, String> {
        let canonical_vault = fs::canonicalize(&vault_root)
            .map_err(|e| format!("failed to resolve vault '{}': {e}", vault_root.display()))?;
        if !canonical_vault.is_dir() {
            return Err(format!(
                "vault path is not a directory: {}",
                canonical_vault.display()
            ));
        }
        Ok(Self {
            vault_root,
            canonical_vault,
            policy,
        })
    }

    pub fn policy(&self) -> &McpPolicy {
        &self.policy
    }

    fn verify_canonical_inside_allowed(&self, canonical_target: &Path) -> Result<(), String> {
        if !canonical_target.starts_with(&self.canonical_vault) {
            return Err("access denied: path escapes vault root via symlink".to_string());
        }
        if self.policy.allow_all() {
            return Ok(());
        }
        for dir in self.policy.allowed_dirs() {
            if let Ok(canon_dir) = fs::canonicalize(self.canonical_vault.join(dir)) {
                if canonical_target.starts_with(&canon_dir) {
                    return Ok(());
                }
            }
        }
        Err("access denied: resolved path is outside --allow-dir".to_string())
    }

    fn resolve_existing_read_path(&self, raw_rel: &str) -> Result<(String, PathBuf), String> {
        let rel = self.policy.check_read_path(raw_rel)?;
        let candidate = self.canonical_vault.join(&rel);
        let canonical = fs::canonicalize(&candidate)
            .map_err(|e| format!("note not found or unreadable '{rel}': {e}"))?;
        self.verify_canonical_inside_allowed(&canonical)?;
        if !canonical.is_file() {
            return Err(format!("not a regular file: {rel}"));
        }
        Ok((rel, canonical))
    }

    fn reject_symlinks_along_rel(&self, rel: &str) -> Result<(), String> {
        let mut cur = self.canonical_vault.clone();
        for seg in rel.split('/') {
            cur.push(seg);
            if let Ok(meta) = fs::symlink_metadata(&cur) {
                if meta.file_type().is_symlink() {
                    return Err(format!("symlinks are not allowed in write path: {rel}"));
                }
            }
        }
        Ok(())
    }

    fn resolve_write_path(&self, raw_rel: &str) -> Result<(String, PathBuf), String> {
        let rel = self.policy.check_write_path(raw_rel)?;
        self.reject_symlinks_along_rel(&rel)?;
        let target = self.canonical_vault.join(&rel);
        if target.exists() {
            let canonical = fs::canonicalize(&target)
                .map_err(|e| format!("cannot resolve target '{rel}': {e}"))?;
            self.verify_canonical_inside_allowed(&canonical)?;
            if !canonical.is_file() {
                return Err(format!("target is not a regular file: {rel}"));
            }
            return Ok((rel, canonical));
        }

        // For new files, verify the nearest existing ancestor stays inside canonical_vault.
        let mut ancestor = target.parent().unwrap_or(&self.canonical_vault).to_path_buf();
        while !ancestor.exists() {
            let Some(parent) = ancestor.parent() else {
                break;
            };
            ancestor = parent.to_path_buf();
        }
        let canon_ancestor = fs::canonicalize(&ancestor)
            .map_err(|e| format!("cannot resolve parent directory for '{rel}': {e}"))?;
        if !canon_ancestor.starts_with(&self.canonical_vault) {
            return Err("access denied: target parent escapes vault root".to_string());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create parent dir: {e}"))?;
            let canon_parent = fs::canonicalize(parent)
                .map_err(|e| format!("cannot resolve created parent '{rel}': {e}"))?;
            self.verify_canonical_inside_allowed(&canon_parent)?;
        }
        Ok((rel, target))
    }

    fn collect_allowed_files(&self, folder: Option<&str>) -> Result<Vec<FileEntry>, String> {
        if self.policy.is_fail_closed() {
            return Ok(Vec::new());
        }

        let roots: Vec<PathBuf> = if let Some(f) = folder {
            let clean = self.policy.is_rel_dir_allowed(f)?;
            vec![self.canonical_vault.join(clean)]
        } else if self.policy.allow_all() {
            vec![self.canonical_vault.clone()]
        } else {
            self.policy
                .allowed_dirs()
                .iter()
                .map(|d| self.canonical_vault.join(d))
                .collect()
        };

        let mut entries: BTreeMap<String, u64> = BTreeMap::new();
        for root in roots {
            if !root.is_dir() {
                continue;
            }
            let walker = WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| {
                    if e.depth() == 0 {
                        return true;
                    }
                    let name = e.file_name().to_string_lossy();
                    !name.starts_with('.')
                });

            for entry in walker.flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Ok(rel_os) = entry.path().strip_prefix(&self.canonical_vault) else {
                    continue;
                };
                let rel = rel_os.to_string_lossy().replace('\\', "/");
                if !self.policy.is_rel_path_allowed(&rel) {
                    continue;
                }
                let Ok(canonical) = fs::canonicalize(entry.path()) else {
                    continue;
                };
                if self.verify_canonical_inside_allowed(&canonical).is_err() {
                    continue;
                }
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                entries.insert(rel, size);
            }
        }

        Ok(entries
            .into_iter()
            .map(|(path, size)| FileEntry { path, size })
            .collect())
    }

    pub fn list_files(&self, folder: Option<&str>) -> Result<Value, String> {
        if self.policy.is_fail_closed() {
            return Ok(json!({
                "files": [],
                "fail_closed": true,
                "note": "No directories allowed (pass --allow-dir or --allow-all)"
            }));
        }
        let files = self.collect_allowed_files(folder)?;
        Ok(json!({
            "count": files.len(),
            "files": files
        }))
    }

    pub fn read(&self, path: &str, max_lines: Option<usize>) -> Result<Value, String> {
        let (rel, canonical) = self.resolve_existing_read_path(path)?;
        let meta = fs::metadata(&canonical).map_err(|e| format!("stat '{rel}': {e}"))?;
        if meta.len() > MAX_READ_FILE_BYTES {
            return Err(format!(
                "file '{rel}' exceeds 2 MiB safety limit ({} bytes)",
                meta.len()
            ));
        }
        let raw = fs::read_to_string(&canonical)
            .map_err(|e| format!("failed to read '{rel}' as UTF-8: {e}"))?;
        let slice = slice_markdown_for_read(&raw, max_lines, MAX_READ_RETURN_BYTES);
        Ok(json!({
            "path": rel,
            "content": slice.content,
            "total_lines": slice.total_lines,
            "returned_lines": slice.returned_lines,
            "truncated": slice.truncated
        }))
    }

    pub fn append(
        &self,
        path: &str,
        content: &str,
        heading: Option<&str>,
    ) -> Result<Value, String> {
        let (rel, target) = self.resolve_write_path(path)?;
        let existing = if target.exists() {
            fs::read_to_string(&target)
                .map_err(|e| format!("failed to read '{rel}' before append: {e}"))?
        } else {
            String::new()
        };
        let updated = append_to_markdown(&existing, content, heading);
        write_file_atomic(&target, updated.as_bytes())?;
        Ok(json!({
            "path": rel,
            "bytes_written": updated.len(),
            "heading": heading
        }))
    }

    pub fn search(
        &self,
        query: &str,
        limit: Option<usize>,
        folder: Option<&str>,
    ) -> Result<Value, String> {
        let limit = limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, HARD_SEARCH_LIMIT);
        let folder_filter = match folder {
            Some(f) if !f.trim().is_empty() => Some(self.policy.is_rel_dir_allowed(f)?),
            _ => None,
        };
        if self.policy.is_fail_closed() {
            return Ok(json!({ "hits": [], "source": "fail_closed" }));
        }

        let parsed = parse_search_query(query);
        if let Some(hits) = self.search_via_index(&parsed, limit, folder_filter.as_deref()) {
            return Ok(json!({
                "count": hits.len(),
                "source": "index.db",
                "hits": hits
            }));
        }

        let hits = self.search_via_walk(&parsed, limit, folder_filter.as_deref())?;
        Ok(json!({
            "count": hits.len(),
            "source": "vault_walk",
            "hits": hits
        }))
    }

    fn search_via_index(
        &self,
        parsed: &ParsedSearchQuery,
        limit: usize,
        folder_filter: Option<&str>,
    ) -> Option<Vec<SearchResultItem>> {
        let db_path = self.vault_root.join(".krono").join("index.db");
        if !db_path.is_file() {
            return None;
        }
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .ok()?;
        let _ = conn.busy_timeout(Duration::from_millis(500));

        let fts_expr = build_fts5_query(&parsed.terms);
        let fetch_cap = (limit * 8).clamp(50, 400) as i64;

        let rows: Vec<(String, String, String, String, String)> = if let Some(expr) = fts_expr {
            let mut stmt = conn
                .prepare(
                    "SELECT n.path, n.title, n.status, n.tags, n.excerpt
                     FROM notes_fts
                     JOIN notes n ON n.path = notes_fts.path
                     WHERE n.encrypted = 0 AND notes_fts MATCH ?1
                     ORDER BY bm25(notes_fts) ASC, n.modified DESC
                     LIMIT ?2",
                )
                .ok()?;
            let iter = stmt
                .query_map(rusqlite::params![expr, fetch_cap], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .ok()?;
            iter.flatten().collect()
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT path, title, status, tags, excerpt
                     FROM notes
                     WHERE encrypted = 0
                     ORDER BY modified DESC
                     LIMIT ?1",
                )
                .ok()?;
            let iter = stmt
                .query_map(rusqlite::params![fetch_cap], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .ok()?;
            iter.flatten().collect()
        };

        let mut hits = Vec::new();
        for (path, title, status, tags_json, excerpt) in rows {
            if !self.policy.is_rel_path_allowed(&path) {
                continue;
            }
            if let Some(dir) = folder_filter {
                if !path_starts_with_dir(&path, dir) {
                    continue;
                }
            }
            if !matches_path_filters(&path, &parsed.paths) {
                continue;
            }
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            if !matches_tag_filters(&tags, &parsed.tags) {
                continue;
            }
            let status_opt = (!status.trim().is_empty()).then_some(status);
            if !matches_status_filters(status_opt.as_deref(), &parsed.statuses) {
                continue;
            }
            // Defense in depth: confirm file exists and does not escape via symlink.
            let Ok((_, canonical)) = self.resolve_existing_read_path(&path) else {
                continue;
            };
            let snippet = if parsed.terms.is_empty() {
                extract_search_snippet(&excerpt, &[], 180)
            } else {
                match fs::read_to_string(&canonical) {
                    Ok(raw) => {
                        let doc = NoteDocument::parse(&raw, vault_file_stem(&path));
                        extract_search_snippet(&doc.body, &parsed.terms, 180)
                    }
                    Err(_) => extract_search_snippet(&excerpt, &parsed.terms, 180),
                }
            };
            hits.push(SearchResultItem {
                path,
                title,
                status: status_opt,
                tags,
                snippet,
            });
            if hits.len() >= limit {
                break;
            }
        }
        Some(hits)
    }

    fn search_via_walk(
        &self,
        parsed: &ParsedSearchQuery,
        limit: usize,
        folder_filter: Option<&str>,
    ) -> Result<Vec<SearchResultItem>, String> {
        let files = self.collect_allowed_files(folder_filter)?;
        let mut hits = Vec::new();

        for file in files {
            let Ok((rel, canonical)) = self.resolve_existing_read_path(&file.path) else {
                continue;
            };
            if !matches_path_filters(&rel, &parsed.paths) {
                continue;
            }
            let Ok(meta) = fs::metadata(&canonical) else {
                continue;
            };
            if meta.len() > MAX_READ_FILE_BYTES {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&canonical) else {
                continue;
            };
            let doc = NoteDocument::parse(&raw, vault_file_stem(&rel));
            let tags = &doc.frontmatter.tags;
            if !matches_tag_filters(tags, &parsed.tags) {
                continue;
            }
            let status_str = doc
                .frontmatter
                .status
                .map(|s| s.as_yaml_str().to_string());
            if !matches_status_filters(status_str.as_deref(), &parsed.statuses) {
                continue;
            }
            if !parsed.terms.is_empty() {
                let haystack = format!(
                    "{}\n{}\n{}",
                    doc.frontmatter.title,
                    tags.join(" "),
                    doc.body
                )
                .to_lowercase();
                if !parsed
                    .terms
                    .iter()
                    .all(|t| haystack.contains(&t.to_lowercase()))
                {
                    continue;
                }
            }
            let snippet = extract_search_snippet(&doc.body, &parsed.terms, 180);
            hits.push(SearchResultItem {
                path: rel,
                title: doc.frontmatter.title,
                status: status_str,
                tags: tags.clone(),
                snippet,
            });
            if hits.len() >= limit {
                break;
            }
        }

        Ok(hits)
    }
}

fn path_starts_with_dir(path: &str, dir: &str) -> bool {
    path.starts_with(dir) && path.as_bytes().get(dir.len()) == Some(&b'/')
}

fn matches_path_filters(rel_path: &str, path_filters: &[String]) -> bool {
    if path_filters.is_empty() {
        return true;
    }
    let lower = rel_path.to_lowercase();
    path_filters
        .iter()
        .all(|p| lower.contains(&p.to_lowercase()))
}

fn matches_tag_filters(note_tags: &[String], tag_filters: &[String]) -> bool {
    if tag_filters.is_empty() {
        return true;
    }
    let lower_tags: Vec<String> = note_tags.iter().map(|t| t.to_lowercase()).collect();
    tag_filters
        .iter()
        .all(|req| lower_tags.iter().any(|t| t == req))
}

fn matches_status_filters(status: Option<&str>, status_filters: &[String]) -> bool {
    if status_filters.is_empty() {
        return true;
    }
    let Some(s) = status else {
        return false;
    };
    let lower = s.trim().to_lowercase();
    status_filters.iter().any(|req| req == &lower)
}

fn build_fts5_query(terms: &[String]) -> Option<String> {
    let parts: Vec<String> = terms
        .iter()
        .filter_map(|t| {
            let clean = t.trim().replace('"', "");
            if clean.is_empty() {
                None
            } else {
                Some(format!("\"{clean}\"*"))
            }
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

fn write_file_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create parent dir: {e}"))?;
    }
    let tmp = target.with_extension("krono.mcp.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write temp file: {e}"))?;
    if fs::rename(&tmp, target).is_err() {
        if target.exists() {
            let _ = fs::remove_file(target);
        }
        fs::rename(&tmp, target).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("rename temp file: {e}")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_vault() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "krono_mcp_test_{}_{}",
            std::process::id(),
            id
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("notes")).unwrap();
        fs::create_dir_all(dir.join("notes-secret")).unwrap();
        fs::create_dir_all(dir.join(".krono")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();

        fs::write(
            dir.join("notes/plan.md"),
            "---\ntitle: План\nstatus: todo\ntags: [work, mcp]\n---\nЛокальный MCP сервер на stdio.\n",
        )
        .unwrap();
        fs::write(dir.join("notes/secret.md.enc"), "ENCRYPTED_BLOB").unwrap();
        fs::write(dir.join("notes-secret/private.md"), "Чужая папка").unwrap();
        fs::write(dir.join(".krono/internal.md"), "Internal").unwrap();
        fs::write(dir.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        dir
    }

    #[test]
    fn sandbox_enforces_fail_closed_allowlist_and_hides_secrets() {
        let vault = temp_vault();

        let closed_policy = McpPolicy::new(&[], false, false).unwrap();
        let closed = VaultSandbox::new(vault.clone(), closed_policy).unwrap();
        let list = closed.list_files(None).unwrap();
        assert_eq!(list["files"].as_array().unwrap().len(), 0);
        assert!(closed.read("notes/plan.md", None).is_err());

        let policy = McpPolicy::new(&["notes".to_string()], false, false).unwrap();
        let sb = VaultSandbox::new(vault.clone(), policy).unwrap();
        let list = sb.list_files(None).unwrap();
        let files = list["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["path"], "notes/plan.md");

        assert!(sb.read("notes/secret.md.enc", None).is_err());
        assert!(sb.read("notes-secret/private.md", None).is_err());
        assert!(sb.read("../outside.md", None).is_err());
        assert!(sb.read(".krono/internal.md", None).is_err());

        let read_res = sb.read("notes/plan.md", Some(2)).unwrap();
        assert_eq!(read_res["returned_lines"], 2);
        assert_eq!(read_res["truncated"], true);

        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn search_uses_readonly_index_and_filters_encrypted_and_disallowed() {
        let vault = temp_vault();
        let db_path = vault.join(".krono").join("index.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                path TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                tags TEXT NOT NULL,
                excerpt TEXT NOT NULL,
                modified INTEGER NOT NULL,
                encrypted INTEGER NOT NULL DEFAULT 0
             );
             CREATE VIRTUAL TABLE notes_fts USING fts5(path UNINDEXED, title, tags, body, tokenize='unicode61');",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, ?5, 100, 0)",
            rusqlite::params!["notes/plan.md", "План", "todo", "[\"work\",\"mcp\"]", "Локальный MCP"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes_fts VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["notes/plan.md", "План", "work mcp", "Локальный MCP сервер на stdio"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, ?5, 100, 1)",
            rusqlite::params!["notes/secret.md.enc", "Secret", "todo", "[\"mcp\"]", "hidden"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes VALUES (?1, ?2, ?3, ?4, ?5, 100, 0)",
            rusqlite::params!["notes-secret/private.md", "Private", "todo", "[\"mcp\"]", "outside"],
        )
        .unwrap();
        drop(conn);

        let policy = McpPolicy::new(&["notes".to_string()], false, false).unwrap();
        let sb = VaultSandbox::new(vault.clone(), policy).unwrap();
        let res = sb.search("tag:mcp протокол?", None, None).unwrap();
        assert_eq!(res["source"], "index.db");
        // "протокол?" is not in notes/plan.md, check matching query:
        let res2 = sb.search("tag:mcp stdio", None, None).unwrap();
        let hits = res2["hits"].as_array().unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0]["path"], "notes/plan.md");
        assert!(res["hits"].as_array().unwrap().is_empty());

        let _ = fs::remove_dir_all(&vault);
    }

    #[test]
    fn append_requires_allow_write_and_writes_atomically() {
        let vault = temp_vault();
        let ro_policy = McpPolicy::new(&["notes".to_string()], false, false).unwrap();
        let ro = VaultSandbox::new(vault.clone(), ro_policy).unwrap();
        assert!(ro.append("notes/plan.md", "test", None).is_err());

        let rw_policy = McpPolicy::new(&["notes".to_string()], false, true).unwrap();
        let rw = VaultSandbox::new(vault.clone(), rw_policy).unwrap();
        rw.append("notes/plan.md", "- [ ] Проверить", Some("## Итоги"))
            .unwrap();
        let content = fs::read_to_string(vault.join("notes/plan.md")).unwrap();
        assert!(content.contains("## Итоги\n- [ ] Проверить\n"));

        let _ = fs::remove_dir_all(&vault);
    }
}
