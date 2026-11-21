use crate::i18n::{strings, Lang};
use crate::storage::index::CollectionMeta;
use crate::ui::components::{
    context_menu_row, focus_once, hairline_h, icon_button, primary_button, reset_focus_gate,
    secondary_button, styled_text_input,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::image_cache::ImageTextureCache;
use crate::ui::markdown_preview::{
    render_markdown_preview, resolve_embedded_note, EmbeddedNoteData, PreviewAction,
};
use crate::ui::motion::{lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::viewport_2d::{draw_dot_grid, render_zoom_hud, Viewport2D};
use crate::ui::{LayoutClass, UiAction};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, Layout, Margin, Pos2, Rect, Rounding,
    Sense, Stroke, Ui, Vec2,
};
use krono_core::{
    canvas::{
        canvas_folder_for_path, suggest_note_title_from_card, CanvasDoc, CanvasEnd, CanvasNode,
        CanvasNodeKind, CanvasSide,
    },
    collection::{CollectionDoc, ColorToken},
    daily::SimpleDate,
    note::NoteMeta,
    path::{vault_file_stem, VaultFileKind},
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Instant,
};

pub struct CanvasMarkdownCtx<'a> {
    pub today: SimpleDate,
    pub heading_serif: bool,
    pub resolved_titles: &'a HashSet<String>,
    pub copied_code_hash: Option<u64>,
    pub embedded_boards: &'a HashMap<String, CollectionDoc>,
    pub available_boards: &'a [(String, String)],
    pub embedded_notes: &'a HashMap<String, EmbeddedNoteData>,
}

#[derive(Clone, Debug)]
pub struct DraggingNode {
    pub start_world: Pos2,
    pub orig_positions: Vec<(String, Pos2)>,
}

#[derive(Clone, Debug)]
pub struct ResizingNode {
    pub node_id: String,
    pub start_world: Pos2,
    pub orig_size: Vec2,
}

pub struct CanvasEditorState {
    pub path: Option<String>,
    pub doc: CanvasDoc,
    pub modified: i64,
    pub dirty: bool,
    pub last_edit_at: Option<Instant>,
    pub external_conflict: bool,
    pub viewport: Viewport2D,
    pub needs_fit: bool,
    pub snap_to_grid: bool,
    pub selected_node_id: Option<String>,
    pub selected_node_ids: HashSet<String>,
    pub selected_edge_id: Option<String>,
    pub editing_node_id: Option<String>,
    pub dragging_node: Option<DraggingNode>,
    pub resizing_node: Option<ResizingNode>,
    pub connecting_from: Option<(String, CanvasSide)>,
    pub marquee_mode: bool,
    pub marquee_start_world: Option<Pos2>,
    pub marquee_current_world: Option<Pos2>,
    pub smart_guides_x: Vec<f32>,
    pub smart_guides_y: Vec<f32>,
    pub pressed_on_empty_bg: bool,
    pub drag_undo_pushed: bool,
    pub undo_stack: Vec<CanvasDoc>,
    pub redo_stack: Vec<CanvasDoc>,
    pub show_minimap: bool,
    pub context_menu_pos: Option<Pos2>,
    pub context_menu_world: Pos2,
    pub context_menu_node: Option<String>,
    pub context_menu_edge: Option<String>,
    pub file_picker_open: bool,
    pub file_picker_world: Option<Pos2>,
    pub file_picker_query: String,
    pub convert_card_modal: Option<(String, String)>,
    pub edge_label_draft: String,
    pub search_open: bool,
    pub search_query: String,
    pub focus_edge_label_next: bool,
}

impl Default for CanvasEditorState {
    fn default() -> Self {
        Self {
            path: None,
            doc: CanvasDoc::default(),
            modified: 0,
            dirty: false,
            last_edit_at: None,
            external_conflict: false,
            viewport: Viewport2D::default(),
            needs_fit: true,
            snap_to_grid: true,
            selected_node_id: None,
            selected_node_ids: HashSet::new(),
            selected_edge_id: None,
            editing_node_id: None,
            dragging_node: None,
            resizing_node: None,
            connecting_from: None,
            marquee_mode: false,
            marquee_start_world: None,
            marquee_current_world: None,
            smart_guides_x: Vec::new(),
            smart_guides_y: Vec::new(),
            pressed_on_empty_bg: false,
            drag_undo_pushed: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            show_minimap: true,
            context_menu_pos: None,
            context_menu_world: Pos2::ZERO,
            context_menu_node: None,
            context_menu_edge: None,
            file_picker_open: false,
            file_picker_world: None,
            file_picker_query: String::new(),
            convert_card_modal: None,
            edge_label_draft: String::new(),
            search_open: false,
            search_query: String::new(),
            focus_edge_label_next: false,
        }
    }
}

impl CanvasEditorState {
    pub fn reset_transient(&mut self) {
        self.external_conflict = false;
        self.needs_fit = true;
        self.selected_node_id = None;
        self.selected_node_ids.clear();
        self.selected_edge_id = None;
        self.editing_node_id = None;
        self.dragging_node = None;
        self.resizing_node = None;
        self.connecting_from = None;
        self.marquee_mode = false;
        self.marquee_start_world = None;
        self.marquee_current_world = None;
        self.smart_guides_x.clear();
        self.smart_guides_y.clear();
        self.pressed_on_empty_bg = false;
        self.drag_undo_pushed = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.context_menu_pos = None;
        self.context_menu_node = None;
        self.context_menu_edge = None;
        self.file_picker_open = false;
        self.file_picker_world = None;
        self.file_picker_query.clear();
        self.convert_card_modal = None;
        self.edge_label_draft.clear();
        self.search_open = false;
        self.search_query.clear();
        self.focus_edge_label_next = false;
    }

    pub fn push_undo(&mut self) {
        if self.undo_stack.last().is_some_and(|top| *top == self.doc) {
            return;
        }
        if self.undo_stack.len() >= 50 {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.doc.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(self.doc.clone());
        self.doc = prev;
        self.reconcile_selection_after_doc_replace();
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(self.doc.clone());
        self.doc = next;
        self.reconcile_selection_after_doc_replace();
        true
    }

    pub fn select_single_node(&mut self, id: String) {
        self.selected_node_ids.clear();
        self.selected_node_ids.insert(id.clone());
        self.selected_node_id = Some(id);
        self.selected_edge_id = None;
    }

    pub fn toggle_node_selection(&mut self, id: String) {
        if self.selected_node_ids.contains(&id) {
            self.selected_node_ids.remove(&id);
            if self.selected_node_id.as_deref() == Some(id.as_str()) {
                self.selected_node_id = self.selected_node_ids.iter().next().cloned();
            }
        } else {
            self.selected_node_ids.insert(id.clone());
            self.selected_node_id = Some(id);
        }
        self.selected_edge_id = None;
    }

    pub fn clear_selection(&mut self) {
        self.selected_node_id = None;
        self.selected_node_ids.clear();
        self.selected_edge_id = None;
    }

    pub fn is_node_selected(&self, id: &str) -> bool {
        self.selected_node_id.as_deref() == Some(id) || self.selected_node_ids.contains(id)
    }

    pub fn selected_node_ids_vec(&self) -> Vec<String> {
        let mut out = Vec::new();
        for n in &self.doc.nodes {
            if self.is_node_selected(&n.id) {
                out.push(n.id.clone());
            }
        }
        out
    }

    pub fn align_selected_left(&mut self) -> bool {
        let ids: HashSet<String> = self.selected_node_ids_vec().into_iter().collect();
        if ids.len() < 2 {
            return false;
        }
        let min_x = self
            .doc
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .map(|n| n.x)
            .fold(f32::INFINITY, f32::min);
        if !min_x.is_finite() {
            return false;
        }
        let any_diff = self
            .doc
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .any(|n| (n.x - min_x).abs() > 0.5);
        if !any_diff {
            return false;
        }
        self.push_undo();
        for n in &mut self.doc.nodes {
            if ids.contains(&n.id) {
                n.x = min_x;
            }
        }
        true
    }

    pub fn align_selected_top(&mut self) -> bool {
        let ids: HashSet<String> = self.selected_node_ids_vec().into_iter().collect();
        if ids.len() < 2 {
            return false;
        }
        let min_y = self
            .doc
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .map(|n| n.y)
            .fold(f32::INFINITY, f32::min);
        if !min_y.is_finite() {
            return false;
        }
        let any_diff = self
            .doc
            .nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .any(|n| (n.y - min_y).abs() > 0.5);
        if !any_diff {
            return false;
        }
        self.push_undo();
        for n in &mut self.doc.nodes {
            if ids.contains(&n.id) {
                n.y = min_y;
            }
        }
        true
    }

    fn reconcile_selection_after_doc_replace(&mut self) {
        self.selected_node_ids
            .retain(|nid| self.doc.nodes.iter().any(|n| &n.id == nid));
        if let Some(nid) = &self.selected_node_id {
            if !self.doc.nodes.iter().any(|n| &n.id == nid) {
                self.selected_node_id = self.selected_node_ids.iter().next().cloned();
            }
        }
        if let Some(eid) = &self.selected_edge_id {
            if !self.doc.edges.iter().any(|e| &e.id == eid) {
                self.selected_edge_id = None;
            }
        }
        self.editing_node_id = None;
        self.dragging_node = None;
        self.resizing_node = None;
        self.marquee_start_world = None;
        self.marquee_current_world = None;
    }

    pub fn duplicate_selected(&mut self) -> bool {
        let ids = self.selected_node_ids_vec();
        if ids.is_empty() {
            return false;
        }
        self.push_undo();
        let mut used: HashSet<String> = self.doc.nodes.iter().map(|n| n.id.clone()).collect();
        let mut idx = 1usize;
        let mut new_ids = Vec::new();
        for nid in ids {
            let Some(mut copy) = self.doc.nodes.iter().find(|n| n.id == nid).cloned() else {
                continue;
            };
            let new_id = loop {
                let cand = format!("node_{idx}");
                idx += 1;
                if !used.contains(&cand) {
                    used.insert(cand.clone());
                    break cand;
                }
            };
            copy.id = new_id.clone();
            copy.x += 28.0;
            copy.y += 28.0;
            self.doc.nodes.push(copy);
            new_ids.push(new_id);
        }
        if new_ids.is_empty() {
            return false;
        }
        self.selected_node_ids = new_ids.iter().cloned().collect();
        self.selected_node_id = new_ids.first().cloned();
        self.selected_edge_id = None;
        self.editing_node_id = None;
        true
    }

    pub fn auto_layout(&mut self) -> bool {
        if self.doc.nodes.is_empty() {
            return false;
        }
        self.push_undo();

        let groups: Vec<CanvasNode> = self
            .doc
            .nodes
            .iter()
            .filter(|n| n.kind == CanvasNodeKind::Group)
            .cloned()
            .collect();
        let mut children_by_group: Vec<Vec<usize>> = vec![Vec::new(); groups.len()];
        let mut loose: Vec<usize> = Vec::new();

        for (i, n) in self.doc.nodes.iter().enumerate() {
            if n.kind == CanvasNodeKind::Group {
                continue;
            }
            let nc = pos2(n.x + n.width * 0.5, n.y + n.height * 0.5);
            if let Some((gi, _)) = groups
                .iter()
                .enumerate()
                .find(|(_, g)| node_world_rect(g).contains(nc))
            {
                children_by_group[gi].push(i);
            } else {
                loose.push(i);
            }
        }

        // Layout groups along a clean horizontal shelf, moving each group's cards with it
        let mut gx = 0.0_f32;
        let mut max_grp_bottom = 0.0_f32;
        let group_idxs: Vec<usize> = self
            .doc
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == CanvasNodeKind::Group)
            .map(|(i, _)| i)
            .collect();
        for (slot, gi) in group_idxs.into_iter().enumerate() {
            let old_x = self.doc.nodes[gi].x;
            let old_y = self.doc.nodes[gi].y;
            let dx = gx - old_x;
            let dy = 0.0 - old_y;
            self.doc.nodes[gi].x += dx;
            self.doc.nodes[gi].y += dy;
            if let Some(ch) = children_by_group.get(slot) {
                for &ci in ch {
                    self.doc.nodes[ci].x += dx;
                    self.doc.nodes[ci].y += dy;
                }
            }
            gx += self.doc.nodes[gi].width + 44.0;
            max_grp_bottom = max_grp_bottom.max(self.doc.nodes[gi].height + 48.0);
        }

        // Layout loose nodes in balanced 3-column grid below groups (or from y=0 if no groups)
        let cols = 3usize.min(loose.len().max(1));
        let mut col_x = 0.0_f32;
        let mut row_y = max_grp_bottom;
        let mut row_max_h = 0.0_f32;
        for (k, &ni) in loose.iter().enumerate() {
            if k > 0 && k % cols == 0 {
                col_x = 0.0;
                row_y += row_max_h + 36.0;
                row_max_h = 0.0;
            }
            self.doc.nodes[ni].x = snap(col_x, self.snap_to_grid);
            self.doc.nodes[ni].y = snap(row_y, self.snap_to_grid);
            col_x += self.doc.nodes[ni].width + 40.0;
            row_max_h = row_max_h.max(self.doc.nodes[ni].height);
        }

        self.needs_fit = true;
        true
    }
}

const PALETTE_SWATCHES: [(&str, ColorToken); 6] = [
    ("1", ColorToken::Rose),
    ("2", ColorToken::Coral),
    ("3", ColorToken::Amber),
    ("4", ColorToken::Green),
    ("5", ColorToken::Teal),
    ("6", ColorToken::Violet),
];

