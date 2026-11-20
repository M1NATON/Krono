use crate::i18n::{strings, Lang};
use crate::ui::components::hairline_h;
use crate::ui::embedded_board::{render_embedded_board_widget, EmbeddedBoardAction};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::image_cache::ImageTextureCache;
use crate::ui::motion::{anim_bool_eased, ease_out_back, lerp_color, with_alpha};
use crate::ui::theme::{
    font_body, font_bold, font_medium, font_mono, font_semibold, font_serif_bold,
    font_serif_semibold, ThemePalette,
};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Margin, PopupCloseBehavior, Rect, Rounding, Sense,
    Stroke, TextFormat, Ui,
};
use egui::text::LayoutJob;
use krono_core::{
    collection::CollectionDoc,
    daily::SimpleDate,
    markdown::{parse_inline_task_meta, TaskPriority},
    path::{vault_file_stem, VaultFileKind},
};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Clone, Debug)]
pub enum PreviewAction {
    NavigateWikilink(String),
    ToggleTask(usize),
    CopyCode(String),
    OpenImage(String),
    EmbeddedBoard(EmbeddedBoardAction),
    SendTaskToBoard {
        task_idx: usize,
        board_path: String,
        move_task: bool,
    },
}

#[derive(Clone)]
enum Span {
    Text {
        text: String,
        bold: bool,
        italic: bool,
        strike: bool,
        code: bool,
        url: Option<String>,
    },
    WikiLink {
        target: String,
        display: String,
    },
    LineBreak,
}

#[derive(Debug)]
enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    Quote(Vec<Span>),
    ListItem {
        ordered_num: Option<u64>,
        task: Option<(usize, bool)>,
        depth: usize,
        spans: Vec<Span>,
    },
    CodeBlock {
        lang: String,
        code: String,
    },
    Table {
        headers: Vec<Vec<Span>>,
        rows: Vec<Vec<Vec<Span>>>,
    },
    Image {
        path: String,
        alt: String,
    },
    EmbeddedBoard {
        target: String,
    },
    Rule,
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { text, .. } => f.debug_tuple("Text").field(text).finish(),
            Self::WikiLink { target, display } => {
                f.debug_tuple("WikiLink").field(target).field(display).finish()
            }
            Self::LineBreak => write!(f, "LineBreak"),
        }
    }
}

pub fn resolve_embedded_board<'a>(
    target: &str,
    boards: &'a HashMap<String, CollectionDoc>,
) -> Option<(&'a String, &'a CollectionDoc)> {
    let t = target.trim().replace('\\', "/").to_lowercase();
    if t.is_empty() {
        return None;
    }
    if let Some(hit) = boards.iter().find(|(k, _)| k.to_lowercase() == t) {
        return Some(hit);
    }
    let stem_t = vault_file_stem(&t).to_lowercase();
    boards.iter().find(|(k, doc)| {
        k.to_lowercase().ends_with(&format!("/{t}"))
            || vault_file_stem(k).to_lowercase() == stem_t
            || doc.title.trim().to_lowercase() == stem_t
    })
}

