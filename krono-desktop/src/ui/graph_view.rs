use crate::i18n::{status_label, strings, Lang};
use crate::storage::index::{CollectionMeta, RawLinkEdge};
use crate::ui::components::{
    icon_button, primary_button, secondary_button, section_heading, segmented_control,
    styled_text_input, toggle_switch,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::viewport_2d::{draw_dot_grid, render_zoom_hud, Viewport2D};
use crate::ui::{LayoutClass, UiAction};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Margin, Pos2, Rect, Rounding, Sense, Stroke,
    Ui, Vec2,
};
use krono_core::{
    collection::ColorToken,
    note::{NoteMeta, NoteStatus},
    path::{vault_file_stem, VaultFileKind},
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GraphColorMode {
    #[default]
    ByStatus,
    ByTag,
}

#[derive(Clone, Debug)]
pub struct GraphNode {
    pub path: String,
    pub title: String,
    pub kind: VaultFileKind,
    pub status: Option<NoteStatus>,
    pub tags: Vec<String>,
    pub pos: Pos2,
    pub vel: Vec2,
    pub degree: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub a: usize,
    pub b: usize,
}

pub struct GraphState {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub energy: f32,
    pub viewport: Viewport2D,
    pub needs_fit: bool,
    pub dragging_idx: Option<usize>,
    pub selected_path: Option<String>,
    pub hovered_idx: Option<usize>,
    pub filter_query: String,
    pub status_filter: Option<NoteStatus>,
    pub tag_filter: Option<String>,
    pub show_orphans: bool,
    pub show_collections: bool,
    pub color_mode: GraphColorMode,
    pub filters_open: Option<bool>,
    fingerprint: u64,
}

impl Default for GraphState {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            energy: 0.0,
            viewport: Viewport2D::default(),
            needs_fit: true,
            dragging_idx: None,
            selected_path: None,
            hovered_idx: None,
            filter_query: String::new(),
            status_filter: None,
            tag_filter: None,
            show_orphans: true,
            show_collections: true,
            color_mode: GraphColorMode::ByStatus,
            filters_open: None,
            fingerprint: 0,
        }
    }
}

fn data_fingerprint(
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    links: &[RawLinkEdge],
) -> u64 {
    let mut h: u64 = 14695981039346656037;
    let mut mix = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(1099511628211);
        }
    };
    for n in notes {
        mix(n.path.as_bytes());
        mix(n.title.as_bytes());
        mix(n.status.map(|s| s.as_yaml_str()).unwrap_or("").as_bytes());
        for t in &n.tags {
            mix(t.as_bytes());
        }
    }
    for c in collections {
        mix(c.path.as_bytes());
        mix(c.title.as_bytes());
    }
    for e in links {
        mix(e.source_path.as_bytes());
        mix(e.target_raw.as_bytes());
    }
    h
}

fn golden_seed_pos(path: &str, idx: usize) -> Pos2 {
    let mut h: u32 = 2166136261;
    for b in path.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16777619);
    }
    let jitter = ((h % 1000) as f32) / 1000.0;
    let angle = (idx as f32) * 2.399_963_2 + jitter * 0.65;
    let radius = 78.0 + 58.0 * ((idx + 1) as f32).sqrt();
    pos2(angle.cos() * radius, angle.sin() * radius)
}

fn build_lookup_table(nodes: &[GraphNode]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (idx, n) in nodes.iter().enumerate() {
        let lower_path = n.path.to_lowercase();
        map.entry(lower_path.clone()).or_insert(idx);
        if let Some(no_ext) = lower_path.strip_suffix(".md") {
            map.entry(no_ext.to_string()).or_insert(idx);
        }
        let stem = vault_file_stem(&n.path).to_lowercase();
        if !stem.is_empty() {
            map.entry(stem).or_insert(idx);
        }
        let title_low = n.title.trim().to_lowercase();
        if !title_low.is_empty() {
            map.entry(title_low).or_insert(idx);
        }
    }
    map
}

impl GraphState {
    pub fn sync_if_needed(
        &mut self,
        notes: &[NoteMeta],
        collections: &[CollectionMeta],
        links: &[RawLinkEdge],
    ) {
        let fp = data_fingerprint(notes, collections, links);
        if fp != self.fingerprint || (self.nodes.is_empty() && (!notes.is_empty() || !collections.is_empty())) {
            self.rebuild(notes, collections, links);
            self.fingerprint = fp;
        }
    }

