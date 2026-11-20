use eframe::egui::{self, Color32};
use std::time::Instant;

#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[inline]
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
    }
}

/// Subtle overshoot (~5%) for checkboxes, badges, and toggle check indicators.
#[inline]
pub fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.35_f32;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

#[inline]
pub fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[inline]
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(
        lerp_f32(ar as f32, br as f32, t).round() as u8,
        lerp_f32(ag as f32, bg as f32, t).round() as u8,
        lerp_f32(ab as f32, bb as f32, t).round() as u8,
        lerp_f32(aa as f32, ba as f32, t).round() as u8,
    )
}

#[inline]
pub fn with_alpha(c: Color32, factor: f32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let na = ((a as f32) * factor.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_unmultiplied(r, g, b, na)
}

pub fn anim_bool_eased(
    ctx: &egui::Context,
    id: egui::Id,
    target: bool,
    duration_secs: f32,
) -> f32 {
    let raw = ctx.animate_bool_with_time(id, target, duration_secs);
    if raw > 0.001 && raw < 0.999 {
        ctx.request_repaint();
    }
    ease_out_cubic(raw)
}

pub fn anim_value_smooth(
    ctx: &egui::Context,
    id: egui::Id,
    target: f32,
    duration_secs: f32,
) -> f32 {
    let val = ctx.animate_value_with_time(id, target, duration_secs);
    if (val - target).abs() > 0.25 {
        ctx.request_repaint();
    }
    val
}

/// Time-based entrance progress in `0.0..=1.0`, returning `1.0` once settled (0% CPU at idle).
pub fn anim_since(
    ctx: &egui::Context,
    started_at: Option<Instant>,
    duration_secs: f32,
) -> f32 {
    let Some(at) = started_at else {
        return 1.0;
    };
    let dur = duration_secs.max(0.01);
    let raw = (at.elapsed().as_secs_f32() / dur).clamp(0.0, 1.0);
    if raw < 0.999 {
        ctx.request_repaint();
        ease_out_cubic(raw)
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_and_color_interpolation_work_cleanly() {
        assert!((ease_in_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-6);
        assert!((ease_in_out_cubic(1.0) - 1.0).abs() < 1e-6);

        assert!((ease_out_back(0.0) - 0.0).abs() < 1e-6);
        assert!(ease_out_back(0.65) > 1.0);
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-6);

        let opaque_a = Color32::from_rgb(10, 20, 30);
        let opaque_b = Color32::from_rgb(110, 120, 130);
        assert_eq!(lerp_color(opaque_a, opaque_b, 0.0), opaque_a);
        assert_eq!(lerp_color(opaque_a, opaque_b, 1.0), opaque_b);
        assert_eq!(
            lerp_color(opaque_a, opaque_b, 0.5),
            Color32::from_rgb(60, 70, 80)
        );

        let semi_a = Color32::from_rgba_unmultiplied(200, 100, 50, 40);
        let semi_b = Color32::from_rgba_unmultiplied(200, 100, 50, 240);
        assert_eq!(lerp_color(semi_a, semi_b, 0.5).a(), 140);

        let faded = with_alpha(opaque_b, 0.5);
        assert_eq!(faded.a(), 128);
    }
}