#[allow(clippy::too_many_arguments)]
pub fn render_markdown_preview(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    today: SimpleDate,
    body: &str,
    font_size: f32,
    heading_serif: bool,
    resolved_titles: &HashSet<String>,
    copied_code_hash: Option<u64>,
    vault_dir: &Path,
    image_cache: &mut ImageTextureCache,
    embedded_boards: &HashMap<String, CollectionDoc>,
    available_boards: &[(String, String)],
) -> Option<PreviewAction> {
    let s = strings(lang);
    let blocks = parse_blocks(body);
    let mut action: Option<PreviewAction> = None;
    let body_sz = font_size.clamp(14.0, 18.0);
    let sz_delta = body_sz - 15.0;

    for (b_idx, block) in blocks.iter().enumerate() {
        match block {
            Block::Heading { level, spans } => {
                if b_idx > 0 {
                    ui.add_space(match level {
                        1 => 14.0,
                        2 => 10.0,
                        _ => 6.0,
                    });
                }
                let size = match level {
                    1 => 25.0 + sz_delta,
                    2 => 20.0 + sz_delta,
                    3 => 17.0 + sz_delta,
                    _ => 15.5 + sz_delta,
                };
                render_spans(
                    ui,
                    p,
                    spans,
                    size,
                    *level,
                    heading_serif,
                    false,
                    p.text_primary,
                    resolved_titles,
                    &mut action,
                );
                ui.add_space(4.0);
            }
            Block::Paragraph(spans) => {
                render_spans(
                    ui,
                    p,
                    spans,
                    body_sz,
                    0,
                    heading_serif,
                    false,
                    p.text_primary,
                    resolved_titles,
                    &mut action,
                );
                ui.add_space(8.0);
            }
            Block::Quote(spans) => {
                let quote_max_w = (ui.available_width() - 30.0).max(80.0);
                let mut frame = egui::Frame::none()
                    .fill(p.bg_elevated)
                    .rounding(Rounding::same(6.0))
                    .inner_margin(Margin {
                        left: 16.0,
                        right: 14.0,
                        top: 9.0,
                        bottom: 9.0,
                    })
                    .begin(ui);
                frame.content_ui.set_width(quote_max_w);
                frame.content_ui.set_max_width(quote_max_w);
                render_spans(
                    &mut frame.content_ui,
                    p,
                    spans,
                    (body_sz - 0.5).max(13.0),
                    0,
                    heading_serif,
                    true,
                    p.text_secondary,
                    resolved_titles,
                    &mut action,
                );
                let resp = frame.end(ui);
                let bar = Rect::from_min_max(
                    pos2(resp.rect.left(), resp.rect.top() + 3.0),
                    pos2(resp.rect.left() + 3.0, resp.rect.bottom() - 3.0),
                );
                ui.painter().rect_filled(bar, Rounding::same(2.0), p.accent);
                ui.add_space(8.0);
            }
            Block::ListItem {
                ordered_num,
                task,
                depth,
                spans,
            } => {
                let indent = (*depth as f32) * 18.0;
                ui.horizontal_top(|ui| {
                    ui.add_space(indent);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some((task_idx, checked)) = *task {
                        let (chk_rect, chk_resp) =
                            ui.allocate_exact_size(vec2(17.0, 21.0), Sense::click());
                        let box_rect =
                            Rect::from_center_size(chk_rect.center(), vec2(15.0, 15.0));
                        let chk_t = anim_bool_eased(
                            ui.ctx(),
                            ui.make_persistent_id(("md_task_chk", task_idx)),
                            checked,
                            0.15,
                        );
                        let hov_t = anim_bool_eased(
                            ui.ctx(),
                            ui.make_persistent_id(("md_task_hov", task_idx)),
                            chk_resp.hovered(),
                            0.10,
                        );
                        let unchecked_bg = lerp_color(p.bg_input, p.bg_hover, hov_t);
                        let fill_col = lerp_color(unchecked_bg, p.accent, chk_t);
                        ui.painter()
                            .rect_filled(box_rect, Rounding::same(4.0), fill_col);
                        if chk_t < 0.99 {
                            let border_base =
                                lerp_color(p.border_strong, p.accent, hov_t * 0.85);
                            ui.painter().rect_stroke(
                                box_rect,
                                Rounding::same(4.0),
                                Stroke::new(1.2_f32, with_alpha(border_base, 1.0 - chk_t)),
                            );
                        }
                        if chk_t > 0.05 {
                            let pop = ease_out_back(chk_t).max(0.1);
                            draw_icon(
                                ui.painter(),
                                Rect::from_center_size(
                                    box_rect.center(),
                                    vec2(11.0 * pop, 11.0 * pop),
                                ),
                                Icon::Check,
                                with_alpha(p.accent_on_fill, chk_t),
                            );
                        }
                        if chk_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            action = Some(PreviewAction::ToggleTask(task_idx));
                        }
                        let raw_task = spans_plain_text(spans);
                        let parsed_meta = parse_inline_task_meta(&raw_task, today);
                        let text_col = if checked { p.text_muted } else { p.text_primary };
                        let mut adjusted_spans = strip_task_meta_tokens_from_spans(spans, today);
                        if checked {
                            for sp in &mut adjusted_spans {
                                if let Span::Text { strike, .. } = sp {
                                    *strike = true;
                                }
                            }
                        }
                        let send_btn_w = 24.0;
                        let text_w = (ui.available_width() - send_btn_w - 6.0).max(80.0);
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.set_max_width(text_w);
                            render_spans(
                                ui,
                                p,
                                &adjusted_spans,
                                body_sz,
                                0,
                                heading_serif,
                                false,
                                text_col,
                                resolved_titles,
                                &mut action,
                            );
                            if parsed_meta.due_date.is_some() || parsed_meta.priority.is_some() {
                                ui.add_space(2.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(6.0, 3.0);
                                    if let Some(ref due) = parsed_meta.due_date {
                                        let due_d = SimpleDate::parse_iso(due);
                                        let overdue = !checked && due_d.is_some_and(|d| d < today);
                                        let is_today = due_d == Some(today);
                                        let (bg, fg) = if overdue {
                                            (p.danger_soft, p.danger)
                                        } else if is_today {
                                            (p.warning_soft, p.warning)
                                        } else {
                                            (p.accent_soft, p.accent_text)
                                        };
                                        render_task_badge(ui, Some(Icon::CalendarDays), due, bg, fg);
                                    }
                                    if let Some(prio) = parsed_meta.priority {
                                        let (lbl, bg, fg) = match prio {
                                            TaskPriority::High => {
                                                ("!High", p.danger_soft, p.danger)
                                            }
                                            TaskPriority::Medium => {
                                                ("!Med", p.warning_soft, p.warning)
                                            }
                                            TaskPriority::Low => {
                                                ("!Low", p.bg_elevated, p.text_muted)
                                            }
                                        };
                                        render_task_badge(ui, None, lbl, bg, fg);
                                    }
                                });
                            }
                        });

                        let (b_rect, b_resp) =
                            ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                        let b_hov = b_resp.hovered();
                        if b_hov {
                            ui.painter()
                                .rect_filled(b_rect, Rounding::same(5.0), p.bg_hover);
                        }
                        draw_icon(
                            ui.painter(),
                            Rect::from_center_size(b_rect.center(), vec2(12.5, 12.5)),
                            Icon::Board,
                            if b_hov { p.accent_text } else { p.text_muted },
                        );
                        let pop_id = ui.make_persistent_id(("task_to_board_pop", task_idx));
                        if b_resp
                            .clone()
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(s.task_send_board_tip)
                            .clicked()
                        {
                            ui.memory_mut(|m| m.toggle_popup(pop_id));
                        }
                        egui::popup_below_widget(
                            ui,
                            pop_id,
                            &b_resp,
                            PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_min_width(230.0);
                                if available_boards.is_empty() {
                                    ui.label(
                                        egui::RichText::new(s.task_no_boards)
                                            .font(font_body(12.0))
                                            .color(p.text_muted),
                                    );
                                } else {
                                    for (bpath, btitle) in available_boards {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 5.0;
                                            let title_show = if btitle.trim().is_empty() {
                                                vault_file_stem(bpath)
                                            } else {
                                                btitle.trim()
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(title_show)
                                                        .font(font_medium(12.0))
                                                        .color(p.text_primary),
                                                )
                                                .truncate(),
                                            );
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if task_pop_btn(
                                                        ui,
                                                        p,
                                                        s.task_move_to_board,
                                                        true,
                                                    )
                                                    .clicked()
                                                    {
                                                        action = Some(
                                                            PreviewAction::SendTaskToBoard {
                                                                task_idx,
                                                                board_path: bpath.clone(),
                                                                move_task: true,
                                                            },
                                                        );
                                                        ui.memory_mut(|m| m.close_popup());
                                                    }
                                                    if task_pop_btn(
                                                        ui,
                                                        p,
                                                        s.task_copy_to_board,
                                                        false,
                                                    )
                                                    .clicked()
                                                    {
                                                        action = Some(
                                                            PreviewAction::SendTaskToBoard {
                                                                task_idx,
                                                                board_path: bpath.clone(),
                                                                move_task: false,
                                                            },
                                                        );
                                                        ui.memory_mut(|m| m.close_popup());
                                                    }
                                                },
                                            );
                                        });
                                        ui.add_space(2.0);
                                    }
                                }
                            },
                        );
                    } else {
                        let marker = match ordered_num {
                            Some(n) => format!("{n}."),
                            None => "•".to_string(),
                        };
                        let (m_rect, _) =
                            ui.allocate_exact_size(vec2(16.0, 21.0), Sense::hover());
                        let g = ui.painter().layout_no_wrap(
                            marker,
                            font_semibold((body_sz - 0.5).max(13.0)),
                            p.accent_text,
                        );
                        ui.painter().galley(
                            pos2(m_rect.left() + 2.0, m_rect.top() + 2.0),
                            g,
                            p.accent_text,
                        );
                        let text_w = ui.available_width().max(80.0);
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.set_max_width(text_w);
                            render_spans(
                                ui,
                                p,
                                spans,
                                body_sz,
                                0,
                                heading_serif,
                                false,
                                p.text_primary,
                                resolved_titles,
                                &mut action,
                            );
                        });
                    }
                });
                ui.add_space(4.0);
            }
            Block::CodeBlock { lang: code_lang, code } => {
                let s = strings(lang);
                let code_hash = hash_str(code);
                let just_copied = copied_code_hash == Some(code_hash);
                egui::Frame::none()
                    .fill(p.bg_code)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(14.0, 10.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let header_lbl = if code_lang.is_empty() {
                                "code"
                            } else {
                                code_lang.as_str()
                            };
                            ui.label(
                                egui::RichText::new(header_lbl)
                                    .font(font_mono(11.5))
                                    .color(p.text_muted),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let btn_lbl = if just_copied {
                                        s.copied_btn
                                    } else {
                                        s.copy_btn
                                    };
                                    let fg = if just_copied {
                                        p.success
                                    } else {
                                        p.text_secondary
                                    };
                                    let g = ui.painter().layout_no_wrap(
                                        btn_lbl.to_string(),
                                        font_medium(11.5),
                                        fg,
                                    );
                                    let (brect, bresp) = ui.allocate_exact_size(
                                        vec2(g.size().x + 24.0, 22.0),
                                        Sense::click(),
                                    );
                                    if bresp.hovered() {
                                        ui.painter().rect_filled(
                                            brect,
                                            Rounding::same(5.0),
                                            p.bg_hover,
                                        );
                                    }
                                    let ir = Rect::from_center_size(
                                        pos2(brect.left() + 10.0, brect.center().y),
                                        vec2(12.0, 12.0),
                                    );
                                    draw_icon(
                                        ui.painter(),
                                        ir,
                                        if just_copied { Icon::Check } else { Icon::Copy },
                                        fg,
                                    );
                                    ui.painter().galley(
                                        pos2(
                                            brect.left() + 19.0,
                                            brect.center().y - g.size().y * 0.5,
                                        ),
                                        g,
                                        fg,
                                    );
                                    if bresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        action = Some(PreviewAction::CopyCode(code.clone()));
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        egui::ScrollArea::horizontal()
                            .id_salt(("code_block", b_idx))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(code.trim_end_matches('\n'))
                                        .font(font_mono(13.5))
                                        .color(p.text_primary),
                                );
                            });
                    });
                ui.add_space(10.0);
            }
            Block::Table { headers, rows } => {
                egui::Frame::none()
                    .fill(p.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        egui::Grid::new(("md_table", b_idx))
                            .striped(true)
                            .spacing([20.0, 8.0])
                            .show(ui, |ui| {
                                for cell in headers {
                                    render_spans(
                                        ui,
                                        p,
                                        cell,
                                        13.5,
                                        3,
                                        heading_serif,
                                        false,
                                        p.text_primary,
                                        resolved_titles,
                                        &mut action,
                                    );
                                }
                                ui.end_row();
                                for row in rows {
                                    for cell in row {
                                        render_spans(
                                            ui,
                                            p,
                                            cell,
                                            13.5,
                                            0,
                                            heading_serif,
                                            false,
                                            p.text_secondary,
                                            resolved_titles,
                                            &mut action,
                                        );
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                ui.add_space(10.0);
            }
            Block::Rule => {
                ui.add_space(6.0);
                hairline_h(ui, p);
                ui.add_space(10.0);
            }
            Block::Image { path, alt } => {
                let s = strings(lang);
                let avail_w = ui.available_width().min(680.0).max(160.0);
                if let Some((tex_id, img_sz)) = image_cache.get_or_load(ui.ctx(), vault_dir, path) {
                    let max_h = 420.0_f32;
                    let scale = (avail_w / img_sz.x.max(1.0))
                        .min(max_h / img_sz.y.max(1.0))
                        .min(1.0);
                    let disp = img_sz * scale;
                    let (rect, resp) = ui.allocate_exact_size(disp, Sense::click());
                    ui.painter().image(
                        tex_id,
                        rect,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.painter().rect_stroke(
                        rect,
                        Rounding::same(8.0),
                        Stroke::new(1.0_f32, p.border_hairline),
                    );
                    if resp.on_hover_cursor(CursorIcon::ZoomIn).clicked() {
                        action = Some(PreviewAction::OpenImage(path.clone()));
                    }
                    let caption = alt.trim();
                    if !caption.is_empty() && caption != path {
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new(caption)
                                .font(font_body(11.5))
                                .color(p.text_muted),
                        );
                    }
                } else {
                    egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_hairline))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                draw_icon(ui.painter(), ir, Icon::Image, p.text_muted);
                                ui.label(
                                    egui::RichText::new(format!("{}: {path}", s.image_not_found))
                                        .font(font_mono(11.5))
                                        .color(p.text_muted),
                                );
                            });
                        });
                }
                ui.add_space(10.0);
            }
            Block::EmbeddedBoard { target } => {
                let s = strings(lang);
                if let Some((board_path, doc)) = resolve_embedded_board(target, embedded_boards) {
                    let mut board_acts = Vec::new();
                    let salt = format!("pv_{b_idx}");
                    render_embedded_board_widget(
                        ui,
                        p,
                        lang,
                        board_path,
                        doc,
                        &salt,
                        &mut board_acts,
                    );
                    if let Some(first) = board_acts.into_iter().next() {
                        action = Some(PreviewAction::EmbeddedBoard(first));
                    }
                } else {
                    egui::Frame::none()
                        .fill(p.bg_elevated)
                        .stroke(Stroke::new(1.0_f32, p.border_hairline))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::symmetric(10.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                draw_icon(ui.painter(), ir, Icon::Board, p.text_muted);
                                ui.label(
                                    egui::RichText::new(format!("{}: {target}", s.embedded_board_missing))
                                        .font(font_mono(11.5))
                                        .color(p.text_muted),
                                );
                            });
                        });
                }
                ui.add_space(10.0);
            }
        }
    }

    action
}

