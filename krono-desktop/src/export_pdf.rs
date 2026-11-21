use krono_core::export::{parse_markdown_to_export_blocks, resolve_transclusions, wrap_text_chars, ExportBlock};
use printpdf::{
    Color, ColorBits, ColorSpace, Image, ImageTransform, ImageXObject, IndirectFontRef, Line,
    Mm, PdfDocument, PdfDocumentReference, PdfLayerIndex, PdfLayerReference, PdfPageIndex, Point,
    Polygon, Px, Rgb,
};
use std::io::{BufWriter, Cursor};

const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf");
const FONT_BOLD: &[u8] = include_bytes!("../assets/fonts/IBMPlexSans-Bold.ttf");

const PAGE_W_MM: f32 = 210.0;
const PAGE_H_MM: f32 = 297.0;
const MARGIN_MM: f32 = 18.0;
const USABLE_W_MM: f32 = PAGE_W_MM - 2.0 * MARGIN_MM;

struct PdfCursor<'a> {
    doc: &'a PdfDocumentReference,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    y_mm: f32,
    font_reg: &'a IndirectFontRef,
    font_bold: &'a IndirectFontRef,
}

impl<'a> PdfCursor<'a> {
    fn current_layer(&self) -> PdfLayerReference {
        self.doc.get_page(self.page).get_layer(self.layer)
    }

