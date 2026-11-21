use super::{ImagePickTarget, KronoApp};
use crate::i18n::{strings, Lang};
use crate::storage::vault::{
    import_image_file_to_assets, read_canvas, read_collection, read_note, save_canvas,
    save_image_bytes_to_assets,
};
use crate::ui::{
    components::ToastKind,
    image_cache::{encode_rgba_to_png, image_dimensions_from_bytes},
    EditorMode, NavTarget, ViewMode,
};
use eframe::egui;
use krono_core::path::{conflict_path, is_image_ext, vault_file_stem};
use std::{sync::mpsc, time::Instant};

impl KronoApp {
    pub(crate) fn open_canvas(&mut self, rel_path: &str) {
        self.flush_unsaved_documents();
        match read_canvas(&self.config.vault_dir, rel_path) {
            Ok((doc, modified, _)) => {
                self.active_canvas.path = Some(rel_path.to_string());
                self.active_canvas.doc = doc;
                self.active_canvas.modified = modified;
                self.active_canvas.dirty = false;
                self.active_canvas.last_edit_at = None;
                self.active_canvas.reset_transient();
                self.expand_parents(rel_path);
                if self.view_mode != ViewMode::Canvas {
                    self.view_anim_at = Some(Instant::now());
                }
                self.view_mode = ViewMode::Canvas;
                self.previous_view_mode = ViewMode::Canvas;
                self.last_notes_tab_mode = ViewMode::Canvas;
                self.last_notes_showing_list = false;
                self.record_nav(NavTarget::Canvas(rel_path.to_string()));
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn save_current_canvas(&mut self) {
        let Some(rel_path) = self.active_canvas.path.clone() else {
            return;
        };
        if self.active_canvas.doc.title.trim().is_empty() {
            let stem = krono_core::path::vault_file_stem(&rel_path).to_string();
            self.active_canvas.doc.title = if stem.is_empty() {
                "Untitled".to_string()
            } else {
                stem
            };
        }
        self.watcher.mark_internal_change();
        match save_canvas(&self.config.vault_dir, &rel_path, &self.active_canvas.doc) {
            Ok((modified, size)) => {
                self.active_canvas.modified = modified;
                self.active_canvas.dirty = false;
                self.active_canvas.external_conflict = false;
                self.active_canvas.last_edit_at = None;
                let _ = self
                    .index
                    .upsert_canvas(&rel_path, &self.active_canvas.doc, modified, size);
                self.reload_index_caches();
                self.refresh_document_derivations();
                self.refresh_embedded_boards();
                self.last_local_save_at = Some(Instant::now());
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn save_canvas_external_conflict_copy(&mut self) {
        let Some(sel) = self.active_canvas.path.clone() else {
            return;
        };
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
        let copy_rel = conflict_path(&sel, &ts);
        self.watcher.mark_internal_change();
        match save_canvas(&self.config.vault_dir, &copy_rel, &self.active_canvas.doc) {
            Ok((modified, size)) => {
                let _ = self
                    .index
                    .upsert_canvas(&copy_rel, &self.active_canvas.doc, modified, size);
                self.active_canvas.dirty = false;
                self.active_canvas.external_conflict = false;
                self.open_canvas(&sel);
                self.reload_index_caches();
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(super) fn export_active_canvas_to_markdown(&mut self) {
        if self.active_canvas.dirty {
            self.save_current_canvas();
        }
        let Some(canvas_path) = self.active_canvas.path.clone() else {
            return;
        };
        let vault_dir = self.config.vault_dir.clone();
        let md_body = self.active_canvas.doc.export_to_markdown(|file_rel| {
            if let Ok((doc, _, _)) = read_note(&vault_dir, file_rel) {
                return Some((doc.frontmatter.title, doc.body));
            }
            if let Ok((col, _, _, _)) = read_collection(&vault_dir, file_rel) {
                let txt = col.search_text();
                return Some((col.title, txt));
            }
            None
        });
        let base_title = if self.active_canvas.doc.title.trim().is_empty() {
            krono_core::path::vault_file_stem(&canvas_path).to_string()
        } else {
            self.active_canvas.doc.title.trim().to_string()
        };
        let script_title = if self.config.language == Lang::Ru {
            format!("{base_title} (Сценарий)")
        } else {
            format!("{base_title} (Script)")
        };
        let target_arg = match canvas_path.rsplit_once('/') {
            Some((folder, _)) => format!("{folder}/{script_title}"),
            None => script_title,
        };
        self.create_and_open_note(&target_arg, None, &md_body);
        let s = strings(self.config.language);
        self.push_toast(s.canvas_export_toast, ToastKind::Success);
    }


    pub(super) fn insert_markdown_into_current_note(&mut self, ctx: &egui::Context, snippet: &str) {
        if self.selected_path.is_none() {
            return;
        }
        let editor_id = if self.editor_mode == EditorMode::Split {
            egui::Id::new("krono_md_editor_split")
        } else {
            egui::Id::new("krono_md_editor_main")
        };
        let char_count = self.current_doc.body.chars().count();
        let mut inserted = false;

        if let Some(mut state) = egui::TextEdit::load_state(ctx, editor_id) {
            if let Some(range) = state.cursor.char_range() {
                let char_pos = range.primary.index.min(char_count);
                let byte_pos = self
                    .current_doc
                    .body
                    .char_indices()
                    .nth(char_pos)
                    .map(|(b, _)| b)
                    .unwrap_or(self.current_doc.body.len());
                let need_lead = byte_pos > 0 && !self.current_doc.body[..byte_pos].ends_with('\n');
                let need_trail = byte_pos < self.current_doc.body.len()
                    && !self.current_doc.body[byte_pos..].starts_with('\n');
                let mut to_insert = String::new();
                if need_lead {
                    to_insert.push('\n');
                }
                to_insert.push_str(snippet);
                if need_trail {
                    to_insert.push('\n');
                }
                self.current_doc.body.insert_str(byte_pos, &to_insert);
                let new_char = char_pos + to_insert.chars().count();
                let cc = egui::text::CCursor::new(new_char);
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                state.store(ctx, editor_id);
                inserted = true;
            }
        }

        if !inserted {
            if !self.current_doc.body.is_empty() && !self.current_doc.body.ends_with('\n') {
                self.current_doc.body.push('\n');
            }
            if !self.current_doc.body.trim_end().is_empty() {
                self.current_doc.body.push('\n');
            }
            self.current_doc.body.push_str(snippet);
            self.current_doc.body.push('\n');
        }
        self.save_current_note();
    }

    pub(super) fn add_image_card_to_active_canvas(
        &mut self,
        rel_path: &str,
        aspect_dims: Option<(u32, u32)>,
        stagger_idx: usize,
    ) {
        if self.active_canvas.path.is_none() {
            return;
        }
        self.active_canvas.push_undo();
        let (cw, ch) = match aspect_dims {
            Some((w, h)) if w > 0 && h > 0 => {
                let wf = w as f32;
                let hf = h as f32;
                let scale = (340.0 / wf).min(260.0 / hf);
                let nw = (wf * scale).clamp(150.0, 360.0);
                let nh = (hf * scale).clamp(110.0, 280.0);
                ((nw / 10.0).round() * 10.0, (nh / 10.0).round() * 10.0)
            }
            _ => (300.0, 220.0),
        };
        let center = self.active_canvas.viewport.pan;
        let offset = (stagger_idx as f32) * 28.0;
        let x = ((center.x - cw * 0.5 + offset) / 20.0).round() * 20.0;
        let y = ((center.y - ch * 0.5 + offset) / 20.0).round() * 20.0;
        let id = self.active_canvas.doc.add_file_node(x, y, rel_path, None);
        if let Some(node) = self.active_canvas.doc.nodes.iter_mut().find(|n| n.id == id) {
            node.width = cw;
            node.height = ch;
        }
        self.active_canvas.selected_node_id = Some(id);
        self.active_canvas.selected_edge_id = None;
        self.save_current_canvas();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_imported_image(
        &mut self,
        ctx: &egui::Context,
        target: ImagePickTarget,
        rel_path: String,
        bytes: &[u8],
        modified: i64,
        size: u64,
        aspect_dims: Option<(u32, u32)>,
        stagger_idx: usize,
    ) {
        let _ = self.index.upsert_asset(&rel_path, bytes, modified, size);
        self.reload_index_caches();
        self.last_local_save_at = Some(Instant::now());

        match target {
            ImagePickTarget::Editor => {
                let alt = vault_file_stem(&rel_path).to_string();
                let md = format!("![{alt}]({rel_path})");
                self.insert_markdown_into_current_note(ctx, &md);
            }
            ImagePickTarget::Canvas => {
                self.add_image_card_to_active_canvas(&rel_path, aspect_dims, stagger_idx);
            }
        }
        let s = strings(self.config.language);
        self.push_toast(format!("{} ({rel_path})", s.toast_image_inserted), ToastKind::Success);
    }

    pub(super) fn spawn_image_picker(&mut self, ctx: &egui::Context, target: ImagePickTarget) {
        if self.pending_image_pick.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let ctx_wake = ctx.clone();
        self.pending_image_pick = Some((target, rx));
        std::thread::spawn(move || {
            let picked = rfd::FileDialog::new()
                .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                .pick_file();
            let _ = tx.send(picked);
            ctx_wake.request_repaint();
        });
    }

    pub(super) fn try_paste_image_from_clipboard(&mut self, ctx: &egui::Context, warn_if_empty: bool) {
        let target = if self.view_mode == ViewMode::Canvas && self.active_canvas.path.is_some() {
            ImagePickTarget::Canvas
        } else if self.view_mode == ViewMode::Editor && self.selected_path.is_some() {
            ImagePickTarget::Editor
        } else {
            return;
        };

        let Ok(mut cb) = arboard::Clipboard::new() else {
            return;
        };
        if let Ok(img) = cb.get_image() {
            match encode_rgba_to_png(img.width, img.height, &img.bytes) {
                Ok(png_bytes) => {
                    let existing = self.existing_paths_set();
                    let stem = format!("paste-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                    self.watcher.mark_internal_change();
                    match save_image_bytes_to_assets(
                        &self.config.vault_dir,
                        &stem,
                        "png",
                        &png_bytes,
                        &existing,
                    ) {
                        Ok((rel_path, modified, size)) => {
                            let dims = Some((img.width as u32, img.height as u32));
                            self.handle_imported_image(
                                ctx,
                                target,
                                rel_path,
                                &png_bytes,
                                modified,
                                size,
                                dims,
                                0,
                            );
                            ctx.request_repaint();
                        }
                        Err(e) => self.push_toast(e, ToastKind::Error),
                    }
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
            return;
        }

        if let Ok(files) = cb.get().file_list() {
            let mut imported = 0usize;
            for path in files {
                if !is_image_ext(&path.to_string_lossy()) {
                    continue;
                }
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match import_image_file_to_assets(&self.config.vault_dir, &path, &existing) {
                    Ok((rel_path, bytes, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            imported,
                        );
                        imported += 1;
                        ctx.request_repaint();
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
            if imported > 0 {
                return;
            }
        }

        if warn_if_empty {
            let msg = if self.config.language == Lang::Ru {
                "В буфере обмена нет изображения"
            } else {
                "No image found in clipboard"
            };
            self.push_toast(msg, ToastKind::Info);
        }
    }

    pub(super) fn process_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let target = if self.view_mode == ViewMode::Canvas && self.active_canvas.path.is_some() {
            ImagePickTarget::Canvas
        } else if self.selected_path.is_some() {
            ImagePickTarget::Editor
        } else {
            return;
        };

        for (idx, df) in dropped.into_iter().enumerate() {
            if let Some(path) = df.path {
                if !is_image_ext(&path.to_string_lossy()) {
                    continue;
                }
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match import_image_file_to_assets(&self.config.vault_dir, &path, &existing) {
                    Ok((rel_path, bytes, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            idx,
                        );
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            } else if let Some(bytes) = df.bytes {
                let name = if df.name.is_empty() {
                    "dropped.png".to_string()
                } else {
                    df.name
                };
                if !is_image_ext(&name) {
                    continue;
                }
                let ext = std::path::Path::new(&name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("png");
                let stem = std::path::Path::new(&name)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("dropped");
                let existing = self.existing_paths_set();
                self.watcher.mark_internal_change();
                match save_image_bytes_to_assets(&self.config.vault_dir, stem, ext, &bytes, &existing) {
                    Ok((rel_path, modified, size)) => {
                        let dims = image_dimensions_from_bytes(&bytes);
                        self.handle_imported_image(
                            ctx,
                            target,
                            rel_path,
                            &bytes,
                            modified,
                            size,
                            dims,
                            idx,
                        );
                    }
                    Err(e) => self.push_toast(e, ToastKind::Error),
                }
            }
        }
    }

}