fn parse_hex_color(raw: &str) -> Option<Color32> {
    let hex = raw.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

fn canvas_accent_color(p: &ThemePalette, color: Option<&str>) -> Option<Color32> {
    let raw = color?.trim();
    if let Some((_, tok)) = PALETTE_SWATCHES.iter().find(|(k, _)| *k == raw) {
        return Some(p.token_colors(*tok).1);
    }
    parse_hex_color(raw)
}

fn snap(v: f32, enabled: bool) -> f32 {
    if enabled {
        (v / 20.0).round() * 20.0
    } else {
        v
    }
}

fn node_world_rect(n: &CanvasNode) -> Rect {
    Rect::from_min_size(pos2(n.x, n.y), vec2(n.width.max(120.0), n.height.max(64.0)))
}

fn port_world_pos(n: &CanvasNode, side: CanvasSide) -> Pos2 {
    let r = node_world_rect(n);
    match side {
        CanvasSide::Top => pos2(r.center().x, r.top()),
        CanvasSide::Right => pos2(r.right(), r.center().y),
        CanvasSide::Bottom => pos2(r.center().x, r.bottom()),
        CanvasSide::Left => pos2(r.left(), r.center().y),
    }
}

fn side_tangent(side: CanvasSide) -> Vec2 {
    match side {
        CanvasSide::Top => vec2(0.0, -1.0),
        CanvasSide::Right => vec2(1.0, 0.0),
        CanvasSide::Bottom => vec2(0.0, 1.0),
        CanvasSide::Left => vec2(-1.0, 0.0),
    }
}

fn choose_best_sides(a: &CanvasNode, b: &CanvasNode) -> (CanvasSide, CanvasSide) {
    let ca = node_world_rect(a).center();
    let cb = node_world_rect(b).center();
    let dx = cb.x - ca.x;
    let dy = cb.y - ca.y;
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            (CanvasSide::Right, CanvasSide::Left)
        } else {
            (CanvasSide::Left, CanvasSide::Right)
        }
    } else if dy >= 0.0 {
        (CanvasSide::Bottom, CanvasSide::Top)
    } else {
        (CanvasSide::Top, CanvasSide::Bottom)
    }
}

fn sample_cubic_bezier(p0: Pos2, c1: Pos2, c2: Pos2, p3: Pos2, steps: usize) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = (i as f32) / (steps as f32);
        let u = 1.0 - t;
        let uu = u * u;
        let tt = t * t;
        let x = uu * u * p0.x + 3.0 * uu * t * c1.x + 3.0 * u * tt * c2.x + tt * t * p3.x;
        let y = uu * u * p0.y + 3.0 * uu * t * c1.y + 3.0 * u * tt * c2.y + tt * t * p3.y;
        pts.push(pos2(x, y));
    }
    pts
}

fn point_to_seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 0.0001 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn dist_to_polyline(pts: &[Pos2], p: Pos2) -> f32 {
    pts.windows(2)
        .map(|w| point_to_seg_dist(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn draw_arrow_head(
    painter: &egui::Painter,
    tip: Pos2,
    dir: Vec2,
    color: Color32,
    size: f32,
) {
    let n = dir.normalized();
    if n.length_sq() < 0.5 {
        return;
    }
    let perp = vec2(-n.y, n.x);
    let back = tip - n * size;
    let left = back + perp * (size * 0.48);
    let right = back - perp * (size * 0.48);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, left, right],
        color,
        Stroke::NONE,
    ));
}

