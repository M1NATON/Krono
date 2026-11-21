use super::KronoApp;
use crate::i18n::strings;
use crate::storage::vault::{
    full_vault_path, list_collection_files, list_markdown_files, save_collection,
    write_note_bytes_atomic,
};
use crate::ui::components::ToastKind;
use krono_core::{
    bulk_edit::{
        apply_find_replace, normalize_tag_name, preview_find_replace,
        preview_tag_rename_collection, preview_tag_rename_markdown, rename_tag_in_collection,
        rename_tag_in_markdown,
    },
    collection::CollectionDoc,
    note::NoteDocument,
    path::VaultFileKind,
};
use std::fs;

impl KronoApp {
    pub(super) fn open_global_replace_modal(&mut self) {
        self.global_replace.open = true;
        self.global_replace.excluded_paths.clear();
        if !self.global_replace.find.is_empty() {
            self.scan_global_replace();
        } else {
            self.global_replace.previews.clear();
            self.global_replace.scanned = false;
        }
    }

    pub(super) fn open_tag_rename_modal(&mut self, initial_tag: Option<String>) {
        self.tag_rename.open = true;
        self.tag_rename.excluded_paths.clear();
        if let Some(t) = initial_tag {
            self.tag_rename.old_tag = t;
        }
        if !self.tag_rename.old_tag.trim().is_empty() && !self.tag_rename.new_tag.trim().is_empty() {
            self.scan_tag_rename();
        } else {
            self.tag_rename.previews.clear();
            self.tag_rename.scanned = false;
        }
    }

    pub(super) fn scan_global_replace(&mut self) {
        self.global_replace.previews.clear();
        self.global_replace.excluded_paths.clear();
        self.global_replace.scanned = true;
        if self.global_replace.find.is_empty() {
            return;
        }

        let files = list_markdown_files(&self.config.vault_dir);
        for f in files {
            let content_opt = if self.selected_path.as_deref() == Some(f.rel_path.as_str()) {
                Some(self.current_doc.serialize())
            } else {
                full_vault_path(&self.config.vault_dir, &f.rel_path)
                    .ok()
                    .and_then(|p| fs::read_to_string(p).ok())
            };
            if let Some(content) = content_opt {
                if let Some(prev) = preview_find_replace(
                    &f.rel_path,
                    &content,
                    &self.global_replace.find,
                    &self.global_replace.replace,
                    self.global_replace.case_sensitive,
                ) {
                    self.global_replace.previews.push(prev);
                }
            }
        }
    }

