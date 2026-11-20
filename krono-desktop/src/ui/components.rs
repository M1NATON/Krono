use crate::i18n::{status_label, Lang};
use crate::ui::icons::{draw_icon, draw_sync_icon, Icon};
use crate::ui::motion::{
    anim_bool_eased, anim_value_smooth, ease_in_out_cubic, ease_out_cubic, lerp_color, with_alpha,
};
use crate::ui::theme::{font_body, font_medium, font_mono, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, Layout, Margin, Painter,
    PopupCloseBehavior, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui,
};
use krono_core::note::NoteStatus;
use std::{hash::Hash, time::Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub message: String,
    pub kind: ToastKind,
    pub created_at: Instant,
    pub ttl_secs: f32,
}

impl Toast {
    pub fn new(message: impl Into<String>, kind: ToastKind) -> Self {
        Self {
            message: message.into(),
            kind,
            created_at: Instant::now(),
            ttl_secs: if kind == ToastKind::Error { 6.0 } else { 3.5 },
        }
    }
}

pub fn render_toasts(ctx: &egui::Context, p: &ThemePalette, toasts: &mut Vec<Toast>) {
    toasts.retain(|t| t.created_at.elapsed().as_secs_f32() < t.ttl_secs);
    if toasts.is_empty() {
        return;
    }

    let min_remaining = toasts
        .iter()
        .map(|t| {
            let e = t.created_at.elapsed().as_secs_f32();
            if e < 0.25 || t.ttl_secs - e < 0.4 {
                16
            } else {
                120
            }
        })
        .min()
        .unwrap_or(120);
    ctx.request_repaint_after(std::time::Duration::from_millis(min_remaining));

    egui::Area::new(egui::Id::new("krono_toasts_overlay"))
        .order(egui::Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -20.0))
        .show(ctx, |ui| {
            ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for toast in toasts.iter().rev() {
                    let elapsed = toast.created_at.elapsed().as_secs_f32();
                    let enter_t = ease_out_cubic((elapsed / 0.18).clamp(0.0, 1.0));
                    let exit_t = ((toast.ttl_secs - elapsed) / 0.35).clamp(0.0, 1.0);
                    let alpha = (enter_t * exit_t).clamp(0.0, 1.0);
                    let slide_y = (1.0 - enter_t) * 10.0;

                    let (accent, icon) = match toast.kind {
                        ToastKind::Success => (p.success, Icon::Check),
                        ToastKind::Error => (p.danger, Icon::Alert),
                        ToastKind::Info => (p.accent_text, Icon::Sync),
                    };

                    ui.add_space(slide_y);
                    egui::Frame::none()
                        .fill(with_alpha(p.bg_elevated, alpha))
                        .stroke(Stroke::new(1.0_f32, with_alpha(p.border_strong, alpha)))
                        .rounding(Rounding::same(8.0))
                        .shadow(p.shadow_elevated)
                        .inner_margin(Margin::symmetric(12.0, 9.0))
                        .show(ui, |ui| {
                            ui.set_max_width(360.0);
                            ui.horizontal(|ui| {
                                let (icon_rect, _) =
                                    ui.allocate_exact_size(vec2(15.0, 15.0), Sense::hover());
                                draw_icon(
                                    ui.painter(),
                                    icon_rect,
                                    icon,
                                    with_alpha(accent, alpha),
                                );
                                ui.add_space(2.0);
                                ui.label(
                                    egui::RichText::new(&toast.message)
                                        .font(font_medium(12.5))
                                        .color(with_alpha(p.text_primary, alpha)),
                                );
                            });
                        });
                }
            });
        });
}

pub fn draw_outer_right_border(ctx: &egui::Context, rect: Rect, p: &ThemePalette) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("outer_borders_layer"),
    ));
    let x = rect.right().round();
    painter.line_segment(
        [pos2(x, rect.top()), pos2(x, rect.bottom())],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}

