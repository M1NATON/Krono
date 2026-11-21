use crate::i18n::{month_name, strings, Lang};
use crate::storage::index::{CollectionMeta, RawLinkEdge};
use crate::ui::components::{icon_button, secondary_button, styled_text_input};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::viewport_2d::{draw_dot_grid, render_zoom_hud, Viewport2D};
use crate::ui::{LayoutClass, PagesKindFilter, UiAction, ViewMode};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Margin, Pos2, Rect, Rounding, Sense, Shape, Stroke, Ui,
    Vec2,
};
use krono_core::{
    note::NoteMeta,
    path::{vault_file_stem, VaultFileKind},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub struct VaultMapState {
    pub viewport: Viewport2D,
    pub needs_fit: bool,
    pub search_query: String,
    pub kind_filter: PagesKindFilter,
    pub show_links: bool,
    pub folder_offsets: HashMap<String, [f32; 2]>,
    pub dragging_folder: Option<(String, Pos2, Vec2)>,
    pub selected_path: Option<String>,
    pub hovered_path: Option<String>,
    pub layout_loaded_for: Option<PathBuf>,
}

impl Default for VaultMapState {
    fn default() -> Self {
        Self {
            viewport: Viewport2D::default(),
            needs_fit: true,
            search_query: String::new(),
            kind_filter: PagesKindFilter::All,
            show_links: true,
            folder_offsets: HashMap::new(),
            dragging_folder: None,
            selected_path: None,
            hovered_path: None,
            layout_loaded_for: None,
        }
    }
}

impl VaultMapState {
    pub fn load_layout(&mut self, vault_dir: &Path) {
        self.layout_loaded_for = Some(vault_dir.to_path_buf());
        let p = vault_dir.join(".krono").join("vault_map_layout.json");
        if let Ok(raw) = fs::read_to_string(p) {
            if let Ok(parsed) = serde_json::from_str::<HashMap<String, [f32; 2]>>(&raw) {
                self.folder_offsets = parsed;
            }
        }
    }

    pub fn save_layout(&self, vault_dir: &Path) {
        let dir = vault_dir.join(".krono");
        let _ = fs::create_dir_all(&dir);
        if let Ok(json) = serde_json::to_string_pretty(&self.folder_offsets) {
            let _ = fs::write(dir.join("vault_map_layout.json"), json);
        }
    }
}

#[derive(Clone)]
struct MapItem {
    path: String,
    title: String,
    kind: VaultFileKind,
    accent: Color32,
    tags: Vec<String>,
}

fn folder_of_path(rel_path: &str) -> String {
    match rel_path.rsplit_once('/') {
        Some((dir, _)) => dir.to_string(),
        None => String::new(),
    }
}

fn pretty_folder_title(lang: Lang, folder: &str) -> String {
    let s = strings(lang);
    if folder.is_empty() {
        return s.create_folder_root.to_string();
    }
    if let Some(rest) = folder.strip_prefix("daily/boards/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 2 {
            if let Ok(mm) = parts[1].parse::<u32>() {
                return format!(
                    "{} · {} / {}",
                    s.diary_boards_folder,
                    parts[0],
                    month_name(lang, mm)
                );
            }
        }
        return format!("{} / {rest}", s.diary_boards_folder);
    }
    if folder == "daily/boards" {
        return format!("{} · {}", s.diary_section, s.diary_boards_folder);
    }
    if let Some(rest) = folder.strip_prefix("daily/") {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 2 {
            if let Ok(mm) = parts[1].parse::<u32>() {
                return format!(
                    "{} · {} / {:02} · {}",
                    s.diary_section,
                    parts[0],
                    mm,
                    month_name(lang, mm)
                );
            }
        }
        return format!("{} · {rest}", s.diary_section);
    }
    if folder == "daily" {
        return s.diary_section.to_string();
    }
    folder.to_string()
}

fn kind_icon(kind: VaultFileKind) -> Icon {
    match kind {
        VaultFileKind::Markdown => Icon::Document,
        VaultFileKind::EncryptedMarkdown => Icon::Lock,
        VaultFileKind::Board => Icon::Board,
        VaultFileKind::Table => Icon::Table,
        VaultFileKind::Canvas => Icon::Canvas,
        VaultFileKind::Asset => Icon::Image,
    }
}