    pub fn rebuild(
        &mut self,
        notes: &[NoteMeta],
        collections: &[CollectionMeta],
        links: &[RawLinkEdge],
    ) {
        let old_positions: HashMap<String, Pos2> =
            self.nodes.iter().map(|n| (n.path.clone(), n.pos)).collect();
        let had_existing = !old_positions.is_empty();

        let mut nodes = Vec::with_capacity(notes.len() + collections.len());
        for (i, n) in notes.iter().enumerate() {
            let pos = old_positions
                .get(&n.path)
                .copied()
                .unwrap_or_else(|| golden_seed_pos(&n.path, i));
            nodes.push(GraphNode {
                path: n.path.clone(),
                title: n.title.clone(),
                kind: VaultFileKind::Markdown,
                status: n.status,
                tags: n.tags.clone(),
                pos,
                vel: Vec2::ZERO,
                degree: 0,
            });
        }
        let base_idx = nodes.len();
        for (j, c) in collections.iter().enumerate() {
            let pos = old_positions
                .get(&c.path)
                .copied()
                .unwrap_or_else(|| golden_seed_pos(&c.path, base_idx + j));
            nodes.push(GraphNode {
                path: c.path.clone(),
                title: c.title.clone(),
                kind: c.kind,
                status: None,
                tags: Vec::new(),
                pos,
                vel: Vec2::ZERO,
                degree: 0,
            });
        }

        let lookup = build_lookup_table(&nodes);
        let mut edge_set = HashSet::new();
        let mut edges = Vec::new();
        for raw in links {
            let Some(&src) = lookup.get(&raw.source_path.to_lowercase()) else {
                continue;
            };
            let target_key = raw.target_raw.trim().to_lowercase();
            let Some(&dst) = lookup.get(&target_key) else {
                continue;
            };
            if src == dst {
                continue;
            }
            let (a, b) = if src < dst { (src, dst) } else { (dst, src) };
            if edge_set.insert((a, b)) {
                edges.push(GraphEdge { a, b });
                nodes[a].degree += 1;
                nodes[b].degree += 1;
            }
        }

        self.nodes = nodes;
        self.edges = edges;
        self.dragging_idx = None;
        self.hovered_idx = None;

        if !had_existing {
            self.energy = 1.0;
            self.step_warm_up(140);
            self.needs_fit = true;
        } else {
            self.energy = 0.45;
        }
    }

    pub fn reheat(&mut self) {
        for (i, n) in self.nodes.iter_mut().enumerate() {
            n.pos = golden_seed_pos(&n.path, i);
            n.vel = Vec2::ZERO;
        }
        self.energy = 1.0;
        self.step_warm_up(100);
        self.needs_fit = true;
    }

    pub fn step_warm_up(&mut self, ticks: usize) {
        for _ in 0..ticks {
            self.step_physics(0.016);
        }
    }

    pub fn step_physics(&mut self, dt: f32) -> bool {
        let n = self.nodes.len();
        if n == 0 || (self.energy <= 0.008 && self.dragging_idx.is_none()) {
            self.energy = 0.0;
            return false;
        }

        let dt = dt.clamp(0.004, 0.032);
        let k_rep = 145_000.0_f32;
        let k_att = 0.055_f32;
        let rest_len = 228.0_f32;
        let k_grav = 0.009_f32;
        let damp = 0.84_f32;

        let mut forces = vec![Vec2::ZERO; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let mut diff = self.nodes[i].pos - self.nodes[j].pos;
                let mut d2 = diff.length_sq();
                if d2 < 1.0 {
                    diff = vec2(((i % 7) as f32 - 3.0) * 0.4 + 0.5, ((j % 5) as f32 - 2.0) * 0.4 + 0.3);
                    d2 = diff.length_sq().max(1.0);
                }
                let d = d2.sqrt().max(24.0);
                let mag = k_rep / (d * d);
                let f = (diff / d) * mag;
                forces[i] += f;
                forces[j] -= f;
            }
        }

        for edge in &self.edges {
            let diff = self.nodes[edge.b].pos - self.nodes[edge.a].pos;
            let dist = diff.length().max(1.0);
            let stretch = dist - rest_len;
            let f = (diff / dist) * (stretch * k_att * 60.0);
            forces[edge.a] += f;
            forces[edge.b] -= f;
        }

        for i in 0..n {
            forces[i] -= self.nodes[i].pos.to_vec2() * (k_grav * 60.0);
        }

        let sim_gain = self.energy.clamp(0.18, 1.0);
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if self.dragging_idx == Some(i) {
                node.vel = Vec2::ZERO;
                continue;
            }
            node.vel = (node.vel + forces[i] * dt) * damp;
            let speed = node.vel.length();
            if speed > 32.0 {
                node.vel *= 32.0 / speed;
            }
            node.pos += node.vel * (sim_gain * (dt * 60.0));
        }

        if self.dragging_idx.is_none() {
            self.energy *= 0.984;
            if self.energy <= 0.008 {
                self.energy = 0.0;
            }
        }
        true
    }

    pub fn visible_mask(&self) -> Vec<bool> {
        let q = self.filter_query.trim().to_lowercase();
        let mut base: Vec<bool> = self
            .nodes
            .iter()
            .map(|n| {
                if !self.show_collections && n.kind != VaultFileKind::Markdown {
                    return false;
                }
                if let Some(st) = self.status_filter {
                    if n.kind != VaultFileKind::Markdown || n.status != Some(st) {
                        return false;
                    }
                }
                if let Some(tag) = &self.tag_filter {
                    if !n.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        return false;
                    }
                }
                if !q.is_empty() {
                    let hay = format!(
                        "{} {} {}",
                        n.title.to_lowercase(),
                        n.path.to_lowercase(),
                        n.tags.join(" ").to_lowercase()
                    );
                    if !hay.contains(&q) {
                        return false;
                    }
                }
                true
            })
            .collect();

        if !self.show_orphans {
            let mut has_visible_edge = vec![false; self.nodes.len()];
            for e in &self.edges {
                if base[e.a] && base[e.b] {
                    has_visible_edge[e.a] = true;
                    has_visible_edge[e.b] = true;
                }
            }
            for i in 0..base.len() {
                if !has_visible_edge[i] {
                    base[i] = false;
                }
            }
        }
        base
    }

    pub fn bounds(&self, mask: &[bool]) -> Option<Rect> {
        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut any = false;
        for (i, n) in self.nodes.iter().enumerate() {
            if mask.get(i).copied().unwrap_or(false) {
                any = true;
                min_x = min_x.min(n.pos.x - 95.0);
                min_y = min_y.min(n.pos.y - 68.0);
                max_x = max_x.max(n.pos.x + 95.0);
                max_y = max_y.max(n.pos.y + 68.0);
            }
        }
        if any {
            Some(Rect::from_min_max(pos2(min_x, min_y), pos2(max_x, max_y)))
        } else {
            None
        }
    }

    pub fn focus_node(&mut self, path: &str, canvas_rect: Rect) {
        self.selected_path = Some(path.to_string());
        if let Some(n) = self.nodes.iter().find(|n| n.path == path) {
            self.viewport.target_pan = n.pos.to_vec2();
            self.viewport.target_zoom = 1.15;
            if canvas_rect.width() > 40.0 {
                self.needs_fit = false;
            }
        }
    }
}

