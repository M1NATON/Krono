use crate::i18n::{strings, Lang};
use crate::storage::vault::CustomTemplateInfo;
use crate::ui::components::{
    focus_once, hairline_h, icon_button, primary_button, reset_focus_gate, secondary_button,
    styled_text_input,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_semibold, ThemePalette};
use crate::ui::{CreateItemKind, UiAction};
use eframe::egui::{self, pos2, vec2, Align2, Color32, CursorIcon, Margin, Rect, Rounding, Sense, Stroke};

#[derive(Clone, Debug)]
pub struct CreateModalState {
    pub open: bool,
    pub kind: CreateItemKind,
    pub title: String,
    pub folder: String,
    pub preset_id: String,
}

impl Default for CreateModalState {
    fn default() -> Self {
        Self {
            open: false,
            kind: CreateItemKind::Note,
            title: String::new(),
            folder: String::new(),
            preset_id: default_preset_id(CreateItemKind::Note).to_string(),
        }
    }
}

impl CreateModalState {
    pub fn open_with(&mut self, kind: CreateItemKind) {
        self.open = true;
        self.kind = kind;
        self.title.clear();
        self.folder.clear();
        self.preset_id = default_preset_id(kind).to_string();
    }
}

struct CreatePreset {
    id: &'static str,
    icon: Icon,
    title_ru: &'static str,
    desc_ru: &'static str,
    title_en: &'static str,
    desc_en: &'static str,
}

fn default_preset_id(kind: CreateItemKind) -> &'static str {
    match kind {
        CreateItemKind::Note => "blank",
        CreateItemKind::Board => "kanban",
        CreateItemKind::Table => "general",
        CreateItemKind::Canvas => "storyboard",
    }
}