fn kind_matches_filter(kind: VaultFileKind, filter: PagesKindFilter) -> bool {
    match filter {
        PagesKindFilter::All => true,
        PagesKindFilter::Notes => {
            matches!(kind, VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown)
        }
        PagesKindFilter::Boards => kind == VaultFileKind::Board,
        PagesKindFilter::Tables => kind == VaultFileKind::Table,
        PagesKindFilter::Canvases => kind == VaultFileKind::Canvas,
    }
}

fn open_action_for(kind: VaultFileKind, rel_path: String, actions: &mut Vec<UiAction>) {
    match kind {
        VaultFileKind::Markdown | VaultFileKind::EncryptedMarkdown => {
            actions.push(UiAction::OpenNote(rel_path));
            actions.push(UiAction::SetViewMode(ViewMode::Editor));
        }
        VaultFileKind::Board | VaultFileKind::Table => {
            actions.push(UiAction::OpenCollection(rel_path));
        }
        VaultFileKind::Canvas => {
            actions.push(UiAction::OpenCanvas(rel_path));
        }
        VaultFileKind::Asset => {
            actions.push(UiAction::OpenImageLightbox(rel_path));
        }
    }
}

fn resolve_link_target(target_raw: &str, lookup: &HashMap<String, String>) -> Option<String> {
    let key = target_raw.trim().replace('\\', "/").to_lowercase();
    if let Some(p) = lookup.get(&key) {
        return Some(p.clone());
    }
    let stem = vault_file_stem(&key);
    lookup.get(stem).cloned()
}