#[allow(clippy::too_many_arguments)]
fn render_spans(
    ui: &mut Ui,
    p: &ThemePalette,
    spans: &[Span],
    font_size: f32,
    heading_level: u8,
    heading_serif: bool,
    force_italic: bool,
    default_color: Color32,
    resolved_titles: &HashSet<String>,
    action: &mut Option<PreviewAction>,
) {
    let has_wikilinks = spans.iter().any(|s| matches!(s, Span::WikiLink { .. }));
    let wrap_w = ui.available_width().max(60.0);
    if !has_wikilinks {
        let job = build_spans_job(
            spans,
            font_size,
            heading_level,
            heading_serif,
            force_italic,
            default_color,
            wrap_w,
            p,
        );
        let resp = ui.add(egui::Label::new(job).wrap().sense(Sense::click()));
        if resp.clicked() {
            if let Some(url) = spans.iter().find_map(|s| match s {
                Span::Text { url: Some(u), .. } => Some(u.clone()),
                _ => None,
            }) {
                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
            }
        }
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(wrap_w);
        ui.spacing_mut().item_spacing = vec2(0.0, 3.0);
        for sp in spans {
            match sp {
                Span::WikiLink { target, display } => {
                    let exists = resolved_titles.contains(&target.to_lowercase());
                    let fg = if exists { p.accent_text } else { p.text_secondary };
                    let bg = if exists { p.accent_soft } else { p.bg_hover };
                    let mut job = LayoutJob::single_section(
                        display.clone(),
                        TextFormat {
                            font_id: font_medium(font_size),
                            color: fg,
                            ..Default::default()
                        },
                    );
                    job.wrap = egui::text::TextWrapping {
                        max_width: (wrap_w - 12.0).max(40.0),
                        max_rows: 1,
                        break_anywhere: false,
                        overflow_character: Some('…'),
                    };
                    let g = ui.fonts(|f| f.layout_job(job));
                    let (rect, resp) =
                        ui.allocate_exact_size(vec2(g.size().x + 8.0, font_size * 1.38), Sense::click());
                    ui.painter().rect_filled(rect, Rounding::same(4.0), bg);
                    if !exists {
                        ui.painter().rect_stroke(
                            rect,
                            Rounding::same(4.0),
                            Stroke::new(1.0_f32, p.border_hairline),
                        );
                    }
                    ui.painter().galley(
                        pos2(rect.left() + 4.0, rect.center().y - g.size().y * 0.5),
                        g,
                        fg,
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        *action = Some(PreviewAction::NavigateWikilink(target.clone()));
                    }
                }
                Span::LineBreak => {
                    ui.end_row();
                }
                Span::Text { text, bold, italic, strike, code, url } => {
                    for chunk in split_preserving_spaces(text) {
                        let piece = Span::Text {
                            text: chunk,
                            bold: *bold,
                            italic: *italic,
                            strike: *strike,
                            code: *code,
                            url: url.clone(),
                        };
                        let job = build_spans_job(
                            &[piece],
                            font_size,
                            heading_level,
                            heading_serif,
                            force_italic,
                            default_color,
                            wrap_w,
                            p,
                        );
                        let resp = ui.add(egui::Label::new(job).wrap().sense(Sense::click()));
                        if resp.clicked() {
                            if let Some(u) = url {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(u.clone()));
                            }
                        }
                    }
                }
            }
        }
    });
}