fn presets_for_kind(kind: CreateItemKind) -> &'static [CreatePreset] {
    match kind {
        CreateItemKind::Note => &[
            CreatePreset {
                id: "blank",
                icon: Icon::Document,
                title_ru: "Пустая страница",
                desc_ru: "Чистый Markdown-документ",
                title_en: "Blank Page",
                desc_en: "Clean empty Markdown note",
            },
            CreatePreset {
                id: "meeting",
                icon: Icon::CalendarDays,
                title_ru: "Встреча / Созвон",
                desc_ru: "Участники, повестка и задачи",
                title_en: "Meeting Notes",
                desc_en: "Agenda, decisions & actions",
            },
            CreatePreset {
                id: "concept",
                icon: Icon::Sparkles,
                title_ru: "Конспект / Идея",
                desc_ru: "Главная мысль, тезисы и [[связи]]",
                title_en: "Concept / Idea",
                desc_en: "Thesis, takeaways & [[links]]",
            },
            CreatePreset {
                id: "script",
                icon: Icon::FileOutput,
                title_ru: "Сценарий / Статья",
                desc_ru: "Хук, структура и чек-лист",
                title_en: "Script / Article",
                desc_en: "Hook, outline & publish check",
            },
            CreatePreset {
                id: "english_lesson",
                icon: Icon::Globe,
                title_ru: "Урок английского",
                desc_ru: "Словарь, грамматика и практика",
                title_en: "English Lesson",
                desc_en: "Vocabulary, grammar & practice",
            },
        ],
        CreateItemKind::Board => &[
            CreatePreset {
                id: "kanban",
                icon: Icon::Board,
                title_ru: "Канбан-доска",
                desc_ru: "Идея · В работе · Проверка · Готово",
                title_en: "Kanban Flow",
                desc_en: "Idea · Progress · Review · Done",
            },
            CreatePreset {
                id: "content",
                icon: Icon::Sparkles,
                title_ru: "Контент-план",
                desc_ru: "Идеи, сценарий, съёмка, вышло",
                title_en: "Content Plan",
                desc_en: "Ideas, script, filming, published",
            },
            CreatePreset {
                id: "sprint",
                icon: Icon::Flame,
                title_ru: "Спринт задач",
                desc_ru: "Бэклог, ревью и оценка в часах",
                title_en: "Work Sprint",
                desc_en: "Backlog, review & point estimate",
            },
            CreatePreset {
                id: "blank",
                icon: Icon::Plus,
                title_ru: "Чистая доска",
                desc_ru: "3 базовые колонки без шаблонов",
                title_en: "Clean 3-Column",
                desc_en: "Three clean starter columns",
            },
        ],
        CreateItemKind::Table => &[
            CreatePreset {
                id: "general",
                icon: Icon::Table,
                title_ru: "Универсальная",
                desc_ru: "Статус, детали, оценка, дата",
                title_en: "General Tracker",
                desc_en: "Status, notes, score, date",
            },
            CreatePreset {
                id: "subscriptions",
                icon: Icon::Check,
                title_ru: "Подписки и траты",
                desc_ru: "Категория, сумма/мес, дата",
                title_en: "Subscriptions",
                desc_en: "Category, monthly cost, renewal",
            },
            CreatePreset {
                id: "reading",
                icon: Icon::Notes,
                title_ru: "Книги и курсы",
                desc_ru: "Автор, статус, оценка, конспект",
                title_en: "Reading List",
                desc_en: "Author, progress, rating, notes",
            },
            CreatePreset {
                id: "crm",
                icon: Icon::Link,
                title_ru: "Контакты / CRM",
                desc_ru: "Компания, статус, канал, шаг",
                title_en: "Contacts / CRM",
                desc_en: "Role, stage, channel, follow-up",
            },
            CreatePreset {
                id: "vocabulary",
                icon: Icon::Globe,
                title_ru: "Словарь (English)",
                desc_ru: "Слова, уровень и повторение",
                title_en: "Vocabulary (SRS)",
                desc_en: "Words, level & spaced review",
            },
        ],
        CreateItemKind::Canvas => &[
            CreatePreset {
                id: "storyboard",
                icon: Icon::Canvas,
                title_ru: "Раскадровка",
                desc_ru: "3 секции со связанными карточками",
                title_en: "Storyboard",
                desc_en: "3 linked sections & flow",
            },
            CreatePreset {
                id: "mindmap",
                icon: Icon::Graph,
                title_ru: "Брейншторм",
                desc_ru: "Центральная тема и 4 ветки",
                title_en: "Mindmap",
                desc_en: "Central node + 4 connected branches",
            },
            CreatePreset {
                id: "architecture",
                icon: Icon::Group,
                title_ru: "Архитектура",
                desc_ru: "Клиент, логика и данные со связями",
                title_en: "Architecture",
                desc_en: "Client, logic & storage blocks",
            },
            CreatePreset {
                id: "blank",
                icon: Icon::StickyNote,
                title_ru: "Пустой холст",
                desc_ru: "Чистая бесконечная сетка",
                title_en: "Blank Canvas",
                desc_en: "Empty infinite dot canvas",
            },
        ],
    }
}

