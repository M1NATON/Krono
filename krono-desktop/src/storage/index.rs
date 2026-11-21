use crate::storage::vault::list_vault_files;
use chrono::Datelike;
use krono_core::{
    canvas::CanvasDoc,
    collection::{CollectionDoc, CollectionViewKind},
    daily::{parse_daily_note_path, SimpleDate},
    hash::{git_blob_sha1, normalize_lf_bytes},
    markdown::{card_preview_excerpt, count_words, extract_wikilinks, TaskPriority},
    note::{parse_tags_value, NoteDocument, NoteMeta, NoteStatus},
    path::{vault_file_stem, VaultFileKind},
    sync_plan::count_local_changes,
    tasks::{extract_tasks, TaskRecurrence},
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::Path,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiskSyncReport {
    pub changed_paths: Vec<String>,
    pub deleted_paths: Vec<String>,
}

impl DiskSyncReport {
    pub fn is_empty(&self) -> bool {
        self.changed_paths.is_empty() && self.deleted_paths.is_empty()
    }

    pub fn touches(&self, rel_path: &str) -> bool {
        self.changed_paths.iter().any(|p| p == rel_path)
            || self.deleted_paths.iter().any(|p| p == rel_path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionMeta {
    pub path: String,
    pub kind: VaultFileKind,
    pub title: String,
    pub item_count: usize,
    pub column_count: usize,
    pub excerpt: String,
    pub modified: i64,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub path: String,
    pub title: String,
    pub kind: VaultFileKind,
    pub snippet: String,
}

#[derive(Clone, Debug)]
pub struct BacklinkHit {
    pub source_path: String,
    pub source_title: String,
    pub context: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawLinkEdge {
    pub source_path: String,
    pub target_raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VaultTaskRow {
    pub path: String,
    pub note_title: String,
    pub line_idx: usize,
    pub task_idx: usize,
    pub raw_text: String,
    pub clean_text: String,
    pub done: bool,
    pub due_date: Option<SimpleDate>,
    pub priority: Option<TaskPriority>,
    pub recurrence: Option<TaskRecurrence>,
    pub tags: Vec<String>,
}

fn local_today() -> SimpleDate {
    let now = chrono::Local::now().date_naive();
    SimpleDate::from_ymd(now.year(), now.month(), now.day())
        .unwrap_or(SimpleDate { year: 2026, month: 1, day: 1 })
}

pub struct VaultIndex {
    conn: Connection,
}

impl VaultIndex {
    /// Opens or creates `.krono/index.db`; if corrupted, deletes & rebuilds cleanly without panicking.
    pub fn open_or_recover(vault_dir: &Path) -> (Self, bool) {
        if let Ok(idx) = Self::open_for_vault(vault_dir) {
            return (idx, false);
        }
        let krono_dir = vault_dir.join(".krono");
        for name in ["index.db", "index.db-wal", "index.db-shm"] {
            let _ = fs::remove_file(krono_dir.join(name));
        }
        if let Ok(idx) = Self::open_for_vault(vault_dir) {
            return (idx, true);
        }
        let conn = Connection::open_in_memory().expect("in-memory sqlite");
        let idx = Self { conn };
        let _ = idx.init_schema();
        (idx, true)
    }

    pub fn open_for_vault(vault_dir: &Path) -> Result<Self, String> {
        let krono_dir = vault_dir.join(".krono");
        let conn = if fs::create_dir_all(&krono_dir).is_ok() {
            let db_path = krono_dir.join("index.db");
            Connection::open(&db_path)
                .or_else(|_| Connection::open_in_memory())
                .map_err(|e| format!("open sqlite: {e}"))?
        } else {
            Connection::open_in_memory().map_err(|e| format!("open in-memory sqlite: {e}"))?
        };

        let idx = Self { conn };
        idx.init_schema()?;
        Ok(idx)
    }

    pub fn open_in_memory_checked() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| format!("open in-memory sqlite: {e}"))?;
        let idx = Self { conn };
        idx.init_schema()?;
        Ok(idx)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        let idx = Self { conn };
        idx.init_schema().unwrap();
        idx
    }

    fn init_schema(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 CREATE TABLE IF NOT EXISTS notes (
                     path TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     status TEXT NOT NULL,
                     tags TEXT NOT NULL,
                     excerpt TEXT NOT NULL,
                     modified INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     word_count INTEGER NOT NULL,
                     pinned INTEGER NOT NULL DEFAULT 0,
                     board_order INTEGER NOT NULL DEFAULT 0,
                     blob_sha TEXT NOT NULL DEFAULT ''
                 );
                 CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                     path UNINDEXED,
                     title,
                     tags,
                     body,
                     tokenize='unicode61'
                 );
                 CREATE TABLE IF NOT EXISTS links (
                     source_path TEXT NOT NULL,
                     target_raw TEXT NOT NULL,
                     context TEXT NOT NULL
                 );
                 CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_path);
                 CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_raw);
                 CREATE TABLE IF NOT EXISTS sync_state (
                     path TEXT PRIMARY KEY,
                     base_sha TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS meta (
                     key TEXT PRIMARY KEY,
                     value TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS collections (
                     path TEXT PRIMARY KEY,
                     kind TEXT NOT NULL,
                     title TEXT NOT NULL,
                     item_count INTEGER NOT NULL DEFAULT 0,
                     column_count INTEGER NOT NULL DEFAULT 0,
                     excerpt TEXT NOT NULL DEFAULT '',
                     modified INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     pinned INTEGER NOT NULL DEFAULT 0,
                     blob_sha TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE IF NOT EXISTS assets (
                     path TEXT PRIMARY KEY,
                     modified INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     blob_sha TEXT NOT NULL DEFAULT ''
                 );
                 CREATE TABLE IF NOT EXISTS tasks (
                     path TEXT NOT NULL,
                     line_idx INTEGER NOT NULL,
                     task_idx INTEGER NOT NULL,
                     raw_text TEXT NOT NULL,
                     clean_text TEXT NOT NULL,
                     done INTEGER NOT NULL,
                     due_date TEXT,
                     priority TEXT,
                     recurrence TEXT,
                     tags TEXT NOT NULL DEFAULT '',
                     PRIMARY KEY (path, task_idx)
                 );
                 CREATE INDEX IF NOT EXISTS idx_tasks_due ON tasks(done, due_date);",
            )
            .map_err(|e| format!("init sqlite schema: {e}"))?;

        let cols: HashSet<String> = {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(notes)")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| e.to_string())?;
            rows.flatten().collect()
        };
        if !cols.contains("pinned") {
            self.conn
                .execute(
                    "ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
                    [],
                )
                .map_err(|e| format!("migrate notes.pinned: {e}"))?;
        }
        if !cols.contains("blob_sha") {
            self.conn
                .execute(
                    "ALTER TABLE notes ADD COLUMN blob_sha TEXT NOT NULL DEFAULT ''",
                    [],
                )
                .map_err(|e| format!("migrate notes.blob_sha: {e}"))?;
        }
        if !cols.contains("board_order") {
            self.conn
                .execute(
                    "ALTER TABLE notes ADD COLUMN board_order INTEGER NOT NULL DEFAULT 0",
                    [],
                )
                .map_err(|e| format!("migrate notes.board_order: {e}"))?;
        }
        Ok(())
    }

    /// Incrementally synchronizes SQLite index with disk using `(modified_ms, size, blob_sha)` fingerprints.
    pub fn sync_from_disk(&mut self, vault_dir: &Path) -> Result<DiskSyncReport, String> {
        let disk_files = list_vault_files(vault_dir);

        let mut existing_notes: HashMap<String, (i64, u64, String)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT path, modified, size, blob_sha FROM notes")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?.max(0) as u64,
                        row.get::<_, String>(3).unwrap_or_default(),
                    ))
                })
                .map_err(|e| e.to_string())?;
            for row in rows.flatten() {
                existing_notes.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut existing_cols: HashMap<String, (i64, u64, String)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT path, modified, size, blob_sha FROM collections")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?.max(0) as u64,
                        row.get::<_, String>(3).unwrap_or_default(),
                    ))
                })
                .map_err(|e| e.to_string())?;
            for row in rows.flatten() {
                existing_cols.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut existing_assets: HashMap<String, (i64, u64, String)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT path, modified, size, blob_sha FROM assets")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?.max(0) as u64,
                        row.get::<_, String>(3).unwrap_or_default(),
                    ))
                })
                .map_err(|e| e.to_string())?;
            for row in rows.flatten() {
                existing_assets.insert(row.0, (row.1, row.2, row.3));
            }
        }

        let mut note_disk_set = HashSet::new();
        let mut col_disk_set = HashSet::new();
        let mut asset_disk_set = HashSet::new();
        let mut notes_to_upsert = Vec::new();
        let mut cols_to_upsert = Vec::new();
        let mut canvases_to_upsert = Vec::new();
        let mut assets_to_upsert = Vec::new();

        let needs_schema_refresh = self
            .get_meta("schema_rev")
            .ok()
            .flatten()
            .as_deref()
            != Some("2");

        for f in disk_files {
            match f.kind {
                VaultFileKind::Markdown => {
                    note_disk_set.insert(f.rel_path.clone());
                    let unchanged = !needs_schema_refresh
                        && existing_notes.get(&f.rel_path).is_some_and(|(m, s, sha)| {
                            *m == f.modified && *s == f.size && !sha.is_empty()
                        });
                    if !unchanged {
                        notes_to_upsert.push(f);
                    }
                }
                VaultFileKind::Board | VaultFileKind::Table => {
                    col_disk_set.insert(f.rel_path.clone());
                    let unchanged = existing_cols.get(&f.rel_path).is_some_and(|(m, s, sha)| {
                        *m == f.modified && *s == f.size && !sha.is_empty()
                    });
                    if !unchanged {
                        cols_to_upsert.push(f);
                    }
                }
                VaultFileKind::Canvas => {
                    col_disk_set.insert(f.rel_path.clone());
                    let unchanged = existing_cols.get(&f.rel_path).is_some_and(|(m, s, sha)| {
                        *m == f.modified && *s == f.size && !sha.is_empty()
                    });
                    if !unchanged {
                        canvases_to_upsert.push(f);
                    }
                }
                VaultFileKind::Asset => {
                    asset_disk_set.insert(f.rel_path.clone());
                    let unchanged = existing_assets.get(&f.rel_path).is_some_and(|(m, s, sha)| {
                        *m == f.modified && *s == f.size && !sha.is_empty()
                    });
                    if !unchanged {
                        assets_to_upsert.push(f);
                    }
                }
            }
        }

        let notes_to_delete: Vec<String> = existing_notes
            .keys()
            .filter(|p| !note_disk_set.contains(*p))
            .cloned()
            .collect();
        let cols_to_delete: Vec<String> = existing_cols
            .keys()
            .filter(|p| !col_disk_set.contains(*p))
            .cloned()
            .collect();
        let assets_to_delete: Vec<String> = existing_assets
            .keys()
            .filter(|p| !asset_disk_set.contains(*p))
            .cloned()
            .collect();

        if notes_to_upsert.is_empty()
            && notes_to_delete.is_empty()
            && cols_to_upsert.is_empty()
            && canvases_to_upsert.is_empty()
            && cols_to_delete.is_empty()
            && assets_to_upsert.is_empty()
            && assets_to_delete.is_empty()
        {
            return Ok(DiskSyncReport::default());
        }

        let mut report = DiskSyncReport::default();
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        for path in notes_to_delete {
            Self::delete_rows_tx(&tx, &path)?;
            report.deleted_paths.push(path);
        }
        for path in cols_to_delete {
            Self::delete_collection_rows_tx(&tx, &path)?;
            report.deleted_paths.push(path);
        }
        for path in assets_to_delete {
            tx.execute("DELETE FROM assets WHERE path = ?1", params![&path])
                .map_err(|e| e.to_string())?;
            report.deleted_paths.push(path);
        }
        for info in notes_to_upsert {
            if let Ok(raw_bytes) = fs::read(&info.full_path) {
                let norm = normalize_lf_bytes(&raw_bytes);
                let sha = git_blob_sha1(&norm);
                let raw = String::from_utf8_lossy(&norm);
                let stem = info
                    .full_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                let doc = NoteDocument::parse(&raw, &stem);
                Self::upsert_rows_tx(&tx, &info.rel_path, &doc, info.modified, info.size, &sha)?;
                report.changed_paths.push(info.rel_path);
            }
        }
        for info in cols_to_upsert {
            if let Ok(raw_bytes) = fs::read(&info.full_path) {
                let norm = normalize_lf_bytes(&raw_bytes);
                let sha = git_blob_sha1(&norm);
                let raw = String::from_utf8_lossy(&norm);
                let stem = vault_file_stem(&info.rel_path);
                let fallback_view = match info.kind {
                    VaultFileKind::Table => CollectionViewKind::Table,
                    _ => CollectionViewKind::Board,
                };
                if let Ok(doc) = CollectionDoc::parse(&raw, stem, fallback_view) {
                    Self::upsert_collection_rows_tx(
                        &tx,
                        &info.rel_path,
                        info.kind,
                        &doc,
                        info.modified,
                        info.size,
                        &sha,
                    )?;
                    report.changed_paths.push(info.rel_path);
                }
            }
        }
        for info in canvases_to_upsert {
            if let Ok(raw_bytes) = fs::read(&info.full_path) {
                let norm = normalize_lf_bytes(&raw_bytes);
                let sha = git_blob_sha1(&norm);
                let raw = String::from_utf8_lossy(&norm);
                let stem = vault_file_stem(&info.rel_path);
                if let Ok(doc) = CanvasDoc::parse(&raw, stem) {
                    Self::upsert_canvas_rows_tx(
                        &tx,
                        &info.rel_path,
                        &doc,
                        info.modified,
                        info.size,
                        &sha,
                    )?;
                    report.changed_paths.push(info.rel_path);
                }
            }
        }
        for info in assets_to_upsert {
            if let Ok(raw_bytes) = fs::read(&info.full_path) {
                let sha = git_blob_sha1(&raw_bytes);
                Self::upsert_asset_tx(&tx, &info.rel_path, info.modified, info.size, &sha)?;
                report.changed_paths.push(info.rel_path);
            }
        }
        if needs_schema_refresh {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('schema_rev', '2')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(report)
    }

    pub fn upsert_note(
        &mut self,
        rel_path: &str,
        doc: &NoteDocument,
        modified: i64,
        size: u64,
    ) -> Result<(), String> {
        let serialized = normalize_lf_bytes(doc.serialize().as_bytes());
        let blob_sha = git_blob_sha1(&serialized);
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::upsert_rows_tx(&tx, rel_path, doc, modified, size, &blob_sha)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn remove_note(&mut self, rel_path: &str) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::delete_rows_tx(&tx, rel_path)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn upsert_collection(
        &mut self,
        rel_path: &str,
        kind: VaultFileKind,
        doc: &CollectionDoc,
        modified: i64,
        size: u64,
    ) -> Result<(), String> {
        let json = doc.serialize_pretty()?;
        let serialized = normalize_lf_bytes(json.as_bytes());
        let blob_sha = git_blob_sha1(&serialized);
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::upsert_collection_rows_tx(&tx, rel_path, kind, doc, modified, size, &blob_sha)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn remove_collection(&mut self, rel_path: &str) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::delete_collection_rows_tx(&tx, rel_path)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn upsert_canvas(
        &mut self,
        rel_path: &str,
        doc: &CanvasDoc,
        modified: i64,
        size: u64,
    ) -> Result<(), String> {
        let json = doc.serialize_pretty()?;
        let serialized = normalize_lf_bytes(json.as_bytes());
        let blob_sha = git_blob_sha1(&serialized);
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::upsert_canvas_rows_tx(&tx, rel_path, doc, modified, size, &blob_sha)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn upsert_asset(
        &mut self,
        rel_path: &str,
        raw_bytes: &[u8],
        modified: i64,
        size: u64,
    ) -> Result<(), String> {
        let blob_sha = git_blob_sha1(raw_bytes);
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::upsert_asset_tx(&tx, rel_path, modified, size, &blob_sha)?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn list_asset_paths(&self) -> Result<Vec<String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM assets ORDER BY path COLLATE NOCASE ASC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    pub fn remove_asset(&mut self, rel_path: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM assets WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn upsert_asset_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        tx.execute(
            "INSERT INTO assets (path, modified, size, blob_sha)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               modified = excluded.modified,
               size = excluded.size,
               blob_sha = excluded.blob_sha",
            params![rel_path, modified, size as i64, blob_sha],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_rows_tx(tx: &rusqlite::Transaction<'_>, rel_path: &str) -> Result<(), String> {
        tx.execute("DELETE FROM notes WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM links WHERE source_path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM tasks WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_collection_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
    ) -> Result<(), String> {
        tx.execute("DELETE FROM collections WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM links WHERE source_path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn upsert_collection_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        kind: VaultFileKind,
        doc: &CollectionDoc,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        let kind_str = kind_to_db_str(kind);
        let excerpt: String = doc
            .records
            .iter()
            .take(3)
            .map(|r| r.title.trim())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        let search_body = doc.search_text();

        tx.execute(
            "INSERT INTO collections (path, kind, title, item_count, column_count, excerpt, modified, size, pinned, blob_sha)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(path) DO UPDATE SET
               kind = excluded.kind,
               title = excluded.title,
               item_count = excluded.item_count,
               column_count = excluded.column_count,
               excerpt = excluded.excerpt,
               modified = excluded.modified,
               size = excluded.size,
               pinned = excluded.pinned,
               blob_sha = excluded.blob_sha",
            params![
                rel_path,
                kind_str,
                doc.title,
                doc.records.len() as i64,
                doc.columns.len() as i64,
                excerpt,
                modified,
                size as i64,
                if doc.pinned { 1i64 } else { 0i64 },
                blob_sha,
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO notes_fts (path, title, tags, body) VALUES (?1, ?2, '', ?3)",
            params![rel_path, doc.title, search_body],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM links WHERE source_path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        let mut insert_link = tx
            .prepare_cached(
                "INSERT INTO links (source_path, target_raw, context) VALUES (?1, ?2, ?3)",
            )
            .map_err(|e| e.to_string())?;
        for (target, context) in doc.extract_note_links() {
            insert_link
                .execute(params![rel_path, target, context])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn upsert_canvas_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        doc: &CanvasDoc,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        let excerpt: String = doc
            .nodes
            .iter()
            .filter_map(|n| {
                let s = n.display_title();
                if s.is_empty() || s == "Card" || s == "Group" || s == "File" {
                    None
                } else {
                    Some(s)
                }
            })
            .take(3)
            .collect::<Vec<_>>()
            .join(", ");
        let search_body = doc.search_text();

        tx.execute(
            "INSERT INTO collections (path, kind, title, item_count, column_count, excerpt, modified, size, pinned, blob_sha)
             VALUES (?1, 'canvas', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(path) DO UPDATE SET
               kind = 'canvas',
               title = excluded.title,
               item_count = excluded.item_count,
               column_count = excluded.column_count,
               excerpt = excluded.excerpt,
               modified = excluded.modified,
               size = excluded.size,
               pinned = excluded.pinned,
               blob_sha = excluded.blob_sha",
            params![
                rel_path,
                doc.title,
                doc.nodes.len() as i64,
                doc.edges.len() as i64,
                excerpt,
                modified,
                size as i64,
                if doc.pinned { 1i64 } else { 0i64 },
                blob_sha,
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO notes_fts (path, title, tags, body) VALUES (?1, ?2, '', ?3)",
            params![rel_path, doc.title, search_body],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM links WHERE source_path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        let mut insert_link = tx
            .prepare_cached(
                "INSERT INTO links (source_path, target_raw, context) VALUES (?1, ?2, ?3)",
            )
            .map_err(|e| e.to_string())?;
        for (target, context) in doc.extract_note_links() {
            insert_link
                .execute(params![rel_path, target, context])
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn upsert_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        doc: &NoteDocument,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        let tags_joined = doc.frontmatter.tags.join(", ");
        let excerpt = card_preview_excerpt(&doc.body, 2, 72);
        let (word_count, _) = count_words(&doc.body);

        tx.execute(
            "INSERT INTO notes (path, title, status, tags, excerpt, modified, size, word_count, pinned, board_order, blob_sha)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET
               title = excluded.title,
               status = excluded.status,
               tags = excluded.tags,
               excerpt = excluded.excerpt,
               modified = excluded.modified,
               size = excluded.size,
               word_count = excluded.word_count,
               pinned = excluded.pinned,
               board_order = excluded.board_order,
               blob_sha = excluded.blob_sha",
            params![
                rel_path,
                doc.frontmatter.title,
                doc.frontmatter.status.map(|s| s.as_yaml_str()).unwrap_or(""),
                tags_joined,
                excerpt,
                modified,
                size as i64,
                word_count as i64,
                if doc.frontmatter.pinned { 1i64 } else { 0i64 },
                doc.frontmatter.board_order.unwrap_or(0),
                blob_sha,
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM notes_fts WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.execute(
            "INSERT INTO notes_fts (path, title, tags, body) VALUES (?1, ?2, ?3, ?4)",
            params![rel_path, doc.frontmatter.title, tags_joined, doc.body],
        )
        .map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM links WHERE source_path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        let mut insert_link = tx
            .prepare_cached(
                "INSERT INTO links (source_path, target_raw, context) VALUES (?1, ?2, ?3)",
            )
            .map_err(|e| e.to_string())?;
        for wl in extract_wikilinks(&doc.body) {
            insert_link
                .execute(params![rel_path, wl.target, wl.context])
                .map_err(|e| e.to_string())?;
        }

        tx.execute("DELETE FROM tasks WHERE path = ?1", params![rel_path])
            .map_err(|e| e.to_string())?;
        let ref_date = parse_daily_note_path(rel_path).unwrap_or_else(local_today);
        let parsed_tasks = extract_tasks(&doc.body, ref_date);
        if !parsed_tasks.is_empty() {
            let mut insert_task = tx
                .prepare_cached(
                    "INSERT INTO tasks (path, line_idx, task_idx, raw_text, clean_text, done, due_date, priority, recurrence, tags)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                )
                .map_err(|e| e.to_string())?;
            for t in parsed_tasks {
                let due_str = t.due_date.map(|d| d.format_iso());
                let prio_str = t.priority.map(|p| match p {
                    TaskPriority::High => "high",
                    TaskPriority::Medium => "med",
                    TaskPriority::Low => "low",
                });
                let rec_str = t.recurrence.map(TaskRecurrence::as_db_str);
                let tags_str = t.tags.join(", ");
                insert_task
                    .execute(params![
                        rel_path,
                        t.line_idx as i64,
                        t.task_idx as i64,
                        t.raw_text,
                        t.clean_text,
                        if t.done { 1i64 } else { 0i64 },
                        due_str,
                        prio_str,
                        rec_str,
                        tags_str,
                    ])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn list_notes(&self) -> Result<Vec<NoteMeta>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT path, title, status, tags, excerpt, modified, word_count, pinned, board_order
                 FROM notes
                 ORDER BY pinned DESC, modified DESC, title COLLATE NOCASE ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], row_to_note_meta)
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    pub fn list_tasks(&self) -> Result<Vec<VaultTaskRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT t.path, COALESCE(n.title, t.path), t.line_idx, t.task_idx,
                        t.raw_text, t.clean_text, t.done, t.due_date, t.priority, t.recurrence, t.tags
                 FROM tasks t
                 LEFT JOIN notes n ON n.path = t.path
                 ORDER BY t.done ASC,
                          CASE WHEN t.due_date IS NULL OR t.due_date = '' THEN 1 ELSE 0 END ASC,
                          t.due_date ASC,
                          t.path ASC,
                          t.task_idx ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                let due_raw: Option<String> = row.get(7)?;
                let prio_raw: Option<String> = row.get(8)?;
                let rec_raw: Option<String> = row.get(9)?;
                let tags_raw: String = row.get(10)?;
                let due_date = due_raw.as_deref().and_then(SimpleDate::parse_iso);
                let priority = match prio_raw.as_deref() {
                    Some("high") => Some(TaskPriority::High),
                    Some("med") => Some(TaskPriority::Medium),
                    Some("low") => Some(TaskPriority::Low),
                    _ => None,
                };
                let recurrence = rec_raw.as_deref().and_then(TaskRecurrence::from_db_str);
                Ok(VaultTaskRow {
                    path: row.get(0)?,
                    note_title: row.get(1)?,
                    line_idx: row.get::<_, i64>(2)?.max(0) as usize,
                    task_idx: row.get::<_, i64>(3)?.max(0) as usize,
                    raw_text: row.get(4)?,
                    clean_text: row.get(5)?,
                    done: row.get::<_, i64>(6)? != 0,
                    due_date,
                    priority,
                    recurrence,
                    tags: parse_tags_value(&tags_raw),
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    pub fn list_collections(&self) -> Result<Vec<CollectionMeta>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT path, kind, title, item_count, column_count, excerpt, modified, pinned
                 FROM collections
                 ORDER BY pinned DESC, modified DESC, title COLLATE NOCASE ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], row_to_collection_meta)
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    pub fn search(&self, raw_query: &str) -> Result<Vec<SearchHit>, String> {
        let Some(fts_query) = build_fts_query(raw_query) else {
            let mut combined: Vec<(bool, i64, SearchHit)> = Vec::new();
            for meta in self.list_notes()? {
                combined.push((
                    meta.pinned,
                    meta.modified,
                    SearchHit {
                        path: meta.path,
                        title: meta.title,
                        kind: VaultFileKind::Markdown,
                        snippet: meta.excerpt,
                    },
                ));
            }
            for col in self.list_collections()? {
                combined.push((
                    col.pinned,
                    col.modified,
                    SearchHit {
                        path: col.path,
                        title: col.title,
                        kind: col.kind,
                        snippet: col.excerpt,
                    },
                ));
            }
            combined.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| a.2.title.to_lowercase().cmp(&b.2.title.to_lowercase()))
            });
            return Ok(combined.into_iter().map(|(_, _, h)| h).collect());
        };

        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.path,
                        COALESCE(n.title, c.title, f.path),
                        COALESCE(c.kind, 'markdown'),
                        COALESCE(n.excerpt, c.excerpt, ''),
                        snippet(notes_fts, 3, '', '', '…', 16),
                        bm25(notes_fts, 5.0, 2.5, 1.0) AS rank_score
                 FROM notes_fts f
                 LEFT JOIN notes n ON n.path = f.path
                 LEFT JOIN collections c ON c.path = f.path
                 WHERE notes_fts MATCH ?1 AND (n.path IS NOT NULL OR c.path IS NOT NULL)
                 ORDER BY rank_score
                 LIMIT 50",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![fts_query], |row| {
                let path: String = row.get(0)?;
                let title: String = row.get(1)?;
                let kind_str: String = row.get(2)?;
                let fallback_excerpt: String = row.get(3)?;
                let snippet: String = row.get(4)?;
                let final_snippet = if snippet.trim().is_empty() {
                    fallback_excerpt
                } else {
                    snippet.split_whitespace().collect::<Vec<_>>().join(" ")
                };
                Ok(SearchHit {
                    path,
                    title,
                    kind: db_str_to_kind(&kind_str),
                    snippet: final_snippet,
                })
            })
            .map_err(|e| e.to_string())?;

        Ok(rows.flatten().collect())
    }

    /// O(1) SQLite lookup for incoming backlinks to `(target_path, target_title)`.
    pub fn backlinks_for(
        &self,
        target_path: &str,
        target_title: &str,
    ) -> Result<Vec<BacklinkHit>, String> {
        let stem_no_ext = target_path.strip_suffix(".md").unwrap_or(target_path);
        let file_stem = Path::new(target_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(target_title);

        let mut stmt = self
            .conn
            .prepare(
                "SELECT l.source_path, COALESCE(n.title, c.title, l.source_path) AS src_title, l.context
                 FROM links l
                 LEFT JOIN notes n ON n.path = l.source_path
                 LEFT JOIN collections c ON c.path = l.source_path
                 WHERE l.source_path != ?1
                   AND (n.path IS NOT NULL OR c.path IS NOT NULL)
                   AND (
                     lower(l.target_raw) = lower(?2)
                     OR lower(l.target_raw) = lower(?3)
                     OR lower(l.target_raw) = lower(?4)
                     OR lower(l.target_raw) = lower(?1)
                   )
                 ORDER BY src_title COLLATE NOCASE ASC",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(
                params![target_path, target_title, stem_no_ext, file_stem],
                |row| {
                    Ok(BacklinkHit {
                        source_path: row.get(0)?,
                        source_title: row.get(1)?,
                        context: row.get(2)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?;

        let mut seen_sources = HashSet::new();
        let mut out = Vec::new();
        for hit in rows.flatten() {
            if seen_sources.insert(hit.source_path.clone()) {
                out.push(hit);
            }
        }
        Ok(out)
    }

    pub fn tag_counts(&self) -> Result<Vec<(String, usize)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT tags FROM notes")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;

        let mut counts: BTreeMap<String, (String, usize)> = BTreeMap::new();
        for raw_tags in rows.flatten() {
            for tag in parse_tags_value(&raw_tags) {
                let key = tag.to_lowercase();
                counts
                    .entry(key)
                    .and_modify(|e| e.1 += 1)
                    .or_insert((tag, 1));
            }
        }

        let mut out: Vec<(String, usize)> = counts.into_values().collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        Ok(out)
    }

    pub fn load_sync_state(&self) -> Result<HashMap<String, String>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, base_sha FROM sync_state")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    pub fn save_sync_state(&mut self, state: &HashMap<String, String>) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM sync_state", [])
            .map_err(|e| e.to_string())?;
        {
            let mut stmt = tx
                .prepare("INSERT INTO sync_state (path, base_sha) VALUES (?1, ?2)")
                .map_err(|e| e.to_string())?;
            for (path, sha) in state {
                stmt.execute(params![path, sha]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn total_links_count(&self) -> usize {
        self.conn
            .query_row("SELECT COUNT(*) FROM links", [], |row| row.get::<_, i64>(0))
            .map(|n| n.max(0) as usize)
            .unwrap_or(0)
    }

    pub fn list_all_links(&self) -> Result<Vec<RawLinkEdge>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_path, target_raw FROM links")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(RawLinkEdge {
                    source_path: row.get(0)?,
                    target_raw: row.get(1)?,
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.flatten().collect())
    }

    /// Computes local unsynced count purely from SQLite (`notes.blob_sha` + `collections.blob_sha` vs `sync_state`).
    pub fn count_unsynced_changes(&self) -> usize {
        let mut local: HashMap<String, String> = HashMap::new();
        for sql in [
            "SELECT path, blob_sha FROM notes WHERE blob_sha != ''",
            "SELECT path, blob_sha FROM collections WHERE blob_sha != ''",
            "SELECT path, blob_sha FROM assets WHERE blob_sha != ''",
        ] {
            if let Ok(mut stmt) = self.conn.prepare(sql) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                }) {
                    for (p, s) in rows.flatten() {
                        local.insert(p, s);
                    }
                }
            }
        }
        let base = self.load_sync_state().unwrap_or_default();
        count_local_changes(&base, &local)
    }

    pub fn rebuild_from_disk(&mut self, vault_dir: &Path) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM notes", []).map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM collections", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM assets", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM notes_fts", [])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM links", []).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        self.sync_from_disk(vault_dir)?;
        Ok(())
    }
}

fn kind_to_db_str(kind: VaultFileKind) -> &'static str {
    match kind {
        VaultFileKind::Board => "board",
        VaultFileKind::Table => "table",
        VaultFileKind::Canvas => "canvas",
        VaultFileKind::Asset => "asset",
        VaultFileKind::Markdown => "markdown",
    }
}

fn db_str_to_kind(raw: &str) -> VaultFileKind {
    match raw {
        "board" => VaultFileKind::Board,
        "table" => VaultFileKind::Table,
        "canvas" => VaultFileKind::Canvas,
        _ => VaultFileKind::Markdown,
    }
}

fn row_to_collection_meta(row: &rusqlite::Row<'_>) -> rusqlite::Result<CollectionMeta> {
    let kind_raw: String = row.get(1)?;
    Ok(CollectionMeta {
        path: row.get(0)?,
        kind: db_str_to_kind(&kind_raw),
        title: row.get(2)?,
        item_count: row.get::<_, i64>(3)?.max(0) as usize,
        column_count: row.get::<_, i64>(4)?.max(0) as usize,
        excerpt: row.get(5)?,
        modified: row.get(6)?,
        pinned: row.get::<_, i64>(7).unwrap_or(0) != 0,
    })
}

fn row_to_note_meta(row: &rusqlite::Row<'_>) -> rusqlite::Result<NoteMeta> {
    let tags_raw: String = row.get(3)?;
    let status_raw: Option<String> = row.get(2).ok().flatten();
    Ok(NoteMeta {
        path: row.get(0)?,
        title: row.get(1)?,
        status: status_raw.as_deref().and_then(NoteStatus::parse_optional),
        tags: parse_tags_value(&tags_raw),
        excerpt: row.get(4)?,
        modified: row.get(5)?,
        word_count: row.get::<_, i64>(6)?.max(0) as usize,
        pinned: row.get::<_, i64>(7).unwrap_or(0) != 0,
        board_order: row.get::<_, i64>(8).unwrap_or(0),
    })
}

/// Safely quotes user input tokens as FTS5 prefix queries (`"token"*`),
/// preventing crashes on quotes, colons, hyphens, or boolean operators.
pub fn build_fts_query(raw: &str) -> Option<String> {
    let mut terms = Vec::new();
    for part in raw.split_whitespace() {
        let clean: String = part
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if clean.chars().any(|c| c.is_alphanumeric()) {
            terms.push(format!("\"{}\"*", clean.replace('"', "")));
        }
    }
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use krono_core::note::Frontmatter;

    #[test]
    fn fts5_search_handles_special_chars_and_cyrillic() {
        let mut idx = VaultIndex::open_in_memory();
        let doc1 = NoteDocument {
            frontmatter: Frontmatter {
                title: "Архитектура проекта".into(),
                status: Some(NoteStatus::InProgress),
                tags: vec!["rust".into(), "дизайн".into()],
                pinned: false,
                board_order: None,
                extra_lines: vec![],
            },
            body: "Быстрый нативный редактор заметок со ссылкой на [[Дорожная карта]].".into(),
        };
        let doc2 = NoteDocument {
            frontmatter: Frontmatter::new("Дорожная карта"),
            body: "Back to [[Архитектура проекта]].".into(),
        };

        idx.upsert_note("arch.md", &doc1, 100, 10).unwrap();
        idx.upsert_note("road.md", &doc2, 200, 10).unwrap();

        let hits = idx.search("нативн \"архит\" : () -").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "arch.md");

        // Ensure standalone FTS keywords inside quotes never cause a syntax error
        let _ = idx.search("AND OR NOT * ^").unwrap();

        let bl = idx
            .backlinks_for("road.md", "Дорожная карта")
            .unwrap();
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "arch.md");

        let tags = idx.tag_counts().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn persists_sync_state() {
        let mut idx = VaultIndex::open_in_memory();
        let mut state = HashMap::new();
        state.insert("a.md".into(), "deadbeef".into());
        idx.save_sync_state(&state).unwrap();
        assert_eq!(idx.load_sync_state().unwrap(), state);
    }

    #[test]
    fn counts_links_and_rebuilds_from_disk() {
        let dir = std::env::temp_dir().join(format!("krono_idx_rebuild_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("one.md"),
            "---\ntitle: One\nstatus: todo\ntags: [demo]\n---\nSee [[Two]] and [[Three]].\n",
        )
        .unwrap();

        let mut idx = VaultIndex::open_for_vault(&dir).unwrap();
        idx.sync_from_disk(&dir).unwrap();
        assert_eq!(idx.total_links_count(), 2);

        idx.set_meta("last_sync_commit", "abc1234").unwrap();
        idx.rebuild_from_disk(&dir).unwrap();
        assert_eq!(idx.list_notes().unwrap().len(), 1);
        assert_eq!(idx.total_links_count(), 2);
        assert_eq!(idx.get_meta("last_sync_commit").unwrap().as_deref(), Some("abc1234"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_notes_table_and_sorts_pinned_first() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (
                path TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                tags TEXT NOT NULL,
                excerpt TEXT NOT NULL,
                modified INTEGER NOT NULL,
                size INTEGER NOT NULL,
                word_count INTEGER NOT NULL
            );",
        )
        .unwrap();
        let idx = VaultIndex { conn };
        idx.init_schema().unwrap();

        let mut idx = idx;
        let mut d1 = NoteDocument::parse("Old note", "Old");
        d1.frontmatter.pinned = true;
        let d2 = NoteDocument::parse("New note", "New");
        idx.upsert_note("old.md", &d1, 100, 10).unwrap();
        idx.upsert_note("new.md", &d2, 999, 10).unwrap();

        let listed = idx.list_notes().unwrap();
        assert_eq!(listed[0].path, "old.md");
        assert!(listed[0].pinned);
        assert!(!listed[1].pinned);
    }

    #[test]
    fn counts_unsynced_changes_via_sql_and_recovers_corrupt_db() {
        let dir = std::env::temp_dir().join(format!("krono_idx_recover_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".krono")).unwrap();
        fs::write(dir.join(".krono/index.db"), b"NOT_A_VALID_SQLITE_HEADER").unwrap();

        let (mut idx, recovered) = VaultIndex::open_or_recover(&dir);
        assert!(recovered);

        let d1 = NoteDocument::parse("Body A", "A");
        idx.upsert_note("a.md", &d1, 100, 6).unwrap();
        assert_eq!(idx.count_unsynced_changes(), 1);

        let sha = git_blob_sha1(&normalize_lf_bytes(d1.serialize().as_bytes()));
        let mut synced = HashMap::new();
        synced.insert("a.md".into(), sha);
        idx.save_sync_state(&synced).unwrap();
        assert_eq!(idx.count_unsynced_changes(), 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn indexes_custom_collections_in_search_backlinks_and_sync_count() {
        let dir =
            std::env::temp_dir().join(format!("krono_idx_collections_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("boards")).unwrap();

        let note = NoteDocument::parse("Main architecture overview.", "Architecture");
        fs::write(dir.join("architecture.md"), note.serialize()).unwrap();

        let mut board = CollectionDoc::new_board("YouTube План", true);
        board.pinned = true;
        board.add_record("Видео про Krono", Some(("stage", "opt_work")), 10);
        board.records[0].description = "Ссылается на [[Architecture]] для деталей.".into();
        let board_json = board.serialize_pretty().unwrap();
        fs::write(dir.join("boards/youtube.board.json"), &board_json).unwrap();

        let mut idx = VaultIndex::open_for_vault(&dir).unwrap();
        let report = idx.sync_from_disk(&dir).unwrap();
        assert!(!report.is_empty());

        let cols = idx.list_collections().unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].kind, VaultFileKind::Board);
        assert!(cols[0].pinned);
        assert_eq!(cols[0].item_count, 1);

        let hits = idx.search("Krono").unwrap();
        assert!(hits
            .iter()
            .any(|h| h.path == "boards/youtube.board.json" && h.kind == VaultFileKind::Board));

        let bl = idx.backlinks_for("architecture.md", "Architecture").unwrap();
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "boards/youtube.board.json");

        assert_eq!(idx.count_unsynced_changes(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn indexes_canvases_and_lists_graph_links() {
        let dir =
            std::env::temp_dir().join(format!("krono_idx_canvas_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("scenes")).unwrap();

        let note = NoteDocument::parse("Linked scene overview.", "Scene One");
        fs::write(dir.join("scene-one.md"), note.serialize()).unwrap();

        let mut canvas = CanvasDoc::new_storyboard("Видео #1", true);
        canvas.pinned = true;
        canvas.add_file_node(320.0, 20.0, "scene-one.md", Some("2"));
        canvas.add_text_node(320.0, 180.0, "Смотри [[Scene One]]", None);
        fs::write(
            dir.join("scenes/video1.canvas"),
            canvas.serialize_pretty().unwrap(),
        )
        .unwrap();

        let mut idx = VaultIndex::open_for_vault(&dir).unwrap();
        let report = idx.sync_from_disk(&dir).unwrap();
        assert!(!report.is_empty());

        let cols = idx.list_collections().unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].kind, VaultFileKind::Canvas);
        assert!(cols[0].pinned);

        let hits = idx.search("Видео").unwrap();
        assert!(hits
            .iter()
            .any(|h| h.path == "scenes/video1.canvas" && h.kind == VaultFileKind::Canvas));

        let links = idx.list_all_links().unwrap();
        assert_eq!(links.len(), 2);

        let bl = idx.backlinks_for("scene-one.md", "Scene One").unwrap();
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0].source_path, "scenes/video1.canvas");

        assert_eq!(idx.count_unsynced_changes(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn optional_status_sqlite_roundtrip_and_null_compat() {
        let mut idx = VaultIndex::open_in_memory();
        let no_stage = NoteDocument::parse("Plain note without status", "No Stage");
        idx.upsert_note("no-stage.md", &no_stage, 100, 20).unwrap();

        let with_stage = NoteDocument::parse("---\ntitle: Staged\nstatus: Done\n---\nOk", "Staged");
        idx.upsert_note("staged.md", &with_stage, 101, 30).unwrap();

        let listed = idx.list_notes().unwrap();
        let m_none = listed.iter().find(|n| n.path == "no-stage.md").unwrap();
        let m_done = listed.iter().find(|n| n.path == "staged.md").unwrap();
        assert_eq!(m_none.status, None);
        assert_eq!(m_done.status, Some(NoteStatus::Done));

        // Legacy nullable row check via direct query using row_to_note_meta
        let meta = idx
            .conn
            .query_row(
                "SELECT 'legacy.md', 'Legacy', NULL, '', '', 10, 5, 0, 0",
                [],
                row_to_note_meta,
            )
            .unwrap();
        assert_eq!(meta.status, None);
    }

    #[test]
    fn indexes_tasks_and_cleans_up_on_delete() {
        let mut idx = VaultIndex::open_in_memory();
        let doc = NoteDocument::parse(
            "---\ntitle: Sprint\n---\n- [ ] Ship v0.3 @2026-11-25 !high @every week #release\n- [x] Setup CI\n",
            "Sprint",
        );
        idx.upsert_note("sprint.md", &doc, 200, 80).unwrap();

        let tasks = idx.list_tasks().unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].clean_text, "Ship v0.3 #release");
        assert_eq!(tasks[0].due_date, SimpleDate::from_ymd(2026, 11, 25));
        assert_eq!(tasks[0].priority, Some(TaskPriority::High));
        assert_eq!(tasks[0].recurrence, Some(TaskRecurrence::Weekly));
        assert!(!tasks[0].done);
        assert!(tasks[1].done);

        idx.remove_note("sprint.md").unwrap();
        assert!(idx.list_tasks().unwrap().is_empty());
    }
}