pub fn draw_outer_left_border(ctx: &egui::Context, rect: Rect, p: &ThemePalette) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("outer_borders_layer"),
    ));
    let x = rect.left().round();
    painter.line_segment(
        [pos2(x, rect.top()), pos2(x, rect.bottom())],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}

pub fn draw_outer_bottom_border(ctx: &egui::Context, rect: Rect, p: &ThemePalette) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("outer_borders_layer"),
    ));
    let y = rect.bottom().round();
    painter.line_segment(
        [pos2(rect.left(), y), pos2(rect.right(), y)],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}

pub fn draw_outer_top_border(ctx: &egui::Context, rect: Rect, p: &ThemePalette) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("outer_borders_layer"),
    ));
    let y = rect.top().round();
    painter.line_segment(
        [pos2(rect.left(), y), pos2(rect.right(), y)],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}

pub fn hairline_h(ui: &mut Ui, p: &ThemePalette) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 1.0), Sense::hover());
    let y = rect.center().y.round();
    ui.painter().line_segment(
        [pos2(rect.left(), y), pos2(rect.right(), y)],
        Stroke::new(1.0_f32, p.border_hairline),
    );
}

pub fn icon_button(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Icon,
    tooltip: &str,
    active: bool,
) -> Response {
    let size = vec2(28.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.11);
    let press_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("prs"),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    let vis_rect = rect.shrink(press_t * 0.8);

    let base_bg = if active {
        p.bg_selected
    } else {
        Color32::TRANSPARENT
    };
    let target_bg = if active { p.bg_selected } else { p.bg_hover };
    let bg = lerp_color(base_bg, target_bg, hover_t);

    if bg.a() > 0 {
        ui.painter().rect_filled(vis_rect, Rounding::same(6.0), bg);
    }

    let fg_rest = if active { p.text_primary } else { p.text_secondary };
    let fg = lerp_color(fg_rest, p.text_primary, hover_t);

    let icon_rect = Rect::from_center_size(vis_rect.center(), vec2(15.0, 15.0));
    draw_icon(ui.painter(), icon_rect, icon, fg);

    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

pub fn primary_button(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Option<Icon>,
    label: &str,
    full_width: bool,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(13.0), p.accent_on_fill);
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    let content_w = galley.size().x + icon_w;
    let desired_w = if full_width {
        ui.available_width().max(content_w + 24.0)
    } else {
        content_w + 24.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(desired_w, 30.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.11);
    let press_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("prs"),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    let vis_rect = rect.shrink(press_t * 0.8);
    let bg = lerp_color(p.accent, p.accent_hover, hover_t);
    ui.painter().rect_filled(vis_rect, Rounding::same(6.0), bg);

    let start_x = vis_rect.center().x - content_w * 0.5;
    if let Some(ic) = icon {
        let ir = Rect::from_center_size(pos2(start_x + 6.5, vis_rect.center().y), vec2(14.0, 14.0));
        draw_icon(ui.painter(), ir, ic, p.accent_on_fill);
    }
    ui.painter().galley(
        pos2(start_x + icon_w, vis_rect.center().y - galley.size().y * 0.5),
        galley,
        p.accent_on_fill,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn secondary_button(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Option<Icon>,
    label: &str,
) -> Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.5), p.text_primary);
    let icon_w = if icon.is_some() { 18.0 } else { 0.0 };
    let desired_w = galley.size().x + icon_w + 20.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(desired_w, 29.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.11);
    let press_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("prs"),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    let vis_rect = rect.shrink(press_t * 0.8);

    let bg = lerp_color(p.bg_elevated, p.bg_hover, hover_t);
    let border = lerp_color(p.border_subtle, p.border_strong, hover_t);
    ui.painter().rect_filled(vis_rect, Rounding::same(6.0), bg);
    ui.painter()
        .rect_stroke(vis_rect, Rounding::same(6.0), Stroke::new(1.0_f32, border));

    let mut x = vis_rect.left() + 10.0;
    if let Some(ic) = icon {
        let ir = Rect::from_center_size(pos2(x + 6.5, vis_rect.center().y), vec2(13.5, 13.5));
        draw_icon(ui.painter(), ir, ic, p.text_secondary);
        x += 18.0;
    }
    ui.painter().galley(
        pos2(x, vis_rect.center().y - galley.size().y * 0.5),
        galley,
        p.text_primary,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn ghost_sync_button(
    ui: &mut Ui,
    p: &ThemePalette,
    label: &str,
    spinning: bool,
    angle: f32,
    configured: bool,
) -> Response {
    let fg_rest = if spinning {
        p.accent_text
    } else if configured {
        p.text_secondary
    } else {
        p.text_muted
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.5), fg_rest);
    let desired_w = galley.size().x + 32.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(desired_w, 28.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.11);

    let bg = if spinning {
        p.accent_soft
    } else {
        with_alpha(p.bg_hover, hover_t)
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
    }

    let fg = if spinning {
        p.accent_text
    } else {
        lerp_color(fg_rest, p.text_primary, hover_t)
    };
    let ir = Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(14.0, 14.0));
    draw_sync_icon(ui.painter(), ir, fg, angle);
    ui.painter().galley(
        pos2(
            rect.left() + 26.0,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn compact_sync_icon_button(
    ui: &mut Ui,
    p: &ThemePalette,
    tooltip: &str,
    spinning: bool,
    angle: f32,
    configured: bool,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
    let bg = if spinning {
        p.accent_soft
    } else {
        with_alpha(p.bg_hover, hov)
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
    }
    let fg = if spinning {
        p.accent_text
    } else if configured {
        lerp_color(p.text_secondary, p.text_primary, hov)
    } else {
        lerp_color(p.text_muted, p.text_secondary, hov)
    };
    draw_sync_icon(
        ui.painter(),
        Rect::from_center_size(rect.center(), vec2(14.5, 14.5)),
        fg,
        angle,
    );
    if configured && !spinning {
        ui.painter()
            .circle_filled(pos2(rect.right() - 5.5, rect.top() + 5.5), 2.3, p.success);
    }
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if tooltip.is_empty() {
        resp
    } else {
        resp.on_hover_text(tooltip)
    }
}

/// Smoothly gliding segmented control that ALWAYS lays out left-to-right
/// (even when placed inside a `right_to_left` topbar).
pub fn segmented_control<T: Copy + PartialEq>(
    ui: &mut Ui,
    p: &ThemePalette,
    id_salt: impl std::hash::Hash,
    current: &mut T,
    items: &[(T, Option<Icon>, &str)],
) -> bool {
    let mut changed = false;
    let group_id = ui.make_persistent_id(id_salt);

    struct MeasuredSeg<T> {
        value: T,
        icon: Option<Icon>,
        width: f32,
        galley: std::sync::Arc<egui::Galley>,
        icon_only: bool,
        active: bool,
    }

    let mut measured = Vec::with_capacity(items.len());
    let mut total_inner_w = 0.0_f32;
    for (idx, &(value, icon, label)) in items.iter().enumerate() {
        let active = *current == value;
        let fg = if active { p.text_primary } else { p.text_secondary };
        let font = if active { font_medium(12.5) } else { font_body(12.5) };
        let galley = ui.painter().layout_no_wrap(label.to_string(), font, fg);
        let icon_only = label.is_empty() && icon.is_some();
        let w = if icon_only {
            28.0
        } else {
            let icon_w = if icon.is_some() { 16.0 } else { 0.0 };
            galley.size().x + icon_w + 16.0
        };
        if idx > 0 {
            total_inner_w += 2.0;
        }
        total_inner_w += w;
        measured.push(MeasuredSeg {
            value,
            icon,
            width: w,
            galley,
            icon_only,
            active,
        });
    }

    let outer_size = vec2(total_inner_w + 8.0, 30.0);
    ui.allocate_ui_with_layout(
        outer_size,
        Layout::left_to_right(Align::Center),
        |ui| {
            egui::Frame::none()
                .fill(p.bg_input)
                .stroke(Stroke::new(1.0_f32, p.border_hairline))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::same(3.0))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;

                    struct SegSlot<T> {
                        value: T,
                        icon: Option<Icon>,
                        rect: Rect,
                        resp: Response,
                        galley: std::sync::Arc<egui::Galley>,
                        icon_only: bool,
                        active: bool,
                    }

                    let mut slots = Vec::with_capacity(measured.len());
                    let mut active_rect: Option<Rect> = None;

                    for m in measured {
                        let (rect, resp) =
                            ui.allocate_exact_size(vec2(m.width, 24.0), Sense::click());
                        if m.active {
                            active_rect = Some(rect);
                        }
                        slots.push(SegSlot {
                            value: m.value,
                            icon: m.icon,
                            rect,
                            resp,
                            galley: m.galley,
                            icon_only: m.icon_only,
                            active: m.active,
                        });
                    }

                    if let Some(target_r) = active_rect {
                        let ax = anim_value_smooth(
                            ui.ctx(),
                            group_id.with("pill_x"),
                            target_r.left(),
                            0.15,
                        );
                        let aw = anim_value_smooth(
                            ui.ctx(),
                            group_id.with("pill_w"),
                            target_r.width(),
                            0.15,
                        );
                        let pill_r = Rect::from_min_size(
                            pos2(ax, target_r.top()),
                            vec2(aw.max(12.0), target_r.height()),
                        );
                        ui.painter()
                            .add(p.shadow_elevated.as_shape(pill_r, Rounding::same(5.5)));
                        ui.painter()
                            .rect_filled(pill_r, Rounding::same(5.5), p.bg_elevated);
                    }

                    for slot in slots {
                        let hover_t = anim_bool_eased(
                            ui.ctx(),
                            slot.resp.id.with("hov"),
                            slot.resp.hovered(),
                            0.10,
                        );
                        if !slot.active && hover_t > 0.01 {
                            ui.painter().rect_filled(
                                slot.rect,
                                Rounding::same(5.0),
                                with_alpha(p.bg_hover, hover_t),
                            );
                        }
                        let fg = if slot.active {
                            p.text_primary
                        } else {
                            lerp_color(p.text_secondary, p.text_primary, hover_t)
                        };
                        if slot.icon_only {
                            if let Some(ic) = slot.icon {
                                let ir = Rect::from_center_size(slot.rect.center(), vec2(13.5, 13.5));
                                draw_icon(ui.painter(), ir, ic, fg);
                            }
                        } else {
                            let mut x = slot.rect.left() + 8.0;
                            if let Some(ic) = slot.icon {
                                let ir = Rect::from_center_size(
                                    pos2(x + 6.0, slot.rect.center().y),
                                    vec2(12.5, 12.5),
                                );
                                draw_icon(ui.painter(), ir, ic, fg);
                                x += 16.0;
                            }
                            ui.painter().galley(
                                pos2(x, slot.rect.center().y - slot.galley.size().y * 0.5),
                                slot.galley,
                                fg,
                            );
                        }
                        if slot
                            .resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .clicked()
                            && !slot.active
                        {
                            *current = slot.value;
                            changed = true;
                        }
                    }
                });
        },
    );
    changed
}

