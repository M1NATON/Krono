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
    pub completed_at: Option<SimpleDate>,
    pub priority: Option<TaskPriority>,
    pub recurrence: Option<TaskRecurrence>,
    pub tags: Vec<String>,
}

fn local_today() -> SimpleDate {
    let now = chrono::Local::now().date_naive();
    SimpleDate::from_ymd(now.year(), now.month(), now.day())
        .unwrap_or(SimpleDate { year: 2026, month: 1, day: 1 })
}

fn date_from_unix_ts(ts: i64) -> Option<SimpleDate> {
    if ts <= 0 {
        return None;
    }
    let dt = if ts > 100_000_000_000 {
        chrono::DateTime::from_timestamp_millis(ts)
    } else {
        chrono::DateTime::from_timestamp(ts, 0)
    }?;
    let local = dt.with_timezone(&chrono::Local).date_naive();
    SimpleDate::from_ymd(local.year(), local.month(), local.day())
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
                     blob_sha TEXT NOT NULL DEFAULT '',
                     encrypted INTEGER NOT NULL DEFAULT 0
                 );
                 CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
                     path UNINDEXED,
                     title,
                     tags,
                     body,
                     tokenize='unicode61'
                 );
                 CREATE TABLE IF NOT EXISTS fts_map (
                     path TEXT PRIMARY KEY,
                     fts_rowid INTEGER NOT NULL
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
                     completed_at TEXT,
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
        if !cols.contains("encrypted") {
            self.conn
                .execute(
                    "ALTER TABLE notes ADD COLUMN encrypted INTEGER NOT NULL DEFAULT 0",
                    [],
                )
                .map_err(|e| format!("migrate notes.encrypted: {e}"))?;
        }
        let task_cols: HashSet<String> = {
            let mut stmt = self
                .conn
                .prepare("PRAGMA table_info(tasks)")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|e| e.to_string())?;
            rows.flatten().collect()
        };
        if !task_cols.contains("completed_at") {
            self.conn
                .execute("ALTER TABLE tasks ADD COLUMN completed_at TEXT", [])
                .map_err(|e| format!("migrate tasks.completed_at: {e}"))?;
        }
        let has_fts_rows: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM notes_fts LIMIT 1)", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(0)
            != 0;
        let has_map_rows: bool = self
            .conn
            .query_row("SELECT EXISTS(SELECT 1 FROM fts_map LIMIT 1)", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap_or(0)
            != 0;
        if has_fts_rows && !has_map_rows {
            self.conn
                .execute(
                    "INSERT OR REPLACE INTO fts_map (path, fts_rowid)
                     SELECT path, rowid FROM notes_fts",
                    [],
                )
                .map_err(|e| format!("backfill fts_map: {e}"))?;
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
            != Some("3");

        for f in disk_files {
            match f.kind {
                VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => {
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
                if info.kind == VaultFileKind::EncryptedMarkdown {
                    let stem = vault_file_stem(&info.rel_path);
                    Self::upsert_encrypted_rows_tx(
                        &tx,
                        &info.rel_path,
                        stem,
                        info.modified,
                        info.size,
                        &sha,
                    )?;
                } else {
                    let raw = String::from_utf8_lossy(&norm);
                    let stem = info
                        .full_path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());
                    let doc = NoteDocument::parse(&raw, &stem);
                    Self::upsert_rows_tx(&tx, &info.rel_path, &doc, info.modified, info.size, &sha)?;
                }
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
                "INSERT INTO meta (key, value) VALUES ('schema_rev', '3')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(report)
    }

    pub fn upsert_encrypted_note(
        &mut self,
        rel_path: &str,
        title: &str,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        let tx = self.conn.transaction().map_err(|e| e.to_string())?;
        Self::upsert_encrypted_rows_tx(&tx, rel_path, title, modified, size, blob_sha)?;
        tx.commit().map_err(|e| e.to_string())
    }

    /// Indexes ONLY metadata of an encrypted `.md.enc` note — NEVER plaintext in FTS5/links/tasks.
    fn upsert_encrypted_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        title: &str,
        modified: i64,
        size: u64,
        blob_sha: &str,
    ) -> Result<(), String> {
        tx.prepare_cached(
            "INSERT INTO notes (path, title, status, tags, excerpt, modified, size, word_count, pinned, board_order, blob_sha, encrypted)
             VALUES (?1, ?2, '', '', '', ?3, ?4, 0, 0, 0, ?5, 1)
             ON CONFLICT(path) DO UPDATE SET
               title = excluded.title,
               status = '',
               tags = '',
               excerpt = '',
               modified = excluded.modified,
               size = excluded.size,
               word_count = 0,
               blob_sha = excluded.blob_sha,
               encrypted = 1",
        )
        .map_err(|e| e.to_string())?
        .execute(params![rel_path, title, modified, size as i64, blob_sha])
        .map_err(|e| e.to_string())?;
        Self::delete_fts_by_path_tx(tx, rel_path)?;
        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.prepare_cached("DELETE FROM tasks WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        Ok(())
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
        tx.prepare_cached(
            "INSERT INTO assets (path, modified, size, blob_sha)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
               modified = excluded.modified,
               size = excluded.size,
               blob_sha = excluded.blob_sha",
        )
        .map_err(|e| e.to_string())?
        .execute(params![rel_path, modified, size as i64, blob_sha])
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_fts_by_path_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
    ) -> Result<(), String> {
        let rid: Option<i64> = tx
            .prepare_cached("SELECT fts_rowid FROM fts_map WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .query_row(params![rel_path], |row| row.get(0))
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(rid) = rid {
            tx.prepare_cached("DELETE FROM notes_fts WHERE rowid = ?1")
                .map_err(|e| e.to_string())?
                .execute(params![rid])
                .map_err(|e| e.to_string())?;
            tx.prepare_cached("DELETE FROM fts_map WHERE path = ?1")
                .map_err(|e| e.to_string())?
                .execute(params![rel_path])
                .map_err(|e| e.to_string())?;
        } else {
            let map_has_rows: Option<i64> = tx
                .prepare_cached("SELECT 1 FROM fts_map LIMIT 1")
                .map_err(|e| e.to_string())?
                .query_row([], |row| row.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            if map_has_rows.is_none() {
                tx.prepare_cached("DELETE FROM notes_fts WHERE path = ?1")
                    .map_err(|e| e.to_string())?
                    .execute(params![rel_path])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn replace_fts_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
        title: &str,
        tags: &str,
        body: &str,
    ) -> Result<(), String> {
        Self::delete_fts_by_path_tx(tx, rel_path)?;
        tx.prepare_cached(
            "INSERT INTO notes_fts (path, title, tags, body) VALUES (?1, ?2, ?3, ?4)",
        )
        .map_err(|e| e.to_string())?
        .execute(params![rel_path, title, tags, body])
        .map_err(|e| e.to_string())?;
        let rid = tx.last_insert_rowid();
        tx.prepare_cached("INSERT OR REPLACE INTO fts_map (path, fts_rowid) VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path, rid])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_rows_tx(tx: &rusqlite::Transaction<'_>, rel_path: &str) -> Result<(), String> {
        tx.prepare_cached("DELETE FROM notes WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        Self::delete_fts_by_path_tx(tx, rel_path)?;
        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        tx.prepare_cached("DELETE FROM tasks WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn delete_collection_rows_tx(
        tx: &rusqlite::Transaction<'_>,
        rel_path: &str,
    ) -> Result<(), String> {
        tx.prepare_cached("DELETE FROM collections WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        Self::delete_fts_by_path_tx(tx, rel_path)?;
        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
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

        tx.prepare_cached(
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
        )
        .map_err(|e| e.to_string())?
        .execute(params![
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
        ])
        .map_err(|e| e.to_string())?;

        Self::replace_fts_tx(tx, rel_path, &doc.title, "", &search_body)?;

        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
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

        tx.prepare_cached(
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
        )
        .map_err(|e| e.to_string())?
        .execute(params![
            rel_path,
            doc.title,
            doc.nodes.len() as i64,
            doc.edges.len() as i64,
            excerpt,
            modified,
            size as i64,
            if doc.pinned { 1i64 } else { 0i64 },
            blob_sha,
        ])
        .map_err(|e| e.to_string())?;

        Self::replace_fts_tx(tx, rel_path, &doc.title, "", &search_body)?;

        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
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

        tx.prepare_cached(
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
        )
        .map_err(|e| e.to_string())?
        .execute(params![
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
        ])
        .map_err(|e| e.to_string())?;

        Self::replace_fts_tx(tx, rel_path, &doc.frontmatter.title, &tags_joined, &doc.body)?;

        tx.prepare_cached("DELETE FROM links WHERE source_path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
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

        let mut prev_completed: HashMap<String, String> = HashMap::new();
        {
            let mut prev_stmt = tx
                .prepare_cached(
                    "SELECT clean_text, completed_at FROM tasks
                     WHERE path = ?1 AND done = 1 AND completed_at IS NOT NULL AND completed_at != ''",
                )
                .map_err(|e| e.to_string())?;
            let prev_rows = prev_stmt
                .query_map(params![rel_path], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            for (ct, dt) in prev_rows.flatten() {
                prev_completed.entry(ct).or_insert(dt);
            }
        }

        tx.prepare_cached("DELETE FROM tasks WHERE path = ?1")
            .map_err(|e| e.to_string())?
            .execute(params![rel_path])
            .map_err(|e| e.to_string())?;
        let daily_date = parse_daily_note_path(rel_path);
        let ref_date = daily_date.unwrap_or_else(local_today);
        let parsed_tasks = extract_tasks(&doc.body, ref_date);
        if !parsed_tasks.is_empty() {
            let mut insert_task = tx
                .prepare_cached(
                    "INSERT INTO tasks (path, line_idx, task_idx, raw_text, clean_text, done, due_date, completed_at, priority, recurrence, tags)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                )
                .map_err(|e| e.to_string())?;
            for t in parsed_tasks {
                let due_str = t.due_date.map(|d| d.format_iso());
                let completed_str = if t.done {
                    t.completed_at
                        .map(|d| d.format_iso())
                        .or_else(|| daily_date.map(|d| d.format_iso()))
                        .or_else(|| prev_completed.get(&t.clean_text).cloned())
                        .or_else(|| date_from_unix_ts(modified).map(|d| d.format_iso()))
                } else {
                    None
                };
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
                        completed_str,
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
                        t.raw_text, t.clean_text, t.done, t.due_date, t.priority, t.recurrence, t.tags,
                        t.completed_at
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
                let completed_raw: Option<String> = row.get(11)?;
                let due_date = due_raw.as_deref().and_then(SimpleDate::parse_iso);
                let completed_at = completed_raw.as_deref().and_then(SimpleDate::parse_iso);
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
                    completed_at,
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
        let parsed = krono_core::search::parse_search_query(raw_query);
        if parsed.is_empty() {
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
        }

        let matches_filters = |path: &str, status_raw: &str, tags_raw: &str| -> bool {
            if !parsed.paths.is_empty() {
                let path_lower = path.to_lowercase();
                if !parsed.paths.iter().all(|pf| path_lower.contains(pf.as_str())) {
                    return false;
                }
            }
            if !parsed.statuses.is_empty() {
                let norm_status = status_raw.trim().to_lowercase().replace([' ', '-'], "_");
                if norm_status.is_empty()
                    || !parsed.statuses.iter().any(|sf| sf == &norm_status)
                {
                    return false;
                }
            }
            if !parsed.tags.is_empty() {
                let note_tags = parse_tags_value(tags_raw);
                for req_tag in &parsed.tags {
                    if !note_tags.iter().any(|nt| {
                        let nt_low = nt.to_lowercase();
                        nt_low.starts_with(req_tag.as_str()) || nt_low.contains(req_tag.as_str())
                    }) {
                        return false;
                    }
                }
            }
            true
        };

        let fts_query_opt = if parsed.terms.is_empty() {
            None
        } else {
            build_fts_query(&parsed.terms.join(" "))
        };

        let Some(fts_query) = fts_query_opt else {
            let mut combined: Vec<(bool, i64, SearchHit)> = Vec::new();
            let mut n_stmt = self
                .conn
                .prepare_cached(
                    "SELECT path, title, status, tags, excerpt, modified, pinned
                     FROM notes",
                )
                .map_err(|e| e.to_string())?;
            let n_rows = n_stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6).unwrap_or(0) != 0,
                    ))
                })
                .map_err(|e| e.to_string())?;
            for r in n_rows.flatten() {
                if matches_filters(&r.0, &r.2, &r.3) {
                    combined.push((
                        r.6,
                        r.5,
                        SearchHit {
                            path: r.0,
                            title: r.1,
                            kind: VaultFileKind::Markdown,
                            snippet: r.4,
                        },
                    ));
                }
            }

            if parsed.statuses.is_empty() {
                let mut c_stmt = self
                    .conn
                    .prepare_cached(
                        "SELECT path, title, kind, excerpt, modified, pinned
                         FROM collections",
                    )
                    .map_err(|e| e.to_string())?;
                let c_rows = c_stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, i64>(4)?,
                            row.get::<_, i64>(5).unwrap_or(0) != 0,
                        ))
                    })
                    .map_err(|e| e.to_string())?;
                for r in c_rows.flatten() {
                    if matches_filters(&r.0, "", "") {
                        combined.push((
                            r.5,
                            r.4,
                            SearchHit {
                                path: r.0,
                                title: r.1,
                                kind: db_str_to_kind(&r.2),
                                snippet: r.3,
                            },
                        ));
                    }
                }
            }

            combined.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| a.2.title.to_lowercase().cmp(&b.2.title.to_lowercase()))
            });
            return Ok(combined.into_iter().take(50).map(|(_, _, h)| h).collect());
        };

        let has_filters =
            !parsed.paths.is_empty() || !parsed.statuses.is_empty() || !parsed.tags.is_empty();
        let fetch_limit: i64 = if has_filters { 250 } else { 60 };

        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT f.rowid,
                        f.path,
                        COALESCE(n.title, c.title, f.path),
                        COALESCE(c.kind, 'markdown'),
                        COALESCE(n.excerpt, c.excerpt, ''),
                        COALESCE(n.status, ''),
                        COALESCE(n.tags, f.tags, '')
                 FROM notes_fts f
                 LEFT JOIN notes n ON n.path = f.path
                 LEFT JOIN collections c ON c.path = f.path
                 WHERE notes_fts MATCH ?1 AND (n.path IS NOT NULL OR c.path IS NOT NULL)
                 ORDER BY bm25(notes_fts, 5.0, 2.5, 1.0)
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;

        let rows = stmt
            .query_map(params![fts_query, fetch_limit], |row| {
                let rowid: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                let title: String = row.get(2)?;
                let kind_str: String = row.get(3)?;
                let fallback_excerpt: String = row.get(4)?;
                let status_raw: String = row.get(5)?;
                let tags_raw: String = row.get(6)?;
                Ok((
                    rowid,
                    status_raw,
                    tags_raw,
                    fallback_excerpt,
                    SearchHit {
                        path,
                        title,
                        kind: db_str_to_kind(&kind_str),
                        snippet: String::new(),
                    },
                ))
            })
            .map_err(|e| e.to_string())?;

        let mut winners: Vec<(i64, String, SearchHit)> = Vec::with_capacity(50);
        for (rowid, status_raw, tags_raw, fallback_excerpt, hit) in rows.flatten() {
            if matches_filters(&hit.path, &status_raw, &tags_raw) {
                winners.push((rowid, fallback_excerpt, hit));
                if winners.len() >= 50 {
                    break;
                }
            }
        }

        let mut snip_stmt = self
            .conn
            .prepare_cached(
                "SELECT snippet(notes_fts, 3, '', '', '…', 16)
                 FROM notes_fts
                 WHERE notes_fts MATCH ?1 AND rowid = ?2",
            )
            .map_err(|e| e.to_string())?;

        let mut out = Vec::with_capacity(winners.len());
        for (rowid, fallback_excerpt, mut hit) in winners {
            let snippet_raw: String = snip_stmt
                .query_row(params![fts_query, rowid], |row| row.get(0))
                .unwrap_or_default();
            hit.snippet = if snippet_raw.trim().is_empty() {
                fallback_excerpt
            } else {
                snippet_raw.split_whitespace().collect::<Vec<_>>().join(" ")
            };
            out.push(hit);
        }
        Ok(out)
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
        VaultFileKind::EncryptedMarkdown => "encrypted_markdown",
        VaultFileKind::Markdown => "markdown",
    }
}

fn db_str_to_kind(raw: &str) -> VaultFileKind {
    match raw {
        "board" => VaultFileKind::Board,
        "table" => VaultFileKind::Table,
        "canvas" => VaultFileKind::Canvas,
        "encrypted_markdown" => VaultFileKind::EncryptedMarkdown,
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

        // Operator search tests: tag:, status:, path:, and combined with text
        let by_tag = idx.search("tag:rust").unwrap();
        assert_eq!(by_tag.len(), 1);
        assert_eq!(by_tag[0].path, "arch.md");

        let by_status = idx.search("status:doing").unwrap();
        assert_eq!(by_status.len(), 1);
        assert_eq!(by_status[0].path, "arch.md");

        let by_path_and_text = idx.search("path:\"road\" Дорожная").unwrap();
        assert_eq!(by_path_and_text.len(), 1);
        assert_eq!(by_path_and_text[0].path, "road.md");

        let no_match = idx.search("tag:rust status:done").unwrap();
        assert!(no_match.is_empty());
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

    #[test]
    #[ignore]
    fn stress_vault_benchmark() {
        use std::time::Instant;

        struct TempDirGuard(std::path::PathBuf);
        impl Drop for TempDirGuard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        #[cfg(windows)]
        fn current_ram_mb() -> (f64, f64) {
            #[repr(C)]
            #[allow(non_snake_case)]
            struct ProcessMemoryCountersEx {
                cb: u32,
                PageFaultCount: u32,
                PeakWorkingSetSize: usize,
                WorkingSetSize: usize,
                QuotaPeakPagedPoolUsage: usize,
                QuotaPagedPoolUsage: usize,
                QuotaPeakNonPagedPoolUsage: usize,
                QuotaNonPagedPoolUsage: usize,
                PagefileUsage: usize,
                PeakPagefileUsage: usize,
                PrivateUsage: usize,
            }
            #[link(name = "kernel32")]
            extern "system" {
                fn GetCurrentProcess() -> isize;
                fn K32GetProcessMemoryInfo(
                    process: isize,
                    ppsmem_counters: *mut ProcessMemoryCountersEx,
                    cb: u32,
                ) -> i32;
            }
            let mut pmc: ProcessMemoryCountersEx = unsafe { std::mem::zeroed() };
            pmc.cb = std::mem::size_of::<ProcessMemoryCountersEx>() as u32;
            let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb) };
            if ok != 0 {
                let mb = 1024.0 * 1024.0;
                (pmc.WorkingSetSize as f64 / mb, pmc.PrivateUsage as f64 / mb)
            } else {
                (0.0, 0.0)
            }
        }

        #[cfg(not(windows))]
        fn current_ram_mb() -> (f64, f64) {
            (0.0, 0.0)
        }

        fn dir_size_bytes(dir: &Path) -> u64 {
            let mut total = 0u64;
            let Ok(entries) = std::fs::read_dir(dir) else {
                return 0;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    total += dir_size_bytes(&p);
                } else if let Ok(m) = e.metadata() {
                    total += m.len();
                }
            }
            total
        }

        fn run_scale(note_count: usize, board_count: usize, canvas_count: usize) {
            let root = std::env::temp_dir().join(format!(
                "krono_stress_{}_{}",
                std::process::id(),
                note_count
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let guard = TempDirGuard(root.clone());

            let folders = [
                "projects",
                "notes",
                "research",
                "daily/2026/11",
                "archive",
            ];
            for f in folders {
                std::fs::create_dir_all(root.join(f)).unwrap();
            }

            let filler = "Krono stores every note as plain UTF-8 Markdown with YAML frontmatter, \
                instant SQLite FTS5 indexing, wikilinks, and local-first privacy. \
                Быстрый нативный редактор на Rust без Electron и облачной привязки. \n"
                .repeat(12);

            let t_gen = Instant::now();
            for i in 0..note_count {
                let folder = folders[i % folders.len()];
                let rel = format!("{folder}/note_{i:04}.md");
                let status = match i % 3 {
                    0 => "todo",
                    1 => "in_progress",
                    _ => "done",
                };
                let link_a = (i + 1) % note_count;
                let link_b = (i + 7) % note_count;
                let link_c = (i + 31) % note_count;
                let link_d = (i + 97) % note_count;
                let content = format!(
                    "---\ntitle: Note {i:04} Architecture\nstatus: {status}\ntags: [rust, benchmark, fts5]\npinned: {}\n---\n\n\
                     # Note {i:04}\n\n\
                     Connected to [[note_{link_a:04}]], [[note_{link_b:04}]], [[note_{link_c:04}]], [[note_{link_d:04}]].\n\n\
                     - [ ] Review architecture @2026-11-25 !high\n\
                     - [ ] Verify SQLite FTS5 query @today\n\
                     - [x] Initial benchmark setup\n\
                     - [ ] Check дизайн and Cyrillic search\n\
                     - [ ] Sync with GitHub @every week\n\n\
                     {filler}",
                    i % 25 == 0
                );
                std::fs::write(root.join(&rel), content).unwrap();
            }

            for b in 0..board_count {
                let mut doc = krono_core::collection::CollectionDoc::new_board_preset(
                    &format!("Board {b:02}"),
                    "kanban",
                    false,
                );
                let stage_col = doc.effective_group_column_id().unwrap_or("stage").to_string();
                let opt_ids: Vec<String> = doc
                    .columns
                    .iter()
                    .find(|c| c.id == stage_col)
                    .map(|c| c.options.iter().map(|o| o.id.clone()).collect())
                    .unwrap_or_default();
                if !opt_ids.is_empty() {
                    for r in 0..10 {
                        let opt = &opt_ids[r % opt_ids.len()];
                        doc.add_record(&format!("Task {b}-{r}"), Some((&stage_col, opt)), (r * 10) as i64);
                    }
                }
                let json = serde_json::to_string_pretty(&doc).unwrap();
                std::fs::write(root.join(format!("projects/board_{b:02}.board.json")), json).unwrap();
            }

            for c in 0..canvas_count {
                let mut cdoc = krono_core::canvas::CanvasDoc {
                    title: format!("Canvas {c:02}"),
                    pinned: false,
                    nodes: Vec::new(),
                    edges: Vec::new(),
                };
                let mut prev: Option<String> = None;
                for n in 0..6 {
                    let id = cdoc.add_text_node(
                        (n * 240) as f32,
                        ((n % 2) * 160) as f32,
                        &format!("### Node {n}\nMarkdown card inside canvas {c}"),
                        None,
                    );
                    if let Some(p) = prev {
                        cdoc.add_edge(&p, None, &id, None, Some("link"));
                    }
                    prev = Some(id);
                }
                let json = serde_json::to_string_pretty(&cdoc).unwrap();
                std::fs::write(root.join(format!("projects/map_{c:02}.canvas")), json).unwrap();
            }
            let gen_ms = t_gen.elapsed().as_secs_f64() * 1000.0;
            let vault_mb = dir_size_bytes(&root) as f64 / (1024.0 * 1024.0);
            let (ram_ws_before, ram_priv_before) = current_ram_mb();

            let t_open = Instant::now();
            let mut idx = VaultIndex::open_for_vault(&root).unwrap();
            let cold = idx.sync_from_disk(&root).unwrap();
            let cold_ms = t_open.elapsed().as_secs_f64() * 1000.0;
            let db_mb = std::fs::metadata(root.join(".krono/index.db"))
                .map(|m| m.len() as f64 / (1024.0 * 1024.0))
                .unwrap_or(0.0);

            let t_warm = Instant::now();
            let _warm = idx.sync_from_disk(&root).unwrap();
            let warm_ms = t_warm.elapsed().as_secs_f64() * 1000.0;

            for i in 0..10.min(note_count) {
                let folder = folders[i % folders.len()];
                let p = root.join(format!("{folder}/note_{i:04}.md"));
                let mut txt = std::fs::read_to_string(&p).unwrap();
                txt.push_str("\nUpdated line for incremental sync benchmark.\n");
                std::fs::write(&p, txt).unwrap();
            }
            let t_inc = Instant::now();
            let inc = idx.sync_from_disk(&root).unwrap();
            let inc_ms = t_inc.elapsed().as_secs_f64() * 1000.0;

            let queries = [
                "architecture",
                "rust",
                "дизайн",
                "tag:rust",
                "status:in_progress",
                "path:projects",
                "tag:rust architecture",
                "sqlite fts5",
                "benchmark",
                "nonexistent_xyz",
            ];
            let mut search_total_ms = 0.0f64;
            let mut search_max_ms = 0.0f64;
            let mut total_hits = 0usize;
            for _round in 0..5 {
                for q in queries {
                    let ts = Instant::now();
                    let hits = idx.search(q).unwrap();
                    let ms = ts.elapsed().as_secs_f64() * 1000.0;
                    search_total_ms += ms;
                    if ms > search_max_ms {
                        search_max_ms = ms;
                    }
                    total_hits += hits.len();
                }
            }
            let search_avg_ms = search_total_ms / 50.0;

            let t_notes = Instant::now();
            let notes = idx.list_notes().unwrap();
            let notes_ms = t_notes.elapsed().as_secs_f64() * 1000.0;

            let t_tasks = Instant::now();
            let tasks = idx.list_tasks().unwrap();
            let tasks_ms = t_tasks.elapsed().as_secs_f64() * 1000.0;

            let t_cols = Instant::now();
            let collections = idx.list_collections().unwrap();
            let cols_ms = t_cols.elapsed().as_secs_f64() * 1000.0;

            let t_links = Instant::now();
            let links = idx.list_all_links().unwrap();
            let links_ms = t_links.elapsed().as_secs_f64() * 1000.0;

            let t_graph = Instant::now();
            let mut graph = crate::ui::graph_view::GraphState::default();
            graph.rebuild(&notes, &collections, &links);
            let graph_ms = t_graph.elapsed().as_secs_f64() * 1000.0;

            let (ram_ws_after, ram_priv_after) = current_ram_mb();

            println!(
                "\n=== STRESS SCALE: {note_count} notes, {board_count} boards, {canvas_count} canvases ==="
            );
            println!("Vault on disk:      {vault_mb:.2} MB (generated in {gen_ms:.1} ms)");
            println!(
                "Cold sync+open:     {cold_ms:.1} ms (added {}, index.db: {db_mb:.2} MB)",
                cold.changed_paths.len()
            );
            println!("Warm sync (0 chg):  {warm_ms:.1} ms");
            println!(
                "Incr sync (10 chg): {inc_ms:.1} ms (updated {})",
                inc.changed_paths.len()
            );
            println!(
                "FTS5 search (50x):  avg {search_avg_ms:.2} ms | max {search_max_ms:.2} ms (total hits: {total_hits})"
            );
            println!(
                "Load lists:         notes={} ({notes_ms:.2} ms), tasks={} ({tasks_ms:.2} ms), cols={} ({cols_ms:.2} ms), links={} ({links_ms:.2} ms)",
                notes.len(),
                tasks.len(),
                collections.len(),
                links.len()
            );
            println!("Graph rebuild:      {graph_ms:.2} ms");
            println!(
                "RAM WorkingSet:     {ram_ws_before:.1} -> {ram_ws_after:.1} MB (delta +{:.1} MB)",
                (ram_ws_after - ram_ws_before).max(0.0)
            );
            println!(
                "RAM PrivateBytes:   {ram_priv_before:.1} -> {ram_priv_after:.1} MB (delta +{:.1} MB)",
                (ram_priv_after - ram_priv_before).max(0.0)
            );

            drop(graph);
            drop(links);
            drop(collections);
            drop(tasks);
            drop(notes);
            drop(idx);
            drop(guard);
            assert!(
                !root.exists(),
                "temp stress vault must be deleted cleanly: {}",
                root.display()
            );
        }

        // Crypto benchmark on ~10 KB note
        let plain_10k = "Secret benchmark payload line with Cyrillic и ASCII.\n".repeat(180);
        let t_enc = Instant::now();
        let cipher = krono_core::crypto::encrypt_note_str(&plain_10k, "strong-stress-pass").unwrap();
        let enc_ms = t_enc.elapsed().as_secs_f64() * 1000.0;

        let t_dec = Instant::now();
        let decrypted = krono_core::crypto::decrypt_note_str(&cipher, "strong-stress-pass").unwrap();
        let dec_ms = t_dec.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(&*decrypted, &plain_10k);
        println!(
            "\n=== CRYPTO BENCHMARK (~10 KB note, Argon2id + ChaCha20Poly1305) ===\nEncrypt: {enc_ms:.1} ms | Decrypt: {dec_ms:.1} ms"
        );

        run_scale(1_000, 20, 10);
        run_scale(5_000, 50, 20);
    }

    #[test]
    fn migrates_legacy_db_without_fts_map_without_duplicates() {
        let mut idx = VaultIndex::open_in_memory();
        let doc1 = NoteDocument::parse("---\ntitle: Legacy\n---\nOld content", "Legacy");
        idx.upsert_note("legacy.md", &doc1, 100, 32).unwrap();

        // Simulate pre-v0.3.0 DB: notes_fts has the row, fts_map does not
        idx.conn.execute("DELETE FROM fts_map", []).unwrap();
        idx.init_schema().unwrap();

        let doc2 = NoteDocument::parse("---\ntitle: Legacy\n---\nUpdated content", "Legacy");
        idx.upsert_note("legacy.md", &doc2, 200, 36).unwrap();

        let count: i64 = idx
            .conn
            .query_row(
                "SELECT COUNT(*) FROM notes_fts WHERE path = ?1",
                params!["legacy.md"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn indexes_task_completed_at_from_marker_and_daily_path() {
        let mut idx = VaultIndex::open_in_memory();
        let note = NoteDocument::parse(
            "- [x] Выполнено по маркеру ✅ 2026-11-20\n- [ ] Открытая\n",
            "Tasks Note",
        );
        idx.upsert_note("tasks.md", &note, 1_763_800_000, 50).unwrap();

        let daily = NoteDocument::parse("- [x] Из дневника\n", "2026-11-19");
        idx.upsert_note("daily/2026/11/2026-11-19.md", &daily, 1_763_800_000, 30)
            .unwrap();

        let rows = idx.list_tasks().unwrap();
        let t_marker = rows.iter().find(|r| r.path == "tasks.md" && r.done).unwrap();
        assert_eq!(t_marker.completed_at, SimpleDate::from_ymd(2026, 11, 20));
        let t_open = rows.iter().find(|r| r.path == "tasks.md" && !r.done).unwrap();
        assert_eq!(t_open.completed_at, None);
        let t_daily = rows
            .iter()
            .find(|r| r.path == "daily/2026/11/2026-11-19.md")
            .unwrap();
        assert_eq!(t_daily.completed_at, SimpleDate::from_ymd(2026, 11, 19));
    }

    #[test]
    fn search_matches_cyrillic_tag_prefix() {
        let mut idx = VaultIndex::open_in_memory();
        let n1 = NoteDocument::parse(
            "---\ntitle: Фильмы\ntags: [кинотека]\n---\nСписок фильмов",
            "Фильмы",
        );
        let n2 = NoteDocument::parse(
            "---\ntitle: Книги\ntags: [литература]\n---\nСписок книг",
            "Книги",
        );
        idx.upsert_note("movies.md", &n1, 100, 40).unwrap();
        idx.upsert_note("books.md", &n2, 101, 40).unwrap();

        let hits = idx.search("tag:кин").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "movies.md");

        let hash_hits = idx.search("#ЛИТ").unwrap();
        assert_eq!(hash_hits.len(), 1);
        assert_eq!(hash_hits[0].path, "books.md");
    }
}