fn split_preserving_spaces(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in input.chars() {
        cur.push(ch);
        if ch.is_whitespace() && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn build_spans_job(
    spans: &[Span],
    font_size: f32,
    heading_level: u8,
    heading_serif: bool,
    force_italic: bool,
    default_color: Color32,
    wrap_width: f32,
    p: &ThemePalette,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let lh = Some((font_size * 1.56).round());

    for sp in spans {
        match sp {
            Span::Text {
                text,
                bold,
                italic,
                strike,
                code,
                url,
            } => {
                let font_id = if *code {
                    font_mono((font_size - 1.5).max(12.0))
                } else if heading_level == 1 {
                    if heading_serif {
                        font_serif_bold(font_size)
                    } else {
                        font_bold(font_size)
                    }
                } else if heading_level == 2 {
                    if heading_serif {
                        font_serif_semibold(font_size)
                    } else {
                        font_semibold(font_size)
                    }
                } else if heading_level >= 3 {
                    font_semibold(font_size)
                } else if *bold {
                    font_bold(font_size)
                } else {
                    font_body(font_size)
                };
                let color = if *code || url.is_some() {
                    p.accent_text
                } else {
                    default_color
                };
                let background = if *code {
                    p.bg_code
                } else {
                    Color32::TRANSPARENT
                };
                let underline = if url.is_some() {
                    Stroke::new(1.0_f32, p.accent_text)
                } else {
                    Stroke::NONE
                };
                let strikethrough = if *strike {
                    Stroke::new(1.0_f32, color)
                } else {
                    Stroke::NONE
                };
                job.append(
                    text,
                    0.0,
                    TextFormat {
                        font_id,
                        color,
                        background,
                        italics: *italic || force_italic,
                        underline,
                        strikethrough,
                        line_height: lh,
                        ..Default::default()
                    },
                );
            }
            Span::LineBreak => {
                job.append(
                    "\n",
                    0.0,
                    TextFormat {
                        font_id: font_body(font_size),
                        color: default_color,
                        line_height: lh,
                        ..Default::default()
                    },
                );
            }
            Span::WikiLink { .. } => {}
        }
    }
    job
}

fn split_wikilink_spans(raw_spans: Vec<Span>) -> Vec<Span> {
    // Coalesce adjacent text spans emitted by pulldown-cmark (which splits `[` / `]` into
    // separate `Event::Text` items) before resolving `[[Target]]` / `[[Target|Alias]]`.
    let mut coalesced: Vec<Span> = Vec::with_capacity(raw_spans.len());
    for sp in raw_spans {
        match (coalesced.last_mut(), sp) {
            (
                Some(Span::Text {
                    text: prev,
                    bold: pb,
                    italic: pi,
                    strike: ps,
                    code: pc,
                    url: pu,
                }),
                Span::Text {
                    text: cur,
                    bold: cb,
                    italic: ci,
                    strike: cs,
                    code: cc,
                    url: cu,
                },
            ) if *pb == cb && *pi == ci && *ps == cs && *pc == cc && *pu == cu && !*pc => {
                prev.push_str(&cur);
            }
            (_, other) => coalesced.push(other),
        }
    }

    let mut out = Vec::with_capacity(coalesced.len());
    for sp in coalesced {
        match sp {
            Span::Text {
                text,
                bold,
                italic,
                strike,
                code,
                url,
            } if !code => {
                push_text_with_wikilinks(&mut out, &text, bold, italic, strike, &url);
            }
            other => out.push(other),
        }
    }
    out
}

/// Splits raw inline text on `[[Target]]` / `[[Target|Alias]]` tokens.
fn push_text_with_wikilinks(
    out: &mut Vec<Span>,
    raw: &str,
    bold: bool,
    italic: bool,
    strike: bool,
    url: &Option<String>,
) {
    let mut rest = raw;
    while let Some(start) = rest.find("[[") {
        let before = &rest[..start];
        if !before.is_empty() {
            out.push(Span::Text {
                text: before.to_string(),
                bold,
                italic,
                strike,
                code: false,
                url: url.clone(),
            });
        }
        let after = &rest[start + 2..];
        if let Some(end) = after.find("]]") {
            let inner = after[..end].trim();
            if !inner.is_empty() {
                let (target, display) = match inner.split_once('|') {
                    Some((t, a)) => (t.trim(), a.trim()),
                    None => (inner, inner),
                };
                if !target.is_empty() {
                    out.push(Span::WikiLink {
                        target: target.to_string(),
                        display: display.to_string(),
                    });
                }
            }
            rest = &after[end + 2..];
        } else {
            break;
        }
    }
    if !rest.is_empty() {
        out.push(Span::Text {
            text: rest.to_string(),
            bold,
            italic,
            strike,
            code: false,
            url: url.clone(),
        });
    }
}

fn parse_blocks(body: &str) -> Vec<Block> {
    let opts =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(body, opts);

    let mut blocks = Vec::new();
    let mut current_spans: Vec<Span> = Vec::new();
    let mut heading_level: Option<u8> = None;
    let mut quote_depth = 0usize;
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_item = false;
    let mut item_ordered_num: Option<u64> = None;
    let mut item_task: Option<(usize, bool)> = None;
    let mut task_counter = 0usize;

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();

    let mut in_table = false;
    let mut in_table_head = false;
    let mut table_headers: Vec<Vec<Span>> = Vec::new();
    let mut table_rows: Vec<Vec<Vec<Span>>> = Vec::new();
    let mut current_row: Vec<Vec<Span>> = Vec::new();
    let mut in_cell = false;

    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let mut current_url: Option<String> = None;
    let mut in_image: Option<String> = None;
    let mut image_alt = String::new();

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    current_spans.clear();
                    heading_level = Some(match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        _ => 4,
                    });
                }
                Tag::Paragraph => {
                    if !in_item {
                        current_spans.clear();
                    }
                }
                Tag::Image { dest_url, .. } => {
                    in_image = Some(dest_url.to_string());
                    image_alt.clear();
                }
                Tag::BlockQuote(_) => {
                    quote_depth += 1;
                    current_spans.clear();
                }
                Tag::List(first) => {
                    list_stack.push(first);
                }
                Tag::Item => {
                    in_item = true;
                    item_task = None;
                    current_spans.clear();
                    item_ordered_num = match list_stack.last_mut() {
                        Some(Some(num)) => {
                            let val = *num;
                            *num += 1;
                            Some(val)
                        }
                        _ => None,
                    };
                }
                Tag::CodeBlock(kind) => {
                    in_code_block = true;
                    code_buf.clear();
                    code_lang = match kind {
                        CodeBlockKind::Fenced(l) => l.to_string(),
                        CodeBlockKind::Indented => String::new(),
                    };
                }
                Tag::Table(_) => {
                    in_table = true;
                    table_headers.clear();
                    table_rows.clear();
                }
                Tag::TableHead => {
                    in_table_head = true;
                    current_row.clear();
                }
                Tag::TableRow => {
                    current_row.clear();
                }
                Tag::TableCell => {
                    in_cell = true;
                    current_spans.clear();
                }
                Tag::Strong => bold = true,
                Tag::Emphasis => italic = true,
                Tag::Strikethrough => strike = true,
                Tag::Link { dest_url, .. } => {
                    current_url = Some(dest_url.to_string());
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    let lvl = heading_level.take().unwrap_or(2);
                    blocks.push(Block::Heading {
                        level: lvl,
                        spans: split_wikilink_spans(std::mem::take(&mut current_spans)),
                    });
                }
                TagEnd::Paragraph => {
                    if in_item {
                        if !current_spans.is_empty() {
                            current_spans.push(Span::LineBreak);
                        }
                    } else if quote_depth > 0 {
                        if !current_spans.is_empty() {
                            blocks.push(Block::Quote(split_wikilink_spans(std::mem::take(
                                &mut current_spans,
                            ))));
                        }
                    } else if !current_spans.is_empty() {
                        let raw_joined = extract_plain_paragraph_if_embed(&current_spans);
                        if let Some(block) = raw_joined.as_deref().and_then(try_parse_embed_line) {
                            current_spans.clear();
                            blocks.push(block);
                        } else {
                            blocks.push(Block::Paragraph(split_wikilink_spans(std::mem::take(
                                &mut current_spans,
                            ))));
                        }
                    }
                }
                TagEnd::Image => {
                    if let Some(path) = in_image.take() {
                        if !current_spans.is_empty() && quote_depth == 0 && !in_item {
                            blocks.push(Block::Paragraph(split_wikilink_spans(std::mem::take(
                                &mut current_spans,
                            ))));
                        }
                        blocks.push(Block::Image {
                            path: path.trim().replace('\\', "/"),
                            alt: std::mem::take(&mut image_alt),
                        });
                    }
                }
                TagEnd::BlockQuote(_) => {
                    quote_depth = quote_depth.saturating_sub(1);
                    if !current_spans.is_empty() {
                        blocks.push(Block::Quote(split_wikilink_spans(std::mem::take(
                            &mut current_spans,
                        ))));
                    }
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                }
                TagEnd::Item => {
                    while matches!(current_spans.last(), Some(Span::LineBreak)) {
                        current_spans.pop();
                    }
                    blocks.push(Block::ListItem {
                        ordered_num: item_ordered_num.take(),
                        task: item_task.take(),
                        depth: list_stack.len().saturating_sub(1),
                        spans: split_wikilink_spans(std::mem::take(&mut current_spans)),
                    });
                    in_item = false;
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    blocks.push(Block::CodeBlock {
                        lang: std::mem::take(&mut code_lang),
                        code: std::mem::take(&mut code_buf),
                    });
                }
                TagEnd::TableCell => {
                    in_cell = false;
                    current_row.push(split_wikilink_spans(std::mem::take(&mut current_spans)));
                }
                TagEnd::TableHead => {
                    in_table_head = false;
                    table_headers = std::mem::take(&mut current_row);
                }
                TagEnd::TableRow => {
                    if !in_table_head {
                        table_rows.push(std::mem::take(&mut current_row));
                    }
                }
                TagEnd::Table => {
                    in_table = false;
                    blocks.push(Block::Table {
                        headers: std::mem::take(&mut table_headers),
                        rows: std::mem::take(&mut table_rows),
                    });
                }
                TagEnd::Strong => bold = false,
                TagEnd::Emphasis => italic = false,
                TagEnd::Strikethrough => strike = false,
                TagEnd::Link => current_url = None,
                _ => {}
            },
            Event::Text(t) => {
                if in_image.is_some() {
                    image_alt.push_str(&t);
                } else if in_code_block {
                    code_buf.push_str(&t);
                } else {
                    current_spans.push(Span::Text {
                        text: t.to_string(),
                        bold,
                        italic,
                        strike,
                        code: false,
                        url: current_url.clone(),
                    });
                }
            }
            Event::Code(t) => {
                current_spans.push(Span::Text {
                    text: t.to_string(),
                    bold: false,
                    italic: false,
                    strike,
                    code: true,
                    url: None,
                });
            }
            Event::SoftBreak => {
                if in_code_block {
                    code_buf.push('\n');
                } else if !in_cell {
                    current_spans.push(Span::Text {
                        text: " ".to_string(),
                        bold,
                        italic,
                        strike,
                        code: false,
                        url: current_url.clone(),
                    });
                }
            }
            Event::HardBreak => {
                if in_code_block {
                    code_buf.push('\n');
                } else {
                    current_spans.push(Span::LineBreak);
                }
            }
            Event::TaskListMarker(checked) => {
                item_task = Some((task_counter, checked));
                task_counter += 1;
            }
            Event::Rule => {
                if !in_table {
                    blocks.push(Block::Rule);
                }
            }
            _ => {}
        }
    }

    blocks
}