pub fn sidebar_row(
    ui: &mut Ui,
    p: &ThemePalette,
    id_salt: impl std::hash::Hash,
    icon: Icon,
    label: &str,
    count_opt: Option<usize>,
    selected: bool,
    indent_px: f32,
) -> Response {
    let w = ui.available_width().max(80.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    let id = ui.make_persistent_id(id_salt);
    let hover_t = anim_bool_eased(ui.ctx(), id.with("hov"), resp.hovered(), 0.10);

    if selected {
        ui.painter()
            .rect_filled(rect, Rounding::same(6.0), p.bg_selected);
    } else if hover_t > 0.01 {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            with_alpha(p.bg_hover, hover_t),
        );
    }

    let icon_color = if selected {
        p.text_primary
    } else {
        lerp_color(p.text_muted, p.text_secondary, hover_t)
    };
    let text_color = if selected {
        p.text_primary
    } else {
        lerp_color(p.text_secondary, p.text_primary, hover_t)
    };

    let icon_x = rect.left() + 8.0 + indent_px;
    let ir = Rect::from_center_size(pos2(icon_x + 7.0, rect.center().y), vec2(14.0, 14.0));
    draw_icon(ui.painter(), ir, icon, icon_color);

    let right_reserve = if count_opt.is_some() { 30.0 } else { 8.0 };
    let text_left = icon_x + 21.0;
    let max_text_w = (rect.right() - right_reserve - text_left).max(20.0);

    let mut job = egui::text::LayoutJob::single_section(
        label.to_string(),
        egui::TextFormat {
            font_id: if selected {
                font_medium(13.0)
            } else {
                font_body(13.0)
            },
            color: text_color,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_text_w,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(
        pos2(text_left, rect.center().y - galley.size().y * 0.5),
        galley,
        text_color,
    );

    if let Some(count) = count_opt {
        let count_galley = ui.painter().layout_no_wrap(
            count.to_string(),
            font_medium(11.0),
            p.text_muted,
        );
        ui.painter().galley(
            pos2(
                rect.right() - 10.0 - count_galley.size().x,
                rect.center().y - count_galley.size().y * 0.5,
            ),
            count_galley,
            p.text_muted,
        );
    }

    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Tactile status pill with soft tint, dot, and clean dropdown popup.
pub fn status_pill_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    id_salt: impl std::hash::Hash,
    current: NoteStatus,
) -> Option<NoteStatus> {
    let (pill_bg, dot_col, text_col) = p.status_colors(current);
    let label = status_label(lang, current);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.0), text_col);
    let w = galley.size().x + 34.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);

    ui.painter().rect_filled(rect, Rounding::same(12.0), pill_bg);
    if hover_t > 0.01 {
        ui.painter().rect_stroke(
            rect,
            Rounding::same(12.0),
            Stroke::new(1.0_f32, with_alpha(p.border_subtle, hover_t)),
        );
    }
    ui.painter()
        .circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.2, dot_col);
    ui.painter().galley(
        pos2(rect.left() + 19.0, rect.center().y - galley.size().y * 0.5),
        galley,
        text_col,
    );
    let chev_rect =
        Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(10.0, 10.0));
    draw_icon(ui.painter(), chev_rect, Icon::ChevronDown, text_col);

    let popup_id = ui.make_persistent_id(id_salt);
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    let mut chosen: Option<NoteStatus> = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &resp,
        PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(152.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            for opt in NoteStatus::ALL {
                let (obg, odot, otext) = p.status_colors(opt);
                let olbl = status_label(lang, opt);
                let og = ui
                    .painter()
                    .layout_no_wrap(olbl.to_string(), font_medium(12.5), otext);
                let (orect, oresp) =
                    ui.allocate_exact_size(vec2(146.0, 26.0), Sense::click());
                if oresp.hovered() || opt == current {
                    ui.painter().rect_filled(
                        orect,
                        Rounding::same(5.0),
                        if opt == current { obg } else { p.bg_hover },
                    );
                }
                ui.painter()
                    .circle_filled(pos2(orect.left() + 10.0, orect.center().y), 3.4, odot);
                ui.painter().galley(
                    pos2(orect.left() + 20.0, orect.center().y - og.size().y * 0.5),
                    og,
                    if opt == current { otext } else { p.text_primary },
                );
                if opt == current {
                    let chk = Rect::from_center_size(
                        pos2(orect.right() - 12.0, orect.center().y),
                        vec2(12.0, 12.0),
                    );
                    draw_icon(ui.painter(), chk, Icon::Check, otext);
                }
                if oresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    chosen = Some(opt);
                }
            }
        },
    );
    chosen
}

