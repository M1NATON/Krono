use crate::ui::theme::font_lucide;
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, Painter, Pos2, Rect, Rounding, Stroke,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Logo,
    Document,
    Folder,
    FolderOpen,
    ChevronDown,
    Search,
    Plus,
    Table,
    Board,
    Notes,
    Sync,
    Settings,
    SidebarToggle,
    InspectorToggle,
    Sun,
    Moon,
    Edit,
    Split,
    Eye,
    Link,
    OutLink,
    Trash,
    Copy,
    Check,
    Close,
    Alert,
    GitHub,
    Globe,
    ArrowLeft,
    Palette,
    Type,
    HardDrive,
    Keyboard,
    ExternalLink,
    RotateCcw,
    EyeOff,
    Heading1,
    Heading2,
    Heading3,
    Bold,
    Italic,
    Code,
    FileCode,
    Task,
    List,
    ListOrdered,
    Quote,
    Minus,
    CalendarDays,
    Pin,
    PinOff,
    ChevronLeft,
    ChevronRight,
    CircleHelp,
    Flame,
    Sparkles,
    Graph,
    Canvas,
    StickyNote,
    FilePlus,
    Group,
    FileOutput,
    ZoomIn,
    ZoomOut,
    Maximize,
    Magnet,
    Filter,
    Undo,
    Redo,
    Sort,
    Map,
    Image,
    Star,
    SwapArrows,
    LayoutGrid,
    SelectBox,
}

fn lucide_char(icon: Icon) -> Option<char> {
    match icon {
        Icon::Logo => None,
        Icon::Document => Some('\u{e0cc}'),        // file-text
        Icon::Folder => Some('\u{e0d7}'),          // folder
        Icon::FolderOpen => Some('\u{e247}'),      // folder-open
        Icon::ChevronDown => Some('\u{e06d}'),     // chevron-down
        Icon::ChevronLeft => Some('\u{e06e}'),     // chevron-left
        Icon::ChevronRight => Some('\u{e06f}'),    // chevron-right
        Icon::Search => Some('\u{e151}'),          // search
        Icon::Plus => Some('\u{e13d}'),            // plus
        Icon::Table => Some('\u{e2f9}'),           // table-2
        Icon::Board => Some('\u{e4dc}'),           // kanban
        Icon::Notes => Some('\u{e05f}'),           // book-open
        Icon::Sync => Some('\u{e145}'),            // refresh-cw
        Icon::Settings => Some('\u{e154}'),        // settings
        Icon::SidebarToggle => Some('\u{e12a}'),   // panel-left
        Icon::InspectorToggle => Some('\u{e431}'), // panel-right
        Icon::Sun => Some('\u{e178}'),             // sun
        Icon::Moon => Some('\u{e11e}'),            // moon
        Icon::Edit => Some('\u{e1f9}'),            // pencil
        Icon::Split => Some('\u{e098}'),           // columns-2
        Icon::Eye => Some('\u{e0ba}'),             // eye
        Icon::Link => Some('\u{e103}'),            // link-2
        Icon::OutLink => Some('\u{e04d}'),         // arrow-up-right
        Icon::Trash => Some('\u{e18e}'),           // trash-2
        Icon::Copy => Some('\u{e09e}'),            // copy
        Icon::Check => Some('\u{e06c}'),           // check
        Icon::Close => Some('\u{e1b2}'),           // x
        Icon::Alert => Some('\u{e193}'),           // triangle-alert
        Icon::GitHub => Some('\u{e0e2}'),          // git-branch
        Icon::Globe => Some('\u{e0e8}'),           // globe
        Icon::ArrowLeft => Some('\u{e048}'),       // arrow-left
        Icon::Palette => Some('\u{e1dd}'),         // palette
        Icon::Type => Some('\u{e198}'),            // type
        Icon::HardDrive => Some('\u{e0ed}'),       // hard-drive
        Icon::Keyboard => Some('\u{e284}'),        // keyboard
        Icon::ExternalLink => Some('\u{e0b9}'),    // external-link
        Icon::RotateCcw => Some('\u{e148}'),       // rotate-ccw
        Icon::EyeOff => Some('\u{e0bb}'),          // eye-off
        Icon::Heading1 => Some('\u{e385}'),        // heading-1
        Icon::Heading2 => Some('\u{e386}'),        // heading-2
        Icon::Heading3 => Some('\u{e387}'),        // heading-3
        Icon::Bold => Some('\u{e05d}'),            // bold
        Icon::Italic => Some('\u{e0fb}'),          // italic
        Icon::Code => Some('\u{e093}'),            // code
        Icon::FileCode => Some('\u{e0c3}'),        // file-code
        Icon::Task => Some('\u{e559}'),            // square-check
        Icon::List => Some('\u{e106}'),            // list
        Icon::ListOrdered => Some('\u{e1d1}'),     // list-ordered
        Icon::Quote => Some('\u{e239}'),           // quote
        Icon::Minus => Some('\u{e11c}'),           // minus
        Icon::CalendarDays => Some('\u{e2b9}'),    // calendar-days
        Icon::Pin => Some('\u{e259}'),             // pin
        Icon::PinOff => Some('\u{e2b6}'),          // pin-off
        Icon::CircleHelp => Some('\u{e082}'),      // circle-help
        Icon::Flame => Some('\u{e0d2}'),           // flame
        Icon::Sparkles => Some('\u{e412}'),        // sparkles
        Icon::Graph => Some('\u{e542}'),           // waypoints
        Icon::Canvas => Some('\u{e425}'),          // workflow
        Icon::StickyNote => Some('\u{e303}'),      // sticky-note
        Icon::FilePlus => Some('\u{e0c9}'),        // file-plus
        Icon::Group => Some('\u{e464}'),           // group
        Icon::FileOutput => Some('\u{e0c8}'),      // file-output
        Icon::ZoomIn => Some('\u{e1b6}'),          // zoom-in
        Icon::ZoomOut => Some('\u{e1b7}'),         // zoom-out
        Icon::Maximize => Some('\u{e112}'),        // maximize
        Icon::Magnet => Some('\u{e2b5}'),          // magnet
        Icon::Filter => Some('\u{e0dc}'),          // filter
        Icon::Undo
        | Icon::Redo
        | Icon::Sort
        | Icon::Map
        | Icon::Image
        | Icon::Star
        | Icon::SwapArrows
        | Icon::LayoutGrid
        | Icon::SelectBox => None,
    }
}