#[allow(clippy::too_many_arguments)]
pub fn render_vault_map_view(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    vault_dir: &Path,
    state: &mut VaultMapState,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    all_links: &[RawLinkEdge],
) -> Vec<UiAction> {
    let s = strings(lang);
    let mut actions = Vec::new();

    if state.layout_loaded_for.as_deref() != Some(vault_dir) {
        state.load_layout(vault_dir);
    }

    let canvas_rect = ui.available_rect_before_wrap();
    if canvas_rect.width() < 20.0 || canvas_rect.height() < 20.0 {
        return actions;
    }

    let painter = ui.painter_at(canvas_rect);
    painter.rect_filled(canvas_rect, Rounding::ZERO, p.bg_canvas);
    draw_dot_grid(
        &painter,
        canvas_rect,
        &state.viewport,
        28.0,
        with_alpha(p.border_hairline, 0.72),
    );

    // 1. Collect & group filtered items by folder
    let q_lower = state.search_query.trim().to_lowercase();
    let mut groups: BTreeMap<String, Vec<MapItem>> = BTreeMap::new();
    let mut item_lookup: HashMap<String, String> = HashMap::new();

    for n in notes {
        let stem = vault_file_stem(&n.path).to_lowercase();
        let no_ext = n.path.trim_end_matches(".md").to_lowercase();
        item_lookup.insert(n.path.to_lowercase(), n.path.clone());
        item_lookup.insert(no_ext, n.path.clone());
        item_lookup.insert(stem, n.path.clone());
        if !n.title.trim().is_empty() {
            item_lookup.insert(n.title.trim().to_lowercase(), n.path.clone());
        }
        if !kind_matches_filter(VaultFileKind::Markdown, state.kind_filter) {
            continue;
        }
        if !q_lower.is_empty()
            && !n.title.to_lowercase().contains(&q_lower)
            && !n.path.to_lowercase().contains(&q_lower)
            && !n.tags.iter().any(|t| t.to_lowercase().contains(&q_lower))
        {
            continue;
        }
        let dot = n
            .status
            .map(|st| p.status_colors(st).1)
            .unwrap_or(p.text_muted);
        groups
            .entry(folder_of_path(&n.path))
            .or_default()
            .push(MapItem {
                path: n.path.clone(),
                title: n.title.clone(),
                kind: VaultFileKind::Markdown,
                accent: dot,
                tags: n.tags.clone(),
            });
    }

    for c in collections {
        let stem = vault_file_stem(&c.path).to_lowercase();
        item_lookup.insert(c.path.to_lowercase(), c.path.clone());
        item_lookup.insert(stem, c.path.clone());
        if !c.title.trim().is_empty() {
            item_lookup.insert(c.title.trim().to_lowercase(), c.path.clone());
        }
        if !kind_matches_filter(c.kind, state.kind_filter) {
            continue;
        }
        if !q_lower.is_empty()
            && !c.title.to_lowercase().contains(&q_lower)
            && !c.path.to_lowercase().contains(&q_lower)
        {
            continue;
        }
        let accent = match c.kind {
            VaultFileKind::Board => p.accent,
            VaultFileKind::Table => p.warning,
            VaultFileKind::Canvas => p.success,
            _ => p.text_secondary,
        };
        groups
            .entry(folder_of_path(&c.path))
            .or_default()
            .push(MapItem {
                path: c.path.clone(),
                title: c.title.clone(),
                kind: c.kind,
                accent,
                tags: Vec::new(),
            });
    }

    for items in groups.values_mut() {
        items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    }

    // 2. Auto masonry layout for folder containers + user folder_offsets
    const ITEM_H: f32 = 58.0;
    const ITEM_GAP: f32 = 8.0;
    const HEADER_H: f32 = 34.0;
    const GROUP_PAD: f32 = 12.0;
    const COL_W: f32 = 248.0;
    const COL_GAP: f32 = 44.0;
    const ROW_GAP: f32 = 36.0;

    let col_count = ((groups.len() as f32).sqrt().ceil() as usize).clamp(1, 4);
    let mut col_heights = vec![0.0_f32; col_count];

    struct PlacedCard {
        item: MapItem,
        wrect: Rect,
    }
    struct PlacedGroup {
        folder_key: String,
        title: String,
        wrect: Rect,
        header_wrect: Rect,
        cards: Vec<PlacedCard>,
    }

    let mut placed_groups: Vec<PlacedGroup> = Vec::with_capacity(groups.len());
    let mut card_centers: HashMap<String, Pos2> = HashMap::new();
    let mut all_bounds: Option<Rect> = None;

    for (folder_key, items) in groups {
        let best_col = col_heights
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let base_x = (best_col as f32) * (COL_W + COL_GAP);
        let base_y = col_heights[best_col];
        let body_h = (items.len() as f32) * (ITEM_H + ITEM_GAP) - ITEM_GAP.min(ITEM_GAP * items.len().min(1) as f32);
        let group_h = HEADER_H + GROUP_PAD + body_h.max(40.0) + GROUP_PAD;
        col_heights[best_col] = base_y + group_h + ROW_GAP;

        let offset = state
            .folder_offsets
            .get(&folder_key)
            .copied()
            .unwrap_or([0.0, 0.0]);
        let gx = base_x + offset[0];
        let gy = base_y + offset[1];
        let wrect = Rect::from_min_size(pos2(gx, gy), vec2(COL_W, group_h));
        let header_wrect = Rect::from_min_size(pos2(gx, gy), vec2(COL_W, HEADER_H));
        all_bounds = Some(match all_bounds {
            Some(b) => b.union(wrect),
            None => wrect,
        });

        let mut cards = Vec::with_capacity(items.len());
        let card_w = COL_W - GROUP_PAD * 2.0;
        for (idx, item) in items.into_iter().enumerate() {
            let cy = gy + HEADER_H + GROUP_PAD + (idx as f32) * (ITEM_H + ITEM_GAP);
            let crect = Rect::from_min_size(pos2(gx + GROUP_PAD, cy), vec2(card_w, ITEM_H));
            card_centers.insert(item.path.clone(), crect.center());
            cards.push(PlacedCard { item, wrect: crect });
        }
        placed_groups.push(PlacedGroup {
            title: pretty_folder_title(lang, &folder_key),
            folder_key,
            wrect,
            header_wrect,
            cards,
        });
    }

    if state.needs_fit {
        if let Some(b) = all_bounds {
            state.viewport.fit_bounds(canvas_rect, b, 72.0, true);
        } else {
            state.viewport = Viewport2D::default();
        }
        state.needs_fit = false;
    }
    state.viewport.step_animation(ctx);
    let zoom = state.viewport.zoom;

    let bg_resp = ui.interact(
        canvas_rect,
        ui.make_persistent_id("krono_vault_map_bg"),
        Sense::click_and_drag(),
    );

    let zoom_hud_block = Rect::from_min_max(
        pos2(canvas_rect.right() - 210.0, canvas_rect.bottom() - 54.0),
        canvas_rect.max,
    );
    let tb_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("vmap_hud_tb")));
    let sel_block = ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("vmap_hud_sel")));
    let pointer_blocked = |mp: Pos2| -> bool {
        zoom_hud_block.contains(mp)
            || tb_block.is_some_and(|r| r.contains(mp))
            || sel_block.is_some_and(|r| r.contains(mp))
    };

    let raw_ptr = ctx.pointer_hover_pos();
    let pointer_pos = raw_ptr.filter(|mp| canvas_rect.contains(*mp) && !pointer_blocked(*mp));
    let pointer_world = raw_ptr.map(|mp| state.viewport.screen_to_world(canvas_rect, mp));
    let primary_pressed = pointer_pos.is_some() && ctx.input(|i| i.pointer.primary_pressed());
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let double_clicked = pointer_pos.is_some() && bg_resp.double_clicked();

    // Dragging folder group update
    if !primary_down {
        if state.dragging_folder.take().is_some() {
            state.save_layout(vault_dir);
        }
    } else if let (Some((ref fkey, start_w, orig_off)), Some(wp)) =
        (state.dragging_folder.clone(), pointer_world)
    {
        let d = wp - start_w;
        state
            .folder_offsets
            .insert(fkey.clone(), [orig_off.x + d.x, orig_off.y + d.y]);
        ctx.set_cursor_icon(CursorIcon::Grabbing);
    }

    // Track hovered card
    state.hovered_path = None;
    if let Some(mp) = pointer_pos {
        'hover_search: for grp in placed_groups.iter().rev() {
            for card in grp.cards.iter().rev() {
                let srect = state.viewport.world_rect_to_screen(canvas_rect, card.wrect);
                if srect.contains(mp) {
                    state.hovered_path = Some(card.item.path.clone());
                    break 'hover_search;
                }
            }
        }
    }

    // 3. Paint folder group frames
    let mut add_btn_rects: Vec<(String, Rect)> = Vec::new();
    for grp in &placed_groups {
        let srect = state.viewport.world_rect_to_screen(canvas_rect, grp.wrect);
        if !canvas_rect.expand(40.0).intersects(srect) {
            continue;
        }
        let rad = (10.0 * zoom).clamp(4.0, 14.0);
        painter.rect_filled(srect, Rounding::same(rad), with_alpha(p.bg_sidebar, 0.90));
        painter.rect_stroke(
            srect,
            Rounding::same(rad),
            Stroke::new(1.0_f32, p.border_hairline),
        );

        let hrect = state.viewport.world_rect_to_screen(canvas_rect, grp.header_wrect);
        if zoom >= 0.42 {
            let f_ic_r = Rect::from_center_size(
                pos2(hrect.left() + 14.0 * zoom, hrect.center().y),
                vec2((13.0 * zoom).clamp(8.0, 15.0), (13.0 * zoom).clamp(8.0, 15.0)),
            );
            draw_icon(&painter, f_ic_r, Icon::Folder, p.accent);

            let count_txt = grp.cards.len().to_string();
            let add_size = (20.0 * zoom).clamp(14.0, 22.0);
            let add_r = Rect::from_center_size(
                pos2(hrect.right() - 14.0 * zoom, hrect.center().y),
                vec2(add_size, add_size),
            );
            let add_hov = pointer_pos.is_some_and(|mp| add_r.contains(mp));
            if add_hov {
                painter.rect_filled(add_r, Rounding::same(5.0), p.bg_hover);
            }
            draw_icon(
                &painter,
                Rect::from_center_size(add_r.center(), vec2(add_size * 0.62, add_size * 0.62)),
                Icon::Plus,
                if add_hov { p.accent_text } else { p.text_muted },
            );
            add_btn_rects.push((grp.folder_key.clone(), add_r));

            let fs = (12.0 * zoom).clamp(9.0, 13.5);
            let title_str = format!("{} ({})", grp.title, count_txt);
            let mut title_job = egui::text::LayoutJob::single_section(
                title_str,
                egui::TextFormat {
                    font_id: font_semibold(fs),
                    color: p.text_primary,
                    ..Default::default()
                },
            );
            title_job.wrap.max_width = (hrect.width() - 48.0 * zoom).max(40.0);
            title_job.wrap.max_rows = 1;
            title_job.wrap.break_anywhere = true;
            let galley = painter.layout_job(title_job);
            painter.galley(
                pos2(f_ic_r.right() + 6.0 * zoom, hrect.center().y - galley.size().y * 0.5),
                galley,
                p.text_primary,
            );
        }
    }

    // 4. Paint links (bezier curves) between visible items
    let focus_path = state.hovered_path.as_deref().or(state.selected_path.as_deref());
    let mut drawn_edges: HashSet<(String, String)> = HashSet::new();
    if state.show_links {
        for raw in all_links {
            let Some(dst) = resolve_link_target(&raw.target_raw, &item_lookup) else {
                continue;
            };
            if raw.source_path == dst {
                continue;
            }
            let key = if raw.source_path < dst {
                (raw.source_path.clone(), dst.clone())
            } else {
                (dst.clone(), raw.source_path.clone())
            };
            if !drawn_edges.insert(key) {
                continue;
            }
            let (Some(&w_a), Some(&w_b)) = (card_centers.get(&raw.source_path), card_centers.get(&dst)) else {
                continue;
            };
            let sa = state.viewport.world_to_screen(canvas_rect, w_a);
            let sb = state.viewport.world_to_screen(canvas_rect, w_b);
            let focused = focus_path.is_some_and(|fp| fp == raw.source_path || fp == dst);
            let (col, width) = if focused {
                (with_alpha(p.accent, 0.88), (2.2 * zoom).clamp(1.3, 3.0))
            } else if focus_path.is_some() {
                (with_alpha(p.border_strong, 0.22), 1.0)
            } else {
                (with_alpha(p.accent, 0.34), (1.3 * zoom).clamp(0.9, 2.0))
            };
            let dx = (sb.x - sa.x).abs().max(40.0) * 0.42;
            let c1 = pos2(sa.x + if sb.x >= sa.x { dx } else { -dx }, sa.y);
            let c2 = pos2(sb.x - if sb.x >= sa.x { dx } else { -dx }, sb.y);
            painter.add(Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
                [sa, c1, c2, sb],
                false,
                Color32::TRANSPARENT,
                Stroke::new(width, col),
            )));
        }
    }

    // 5. Paint item cards inside each folder
    for grp in &placed_groups {
        for card in &grp.cards {
            let srect = state.viewport.world_rect_to_screen(canvas_rect, card.wrect);
            if !canvas_rect.expand(40.0).intersects(srect) {
                continue;
            }
            let is_sel = state.selected_path.as_deref() == Some(card.item.path.as_str());
            let is_hov = state.hovered_path.as_deref() == Some(card.item.path.as_str());
            let rad = (7.0 * zoom).clamp(3.0, 9.0);
            let fill = if is_sel {
                lerp_color(p.bg_card, p.accent, 0.14)
            } else if is_hov {
                p.bg_hover
            } else {
                p.bg_card
            };
            let border = if is_sel {
                p.accent
            } else if is_hov {
                with_alpha(p.accent, 0.65)
            } else {
                p.border_hairline
            };
            painter.rect_filled(srect, Rounding::same(rad), fill);
            painter.rect_stroke(
                srect,
                Rounding::same(rad),
                Stroke::new(if is_sel { 1.6_f32 } else { 1.0_f32 }, border),
            );

            if zoom >= 0.42 {
                let card_painter = painter.with_clip_rect(canvas_rect.intersect(srect.shrink(3.0)));
                let pad = (8.0 * zoom).clamp(4.0, 10.0);
                let ic_sz = (12.0 * zoom).clamp(8.0, 14.0);
                let ic_rect = Rect::from_center_size(
                    pos2(srect.left() + pad + ic_sz * 0.5, srect.top() + pad + ic_sz * 0.5 + 1.0),
                    vec2(ic_sz, ic_sz),
                );
                draw_icon(&card_painter, ic_rect, kind_icon(card.item.kind), card.item.accent);

                let t_fs = (12.0 * zoom).clamp(8.5, 13.5);
                let t_w = (srect.width() - pad * 2.0 - ic_sz - 6.0).max(20.0);
                let t_title = if card.item.title.trim().is_empty() {
                    vault_file_stem(&card.item.path).to_string()
                } else {
                    card.item.title.clone()
                };
                let t_galley = card_painter.layout(t_title, font_medium(t_fs), p.text_primary, t_w);
                card_painter.galley(
                    pos2(ic_rect.right() + 6.0 * zoom, srect.top() + pad),
                    t_galley,
                    p.text_primary,
                );

                if !card.item.tags.is_empty() && zoom >= 0.58 {
                    let tags_str = card
                        .item
                        .tags
                        .iter()
                        .take(3)
                        .map(|t| format!("#{t}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let tg = card_painter.layout_no_wrap(
                        tags_str,
                        font_mono((10.0 * zoom).clamp(8.0, 11.0)),
                        p.text_muted,
                    );
                    card_painter.galley(
                        pos2(srect.left() + pad, srect.bottom() - pad - tg.size().y),
                        tg,
                        p.text_muted,
                    );
                }
            }
        }
    }

    if placed_groups.is_empty() {
        let g = painter.layout_no_wrap(
            s.overview_empty.to_string(),
            font_body(14.0),
            p.text_muted,
        );
        painter.galley(
            pos2(
                canvas_rect.center().x - g.size().x * 0.5,
                canvas_rect.center().y - g.size().y * 0.5,
            ),
            g,
            p.text_muted,
        );
    }

    // 6. Click / Drag / Double-click handling
    if primary_pressed {
        if let Some(mp) = pointer_pos {
            let mut hit_something = false;
            for (fkey, add_r) in &add_btn_rects {
                if add_r.contains(mp) {
                    actions.push(UiAction::OpenCreateModalInFolder(fkey.clone()));
                    hit_something = true;
                    break;
                }
            }
            if !hit_something {
                if let Some(hp) = state.hovered_path.clone() {
                    state.selected_path = Some(hp);
                    hit_something = true;
                }
            }
            if !hit_something {
                for grp in placed_groups.iter().rev() {
                    let hrect = state.viewport.world_rect_to_screen(canvas_rect, grp.header_wrect);
                    if hrect.contains(mp) {
                        if let Some(wp) = pointer_world {
                            let off = state
                                .folder_offsets
                                .get(&grp.folder_key)
                                .copied()
                                .unwrap_or([0.0, 0.0]);
                            state.dragging_folder =
                                Some((grp.folder_key.clone(), wp, vec2(off[0], off[1])));
                        }
                        hit_something = true;
                        break;
                    }
                }
            }
            if !hit_something {
                state.selected_path = None;
            }
        }
    }

    if double_clicked {
        if let Some(mp) = pointer_pos {
            'dbl: for grp in placed_groups.iter().rev() {
                for card in grp.cards.iter().rev() {
                    let srect = state.viewport.world_rect_to_screen(canvas_rect, card.wrect);
                    if srect.contains(mp) {
                        open_action_for(card.item.kind, card.item.path.clone(), &mut actions);
                        break 'dbl;
                    }
                }
            }
        }
    }

    let allow_pan = state.dragging_folder.is_none();
    state
        .viewport
        .handle_pan_zoom(ui, canvas_rect, &bg_resp, allow_pan, false);

    // 7. Top Floating Filter Toolbar
    let tb_w = (canvas_rect.width() - 24.0).clamp(240.0, 740.0);
    let tb_rect = Rect::from_min_size(
        pos2(canvas_rect.left() + 12.0, canvas_rect.top() + 12.0),
        vec2(tb_w, 44.0),
    );
    let mut tb_ui = ui.new_child(egui::UiBuilder::new().max_rect(tb_rect));
    tb_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
    let tb_resp = egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_subtle))
        .rounding(Rounding::same(10.0))
        .shadow(p.shadow_elevated)
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(&mut tb_ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let compact = layout.is_mobile() || tb_w < 540.0;
                styled_text_input(
                    ui,
                    p,
                    &mut state.search_query,
                    s.overview_search_hint,
                    false,
                    if compact { 120.0 } else { 170.0 },
                );
                for (kf, lbl) in [
                    (PagesKindFilter::All, s.filter_all),
                    (PagesKindFilter::Notes, s.filter_notes),
                    (PagesKindFilter::Boards, s.filter_boards),
                    (PagesKindFilter::Tables, s.filter_tables),
                    (PagesKindFilter::Canvases, s.filter_canvases),
                ] {
                    let active = state.kind_filter == kf;
                    let fg = if active { p.accent_text } else { p.text_secondary };
                    let g = ui
                        .painter()
                        .layout_no_wrap(lbl.to_string(), font_medium(11.5), fg);
                    let (pr, presp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 24.0), Sense::click());
                    ui.painter().rect_filled(
                        pr,
                        Rounding::same(6.0),
                        if active {
                            p.accent_soft
                        } else if presp.hovered() {
                            p.bg_hover
                        } else {
                            p.bg_input
                        },
                    );
                    ui.painter().galley(
                        pos2(pr.left() + 7.0, pr.center().y - g.size().y * 0.5),
                        g,
                        fg,
                    );
                    if presp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        state.kind_filter = kf;
                    }
                }
                if icon_button(ui, p, Icon::Link, s.overview_show_links, state.show_links).clicked() {
                    state.show_links = !state.show_links;
                }
                if icon_button(ui, p, Icon::RotateCcw, s.overview_reset_layout, false).clicked() {
                    state.folder_offsets.clear();
                    state.needs_fit = true;
                    state.save_layout(vault_dir);
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("vmap_hud_tb"), tb_resp.response.rect));

    // 8. Bottom-Left Selected Item Action Pill
    if let Some(sel_p) = state.selected_path.clone() {
        if let Some(sel_item) = placed_groups
            .iter()
            .flat_map(|g| g.cards.iter())
            .find(|c| c.item.path == sel_p)
            .map(|c| c.item.clone())
        {
            let sw = (canvas_rect.width() - 220.0).clamp(210.0, 380.0);
            let srect = Rect::from_min_size(
                pos2(
                    canvas_rect.left() + 14.0,
                    (canvas_rect.bottom() - 52.0).max(canvas_rect.top() + 60.0),
                ),
                vec2(sw, 40.0),
            );
            let mut s_ui = ui.new_child(egui::UiBuilder::new().max_rect(srect));
            s_ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));
            let s_resp = egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(9.0))
                .shadow(p.shadow_elevated)
                .inner_margin(Margin::symmetric(10.0, 5.0))
                .show(&mut s_ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(&sel_item.title)
                                    .font(font_semibold(12.5))
                                    .color(p.text_primary),
                            )
                            .truncate(),
                        );
                        if secondary_button(ui, p, Some(Icon::ExternalLink), s.open_btn).clicked() {
                            open_action_for(sel_item.kind, sel_item.path.clone(), &mut actions);
                        }
                    });
                });
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("vmap_hud_sel"), s_resp.response.rect));
        } else {
            ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("vmap_hud_sel")));
        }
    } else {
        ctx.data_mut(|d| d.remove::<Rect>(egui::Id::new("vmap_hud_sel")));
    }

    // 9. Bottom-Right Zoom HUD
    render_zoom_hud(ui, canvas_rect, p, lang, &mut state.viewport, all_bounds);

    actions
}