pub fn tag_chip(
    ui: &mut Ui,
    p: &ThemePalette,
    tag: &str,
    removable: bool,
) -> (Response, bool) {
    let (bg, fg) = p.tag_colors(tag);
    let text = format!("#{tag}");
    let galley = ui.painter().layout_no_wrap(text, font_medium(11.5), fg);
    let extra = if removable { 16.0 } else { 0.0 };
    let w = galley.size().x + 14.0 + extra;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 21.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);

    ui.painter().rect_filled(rect, Rounding::same(5.0), bg);
    if hover_t > 0.01 {
        ui.painter().rect_stroke(
            rect,
            Rounding::same(5.0),
            Stroke::new(1.0_f32, with_alpha(p.border_subtle, hover_t)),
        );
    }
    ui.painter().galley(
        pos2(rect.left() + 7.0, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );

    let mut removed = false;
    if removable {
        let close_rect =
            Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(12.0, 12.0));
        let close_resp = ui.interact(close_rect, resp.id.with("rm_tag"), Sense::click());
        draw_icon(
            ui.painter(),
            Rect::from_center_size(close_rect.center(), vec2(9.0, 9.0)),
            Icon::Close,
            if close_resp.hovered() { p.danger } else { fg },
        );
        if close_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            removed = true;
        }
    }

    (resp.on_hover_cursor(CursorIcon::PointingHand), removed)
}

