use super::KronoApp;
use crate::i18n::Lang;
use crate::storage::vault::{
    create_collection_preset, create_language_lesson_note, read_canvas, save_canvas,
};
use crate::ui::{components::ToastKind, ViewMode};
use krono_core::{
    canvas::{CanvasDoc, CanvasNodeKind, CanvasSide},
    collection::CollectionViewKind,
};
use std::time::Instant;

pub fn is_english_lesson_path(rel_path: &str) -> bool {
    let norm = rel_path.replace('\\', "/").to_lowercase();
    norm.contains("/уроки/")
        || norm.contains("/lessons/")
        || norm.starts_with("уроки/")
        || norm.starts_with("lessons/")
}

pub fn is_english_canvas_path(rel_path: &str) -> bool {
    let norm = rel_path.replace('\\', "/").to_lowercase();
    norm.ends_with(".canvas")
        && (norm.ends_with("карта обучения.canvas")
            || norm.ends_with("learning map.canvas")
            || norm.starts_with("английский/")
            || norm.starts_with("english/"))
}

impl KronoApp {
    fn english_paths(&self) -> (String, String, String, String) {
        let is_ru = self.config.language == Lang::Ru;
        let base_dir: String = self
            .config
            .localized_study_language()
            .replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], " ")
            .trim()
            .to_string();
        let base_dir = if base_dir.is_empty() {
            if is_ru { "Английский" } else { "English" }.to_string()
        } else {
            base_dir
        };
        let lessons_dir = if is_ru {
            format!("{base_dir}/Уроки")
        } else {
            format!("{base_dir}/Lessons")
        };
        let vocab_rel = if is_ru {
            format!("{base_dir}/Словарь.table.json")
        } else {
            format!("{base_dir}/Vocabulary.table.json")
        };
        let canvas_rel = if is_ru {
            format!("{base_dir}/Карта обучения.canvas")
        } else {
            format!("{base_dir}/Learning Map.canvas")
        };
        (base_dir, lessons_dir, vocab_rel, canvas_rel)
    }

    pub(crate) fn open_or_create_english_workspace(&mut self) {
        self.flush_unsaved_documents();
        let is_ru = self.config.language == Lang::Ru;
        let (base_dir, lessons_dir, default_vocab, default_canvas) = self.english_paths();
        let base_prefix = format!("{base_dir}/");

        // 1. Ensure Vocabulary table exists in this study language folder
        let existing_vocab = self
            .collections
            .iter()
            .find(|c| c.path.starts_with(&base_prefix) && c.path.ends_with(".table.json"))
            .map(|c| c.path.clone());
        let vocab_path = if let Some(p) = existing_vocab {
            p
        } else {
            let existing = self.existing_paths_set();
            let raw = if is_ru {
                format!("{base_dir}/Словарь")
            } else {
                format!("{base_dir}/Vocabulary")
            };
            self.watcher.mark_internal_change();
            match create_collection_preset(
                &self.config.vault_dir,
                &raw,
                CollectionViewKind::Table,
                "vocabulary",
                is_ru,
                &existing,
            ) {
                Ok((rel, fkind, doc, modified, size)) => {
                    let _ = self.index.upsert_collection(&rel, fkind, &doc, modified, size);
                    self.reload_index_caches();
                    rel
                }
                Err(e) => {
                    self.push_toast(e, ToastKind::Error);
                    default_vocab
                }
            }
        };

        // 2. Ensure at least 1 lesson note exists under lessons_dir/
        let lessons_prefix = format!("{lessons_dir}/");
        let mut lesson_paths: Vec<String> = self
            .notes
            .iter()
            .filter(|n| n.path.starts_with(&lessons_prefix))
            .map(|n| n.path.clone())
            .collect();
        lesson_paths.sort();

        if lesson_paths.is_empty() {
            let existing = self.existing_paths_set();
            let raw_lesson = if is_ru {
                format!("{lessons_dir}/Урок 1 — Базовая лексика")
            } else {
                format!("{lessons_dir}/Lesson 1 — Core Vocabulary")
            };
            self.watcher.mark_internal_change();
            match create_language_lesson_note(
                &self.config.vault_dir,
                &raw_lesson,
                self.config.effective_study_language(),
                self.config.language,
                &existing,
            ) {
                Ok((rel, doc, modified, size)) => {
                    let _ = self.index.upsert_note(&rel, &doc, modified, size);
                    self.reload_index_caches();
                    lesson_paths.push(rel);
                }
                Err(e) => {
                    self.push_toast(e, ToastKind::Error);
                }
            }
        }

        // 3. Ensure Learning Map canvas exists in this study language folder
        let existing_canvas = self
            .collections
            .iter()
            .find(|c| c.path.starts_with(&base_prefix) && is_english_canvas_path(&c.path))
            .map(|c| c.path.clone());
        let canvas_path = existing_canvas.unwrap_or(default_canvas);

        if !self.config.vault_dir.join(&canvas_path).exists() {
            let title = if is_ru {
                format!("Карта обучения — {base_dir}")
            } else {
                format!("{base_dir} Learning Map")
            };
            let mut doc = CanvasDoc {
                title,
                pinned: false,
                nodes: Vec::new(),
                edges: Vec::new(),
            };
            doc.add_group_node(
                -380.0,
                -110.0,
                320.0,
                400.0,
                if is_ru {
                    "База и грамматика"
                } else {
                    "Core & Grammar"
                },
                Some("6"),
            );
            let vocab_node = doc.add_file_node(-350.0, -50.0, &vocab_path, Some("4"));
            let grammar_node = doc.add_text_node(
                -350.0,
                110.0,
                if is_ru {
                    "### Грамматика и план\n- Базовая структура и времена\n- Устойчивые выражения\n- Разговорная практика"
                } else {
                    "### Grammar Roadmap\n- Core Tenses & Structure\n- Key Expressions\n- Speaking Practice"
                },
                Some("6"),
            );

            let mut prev_lesson_node: Option<String> = None;
            for (idx, lp) in lesson_paths.iter().enumerate() {
                let lnode = doc.add_file_node(40.0 + (idx as f32) * 290.0, 20.0, lp, Some("5"));
                if idx == 0 {
                    doc.add_edge(
                        &vocab_node,
                        Some(CanvasSide::Right),
                        &lnode,
                        Some(CanvasSide::Left),
                        None,
                    );
                    doc.add_edge(
                        &grammar_node,
                        Some(CanvasSide::Right),
                        &lnode,
                        Some(CanvasSide::Left),
                        None,
                    );
                } else if let Some(prev) = &prev_lesson_node {
                    doc.add_edge(
                        prev,
                        Some(CanvasSide::Right),
                        &lnode,
                        Some(CanvasSide::Left),
                        None,
                    );
                }
                prev_lesson_node = Some(lnode);
            }

            self.watcher.mark_internal_change();
            match save_canvas(&self.config.vault_dir, &canvas_path, &doc) {
                Ok((modified, size)) => {
                    let _ = self.index.upsert_canvas(&canvas_path, &doc, modified, size);
                    self.reload_index_caches();
                }
                Err(e) => self.push_toast(e, ToastKind::Error),
            }
        }

        self.open_canvas(&canvas_path);
    }

    pub(crate) fn create_next_english_lesson(&mut self, open_note_after: bool) {
        self.flush_unsaved_documents();
        let is_ru = self.config.language == Lang::Ru;
        let (base_dir, lessons_dir, _vocab, default_canvas) = self.english_paths();
        let base_prefix = format!("{base_dir}/");
        let lessons_prefix = format!("{lessons_dir}/");

        let existing_lessons: Vec<String> = self
            .notes
            .iter()
            .filter(|n| n.path.starts_with(&lessons_prefix))
            .map(|n| n.path.clone())
            .collect();
        let next_num = existing_lessons.len() + 1;
        let raw_lesson = if is_ru {
            format!("{lessons_dir}/Урок {next_num}")
        } else {
            format!("{lessons_dir}/Lesson {next_num}")
        };

        let existing = self.existing_paths_set();
        self.watcher.mark_internal_change();
        let new_lesson_path = match create_language_lesson_note(
            &self.config.vault_dir,
            &raw_lesson,
            self.config.effective_study_language(),
            self.config.language,
            &existing,
        ) {
            Ok((rel, doc, modified, size)) => {
                let _ = self.index.upsert_note(&rel, &doc, modified, size);
                self.reload_index_caches();
                rel
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
                return;
            }
        };

        let canvas_path = self
            .collections
            .iter()
            .find(|c| c.path.starts_with(&base_prefix) && is_english_canvas_path(&c.path))
            .map(|c| c.path.clone())
            .unwrap_or(default_canvas);

        let is_active_canvas = self.active_canvas.path.as_deref() == Some(canvas_path.as_str());
        let mut canvas_doc = if is_active_canvas {
            Some(self.active_canvas.doc.clone())
        } else {
            read_canvas(&self.config.vault_dir, &canvas_path)
                .ok()
                .map(|(d, _, _)| d)
        };

        if let Some(ref mut doc) = canvas_doc {
            let last_lesson = doc
                .nodes
                .iter()
                .filter(|n| {
                    n.kind == CanvasNodeKind::File
                        && n.file.as_deref().is_some_and(is_english_lesson_path)
                })
                .max_by(|a, b| {
                    a.x.partial_cmp(&b.x)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|n| (n.id.clone(), n.x, n.y));

            let (new_x, new_y) = match &last_lesson {
                Some((_, lx, ly)) => (lx + 290.0, *ly),
                None => (40.0, 20.0),
            };
            let new_node_id = doc.add_file_node(new_x, new_y, &new_lesson_path, Some("5"));
            if let Some((prev_id, _, _)) = last_lesson {
                doc.add_edge(
                    &prev_id,
                    Some(CanvasSide::Right),
                    &new_node_id,
                    Some(CanvasSide::Left),
                    None,
                );
            }

            self.watcher.mark_internal_change();
            if let Ok((modified, size)) = save_canvas(&self.config.vault_dir, &canvas_path, doc) {
                let _ = self.index.upsert_canvas(&canvas_path, doc, modified, size);
                self.reload_index_caches();
                if is_active_canvas {
                    self.active_canvas.doc = doc.clone();
                    self.active_canvas.modified = modified;
                    self.active_canvas.dirty = false;
                    self.active_canvas.selected_node_id = Some(new_node_id);
                    self.active_canvas.needs_fit = true;
                    self.last_local_save_at = Some(Instant::now());
                }
            }
        }

        if open_note_after {
            self.open_note(&new_lesson_path);
            self.view_mode = ViewMode::Editor;
            self.previous_view_mode = ViewMode::Editor;
            self.last_notes_tab_mode = ViewMode::Editor;
        }
    }

    pub(crate) fn open_english_vocabulary(&mut self) {
        let (base_dir, ..) = self.english_paths();
        let base_prefix = format!("{base_dir}/");
        let found = self
            .collections
            .iter()
            .find(|c| c.path.starts_with(&base_prefix) && c.path.ends_with(".table.json"))
            .map(|c| c.path.clone());
        if let Some(path) = found {
            self.open_collection(&path);
        } else {
            self.open_or_create_english_workspace();
        }
    }
}
