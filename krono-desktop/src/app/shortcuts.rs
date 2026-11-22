use super::KronoApp;
use crate::keybindings::{KeyCode, ShortcutAction};
use crate::ui::{UiAction, ViewMode};
use eframe::egui;
use std::time::{Duration, Instant};

impl KronoApp {
    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if self.view_mode == ViewMode::Settings && self.recording_shortcut.is_some() {
            return;
        }
        let title_focused = ctx.memory(|m| m.has_focus(egui::Id::new("krono_doc_title_input")));
        let wants_text = ctx.wants_keyboard_input();
        let kb = self.config.keybindings.clone();
        let canvas_paste_ready = self.view_mode == ViewMode::Canvas
            && self.active_canvas.path.is_some()
            && self.active_canvas.editing_node_id.is_none()
            && !self.active_canvas.search_open
            && !self.active_canvas.file_picker_open
            && !ctx.wants_keyboard_input();
        let mut paste_image_requested = false;
        let mut tab_delta: Option<isize> = None;
        let mut tab_index: Option<usize> = None;
        let mut close_tab = false;
        let mut focus_zone_req: Option<crate::ui::FocusZone> = None;
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
            let fires = |action: ShortcutAction| -> bool {
                let c = kb.combo_for(action);
                if wants_text && !c.is_safe_when_typing() {
                    return false;
                }
                c.matches_input(i)
            };