fn canvas_world_bounds(doc: &CanvasDoc) -> Option<Rect> {
    if doc.nodes.is_empty() {
        return None;
    }
    let mut r = node_world_rect(&doc.nodes[0]);
    for n in &doc.nodes[1..] {
        r = r.union(node_world_rect(n));
    }
    Some(r)
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn render_canvas_view(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    state: &mut CanvasEditorState,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_dir: &Path,
    image_cache: &mut ImageTextureCache,
    md_ctx: &CanvasMarkdownCtx<'_>,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    let canvas_rect = ui.available_rect_before_wrap();
    if canvas_rect.width() < 20.0 || canvas_rect.height() < 20.0 {
        return actions;
    }

    let bounds = canvas_world_bounds(&state.doc);
    if state.needs_fit {
        if let Some(b) = bounds {
            state.viewport.fit_bounds(canvas_rect, b, 68.0, true);
        } else {
            state.viewport = Viewport2D::default();
        }
        state.needs_fit = false;
    }
    state.viewport.step_animation(ctx);

    let zoom = state.viewport.zoom;

    // 1. Background interactive area + dot grid
    let bg_resp = ui.interact(
        canvas_rect,
        ui.make_persistent_id("krono_canvas_bg"),
        Sense::click_and_drag(),
    );
    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, Rounding::ZERO, p.bg_canvas);
    draw_dot_grid(
        &painter,
        canvas_rect,
        &state.viewport,
        24.0,
        with_alpha(p.border_hairline, 0.85),
    );

    let mut dirty = false;

    // Keyboard shortcuts: Delete/Backspace, Ctrl+Z/Y, Ctrl+D Duplicate, Ctrl+F Search, Arrow Nudging
    if state.editing_node_id.is_none()
        && !state.file_picker_open
        && state.convert_card_modal.is_none()
        && !ctx.wants_keyboard_input()
    {
        let mut undo_hit = false;
        let mut redo_hit = false;
        let mut dup_hit = false;
        let mut del_hit = false;
        let mut search_hit = false;
        let mut marquee_hit = false;
        let mut nudge = Vec2::ZERO;

        ctx.input(|i| {
            let cmd = i.modifiers.command;
            let shift = i.modifiers.shift;
            if cmd && i.key_pressed(egui::Key::F) {
                search_hit = true;
            } else if cmd && !shift && i.key_pressed(egui::Key::Z) {
                undo_hit = true;
            } else if (cmd && shift && i.key_pressed(egui::Key::Z))
                || (cmd && i.key_pressed(egui::Key::Y))
            {
                redo_hit = true;
            } else if cmd && i.key_pressed(egui::Key::D) {
                dup_hit = true;
            } else if !cmd && !shift && !i.modifiers.alt && i.key_pressed(egui::Key::V) {
                marquee_hit = true;
            } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                del_hit = true;
            } else {
                let step = if shift {
                    40.0
                } else if state.snap_to_grid {
                    20.0
                } else {
                    4.0
                };
                if i.key_pressed(egui::Key::ArrowLeft) {
                    nudge.x -= step;
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    nudge.x += step;
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    nudge.y -= step;
                }
                if i.key_pressed(egui::Key::ArrowDown) {
                    nudge.y += step;
                }
            }
        });

        if search_hit {
            state.search_open = !state.search_open;
        } else if marquee_hit {
            state.marquee_mode = !state.marquee_mode;
        } else if undo_hit && state.undo() {
            dirty = true;
        } else if redo_hit && state.redo() {
            dirty = true;
        } else if dup_hit && state.duplicate_selected() {
            dirty = true;
        } else if del_hit {
            let sel_ids = state.selected_node_ids_vec();
            if !sel_ids.is_empty() {
                state.push_undo();
                state.clear_selection();
                for nid in sel_ids {
                    state.doc.remove_node(&nid);
                }
                dirty = true;
            } else if let Some(eid) = state.selected_edge_id.clone() {
                state.push_undo();
                state.selected_edge_id = None;
                state.doc.remove_edge(&eid);
                dirty = true;
            }
        } else if nudge != Vec2::ZERO {
            let sel_ids: HashSet<String> = state.selected_node_ids_vec().into_iter().collect();
            if !sel_ids.is_empty() {
                state.push_undo();
                for n in &mut state.doc.nodes {
                    if sel_ids.contains(&n.id) {
                        n.x += nudge.x;
                        n.y += nudge.y;
                    }
                }
                dirty = true;
            }
        }
    }
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.editing_node_id = None;
        state.connecting_from = None;
        state.context_menu_pos = None;
        state.file_picker_open = false;
        state.search_open = false;
        state.marquee_start_world = None;
        state.marquee_current_world = None;
        state.marquee_mode = false;
    }

    // Separate Group nodes (background) from Content nodes (foreground)
    let mut group_indices: Vec<usize> = state
        .doc
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == CanvasNodeKind::Group)
        .map(|(i, _)| i)
        .collect();
    // Draw larger groups behind smaller ones
    group_indices.sort_by(|&a, &b| {
        let area_a = state.doc.nodes[a].width * state.doc.nodes[a].height;
        let area_b = state.doc.nodes[b].width * state.doc.nodes[b].height;
        area_b
            .partial_cmp(&area_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let content_indices: Vec<usize> = state
        .doc
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind != CanvasNodeKind::Group)
        .map(|(i, _)| i)
        .collect();

    let zoom_hud_block = Rect::from_min_max(
        pos2(canvas_rect.right() - 210.0, canvas_rect.bottom() - 54.0),
        canvas_rect.max,
    );
    let edge_pill_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("c_hud_block_edge")));
    let minimap_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("c_hud_block_minimap")));
    let ctx_menu_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("c_hud_block_ctx")));
    let prev_tb_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("c_hud_block_tb")));
    let search_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("c_hud_block_search")));
    let pointer_blocked_by_hud = |mp: Pos2| -> bool {
        state.file_picker_open
            || state.convert_card_modal.is_some()
            || zoom_hud_block.contains(mp)
            || edge_pill_block.is_some_and(|r| r.contains(mp))
            || minimap_block.is_some_and(|r| r.contains(mp))
            || ctx_menu_block.is_some_and(|r| r.contains(mp))
            || prev_tb_block.is_some_and(|r| r.contains(mp))
            || search_block.is_some_and(|r| r.contains(mp))
    };

    let raw_pointer_pos = ctx.pointer_hover_pos();
    let pointer_pos = raw_pointer_pos.filter(|mp| canvas_rect.contains(*mp) && !pointer_blocked_by_hud(*mp));
    let pointer_world = raw_pointer_pos.map(|mp| state.viewport.screen_to_world(canvas_rect, mp));
    let primary_pressed = pointer_pos.is_some() && ctx.input(|i| i.pointer.primary_pressed());
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_released = ctx.input(|i| i.pointer.primary_released());
    let double_clicked = pointer_pos.is_some() && bg_resp.double_clicked();
    let clicked = pointer_pos.is_some() && bg_resp.clicked();
    let secondary_clicked = pointer_pos.is_some() && ctx.input(|i| i.pointer.secondary_clicked());

    if primary_pressed && state.context_menu_pos.is_some() && pointer_pos.is_some() {
        state.context_menu_pos = None;
    }

    // Update ongoing drag / resize before rendering
    state.smart_guides_x.clear();
    state.smart_guides_y.clear();
    if !primary_down {
        state.dragging_node = None;
        state.resizing_node = None;
        state.drag_undo_pushed = false;
    }
    if let (Some(drag), Some(wp)) = (state.dragging_node.clone(), pointer_world) {
        let raw_delta = wp - drag.start_world;
        if raw_delta.length_sq() > 4.0 && !state.drag_undo_pushed {
            state.push_undo();
            state.drag_undo_pushed = true;
        }
        if state.drag_undo_pushed {
            let mut dx = raw_delta.x;
            let mut dy = raw_delta.y;
            let dragged_ids: HashSet<&str> = drag
                .orig_positions
                .iter()
                .map(|(id, _)| id.as_str())
                .collect();

            // Compute raw bounding box of dragged items for smart alignment guides
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for (nid, orig) in &drag.orig_positions {
                if let Some(n) = state.doc.nodes.iter().find(|n| n.id == *nid) {
                    min_x = min_x.min(orig.x + raw_delta.x);
                    max_x = max_x.max(orig.x + raw_delta.x + n.width);
                    min_y = min_y.min(orig.y + raw_delta.y);
                    max_y = max_y.max(orig.y + raw_delta.y + n.height);
                }
            }

            let snap_eps = (7.0 / zoom.max(0.25)).clamp(4.0, 14.0);
            let mut best_sx: Option<(f32, f32)> = None; // (abs_err, shift)
            let mut best_sy: Option<(f32, f32)> = None;

            if min_x.is_finite() {
                let d_xs = [min_x, (min_x + max_x) * 0.5, max_x];
                let d_ys = [min_y, (min_y + max_y) * 0.5, max_y];
                for other in &state.doc.nodes {
                    if dragged_ids.contains(other.id.as_str()) {
                        continue;
                    }
                    let o_xs = [other.x, other.x + other.width * 0.5, other.x + other.width];
                    let o_ys = [other.y, other.y + other.height * 0.5, other.y + other.height];
                    for &ox in &o_xs {
                        for &dx_cand in &d_xs {
                            let shift = ox - dx_cand;
                            let err = shift.abs();
                            if err <= snap_eps
                                && best_sx.is_none_or(|(be, _)| err < be)
                            {
                                best_sx = Some((err, shift));
                            }
                        }
                    }
                    for &oy in &o_ys {
                        for &dy_cand in &d_ys {
                            let shift = oy - dy_cand;
                            let err = shift.abs();
                            if err <= snap_eps
                                && best_sy.is_none_or(|(be, _)| err < be)
                            {
                                best_sy = Some((err, shift));
                            }
                        }
                    }
                }

                if let Some((_, shift_x)) = best_sx {
                    dx += shift_x;
                    let snapped_xs = [min_x + shift_x, (min_x + max_x) * 0.5 + shift_x, max_x + shift_x];
                    for other in &state.doc.nodes {
                        if dragged_ids.contains(other.id.as_str()) {
                            continue;
                        }
                        for ox in [other.x, other.x + other.width * 0.5, other.x + other.width] {
                            if snapped_xs.iter().any(|sx| (sx - ox).abs() < 0.8) {
                                state.smart_guides_x.push(ox);
                            }
                        }
                    }
                } else if state.snap_to_grid {
                    if let Some((_, orig0)) = drag.orig_positions.first() {
                        dx = snap(orig0.x + raw_delta.x, true) - orig0.x;
                    }
                }

                if let Some((_, shift_y)) = best_sy {
                    dy += shift_y;
                    let snapped_ys = [min_y + shift_y, (min_y + max_y) * 0.5 + shift_y, max_y + shift_y];
                    for other in &state.doc.nodes {
                        if dragged_ids.contains(other.id.as_str()) {
                            continue;
                        }
                        for oy in [other.y, other.y + other.height * 0.5, other.y + other.height] {
                            if snapped_ys.iter().any(|sy| (sy - oy).abs() < 0.8) {
                                state.smart_guides_y.push(oy);
                            }
                        }
                    }
                } else if state.snap_to_grid {
                    if let Some((_, orig0)) = drag.orig_positions.first() {
                        dy = snap(orig0.y + raw_delta.y, true) - orig0.y;
                    }
                }
            }

            for (nid, orig) in &drag.orig_positions {
                if let Some(node) = state.doc.nodes.iter_mut().find(|n| n.id == *nid) {
                    node.x = orig.x + dx;
                    node.y = orig.y + dy;
                }
            }
            dirty = true;
        }
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }
    if let (Some(resz), Some(wp)) = (state.resizing_node.clone(), pointer_world) {
        let delta = wp - resz.start_world;
        if delta.length_sq() > 4.0 && !state.drag_undo_pushed {
            state.push_undo();
            state.drag_undo_pushed = true;
        }
        if state.drag_undo_pushed {
            if let Some(node) = state.doc.nodes.iter_mut().find(|n| n.id == resz.node_id) {
                node.width = snap((resz.orig_size.x + delta.x).max(160.0), state.snap_to_grid);
                node.height = snap((resz.orig_size.y + delta.y).max(80.0), state.snap_to_grid);
                dirty = true;
            }
        }
        ctx.set_cursor_icon(CursorIcon::ResizeNwSe);
    }

    // 2. Render Group nodes
    let active_edit_id = state.editing_node_id.clone();
    for &gi in &group_indices {
        let node = &state.doc.nodes[gi];
        let srect = state
            .viewport
            .world_rect_to_screen(canvas_rect, node_world_rect(node));
        if !canvas_rect.expand(40.0).intersects(srect) {
            continue;
        }
        let accent = canvas_accent_color(p, node.color.as_deref()).unwrap_or(p.border_subtle);
        let selected = state.is_node_selected(&node.id);
        let fill = with_alpha(accent, if p.is_dark { 0.08 } else { 0.07 });
        let border = if selected { p.accent } else { with_alpha(accent, 0.52) };
        painter.rect_filled(srect, Rounding::same((10.0 * zoom).clamp(4.0, 14.0)), fill);
        painter.rect_stroke(
            srect,
            Rounding::same((10.0 * zoom).clamp(4.0, 14.0)),
            Stroke::new(if selected { 1.8_f32 } else { 1.2_f32 }, border),
        );

        let header_h = (28.0 * zoom).clamp(18.0, 32.0);
        let grp_foc_gate = egui::Id::new(("canvas_grp_foc", &state.doc.nodes[gi].id));

        if active_edit_id.as_deref() == Some(&state.doc.nodes[gi].id) {
            let edit_r = Rect::from_min_size(
                pos2(srect.left() + 8.0, srect.top() + 3.0),
                vec2((srect.width() - 16.0).max(80.0), (header_h - 4.0).max(22.0)),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_r));
            child.set_clip_rect(canvas_rect.intersect(srect));
            let label_buf = state.doc.nodes[gi].label.get_or_insert_with(String::new);
            let resp = child.add(
                egui::TextEdit::singleline(label_buf)
                    .frame(false)
                    .font(font_semibold(12.5))
                    .text_color(p.text_primary)
                    .desired_width(edit_r.width()),
            );
            focus_once(child.ctx(), grp_foc_gate, true, resp.id);
            if resp.changed() {
                dirty = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                state.editing_node_id = None;
            }
        } else {
            reset_focus_gate(ui.ctx(), grp_foc_gate);
            let lbl = node.label.as_deref().unwrap_or("Group");
            let pill_fill = lerp_color(p.bg_elevated, accent, if p.is_dark { 0.22 } else { 0.16 });
            let g = painter.layout_no_wrap(
                lbl.to_string(),
                font_semibold((12.0 * zoom).clamp(10.0, 15.0)),
                p.text_primary,
            );
            let pill_r = Rect::from_min_size(
                pos2(srect.left() + 8.0 * zoom, srect.top() - 11.0 * zoom),
                vec2(g.size().x + 14.0, g.size().y + 6.0),
            );
            painter.rect_filled(pill_r, Rounding::same(6.0), pill_fill);
            painter.rect_stroke(pill_r, Rounding::same(6.0), Stroke::new(1.0_f32, border));
            painter.galley(
                pos2(pill_r.left() + 7.0, pill_r.center().y - g.size().y * 0.5),
                g,
                p.text_primary,
            );

            let resize_r = Rect::from_min_size(
                pos2(srect.right() - 16.0, srect.bottom() - 16.0),
                vec2(16.0, 16.0),
            );
            if selected {
                painter.circle_filled(resize_r.center(), 3.5, p.accent);
            }
        }
    }

    // 3. Render Edges + hit test + live connection preview
    struct EdgeDraw {
        id: String,
        pts: Vec<Pos2>,
        draw_from_arrow: bool,
        draw_arrow: bool,
        label: Option<String>,
        color: Option<String>,
        label_rect: Option<Rect>,
    }
    let mut edge_draws = Vec::new();
    for e in &state.doc.edges {
        let Some(from_n) = state.doc.nodes.iter().find(|n| n.id == e.from_node) else {
            continue;
        };
        let Some(to_n) = state.doc.nodes.iter().find(|n| n.id == e.to_node) else {
            continue;
        };
        let (auto_from, auto_to) = choose_best_sides(from_n, to_n);
        let fs = e.from_side.unwrap_or(auto_from);
        let ts = e.to_side.unwrap_or(auto_to);
        let w0 = port_world_pos(from_n, fs);
        let w3 = port_world_pos(to_n, ts);
        let d = (w0.distance(w3) * 0.42).clamp(36.0, 180.0);
        let wc1 = w0 + side_tangent(fs) * d;
        let wc2 = w3 + side_tangent(ts) * d;

        let sp0 = state.viewport.world_to_screen(canvas_rect, w0);
        let sp1 = state.viewport.world_to_screen(canvas_rect, wc1);
        let sp2 = state.viewport.world_to_screen(canvas_rect, wc2);
        let sp3 = state.viewport.world_to_screen(canvas_rect, w3);
        let pts = sample_cubic_bezier(sp0, sp1, sp2, sp3, 24);
        let mut label_rect = None;
        if zoom >= 0.48 {
            if let Some(lbl) = e.label.as_deref().filter(|l| !l.trim().is_empty()) {
                let mid = pts[pts.len() / 2];
                let g = painter.layout_no_wrap(
                    lbl.trim().to_string(),
                    font_medium((11.0 * zoom).clamp(9.5, 13.0)),
                    p.text_secondary,
                );
                label_rect = Some(Rect::from_center_size(
                    mid,
                    vec2(g.size().x + 12.0, g.size().y + 6.0),
                ));
            }
        }
        edge_draws.push(EdgeDraw {
            id: e.id.clone(),
            pts,
            draw_from_arrow: e.from_end.unwrap_or(CanvasEnd::None) == CanvasEnd::Arrow,
            draw_arrow: e.to_end.unwrap_or(CanvasEnd::Arrow) == CanvasEnd::Arrow,
            label: e.label.clone(),
            color: e.color.clone(),
            label_rect,
        });
    }

    let mut hovered_edge_id: Option<String> = None;
    if let Some(mp) = pointer_pos {
        if canvas_rect.contains(mp) {
            let mut best = 10.5_f32;
            for ed in &edge_draws {
                if ed.label_rect.is_some_and(|lr| lr.expand(3.0).contains(mp)) {
                    hovered_edge_id = Some(ed.id.clone());
                    break;
                }
                let d = dist_to_polyline(&ed.pts, mp);
                if d < best {
                    best = d;
                    hovered_edge_id = Some(ed.id.clone());
                }
            }
        }
    }

    for ed in &edge_draws {
        let is_sel = state.selected_edge_id.as_deref() == Some(&ed.id);
        let is_hov = hovered_edge_id.as_deref() == Some(&ed.id);
        let custom_col = canvas_accent_color(p, ed.color.as_deref());
        let stroke_col = if is_sel {
            p.accent
        } else if is_hov {
            p.accent_hover
        } else {
            custom_col.unwrap_or(p.text_muted)
        };
        let sw = if is_sel || is_hov { 2.4_f32 } else { 1.6_f32 };
        painter.add(egui::Shape::line(ed.pts.clone(), Stroke::new(sw, stroke_col)));
        let arrow_sz = (9.5 * zoom).clamp(6.5, 13.0);
        if ed.draw_arrow && ed.pts.len() >= 2 {
            let p_prev = ed.pts[ed.pts.len() - 2];
            let p_tip = ed.pts[ed.pts.len() - 1];
            draw_arrow_head(&painter, p_tip, p_tip - p_prev, stroke_col, arrow_sz);
        }
        if ed.draw_from_arrow && ed.pts.len() >= 2 {
            let p_tip = ed.pts[0];
            let p_next = ed.pts[1];
            draw_arrow_head(&painter, p_tip, p_tip - p_next, stroke_col, arrow_sz);
        }
        if zoom >= 0.48 {
            if let (Some(lbl), Some(lr)) = (
                ed.label.as_deref().filter(|l| !l.trim().is_empty()),
                ed.label_rect,
            ) {
                let g = painter.layout_no_wrap(
                    lbl.trim().to_string(),
                    font_medium((11.0 * zoom).clamp(9.5, 13.0)),
                    p.text_secondary,
                );
                painter.rect_filled(lr, Rounding::same(6.0), p.bg_elevated);
                painter.rect_stroke(lr, Rounding::same(6.0), Stroke::new(1.0_f32, stroke_col));
                painter.galley(
                    pos2(lr.center().x - g.size().x * 0.5, lr.center().y - g.size().y * 0.5),
                    g,
                    p.text_secondary,
                );
            }
        }
    }

    if let (Some((from_id, from_side)), Some(mp)) = (&state.connecting_from, pointer_pos) {
        if let Some(from_n) = state.doc.nodes.iter().find(|n| n.id == *from_id) {
            let w0 = port_world_pos(from_n, *from_side);
            let sp0 = state.viewport.world_to_screen(canvas_rect, w0);
            let d = (sp0.distance(mp) * 0.42).clamp(28.0, 140.0);
            let sp1 = sp0 + side_tangent(*from_side) * d;
            let sp2 = mp;
            let pts = sample_cubic_bezier(sp0, sp1, sp2, mp, 20);
            painter.add(egui::Shape::line(pts.clone(), Stroke::new(2.0_f32, p.accent)));
            if pts.len() >= 2 {
                draw_arrow_head(&painter, mp, mp - pts[pts.len() - 2], p.accent, 9.0);
            }
        }
    }

    // 4. Content nodes (TextNode, FileNode, LinkNode)
    let mut open_external_target: Option<String> = None;
    let mut complete_connection_to: Option<(String, CanvasSide)> = None;
    let mut port_drag_started: Option<(String, CanvasSide)> = None;
    let mut pointer_over_selected_card = false;
    let mut external_label_rects: Vec<(Rect, String)> = Vec::new();

    for &ci in &content_indices {
        let node = state.doc.nodes[ci].clone();
        let wrect = node_world_rect(&node);
        let srect = state.viewport.world_rect_to_screen(canvas_rect, wrect);
        if !canvas_rect.expand(40.0).intersects(srect) {
            continue;
        }

        let accent = canvas_accent_color(p, node.color.as_deref());
        let selected = state.is_node_selected(&node.id);
        let is_hovered = pointer_pos.is_some_and(|mp| srect.expand(8.0).contains(mp));
        let is_editing = state.editing_node_id.as_deref() == Some(&node.id);

        let card_fill = match accent {
            Some(c) => lerp_color(p.bg_elevated, c, if p.is_dark { 0.14 } else { 0.10 }),
            None => p.bg_elevated,
        };
        let border_col = if selected {
            p.accent
        } else {
            accent.unwrap_or(p.border_subtle)
        };
        let rad = (9.0 * zoom).clamp(4.0, 13.0);
        painter.rect_filled(srect, Rounding::same(rad), card_fill);
        painter.rect_stroke(
            srect,
            Rounding::same(rad),
            Stroke::new(if selected { 2.0_f32 } else { 1.1_f32 }, border_col),
        );

        let pad = (11.0 * zoom).clamp(6.0, 15.0);
        let inner = srect.shrink(pad);

        let card_painter = painter.with_clip_rect(canvas_rect.intersect(srect.shrink(2.5)));
        let card_foc_gate = egui::Id::new(("canvas_card_foc", &node.id));

        if node.kind == CanvasNodeKind::File && zoom >= 0.40 {
            if let Some(fp) = node.file.as_deref() {
                if VaultFileKind::from_path(fp) == Some(VaultFileKind::Markdown) {
                    let lbl_text = notes
                        .iter()
                        .find(|n| n.path.eq_ignore_ascii_case(fp))
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| vault_file_stem(fp).to_string());
                    let lbl_font = font_medium((11.0 * zoom).clamp(9.5, 13.0));
                    let lbl_g = painter.layout_no_wrap(lbl_text, lbl_font, p.text_secondary);
                    let max_lbl_w = (srect.width() - 8.0).max(60.0);
                    let pill_w = (lbl_g.size().x + 34.0).min(max_lbl_w);
                    let pill_h = (lbl_g.size().y + 6.0).max(18.0);
                    let pill_r = Rect::from_min_size(
                        pos2(srect.left() + 2.0, srect.top() - pill_h - 4.0),
                        vec2(pill_w, pill_h),
                    );
                    if canvas_rect.intersects(pill_r) {
                        let lbl_hov = pointer_pos.is_some_and(|mp| pill_r.contains(mp));
                        painter.rect_filled(
                            pill_r,
                            Rounding::same(5.0),
                            if lbl_hov { p.bg_hover } else { p.bg_elevated },
                        );
                        painter.rect_stroke(
                            pill_r,
                            Rounding::same(5.0),
                            Stroke::new(1.0_f32, if selected || lbl_hov { p.accent } else { p.border_hairline }),
                        );
                        let ic_r = Rect::from_center_size(
                            pos2(pill_r.left() + 10.0, pill_r.center().y),
                            vec2(11.0, 11.0),
                        );
                        draw_icon(&painter, ic_r, Icon::Document, p.accent_text);
                        let text_clip = painter.with_clip_rect(
                            canvas_rect.intersect(Rect::from_min_max(
                                pos2(pill_r.left() + 18.0, pill_r.top()),
                                pos2(pill_r.right() - 16.0, pill_r.bottom()),
                            )),
                        );
                        text_clip.galley(
                            pos2(pill_r.left() + 18.0, pill_r.center().y - lbl_g.size().y * 0.5),
                            lbl_g,
                            if lbl_hov { p.text_primary } else { p.text_secondary },
                        );
                        let ext_r = Rect::from_center_size(
                            pos2(pill_r.right() - 9.0, pill_r.center().y),
                            vec2(10.0, 10.0),
                        );
                        draw_icon(&painter, ext_r, Icon::OutLink, p.accent_text);
                        external_label_rects.push((pill_r, fp.to_string()));
                    }
                }
            }
        }

        if zoom < 0.40 || srect.height() < 48.0 || srect.width() < 84.0 {
            reset_focus_gate(ui.ctx(), card_foc_gate);
            let mini_title = node.display_title();
            let mut job = egui::text::LayoutJob::single_section(
                mini_title,
                egui::TextFormat {
                    font_id: font_semibold((9.5 * (zoom / 0.35).clamp(0.78, 1.0)).clamp(7.5, 10.5)),
                    color: p.text_primary,
                    ..Default::default()
                },
            );
            job.wrap = egui::text::TextWrapping {
                max_width: (srect.width() - 8.0).max(16.0),
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let g = card_painter.layout_job(job);
            card_painter.galley(
                pos2(srect.left() + 4.0, (srect.center().y - g.size().y * 0.5).max(srect.top() + 3.0)),
                g,
                p.text_primary,
            );
        } else if is_editing && node.kind == CanvasNodeKind::Text {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(Layout::top_down(Align::LEFT)),
            );
            child.set_clip_rect(canvas_rect.intersect(srect));
            let text_buf = state.doc.nodes[ci].text.get_or_insert_with(String::new);
            let r = child.add(
                egui::TextEdit::multiline(text_buf)
                    .frame(false)
                    .font(font_body((12.5 * zoom).clamp(11.0, 16.0)))
                    .text_color(p.text_primary)
                    .desired_width(inner.width().max(40.0)),
            );
            focus_once(child.ctx(), card_foc_gate, true, r.id);
            if r.changed() {
                dirty = true;
            }
        } else {
            reset_focus_gate(ui.ctx(), card_foc_gate);
            match node.kind {
                CanvasNodeKind::Text => {
                    let body = node.text.as_deref().unwrap_or("");
                    render_text_card_body(&card_painter, inner, zoom, p, body);
                }
                CanvasNodeKind::File => {
                    let file_path = node.file.as_deref().unwrap_or("");
                    let kind = VaultFileKind::from_path(file_path).unwrap_or(VaultFileKind::Markdown);
                    if kind == VaultFileKind::Asset {
                        let img_area = srect.shrink((6.0 * zoom).clamp(4.0, 10.0));
                        if let Some((tex_id, img_sz)) = image_cache.get_or_load(ctx, vault_dir, file_path) {
                            let scale = (img_area.width() / img_sz.x.max(1.0))
                                .min(img_area.height() / img_sz.y.max(1.0));
                            let disp = img_sz * scale;
                            let disp_r = Rect::from_center_size(img_area.center(), disp);
                            card_painter.image(
                                tex_id,
                                disp_r,
                                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                                eframe::egui::Color32::WHITE,
                            );
                            if (is_hovered || selected) && zoom >= 0.48 {
                                let name = vault_file_stem(file_path).to_string();
                                let ng = card_painter.layout_no_wrap(
                                    name,
                                    font_mono((10.0 * zoom).clamp(8.5, 11.5)),
                                    p.text_primary,
                                );
                                let cap_r = Rect::from_min_size(
                                    pos2(inner.left(), inner.bottom() - ng.size().y - 6.0),
                                    vec2((ng.size().x + 10.0).min(inner.width()), ng.size().y + 4.0),
                                );
                                card_painter.rect_filled(
                                    cap_r,
                                    Rounding::same(4.0),
                                    with_alpha(p.bg_elevated, 0.85),
                                );
                                card_painter.galley(pos2(cap_r.left() + 5.0, cap_r.top() + 2.0), ng, p.text_primary);
                            }
                        } else if kind == VaultFileKind::Markdown {
                        let card_scroll_active = selected
                            && state.dragging_node.is_none()
                            && state.resizing_node.is_none()
                            && !state.file_picker_open
                            && state.convert_card_modal.is_none();
                        if card_scroll_active && pointer_pos.is_some_and(|mp| srect.contains(mp)) {
                            pointer_over_selected_card = true;
                        }
                        if let Some(note_data) = resolve_embedded_note(file_path, md_ctx.embedded_notes) {
                            let content_rect = srect.shrink((8.0 * zoom).clamp(5.0, 12.0));
                            let strict_clip = canvas_rect.intersect(srect.shrink(2.0));
                            if content_rect.width() >= 24.0 && content_rect.height() >= 24.0 && strict_clip.is_positive() {
                                let mut child = ui.new_child(
                                    egui::UiBuilder::new()
                                        .max_rect(content_rect)
                                        .layout(Layout::top_down(Align::LEFT)),
                                );
                                child.set_clip_rect(strict_clip);
                                let scroll_h = content_rect.height().max(24.0);
                                let scroll_w = content_rect.width().max(32.0);
                                egui::ScrollArea::vertical()
                                    .id_salt(("canvas_md_scroll", &node.id))
                                    .enable_scrolling(card_scroll_active)
                                    .max_height(scroll_h)
                                    .show(&mut child, |ui| {
                                        ui.set_width(scroll_w);
                                        ui.set_max_width(scroll_w);
                                        if let Some(pv_act) = render_markdown_preview(
                                            ui,
                                            p,
                                            lang,
                                            md_ctx.today,
                                            &note_data.body,
                                            (13.0 * zoom).clamp(9.5, 18.0),
                                            md_ctx.heading_serif,
                                            md_ctx.resolved_titles,
                                            md_ctx.copied_code_hash,
                                            vault_dir,
                                            image_cache,
                                            md_ctx.embedded_boards,
                                            md_ctx.available_boards,
                                            md_ctx.embedded_notes,
                                            Some(&note_data.path),
                                        ) {
                                            if selected {
                                                match pv_act {
                                                    PreviewAction::NavigateWikilink(target) => {
                                                        actions.push(UiAction::OpenOrCreateWikilink(target));
                                                    }
                                                    PreviewAction::CopyCode(code) => {
                                                        actions.push(UiAction::CopyText(code));
                                                    }
                                                    PreviewAction::OpenImage(path) => {
                                                        actions.push(UiAction::OpenImageLightbox(path));
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    });
                            }
                        } else {
                            let resolved_title = notes
                                .iter()
                                .find(|n| n.path.eq_ignore_ascii_case(file_path))
                                .map(|n| n.title.clone())
                                .unwrap_or_else(|| vault_file_stem(file_path).to_string());
                            let ic_r = Rect::from_center_size(
                                pos2(inner.left() + 8.0 * zoom, inner.top() + 9.0 * zoom),
                                vec2(14.0 * zoom, 14.0 * zoom),
                            );
                            draw_icon(&card_painter, ic_r, Icon::Document, accent.unwrap_or(p.accent_text));
                            let title_g = card_painter.layout(
                                resolved_title,
                                font_semibold((13.0 * zoom).clamp(10.0, 16.0)),
                                p.text_primary,
                                (inner.width() - 40.0 * zoom).max(40.0),
                            );
                            card_painter.galley(
                                pos2(inner.left() + 20.0 * zoom, inner.top()),
                                title_g,
                                p.text_primary,
                            );
                        }
                    } else {
                            let ic_r = Rect::from_center_size(img_area.center(), vec2(26.0 * zoom, 26.0 * zoom));
                            draw_icon(&card_painter, ic_r, Icon::Image, p.text_muted);
                        }
                        if (is_hovered || selected) && zoom >= 0.48 {
                            let open_r = Rect::from_center_size(
                                pos2(inner.right() - 8.0, inner.top() + 8.0),
                                vec2(18.0, 18.0),
                            );
                            card_painter.rect_filled(
                                open_r,
                                Rounding::same(4.0),
                                with_alpha(p.bg_elevated, 0.85),
                            );
                            draw_icon(&card_painter, open_r.shrink(3.0), Icon::Maximize, p.accent_text);
                        }
                    } else {
                        let icon = match kind {
                            VaultFileKind::Board => Icon::Board,
                            VaultFileKind::Table => Icon::Table,
                            VaultFileKind::Canvas => Icon::Canvas,
                            VaultFileKind::Asset => Icon::Image,
                            VaultFileKind::Markdown => Icon::Document,
                        };
                        let resolved_title = notes
                            .iter()
                            .find(|n| n.path.eq_ignore_ascii_case(file_path))
                            .map(|n| n.title.clone())
                            .or_else(|| {
                                collections
                                    .iter()
                                    .find(|c| c.path.eq_ignore_ascii_case(file_path))
                                    .map(|c| c.title.clone())
                            })
                            .unwrap_or_else(|| vault_file_stem(file_path).to_string());
                        let resolved_excerpt = notes
                            .iter()
                            .find(|n| n.path.eq_ignore_ascii_case(file_path))
                            .map(|n| n.excerpt.clone())
                            .or_else(|| {
                                collections
                                    .iter()
                                    .find(|c| c.path.eq_ignore_ascii_case(file_path))
                                    .map(|c| c.excerpt.clone())
                            })
                            .unwrap_or_default();

                        let ic_r = Rect::from_center_size(
                            pos2(inner.left() + 8.0 * zoom, inner.top() + 9.0 * zoom),
                            vec2(14.0 * zoom, 14.0 * zoom),
                        );
                        draw_icon(&card_painter, ic_r, icon, accent.unwrap_or(p.accent_text));

                        let title_g = card_painter.layout(
                            resolved_title,
                            font_semibold((13.0 * zoom).clamp(10.0, 16.0)),
                            p.text_primary,
                            (inner.width() - 40.0 * zoom).max(40.0),
                        );
                        let title_h = title_g.size().y;
                        card_painter.galley(
                            pos2(inner.left() + 20.0 * zoom, inner.top()),
                            title_g,
                            p.text_primary,
                        );

                        let path_y = inner.top() + title_h + 3.0 * zoom;
                        let path_g = card_painter.layout_no_wrap(
                            file_path.to_string(),
                            font_mono((10.0 * zoom).clamp(8.5, 12.0)),
                            p.text_muted,
                        );
                        card_painter.galley(pos2(inner.left(), path_y), path_g, p.text_muted);

                        if !resolved_excerpt.is_empty() && path_y + 22.0 < inner.bottom() {
                            let ex_g = card_painter.layout(
                                resolved_excerpt,
                                font_body((11.5 * zoom).clamp(9.0, 14.0)),
                                p.text_secondary,
                                inner.width().max(40.0),
                            );
                            card_painter.galley(
                                pos2(inner.left(), path_y + 17.0 * zoom),
                                ex_g,
                                p.text_secondary,
                            );
                        }

                        // External jump icon at top-right (hit-tested below)
                        let open_r = Rect::from_center_size(
                            pos2(inner.right() - 8.0, inner.top() + 8.0),
                            vec2(18.0, 18.0),
                        );
                        draw_icon(&card_painter, open_r.shrink(3.0), Icon::OutLink, p.accent_text);
                    }
                }
                CanvasNodeKind::Link | CanvasNodeKind::Group => {
                    let lbl = node.display_title();
                    let g = card_painter.layout(
                        lbl,
                        font_medium((12.5 * zoom).clamp(10.0, 15.0)),
                        p.accent_text,
                        inner.width().max(40.0),
                    );
                    card_painter.galley(inner.min, g, p.accent_text);
                }
            }
        }

        // Connection Ports (show on hover/selected or while actively dragging a connection)
        let show_ports = is_hovered || selected || state.connecting_from.is_some();
        let sides = [
            CanvasSide::Top,
            CanvasSide::Right,
            CanvasSide::Bottom,
            CanvasSide::Left,
        ];
        if show_ports {
            for side in sides {
                let wp = port_world_pos(&node, side);
                let sp = state.viewport.world_to_screen(canvas_rect, wp);
                let pr = (5.0 * zoom.sqrt()).clamp(4.0, 6.5);
                let port_hit = Rect::from_center_size(sp, vec2(18.0, 18.0));
                let port_hov = pointer_pos.is_some_and(|mp| port_hit.contains(mp));
                painter.circle_filled(sp, pr, if port_hov { p.accent } else { p.bg_elevated });
                painter.circle_stroke(sp, pr, Stroke::new(1.6_f32, p.accent));

                if port_hov && primary_pressed && state.connecting_from.is_none() && port_drag_started.is_none() {
                    port_drag_started = Some((node.id.clone(), side));
                }
                if state.connecting_from.is_some()
                    && primary_released
                    && port_hov
                    && state.connecting_from.as_ref().is_some_and(|(fid, _)| fid != &node.id)
                {
                    complete_connection_to = Some((node.id.clone(), side));
                }
            }
        }

        // Drop connection anywhere on card body
        if let Some((ref from_id, _)) = state.connecting_from {
            if primary_released
                && from_id != &node.id
                && pointer_pos.is_some_and(|mp| srect.contains(mp))
                && complete_connection_to.is_none()
            {
                let side = if let Some(from_n) = state.doc.nodes.iter().find(|n| n.id == *from_id) {
                    choose_best_sides(from_n, &node).1
                } else {
                    CanvasSide::Left
                };
                complete_connection_to = Some((node.id.clone(), side));
            }
        }

        // Bottom-right resize corner
        let resize_r = Rect::from_min_size(
            pos2(srect.right() - 16.0, srect.bottom() - 16.0),
            vec2(16.0, 16.0),
        );
        if selected || is_hovered {
            painter.circle_filled(
                resize_r.center(),
                3.2,
                if selected { p.accent } else { p.text_muted },
            );
        }
    }

    if let Some((to_id, to_side)) = complete_connection_to {
        if let Some((from_id, from_side)) = state.connecting_from.take() {
            state.push_undo();
            if let Some(eid) = state
                .doc
                .add_edge(&from_id, Some(from_side), &to_id, Some(to_side), None)
            {
                state.selected_edge_id = Some(eid);
                state.selected_node_id = None;
                state.edge_label_draft.clear();
                dirty = true;
            }
        }
    } else if primary_released {
        if let Some((from_id, from_side)) = state.connecting_from.take() {
            if let (Some(_mp), Some(wp)) = (pointer_pos, pointer_world) {
                let far_enough = state
                    .doc
                    .nodes
                    .iter()
                    .find(|n| n.id == from_id)
                    .is_some_and(|fn_node| wp.distance(port_world_pos(fn_node, from_side)) >= 40.0);
                if far_enough {
                    state.push_undo();
                    let new_id = state.doc.add_text_node(
                        snap(wp.x - 120.0, state.snap_to_grid),
                        snap(wp.y - 60.0, state.snap_to_grid),
                        if lang == Lang::Ru {
                            "Новая карточка"
                    } else {
                            "New card"
                        },
                        Some("5"),
                    );
                    let to_side = match from_side {
                        CanvasSide::Right => CanvasSide::Left,
                        CanvasSide::Left => CanvasSide::Right,
                        CanvasSide::Bottom => CanvasSide::Top,
                        CanvasSide::Top => CanvasSide::Bottom,
                    };
                    let _ = state
                        .doc
                        .add_edge(&from_id, Some(from_side), &new_id, Some(to_side), None);
                    state.selected_node_id = Some(new_id.clone());
                    state.selected_edge_id = None;
                    state.editing_node_id = Some(new_id);
                    dirty = true;
                }
            }
        }
    }

    // Strict top-down Z-order hit-testing on press, secondary-click, and double-click
    // Layer order: 1. Content Nodes -> 2. Edges -> 3. Group Nodes
    let mut hit_handled = false;
    if let Some((fid, fside)) = port_drag_started {
        state.connecting_from = Some((fid, fside));
        state.pressed_on_empty_bg = false;
        hit_handled = true;
    }

    if !hit_handled && primary_pressed {
        if let Some(mp) = pointer_pos {
            let mut hit_any = false;
            // 0. External note filename labels above .md File cards
            for (lbl_r, fp) in external_label_rects.iter().rev() {
                if lbl_r.contains(mp) {
                    open_external_target = Some(fp.clone());
                    hit_any = true;
                    break;
                }
            }

            // 1. Content nodes (topmost first)
            if !hit_any {
            for &ci in content_indices.iter().rev() {
                let node = state.doc.nodes[ci].clone();
                if state.editing_node_id.as_deref() == Some(&node.id) {
                    let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(&node));
                    if srect.contains(mp) {
                        hit_any = true;
                        break;
                    }
                    continue;
                }
                let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(&node));
                let resize_r = Rect::from_min_size(pos2(srect.right() - 16.0, srect.bottom() - 16.0), vec2(16.0, 16.0));
                let pad = (11.0 * zoom).clamp(6.0, 15.0);
                let inner = srect.shrink(pad);
                let open_r = Rect::from_center_size(pos2(inner.right() - 8.0, inner.top() + 8.0), vec2(18.0, 18.0));

                if node.kind == CanvasNodeKind::File && zoom >= 0.48 && open_r.contains(mp) {
                    if let Some(fp) = &node.file {
                        open_external_target = Some(fp.clone());
                    }
                    hit_any = true;
                    break;
                } else if resize_r.contains(mp) {
                    state.select_single_node(node.id.clone());
                    state.editing_node_id = None;
                    if let Some(wp) = pointer_world {
                        state.resizing_node = Some(ResizingNode {
                            node_id: node.id,
                            start_world: wp,
                            orig_size: vec2(node.width, node.height),
                        });
                    }
                    hit_any = true;
                    break;
                } else if srect.contains(mp) {
                    let ctrl_held = ctx.input(|i| i.modifiers.command);
                    if ctrl_held {
                        state.toggle_node_selection(node.id.clone());
                        state.editing_node_id = None;
                    } else if state.is_node_selected(&node.id)
                        && state.selected_node_ids_vec().len() > 1
                    {
                        state.editing_node_id = None;
                        if let Some(wp) = pointer_world {
                            let orig_positions = state
                                .selected_node_ids_vec()
                                .into_iter()
                                .filter_map(|sid| {
                                    state
                                        .doc
                                        .nodes
                                        .iter()
                                        .find(|n| n.id == sid)
                                        .map(|n| (sid, pos2(n.x, n.y)))
                                })
                                .collect();
                            state.dragging_node = Some(DraggingNode {
                                start_world: wp,
                                orig_positions,
                            });
                        }
                    } else {
                        state.select_single_node(node.id.clone());
                        state.editing_node_id = None;
                        if let Some(wp) = pointer_world {
                            state.dragging_node = Some(DraggingNode {
                                start_world: wp,
                                orig_positions: vec![(node.id, pos2(node.x, node.y))],
                            });
                        }
                    }
                    hit_any = true;
                    break;
                }
            }
            }

            // 2. Edges (sit ABOVE Groups so edges inside a group are always selectable)
            if !hit_any {
                if let Some(eid) = hovered_edge_id.clone() {
                    state.clear_selection();
                    state.selected_edge_id = Some(eid.clone());
                    state.editing_node_id = None;
                    state.edge_label_draft = state
                        .doc
                        .edges
                        .iter()
                        .find(|e| e.id == eid)
                        .and_then(|e| e.label.clone())
                        .unwrap_or_default();
                    hit_any = true;
                }
            }

            // 3. Group nodes (smallest/topmost first)
            if !hit_any {
                for &gi in group_indices.iter().rev() {
                    let gnode = &state.doc.nodes[gi];
                    let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(gnode));
                    let header_h = (28.0 * zoom).clamp(18.0, 32.0);
                    let header_rect = Rect::from_min_size(srect.min, vec2(srect.width(), header_h));
                    let pill_hit = Rect::from_min_size(
                        pos2(srect.left(), srect.top() - 14.0 * zoom),
                        vec2((srect.width() * 0.7).clamp(72.0, 220.0), header_h + 14.0 * zoom),
                    );
                    let resize_r = Rect::from_min_size(pos2(srect.right() - 16.0, srect.bottom() - 16.0), vec2(16.0, 16.0));
                    let on_border = srect.expand(4.0).contains(mp) && !srect.shrink(10.0).contains(mp);

                    if state.is_node_selected(&gnode.id) && resize_r.contains(mp) {
                        let nid = gnode.id.clone();
                        state.resizing_node = Some(ResizingNode {
                            node_id: nid,
                            start_world: pointer_world.unwrap_or(mp),
                            orig_size: vec2(gnode.width, gnode.height),
                        });
                        hit_any = true;
                        break;
                    } else if header_rect.union(pill_hit).contains(mp) || on_border {
                        let gid = gnode.id.clone();
                        let grect = node_world_rect(gnode);
                        let gpos = pos2(gnode.x, gnode.y);
                        state.select_single_node(gid.clone());
                        state.editing_node_id = None;
                        if let Some(wp) = pointer_world {
                            let mut orig_positions = vec![(gid, gpos)];
                            for child in &state.doc.nodes {
                                if child.kind != CanvasNodeKind::Group {
                                    let cc = pos2(child.x + child.width * 0.5, child.y + child.height * 0.5);
                                    if grect.contains(cc) {
                                        orig_positions.push((child.id.clone(), pos2(child.x, child.y)));
                                    }
                                }
                            }
                            state.dragging_node = Some(DraggingNode { start_world: wp, orig_positions });
                        }
                        hit_any = true;
                        break;
                    } else if srect.contains(mp) {
                        let shift_held = ctx.input(|i| i.modifiers.shift);
                        if state.marquee_mode || shift_held {
                            break;
                        }
                        let gid = gnode.id.clone();
                        state.select_single_node(gid);
                        state.editing_node_id = None;
                        hit_any = true;
                        break;
                    }
                }
            }

            if !hit_any && (state.marquee_mode || ctx.input(|i| i.modifiers.shift)) {
                if let Some(wp) = pointer_world {
                    state.marquee_start_world = Some(wp);
                    state.marquee_current_world = Some(wp);
                }
                state.pressed_on_empty_bg = false;
                hit_handled = true;
            } else {
                state.pressed_on_empty_bg = !hit_any;
                hit_handled = hit_any;
            }
        }
    }

    // Handle Double-click INDEPENDENTLY of primary_pressed so release fires cleanly!
    if double_clicked {
        if let Some(mp) = pointer_pos {
            let mut dc_handled = false;
            let mut hit_content: Option<(String, CanvasNodeKind, Option<String>)> = None;
            for &ci in content_indices.iter().rev() {
                let node = &state.doc.nodes[ci];
                let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(node));
                if srect.contains(mp) {
                    hit_content = Some((node.id.clone(), node.kind, node.file.clone()));
                    break;
                }
            }
            if let Some((node_id, kind, file)) = hit_content {
                state.selected_node_id = Some(node_id.clone());
                state.selected_edge_id = None;
                if kind == CanvasNodeKind::Text {
                    state.push_undo();
                    state.editing_node_id = Some(node_id);
                } else if kind == CanvasNodeKind::File {
                    if let Some(fp) = file {
                        open_external_target = Some(fp);
                    }
                }
                dc_handled = true;
            }
            if !dc_handled {
                if let Some(eid) = hovered_edge_id.clone() {
                    state.selected_edge_id = Some(eid.clone());
                    state.selected_node_id = None;
                    state.editing_node_id = None;
                    state.edge_label_draft = state
                        .doc
                        .edges
                        .iter()
                        .find(|e| e.id == eid)
                        .and_then(|e| e.label.clone())
                        .unwrap_or_default();
                    state.focus_edge_label_next = true;
                    dc_handled = true;
                }
            }
            if !dc_handled {
                let mut hit_group: Option<String> = None;
                for &gi in group_indices.iter().rev() {
                    let gnode = &state.doc.nodes[gi];
                    let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(gnode));
                    let header_h = (28.0 * zoom).clamp(18.0, 32.0);
                    let pill_hit = Rect::from_min_size(
                        pos2(srect.left(), srect.top() - 14.0 * zoom),
                        vec2((srect.width() * 0.7).clamp(72.0, 220.0), header_h + 14.0 * zoom),
                    );
                    if srect.contains(mp) || pill_hit.contains(mp) {
                        hit_group = Some(gnode.id.clone());
                        break;
                    }
                }
                if let Some(gid) = hit_group {
                    state.push_undo();
                    state.selected_node_id = Some(gid.clone());
                    state.editing_node_id = Some(gid);
                    dc_handled = true;
                }
            }
            if !dc_handled {
                if let Some(wp) = pointer_world {
                    state.push_undo();
                    let id = state.doc.add_text_node(
                        snap(wp.x - 115.0, state.snap_to_grid),
                        snap(wp.y - 58.0, state.snap_to_grid),
                        if lang == Lang::Ru { "Новая карточка" } else { "New card" },
                        Some("5"),
                    );
                    state.selected_node_id = Some(id.clone());
                    state.selected_edge_id = None;
                    state.editing_node_id = Some(id);
                    dirty = true;
                }
            }
        }
    }

    // Handle Right-click Context Menu
    if secondary_clicked {
        if let (Some(mp), Some(wp)) = (pointer_pos, pointer_world) {
            let mut hit_node: Option<String> = None;
            for &ci in content_indices.iter().rev() {
                let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(&state.doc.nodes[ci]));
                if srect.contains(mp) {
                    hit_node = Some(state.doc.nodes[ci].id.clone());
                    break;
                }
            }
            state.context_menu_pos = Some(mp);
            state.context_menu_world = wp;
            if let Some(nid) = hit_node {
                state.selected_node_id = Some(nid.clone());
                state.selected_edge_id = None;
                state.context_menu_node = Some(nid);
                state.context_menu_edge = None;
            } else if let Some(eid) = hovered_edge_id.clone() {
                state.selected_edge_id = Some(eid.clone());
                state.selected_node_id = None;
                state.context_menu_node = None;
                state.context_menu_edge = Some(eid);
            } else {
                let mut hit_group: Option<String> = None;
                for &gi in group_indices.iter().rev() {
                    let srect = state.viewport.world_rect_to_screen(canvas_rect, node_world_rect(&state.doc.nodes[gi]));
                    if srect.contains(mp) {
                        hit_group = Some(state.doc.nodes[gi].id.clone());
                        break;
                    }
                }
                if let Some(gid) = hit_group {
                    state.selected_node_id = Some(gid.clone());
                    state.selected_edge_id = None;
                    state.context_menu_node = Some(gid);
                    state.context_menu_edge = None;
                } else {
                    state.context_menu_node = None;
                    state.context_menu_edge = None;
                }
            }
        }
    }

    if let Some(target_path) = open_external_target {
        match VaultFileKind::from_path(&target_path) {
            Some(VaultFileKind::Board | VaultFileKind::Table) => {
                actions.push(UiAction::OpenCollection(target_path));
            }
            Some(VaultFileKind::Canvas) => {
                actions.push(UiAction::OpenCanvas(target_path));
            }
            Some(VaultFileKind::Asset) => {
                actions.push(UiAction::OpenImageLightbox(target_path));
            }
            _ => {
                actions.push(UiAction::OpenNote(target_path));
            }
        }
    }

    // Smart alignment guides & active marquee rectangle
    for &gx in &state.smart_guides_x {
        let sx = state.viewport.world_to_screen(canvas_rect, pos2(gx, 0.0)).x;
        painter.line_segment(
            [pos2(sx, canvas_rect.top()), pos2(sx, canvas_rect.bottom())],
            Stroke::new(1.0_f32, with_alpha(p.accent, 0.72)),
        );
    }
    for &gy in &state.smart_guides_y {
        let sy = state.viewport.world_to_screen(canvas_rect, pos2(0.0, gy)).y;
        painter.line_segment(
            [pos2(canvas_rect.left(), sy), pos2(canvas_rect.right(), sy)],
            Stroke::new(1.0_f32, with_alpha(p.accent, 0.72)),
        );
    }

    if let Some(m_start) = state.marquee_start_world {
        if let Some(wp) = pointer_world {
            state.marquee_current_world = Some(wp);
        }
        let m_end = state.marquee_current_world.unwrap_or(m_start);
        let world_box = Rect::from_two_pos(m_start, m_end);
        let screen_box = state.viewport.world_rect_to_screen(canvas_rect, world_box);
        if primary_down {
            painter.rect_filled(
                screen_box,
                Rounding::same(3.0),
                with_alpha(p.accent, 0.14),
            );
            painter.rect_stroke(
                screen_box,
                Rounding::same(3.0),
                Stroke::new(1.2_f32, p.accent),
            );
            ctx.set_cursor_icon(CursorIcon::Crosshair);
        } else {
            if world_box.width() > 4.0 || world_box.height() > 4.0 {
                let mut picked: Vec<String> = state
                    .doc
                    .nodes
                    .iter()
                    .filter(|n| n.kind != CanvasNodeKind::Group && world_box.intersects(node_world_rect(n)))
                    .map(|n| n.id.clone())
                    .collect();
                if picked.is_empty() {
                    picked = state
                        .doc
                        .nodes
                        .iter()
                        .filter(|n| world_box.intersects(node_world_rect(n)))
                        .map(|n| n.id.clone())
                        .collect();
                }
                state.clear_selection();
                state.selected_node_ids = picked.iter().cloned().collect();
                state.selected_node_id = picked.first().cloned();
            }
            state.marquee_start_world = None;
            state.marquee_current_world = None;
        }
    }

    // Background Pan / Zoom & Safe Single-Click Deselect (only if pressed on empty bg!)
    let allow_pan = state.dragging_node.is_none()
        && state.resizing_node.is_none()
        && state.connecting_from.is_none()
        && state.marquee_start_world.is_none()
        && state.editing_node_id.is_none();
    state
        .viewport
        .handle_pan_zoom_ex(ui, canvas_rect, &bg_resp, allow_pan, false, !pointer_over_selected_card);
    if clicked {
        if state.pressed_on_empty_bg && !hit_handled {
            state.clear_selection();
            state.editing_node_id = None;
        }
        state.pressed_on_empty_bg = false;
    } else if primary_released {
        state.pressed_on_empty_bg = false;
    }

    // 5. External conflict banner (if open canvas changed on disk while dirty)
    let mut toolbar_top_offset = 12.0;
    if state.external_conflict {
        toolbar_top_offset = 56.0;
        let banner_r = Rect::from_min_size(
            pos2(canvas_rect.left() + 14.0, canvas_rect.top() + 10.0),
            vec2((canvas_rect.width() - 28.0).max(200.0), 38.0),
        );
        let mut b_ui = ui.new_child(egui::UiBuilder::new().max_rect(banner_r));
        b_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
        egui::Frame::none()
            .fill(p.warning_soft)
            .stroke(Stroke::new(1.0_f32, p.warning))
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(12.0, 6.0))
            .show(&mut b_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(s.ext_conflict_msg)
                            .font(font_medium(12.0))
                            .color(p.text_primary),
                    );
                    if secondary_button(ui, p, None, s.ext_conflict_reload).clicked() {
                        actions.push(UiAction::CanvasExternalReload);
                    }
                    if secondary_button(ui, p, None, s.ext_conflict_keep).clicked() {
                        actions.push(UiAction::CanvasExternalKeepMine);
                    }
                    if secondary_button(ui, p, None, s.ext_conflict_copy).clicked() {
                        actions.push(UiAction::CanvasExternalSaveConflictCopy);
                    }
                });
            });
    }

    // 6. Floating Top-Center Canvas Toolbar
    let has_sel_node = !state.selected_node_ids_vec().is_empty();
    let has_sel_edge = state.selected_edge_id.is_some();
    let compact_tb = layout.is_mobile() || canvas_rect.width() < 620.0;
    let tb_max_w = (canvas_rect.width() - 24.0).max(220.0);
    let tb_rect = Rect::from_min_size(
        pos2(canvas_rect.left() + 12.0, canvas_rect.top() + toolbar_top_offset),
        vec2(tb_max_w, 46.0),
    );
    let mut tb_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(tb_rect)
            .layout(Layout::top_down(Align::LEFT)),
    );
    tb_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
    tb_ui.spacing_mut().item_spacing.x = 5.0;

    let tb_frame_resp = egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(10.0))
        .shadow(p.shadow_elevated)
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(&mut tb_ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                let center_world = state.viewport.pan;
                if compact_tb {
                    if icon_button(ui, p, Icon::StickyNote, s.canvas_add_text, false).clicked() {
                        state.push_undo();
                        let id = state.doc.add_text_node(
                            snap(center_world.x - 124.0, state.snap_to_grid),
                            snap(center_world.y - 66.0, state.snap_to_grid),
                            if lang == Lang::Ru { "Новая карточка" } else { "New card" },
                            Some("5"),
                        );
                        state.selected_node_id = Some(id.clone());
                        state.editing_node_id = Some(id);
                        dirty = true;
                    }
                    if icon_button(ui, p, Icon::FilePlus, s.canvas_add_file, false).clicked() {
                        state.file_picker_open = true;
                        state.file_picker_world = None;
                        state.file_picker_query.clear();
                    }
                    if icon_button(ui, p, Icon::Image, s.canvas_add_image, false).clicked() {
                        actions.push(UiAction::PickImageForCanvas);
                    }
                    if icon_button(ui, p, Icon::Group, s.canvas_add_group, false).clicked() {
                        state.push_undo();
                        let id = state.doc.add_group_node(
                            snap(center_world.x - 160.0, state.snap_to_grid),
                            snap(center_world.y - 110.0, state.snap_to_grid),
                            320.0,
                            220.0,
                            if lang == Lang::Ru { "Секция" } else { "Section" },
                            Some("6"),
                        );
                        state.selected_node_id = Some(id);
                        dirty = true;
                    }
                } else {
                    if secondary_button(ui, p, Some(Icon::StickyNote), s.canvas_add_text).clicked() {
                        state.push_undo();
                        let id = state.doc.add_text_node(
                            snap(center_world.x - 124.0, state.snap_to_grid),
                            snap(center_world.y - 66.0, state.snap_to_grid),
                            if lang == Lang::Ru { "Новая карточка" } else { "New card" },
                            Some("5"),
                        );
                        state.selected_node_id = Some(id.clone());
                        state.editing_node_id = Some(id);
                        dirty = true;
                    }
                    if secondary_button(ui, p, Some(Icon::FilePlus), s.canvas_add_file).clicked() {
                        state.file_picker_open = true;
                        state.file_picker_world = None;
                        state.file_picker_query.clear();
                    }
                    if secondary_button(ui, p, Some(Icon::Image), s.canvas_add_image).clicked() {
                        actions.push(UiAction::PickImageForCanvas);
                    }
                    if secondary_button(ui, p, Some(Icon::Group), s.canvas_add_group).clicked() {
                        state.push_undo();
                        let id = state.doc.add_group_node(
                            snap(center_world.x - 160.0, state.snap_to_grid),
                            snap(center_world.y - 110.0, state.snap_to_grid),
                            320.0,
                            220.0,
                            if lang == Lang::Ru { "Секция" } else { "Section" },
                            Some("6"),
                        );
                        state.selected_node_id = Some(id);
                        dirty = true;
                    }
                }

                if icon_button(ui, p, Icon::Search, s.canvas_search_hint, state.search_open).clicked() {
                    state.search_open = !state.search_open;
                }
                if icon_button(
                    ui,
                    p,
                    Icon::SelectBox,
                    s.canvas_marquee_tool,
                    state.marquee_mode,
                )
                .clicked()
                {
                    state.marquee_mode = !state.marquee_mode;
                }
                if state
                    .path
                    .as_deref()
                    .is_some_and(crate::app::english::is_english_canvas_path)
                {
                    if primary_button(ui, p, Some(Icon::Plus), s.eng_new_lesson_btn, false).clicked()
                    {
                        actions.push(UiAction::CreateNextEnglishLesson { open_note: false });
                    }
                    if secondary_button(ui, p, Some(Icon::Table), s.eng_vocab_btn).clicked() {
                        actions.push(UiAction::OpenEnglishVocabulary);
                    }
                }
                if icon_button(ui, p, Icon::Magnet, s.canvas_snap_grid, state.snap_to_grid).clicked() {
                    state.snap_to_grid = !state.snap_to_grid;
                }
                ui.add_enabled_ui(!state.undo_stack.is_empty(), |ui| {
                    if icon_button(ui, p, Icon::Undo, s.canvas_undo, false).clicked() && state.undo() {
                        dirty = true;
                    }
                });
                ui.add_enabled_ui(!state.redo_stack.is_empty(), |ui| {
                    if icon_button(ui, p, Icon::Redo, s.canvas_redo, false).clicked() && state.redo() {
                        dirty = true;
                    }
                });
                if icon_button(ui, p, Icon::Sparkles, s.canvas_tidy, false).clicked() && state.auto_layout() {
                    dirty = true;
                }
                if !compact_tb {
                    if icon_button(ui, p, Icon::Map, s.canvas_minimap, state.show_minimap).clicked() {
                        state.show_minimap = !state.show_minimap;
                    }
                }

                if !compact_tb {
                    if let Some(sel_id) = state.selected_node_id.clone() {
                        if icon_button(ui, p, Icon::Copy, s.canvas_duplicate, false).clicked() && state.duplicate_selected() {
                            dirty = true;
                        }
                        for (code, tok) in PALETTE_SWATCHES {
                            let dot = p.token_colors(tok).1;
                            let is_cur = state
                                .doc
                                .nodes
                                .iter()
                                .find(|n| n.id == sel_id)
                                .and_then(|n| n.color.as_deref())
                                == Some(code);
                            let (sr, sresp) = ui.allocate_exact_size(vec2(15.0, 22.0), Sense::click());
                            ui.painter().circle_filled(sr.center(), 5.2, dot);
                            if is_cur {
                                ui.painter()
                                    .circle_stroke(sr.center(), 7.0, Stroke::new(1.3_f32, p.text_primary));
                            }
                            if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                state.push_undo();
                                let sel_set: HashSet<String> =
                                    state.selected_node_ids_vec().into_iter().collect();
                                for n in &mut state.doc.nodes {
                                    if n.id == sel_id || sel_set.contains(&n.id) {
                                        n.color = Some(code.to_string());
                                        dirty = true;
                                    }
                                }
                            }
                        }
                    }
                }

                if !compact_tb && (has_sel_node || has_sel_edge) {
                    if icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked() {
                        state.push_undo();
                        let sel_ids = state.selected_node_ids_vec();
                        if !sel_ids.is_empty() {
                            state.clear_selection();
                            for nid in sel_ids {
                                state.doc.remove_node(&nid);
                            }
                            dirty = true;
                        } else if let Some(eid) = state.selected_edge_id.take() {
                            state.doc.remove_edge(&eid);
                            dirty = true;
                        }
                    }
                }

                if compact_tb {
                    if icon_button(ui, p, Icon::FileOutput, s.canvas_export_md, false).clicked() {
                        actions.push(UiAction::ExportCanvasToMarkdown);
                    }
                } else if primary_button(ui, p, Some(Icon::FileOutput), s.canvas_export_md, false)
                    .clicked()
                {
                    actions.push(UiAction::ExportCanvasToMarkdown);
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("c_hud_block_tb"), tb_frame_resp.response.rect));

    // 6b. Compact/Mobile floating selection action pill below toolbar
    if compact_tb && (has_sel_node || has_sel_edge) {
        let sel_r = Rect::from_min_size(
            pos2(canvas_rect.left() + 12.0, tb_frame_resp.response.rect.bottom() + 6.0),
            vec2(tb_max_w, 38.0),
        );
        let mut sel_ui = ui.new_child(egui::UiBuilder::new().max_rect(sel_r));
        sel_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
        egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(9.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::symmetric(8.0, 4.0))
            .show(&mut sel_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if let Some(sel_id) = state.selected_node_id.clone() {
                        let is_text = state
                            .doc
                            .nodes
                            .iter()
                            .find(|n| n.id == sel_id)
                            .is_some_and(|n| matches!(n.kind, CanvasNodeKind::Text | CanvasNodeKind::Group));
                        if is_text && icon_button(ui, p, Icon::Edit, s.canvas_edit_card, false).clicked() {
                            state.push_undo();
                            state.editing_node_id = Some(sel_id.clone());
                        }
                        if icon_button(ui, p, Icon::Copy, s.canvas_duplicate, false).clicked()
                            && state.duplicate_selected()
                        {
                            dirty = true;
                        }
                        for (code, tok) in PALETTE_SWATCHES {
                            let dot = p.token_colors(tok).1;
                            let is_cur = state
                                .doc
                                .nodes
                                .iter()
                                .find(|n| n.id == sel_id)
                                .and_then(|n| n.color.as_deref())
                                == Some(code);
                            let (sr, sresp) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::click());
                            ui.painter().circle_filled(sr.center(), 5.0, dot);
                            if is_cur {
                                ui.painter().circle_stroke(
                                    sr.center(),
                                    6.8,
                                    Stroke::new(1.3_f32, p.text_primary),
                                );
                            }
                            if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                state.push_undo();
                                let sel_set: HashSet<String> =
                                    state.selected_node_ids_vec().into_iter().collect();
                                for n in &mut state.doc.nodes {
                                    if n.id == sel_id || sel_set.contains(&n.id) {
                                        n.color = Some(code.to_string());
                                        dirty = true;
                                    }
                                }
                            }
                        }
                    }
                    if icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked() {
                        state.push_undo();
                        let sel_ids = state.selected_node_ids_vec();
                        if !sel_ids.is_empty() {
                            state.clear_selection();
                            for nid in sel_ids {
                                state.doc.remove_node(&nid);
                            }
                            dirty = true;
                        } else if let Some(eid) = state.selected_edge_id.take() {
                            state.doc.remove_edge(&eid);
                            dirty = true;
                        }
                    }
                });
            });
    }

    // 7. Floating Bottom-Left Selected Edge Pill (Label, Styles, Flip Direction, Color Swatches)
    if let Some(eid) = state.selected_edge_id.clone() {
        let ep_w = (canvas_rect.width() - 210.0).clamp(220.0, 468.0);
        let ep_rect = Rect::from_min_size(
            pos2(canvas_rect.left() + 14.0, (canvas_rect.bottom() - 52.0).max(canvas_rect.top() + 54.0)),
            vec2(ep_w, 40.0),
        );
        let mut ep_ui = ui.new_child(egui::UiBuilder::new().max_rect(ep_rect));
        ep_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
        let ep_resp = egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(9.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::symmetric(8.0, 5.0))
            .show(&mut ep_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    let input_w = (ep_w - 252.0).max(86.0);
                    let r = styled_text_input(
                        ui,
                        p,
                        &mut state.edge_label_draft,
                        s.canvas_edge_label_hint,
                        false,
                        input_w,
                    );
                    if state.focus_edge_label_next {
                        r.request_focus();
                        state.focus_edge_label_next = false;
                    }
                    if r.changed() {
                        if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                            let tr = state.edge_label_draft.trim();
                            e.label = if tr.is_empty() {
                                None
                    } else {
                                Some(tr.to_string())
                            };
                            dirty = true;
                        }
                    }

                    if let Some(cur_edge) = state.doc.edges.iter().find(|e| e.id == eid).cloned() {
                        let is_one = cur_edge.from_end.unwrap_or(CanvasEnd::None) == CanvasEnd::None
                            && cur_edge.to_end.unwrap_or(CanvasEnd::Arrow) == CanvasEnd::Arrow;
                        let is_both = cur_edge.from_end == Some(CanvasEnd::Arrow)
                            && cur_edge.to_end == Some(CanvasEnd::Arrow);
                        let is_none = cur_edge.from_end == Some(CanvasEnd::None)
                            && cur_edge.to_end == Some(CanvasEnd::None);

                        for (lbl, active, new_ends) in [
                            ("→", is_one, (None, Some(CanvasEnd::Arrow))),
                            ("↔", is_both, (Some(CanvasEnd::Arrow), Some(CanvasEnd::Arrow))),
                            ("—", is_none, (Some(CanvasEnd::None), Some(CanvasEnd::None))),
                        ] {
                            let (br, bresp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                            ui.painter().rect_filled(
                                br,
                                Rounding::same(5.0),
                                if active {
                                    p.accent_soft
                                } else if bresp.hovered() {
                                    p.bg_hover
                    } else {
                                    p.bg_input
                                },
                            );
                            let fg = if active { p.accent_text } else { p.text_secondary };
                            let g = ui.painter().layout_no_wrap(lbl.to_string(), font_mono(11.5), fg);
                            ui.painter().galley(
                                pos2(br.center().x - g.size().x * 0.5, br.center().y - g.size().y * 0.5),
                                g,
                                fg,
                            );
                            if bresp
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .on_hover_text(s.canvas_arrow_style)
                                .clicked()
                            {
                                state.push_undo();
                                if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                                    e.from_end = new_ends.0;
                                    e.to_end = new_ends.1;
                                    dirty = true;
                                }
                            }
                        }

                        if icon_button(ui, p, Icon::SwapArrows, s.canvas_flip_edge, false).clicked() {
                            state.push_undo();
                            if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                                std::mem::swap(&mut e.from_node, &mut e.to_node);
                                std::mem::swap(&mut e.from_side, &mut e.to_side);
                                std::mem::swap(&mut e.from_end, &mut e.to_end);
                                dirty = true;
                            }
                        }

                        if ep_w >= 380.0 {
                            for (code, tok) in PALETTE_SWATCHES {
                                let dot = p.token_colors(tok).1;
                                let is_cur = cur_edge.color.as_deref() == Some(code);
                                let (sr, sresp) = ui.allocate_exact_size(vec2(13.0, 22.0), Sense::click());
                                ui.painter().circle_filled(sr.center(), 4.6, dot);
                                if is_cur {
                                    ui.painter().circle_stroke(
                                        sr.center(),
                                        6.3,
                                        Stroke::new(1.2_f32, p.text_primary),
                                    );
                                }
                                if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    state.push_undo();
                                    if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                                        e.color = Some(code.to_string());
                                        dirty = true;
                                    }
                                }
                            }
                        }
                    }

                    if icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked() {
                        state.push_undo();
                        state.doc.remove_edge(&eid);
                        state.selected_edge_id = None;
                        dirty = true;
                    }
                });
            });
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("c_hud_block_edge"), ep_resp.response.rect));
    } else {
        let multi_ids = state.selected_node_ids_vec();
        if multi_ids.len() > 1 {
            let mp_w = (canvas_rect.width() - 210.0).clamp(240.0, 520.0);
            let mp_rect = Rect::from_min_size(
                pos2(
                    canvas_rect.left() + 14.0,
                    (canvas_rect.bottom() - 52.0).max(canvas_rect.top() + 54.0),
                ),
                vec2(mp_w, 40.0),
            );
            let mut mp_ui = ui.new_child(egui::UiBuilder::new().max_rect(mp_rect));
            mp_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
            let mp_resp = egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(9.0))
                .shadow(p.shadow_elevated)
                .inner_margin(Margin::symmetric(8.0, 5.0))
                .show(&mut mp_ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                multi_ids.len(),
                                s.canvas_selected_multi
                            ))
                            .font(font_semibold(12.0))
                            .color(p.accent_text),
                        );
                        if secondary_button(ui, p, None, s.canvas_align_left).clicked()
                            && state.align_selected_left()
                        {
                            dirty = true;
                        }
                        if secondary_button(ui, p, None, s.canvas_align_top).clicked()
                            && state.align_selected_top()
                        {
                            dirty = true;
                        }
                        if mp_w >= 430.0 {
                            for (code, tok) in PALETTE_SWATCHES {
                                let dot = p.token_colors(tok).1;
                                let (sr, sresp) =
                                    ui.allocate_exact_size(vec2(13.0, 22.0), Sense::click());
                                ui.painter().circle_filled(sr.center(), 4.6, dot);
                                if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    state.push_undo();
                                    let sel_set: HashSet<&str> =
                                        multi_ids.iter().map(|s| s.as_str()).collect();
                                    for n in &mut state.doc.nodes {
                                        if sel_set.contains(n.id.as_str()) {
                                            n.color = Some(code.to_string());
                                            dirty = true;
                                        }
                                    }
                                }
                            }
                        }
                        if icon_button(ui, p, Icon::Copy, s.canvas_duplicate, false).clicked()
                            && state.duplicate_selected()
                        {
                            dirty = true;
                        }
                        if icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked() {
                            state.push_undo();
                            state.clear_selection();
                            for nid in &multi_ids {
                                state.doc.remove_node(nid);
                            }
                            dirty = true;
                        }
                    });
                });
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("c_hud_block_edge"), mp_resp.response.rect)
            });
        } else {
            ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("c_hud_block_edge")));
        }
    }

    // 7b. Floating Top-Right Canvas Node Search (Ctrl+F)
    let search_gate = egui::Id::new("canvas_search_foc_gate");
    if state.search_open {
        let sw = 246.0_f32.min((canvas_rect.width() - 28.0).max(180.0));
        let srect = Rect::from_min_size(
            pos2(canvas_rect.right() - sw - 12.0, tb_frame_resp.response.rect.bottom() + 8.0),
            vec2(sw, 260.0),
        );
        let mut s_ui = ui.new_child(egui::UiBuilder::new().max_rect(srect));
        s_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
        let s_resp = egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(9.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::same(8.0))
            .show(&mut s_ui, |ui| {
                ui.set_width(sw - 16.0);
                ui.horizontal(|ui| {
                    let sr = styled_text_input(
                        ui,
                        p,
                        &mut state.search_query,
                        s.canvas_search_hint,
                        false,
                        sw - 48.0,
                    );
                    focus_once(ui.ctx(), search_gate, true, sr.id);
                    if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                        state.search_open = false;
                    }
                });

                let q = state.search_query.trim().to_lowercase();
                let matches: Vec<(String, String, Pos2)> = state
                    .doc
                    .nodes
                    .iter()
                    .filter(|n| {
                        if q.is_empty() {
                            return true;
                        }
                        n.display_title().to_lowercase().contains(&q)
                            || n.text.as_deref().is_some_and(|t| t.to_lowercase().contains(&q))
                            || n.file.as_deref().is_some_and(|f| f.to_lowercase().contains(&q))
                    })
                    .take(6)
                    .map(|n| (n.id.clone(), n.display_title(), pos2(n.x + n.width * 0.5, n.y + n.height * 0.5)))
                    .collect();

                if !matches.is_empty() {
                    ui.add_space(6.0);
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    for (idx, (nid, title, center)) in matches.into_iter().enumerate() {
                        let sel = state.selected_node_id.as_deref() == Some(nid.as_str());
                        let row_resp = context_menu_row(ui, p, Icon::StickyNote, &title, false);
                        if sel {
                            ui.painter().rect_stroke(
                                row_resp.rect,
                                Rounding::same(5.0),
                                Stroke::new(1.0_f32, p.accent),
                            );
                        }
                        if row_resp.clicked() || (enter && idx == 0) {
                            state.select_single_node(nid);
                            state.viewport.target_pan = center.to_vec2();
                            state.viewport.target_zoom = state.viewport.target_zoom.max(0.85);
                        }
                    }
                }
            });
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("c_hud_block_search"), s_resp.response.rect));
    } else {
        reset_focus_gate(ctx, search_gate);
        ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("c_hud_block_search")));
    }

    // 8. Bottom-Right Zoom HUD + Interactive Mini-Map
    render_zoom_hud(ui, canvas_rect, p, lang, &mut state.viewport, bounds);
    if !layout.is_mobile() && state.show_minimap && !state.doc.nodes.is_empty() {
        render_canvas_minimap(ui, canvas_rect, p, state, bounds);
    } else {
        ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("c_hud_block_minimap")));
    }

    // 8b. Right-Click Context Menu
    if state.context_menu_pos.is_some() {
        if render_canvas_context_menu(ctx, p, lang, state, &mut actions) {
            dirty = true;
        }
    } else {
        ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("c_hud_block_ctx")));
    }

    // 9. File Picker Overlay (drop Note / Board / Table onto Canvas)
    let fp_gate_id = egui::Id::new("canvas_fp_foc_gate");
    if state.file_picker_open {
        if render_file_picker_modal(ctx, p, lang, state, notes, collections, fp_gate_id) {
            dirty = true;
        }
    } else {
        reset_focus_gate(ctx, fp_gate_id);
    }

    // 10. Convert Text Card to Note Modal
    let convert_gate_id = egui::Id::new("canvas_convert_card_foc_gate");
    if let Some((node_id, mut title_draft)) = state.convert_card_modal.clone() {
        let mut keep_open = true;
        let mut confirm = false;
        let folder_preview = state
            .path
            .as_deref()
            .map(canvas_folder_for_path)
            .unwrap_or_else(|| "Canvas".to_string());
        egui::Window::new(s.canvas_convert_modal_title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(vec2(360.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(p.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                    .rounding(Rounding::same(12.0))
                    .shadow(p.shadow_floating)
                    .inner_margin(Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(format!("{}: {folder_preview}/", s.canvas_convert_folder_hint))
                        .font(font_mono(11.5))
                        .color(p.text_muted),
                );
                ui.add_space(8.0);
                let resp = styled_text_input(ui, p, &mut title_draft, "note_title", false, 328.0);
                focus_once(ctx, convert_gate_id, true, resp.id);
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    confirm = true;
                }
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    keep_open = false;
                }
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                        keep_open = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if primary_button(ui, p, Some(Icon::FileOutput), s.canvas_convert_confirm, false)
                            .clicked()
                        {
                            confirm = true;
                        }
                    });
                });
            });
        if confirm {
            actions.push(UiAction::ConvertCanvasCardToNote {
                node_id,
                custom_title: title_draft,
            });
            state.convert_card_modal = None;
        } else if keep_open {
            state.convert_card_modal = Some((node_id, title_draft));
        } else {
            state.convert_card_modal = None;
        }
    } else {
        reset_focus_gate(ctx, convert_gate_id);
    }

    if dirty {
        actions.push(UiAction::MarkCanvasDirty);
    }

    actions
}