pub fn render_create_item_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CreateModalState,
    existing_folders: &[String],
    custom_templates: &[CustomTemplateInfo],
) -> Option<UiAction> {
    let gate_id = egui::Id::new("create_modal_foc_gate");
    let anim_t = anim_bool_eased(ctx, egui::Id::new("create_modal_anim"), state.open, 0.14);
    if !state.open {
        reset_focus_gate(ctx, gate_id);
        return None;
    }

    let s = strings(lang);
    let mut created: Option<UiAction> = None;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.open = false;
        return None;
    }

    render_modal_backdrop(ctx, p, "create_item_backdrop", anim_t, || {
        state.open = false;
    });

    let screen = ctx.screen_rect();
    let max_modal_h = (screen.height() - 48.0).clamp(300.0, 620.0);
    let top_y = ((screen.height() - max_modal_h) * 0.42).clamp(18.0, 88.0) - (1.0 - anim_t) * 10.0;
    let is_compact = screen.width() < 460.0;
    let modal_margin = if is_compact { 14.0 } else { 18.0 };
    let modal_w = 500.0_f32.min((screen.width() - 28.0).max(276.0));
    let mut shadow = p.shadow_floating;
    shadow.color = with_alpha(shadow.color, anim_t);

    egui::Area::new(egui::Id::new("create_item_modal"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, top_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(with_alpha(p.bg_elevated, anim_t))
                .stroke(Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)))
                .rounding(Rounding::same(13.0))
                .shadow(shadow)
                .inner_margin(Margin::same(modal_margin))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.set_max_height(max_modal_h);

                    // Pinned Header
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(s.create_modal_title)
                                .font(font_semibold(16.5))
                                .color(p.text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                                state.open = false;
                            }
                        });
                    });
                    ui.add_space(10.0);

                    // Scrollable Middle Body
                    let body_max_h = (max_modal_h - 110.0).max(160.0);
                    egui::ScrollArea::vertical()
                        .id_salt("create_modal_scroll")
                        .max_height(body_max_h)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            // 1. 4 Type Tabs (Note / Board / Table / Canvas)
                            let kinds = [
                                (CreateItemKind::Note, Icon::Document, s.create_kind_note),
                                (CreateItemKind::Board, Icon::Board, s.create_kind_board),
                                (CreateItemKind::Table, Icon::Table, s.create_kind_table),
                                (CreateItemKind::Canvas, Icon::Canvas, s.create_kind_canvas),
                            ];
                            let cols = if modal_w < 380.0 { 2 } else { 4 };
                            let gap = 7.0;
                            let cell_w = ((modal_w - gap * (cols as f32 - 1.0)) / cols as f32).max(70.0);
                            for row in kinds.chunks(cols) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    for &(k, ic, lbl) in row {
                                        let sel = state.kind == k;
                                        let (kr, kresp) = ui.allocate_exact_size(vec2(cell_w, 34.0), Sense::click());
                                        let khov = anim_bool_eased(
                                            ui.ctx(),
                                            ui.make_persistent_id(("cm_kind", lbl)),
                                            kresp.hovered(),
                                            0.10,
                                        );
                                        let bg = if sel {
                                            p.accent_soft
                                        } else {
                                            lerp_color(p.bg_input, p.bg_hover, khov)
                                        };
                                        let brd = if sel { p.accent } else { p.border_subtle };
                                        let fg = if sel { p.text_primary } else { p.text_secondary };
                                        let ic_col = if sel { p.accent_text } else { p.text_secondary };
                                        ui.painter().rect_filled(kr, Rounding::same(8.0), bg);
                                        ui.painter().rect_stroke(
                                            kr,
                                            Rounding::same(8.0),
                                            Stroke::new(if sel { 1.4_f32 } else { 1.0_f32 }, brd),
                                        );
                                        draw_icon(
                                            ui.painter(),
                                            Rect::from_center_size(pos2(kr.left() + 17.0, kr.center().y), vec2(14.0, 14.0)),
                                            ic,
                                            ic_col,
                                        );
                                        let lg = ui.painter().layout_no_wrap(lbl.to_string(), font_medium(12.5), fg);
                                        ui.painter().galley(
                                            pos2(kr.left() + 29.0, kr.center().y - lg.size().y * 0.5),
                                            lg,
                                            fg,
                                        );
                                        if kresp.on_hover_cursor(CursorIcon::PointingHand).clicked() && state.kind != k {
                                            state.kind = k;
                                            state.preset_id = default_preset_id(k).to_string();
                                        }
                                    }
                                });
                                ui.add_space(6.0);
                            }

                            ui.add_space(4.0);

                            // 2. Title Field
                            ui.label(
                                egui::RichText::new(s.create_name_label)
                                    .font(font_medium(12.0))
                                    .color(p.text_secondary),
                            );
                            ui.add_space(4.0);
                            let t_resp = styled_text_input(
                                ui,
                                p,
                                &mut state.title,
                                s.create_name_hint,
                                false,
                                f32::INFINITY,
                            );
                            focus_once(ui.ctx(), gate_id, true, t_resp.id);
                            ui.add_space(10.0);

                            // 3. Optional Folder Input + Quick-Pick Chips
                            ui.label(
                                egui::RichText::new(s.create_folder_label)
                                    .font(font_medium(12.0))
                                    .color(p.text_secondary),
                            );
                            ui.add_space(4.0);
                            styled_text_input(
                                ui,
                                p,
                                &mut state.folder,
                                s.create_folder_hint,
                                false,
                                f32::INFINITY,
                            );
                            ui.add_space(5.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                                if folder_pick_chip(ui, p, s.create_folder_root, state.folder.trim().is_empty()).clicked() {
                                    state.folder.clear();
                                }
                                for fld in existing_folders.iter().take(6) {
                                    let active = state.folder.trim().eq_ignore_ascii_case(fld);
                                    if folder_pick_chip(ui, p, fld, active).clicked() {
                                        state.folder = fld.clone();
                                    }
                                }
                            });

                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new(s.create_preset_label)
                                    .font(font_medium(12.0))
                                    .color(p.text_secondary),
                            );
                            ui.add_space(6.0);

                            // 4. 2x2 Curated Preset Cards
                            let presets = presets_for_kind(state.kind);
                            let pcols = if modal_w < 300.0 { 1 } else { 2 };
                            let pcard_w = ((modal_w - gap * (pcols as f32 - 1.0)) / pcols as f32).max(110.0);
                            let pcard_h = if pcard_w < 195.0 { 66.0 } else { 58.0 };
                            for row in presets.chunks(pcols) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    for pr in row {
                                        let sel = state.preset_id == pr.id;
                                        let title = if lang == Lang::Ru { pr.title_ru } else { pr.title_en };
                                        let desc = if lang == Lang::Ru { pr.desc_ru } else { pr.desc_en };
                                        let (rect, resp) = ui.allocate_exact_size(vec2(pcard_w, pcard_h), Sense::click());
                                        let hov = anim_bool_eased(
                                            ui.ctx(),
                                            ui.make_persistent_id(("cm_pr", state.kind as u8, pr.id)),
                                            resp.hovered(),
                                            0.10,
                                        );
                                        let bg = if sel {
                                            p.accent_soft
                                        } else {
                                            lerp_color(p.bg_input, p.bg_hover, hov)
                                        };
                                        ui.painter().rect_filled(rect, Rounding::same(8.0), bg);
                                        ui.painter().rect_stroke(
                                            rect,
                                            Rounding::same(8.0),
                                            Stroke::new(if sel { 1.4_f32 } else { 1.0_f32 }, if sel { p.accent } else { p.border_hairline }),
                                        );
                                        let title_col = p.text_primary;
                                        let desc_col = if sel {
                                            lerp_color(p.text_primary, p.text_secondary, 0.30)
                                        } else {
                                            p.text_secondary
                                        };
                                        draw_icon(
                                            ui.painter(),
                                            Rect::from_center_size(pos2(rect.left() + 17.0, rect.top() + 18.0), vec2(14.0, 14.0)),
                                            pr.icon,
                                            if sel { p.accent_text } else { p.text_secondary },
                                        );
                                        let tg = ui.painter().layout(
                                            title.to_string(),
                                            font_medium(if pcard_w < 195.0 { 11.8 } else { 12.5 }),
                                            title_col,
                                            (pcard_w - 36.0).max(60.0),
                                        );
                                        ui.painter().galley(pos2(rect.left() + 29.0, rect.top() + 10.0), tg, title_col);
                                        let dg = ui.painter().layout(
                                            desc.to_string(),
                                            font_body(if pcard_w < 195.0 { 10.2 } else { 10.8 }),
                                            desc_col,
                                            (pcard_w - 18.0).max(60.0),
                                        );
                                        ui.painter().galley(pos2(rect.left() + 10.0, rect.top() + 30.0), dg, desc_col);

                                        if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                            state.preset_id = pr.id.to_string();
                                        }
                                    }
                                });
                                ui.add_space(gap);
                            }

                            if state.kind == CreateItemKind::Note && !custom_templates.is_empty() {
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(s.create_custom_templates_label)
                                        .font(font_medium(12.0))
                                        .color(p.text_secondary),
                                );
                                ui.add_space(6.0);
                                for row in custom_templates.chunks(pcols) {
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = gap;
                                        for tpl in row {
                                            let cid = format!("custom:{}", tpl.id);
                                            let sel = state.preset_id == cid;
                                            let (rect, resp) = ui.allocate_exact_size(vec2(pcard_w, 46.0), Sense::click());
                                            let bg = if sel { p.accent_soft } else { p.bg_input };
                                            ui.painter().rect_filled(rect, Rounding::same(8.0), bg);
                                            ui.painter().rect_stroke(
                                                rect,
                                                Rounding::same(8.0),
                                                Stroke::new(if sel { 1.4_f32 } else { 1.0_f32 }, if sel { p.accent } else { p.border_hairline }),
                                            );
                                            draw_icon(
                                                ui.painter(),
                                                Rect::from_center_size(pos2(rect.left() + 17.0, rect.center().y), vec2(14.0, 14.0)),
                                                Icon::Document,
                                                if sel { p.accent_text } else { p.text_secondary },
                                            );
                                            let tg = ui.painter().layout(
                                                tpl.name.clone(),
                                                font_medium(12.0),
                                                p.text_primary,
                                                (pcard_w - 36.0).max(60.0),
                                            );
                                            ui.painter().galley(pos2(rect.left() + 29.0, rect.center().y - tg.size().y * 0.5), tg, p.text_primary);
                                            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                                state.preset_id = cid;
                                            }
                                        }
                                    });
                                    ui.add_space(gap);
                                }
                            }
                        });

                    ui.add_space(8.0);
                    hairline_h(ui, p);
                    ui.add_space(10.0);

                    // Pinned Footer
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked()
                            || enter
                        {
                            let presets = presets_for_kind(state.kind);
                            let default_title = presets
                                .iter()
                                .find(|pr| pr.id == state.preset_id)
                                .map(|pr| if lang == Lang::Ru { pr.title_ru } else { pr.title_en })
                                .unwrap_or("New");
                            let raw_title = state.title.trim();
                            let title = if raw_title.is_empty() {
                                default_title.to_string()
                            } else {
                                raw_title.to_string()
                            };
                            created = Some(UiAction::CreateFromModal {
                                kind: state.kind,
                                title,
                                folder: state.folder.trim().to_string(),
                                preset_id: state.preset_id.clone(),
                            });
                            state.open = false;
                        }
                        if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                            state.open = false;
                        }
                    });
                });
        });

    created
}

