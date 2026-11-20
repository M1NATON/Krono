use crate::i18n::{strings, Lang};
use crate::storage::vault::{file_mtime_and_size, full_vault_path};
use crate::ui::components::icon_button;
use crate::ui::icons::Icon;
use crate::ui::theme::{font_medium, font_mono, ThemePalette};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Rect, Rounding, Sense, Stroke, TextureHandle,
    TextureId, TextureOptions, Vec2,
};
use image::{codecs::png::PngEncoder, ImageEncoder};
use krono_core::path::VaultFileKind;
use std::{collections::HashMap, fs, io::Cursor, path::Path};

struct CachedImage {
    modified: i64,
    size: u64,
    demo_pinned: bool,
    texture: Option<TextureHandle>,
    size_px: [usize; 2],
}

#[derive(Default)]
pub struct ImageTextureCache {
    entries: HashMap<String, CachedImage>,
}

impl ImageTextureCache {
    #[cfg(debug_assertions)]
    pub fn insert_demo_texture(
        &mut self,
        ctx: &egui::Context,
        rel_path: &str,
        color_img: egui::ColorImage,
    ) {
        let norm_rel = rel_path.trim().replace('\\', "/");
        let size_px = color_img.size;
        let handle = ctx.load_texture(
            format!("krono_demo_img:{norm_rel}"),
            color_img,
            TextureOptions::LINEAR,
        );
        self.entries.insert(
            norm_rel,
            CachedImage {
                modified: i64::MAX,
                size: 0,
                demo_pinned: true,
                texture: Some(handle),
                size_px,
            },
        );
    }

    pub fn get_or_load(
        &mut self,
        ctx: &egui::Context,
        vault_dir: &Path,
        rel_path: &str,
    ) -> Option<(TextureId, Vec2)> {
        let norm_rel = rel_path.trim().replace('\\', "/");
        if VaultFileKind::from_path(&norm_rel) != Some(VaultFileKind::Asset) {
            return None;
        }
        if let Some(cached) = self.entries.get(&norm_rel) {
            if cached.demo_pinned {
                return cached
                    .texture
                    .as_ref()
                    .map(|t| (t.id(), vec2(cached.size_px[0] as f32, cached.size_px[1] as f32)));
            }
        }
        let full = full_vault_path(vault_dir, &norm_rel).ok()?;
        let (modified, size) = file_mtime_and_size(&full);
        if size == 0 {
            self.entries.remove(&norm_rel);
            return None;
        }

        if let Some(cached) = self.entries.get(&norm_rel) {
            if !cached.demo_pinned && cached.modified == modified && cached.size == size {
                return cached
                    .texture
                    .as_ref()
                    .map(|t| (t.id(), vec2(cached.size_px[0] as f32, cached.size_px[1] as f32)));
            }
        }

        let loaded = fs::read(&full)
            .ok()
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
            .map(|img| {
                let thumb = if img.width() > 2048 || img.height() > 2048 {
                    img.thumbnail(2048, 2048)
                } else {
                    img
                };
                let rgba = thumb.to_rgba8();
                let w = rgba.width() as usize;
                let h = rgba.height() as usize;
                let color_img = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
                let handle = ctx.load_texture(
                    format!("krono_img:{norm_rel}"),
                    color_img,
                    TextureOptions::LINEAR,
                );
                (handle, [w.max(1), h.max(1)])
            });

        let result = loaded.as_ref().map(|(handle, dims)| {
            (handle.id(), vec2(dims[0] as f32, dims[1] as f32))
        });
        let (texture, size_px) = match loaded {
            Some((handle, dims)) => (Some(handle), dims),
            None => (None, [0, 0]),
        };
        self.entries.insert(
            norm_rel,
            CachedImage {
                modified,
                size,
                demo_pinned: false,
                texture,
                size_px,
            },
        );
        result
    }
}

pub fn encode_rgba_to_png(width: usize, height: usize, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return Err("invalid clipboard image buffer".to_string());
    }
    let mut buf = Cursor::new(Vec::new());
    PngEncoder::new(&mut buf)
        .write_image(
            rgba,
            width as u32,
            height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("encode png: {e}"))?;
    Ok(buf.into_inner())
}

pub fn render_image_lightbox(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    vault_dir: &Path,
    cache: &mut ImageTextureCache,
    open_path: &mut Option<String>,
) {
    let Some(rel_path) = open_path.clone() else {
        return;
    };
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open_path = None;
        return;
    }

    let s = strings(lang);
    let screen = ctx.screen_rect();
    let tex_info = cache.get_or_load(ctx, vault_dir, &rel_path);

    egui::Area::new(egui::Id::new("krono_image_lightbox_backdrop"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_response(screen.size(), Sense::click());
            ui.painter()
                .rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(8, 8, 10, 214));
            if resp.clicked() {
                *open_path = None;
            }
        });

    egui::Area::new(egui::Id::new("krono_image_lightbox_card"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let max_w = (screen.width() * 0.88).max(240.0);
            let max_h = (screen.height() * 0.86).max(200.0);

            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(egui::Margin::same(14.0))
                .show(ui, |ui| {
                    let body_max_w = (max_w - 28.0).max(200.0);
                    let body_max_h = (max_h - 72.0).max(120.0);
                    if let Some((tex_id, img_sz)) = tex_info {
                        let scale = (body_max_w / img_sz.x.max(1.0))
                            .min(body_max_h / img_sz.y.max(1.0))
                            .min(2.0);
                        let disp = img_sz * scale;
                        ui.set_width(disp.x.max(220.0));

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&rel_path)
                                    .font(font_mono(11.5))
                                    .color(p.text_secondary),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, p, Icon::Close, s.tooltip_close, false)
                                        .clicked()
                                    {
                                        *open_path = None;
                                    }
                                },
                            );
                        });
                        ui.add_space(8.0);
                        let (img_rect, img_resp) = ui.allocate_exact_size(disp, Sense::click());
                        ui.painter().image(
                            tex_id,
                            img_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.painter().rect_stroke(
                            img_rect,
                            Rounding::same(6.0),
                            Stroke::new(1.0_f32, p.border_hairline),
                        );
                        if img_resp.on_hover_cursor(CursorIcon::ZoomOut).clicked() {
                            *open_path = None;
                        }
                    } else {
                        ui.set_width(280.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{} ({rel_path})", s.image_not_found))
                                    .font(font_medium(12.5))
                                    .color(p.text_muted),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, p, Icon::Close, s.tooltip_close, false)
                                        .clicked()
                                    {
                                        *open_path = None;
                                    }
                                },
                            );
                        });
                    }
                });
        });
}

pub fn image_dimensions_from_bytes(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}