fn render_text_card_body(
    painter: &egui::Painter,
    inner: Rect,
    zoom: f32,
    p: &ThemePalette,
    body: &str,
) {
    let mut y = inner.top();
    for (li, raw_line) in body.lines().enumerate() {
        if y > inner.bottom() - 12.0 {
            break;
        }
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            y += (7.0 * zoom).clamp(4.0, 10.0);
            continue;
        }

        // Visual checklist checkboxes for `- [ ] ` and `- [x] `
        if let Some((done, rest)) = trimmed
            .strip_prefix("- [ ] ")
            .map(|r| (false, r))
            .or_else(|| trimmed.strip_prefix("- [x] ").map(|r| (true, r)))
            .or_else(|| trimmed.strip_prefix("- [X] ").map(|r| (true, r)))
        {
            let font_sz = (12.0 * zoom).clamp(9.0, 15.5);
            let box_sz = (11.5 * zoom).clamp(8.5, 14.0);
            let box_r = Rect::from_min_size(pos2(inner.left(), y + 2.0 * zoom), vec2(box_sz, box_sz));
            if done {
                painter.rect_filled(box_r, Rounding::same(2.5), p.accent);
                draw_icon(painter, box_r.shrink(1.5), Icon::Check, Color32::WHITE);
            } else {
                painter.rect_stroke(box_r, Rounding::same(2.5), Stroke::new(1.1_f32, p.border_strong));
            }
            let text_col = if done { p.text_muted } else { p.text_secondary };
            let indent_x = box_sz + 6.0 * zoom;
            let g = painter.layout(
                rest.to_string(),
                font_body(font_sz),
                text_col,
                (inner.width() - indent_x).max(32.0),
            );
            let gh = g.size().y.max(box_sz);
            painter.galley(pos2(inner.left() + indent_x, y), g, text_col);
            y += gh + (3.0 * zoom).clamp(2.0, 5.0);
            continue;
        }

        let is_h = trimmed.starts_with('#');
        let display = if is_h {
            trimmed.trim_start_matches('#').trim().to_string()
        } else {
            trimmed.to_string()
        };
        let font_sz = ((if is_h || li == 0 { 13.0 } else { 12.0 }) * zoom).clamp(9.0, 16.0);
        let font = if is_h || li == 0 {
            font_semibold(font_sz)
        } else {
            font_body(font_sz)
        };
        let col = if is_h || li == 0 {
            p.text_primary
        } else {
            p.text_secondary
        };
        let g = painter.layout(display, font, col, inner.width().max(40.0));
        let gh = g.size().y;
        painter.galley(pos2(inner.left(), y), g, col);
        y += gh + (3.0 * zoom).clamp(2.0, 5.0);
    }
}