pub fn draw_icon(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    if let Some(ch) = lucide_char(icon) {
        let size = rect.width().min(rect.height()).max(10.0);
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            s,
            font_lucide(size),
            color,
        );
        return;
    }

    let side = rect.width().min(rect.height());
    let sw = (side * 0.11).clamp(1.25, 1.8);
    let c = rect.center();

    match icon {
        Icon::Undo | Icon::Redo => {
            // Sharp U-turn arrow: points left for Undo (dir = -1.0), right for Redo (dir = 1.0)
            let dir = if icon == Icon::Undo { -1.0_f32 } else { 1.0_f32 };
            let r = side * 0.22;
            let top_y = c.y - r * 0.65;
            let bot_y = c.y + r * 1.15;
            let tip = pos2(c.x + dir * side * 0.34, top_y);
            let turn_x = c.x - dir * side * 0.08;
            let arc_cy = (top_y + bot_y) * 0.5;
            let arc_r = (bot_y - top_y) * 0.5;
            let mut pts = Vec::with_capacity(11);
            pts.push(tip);
            pts.push(pos2(turn_x, top_y));
            for i in 1..=7 {
                let t = i as f32 / 8.0;
                // Sweep from top (-PI/2) around opposite side (-dir) to bottom (+PI/2)
                let a = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::PI * (-dir);
                pts.push(pos2(turn_x + a.cos() * arc_r, arc_cy + a.sin() * arc_r));
            }
            pts.push(pos2(turn_x, bot_y));
            pts.push(pos2(c.x + dir * side * 0.22, bot_y));
            painter.add(egui::Shape::line(pts, Stroke::new(sw, color)));
            let wing = side * 0.17;
            painter.line_segment(
                [tip, pos2(tip.x - dir * wing, tip.y - wing)],
                Stroke::new(sw, color),
            );
            painter.line_segment(
                [tip, pos2(tip.x - dir * wing, tip.y + wing)],
                Stroke::new(sw, color),
            );
            return;
        }
        Icon::Sort => {
            let x1 = c.x - side * 0.28;
            let y1 = c.y - side * 0.24;
            let y2 = c.y;
            let y3 = c.y + side * 0.24;
            painter.line_segment([pos2(x1, y1), pos2(c.x + side * 0.30, y1)], Stroke::new(sw, color));
            painter.line_segment([pos2(x1, y2), pos2(c.x + side * 0.12, y2)], Stroke::new(sw, color));
            painter.line_segment([pos2(x1, y3), pos2(c.x - side * 0.06, y3)], Stroke::new(sw, color));
            return;
        }
        Icon::Map => {
            let w = side * 0.78;
            let h = side * 0.62;
            let box_r = Rect::from_center_size(c, vec2(w, h));
            painter.rect_stroke(box_r, Rounding::same(2.0), Stroke::new(sw * 0.9, color));
            let mini_r = Rect::from_min_size(
                pos2(box_r.right() - w * 0.42, box_r.bottom() - h * 0.48),
                vec2(w * 0.32, h * 0.34),
            );
            painter.rect_filled(mini_r, Rounding::same(1.0), color);
            return;
        }
        Icon::Image => {
            let w = side * 0.78;
            let h = side * 0.64;
            let frame_r = Rect::from_center_size(c, vec2(w, h));
            painter.rect_stroke(frame_r, Rounding::same(2.2), Stroke::new(sw * 0.9, color));
            painter.circle_filled(
                pos2(frame_r.left() + w * 0.27, frame_r.top() + h * 0.31),
                (side * 0.08).max(1.3),
                color,
            );
            let pts = vec![
                pos2(frame_r.left() + w * 0.14, frame_r.bottom() - h * 0.16),
                pos2(frame_r.left() + w * 0.42, frame_r.top() + h * 0.48),
                pos2(frame_r.left() + w * 0.60, frame_r.bottom() - h * 0.30),
                pos2(frame_r.left() + w * 0.75, frame_r.top() + h * 0.42),
                pos2(frame_r.right() - w * 0.12, frame_r.bottom() - h * 0.16),
            ];
            painter.add(egui::Shape::line(pts, Stroke::new(sw * 0.85, color)));
            return;
        }
        Icon::Star => {
            paint_star(painter, c, side * 0.44, color, None);
            return;
        }
        Icon::SwapArrows => {
            let dx = side * 0.30;
            let dy = side * 0.18;
            let wing = side * 0.13;
            let top_l = pos2(c.x - dx, c.y - dy);
            let top_r = pos2(c.x + dx, c.y - dy);
            painter.line_segment([top_l, top_r], Stroke::new(sw * 0.9, color));
            painter.line_segment([top_r, pos2(top_r.x - wing, top_r.y - wing)], Stroke::new(sw * 0.9, color));
            painter.line_segment([top_r, pos2(top_r.x - wing, top_r.y + wing)], Stroke::new(sw * 0.9, color));
            let bot_l = pos2(c.x - dx, c.y + dy);
            let bot_r = pos2(c.x + dx, c.y + dy);
            painter.line_segment([bot_l, bot_r], Stroke::new(sw * 0.9, color));
            painter.line_segment([bot_l, pos2(bot_l.x + wing, bot_l.y - wing)], Stroke::new(sw * 0.9, color));
            painter.line_segment([bot_l, pos2(bot_l.x + wing, bot_l.y + wing)], Stroke::new(sw * 0.9, color));
            return;
        }
        Icon::LayoutGrid => {
            let gap = side * 0.12;
            let cell = side * 0.30;
            let x0 = c.x - gap * 0.5 - cell;
            let x1 = c.x + gap * 0.5;
            let y0 = c.y - gap * 0.5 - cell;
            let y1 = c.y + gap * 0.5;
            let stroke = Stroke::new(sw * 0.9, color);
            for (rx, ry) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                painter.rect_stroke(
                    Rect::from_min_size(pos2(rx, ry), vec2(cell, cell)),
                    Rounding::same((side * 0.08).max(1.5)),
                    stroke,
                );
            }
            return;
        }
        Icon::SelectBox => {
            let b = Rect::from_center_size(c, vec2(side * 0.68, side * 0.62));
            let dash = Stroke::new(sw * 0.9, color);
            let seg = side * 0.17;
            // Corner brackets representing marquee selection
            painter.line_segment([b.left_top(), pos2(b.left() + seg, b.top())], dash);
            painter.line_segment([b.left_top(), pos2(b.left(), b.top() + seg)], dash);
            painter.line_segment([b.right_top(), pos2(b.right() - seg, b.top())], dash);
            painter.line_segment([b.right_top(), pos2(b.right(), b.top() + seg)], dash);
            painter.line_segment([b.left_bottom(), pos2(b.left() + seg, b.bottom())], dash);
            painter.line_segment([b.left_bottom(), pos2(b.left(), b.bottom() - seg)], dash);
            painter.line_segment([b.right_bottom(), pos2(b.right() - seg, b.bottom())], dash);
            painter.line_segment([b.right_bottom(), pos2(b.right(), b.bottom() - seg)], dash);
            return;
        }
        _ => {}
    }

    // Bespoke Krono geometric "K" monogram inside clean rounded square
    let side = rect.width().min(rect.height());
    let badge = Rect::from_center_size(rect.center(), vec2(side, side));
    let r = (side * 0.24).clamp(3.0, 7.0);
    let sw = (side * 0.085).clamp(1.3, 2.2);
    painter.rect_stroke(badge, Rounding::same(r), Stroke::new(sw, color));

    let x_left = badge.left() + side * 0.34;
    let x_right = badge.left() + side * 0.68;
    let y_top = badge.top() + side * 0.27;
    let y_mid = badge.center().y;
    let y_bot = badge.bottom() - side * 0.27;

    painter.line_segment(
        [pos2(x_left, y_top), pos2(x_left, y_bot)],
        Stroke::new(sw * 1.05, color),
    );
    painter.line_segment(
        [pos2(x_right, y_top), pos2(x_left + sw * 0.4, y_mid)],
        Stroke::new(sw * 1.05, color),
    );
    painter.line_segment(
        [pos2(x_left + sw * 0.4, y_mid), pos2(x_right, y_bot)],
        Stroke::new(sw * 1.05, color),
    );
}