fn extract_plain_paragraph_if_embed(spans: &[Span]) -> Option<String> {
    let mut joined = String::new();
    for sp in spans {
        match sp {
            Span::Text {
                text,
                bold: false,
                italic: false,
                strike: false,
                code: false,
                url: None,
            } => joined.push_str(text),
            _ => return None,
        }
    }
    Some(joined)
}

fn try_parse_embed_line(raw: &str) -> Option<Block> {
    let trimmed = raw.trim();
    let inner = trimmed.strip_prefix("![[")?.strip_suffix("]]")?;
    if inner.contains("![[") || inner.contains("]]") || inner.contains('\n') {
        return None;
    }
    let (target_raw, alias_raw) = match inner.split_once('|') {
        Some((t, a)) => (t.trim(), a.trim()),
        None => (inner.trim(), ""),
    };
    if target_raw.is_empty() {
        return None;
    }
    let clean_target = target_raw.replace('\\', "/");
    if clean_target.to_lowercase().ends_with(".board.json") {
        Some(Block::EmbeddedBoard {
            target: clean_target,
        })
    } else if VaultFileKind::from_path(&clean_target) == Some(VaultFileKind::Asset) {
        Some(Block::Image {
            path: clean_target,
            alt: alias_raw.to_string(),
        })
    } else {
        None
    }
}