    pub(super) fn apply_global_replace(&mut self) {
        let query = self.global_replace.find.clone();
        let replacement = self.global_replace.replace.clone();
        let case_sensitive = self.global_replace.case_sensitive;
        if query.is_empty() {
            return;
        }

        let targets: Vec<String> = self
            .global_replace
            .previews
            .iter()
            .filter(|p| !self.global_replace.excluded_paths.contains(&p.path))
            .map(|p| p.path.clone())
            .collect();

        if targets.is_empty() {
            return;
        }

        self.watcher.mark_internal_change();
        let mut updated_files = 0usize;
        let mut total_hits = 0usize;

        for rel_path in targets {
            let is_open_main = self.selected_path.as_deref() == Some(rel_path.as_str());
            let content_opt = if is_open_main {
                Some(self.current_doc.serialize())
            } else {
                full_vault_path(&self.config.vault_dir, &rel_path)
                    .ok()
                    .and_then(|p| fs::read_to_string(p).ok())
            };
            let Some(content) = content_opt else {
                continue;
            };
            let (new_content, count) =
                apply_find_replace(&content, &query, &replacement, case_sensitive);
            if count == 0 {
                continue;
            }
            if let Ok((modified, size)) =
                write_note_bytes_atomic(&self.config.vault_dir, &rel_path, new_content.as_bytes())
            {
                let stem = std::path::Path::new(&rel_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Untitled".to_string());
                let doc = NoteDocument::parse(&new_content, &stem);
                let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                if is_open_main {
                    self.current_doc = doc.clone();
                    self.current_modified = modified;
                    self.dirty = false;
                    self.external_conflict_open_note = false;
                    self.refresh_document_derivations();
                }
                if let Some(super::split::SplitContent::Note(ref mut sn)) = self.split.right {
                    if sn.path == rel_path {
                        sn.doc = doc;
                        sn.modified = modified;
                        sn.dirty = false;
                        sn.last_edit_at = None;
                    }
                }
                updated_files += 1;
                total_hits += count;
            }
        }

        self.reload_index_caches();
        self.global_replace.open = false;
        let s = strings(self.config.language);
        self.push_toast(
            format!("{} ({updated_files} / {total_hits})", s.toast_replace_done),
            ToastKind::Success,
        );
    }

    pub(super) fn scan_tag_rename(&mut self) {
        self.tag_rename.previews.clear();
        self.tag_rename.excluded_paths.clear();
        self.tag_rename.scanned = true;
        let old_norm = normalize_tag_name(&self.tag_rename.old_tag);
        let new_norm = normalize_tag_name(&self.tag_rename.new_tag);
        if old_norm.is_empty() || new_norm.is_empty() || old_norm.eq_ignore_ascii_case(&new_norm) {
            return;
        }

        for f in list_markdown_files(&self.config.vault_dir) {
            let content_opt = if self.selected_path.as_deref() == Some(f.rel_path.as_str()) {
                Some(self.current_doc.serialize())
            } else {
                full_vault_path(&self.config.vault_dir, &f.rel_path)
                    .ok()
                    .and_then(|p| fs::read_to_string(p).ok())
            };
            if let Some(content) = content_opt {
                if let Some(prev) =
                    preview_tag_rename_markdown(&f.rel_path, &content, &old_norm, &new_norm)
                {
                    self.tag_rename.previews.push(prev);
                }
            }
        }

        for f in list_collection_files(&self.config.vault_dir) {
            let doc_opt = if self.active_collection.path.as_deref() == Some(f.rel_path.as_str()) {
                Some(self.active_collection.doc.clone())
            } else {
                full_vault_path(&self.config.vault_dir, &f.rel_path)
                    .ok()
                    .and_then(|p| fs::read_to_string(p).ok())
                    .and_then(|raw| serde_json::from_str::<CollectionDoc>(&raw).ok())
            };
            if let Some(doc) = doc_opt {
                if let Some(prev) =
                    preview_tag_rename_collection(&f.rel_path, &doc, &old_norm, &new_norm)
                {
                    self.tag_rename.previews.push(prev);
                }
            }
        }
    }

    pub(super) fn apply_tag_rename(&mut self) {
        let old_norm = normalize_tag_name(&self.tag_rename.old_tag);
        let new_norm = normalize_tag_name(&self.tag_rename.new_tag);
        if old_norm.is_empty() || new_norm.is_empty() || old_norm.eq_ignore_ascii_case(&new_norm) {
            return;
        }

        let targets: Vec<String> = self
            .tag_rename
            .previews
            .iter()
            .filter(|p| !self.tag_rename.excluded_paths.contains(&p.path))
            .map(|p| p.path.clone())
            .collect();

        if targets.is_empty() {
            return;
        }

        self.watcher.mark_internal_change();
        let mut updated_files = 0usize;

        for rel_path in targets {
            if rel_path.ends_with(".md") {
                let is_open_main = self.selected_path.as_deref() == Some(rel_path.as_str());
                let content_opt = if is_open_main {
                    Some(self.current_doc.serialize())
                } else {
                    full_vault_path(&self.config.vault_dir, &rel_path)
                        .ok()
                        .and_then(|p| fs::read_to_string(p).ok())
                };
                let Some(content) = content_opt else {
                    continue;
                };
                let (new_content, count) = rename_tag_in_markdown(&content, &old_norm, &new_norm);
                if count == 0 {
                    continue;
                }
                if let Ok((modified, size)) = write_note_bytes_atomic(
                    &self.config.vault_dir,
                    &rel_path,
                    new_content.as_bytes(),
                ) {
                    let stem = std::path::Path::new(&rel_path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Untitled".to_string());
                    let doc = NoteDocument::parse(&new_content, &stem);
                    let _ = self.index.upsert_note(&rel_path, &doc, modified, size);
                    if is_open_main {
                        self.current_doc = doc.clone();
                        self.current_modified = modified;
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        self.refresh_document_derivations();
                    }
                    if let Some(super::split::SplitContent::Note(ref mut sn)) = self.split.right {
                        if sn.path == rel_path {
                            sn.doc = doc;
                            sn.modified = modified;
                            sn.dirty = false;
                            sn.last_edit_at = None;
                        }
                    }
                    updated_files += 1;
                }
            } else {
                let is_open_col = self.active_collection.path.as_deref() == Some(rel_path.as_str());
                let doc_opt = if is_open_col {
                    Some(self.active_collection.doc.clone())
                } else {
                    full_vault_path(&self.config.vault_dir, &rel_path)
                        .ok()
                        .and_then(|p| fs::read_to_string(p).ok())
                        .and_then(|raw| serde_json::from_str::<CollectionDoc>(&raw).ok())
                };
                let Some(mut doc) = doc_opt else {
                    continue;
                };
                if rename_tag_in_collection(&mut doc, &old_norm, &new_norm) == 0 {
                    continue;
                }
                if let Ok((modified, size)) =
                    save_collection(&self.config.vault_dir, &rel_path, &doc)
                {
                    let kind = VaultFileKind::from_path(&rel_path).unwrap_or(VaultFileKind::Table);
                    let _ = self
                        .index
                        .upsert_collection(&rel_path, kind, &doc, modified, size);
                    if is_open_col {
                        self.active_collection.doc = doc.clone();
                        self.active_collection.dirty = false;
                    }
                    if let Some(super::split::SplitContent::Collection(ref mut sc)) =
                        self.split.right
                    {
                        if sc.path.as_deref() == Some(rel_path.as_str()) {
                            sc.doc = doc;
                            sc.dirty = false;
                            sc.modified = modified;
                        }
                    }
                    updated_files += 1;
                }
            }
        }

        if self
            .active_tag_filter
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(&old_norm))
        {
            self.active_tag_filter = Some(new_norm.clone());
        }
        self.reload_index_caches();
        self.tag_rename.open = false;
        let s = strings(self.config.language);
        self.push_toast(
            format!("{} #{old_norm} → #{new_norm} ({updated_files})", s.toast_tag_renamed),
            ToastKind::Success,
        );
    }
}