/// Draws a crisp 5-pointed star using 10 triangles around `center` (concave polygon safe).
pub fn paint_star(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    stroke: Option<Stroke>,
) {
    use std::f32::consts::{FRAC_PI_2, TAU};
    let inner_r = radius * 0.42;
    let mut outer = [Pos2::ZERO; 5];
    let mut inner = [Pos2::ZERO; 5];
    let mut ring = Vec::with_capacity(10);

    for i in 0..5 {
        let a_out = -FRAC_PI_2 + (i as f32) * (TAU / 5.0);
        let a_in = a_out + TAU / 10.0;
        let po = pos2(center.x + radius * a_out.cos(), center.y + radius * a_out.sin());
        let pi = pos2(center.x + inner_r * a_in.cos(), center.y + inner_r * a_in.sin());
        outer[i] = po;
        inner[i] = pi;
        ring.push(po);
        ring.push(pi);
    }

    if fill != Color32::TRANSPARENT {
        for i in 0..5 {
            painter.add(egui::Shape::convex_polygon(
                vec![center, outer[i], inner[i]],
                fill,
                Stroke::NONE,
            ));
            painter.add(egui::Shape::convex_polygon(
                vec![center, inner[i], outer[(i + 1) % 5]],
                fill,
                Stroke::NONE,
            ));
        }
    }
    if let Some(st) = stroke {
        painter.add(egui::Shape::closed_line(ring, st));
    }
}