            if fires(ShortcutAction::CommandPalette) {
                if self.palette_open {
                    self.palette_open = false;
                } else {
                    actions.push(UiAction::OpenPalette);
                }
            }
            if fires(ShortcutAction::SaveDocument) {
                if self.split.right.is_some()
                    && self.split.active_pane == super::split::SplitPane::Right
                {
                    self.save_split_pane();
                } else {
                    match self.view_mode {
                        ViewMode::Collection => actions.push(UiAction::SaveCurrentCollection),
                        ViewMode::Canvas => actions.push(UiAction::SaveCurrentCanvas),
                        _ => actions.push(UiAction::SaveCurrentNote),
                    }
                }
            }
            if fires(ShortcutAction::ToggleGraph) {
                if self.view_mode == ViewMode::Graph {
                    actions.push(UiAction::SetViewMode(ViewMode::Editor));
                } else {
                    actions.push(UiAction::SetViewMode(ViewMode::Graph));
                }
            }
            if fires(ShortcutAction::SyncNow) {
                actions.push(UiAction::SyncNow);
            }
            if fires(ShortcutAction::GlobalReplace) {
                actions.push(UiAction::OpenGlobalReplace);
            }
            let canvas_duplicating = self.view_mode == ViewMode::Canvas
                && !self.active_canvas.selected_node_ids_vec().is_empty()
                && self.active_canvas.editing_node_id.is_none()
                && !self.active_canvas.file_picker_open;
            if !canvas_duplicating && fires(ShortcutAction::DailyNote) {
                actions.push(UiAction::OpenTodayNote);
            }
            if fires(ShortcutAction::NavBack) {
                actions.push(UiAction::NavBack);
            }
            if fires(ShortcutAction::NavForward) {
                actions.push(UiAction::NavForward);
            }
            if fires(ShortcutAction::ToggleSidebar) {
                actions.push(UiAction::ToggleSidebar);
            }
            if fires(ShortcutAction::ToggleInspector) {
                actions.push(UiAction::ToggleInspector);
            }
            if fires(ShortcutAction::ToggleSplitView) {
                actions.push(UiAction::ToggleSplitRight);
            }
            if fires(ShortcutAction::NewItem) {
                actions.push(UiAction::OpenCreateModal(None));
            }
            if fires(ShortcutAction::QuickScratchpad)
                || (ctrl && shift && !i.modifiers.alt && i.key_pressed(egui::Key::Space))
            {
                actions.push(UiAction::OpenScratchpad);
            }
            if fires(ShortcutAction::OpenSettings) {
                if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                } else {
                    actions.push(UiAction::OpenSettings(self.settings_tab));
                }
            }
            if fires(ShortcutAction::ZoomIn) {
                actions.push(UiAction::AdjustUiZoom(0.10));
            }
            if fires(ShortcutAction::ZoomOut) {
                actions.push(UiAction::AdjustUiZoom(-0.10));
            }
            if fires(ShortcutAction::ZoomReset) {
                actions.push(UiAction::ResetUiZoom);
            }
            let modal_open = self.palette_open
                || self.create_modal.open
                || self.global_replace.open
                || self.tag_rename.open
                || self.rename_target.is_some()
                || self.delete_target.is_some()
                || self.active_collection.editing_record_id.is_some();
            if !modal_open {
                let alt = i.modifiers.alt;
                if fires(ShortcutAction::NextTab) {
                    tab_delta = Some(1);
                } else if fires(ShortcutAction::PrevTab) {
                    tab_delta = Some(-1);
                }
                if fires(ShortcutAction::CloseTab) {
                    close_tab = true;
                }
                if (cmd || ctrl) && !alt && !shift {
                    let num_keys = [
                        egui::Key::Num1,
                        egui::Key::Num2,
                        egui::Key::Num3,
                        egui::Key::Num4,
                        egui::Key::Num5,
                        egui::Key::Num6,
                        egui::Key::Num7,
                        egui::Key::Num8,
                        egui::Key::Num9,
                    ];
                    for (idx, k) in num_keys.into_iter().enumerate() {
                        if i.key_pressed(k) {
                            tab_index = Some(idx);
                            break;
                        }
                    }
                }
                if fires(ShortcutAction::FocusSidebar)
                    || (alt && !cmd && !ctrl && !shift && i.key_pressed(egui::Key::Num1))
                {
                    focus_zone_req = Some(crate::ui::FocusZone::Sidebar);
                } else if fires(ShortcutAction::FocusEditor) {
                    focus_zone_req = Some(crate::ui::FocusZone::Editor);
                } else if fires(ShortcutAction::FocusInspector) {
                    focus_zone_req = Some(crate::ui::FocusZone::Inspector);
                }
            }
            if fires(ShortcutAction::ToggleZenMode) {
                actions.push(UiAction::ToggleZenMode);
            }
            if !self.palette_open
                && !self.create_modal.open
                && !self.global_replace.open
                && !self.tag_rename.open
                && self.rename_target.is_none()
                && self.delete_target.is_none()
                && i.key_pressed(egui::Key::Escape)
            {
                if self.zen_mode {
                    self.zen_mode = false;
                } else if self.drawer_sidebar_open || self.drawer_inspector_open {
                    self.drawer_sidebar_open = false;
                    self.drawer_inspector_open = false;
                } else if self.view_mode == ViewMode::Settings {
                    actions.push(UiAction::CloseSettings);
                }
            }
        });
        if let Some(delta) = tab_delta {
            if self.config.keybindings.combo_for(ShortcutAction::NextTab).key == KeyCode::Tab
                || self.config.keybindings.combo_for(ShortcutAction::PrevTab).key == KeyCode::Tab
            {
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab);
                    i.consume_key(egui::Modifiers::CTRL | egui::Modifiers::SHIFT, egui::Key::Tab);
                });
            }
            self.switch_tab_relative(delta);
        }
        if let Some(idx) = tab_index {
            self.switch_tab_index(idx);
        }
        if close_tab {
            self.close_active_tab();
        }
        if let Some(zone) = focus_zone_req {
            match zone {
                crate::ui::FocusZone::Sidebar => {
                    if !self.config.sidebar_open {
                        self.config.sidebar_open = true;
                    }
                    self.kb_nav.focus_zone = crate::ui::FocusZone::Sidebar;
                    ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                }
                crate::ui::FocusZone::Editor => {
                    self.kb_nav.focus_zone = crate::ui::FocusZone::Editor;
                    if self.view_mode == ViewMode::Editor {
                        self.focus_body_next = true;
                    }
                }
                crate::ui::FocusZone::Inspector => {
                    if !self.config.inspector_open {
                        self.config.inspector_open = true;
                    }
                    self.kb_nav.focus_zone = crate::ui::FocusZone::Inspector;
                    ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                }
            }
        }
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