fn folder_pick_chip(ui: &mut egui::Ui, p: &ThemePalette, label: &str, active: bool) -> egui::Response {
    let fg = if active { p.accent_text } else { p.text_secondary };
    let g = ui.painter().layout_no_wrap(label.to_string(), font_medium(11.0), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 14.0, 21.0), Sense::click());
    ui.painter().rect_filled(
        rect,
        Rounding::same(5.0),
        if active {
            p.accent_soft
        } else if resp.hovered() {
            p.bg_hover
        } else {
            p.bg_input
        },
    );
    if active {
        ui.painter()
            .rect_stroke(rect, Rounding::same(5.0), Stroke::new(1.0_f32, p.accent));
    }
    ui.painter().galley(
        pos2(rect.center().x - g.size().x * 0.5, rect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn render_rename_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    rename_target: &mut Option<String>,
    rename_draft: &mut String,
) -> Option<(String, String)> {
    let is_open = rename_target.is_some();
    let gate_id = egui::Id::new("rename_modal_foc_gate");
    let anim_t = anim_bool_eased(ctx, egui::Id::new("rename_modal_anim"), is_open, 0.14);
    let Some(target) = rename_target.clone() else {
        reset_focus_gate(ctx, gate_id);
        return None;
    };
    let s = strings(lang);
    let mut confirmed = None;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *rename_target = None;
        return None;
    }

    render_modal_backdrop(ctx, p, "rename_backdrop", anim_t, || {
        *rename_target = None;
    });

    let glide_y = -(1.0 - anim_t) * 10.0;
    let mut shadow = p.shadow_floating;
    shadow.color = with_alpha(shadow.color, anim_t);

    egui::Window::new(s.rename_dialog_title)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, glide_y))
        .order(egui::Order::Foreground)
        .frame(
            egui::Frame::none()
                .fill(with_alpha(p.bg_elevated, anim_t))
                .stroke(Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)))
                .rounding(Rounding::same(12.0))
                .shadow(shadow)
                .inner_margin(Margin::same(20.0)),
        )
        .show(ctx, |ui| {
            let modal_w = 356.0_f32.min((ctx.screen_rect().width() - 56.0).max(240.0));
            ui.set_width(modal_w);
            ui.label(
                egui::RichText::new(s.rename_dialog_title)
                    .font(font_semibold(16.0))
                    .color(with_alpha(p.text_primary, anim_t)),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(s.rename_dialog_label)
                    .font(font_body(12.5))
                    .color(with_alpha(p.text_secondary, anim_t)),
            );
            ui.add_space(6.0);
            let resp = styled_text_input(ui, p, rename_draft, "", false, f32::INFINITY);
            focus_once(ui.ctx(), gate_id, true, resp.id);
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if primary_button(ui, p, None, s.apply_btn, false).clicked() || enter {
                    let next = rename_draft.trim().to_string();
                    if !next.is_empty() {
                        confirmed = Some((target.clone(), next));
                        *rename_target = None;
                    }
                }
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    *rename_target = None;
                }
            });
        });

    confirmed
}