/// Toggles the `target_idx`-th Markdown task checkbox outside fenced code blocks.
pub fn toggle_markdown_task(body: &str, target_idx: usize) -> Option<String> {
    let mut out = String::with_capacity(body.len());
    let mut in_fence: Option<char> = None;
    let mut current_idx = 0usize;
    let mut toggled = false;

    for segment in body.split_inclusive('\n') {
        let trimmed_start = segment.trim_start();
        if trimmed_start.starts_with("```") {
            if in_fence == Some('`') {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some('`');
            }
            out.push_str(segment);
            continue;
        } else if trimmed_start.starts_with("~~~") {
            if in_fence == Some('~') {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some('~');
            }
            out.push_str(segment);
            continue;
        }

        if in_fence.is_none() && !toggled {
            let indent = segment.len() - trimmed_start.len();
            let prefix = &segment[..indent];
            let replacements = [
                ("- [ ] ", "- [x] "),
                ("- [x] ", "- [ ] "),
                ("- [X] ", "- [ ] "),
                ("* [ ] ", "* [x] "),
                ("* [x] ", "* [ ] "),
                ("* [X] ", "* [ ] "),
            ];
            let mut matched = None;
            for (from, to) in replacements {
                if let Some(rest) = trimmed_start.strip_prefix(from) {
                    matched = Some((to, rest));
                    break;
                }
            }
            if let Some((to, rest)) = matched {
                if current_idx == target_idx {
                    out.push_str(prefix);
                    out.push_str(to);
                    out.push_str(rest);
                    toggled = true;
                    continue;
                }
                current_idx += 1;
            }
        }
        out.push_str(segment);
    }

    if toggled {
        Some(out)
    } else {
        None
    }
}