pub fn draw_sync_icon(painter: &Painter, rect: Rect, color: Color32, angle_rad: f32) {
    if angle_rad.abs() < 0.001 {
        draw_icon(painter, rect, Icon::Sync, color);
        return;
    }

    // Smooth active spinner during background GitHub sync
    let c = rect.center();
    let rad = rect.width().min(rect.height()) * 0.36;
    let w = (rect.width().min(rect.height()) / 10.5).clamp(1.4, 2.0);
    let stroke = Stroke::new(w, color);

    let rot = |a: f32, r: f32| -> Pos2 {
        let th = a + angle_rad;
        c + vec2(th.cos() * r, th.sin() * r)
    };

    let mut arc1 = Vec::with_capacity(9);
    let mut arc2 = Vec::with_capacity(9);
    for i in 0..=8 {
        let t = i as f32 / 8.0;
        arc1.push(rot(-0.3 + t * 2.2, rad));
        arc2.push(rot(std::f32::consts::PI - 0.3 + t * 2.2, rad));
    }
    painter.add(eframe::egui::Shape::line(arc1, stroke));
    painter.add(eframe::egui::Shape::line(arc2, stroke));
}

/// Generates a crisp 32x32 RGBA window icon for Windows / macOS dock & titlebar.
pub fn build_window_icon() -> egui::IconData {
    let size = 32usize;
    let mut rgba = vec![0u8; size * size * 4];

    let set_px = |buf: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8, a: u8| {
        if x < size && y < size {
            let idx = (y * size + x) * 4;
            buf[idx] = r;
            buf[idx + 1] = g;
            buf[idx + 2] = b;
            buf[idx + 3] = a;
        }
    };

    // Rounded graphite square background (#1B1B19) with subtle rim (#42413C)
    let cx = 15.5_f32;
    let cy = 15.5_f32;
    let half = 13.5_f32;
    let rad = 5.5_f32;

    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - cx).abs() - (half - rad);
            let dy = (y as f32 - cy).abs() - (half - rad);
            let dist = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - rad;

            if dist <= 0.6 {
                let edge_alpha = ((0.6 - dist).clamp(0.0, 1.0) * 255.0) as u8;
                if dist > -1.3 {
                    set_px(&mut rgba, x, y, 78, 76, 70, edge_alpha);
                } else {
                    set_px(&mut rgba, x, y, 24, 24, 22, edge_alpha);
                }
            }
        }
    }

    // Crisp editorial "K" inside in warm alabaster (#EDEBE6) with steel accent (#5A8BE0) upper arm
    let dist_seg = |px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32| -> f32 {
        let vx = bx - ax;
        let vy = by - ay;
        let wx = px - ax;
        let wy = py - ay;
        let len2 = vx * vx + vy * vy;
        let t = if len2 > 0.0 {
            ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (px - (ax + t * vx)).hypot(py - (ay + t * vy))
    };

    for y in 0..size {
        for x in 0..size {
            let px = x as f32;
            let py = y as f32;
            let d_stem = dist_seg(px, py, 10.5, 8.5, 10.5, 22.5);
            let d_up = dist_seg(px, py, 12.2, 16.0, 21.0, 8.5);
            let d_down = dist_seg(px, py, 12.2, 15.0, 21.2, 22.5);

            if d_stem <= 1.35 || d_down <= 1.35 {
                set_px(&mut rgba, x, y, 237, 235, 230, 255);
            } else if d_up <= 1.35 {
                set_px(&mut rgba, x, y, 102, 150, 232, 255);
            }
        }
    }

    egui::IconData {
        rgba,
        width: size as u32,
        height: size as u32,
    }
}