pub fn render_delete_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    delete_target: &mut Option<String>,
) -> Option<String> {
    let is_open = delete_target.is_some();
    let anim_t = anim_bool_eased(ctx, egui::Id::new("delete_modal_anim"), is_open, 0.14);
    let target = delete_target.clone()?;
    let s = strings(lang);
    let mut confirmed = None;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *delete_target = None;
        return None;
    }

    render_modal_backdrop(ctx, p, "delete_backdrop", anim_t, || {
        *delete_target = None;
    });

    let glide_y = -(1.0 - anim_t) * 10.0;
    let mut shadow = p.shadow_floating;
    shadow.color = with_alpha(shadow.color, anim_t);

    egui::Window::new(s.delete_dialog_title)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, glide_y))
        .order(egui::Order::Foreground)
        .frame(
            egui::Frame::none()
                .fill(with_alpha(p.bg_elevated, anim_t))
                .stroke(Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)))
                .rounding(Rounding::same(12.0))
                .shadow(shadow)
                .inner_margin(Margin::same(20.0)),
        )
        .show(ctx, |ui| {
            let modal_w = 370.0_f32.min((ctx.screen_rect().width() - 56.0).max(240.0));
            ui.set_width(modal_w);
            ui.label(
                egui::RichText::new(s.delete_dialog_title)
                    .font(font_semibold(16.0))
                    .color(with_alpha(p.danger, anim_t)),
            );
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(s.delete_dialog_confirm)
                        .font(font_body(13.0))
                        .color(with_alpha(p.text_secondary, anim_t)),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            egui::Frame::none()
                .fill(with_alpha(p.bg_input, anim_t))
                .stroke(Stroke::new(1.0_f32, with_alpha(p.border_hairline, anim_t)))
                .rounding(Rounding::same(6.0))
                .inner_margin(Margin::symmetric(10.0, 6.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(&target)
                            .font(font_medium(12.5))
                            .color(with_alpha(p.text_primary, anim_t)),
                    );
                });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let del_btn = egui::Button::new(
                    egui::RichText::new(s.delete_btn)
                        .font(font_medium(13.0))
                        .color(with_alpha(Color32::WHITE, anim_t)),
                )
                .fill(with_alpha(p.danger, anim_t))
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(6.0));
                if ui.add_sized(vec2(96.0, 29.0), del_btn).clicked() {
                    confirmed = Some(target.clone());
                    *delete_target = None;
                }
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    *delete_target = None;
                }
            });
        });

    confirmed
}

