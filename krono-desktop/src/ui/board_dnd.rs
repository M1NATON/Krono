use eframe::egui::{self, vec2, CursorIcon, Pos2, Rect, Vec2};
use std::collections::HashMap;

pub const DRAG_THRESHOLD_PX: f32 = 6.0;
pub const HYSTERESIS_PX: f32 = 8.0;
pub const SLOT_ANIM_SECS: f32 = 0.12;
pub const SETTLE_SECS: f32 = 0.11;
pub const AUTOSCROLL_EDGE_PX: f32 = 48.0;
pub const AUTOSCROLL_MAX_PX_PER_SEC: f32 = 520.0;
pub const GHOST_LIFT_Y: f32 = 2.0;

#[derive(Clone, Debug)]
pub struct ColumnGeometry {
    pub col_id: String,
    pub drop_rect: Rect,
    pub scroll_rect: Rect,
    pub scroll_offset_y: f32,
    /// Unshifted `(card_id, base_top_y, height)` for non-dragged cards in this column.
    pub base_cards: Vec<(String, f32, f32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DndDropEvent {
    pub card_id: String,
    pub from_col: String,
    pub to_col: String,
    pub insert_before_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub enum DndPhase {
    #[default]
    Idle,
    Pending {
        card_id: String,
        source_col: String,
        source_index: usize,
        press_pos: Pos2,
        grab_offset: Vec2,
        card_size: Vec2,
    },
    Dragging {
        card_id: String,
        source_col: String,
        source_index: usize,
        grab_offset: Vec2,
        card_size: Vec2,
        pointer_pos: Pos2,
        target_col: String,
        target_index: usize,
        slot_anim: HashMap<(String, usize), f32>,
    },
    Settling {
        card_id: String,
        from_min: Pos2,
        to_min: Pos2,
        card_size: Vec2,
        elapsed: f32,
        duration: f32,
        target_rect_resolved: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct BoardDndState {
    pub phase: DndPhase,
    pub columns: HashMap<String, ColumnGeometry>,
    pub board_scroll_rect: Option<Rect>,
    pub board_scroll_offset_x: f32,
    pub card_heights: HashMap<String, f32>,
}

pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

/// Computes insertion slot index from unshifted card rects with midpoint hysteresis.
pub fn compute_target_index(
    pointer_y: f32,
    base_cards: &[(String, f32, f32)],
    prev_index: Option<usize>,
    hysteresis: f32,
) -> usize {
    for (idx, (_, top, h)) in base_cards.iter().enumerate() {
        let mid = *top + *h * 0.5;
        let threshold = match prev_index {
            Some(prev) if idx < prev => mid - hysteresis,
            Some(prev) if idx >= prev => mid + hysteresis,
            _ => mid,
        };
        if pointer_y < threshold {
            return idx;
        }
    }
    base_cards.len()
}

/// Returns dt-scaled scroll offset delta when cursor is inside the 48px edge band.
pub fn edge_autoscroll_delta(
    cursor: f32,
    min_edge: f32,
    max_edge: f32,
    dt: f32,
) -> Option<f32> {
    if max_edge - min_edge <= AUTOSCROLL_EDGE_PX * 2.0 {
        return None;
    }
    if cursor < min_edge + AUTOSCROLL_EDGE_PX {
        let f = ((min_edge + AUTOSCROLL_EDGE_PX - cursor) / AUTOSCROLL_EDGE_PX).clamp(0.0, 1.0);
        let speed = (f * f * 0.7 + f * 0.3) * AUTOSCROLL_MAX_PX_PER_SEC;
        Some(-speed * dt)
    } else if cursor > max_edge - AUTOSCROLL_EDGE_PX {
        let f = ((cursor - (max_edge - AUTOSCROLL_EDGE_PX)) / AUTOSCROLL_EDGE_PX).clamp(0.0, 1.0);
        let speed = (f * f * 0.7 + f * 0.3) * AUTOSCROLL_MAX_PX_PER_SEC;
        Some(speed * dt)
    } else {
        None
    }
}

impl BoardDndState {
    pub fn load(ctx: &egui::Context, id: egui::Id) -> Self {
        ctx.data(|d| d.get_temp::<Self>(id)).unwrap_or_default()
    }

    pub fn store(self, ctx: &egui::Context, id: egui::Id) {
        ctx.data_mut(|d| d.insert_temp(id, self));
    }

    pub fn is_dragging(&self) -> bool {
        matches!(self.phase, DndPhase::Dragging { .. })
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            DndPhase::Dragging { .. } | DndPhase::Settling { .. }
        )
    }

    pub fn dragged_card_id(&self) -> Option<&str> {
        match &self.phase {
            DndPhase::Dragging { card_id, .. } => Some(card_id.as_str()),
            _ => None,
        }
    }

    pub fn hidden_card_id(&self) -> Option<&str> {
        match &self.phase {
            DndPhase::Dragging { card_id, .. } | DndPhase::Settling { card_id, .. } => {
                Some(card_id.as_str())
            }
            _ => None,
        }
    }

    pub fn dragged_card_height(&self) -> f32 {
        match &self.phase {
            DndPhase::Dragging { card_size, .. }
            | DndPhase::Pending { card_size, .. }
            | DndPhase::Settling { card_size, .. } => card_size.y.max(48.0),
            DndPhase::Idle => 72.0,
        }
    }

    pub fn slot_gap_factor(&self, col_id: &str, slot_idx: usize) -> f32 {
        match &self.phase {
            DndPhase::Dragging { slot_anim, .. } => slot_anim
                .iter()
                .find(|((c, idx), _)| c == col_id && *idx == slot_idx)
                .map(|(_, &v)| ease_out_cubic(v))
                .unwrap_or(0.0),
            _ => 0.0,
        }
    }

    pub fn is_active_target_slot(&self, col_id: &str, slot_idx: usize) -> bool {
        match &self.phase {
            DndPhase::Dragging {
                target_col,
                target_index,
                ..
            } => target_col == col_id && *target_index == slot_idx,
            _ => false,
        }
    }

    pub fn is_target_col(&self, col_id: &str) -> bool {
        match &self.phase {
            DndPhase::Dragging { target_col, .. } => target_col == col_id,
            _ => false,
        }
    }

    pub fn is_same_col_sorted_locked(&self, col_id: &str, is_sorted: bool) -> bool {
        if !is_sorted {
            return false;
        }
        match &self.phase {
            DndPhase::Dragging { source_col, .. } => source_col == col_id,
            _ => false,
        }
    }

    pub fn try_begin_pending(
        &mut self,
        card_id: &str,
        col_id: &str,
        card_idx: usize,
        card_rect: Rect,
        press_pos: Pos2,
    ) {
        if !matches!(self.phase, DndPhase::Idle) {
            return;
        }
        self.phase = DndPhase::Pending {
            card_id: card_id.to_string(),
            source_col: col_id.to_string(),
            source_index: card_idx,
            press_pos,
            grab_offset: press_pos - card_rect.min,
            card_size: card_rect.size(),
        };
    }

    pub fn resolve_settling_rect(&mut self, card_id: &str, final_rect: Rect) {
        if let DndPhase::Settling {
            card_id: cid,
            to_min,
            target_rect_resolved,
            ..
        } = &mut self.phase
        {
            if cid == card_id {
                *to_min = final_rect.min;
                *target_rect_resolved = true;
            }
        }
    }

    pub fn active_ghost(&self) -> Option<(&str, Pos2, Vec2)> {
        match &self.phase {
            DndPhase::Dragging {
                card_id,
                pointer_pos,
                grab_offset,
                card_size,
                ..
            } => Some((
                card_id.as_str(),
                *pointer_pos - *grab_offset - vec2(0.0, GHOST_LIFT_Y),
                *card_size,
            )),
            DndPhase::Settling {
                card_id,
                from_min,
                to_min,
                card_size,
                elapsed,
                duration,
                ..
            } => {
                let t = ease_out_cubic((*elapsed / (*duration).max(0.001)).clamp(0.0, 1.0));
                let pos = *from_min + (*to_min - *from_min) * t;
                Some((card_id.as_str(), pos, *card_size))
            }
            _ => None,
        }
    }

    pub fn horizontal_autoscroll_offset(&self, ctx: &egui::Context) -> Option<f32> {
        let DndPhase::Dragging { pointer_pos, .. } = &self.phase else {
            return None;
        };
        let rect = self.board_scroll_rect?;
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.001, 0.05);
        let delta = edge_autoscroll_delta(pointer_pos.x, rect.left(), rect.right(), dt)?;
        ctx.request_repaint();
        Some((self.board_scroll_offset_x + delta).max(0.0))
    }

    pub fn vertical_autoscroll_offset(&self, ctx: &egui::Context, col_id: &str) -> Option<f32> {
        let DndPhase::Dragging {
            pointer_pos,
            target_col,
            ..
        } = &self.phase
        else {
            return None;
        };
        if target_col != col_id {
            return None;
        }
        let col = self.columns.get(col_id)?;
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.001, 0.05);
        let delta =
            edge_autoscroll_delta(pointer_pos.y, col.scroll_rect.top(), col.scroll_rect.bottom(), dt)?;
        ctx.request_repaint();
        Some((col.scroll_offset_y + delta).max(0.0))
    }

    pub fn begin_frame(&mut self, ctx: &egui::Context, is_sorted: bool) -> Option<DndDropEvent> {
        let dt = ctx.input(|i| i.unstable_dt).clamp(0.001, 0.05);
        let ptr_pos = ctx.input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let primary_released = ctx.input(|i| i.pointer.primary_released());
        let secondary_pressed = ctx.input(|i| i.pointer.secondary_pressed());
        let esc_pressed = if matches!(
            self.phase,
            DndPhase::Pending { .. } | DndPhase::Dragging { .. }
        ) {
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        } else {
            false
        };

        self.advance_step(
            ctx,
            is_sorted,
            dt,
            ptr_pos,
            primary_down,
            primary_released,
            secondary_pressed,
            esc_pressed,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn advance_step(
        &mut self,
        ctx: &egui::Context,
        is_sorted: bool,
        dt: f32,
        ptr_pos: Option<Pos2>,
        primary_down: bool,
        primary_released: bool,
        secondary_pressed: bool,
        esc_pressed: bool,
    ) -> Option<DndDropEvent> {
        match &mut self.phase {
            DndPhase::Idle => None,
            DndPhase::Pending {
                card_id,
                source_col,
                source_index,
                press_pos,
                grab_offset,
                card_size,
            } => {
                if esc_pressed || secondary_pressed || !primary_down {
                    self.phase = DndPhase::Idle;
                    return None;
                }
                if let Some(ptr) = ptr_pos {
                    if (ptr - *press_pos).length() >= DRAG_THRESHOLD_PX {
                        let mut slot_anim = HashMap::new();
                        slot_anim.insert((source_col.clone(), *source_index), 1.0);
                        self.phase = DndPhase::Dragging {
                            card_id: card_id.clone(),
                            source_col: source_col.clone(),
                            source_index: *source_index,
                            grab_offset: *grab_offset,
                            card_size: *card_size,
                            pointer_pos: ptr,
                            target_col: source_col.clone(),
                            target_index: *source_index,
                            slot_anim,
                        };
                        ctx.set_cursor_icon(CursorIcon::Grabbing);
                        ctx.request_repaint();
                    }
                }
                None
            }
            DndPhase::Dragging {
                card_id,
                source_col,
                source_index,
                grab_offset,
                card_size,
                pointer_pos,
                target_col,
                target_index,
                slot_anim,
            } => {
                ctx.set_cursor_icon(CursorIcon::Grabbing);
                ctx.request_repaint();
                if let Some(ptr) = ptr_pos {
                    *pointer_pos = ptr;
                }

                let ghost_min = *pointer_pos - *grab_offset - vec2(0.0, GHOST_LIFT_Y);

                if esc_pressed || secondary_pressed {
                    self.phase = DndPhase::Settling {
                        card_id: card_id.clone(),
                        from_min: ghost_min,
                        to_min: ghost_min,
                        card_size: *card_size,
                        elapsed: 0.0,
                        duration: SETTLE_SECS,
                        target_rect_resolved: false,
                    };
                    return None;
                }

                if primary_released || !primary_down {
                    let drop_ev = if *target_col != *source_col {
                        let insert_before_id = if is_sorted {
                            None
                        } else {
                            self.columns
                                .get(target_col.as_str())
                                .and_then(|c| c.base_cards.get(*target_index))
                                .map(|(id, _, _)| id.clone())
                        };
                        Some(DndDropEvent {
                            card_id: card_id.clone(),
                            from_col: source_col.clone(),
                            to_col: target_col.clone(),
                            insert_before_id,
                        })
                    } else if !is_sorted && *target_index != *source_index {
                        let insert_before_id = self
                            .columns
                            .get(target_col.as_str())
                            .and_then(|c| c.base_cards.get(*target_index))
                            .map(|(id, _, _)| id.clone());
                        Some(DndDropEvent {
                            card_id: card_id.clone(),
                            from_col: source_col.clone(),
                            to_col: target_col.clone(),
                            insert_before_id,
                        })
                    } else {
                        None
                    };

                    self.phase = DndPhase::Settling {
                        card_id: card_id.clone(),
                        from_min: ghost_min,
                        to_min: ghost_min,
                        card_size: *card_size,
                        elapsed: 0.0,
                        duration: SETTLE_SECS,
                        target_rect_resolved: false,
                    };
                    return drop_ev;
                }

                // Hit-test columns using full-height drop_rect
                if let Some(hit_col) = self
                    .columns
                    .values()
                    .find(|c| c.drop_rect.contains(*pointer_pos))
                {
                    if is_sorted && hit_col.col_id == *source_col {
                        *target_col = source_col.clone();
                        *target_index = *source_index;
                    } else if is_sorted {
                        *target_col = hit_col.col_id.clone();
                        *target_index = 0;
                    } else {
                        let prev = if *target_col == hit_col.col_id {
                            Some(*target_index)
                        } else {
                            None
                        };
                        *target_index = compute_target_index(
                            pointer_pos.y,
                            &hit_col.base_cards,
                            prev,
                            HYSTERESIS_PX,
                        );
                        *target_col = hit_col.col_id.clone();
                    }
                }

                let active_key = if is_sorted && *target_col == *source_col {
                    None
                } else {
                    Some((target_col.clone(), *target_index))
                };
                if let Some(ref k) = active_key {
                    slot_anim.entry(k.clone()).or_insert(0.0);
                }
                let step = dt / SLOT_ANIM_SECS;
                for (k, v) in slot_anim.iter_mut() {
                    if active_key.as_ref() == Some(k) {
                        *v = (*v + step).min(1.0);
                    } else {
                        *v = (*v - step).max(0.0);
                    }
                }
                slot_anim.retain(|_, v| *v > 0.001);
                None
            }
            DndPhase::Settling {
                elapsed,
                duration,
                ..
            } => {
                *elapsed += dt;
                if *elapsed >= *duration {
                    self.phase = DndPhase::Idle;
                } else {
                    ctx.request_repaint();
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{pos2, vec2};

    #[test]
    fn hysteresis_prevents_slot_jitter_around_card_midpoint() {
        let cards = vec![
            ("c1".into(), 100.0, 80.0), // midpoint = 140.0
            ("c2".into(), 190.0, 80.0), // midpoint = 230.0
        ];
        // Without prev_index: below 140 -> slot 0, above 140 -> slot 1
        assert_eq!(compute_target_index(138.0, &cards, None, HYSTERESIS_PX), 0);
        assert_eq!(compute_target_index(142.0, &cards, None, HYSTERESIS_PX), 1);

        // With prev_index = 1: moving slightly above 140 (to 135) stays at 1 until < 132 (140 - 8)
        assert_eq!(compute_target_index(135.0, &cards, Some(1), HYSTERESIS_PX), 1);
        assert_eq!(compute_target_index(131.0, &cards, Some(1), HYSTERESIS_PX), 0);

        // With prev_index = 0: moving slightly below 140 (to 145) stays at 0 until >= 148 (140 + 8)
        assert_eq!(compute_target_index(145.0, &cards, Some(0), HYSTERESIS_PX), 0);
        assert_eq!(compute_target_index(149.0, &cards, Some(0), HYSTERESIS_PX), 1);
    }

    #[test]
    fn edge_autoscroll_scales_with_dt_and_proximity() {
        assert_eq!(edge_autoscroll_delta(200.0, 0.0, 400.0, 0.016), None);
        let d_near = edge_autoscroll_delta(10.0, 0.0, 400.0, 0.016).unwrap();
        let d_far = edge_autoscroll_delta(38.0, 0.0, 400.0, 0.016).unwrap();
        assert!(d_near < 0.0 && d_far < 0.0);
        assert!(d_near.abs() > d_far.abs());

        let d_bottom = edge_autoscroll_delta(392.0, 0.0, 400.0, 0.016).unwrap();
        assert!(d_bottom > 0.0);
    }

    #[test]
    fn pending_requires_threshold_and_cancel_returns_no_drop() {
        let ctx = egui::Context::default();
        let mut dnd = BoardDndState::default();
        let rect = Rect::from_min_size(pos2(20.0, 40.0), vec2(240.0, 80.0));
        dnd.try_begin_pending("card_1", "todo", 0, rect, pos2(50.0, 60.0));

        // Move < 6px -> stays Pending
        let ev = dnd.advance_step(&ctx, false, 0.016, Some(pos2(53.0, 62.0)), true, false, false, false);
        assert!(ev.is_none());
        assert!(matches!(dnd.phase, DndPhase::Pending { .. }));

        // Move >= 6px -> transitions to Dragging
        let ev = dnd.advance_step(&ctx, false, 0.016, Some(pos2(60.0, 65.0)), true, false, false, false);
        assert!(ev.is_none());
        assert!(dnd.is_dragging());

        // Escape cancels without returning a drop event and enters Settling
        let ev = dnd.advance_step(&ctx, false, 0.016, Some(pos2(120.0, 90.0)), true, false, false, true);
        assert!(ev.is_none());
        assert!(matches!(dnd.phase, DndPhase::Settling { .. }));
    }
}