fn spans_plain_text(spans: &[Span]) -> String {
    let mut out = String::new();
    for sp in spans {
        match sp {
            Span::Text { text, .. } => out.push_str(text),
            Span::WikiLink { display, .. } => out.push_str(display),
            Span::LineBreak => out.push(' '),
        }
    }
    out
}

fn strip_task_meta_tokens_from_spans(spans: &[Span], today: SimpleDate) -> Vec<Span> {
    let mut out = Vec::with_capacity(spans.len());
    for sp in spans {
        match sp {
            Span::Text {
                text,
                bold,
                italic,
                strike,
                code,
                url,
            } if !*code && url.is_none() => {
                let mut rebuilt = String::new();
                for word in text.split_whitespace() {
                    let meta = parse_inline_task_meta(word, today);
                    if meta.due_date.is_some() || meta.priority.is_some() {
                        continue;
                    }
                    if !rebuilt.is_empty() {
                        rebuilt.push(' ');
                    }
                    rebuilt.push_str(word);
                }
                if text.starts_with(' ') && !rebuilt.is_empty() && !out.is_empty() {
                    rebuilt.insert(0, ' ');
                }
                if text.ends_with(' ') && !rebuilt.is_empty() {
                    rebuilt.push(' ');
                }
                if !rebuilt.is_empty() {
                    out.push(Span::Text {
                        text: rebuilt,
                        bold: *bold,
                        italic: *italic,
                        strike: *strike,
                        code: false,
                        url: None,
                    });
                }
            }
            other => out.push(other.clone()),
        }
    }
    out
}