/// Smooth animated toggle-switch (replaces default egui checkbox).
pub fn toggle_switch(
    ui: &mut Ui,
    p: &ThemePalette,
    id_salt: impl Hash,
    checked: &mut bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 21.0), Sense::click());
    let id = ui.make_persistent_id(id_salt);
    let raw = ui.ctx().animate_bool_with_time(id.with("sw"), *checked, 0.16);
    if raw > 0.001 && raw < 0.999 {
        ui.ctx().request_repaint();
    }
    let t = ease_in_out_cubic(raw);
    let hov_t = anim_bool_eased(ui.ctx(), id.with("hov"), resp.hovered(), 0.10);

    let off_bg = lerp_color(p.bg_selected, p.bg_hover, hov_t * 0.6);
    let track_bg = lerp_color(off_bg, p.accent, t);
    ui.painter()
        .rect_filled(rect, Rounding::same(10.5), track_bg);
    if t < 0.99 {
        ui.painter().rect_stroke(
            rect,
            Rounding::same(10.5),
            Stroke::new(1.0_f32, with_alpha(p.border_subtle, 1.0 - t)),
        );
    }

    let thumb_x = crate::ui::motion::lerp_f32(rect.left() + 10.5, rect.right() - 10.5, t);
    let thumb_rest = if p.is_dark { p.text_primary } else { Color32::WHITE };
    let thumb_col = lerp_color(thumb_rest, p.accent_on_fill, t);
    let stretch = (4.0 * raw * (1.0 - raw)).clamp(0.0, 1.0) * 3.5;
    let thumb_r = Rect::from_center_size(pos2(thumb_x, rect.center().y), vec2(15.0 + stretch, 15.0));
    ui.painter()
        .rect_filled(thumb_r, Rounding::same(7.5), thumb_col);

    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