fn node_color(p: &ThemePalette, node: &GraphNode, mode: GraphColorMode) -> Color32 {
    match node.kind {
        VaultFileKind::Canvas => p.token_colors(ColorToken::Violet).1,
        VaultFileKind::Board => p.token_colors(ColorToken::Blue).1,
        VaultFileKind::Table => p.token_colors(ColorToken::Teal).1,
        VaultFileKind::Asset => p.text_muted,
        VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => match mode {
            GraphColorMode::ByStatus => match node.status {
                Some(st) => p.status_colors(st).1,
                None => p.text_muted,
            },
            GraphColorMode::ByTag => {
                if let Some(first_tag) = node.tags.first() {
                    p.tag_colors(first_tag).1
                } else {
                    p.text_muted
                }
            }
        },
    }
}

fn node_world_radius(degree: usize) -> f32 {
    (7.0 + (degree as f32).sqrt() * 3.4).min(21.0)
}

fn open_action_for_node(path: &str, kind: VaultFileKind) -> UiAction {
    match kind {
        VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => {
            UiAction::OpenNote(path.to_string())
        }
        VaultFileKind::Board | VaultFileKind::Table => UiAction::OpenCollection(path.to_string()),
        VaultFileKind::Canvas => UiAction::OpenCanvas(path.to_string()),
        VaultFileKind::Asset => UiAction::OpenImageLightbox(path.to_string()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_graph_view(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    links: &[RawLinkEdge],
    state: &mut GraphState,
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();
    state.sync_if_needed(notes, collections, links);

    let canvas_rect = ui.available_rect_before_wrap();
    if canvas_rect.width() < 20.0 || canvas_rect.height() < 20.0 {
        return actions;
    }

    let filters_open = *state.filters_open.get_or_insert(!layout.is_mobile());
    let mask = state.visible_mask();
    let fit_bounds = state.bounds(&mask);
    if state.needs_fit {
        if let Some(b) = fit_bounds {
            state.viewport.fit_bounds(canvas_rect, b, 58.0, true);
        }
        state.needs_fit = false;
    }

    let dt = ctx.input(|i| i.stable_dt);
    if state.step_physics(dt) {
        ctx.request_repaint();
    }
    state.viewport.step_animation(ctx);

    // 1. Reserve background FIRST so nodes & HUD sit above it
    let bg_resp = ui.interact(
        canvas_rect,
        ui.make_persistent_id("krono_graph_bg"),
        Sense::click_and_drag(),
    );

    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, Rounding::ZERO, p.bg_canvas);
    draw_dot_grid(
        &painter,
        canvas_rect,
        &state.viewport,
        32.0,
        with_alpha(p.border_hairline, 0.72),
    );

    // 2. Hit-test & drag individual graph nodes before background pan (ignoring HUD rects)
    let zoom_hud_block = Rect::from_min_max(
        pos2(canvas_rect.right() - 176.0, canvas_rect.bottom() - 54.0),
        canvas_rect.max,
    );
    let prev_filter_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("g_hud_block_filter")));
    let prev_card_block = if state.selected_path.is_some() {
        ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("g_hud_block_card")))
    } else {
        None
    };
    let pointer_blocked_by_hud = |mp: Pos2| -> bool {
        zoom_hud_block.contains(mp)
            || prev_filter_block.is_some_and(|r| r.contains(mp))
            || prev_card_block.is_some_and(|r| r.contains(mp))
    };

    let raw_pointer_pos = ctx.pointer_hover_pos();
    let pointer_pos =
        raw_pointer_pos.filter(|mp| canvas_rect.contains(*mp) && !pointer_blocked_by_hud(*mp));
    let primary_pressed = pointer_pos.is_some() && ctx.input(|i| i.pointer.primary_pressed());
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_clicked = pointer_pos.is_some() && bg_resp.clicked();
    let double_clicked = pointer_pos.is_some() && bg_resp.double_clicked();

    let mut hovered: Option<usize> = None;
    if let Some(mp) = pointer_pos {
        let mut best_d = f32::INFINITY;
        for (i, node) in state.nodes.iter().enumerate() {
            if !mask[i] {
                continue;
            }
            let sp = state.viewport.world_to_screen(canvas_rect, node.pos);
            let sr = (node_world_radius(node.degree) * state.viewport.zoom).max(9.0) + 5.0;
            let d = sp.distance(mp);
            if d <= sr && d < best_d {
                best_d = d;
                hovered = Some(i);
            }
        }
    }
    state.hovered_idx = hovered;

    if primary_pressed {
        if let Some(hi) = hovered {
            state.dragging_idx = Some(hi);
            state.selected_path = Some(state.nodes[hi].path.clone());
            state.energy = state.energy.max(0.35);
        }
    }
    if !primary_down {
        state.dragging_idx = None;
    }

    if let Some(di) = state.dragging_idx {
        if let Some(mp) = raw_pointer_pos {
            let world_p = state.viewport.screen_to_world(canvas_rect, mp);
            state.nodes[di].pos = world_p;
            state.nodes[di].vel = Vec2::ZERO;
            state.energy = state.energy.max(0.28);
            ctx.request_repaint();
        }
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    } else {
        if hovered.is_some() {
            ctx.set_cursor_icon(CursorIcon::PointingHand);
        }
        state
            .viewport
            .handle_pan_zoom(ui, canvas_rect, &bg_resp, hovered.is_none(), true);
        if hovered.is_none() && primary_clicked {
            state.selected_path = None;
        }
    }

    if double_clicked {
        if let Some(hi) = hovered {
            let n = &state.nodes[hi];
            actions.push(open_action_for_node(&n.path, n.kind));
        }
    }

    let selected_idx = state.selected_path.as_deref().and_then(|sp| {
        state
            .nodes
            .iter()
            .enumerate()
            .position(|(i, n)| mask[i] && n.path == sp)
    });

    let active_focus = hovered.or(selected_idx);
    let mut neighbor_set = HashSet::new();
    if let Some(fi) = active_focus {
        neighbor_set.insert(fi);
        for e in &state.edges {
            if !mask[e.a] || !mask[e.b] {
                continue;
            }
            if e.a == fi {
                neighbor_set.insert(e.b);
            } else if e.b == fi {
                neighbor_set.insert(e.a);
            }
        }
    }

    // 3. Draw Edges & Nodes
    let visible_edges = state
        .edges
        .iter()
        .filter(|e| mask[e.a] && mask[e.b])
        .count();
    let mut vis_notes = 0usize;
    let mut vis_boards = 0usize;
    let mut vis_tables = 0usize;
    let mut vis_canvases = 0usize;
    for (idx, node) in state.nodes.iter().enumerate() {
        if !mask[idx] {
            continue;
        }
        match node.kind {
            VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => vis_notes += 1,
            VaultFileKind::Board => vis_boards += 1,
            VaultFileKind::Table => vis_tables += 1,
            VaultFileKind::Canvas => vis_canvases += 1,
            VaultFileKind::Asset => {}
        }
    }
    let visible_nodes = vis_notes + vis_boards + vis_tables + vis_canvases;
    let stats_lbl = format_graph_stats_label(
        lang,
        vis_notes,
        vis_boards,
        vis_tables,
        vis_canvases,
        visible_edges,
    );

    for edge in &state.edges {
        if !mask[edge.a] || !mask[edge.b] {
            continue;
        }
        let pa = state
            .viewport
            .world_to_screen(canvas_rect, state.nodes[edge.a].pos);
        let pb = state
            .viewport
            .world_to_screen(canvas_rect, state.nodes[edge.b].pos);
        if !canvas_rect.expand(40.0).intersects(Rect::from_two_pos(pa, pb)) {
            continue;
        }
        let is_focus_edge = active_focus.is_some_and(|fi| edge.a == fi || edge.b == fi);
        let col = if active_focus.is_none() {
            with_alpha(p.border_subtle, 0.72)
        } else if is_focus_edge {
            with_alpha(p.accent, 0.85)
        } else {
            with_alpha(p.border_hairline, 0.22)
        };
        let sw = if is_focus_edge { 1.7_f32 } else { 1.05_f32 };
        painter.line_segment([pa, pb], Stroke::new(sw, col));
    }

    if visible_nodes == 0 {
        painter.text(
            canvas_rect.center(),
            Align2::CENTER_CENTER,
            s.graph_empty,
            font_body(13.5),
            p.text_muted,
        );
    } else {
        for (i, node) in state.nodes.iter().enumerate() {
            if !mask[i] {
                continue;
            }
            let sp = state.viewport.world_to_screen(canvas_rect, node.pos);
            let r = (node_world_radius(node.degree) * state.viewport.zoom).clamp(4.0, 28.0);
            if !canvas_rect.expand(r + 60.0).contains(sp) {
                continue;
            }

            let is_focus = active_focus == Some(i);
            let is_neighbor = active_focus.is_none() || neighbor_set.contains(&i);
            let alpha = if is_neighbor { 1.0 } else { 0.22 };
            let base_col = node_color(p, node, state.color_mode);

            if is_focus {
                painter.circle_filled(sp, r + 5.5, with_alpha(p.accent, 0.24));
            }
            painter.circle_filled(sp, r, with_alpha(base_col, alpha));
            painter.circle_stroke(
                sp,
                r,
                Stroke::new(
                    if is_focus { 2.0_f32 } else { 1.0_f32 },
                    if is_focus {
                        p.accent_text
                    } else {
                        with_alpha(p.bg_canvas, alpha)
                    },
                ),
            );

            let show_label = is_focus
                || (active_focus.is_some() && neighbor_set.contains(&i))
                || state.viewport.zoom >= 0.65
                || node.degree >= 2;
            if show_label {
                let lbl_col = if is_focus {
                    p.text_primary
                } else {
                    with_alpha(p.text_secondary, alpha)
                };
                let font = if is_focus {
                    font_semibold(12.0)
                } else {
                    font_body((11.0 * state.viewport.zoom.clamp(0.88, 1.15)).min(12.5))
                };
                let galley = painter.layout_no_wrap(node.title.clone(), font, lbl_col);
                let lpos = pos2(sp.x - galley.size().x * 0.5, sp.y + r + 4.0);
                painter.rect_filled(
                    Rect::from_min_size(
                        pos2(lpos.x - 4.0, lpos.y - 1.5),
                        vec2(galley.size().x + 8.0, galley.size().y + 3.0),
                    ),
                    Rounding::same(4.0),
                    with_alpha(p.bg_canvas, 0.78 * alpha),
                );
                painter.galley(lpos, galley, lbl_col);
            }
        }
    }

    // 4. Floating Top-Left Filter Card (child UI inside canvas_rect)
    let panel_w = if filters_open {
        232.0_f32.min((canvas_rect.width() - 28.0).max(180.0))
    } else {
        118.0
    };
    let panel_max_h = (canvas_rect.height() - 100.0).max(60.0);
    let filter_rect = Rect::from_min_size(
        pos2(canvas_rect.left() + 14.0, canvas_rect.top() + 14.0),
        vec2(panel_w, panel_max_h),
    );
    let mut filter_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(filter_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    filter_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));

    let filter_frame_resp = if !filters_open {
        egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(8.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::symmetric(8.0, 5.0))
            .show(&mut filter_ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if icon_button(ui, p, Icon::Filter, s.graph_title, false).clicked() {
                        state.filters_open = Some(true);
                    }
                    ui.label(
                        egui::RichText::new(&stats_lbl)
                            .font(font_mono(11.0))
                            .color(p.text_secondary),
                    );
                });
            })
    } else {
        egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(10.0))
            .shadow(p.shadow_elevated)
            .inner_margin(Margin::same(11.0))
            .show(&mut filter_ui, |ui| {
                ui.set_width(panel_w - 22.0);
                ui.horizontal(|ui| {
                    let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    draw_icon(ui.painter(), ir, Icon::Graph, p.accent_text);
                    ui.label(
                        egui::RichText::new(s.graph_title)
                            .font(font_semibold(13.0))
                            .color(p.text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                            state.filters_open = Some(false);
                        }
                    });
                });
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(&stats_lbl)
                        .font(font_mono(10.5))
                        .color(p.text_muted),
                );
                ui.add_space(6.0);
                styled_text_input(
                    ui,
                    p,
                    &mut state.filter_query,
                    s.graph_search_hint,
                    false,
                    f32::INFINITY,
                );
                ui.add_space(7.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(s.graph_filter_orphans)
                            .font(font_body(12.0))
                            .color(p.text_secondary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        toggle_switch(ui, p, "graph_orphans_sw", &mut state.show_orphans);
                    });
                });
                ui.add_space(6.0);

                let mut mode = state.color_mode;
                if segmented_control(
                    ui,
                    p,
                    "graph_color_mode_seg",
                    &mut mode,
                    &[
                        (GraphColorMode::ByStatus, None, s.graph_color_status),
                        (GraphColorMode::ByTag, None, s.graph_color_tag),
                    ],
                ) {
                    state.color_mode = mode;
                }

                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                    for st in [NoteStatus::Todo, NoteStatus::InProgress, NoteStatus::Done] {
                        let active = state.status_filter == Some(st);
                        let (bg, dot, fg) = p.status_colors(st);
                        let lbl = status_label(lang, st);
                        let g = ui.painter().layout_no_wrap(lbl.to_string(), font_medium(10.5), fg);
                        let (sr, sresp) =
                            ui.allocate_exact_size(vec2(g.size().x + 20.0, 21.0), Sense::click());
                        ui.painter().rect_filled(
                            sr,
                            Rounding::same(5.0),
                            if active { bg } else { p.bg_input },
                        );
                        if active {
                            ui.painter()
                                .rect_stroke(sr, Rounding::same(5.0), Stroke::new(1.0_f32, dot));
                        }
                        ui.painter()
                            .circle_filled(pos2(sr.left() + 8.0, sr.center().y), 3.0, dot);
                        ui.painter().galley(
                            pos2(sr.left() + 14.0, sr.center().y - g.size().y * 0.5),
                            g,
                            if active { fg } else { p.text_secondary },
                        );
                        if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            state.status_filter = if active { None } else { Some(st) };
                        }
                    }
                });

                ui.add_space(7.0);
                if secondary_button(ui, p, Some(Icon::RotateCcw), s.graph_reheat).clicked() {
                    state.reheat();
                    ctx.request_repaint();
                }
            })
    };
    ctx.data_mut(|d| {
        d.insert_temp(
            egui::Id::new("g_hud_block_filter"),
            filter_frame_resp.response.rect,
        );
    });

    // 5. Floating Bottom-Left Selected Node Preview Card
    if let Some(sel_path) = state.selected_path.clone() {
        if let Some(sel_node) = state.nodes.iter().find(|n| n.path == sel_path).cloned() {
            let card_w = 248.0_f32.min((canvas_rect.width() - 184.0).max(168.0));
            let card_h = 84.0;
            let card_rect = Rect::from_min_size(
                pos2(
                    canvas_rect.left() + 14.0,
                    (canvas_rect.bottom() - card_h - 14.0).max(canvas_rect.top() + 14.0),
                ),
                vec2(card_w, card_h),
            );
            let mut card_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(card_rect)
                    .layout(egui::Layout::top_down(egui::Align::LEFT)),
            );
            card_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
            let card_resp = egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(10.0))
                .shadow(p.shadow_elevated)
                .inner_margin(Margin::symmetric(12.0, 9.0))
                .show(&mut card_ui, |ui| {
                    ui.set_width(card_w - 24.0);
                    ui.horizontal(|ui| {
                        let cdot = node_color(p, &sel_node, state.color_mode);
                        let (dr, _) = ui.allocate_exact_size(vec2(10.0, 14.0), Sense::hover());
                        ui.painter().circle_filled(dr.center(), 4.5, cdot);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&sel_node.title)
                                    .font(font_semibold(13.0))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                    });
                    ui.add_space(2.0);
                    let stats_txt = if layout.is_mobile() {
                        format!("{} связ.", sel_node.degree)
                    } else {
                        format!("{} · {}", sel_node.degree, s.inspector_backlinks)
                    };
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if primary_button(
                                ui,
                                p,
                                Some(Icon::OutLink),
                                s.graph_open_node,
                                false,
                            )
                            .clicked()
                            {
                                actions.push(open_action_for_node(&sel_node.path, sel_node.kind));
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(stats_txt)
                                            .font(font_body(11.0))
                                            .color(p.text_muted),
                                    )
                                    .truncate(),
                                );
                            });
                        });
                    });
                });
            ctx.data_mut(|d| {
                d.insert_temp(egui::Id::new("g_hud_block_card"), card_resp.response.rect);
            });
        }
    }

    // 6. Floating Bottom-Right Zoom HUD
    render_zoom_hud(ui, canvas_rect, p, lang, &mut state.viewport, fit_bounds);

    actions
}