fn render_task_badge(ui: &mut Ui, icon: Option<Icon>, label: &str, bg: Color32, fg: Color32) {
    let icon_w = if icon.is_some() { 14.0 } else { 0.0 };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(11.0), fg);
    let w = g.size().x + icon_w + 12.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, 19.0), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(5.0), bg);
    if let Some(ic) = icon {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(rect.left() + 10.5, rect.center().y), vec2(11.0, 11.0)),
            ic,
            fg,
        );
    }
    ui.painter().galley(
        pos2(
            rect.left() + 6.0 + icon_w,
            rect.center().y - g.size().y * 0.5,
        ),
        g,
        fg,
    );
}

fn task_pop_btn(ui: &mut Ui, p: &ThemePalette, label: &str, accent: bool) -> egui::Response {
    let fg = if accent { p.accent_text } else { p.text_secondary };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(11.5), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 22.0), Sense::click());
    let bg = if accent {
        if resp.hovered() {
            lerp_color(p.accent_soft, p.accent, 0.25)
        } else {
            p.accent_soft
        }
    } else if resp.hovered() {
        p.bg_hover
    } else {
        p.bg_input
    };
    ui.painter().rect_filled(rect, Rounding::same(5.0), bg);
    ui.painter().galley(
        pos2(rect.left() + 7.0, rect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggles_nth_task_ignoring_code_blocks() {
        let md = "```md\n- [ ] fake inside code\n```\n- [ ] first\n- [x] second\n";
        let toggled = toggle_markdown_task(md, 1).unwrap();
        assert!(toggled.contains("- [ ] fake inside code"));
        assert!(toggled.contains("- [ ] first\n- [ ] second"));
    }

    #[test]
    fn parses_images_and_embedded_boards() {
        let md = "Intro\n\n![Shot](assets/shot.png)\n\n![[assets/photo.jpg|Photo]]\n\n![[projects/tasks.board.json]]\n";
        let blocks = parse_blocks(md);
        assert!(blocks.iter().any(|b| matches!(
            b,
            Block::Image { path, alt } if path == "assets/shot.png" && alt == "Shot"
        )));
        assert!(blocks.iter().any(|b| matches!(
            b,
            Block::Image { path, alt } if path == "assets/photo.jpg" && alt == "Photo"
        )));
        assert!(blocks.iter().any(|b| matches!(
            b,
            Block::EmbeddedBoard { target } if target == "projects/tasks.board.json"
        )));
    }
}