fn render_canvas_minimap(
    ui: &mut Ui,
    canvas_rect: Rect,
    p: &ThemePalette,
    state: &mut CanvasEditorState,
    bounds: Option<Rect>,
) {
    let Some(wb) = bounds else {
        return;
    };
    let map_w = 148.0;
    let map_h = 94.0;
    if canvas_rect.width() < 440.0 || canvas_rect.height() < 260.0 {
        return;
    }
    let map_rect = Rect::from_min_size(
        pos2(canvas_rect.right() - map_w - 14.0, canvas_rect.bottom() - map_h - 56.0),
        vec2(map_w, map_h),
    );
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("c_hud_block_minimap"), map_rect.expand(2.0)));

    let painter = ui.painter_at(canvas_rect);
    painter.add(p.shadow_elevated.as_shape(map_rect, Rounding::same(8.0)));
    painter.rect_filled(map_rect, Rounding::same(8.0), with_alpha(p.bg_elevated, 0.92));
    painter.rect_stroke(map_rect, Rounding::same(8.0), Stroke::new(1.0_f32, p.border_subtle));

    let view_world = Rect::from_center_size(
        state.viewport.pan.to_pos2(),
        vec2(
            canvas_rect.width() / state.viewport.zoom.max(0.1),
            canvas_rect.height() / state.viewport.zoom.max(0.1),
        ),
    );
    let total_world = wb.union(view_world).expand(120.0);
    let inner = map_rect.shrink(6.0);
    let sx = inner.width() / total_world.width().max(1.0);
    let sy = inner.height() / total_world.height().max(1.0);
    let scale = sx.min(sy);
    let off = pos2(
        inner.center().x - total_world.center().x * scale,
        inner.center().y - total_world.center().y * scale,
    );
    let to_mini = |wp: Pos2| -> Pos2 { pos2(off.x + wp.x * scale, off.y + wp.y * scale) };

    for n in &state.doc.nodes {
        let nr = node_world_rect(n);
        let mr = Rect::from_min_max(to_mini(nr.min), to_mini(nr.max));
        let acc = canvas_accent_color(p, n.color.as_deref()).unwrap_or(p.text_muted);
        if n.kind == CanvasNodeKind::Group {
            painter.rect_stroke(mr, Rounding::same(2.0), Stroke::new(1.0_f32, with_alpha(acc, 0.6)));
        } else {
            let is_sel = state.is_node_selected(&n.id);
            painter.rect_filled(
                mr,
                Rounding::same(2.0),
                if is_sel { p.accent } else { with_alpha(acc, 0.72) },
            );
        }
    }

    let vr = Rect::from_min_max(to_mini(view_world.min), to_mini(view_world.max)).intersect(inner);
    if vr.width() > 2.0 && vr.height() > 2.0 {
        painter.rect_filled(vr, Rounding::same(2.0), with_alpha(p.accent, 0.10));
        painter.rect_stroke(vr, Rounding::same(2.0), Stroke::new(1.2_f32, p.accent));
    }

    let resp = ui.interact(map_rect, egui::Id::new("krono_canvas_minimap_hit"), Sense::click_and_drag());
    if (resp.clicked() || resp.dragged()) && resp.interact_pointer_pos().is_some() {
        if let Some(mp) = resp.interact_pointer_pos() {
            state.viewport.pan = vec2((mp.x - off.x) / scale, (mp.y - off.y) / scale);
            state.viewport.target_pan = state.viewport.pan;
        }
    }
}