fn render_modal_backdrop(
    ctx: &egui::Context,
    p: &ThemePalette,
    id: &str,
    anim_t: f32,
    on_dismiss: impl FnOnce(),
) {
    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, (max_alpha * anim_t).round() as u8),
            );
            if resp.clicked() {
                on_dismiss();
            }
        });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstRunAction {
    StartWorkspace,
    StartAndConfigureSync,
}

pub fn render_first_run_screen(
    ui: &mut egui::Ui,
    p: &ThemePalette,
    lang: Lang,
    vault_draft: &mut String,
) -> Option<FirstRunAction> {
    let s = strings(lang);
    let mut action = None;
    let avail = ui.available_width();
    let card_w = 520.0_f32.min((avail - 24.0).max(280.0));
    let side_pad = ((avail - card_w) * 0.5).max(12.0);

    ui.add_space((ui.available_height() * 0.14).max(24.0));
    ui.horizontal(|ui| {
        ui.add_space(side_pad);
        egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_subtle))
            .rounding(Rounding::same(14.0))
            .shadow(p.shadow_floating)
            .inner_margin(Margin::same(24.0))
            .show(ui, |ui| {
                ui.set_width((card_w - 48.0).max(220.0));
                ui.horizontal(|ui| {
                    let (lr, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    draw_icon(ui.painter(), lr, Icon::Logo, p.accent);
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(s.first_run_title)
                            .font(font_semibold(20.0))
                            .color(p.text_primary),
                    );
                });
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(s.first_run_desc)
                        .font(font_body(13.0))
                        .color(p.text_secondary),
                );

                ui.add_space(18.0);
                ui.label(
                    egui::RichText::new(s.first_run_folder_label)
                        .font(font_medium(12.5))
                        .color(p.text_secondary),
                );
                ui.add_space(6.0);
                let input_w = (ui.available_width() - 148.0).max(150.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                    styled_text_input(ui, p, vault_draft, "", false, input_w);
                    if secondary_button(ui, p, Some(Icon::Folder), s.first_run_browse_btn)
                        .clicked()
                    {
                        let mut dlg = rfd::FileDialog::new();
                        let cur = std::path::Path::new(vault_draft.trim());
                        if cur.exists() {
                            dlg = dlg.set_directory(cur);
                        }
                        if let Some(chosen) = dlg.pick_folder() {
                            *vault_draft = chosen.to_string_lossy().to_string();
                        }
                    }
                });

                ui.add_space(14.0);
                egui::Frame::none()
                    .fill(p.bg_card)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 9.0))
                    .show(ui, |ui| {
                        ui.horizontal_top(|ui| {
                            let (ir, _) =
                                ui.allocate_exact_size(vec2(14.0, 16.0), Sense::hover());
                            draw_icon(ui.painter(), ir, Icon::GitHub, p.accent_text);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(s.first_run_sync_tip)
                                    .font(font_body(12.0))
                                    .color(p.text_secondary),
                            );
                        });
                    });

                ui.add_space(18.0);
                let can_start = !vault_draft.trim().is_empty();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                    ui.add_enabled_ui(can_start, |ui| {
                        if primary_button(
                            ui,
                            p,
                            Some(Icon::Check),
                            s.first_run_start_btn,
                            false,
                        )
                        .clicked()
                        {
                            action = Some(FirstRunAction::StartWorkspace);
                        }
                        if secondary_button(ui, p, Some(Icon::GitHub), s.first_run_sync_btn)
                            .clicked()
                        {
                            action = Some(FirstRunAction::StartAndConfigureSync);
                        }
                    });
                });
            });
    });

    action
}

