use super::KronoApp;
use crate::ui::{UiAction, ViewMode};
use eframe::egui;
use std::time::{Duration, Instant};

impl KronoApp {
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let editor_focused = ctx.memory(|m| {
            m.has_focus(egui::Id::new("krono_md_editor_main"))
                || m.has_focus(egui::Id::new("krono_md_editor_split"))
        });
        let title_focused = ctx.memory(|m| m.has_focus(egui::Id::new("krono_doc_title_input")));
        let canvas_paste_ready = self.view_mode == ViewMode::Canvas
            && self.active_canvas.path.is_some()
            && self.active_canvas.editing_node_id.is_none()
            && !self.active_canvas.search_open
            && !self.active_canvas.file_picker_open
            && !ctx.wants_keyboard_input();
        let mut paste_image_requested = false;
        let mut actions = Vec::new();
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let ctrl = i.modifiers.ctrl;
            let shift = i.modifiers.shift;
            if cmd || ctrl {
                self.last_cmd_modifier_at = Some(Instant::now());
            }

            let mut saw_text_paste_this_frame = false;
            for ev in &i.events {
                if let egui::Event::Paste(t) = ev {
                    if !t.is_empty() {
                        saw_text_paste_this_frame = true;
                        self.saw_text_paste_at = Some(Instant::now());
                        if canvas_paste_ready {
                            paste_image_requested = true;
                        }
                    }
                }
            }

            // On desktop winit backends, Ctrl+V is intercepted on press for text clipboards:
            // when the clipboard holds an image or file list, egui-winit drops the Press event
            // and only delivers Key::V Release (with `logical_key.or(physical_key)` -> Key::V
            // on both Latin and Cyrillic keyboard layouts).
            for ev in &i.events {
                if let egui::Event::Key {
                    key,
                    physical_key,
                    pressed,
                    modifiers,
                    ..
                } = ev
                {
                    let is_v = *key == egui::Key::V || *physical_key == Some(egui::Key::V);
                    if is_v {
                        let cmd_held = modifiers.command || modifiers.ctrl || cmd || ctrl;
                        if *pressed {
                            if cmd_held && !modifiers.shift {
                                self.v_key_pressed_unmodified = false;
                                if !saw_text_paste_this_frame {
                                    paste_image_requested = true;
                                    self.saw_press_paste_at = Some(Instant::now());
                                }
                            } else {
                                self.v_key_pressed_unmodified = true;
                            }
                        } else if self.v_key_pressed_unmodified {
                            self.v_key_pressed_unmodified = false;
                        } else {
                            let recent_cmd = cmd_held
                                || self
                                    .last_cmd_modifier_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(700));
                            let recent_text = saw_text_paste_this_frame
                                || self
                                    .saw_text_paste_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
                            let already_pressed = self
                                .saw_press_paste_at
                                .is_some_and(|t| t.elapsed() < Duration::from_millis(450));
                            if recent_cmd && !modifiers.shift && !recent_text && !already_pressed {
                                paste_image_requested = true;
                            }
                        }
                    }

                    let is_insert =
                        *key == egui::Key::Insert || *physical_key == Some(egui::Key::Insert);
                    if is_insert {
                        if *pressed {
                            if modifiers.shift && !(modifiers.command || modifiers.ctrl) {
                                self.insert_key_pressed_unmodified = false;
                                if !saw_text_paste_this_frame {
                                    paste_image_requested = true;
                                    self.saw_press_paste_at = Some(Instant::now());
                                }
                            } else {
                                self.insert_key_pressed_unmodified = true;
                            }
                        } else if self.insert_key_pressed_unmodified {
                            self.insert_key_pressed_unmodified = false;
                        } else if modifiers.shift {
                            let recent_text = saw_text_paste_this_frame
                                || self
                                    .saw_text_paste_at
                                    .is_some_and(|t| t.elapsed() < Duration::from_millis(350));
                            let already_pressed = self
                                .saw_press_paste_at
                                .is_some_and(|t| t.elapsed() < Duration::from_millis(450));
                            if !recent_text && !already_pressed {
                                paste_image_requested = true;
                            }
                        }
                    }
                }
            }
            if cmd && i.key_pressed(egui::Key::K) {
                if self.palette_open {
                    self.palette_open = false;
                } else {
                    actions.push(UiAction::OpenPalette);
                }
            }
            if cmd && !shift && i.key_pressed(egui::Key::S) {
                match self.view_mode {
                    ViewMode::Collection => actions.push(UiAction::SaveCurrentCollection),
                    ViewMode::Canvas => actions.push(UiAction::SaveCurrentCanvas),
                    _ => actions.push(UiAction::SaveCurrentNote),
                }
            }
            if cmd && !shift && i.key_pressed(egui::Key::G) {
                if self.view_mode == ViewMode::Graph {
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                } else {
                    actions.push(UiAction::SetViewMode(ViewMode::Graph));
                }
            }
            if cmd && shift && i.key_pressed(egui::Key::S) {
                actions.push(UiAction::SyncNow);
            }
            let canvas_duplicating = self.view_mode == ViewMode::Canvas
                && !self.active_canvas.selected_node_ids_vec().is_empty()
                && self.active_canvas.editing_node_id.is_none()
                && !self.active_canvas.file_picker_open;
            if cmd && !shift && !canvas_duplicating && i.key_pressed(egui::Key::D) {
                actions.push(UiAction::OpenTodayNote);
            }
            if i.modifiers.alt && !cmd && !ctrl && !shift && i.key_pressed(egui::Key::ArrowLeft) {
                actions.push(UiAction::NavBack);
            }
            if i.modifiers.alt && !cmd && !ctrl && !shift && i.key_pressed(egui::Key::ArrowRight) {
                actions.push(UiAction::NavForward);
            }
            if cmd && i.key_pressed(egui::Key::Backslash) {
                actions.push(UiAction::ToggleSidebar);
            }
            if cmd && ((shift && i.key_pressed(egui::Key::I)) || (!shift && !editor_focused && i.key_pressed(egui::Key::I))) {
                actions.push(UiAction::ToggleInspector);
            }
            if cmd && i.key_pressed(egui::Key::N) {
                actions.push(UiAction::OpenCreateModal(None));
            }
            if cmd && i.key_pressed(egui::Key::Comma) {
                if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                } else {
                    actions.push(UiAction::OpenSettings(self.settings_tab));
                }
            }
            if !self.palette_open
                && !self.create_modal.open
                && self.rename_target.is_none()
                && self.delete_target.is_none()
                && i.key_pressed(egui::Key::Escape)
            {
                if self.drawer_sidebar_open || self.drawer_inspector_open {
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                } else if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                }
            }
        });
        if !actions.is_empty() {
            self.handle_actions(ctx, actions);
        }
        if paste_image_requested
            && !self.palette_open
            && !self.create_modal.open
            && self.rename_target.is_none()
            && self.delete_target.is_none()
            && !title_focused
            && !self.adding_tag
            && (canvas_paste_ready || (self.view_mode == ViewMode::Editor && self.selected_path.is_some()))
        {
            self.try_paste_image_from_clipboard(ctx, false);
        }
    }
}