// ---------------------------------------------------------------------------
// Local Mini-Graph (Inspector)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct LocalGraphNode {
    pub path: String,
    pub title: String,
    pub kind: VaultFileKind,
    pub hop: u8,
    pub unit_pos: Vec2,
}

#[derive(Clone, Debug, Default)]
pub struct LocalGraphData {
    pub center_path: String,
    pub nodes: Vec<LocalGraphNode>,
    pub edges: Vec<(usize, usize)>,
}

pub fn build_local_graph(
    center_path: &str,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    links: &[RawLinkEdge],
    depth: usize,
) -> LocalGraphData {
    let mut temp_nodes = Vec::with_capacity(notes.len() + collections.len());
    for n in notes {
        temp_nodes.push(GraphNode {
            path: n.path.clone(),
            title: n.title.clone(),
            kind: VaultFileKind::Markdown,
            status: n.status,
            tags: Vec::new(),
            pos: Pos2::ZERO,
            vel: Vec2::ZERO,
            degree: 0,
        });
    }
    for c in collections {
        temp_nodes.push(GraphNode {
            path: c.path.clone(),
            title: c.title.clone(),
            kind: c.kind,
            status: None,
            tags: Vec::new(),
            pos: Pos2::ZERO,
            vel: Vec2::ZERO,
            degree: 0,
        });
    }

    let lookup = build_lookup_table(&temp_nodes);
    let Some(&center_idx) = lookup.get(&center_path.to_lowercase()) else {
        return LocalGraphData {
            center_path: center_path.to_string(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };
    };

    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for raw in links {
        let Some(&src) = lookup.get(&raw.source_path.to_lowercase()) else {
            continue;
        };
        let Some(&dst) = lookup.get(&raw.target_raw.trim().to_lowercase()) else {
            continue;
        };
        if src == dst {
            continue;
        }
        adj.entry(src).or_default().push(dst);
        adj.entry(dst).or_default().push(src);
    }

    let mut hop1 = Vec::new();
    let mut seen: HashSet<usize> = HashSet::from([center_idx]);
    if let Some(neigh) = adj.get(&center_idx) {
        for &n in neigh {
            if seen.insert(n) && hop1.len() < 12 {
                hop1.push(n);
            }
        }
    }

    let mut hop2: Vec<(usize, usize)> = Vec::new(); // (node_idx, parent_pos_in_hop1)
    if depth >= 2 {
        for (p_pos, &h1) in hop1.iter().enumerate() {
            if let Some(neigh) = adj.get(&h1) {
                for &n in neigh {
                    if seen.insert(n) && hop1.len() + hop2.len() < 22 {
                        hop2.push((n, p_pos));
                    }
                }
            }
        }
    }

    let mut out_nodes = Vec::new();
    let mut global_to_local = HashMap::new();

    global_to_local.insert(center_idx, 0usize);
    out_nodes.push(LocalGraphNode {
        path: temp_nodes[center_idx].path.clone(),
        title: temp_nodes[center_idx].title.clone(),
        kind: temp_nodes[center_idx].kind,
        hop: 0,
        unit_pos: Vec2::ZERO,
    });

    let h1_count = hop1.len().max(1);
    for (idx, &gi) in hop1.iter().enumerate() {
        let angle = (idx as f32) * std::f32::consts::TAU / (h1_count as f32) - 0.5;
        let radius = if depth >= 2 { 0.48 } else { 0.68 };
        let local_idx = out_nodes.len();
        global_to_local.insert(gi, local_idx);
        out_nodes.push(LocalGraphNode {
            path: temp_nodes[gi].path.clone(),
            title: temp_nodes[gi].title.clone(),
            kind: temp_nodes[gi].kind,
            hop: 1,
            unit_pos: vec2(angle.cos() * radius, angle.sin() * radius),
        });
    }

    for (k, &(gi, parent_h1_idx)) in hop2.iter().enumerate() {
        let parent_angle = (parent_h1_idx as f32) * std::f32::consts::TAU / (h1_count as f32) - 0.5;
        let jitter = (((k % 3) as f32) - 1.0) * 0.28;
        let angle = parent_angle + jitter;
        let local_idx = out_nodes.len();
        global_to_local.insert(gi, local_idx);
        out_nodes.push(LocalGraphNode {
            path: temp_nodes[gi].path.clone(),
            title: temp_nodes[gi].title.clone(),
            kind: temp_nodes[gi].kind,
            hop: 2,
            unit_pos: vec2(angle.cos() * 0.84, angle.sin() * 0.84),
        });
    }

    let mut edge_set = HashSet::new();
    let mut out_edges = Vec::new();
    for (&gi, &li) in &global_to_local {
        if let Some(neigh) = adj.get(&gi) {
            for &gn in neigh {
                if let Some(&ln) = global_to_local.get(&gn) {
                    if li != ln {
                        let pair = if li < ln { (li, ln) } else { (ln, li) };
                        if edge_set.insert(pair) {
                            out_edges.push(pair);
                        }
                    }
                }
            }
        }
    }

    LocalGraphData {
        center_path: center_path.to_string(),
        nodes: out_nodes,
        edges: out_edges,
    }
}

pub fn render_local_graph(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    data: &LocalGraphData,
    depth: &mut usize,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    ui.horizontal(|ui| {
        section_heading(ui, p, s.inspector_local_graph);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(ui, p, Icon::Maximize, s.local_graph_open_full, false).clicked() {
                actions.push(UiAction::OpenGraphForNode(data.center_path.clone()));
            }
            for d in [2usize, 1usize] {
                let active = *depth == d;
                let lbl = if d == 1 { "1" } else { "2" };
                let (rect, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    Rounding::same(5.0),
                    if active { p.accent_soft } else { Color32::TRANSPARENT },
                );
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    lbl,
                    font_mono(11.0),
                    if active { p.accent_text } else { p.text_muted },
                );
                if resp
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(if d == 1 {
                        s.local_graph_depth_1
                    } else {
                        s.local_graph_depth_2
                    })
                    .clicked()
                {
                    *depth = d;
                }
            }
        });
    });
    ui.add_space(6.0);

    let card_w = ui.available_width().max(140.0);
    let card_h = 166.0;
    let (card_rect, card_resp) = ui.allocate_exact_size(vec2(card_w, card_h), Sense::click());
    ui.painter()
        .rect_filled(card_rect, Rounding::same(8.0), p.bg_elevated);
    ui.painter().rect_stroke(
        card_rect,
        Rounding::same(8.0),
        Stroke::new(1.0_f32, p.border_hairline),
    );

    if data.nodes.len() <= 1 {
        let center_pos = card_rect.center();
        ui.painter().circle_filled(center_pos, 9.0, with_alpha(p.accent, 0.25));
        ui.painter().circle_filled(center_pos, 5.5, p.accent);
        ui.painter().text(
            pos2(card_rect.center().x, card_rect.bottom() - 22.0),
            Align2::CENTER_CENTER,
            s.inspector_no_backlinks,
            font_body(11.0),
            p.text_muted,
        );
        return;
    }

    let rx = (card_rect.width() * 0.42).max(30.0);
    let ry = (card_rect.height() * 0.38).max(30.0);
    let node_pos = |idx: usize| -> Pos2 {
        let u = data.nodes[idx].unit_pos;
        pos2(card_rect.center().x + u.x * rx, card_rect.center().y + u.y * ry)
    };

    let hover_pos = ui.ctx().pointer_hover_pos();
    let hovered = hover_pos.and_then(|mp| {
        if !card_rect.contains(mp) {
            return None;
        }
        (0..data.nodes.len()).find(|&i| node_pos(i).distance(mp) <= 11.0)
    });

    let gp = ui.painter().with_clip_rect(card_rect.shrink(1.0));

    for &(a, b) in &data.edges {
        let hl = hovered.is_some_and(|h| h == a || h == b);
        let col = if hl {
            p.accent
        } else {
            with_alpha(p.border_subtle, 0.78)
        };
        gp.line_segment([node_pos(a), node_pos(b)], Stroke::new(if hl { 1.4_f32 } else { 0.9_f32 }, col));
    }

    for (i, n) in data.nodes.iter().enumerate() {
        let sp = node_pos(i);
        let is_center = n.hop == 0;
        let is_hov = hovered == Some(i);
        let r = if is_center {
            6.5
        } else if n.hop == 1 {
            4.8
        } else {
            3.6
        };
        if is_center {
            gp.circle_filled(sp, r + 4.0, with_alpha(p.accent, 0.22));
        } else if is_hov {
            gp.circle_filled(sp, r + 3.5, with_alpha(p.accent_hover, 0.25));
        }
        let fill = if is_center || is_hov {
            p.accent_text
        } else if n.hop == 1 {
            p.text_secondary
        } else {
            p.text_muted
        };
        gp.circle_filled(sp, r, fill);

        if is_center || is_hov || (data.nodes.len() <= 6 && n.hop == 1) {
            let col = if is_hov || is_center {
                p.text_primary
            } else {
                p.text_muted
            };
            let g = gp.layout_no_wrap(n.title.clone(), font_body(10.0), col);
            let min_x = card_rect.left() + 6.0;
            let max_x = (card_rect.right() - g.size().x - 6.0).max(min_x);
            let min_y = card_rect.top() + 3.0;
            let max_y = (card_rect.bottom() - g.size().y - 3.0).max(min_y);
            let lpos = pos2(
                (sp.x - g.size().x * 0.5).clamp(min_x, max_x),
                (sp.y + r + 2.0).clamp(min_y, max_y),
            );
            gp.galley(lpos, g, col);
        }
    }

    if let Some(hi) = hovered {
        let _ = anim_bool_eased(ui.ctx(), card_resp.id.with(("lg_hov", hi)), true, 0.08);
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        if card_resp.clicked() && hi != 0 {
            actions.push(open_action_for_node(&data.nodes[hi].path, data.nodes[hi].kind));
        }
    }
}