/// Requests focus for `target_id` once when `active` transitions from `false` to `true`.
pub fn focus_once(ctx: &egui::Context, gate_id: egui::Id, active: bool, target_id: egui::Id) {
    let was_active = ctx.data(|d| d.get_temp::<bool>(gate_id)).unwrap_or(false);
    if active && !was_active {
        ctx.memory_mut(|m| m.request_focus(target_id));
    }
    ctx.data_mut(|d| d.insert_temp(gate_id, active));
}

/// Clears a `focus_once` gate when its panel or field is dismissed.
pub fn reset_focus_gate(ctx: &egui::Context, gate_id: egui::Id) {
    ctx.data_mut(|d| d.insert_temp(gate_id, false));
}

/// Recessed tonal single-line text field with smooth animated focus halo and zero chalky wireframe.
pub fn styled_text_input(
    ui: &mut Ui,
    p: &ThemePalette,
    text: &mut String,
    hint: &str,
    password: bool,
    desired_width: f32,
) -> Response {
    let bg_slot = ui.painter().add(egui::Shape::Noop);
    let edit_w = if desired_width.is_finite() {
        (desired_width - 20.0).max(40.0)
    } else {
        f32::INFINITY
    };

    let frame_resp = egui::Frame::none()
        .inner_margin(Margin::symmetric(10.0, 7.0))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(text)
                    .frame(false)
                    .password(password)
                    .hint_text(
                        egui::RichText::new(hint)
                            .color(lerp_color(p.text_muted, p.text_secondary, 0.30)),
                    )
                    .font(font_body(13.0))
                    .text_color(p.text_primary)
                    .margin(Margin::ZERO)
                    .desired_width(edit_w),
            )
        });
    let resp = frame_resp.inner;
    let outer_rect = frame_resp.response.rect;

    if ui.input(|i| {
        i.pointer.primary_pressed()
            && i.pointer
                .interact_pos()
                .is_some_and(|pt| outer_rect.contains(pt) && !resp.rect.contains(pt))
    }) {
        resp.request_focus();
    }

    let focus_t = anim_bool_eased(ui.ctx(), resp.id.with("foc"), resp.has_focus(), 0.13);
    let hov_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("hov"),
        resp.hovered() || frame_resp.response.hovered(),
        0.10,
    );
    let rest_stroke = lerp_color(p.border_subtle, p.border_strong, hov_t);
    let stroke_col = lerp_color(rest_stroke, p.accent, focus_t);
    let stroke_w = 1.0 + 0.25 * focus_t;

    ui.painter().set(
        bg_slot,
        egui::Shape::rect_filled(outer_rect, Rounding::same(7.0), p.bg_input),
    );
    ui.painter().rect_stroke(
        outer_rect,
        Rounding::same(7.0),
        Stroke::new(stroke_w, stroke_col),
    );
    if focus_t > 0.02 {
        ui.painter().rect_stroke(
            outer_rect.expand(1.5),
            Rounding::same(8.5),
            Stroke::new(2.0_f32, with_alpha(p.accent, 0.18 * focus_t)),
        );
    }
    resp
}