pub fn render_create_subfolder_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    parent_folder: &mut Option<String>,
    draft: &mut String,
) -> Option<(String, String)> {
    let Some(parent) = parent_folder.clone() else {
        reset_focus_gate(ctx, egui::Id::new("krono_subf_focus_gate"));
        return None;
    };
    let s = strings(lang);
    let mut created: Option<(String, String)> = None;

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *parent_folder = None;
        draft.clear();
        return None;
    }

    render_modal_backdrop(ctx, p, "krono_subfolder_backdrop", 1.0, || {
        *parent_folder = None;
        draft.clear();
    });

    let modal_w = 390.0_f32.min((ctx.screen_rect().width() - 32.0).max(260.0));
    egui::Window::new("krono_create_subfolder_modal")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, -30.0))
        .fixed_size(vec2(modal_w, 0.0))
        .frame(
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(18.0)),
        )
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(s.subfolder_modal_title)
                    .font(font_semibold(16.0))
                    .color(p.text_primary),
            );
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{}: {parent}", s.create_folder_label))
                    .font(font_body(12.0))
                    .color(p.text_muted),
            );
            ui.add_space(10.0);

            let resp = styled_text_input(
                ui,
                p,
                draft,
                s.subfolder_modal_hint,
                false,
                f32::INFINITY,
            );
            focus_once(ctx, egui::Id::new("krono_subf_focus_gate"), true, resp.id);
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if (primary_button(ui, p, Some(Icon::Plus), s.create_btn, false).clicked()
                    || enter)
                    && !draft.trim().is_empty()
                {
                    created = Some((parent.clone(), draft.trim().to_string()));
                    *parent_folder = None;
                    draft.clear();
                }
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    *parent_folder = None;
                    draft.clear();
                }
            });
        });

    created
}