fn format_graph_stats_label(
    lang: Lang,
    notes: usize,
    boards: usize,
    tables: usize,
    canvases: usize,
    edges: usize,
) -> String {
    let mut parts = Vec::new();
    match lang {
        Lang::Ru => {
            parts.push(format!("{notes} зам."));
            if boards > 0 {
                parts.push(format!("{boards} доск."));
            }
            if tables > 0 {
                parts.push(format!("{tables} табл."));
            }
            if canvases > 0 {
                parts.push(format!("{canvases} канв."));
            }
            parts.push(format!("{edges} связ."));
        }
        Lang::En => {
            parts.push(format!("{notes} notes"));
            if boards > 0 {
                parts.push(format!("{boards} boards"));
            }
            if tables > 0 {
                parts.push(format!("{tables} tables"));
            }
            if canvases > 0 {
                parts.push(format!("{canvases} canvases"));
            }
            parts.push(format!("{edges} links"));
        }
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_note(path: &str, title: &str, status: NoteStatus, tags: &[&str]) -> NoteMeta {
        NoteMeta {
            path: path.into(),
            title: title.into(),
            status: Some(status),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            excerpt: String::new(),
            modified: 100,
            word_count: 10,
            pinned: false,
            board_order: 0,
        }
    }

    #[test]
    fn resolves_graph_edges_and_filters_orphans_and_tags() {
        let notes = vec![
            sample_note("a.md", "Alpha", NoteStatus::Done, &["rust"]),
            sample_note("b.md", "Beta", NoteStatus::Todo, &["ui"]),
            sample_note("orphan.md", "Lonely Note", NoteStatus::Todo, &["rust"]),
        ];
        let links = vec![RawLinkEdge {
            source_path: "a.md".into(),
            target_raw: "Beta".into(),
        }];
        let mut state = GraphState::default();
        state.rebuild(&notes, &[], &links);
        assert_eq!(state.nodes.len(), 3);
        assert_eq!(state.edges.len(), 1);
        assert_eq!(state.visible_mask(), vec![true, true, true]);

        state.show_orphans = false;
        assert_eq!(state.visible_mask(), vec![true, true, false]);

        state.show_orphans = true;
        state.tag_filter = Some("rust".into());
        assert_eq!(state.visible_mask(), vec![true, false, true]);
    }

    #[test]
    fn extracts_local_graph_depth_1_and_2() {
        let notes = vec![
            sample_note("a.md", "A", NoteStatus::Todo, &[]),
            sample_note("b.md", "B", NoteStatus::Todo, &[]),
            sample_note("c.md", "C", NoteStatus::Todo, &[]),
            sample_note("d.md", "D", NoteStatus::Todo, &[]),
        ];
        let links = vec![
            RawLinkEdge { source_path: "a.md".into(), target_raw: "B".into() },
            RawLinkEdge { source_path: "b.md".into(), target_raw: "C".into() },
        ];
        let g1 = build_local_graph("a.md", &notes, &[], &links, 1);
        assert_eq!(g1.nodes.len(), 2); // A, B
        let g2 = build_local_graph("a.md", &notes, &[], &links, 2);
        assert_eq!(g2.nodes.len(), 3); // A, B, C
        assert!(!g2.nodes.iter().any(|n| n.path == "d.md"));
    }

    #[test]
    fn force_simulation_separates_overlapping_nodes_and_cools_down() {
        let notes = vec![
            sample_note("a.md", "A", NoteStatus::Todo, &[]),
            sample_note("b.md", "B", NoteStatus::Todo, &[]),
        ];
        let mut state = GraphState::default();
        state.rebuild(&notes, &[], &[]);
        state.nodes[0].pos = Pos2::ZERO;
        state.nodes[1].pos = pos2(1.0, 0.0);
        state.energy = 1.0;

        for _ in 0..40 {
            state.step_physics(0.016);
        }
        assert!(state.nodes[0].pos.distance(state.nodes[1].pos) > 30.0);

        for _ in 0..400 {
            state.step_physics(0.016);
        }
        assert_eq!(state.energy, 0.0);
    }

    #[test]
    fn local_graph_render_handles_narrow_card_and_long_titles() {
        let long_title = "Добро пожаловать в Krono — очень длинный заголовок заметки для проверки";
        let notes = vec![
            sample_note("welcome.md", long_title, NoteStatus::InProgress, &[]),
            sample_note("arch.md", "Architecture", NoteStatus::Done, &[]),
            sample_note("road.md", "Дорожная карта", NoteStatus::Todo, &[]),
        ];
        let links = vec![
            RawLinkEdge { source_path: "welcome.md".into(), target_raw: "Architecture".into() },
            RawLinkEdge { source_path: "welcome.md".into(), target_raw: "Дорожная карта".into() },
        ];
        let data = build_local_graph("welcome.md", &notes, &[], &links, 1);

        let ctx = egui::Context::default();
        crate::ui::theme::setup_fonts(&ctx);
        let p = crate::ui::theme::ThemePalette::for_preset(
            crate::config::ThemePreset::Graphite,
            crate::config::AccentPreset::SteelBlue,
        );
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::SidePanel::right("test_narrow_insp")
                .exact_width(96.0)
                .show(ctx, |ui| {
                    let mut depth = 1;
                    let mut actions = Vec::new();
                    render_local_graph(ui, &p, Lang::Ru, &data, &mut depth, &mut actions);
                });
        });
    }
}