fn render_canvas_context_menu(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CanvasEditorState,
    actions: &mut Vec<UiAction>,
) -> bool {
    let Some(pos) = state.context_menu_pos else {
        return false;
    };
    let s = strings(lang);
    let screen = ctx.screen_rect();
    let menu_w = 186.0;
    let mx = pos.x.clamp(screen.left() + 8.0, (screen.right() - menu_w - 8.0).max(screen.left() + 8.0));
    let my = pos.y.clamp(screen.top() + 8.0, (screen.bottom() - 220.0).max(screen.top() + 8.0));
    let mut dirty = false;
    let mut close = false;

    let area_resp = egui::Area::new(egui::Id::new("canvas_ctx_menu_area"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(mx, my))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(9.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(6.0))
                .show(ui, |ui| {
                    ui.set_width(menu_w - 12.0);
                    ui.spacing_mut().item_spacing.y = 2.0;

                    if let Some(nid) = state.context_menu_node.clone() {
                        let is_editable = state
                            .doc
                            .nodes
                            .iter()
                            .find(|n| n.id == nid)
                            .is_some_and(|n| matches!(n.kind, CanvasNodeKind::Text | CanvasNodeKind::Group));
                        let is_text_card = state
                            .doc
                            .nodes
                            .iter()
                            .find(|n| n.id == nid)
                            .is_some_and(|n| n.kind == CanvasNodeKind::Text);
                        if is_editable
                            && context_menu_row(ui, p, Icon::Edit, s.canvas_edit_card, false).clicked()
                        {
                            state.push_undo();
                            state.selected_node_id = Some(nid.clone());
                            state.editing_node_id = Some(nid.clone());
                            close = true;
                        }
                        if is_text_card
                            && context_menu_row(
                                ui,
                                p,
                                Icon::FileOutput,
                                s.canvas_convert_to_note,
                                false,
                            )
                            .clicked()
                        {
                            let raw_text = state
                                .doc
                                .nodes
                                .iter()
                                .find(|n| n.id == nid)
                                .and_then(|n| n.text.as_deref())
                                .unwrap_or("");
                            let fb = if lang == Lang::Ru { "Заметка" } else { "Note" };
                            let suggested = suggest_note_title_from_card(raw_text, fb);
                            state.convert_card_modal = Some((nid.clone(), suggested));
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Copy, s.canvas_duplicate, false).clicked() {
                            state.selected_node_id = Some(nid.clone());
                            if state.duplicate_selected() {
                                dirty = true;
                            }
                            close = true;
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.add_space(6.0);
                            ui.spacing_mut().item_spacing.x = 5.0;
                            for (code, tok) in PALETTE_SWATCHES {
                                let dot = p.token_colors(tok).1;
                                let (sr, sresp) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::click());
                                ui.painter().circle_filled(sr.center(), 5.5, dot);
                                if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    state.push_undo();
                                    if let Some(n) = state.doc.nodes.iter_mut().find(|n| n.id == nid) {
                                        n.color = Some(code.to_string());
                                        dirty = true;
                                    }
                                    close = true;
                                }
                            }
                        });
                        if context_menu_row(ui, p, Icon::ArrowLeft, s.canvas_front, false).clicked() {
                            if let Some(pos_idx) = state.doc.nodes.iter().position(|n| n.id == nid) {
                                state.push_undo();
                                let item = state.doc.nodes.remove(pos_idx);
                                state.doc.nodes.push(item);
                                dirty = true;
                            }
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Minus, s.canvas_back, false).clicked() {
                            if let Some(pos_idx) = state.doc.nodes.iter().position(|n| n.id == nid) {
                                state.push_undo();
                                let item = state.doc.nodes.remove(pos_idx);
                                state.doc.nodes.insert(0, item);
                                dirty = true;
                            }
                            close = true;
                        }
                        hairline_h(ui, p);
                        if context_menu_row(ui, p, Icon::Trash, s.delete_btn, true).clicked() {
                            state.push_undo();
                            state.doc.remove_node(&nid);
                            if state.selected_node_id.as_deref() == Some(nid.as_str()) {
                                state.selected_node_id = None;
                            }
                            dirty = true;
                            close = true;
                        }
                    } else if let Some(eid) = state.context_menu_edge.clone() {
                        if context_menu_row(ui, p, Icon::Link, s.canvas_arrow_style, false).clicked() {
                            state.push_undo();
                            if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                                match (e.from_end, e.to_end) {
                                    (None, Some(CanvasEnd::Arrow)) | (None, None) => {
                                        e.from_end = Some(CanvasEnd::Arrow);
                                        e.to_end = Some(CanvasEnd::Arrow);
                                    }
                                    (Some(CanvasEnd::Arrow), Some(CanvasEnd::Arrow)) => {
                                        e.from_end = Some(CanvasEnd::None);
                                        e.to_end = Some(CanvasEnd::None);
                                    }
                                    _ => {
                                        e.from_end = None;
                                        e.to_end = Some(CanvasEnd::Arrow);
                                    }
                                }
                                dirty = true;
                            }
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::SwapArrows, s.canvas_flip_edge, false).clicked() {
                            state.push_undo();
                            if let Some(e) = state.doc.edges.iter_mut().find(|e| e.id == eid) {
                                std::mem::swap(&mut e.from_node, &mut e.to_node);
                                std::mem::swap(&mut e.from_side, &mut e.to_side);
                                std::mem::swap(&mut e.from_end, &mut e.to_end);
                                dirty = true;
                            }
                            close = true;
                        }
                        hairline_h(ui, p);
                        if context_menu_row(ui, p, Icon::Trash, s.delete_btn, true).clicked() {
                            state.push_undo();
                            state.doc.remove_edge(&eid);
                            if state.selected_edge_id.as_deref() == Some(eid.as_str()) {
                                state.selected_edge_id = None;
                            }
                            dirty = true;
                            close = true;
                        }
                    } else {
                        let wp = state.context_menu_world;
                        if context_menu_row(ui, p, Icon::StickyNote, s.canvas_add_text, false).clicked() {
                            state.push_undo();
                            let id = state.doc.add_text_node(
                                snap(wp.x - 115.0, state.snap_to_grid),
                                snap(wp.y - 58.0, state.snap_to_grid),
                                if lang == Lang::Ru { "Новая карточка" } else { "New card" },
                                Some("5"),
                            );
                            state.selected_node_id = Some(id.clone());
                            state.editing_node_id = Some(id);
                            dirty = true;
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::FilePlus, s.canvas_add_file, false).clicked() {
                            state.file_picker_open = true;
                            state.file_picker_world = Some(wp);
                            state.file_picker_query.clear();
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Image, s.canvas_add_image, false).clicked() {
                            actions.push(UiAction::PickImageForCanvas);
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Copy, s.canvas_paste_image, false).clicked() {
                            actions.push(UiAction::PasteClipboardImage);
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Group, s.canvas_add_group, false).clicked() {
                            state.push_undo();
                            let id = state.doc.add_group_node(
                                snap(wp.x - 160.0, state.snap_to_grid),
                                snap(wp.y - 110.0, state.snap_to_grid),
                                320.0,
                                220.0,
                                if lang == Lang::Ru { "Секция" } else { "Section" },
                                Some("6"),
                            );
                            state.selected_node_id = Some(id);
                            dirty = true;
                            close = true;
                        }
                        hairline_h(ui, p);
                        if context_menu_row(ui, p, Icon::Sparkles, s.canvas_tidy, false).clicked() {
                            if state.auto_layout() {
                                dirty = true;
                            }
                            close = true;
                        }
                        if context_menu_row(ui, p, Icon::Maximize, s.canvas_fit_all, false).clicked() {
                            state.needs_fit = true;
                            close = true;
                        }
                    }
                });
        });

    ctx.data_mut(|d| d.insert_temp(egui::Id::new("c_hud_block_ctx"), area_resp.response.rect));
    if close {
        state.context_menu_pos = None;
    }
    dirty
}