    fn ensure_space(&mut self, needed_mm: f32) {
        if self.y_mm - needed_mm < MARGIN_MM {
            let (p, l) = self.doc.add_page(Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
            self.page = p;
            self.layer = l;
            self.y_mm = PAGE_H_MM - MARGIN_MM;
        }
    }

    fn write_wrapped_lines(
        &mut self,
        text: &str,
        x_mm: f32,
        avail_w_mm: f32,
        font_pt: f32,
        bold: bool,
        rgb: (f32, f32, f32),
    ) {
        let max_chars = ((avail_w_mm / (font_pt * 0.3527 * 0.56)).floor() as usize).max(12);
        let line_h_mm = (font_pt * 0.3527 * 1.42).max(4.2);
        let lines = wrap_text_chars(text, max_chars);
        let font = if bold { self.font_bold } else { self.font_reg };
        for line in lines {
            self.ensure_space(line_h_mm);
            let layer = self.current_layer();
            layer.set_fill_color(Color::Rgb(Rgb::new(rgb.0, rgb.1, rgb.2, None)));
            layer.use_text(line, font_pt, Mm(x_mm), Mm(self.y_mm), font);
            self.y_mm -= line_h_mm;
        }
    }
}

pub fn render_markdown_to_pdf_bytes<F, I>(
    title: &str,
    body_md: &str,
    resolve_note: F,
    mut resolve_image: I,
) -> Result<Vec<u8>, String>
where
    F: FnMut(&str) -> Option<String>,
    I: FnMut(&str) -> Option<Vec<u8>>,
{
    let expanded = resolve_transclusions(body_md, 3, resolve_note);
    let blocks = parse_markdown_to_export_blocks(&expanded);

    let (doc, page1, layer1) = PdfDocument::new(title, Mm(PAGE_W_MM), Mm(PAGE_H_MM), "Layer 1");
    let font_reg = doc
        .add_external_font(Cursor::new(FONT_REGULAR))
        .map_err(|e| format!("PDF font error: {e}"))?;
    let font_bold = doc
        .add_external_font(Cursor::new(FONT_BOLD))
        .map_err(|e| format!("PDF bold font error: {e}"))?;

    let mut cur = PdfCursor {
        doc: &doc,
        page: page1,
        layer: layer1,
        y_mm: PAGE_H_MM - MARGIN_MM,
        font_reg: &font_reg,
        font_bold: &font_bold,
    };

    if !title.trim().is_empty() {
        cur.write_wrapped_lines(title.trim(), MARGIN_MM, USABLE_W_MM, 20.0, true, (0.08, 0.09, 0.12));
        cur.y_mm -= 3.0;
    }

    for block in blocks {
        match block {
            ExportBlock::Heading { level, text } => {
                cur.y_mm -= 2.5;
                let pt = match level {
                    1 => 16.0,
                    2 => 13.5,
                    3 => 11.5,
                    _ => 10.5,
                };
                cur.write_wrapped_lines(&text, MARGIN_MM, USABLE_W_MM, pt, true, (0.10, 0.12, 0.16));
                cur.y_mm -= 1.5;
            }
            ExportBlock::Paragraph(text) => {
                cur.write_wrapped_lines(&text, MARGIN_MM, USABLE_W_MM, 10.5, false, (0.16, 0.18, 0.22));
                cur.y_mm -= 2.2;
            }
            ExportBlock::ListItem {
                ordered,
                index,
                checked,
                text,
            } => {
                let prefix = match checked {
                    Some(true) => "[x] ".to_string(),
                    Some(false) => "[ ] ".to_string(),
                    None if ordered => format!("{index}. "),
                    None => "• ".to_string(),
                };
                let full = format!("{prefix}{text}");
                cur.write_wrapped_lines(&full, MARGIN_MM + 4.0, USABLE_W_MM - 4.0, 10.5, false, (0.16, 0.18, 0.22));
                cur.y_mm -= 1.2;
            }
            ExportBlock::Quote(text) => {
                let start_y = cur.y_mm;
                cur.write_wrapped_lines(&text, MARGIN_MM + 5.0, USABLE_W_MM - 5.0, 10.0, false, (0.35, 0.39, 0.46));
                let end_y = cur.y_mm;
                if start_y > end_y {
                    let layer = cur.current_layer();
                    layer.set_outline_color(Color::Rgb(Rgb::new(0.28, 0.50, 0.85, None)));
                    layer.set_outline_thickness(1.5);
                    let line = Line {
                        points: vec![
                            (Point::new(Mm(MARGIN_MM + 1.5), Mm(start_y + 2.5)), false),
                            (Point::new(Mm(MARGIN_MM + 1.5), Mm(end_y + 1.5)), false),
                        ],
                        is_closed: false,
                    };
                    layer.add_line(line);
                }
                cur.y_mm -= 2.0;
            }
            ExportBlock::CodeBlock { code, .. } => {
                cur.y_mm -= 1.5;
                cur.write_wrapped_lines(&code, MARGIN_MM + 3.0, USABLE_W_MM - 6.0, 9.0, false, (0.20, 0.24, 0.30));
                cur.y_mm -= 2.5;
            }
            ExportBlock::Table { headers, rows } => {
                let cols = headers.len().max(rows.iter().map(|r| r.len()).max().unwrap_or(0));
                if cols == 0 {
                    continue;
                }
                let col_w = USABLE_W_MM / (cols as f32);
                let row_h = 6.5_f32;
                let draw_row = |cur: &mut PdfCursor<'_>, cells: &[String], is_header: bool| {
                    cur.ensure_space(row_h + 1.0);
                    let top = cur.y_mm + 2.0;
                    let bot = top - row_h;
                    let layer = cur.current_layer();
                    if is_header {
                        layer.set_fill_color(Color::Rgb(Rgb::new(0.93, 0.94, 0.96, None)));
                        let rect = Polygon {
                            rings: vec![vec![
                                (Point::new(Mm(MARGIN_MM), Mm(top)), false),
                                (Point::new(Mm(MARGIN_MM + USABLE_W_MM), Mm(top)), false),
                                (Point::new(Mm(MARGIN_MM + USABLE_W_MM), Mm(bot)), false),
                                (Point::new(Mm(MARGIN_MM), Mm(bot)), false),
                            ]],
                            mode: printpdf::path::PaintMode::Fill,
                            winding_order: printpdf::path::WindingOrder::NonZero,
                        };
                        layer.add_polygon(rect);
                    }
                    layer.set_outline_color(Color::Rgb(Rgb::new(0.80, 0.82, 0.86, None)));
                    layer.set_outline_thickness(0.5);
                    let border = Line {
                        points: vec![
                            (Point::new(Mm(MARGIN_MM), Mm(bot)), false),
                            (Point::new(Mm(MARGIN_MM + USABLE_W_MM), Mm(bot)), false),
                        ],
                        is_closed: false,
                    };
                    layer.add_line(border);

                    let max_cell_chars = ((col_w / (9.0 * 0.3527 * 0.58)).floor() as usize).max(6);
                    let font = if is_header { cur.font_bold } else { cur.font_reg };
                    layer.set_fill_color(Color::Rgb(Rgb::new(0.12, 0.14, 0.18, None)));
                    for (ci, cell) in cells.iter().take(cols).enumerate() {
                        let clipped: String = cell.chars().take(max_cell_chars).collect();
                        layer.use_text(
                            clipped,
                            9.0,
                            Mm(MARGIN_MM + ci as f32 * col_w + 1.5),
                            Mm(bot + 2.0),
                            font,
                        );
                    }
                    cur.y_mm -= row_h;
                };
                if !headers.is_empty() {
                    draw_row(&mut cur, &headers, true);
                }
                for r in &rows {
                    draw_row(&mut cur, r, false);
                }
                cur.y_mm -= 3.0;
            }
            ExportBlock::Image { path, .. } => {
                if let Some(raw_bytes) = resolve_image(&path) {
                    if let Ok(dyn_img) = image::load_from_memory(&raw_bytes) {
                        let rgb = dyn_img.to_rgb8();
                        let (w, h) = rgb.dimensions();
                        if w > 0 && h > 0 {
                            let target_w_mm = (w as f32 * 0.264583).min(USABLE_W_MM);
                            let mut target_h_mm = target_w_mm * (h as f32 / w as f32);
                            let max_h = PAGE_H_MM - 2.0 * MARGIN_MM - 4.0;
                            let mut final_w_mm = target_w_mm;
                            if target_h_mm > max_h {
                                let ratio = max_h / target_h_mm;
                                target_h_mm = max_h;
                                final_w_mm *= ratio;
                            }
                            cur.ensure_space(target_h_mm + 2.0);
                            let dpi_x = (w as f32) / (final_w_mm / 25.4);
                            let xobj = ImageXObject {
                                width: Px(w as usize),
                                height: Px(h as usize),
                                color_space: ColorSpace::Rgb,
                                bits_per_component: ColorBits::Bit8,
                                interpolate: true,
                                image_data: rgb.into_raw(),
                                image_filter: None,
                                clipping_bbox: None,
                                smask: None,
                            };
                            let pdf_img = Image::from(xobj);
                            let layer = cur.current_layer();
                            pdf_img.add_to_layer(
                                layer,
                                ImageTransform {
                                    translate_x: Some(Mm(MARGIN_MM)),
                                    translate_y: Some(Mm(cur.y_mm - target_h_mm)),
                                    dpi: Some(dpi_x),
                                    ..Default::default()
                                },
                            );
                            cur.y_mm -= target_h_mm + 4.0;
                        }
                    }
                }
            }
            ExportBlock::Rule => {
                cur.ensure_space(4.0);
                let layer = cur.current_layer();
                layer.set_outline_color(Color::Rgb(Rgb::new(0.82, 0.84, 0.88, None)));
                layer.set_outline_thickness(0.6);
                let line = Line {
                    points: vec![
                        (Point::new(Mm(MARGIN_MM), Mm(cur.y_mm)), false),
                        (Point::new(Mm(MARGIN_MM + USABLE_W_MM), Mm(cur.y_mm)), false),
                    ],
                    is_closed: false,
                };
                layer.add_line(line);
                cur.y_mm -= 4.0;
            }
        }
    }

    let mut out = Vec::new();
    {
        let mut writer = BufWriter::new(&mut out);
        doc.save(&mut writer)
            .map_err(|e| format!("PDF save error: {e}"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_valid_cyrillic_pdf_bytes() {
        let md = "## План проекта\n\nОчень длинный абзац на русском языке, который должен аккуратно переноситься по ширине страницы формата A4 без обрезания символов и без паник.\n\n- [x] Первый пункт\n- [ ] Второй пункт\n\n| Поле | Значение |\n|---|---|\n| Статус | Готово |\n";
        let bytes = render_markdown_to_pdf_bytes("Тестовый документ", md, |_| None, |_| None)
            .expect("pdf render");
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.len() > 1000);
    }
}