pub fn context_menu_row(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Icon,
    label: &str,
    danger: bool,
) -> Response {
    let w = ui.available_width().max(132.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
    let hov_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.09);

    if hov_t > 0.01 {
        let hover_bg = if danger { p.danger_soft } else { p.bg_hover };
        ui.painter()
            .rect_filled(rect, Rounding::same(5.0), with_alpha(hover_bg, hov_t));
    }

    let fg = if danger {
        p.danger
    } else if resp.hovered() {
        p.text_primary
    } else {
        p.text_secondary
    };

    let icon_r = Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(13.5, 13.5));
    draw_icon(ui.painter(), icon_r, icon, fg);

    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.5), fg);
    ui.painter().galley(
        pos2(rect.left() + 28.0, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );

    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn keycap_badge(ui: &mut Ui, p: &ThemePalette, text: &str) {
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_mono(11.0), p.text_secondary);
    let (rect, _) = ui.allocate_exact_size(vec2(g.size().x + 12.0, 20.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(4.5), p.bg_input);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(4.5),
        Stroke::new(1.0_f32, p.border_subtle),
    );
    ui.painter().galley(
        pos2(
            rect.center().x - g.size().x * 0.5,
            rect.center().y - g.size().y * 0.5,
        ),
        g,
        p.text_secondary,
    );
}

pub fn section_heading(ui: &mut Ui, p: &ThemePalette, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .font(font_medium(12.0))
            .color(p.text_muted),
    );
}

pub fn paint_badge_dot(painter: &Painter, center: Pos2, color: Color32) {
    painter.circle_filled(center, 3.2, color);
}

pub fn shortcut_label(base_ctrl_key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{base_ctrl_key}")
    } else {
        format!("Ctrl+{base_ctrl_key}")
    }
}