fn render_file_picker_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CanvasEditorState,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    fp_gate_id: egui::Id,
) -> bool {
    let s = strings(lang);
    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    egui::Area::new(egui::Id::new("canvas_file_picker_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha as u8),
            );
            if r.clicked() {
                state.file_picker_open = false;
            }
        });

    let mut inserted = false;
    let top_y = ((screen.height() - 420.0) * 0.5).clamp(24.0, 96.0);
    egui::Area::new(egui::Id::new("canvas_file_picker_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, top_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(16.0))
                .show(ui, |ui| {
                    let mw = 400.0_f32.min((screen.width() - 36.0).max(240.0));
                    ui.set_width(mw);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(s.canvas_file_picker_title)
                                .font(font_semibold(14.5))
                                .color(p.text_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                                state.file_picker_open = false;
                            }
                        });
                    });
                    ui.add_space(8.0);
                    let r = styled_text_input(
                        ui,
                        p,
                        &mut state.file_picker_query,
                        s.search_placeholder,
                        false,
                        f32::INFINITY,
                    );
                    focus_once(ui.ctx(), fp_gate_id, true, r.id);
                    ui.add_space(8.0);
                    hairline_h(ui, p);
                    ui.add_space(6.0);

                    let q = state.file_picker_query.trim().to_lowercase();
                    egui::ScrollArea::vertical()
                        .id_salt("canvas_file_picker_scroll")
                        .auto_shrink([false, true])
                        .max_height(260.0)
                        .show(ui, |ui| {
                            let mut items: Vec<(String, String, Icon)> = Vec::new();
                            for n in notes {
                                items.push((n.path.clone(), n.title.clone(), Icon::Document));
                            }
                            for c in collections {
                                if c.kind != VaultFileKind::Canvas {
                                    let ic = match c.kind {
                                        VaultFileKind::Table => Icon::Table,
                                        _ => Icon::Board,
                                    };
                                    items.push((c.path.clone(), c.title.clone(), ic));
                                }
                            }
                            for (path, title, icon) in items {
                                if !q.is_empty()
                                    && !title.to_lowercase().contains(&q)
                                    && !path.to_lowercase().contains(&q)
                                {
                                    continue;
                                }
                                let (row_r, row_resp) =
                                    ui.allocate_exact_size(vec2(mw, 32.0), Sense::click());
                                if row_resp.hovered() {
                                    ui.painter().rect_filled(row_r, Rounding::same(6.0), p.bg_hover);
                                }
                                draw_icon(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        pos2(row_r.left() + 14.0, row_r.center().y),
                                        vec2(14.0, 14.0),
                                    ),
                                    icon,
                                    p.accent_text,
                                );
                                ui.painter().text(
                                    pos2(row_r.left() + 30.0, row_r.center().y),
                                    Align2::LEFT_CENTER,
                                    &title,
                                    font_medium(12.5),
                                    p.text_primary,
                                );
                                if row_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    state.push_undo();
                                    let center = state.file_picker_world.unwrap_or_else(|| state.viewport.pan.to_pos2());
                                    let id = state.doc.add_file_node(
                                        snap(center.x - 120.0, state.snap_to_grid),
                                        snap(center.y - 56.0, state.snap_to_grid),
                                        &path,
                                        Some("2"),
                                    );
                                    state.selected_node_id = Some(id);
                                    state.file_picker_open = false;
                                    state.file_picker_world = None;
                                    inserted = true;
                                }
                            }
                        });
                });
        });

    inserted
}
