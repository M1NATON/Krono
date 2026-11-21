use crate::i18n::{status_label, strings, Lang};
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
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click_and_drag());
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

/// Optional status pill picker (`None` shows `+ Этап` / `+ Stage`; dropdown allows clearing).
pub fn optional_status_pill_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    id_salt: impl std::hash::Hash,
    current: Option<NoteStatus>,
) -> Option<Option<NoteStatus>> {
    let s = strings(lang);
    let (pill_bg, dot_col, text_col, label) = match current {
        Some(st) => {
            let (bg, dot, fg) = p.status_colors(st);
            (bg, Some(dot), fg, status_label(lang, st))
        }
        None => (p.bg_elevated, None, p.text_muted, s.stage_add_btn),
    };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.0), text_col);
    let extra_w = if dot_col.is_some() { 34.0 } else { 24.0 };
    let w = galley.size().x + extra_w;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let hover_t = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);

    ui.painter().rect_filled(rect, Rounding::same(12.0), pill_bg);
    if current.is_none() || hover_t > 0.01 {
        let stroke_col = if current.is_none() {
            lerp_color(p.border_hairline, p.border_subtle, hover_t)
        } else {
            with_alpha(p.border_subtle, hover_t)
        };
        ui.painter()
            .rect_stroke(rect, Rounding::same(12.0), Stroke::new(1.0_f32, stroke_col));
    }
    let text_x = if let Some(dot) = dot_col {
        ui.painter()
            .circle_filled(pos2(rect.left() + 11.0, rect.center().y), 3.2, dot);
        rect.left() + 19.0
    } else {
        rect.left() + 9.0
    };
    ui.painter().galley(
        pos2(text_x, rect.center().y - galley.size().y * 0.5),
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

    let mut chosen: Option<Option<NoteStatus>> = None;
    egui::popup_below_widget(
        ui,
        popup_id,
        &resp,
        PopupCloseBehavior::CloseOnClick,
        |ui| {
            ui.set_min_width(156.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            for opt in NoteStatus::ALL {
                let (obg, odot, otext) = p.status_colors(opt);
                let olbl = status_label(lang, opt);
                let og = ui
                    .painter()
                    .layout_no_wrap(olbl.to_string(), font_medium(12.5), otext);
                let (orect, oresp) =
                    ui.allocate_exact_size(vec2(150.0, 26.0), Sense::click());
                let active = current == Some(opt);
                if oresp.hovered() || active {
                    ui.painter().rect_filled(
                        orect,
                        Rounding::same(5.0),
                        if active { obg } else { p.bg_hover },
                    );
                }
                ui.painter()
                    .circle_filled(pos2(orect.left() + 10.0, orect.center().y), 3.4, odot);
                ui.painter().galley(
                    pos2(orect.left() + 20.0, orect.center().y - og.size().y * 0.5),
                    og,
                    if active { otext } else { p.text_primary },
                );
                if active {
                    let chk = Rect::from_center_size(
                        pos2(orect.right() - 12.0, orect.center().y),
                        vec2(12.0, 12.0),
                    );
                    draw_icon(ui.painter(), chk, Icon::Check, otext);
                }
                if oresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    chosen = Some(Some(opt));
                }
            }
            if current.is_some() {
                ui.add_space(2.0);
                hairline_h(ui, p);
                ui.add_space(2.0);
                let og = ui.painter().layout_no_wrap(
                    s.stage_clear_btn.to_string(),
                    font_medium(12.0),
                    p.text_muted,
                );
                let (orect, oresp) =
                    ui.allocate_exact_size(vec2(150.0, 24.0), Sense::click());
                if oresp.hovered() {
                    ui.painter()
                        .rect_filled(orect, Rounding::same(5.0), p.bg_hover);
                }
                ui.painter().galley(
                    pos2(orect.left() + 10.0, orect.center().y - og.size().y * 0.5),
                    og,
                    if oresp.hovered() {
                        p.text_primary
                    } else {
                        p.text_muted
                    },
                );
                if oresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    chosen = Some(None);
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

/// Normalizes a tag draft: trims, strips leading `#`, collapses whitespace, and lowercases.
pub fn normalize_tag_input(raw: &str) -> String {
    let stripped = raw.trim().trim_start_matches('#').trim();
    if stripped.is_empty() {
        return String::new();
    }
    stripped
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Filters existing tags for autocomplete: prefix matches first, then substring matches,
/// excluding `already_added` (case-insensitive). Returns `(matches, create_option)`.
pub fn filter_tag_suggestions(
    query: &str,
    existing_tags: &[String],
    already_added: &[String],
) -> (Vec<String>, Option<String>) {
    let norm_q = normalize_tag_input(query);
    let mut seen: std::collections::HashSet<String> = already_added
        .iter()
        .map(|t| normalize_tag_input(t))
        .filter(|t| !t.is_empty())
        .collect();

    let mut prefix_hits = Vec::new();
    let mut sub_hits = Vec::new();
    let mut exact_exists = false;

    for raw in existing_tags {
        let trimmed = raw.trim().trim_start_matches('#').trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = normalize_tag_input(trimmed);
        if !norm_q.is_empty() && lower == norm_q {
            exact_exists = true;
        }
        if !seen.insert(lower.clone()) {
            continue;
        }
        if norm_q.is_empty() {
            prefix_hits.push(trimmed.to_string());
        } else if lower.starts_with(&norm_q) {
            prefix_hits.push(trimmed.to_string());
        } else if lower.contains(&norm_q) {
            sub_hits.push(trimmed.to_string());
        }
    }

    prefix_hits.sort_by_key(|a| a.to_lowercase());
    sub_hits.sort_by_key(|a| a.to_lowercase());
    prefix_hits.extend(sub_hits);
    prefix_hits.truncate(8);

    let create_opt = if !norm_q.is_empty()
        && !exact_exists
        && !already_added
            .iter()
            .any(|t| normalize_tag_input(t) == norm_q)
    {
        Some(norm_q)
    } else {
        None
    };

    (prefix_hits, create_opt)
}

/// Tag input with keyboard-navigable autocomplete dropdown (`Up`/`Down` + `Enter`/`Tab` + click).
/// Consumes `Enter`/`Tab`/`Escape` while focused so parent modals or quick-add forms do not submit/close.
#[allow(clippy::too_many_arguments)]
pub fn tag_autocomplete_input(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    id_salt: impl std::hash::Hash,
    draft: &mut String,
    hint: &str,
    width: f32,
    existing_tags: &[String],
    already_added: &[String],
) -> Option<String> {
    let s = strings(lang);
    let base_id = ui.make_persistent_id(id_salt);
    let input_id = base_id.with("inp");
    let sel_id = base_id.with("sel");
    let popup_id = base_id.with("pop");
    let open_id = base_id.with("open");
    let pop_rect_id = base_id.with("pop_rect");

    let has_focus_before = ui.ctx().memory(|m| m.has_focus(input_id));
    let was_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let prev_pop_rect = ui
        .data(|d| d.get_temp::<Option<Rect>>(pop_rect_id))
        .flatten();
    let (mut key_up, mut key_down, mut key_accept, mut key_esc) = (false, false, false, false);
    if has_focus_before || was_open {
        ui.input_mut(|i| {
            key_up = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
            key_down = i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
            key_accept = i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
            key_esc = i.consume_key(egui::Modifiers::NONE, egui::Key::Escape);
        });
    }

    let bg_slot = ui.painter().add(egui::Shape::Noop);
    let frame_resp = egui::Frame::none()
        .inner_margin(Margin::symmetric(9.0, 5.0))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(draft)
                    .id(input_id)
                    .hint_text(hint)
                    .frame(false)
                    .font(font_body(12.5))
                    .desired_width(width),
            )
        });
    let resp = frame_resp.inner;
    let outer_rect = frame_resp.response.rect;

    if frame_resp.response.clicked()
        || ui.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|pt| outer_rect.contains(pt) && !resp.rect.contains(pt))
        })
    {
        resp.request_focus();
    }

    let is_focused = resp.has_focus() || ui.ctx().memory(|m| m.has_focus(input_id));
    let mut is_open = was_open || is_focused || resp.gained_focus() || resp.changed();
    if is_open && ui.input(|i| i.pointer.any_pressed()) {
        let pt = ui.input(|i| i.pointer.interact_pos());
        let in_input = pt.is_some_and(|p| outer_rect.expand(2.0).contains(p));
        let in_popup = pt.is_some_and(|p| prev_pop_rect.is_some_and(|r| r.expand(2.0).contains(p)));
        if !in_input && !in_popup {
            is_open = false;
            if is_focused {
                ui.memory_mut(|m| m.surrender_focus(input_id));
            }
        }
    }

    let focus_t = anim_bool_eased(ui.ctx(), base_id.with("foc"), is_focused || is_open, 0.13);
    let hov_t = anim_bool_eased(
        ui.ctx(),
        base_id.with("hov"),
        resp.hovered() || frame_resp.response.hovered(),
        0.10,
    );
    let rest_stroke = lerp_color(p.border_subtle, p.border_strong, hov_t);
    let stroke_col = lerp_color(rest_stroke, p.accent, focus_t);
    ui.painter().set(
        bg_slot,
        egui::Shape::rect_filled(outer_rect, Rounding::same(7.0), p.bg_input),
    );
    ui.painter().rect_stroke(
        outer_rect,
        Rounding::same(7.0),
        Stroke::new(1.0 + 0.25 * focus_t, stroke_col),
    );

    if key_esc {
        ui.data_mut(|d| {
            d.insert_temp(open_id, false);
            d.insert_temp(pop_rect_id, Option::<Rect>::None);
        });
        ui.memory_mut(|m| m.surrender_focus(input_id));
        return None;
    }

    let (matches, create_opt) = filter_tag_suggestions(draft, existing_tags, already_added);
    let total_items = matches.len() + usize::from(create_opt.is_some());
    let mut sel_idx = ui.data(|d| d.get_temp::<usize>(sel_id)).unwrap_or(0);
    if total_items > 0 {
        if sel_idx >= total_items {
            sel_idx = 0;
        }
        if key_down {
            sel_idx = (sel_idx + 1) % total_items;
        } else if key_up {
            sel_idx = if sel_idx == 0 {
                total_items - 1
            } else {
                sel_idx - 1
            };
        }
    } else {
        sel_idx = 0;
    }
    ui.data_mut(|d| d.insert_temp(sel_id, sel_idx));

    let mut chosen: Option<String> = None;
    if key_accept {
        if sel_idx < matches.len() {
            chosen = Some(matches[sel_idx].clone());
        } else if let Some(ref created) = create_opt {
            chosen = Some(created.clone());
        } else {
            let norm = normalize_tag_input(draft);
            if !norm.is_empty()
                && !already_added
                    .iter()
                    .any(|t| normalize_tag_input(t) == norm)
            {
                chosen = Some(norm);
            }
        }
        if chosen.is_some() {
            draft.clear();
            ui.data_mut(|d| {
                d.insert_temp(sel_id, 0usize);
                d.insert_temp(open_id, true);
            });
            ui.memory_mut(|m| m.request_focus(input_id));
            return chosen;
        }
    }

    if is_open && total_items > 0 {
        let pop_w = outer_rect.width().max(196.0);
        let est_h = (total_items as f32) * 27.0 + 10.0;
        let screen_bottom = ui.ctx().screen_rect().bottom();
        let pop_y = if outer_rect.bottom() + 4.0 + est_h > screen_bottom - 8.0
            && outer_rect.top() - 4.0 - est_h >= 8.0
        {
            outer_rect.top() - 4.0 - est_h
        } else {
            outer_rect.bottom() + 4.0
        };
        let pop_area_resp = egui::Area::new(popup_id)
            .order(egui::Order::Tooltip)
            .interactable(true)
            .fixed_pos(pos2(outer_rect.left(), pop_y))
            .show(ui.ctx(), |ui| {
                egui::Frame::none()
                    .fill(p.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, p.border_subtle))
                    .rounding(Rounding::same(8.0))
                    .shadow(p.shadow_floating)
                    .inner_margin(Margin::same(4.0))
                    .show(ui, |ui| {
                        ui.set_width(pop_w);
                        ui.spacing_mut().item_spacing.y = 2.0;

                        for (idx, tag) in matches.iter().enumerate() {
                            let active = idx == sel_idx;
                            let (bg, fg) = p.tag_colors(tag);
                            let (r, r_resp) =
                                ui.allocate_exact_size(vec2(pop_w - 8.0, 25.0), Sense::click());
                            if r_resp.hovered() {
                                ui.data_mut(|d| d.insert_temp(sel_id, idx));
                            }
                            if active || r_resp.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    Rounding::same(5.0),
                                    if active { p.bg_selected } else { p.bg_hover },
                                );
                            }
                            let dot_r = Rect::from_center_size(
                                pos2(r.left() + 12.0, r.center().y),
                                vec2(8.0, 8.0),
                            );
                            ui.painter().rect_filled(dot_r, Rounding::same(2.0), bg);
                            ui.painter().circle_filled(dot_r.center(), 2.2, fg);
                            let g = ui.painter().layout_no_wrap(
                                format!("#{tag}"),
                                font_medium(12.0),
                                p.text_primary,
                            );
                            ui.painter().galley(
                                pos2(r.left() + 22.0, r.center().y - g.size().y * 0.5),
                                g,
                                p.text_primary,
                            );
                            if r_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                chosen = Some(tag.clone());
                            }
                        }

                        if let Some(ref new_tag) = create_opt {
                            let idx = matches.len();
                            let active = idx == sel_idx;
                            let (r, r_resp) =
                                ui.allocate_exact_size(vec2(pop_w - 8.0, 26.0), Sense::click());
                            if r_resp.hovered() {
                                ui.data_mut(|d| d.insert_temp(sel_id, idx));
                            }
                            if active || r_resp.hovered() {
                                ui.painter().rect_filled(
                                    r,
                                    Rounding::same(5.0),
                                    if active { p.accent_soft } else { p.bg_hover },
                                );
                            }
                            let lbl = format!("{} \"{}\"", s.create_tag_prefix, new_tag);
                            let g = ui.painter().layout_no_wrap(
                                lbl,
                                font_medium(12.0),
                                p.accent_text,
                            );
                            ui.painter().galley(
                                pos2(r.left() + 8.0, r.center().y - g.size().y * 0.5),
                                g,
                                p.accent_text,
                            );
                            if r_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                chosen = Some(new_tag.clone());
                            }
                        }
                    });
            });
        ui.data_mut(|d| d.insert_temp(pop_rect_id, Some(pop_area_resp.response.rect)));
    } else {
        ui.data_mut(|d| d.insert_temp(pop_rect_id, Option::<Rect>::None));
    }

    if chosen.is_some() {
        draft.clear();
        ui.data_mut(|d| {
            d.insert_temp(sel_id, 0usize);
            d.insert_temp(open_id, true);
        });
        ui.memory_mut(|m| m.request_focus(input_id));
    } else {
        ui.data_mut(|d| d.insert_temp(open_id, is_open));
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_autocomplete_filters_orders_and_offers_create() {
        let existing = vec![
            "backend".to_string(),
            "База".to_string(),
            "баг".to_string(),
            "архив".to_string(),
            "ui-ux".to_string(),
        ];
        let already = vec!["База".to_string()];

        let (m, create) = filter_tag_suggestions("  #БА ", &existing, &already);
        assert_eq!(m, vec!["баг"]);
        assert_eq!(create, Some("ба".to_string()));

        let (m2, create2) = filter_tag_suggestions("баг", &existing, &already);
        assert_eq!(m2, vec!["баг"]);
        assert_eq!(create2, None);

        let (m3, create3) = filter_tag_suggestions("  Новый   Тег ", &existing, &already);
        assert!(m3.is_empty());
        assert_eq!(create3, Some("новый тег".to_string()));
    }
}
