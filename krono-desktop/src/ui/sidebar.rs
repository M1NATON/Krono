use crate::i18n::{format_relative_time, month_name, strings, Lang};
use crate::storage::index::CollectionMeta;
use crate::ui::calendar_widget::render_diary_calendar;
use crate::ui::components::{
    context_menu_row, draw_outer_right_border, hairline_h, icon_button, paint_badge_dot,
    section_heading, shortcut_label, sidebar_row, tag_chip,
};
use crate::ui::icons::{draw_icon, draw_sync_icon, Icon};
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{
    CalendarUiState, CreateItemKind, PagesKindFilter, PagesSortOrder, PagesTreeUiState,
    SettingsTab, SidebarModule, SidebarModulesState, SyncUiState, UiAction, ViewMode,
};
use eframe::egui::{
    self, pos2, vec2, Color32, CursorIcon, Margin, Rect, Rounding, Sense, Stroke, Ui,
};
use krono_core::{
    daily::is_daily_hub_path,
    note::NoteMeta,
    path::VaultFileKind,
};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Copy)]
enum TreeEntry<'a> {
    Note(&'a NoteMeta),
    Collection(&'a CollectionMeta),
}

impl<'a> TreeEntry<'a> {
    fn path(self) -> &'a str {
        match self {
            Self::Note(n) => &n.path,
            Self::Collection(c) => &c.path,
        }
    }

    fn title(self) -> &'a str {
        match self {
            Self::Note(n) => &n.title,
            Self::Collection(c) => &c.title,
        }
    }

    fn modified(self) -> i64 {
        match self {
            Self::Note(n) => n.modified,
            Self::Collection(c) => c.modified,
        }
    }

    fn pinned(self) -> bool {
        match self {
            Self::Note(n) => n.pinned,
            Self::Collection(c) => c.pinned,
        }
    }

    fn icon(self, in_pinned: bool) -> Icon {
        match self {
            Self::Note(_) => {
                if in_pinned {
                    Icon::Pin
                } else {
                    Icon::Document
                }
            }
            Self::Collection(c) => match c.kind {
                VaultFileKind::Table => Icon::Table,
                VaultFileKind::Canvas => Icon::Canvas,
                _ => Icon::Board,
            },
        }
    }

    fn is_selected(
        self,
        view_mode: ViewMode,
        selected_note_path: Option<&str>,
        selected_col_path: Option<&str>,
        selected_canvas_path: Option<&str>,
    ) -> bool {
        match self {
            Self::Note(n) => view_mode == ViewMode::Editor && selected_note_path == Some(&n.path),
            Self::Collection(c) => match c.kind {
                VaultFileKind::Canvas => {
                    view_mode == ViewMode::Canvas && selected_canvas_path == Some(&c.path)
                }
                _ => view_mode == ViewMode::Collection && selected_col_path == Some(&c.path),
            },
        }
    }

    fn push_open(self, actions: &mut Vec<UiAction>) {
        match self {
            Self::Note(n) => {
                actions.push(UiAction::OpenNote(n.path.clone()));
                actions.push(UiAction::SetViewMode(ViewMode::Editor));
            }
            Self::Collection(c) => match c.kind {
                VaultFileKind::Canvas => actions.push(UiAction::OpenCanvas(c.path.clone())),
                _ => actions.push(UiAction::OpenCollection(c.path.clone())),
            },
        }
    }
}

#[derive(Default)]
struct FolderNode<'a> {
    subfolders: BTreeMap<String, FolderNode<'a>>,
    entries: Vec<TreeEntry<'a>>,
}

fn ensure_folder_path<'a, 'b>(mut node: &'b mut FolderNode<'a>, folder_path: &str) {
    for seg in folder_path.split('/').filter(|s| !s.is_empty()) {
        node = node.subfolders.entry(seg.to_string()).or_default();
    }
}

fn insert_into_tree<'a>(node: &mut FolderNode<'a>, parts: &[&str], entry: TreeEntry<'a>) {
    if parts.len() <= 1 {
        node.entries.push(entry);
    } else {
        let child = node.subfolders.entry(parts[0].to_string()).or_default();
        insert_into_tree(child, &parts[1..], entry);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_sidebar(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    sidebar_width: &mut f32,
    anim_t: f32,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_folders: &[String],
    tag_counts: &[(String, usize)],
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    active_tag: Option<&str>,
    expanded_folders: &HashSet<String>,
    pinned_folders: &[String],
    folder_order: &[String],
    view_mode: ViewMode,
    pages_ui: &mut PagesTreeUiState<'_>,
    cal: &mut CalendarUiState<'_>,
    sync: SyncUiState<'_>,
    due_tasks_badge: usize,
    modules: SidebarModulesState,
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let target_w = (*sidebar_width).clamp(210.0, 340.0);
    let current_w = (target_w * anim_t).max(1.0);
    let settled = anim_t > 0.99;

    let mut panel = egui::SidePanel::left("krono_sidebar")
        .show_separator_line(false)
        .resizable(settled)
        .frame(
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .inner_margin(Margin::symmetric(12.0, 12.0)),
        );
    panel = if settled {
        panel.default_width(target_w).width_range(210.0..=340.0)
    } else {
        panel.exact_width(current_w)
    };

    let panel_resp = panel.show(ctx, |ui| {
        if anim_t < 0.35 {
            return;
        }
        ui.set_clip_rect(ui.max_rect().expand(4.0));
        render_sidebar_contents(
            ctx,
            ui,
            p,
            lang,
            notes,
            collections,
            vault_folders,
            tag_counts,
            selected_path,
            selected_collection_path,
            selected_canvas_path,
            active_tag,
            expanded_folders,
            pinned_folders,
            folder_order,
            view_mode,
            pages_ui,
            cal,
            sync,
            due_tasks_badge,
            modules,
            &mut actions,
        );
    });

    draw_outer_right_border(ctx, panel_resp.response.rect, p);
    if settled {
        *sidebar_width = panel_resp.response.rect.width().clamp(210.0, 340.0);
    }
    actions
}

#[allow(clippy::too_many_arguments)]
pub fn render_sidebar_drawer(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    anim_t: f32,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_folders: &[String],
    tag_counts: &[(String, usize)],
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    active_tag: Option<&str>,
    expanded_folders: &HashSet<String>,
    _pinned_folders: &[String],
    _folder_order: &[String],
    view_mode: ViewMode,
    pages_ui: &mut PagesTreeUiState<'_>,
    cal: &mut CalendarUiState<'_>,
    sync: SyncUiState<'_>,
    due_tasks_badge: usize,
    modules: SidebarModulesState,
) -> (Vec<UiAction>, bool) {
    let mut actions = Vec::new();
    let mut dismiss = false;
    let screen = ctx.screen_rect();

    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    egui::Area::new(egui::Id::new("krono_sidebar_drawer_backdrop"))
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
                dismiss = true;
            }
        });

    let drawer_w = 276.0_f32.min((screen.width() - 48.0).max(220.0));
    let x = screen.left() - drawer_w * (1.0 - anim_t);
    let mut sh = p.shadow_floating;
    sh.color = with_alpha(sh.color, anim_t);

    let area_resp = egui::Area::new(egui::Id::new("krono_sidebar_drawer"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos2(x, screen.top()))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .shadow(sh)
                .inner_margin(Margin::symmetric(12.0, 12.0))
                .show(ui, |ui| {
                    let inner_w = (drawer_w - 24.0).max(160.0);
                    let inner_h = (screen.height() - 24.0).max(200.0);
                    ui.set_min_size(vec2(inner_w, inner_h));
                    ui.set_max_size(vec2(inner_w, inner_h));
                    if anim_t >= 0.3 {
                        render_sidebar_contents(
                            ctx,
                            ui,
                            p,
                            lang,
                            notes,
                            collections,
                            vault_folders,
                            tag_counts,
                            selected_path,
                            selected_collection_path,
                            selected_canvas_path,
                            active_tag,
                            expanded_folders,
                            &[],
                            &[],
                            view_mode,
                            pages_ui,
                            cal,
                            sync,
                            due_tasks_badge,
                            modules,
                            &mut actions,
                        );
                    }
                });
        });

    let dr = area_resp.response.rect;
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("krono_sidebar_drawer_border"),
    ))
    .line_segment(
        [pos2(dr.right().round(), dr.top()), pos2(dr.right().round(), dr.bottom())],
        Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)),
    );

    if actions.iter().any(|a| {
        matches!(
            a,
            UiAction::OpenNote(_)
                | UiAction::OpenCollection(_)
                | UiAction::OpenCanvas(_)
                | UiAction::CreateNote { .. }
                | UiAction::CreateCollection { .. }
                | UiAction::OpenCreateModal(_)
                | UiAction::CreateInFolder { .. }
                | UiAction::StartCreateSubfolder(_)
                | UiAction::ReturnToNotesTab
                | UiAction::SetViewMode(_)
                | UiAction::OpenSidebarModule(_)
                | UiAction::OpenPalette
                | UiAction::OpenSettings(_)
                | UiAction::CloseSettings
        )
    }) {
        dismiss = true;
    }

    (actions, dismiss)
}

#[allow(clippy::too_many_arguments)]
fn render_sidebar_contents(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_folders: &[String],
    tag_counts: &[(String, usize)],
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    active_tag: Option<&str>,
    expanded_folders: &HashSet<String>,
    pinned_folders: &[String],
    folder_order: &[String],
    view_mode: ViewMode,
    pages_ui: &mut PagesTreeUiState<'_>,
    cal: &mut CalendarUiState<'_>,
    sync: SyncUiState<'_>,
    due_tasks_badge: usize,
    modules: SidebarModulesState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);

    // 1. Quiet Header: Krono mark + Title + New Item Modal (+) + Collapse
    ui.horizontal(|ui| {
        let (logo_rect, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
        draw_icon(ui.painter(), logo_rect, Icon::Logo, p.text_primary);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(s.app_name)
                .font(font_semibold(15.0))
                .color(p.text_primary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if icon_button(
                ui,
                p,
                Icon::SidebarToggle,
                s.tooltip_collapse_sidebar,
                false,
            )
            .clicked()
            {
                actions.push(UiAction::ToggleSidebar);
            }
            if icon_button(ui, p, Icon::Plus, s.tooltip_new_note, false).clicked() {
                actions.push(UiAction::OpenCreateModal(None));
            }
        });
    });

    ui.add_space(10.0);

    // 2. Flat, Calm Search Trigger (`Ctrl/Cmd+K`)
    render_search_trigger(ctx, ui, p, lang, true, actions);

    ui.add_space(12.0);

    // 3. Gliding View Switcher
    {
        struct NavSlot {
            mode: ViewMode,
            icon: Icon,
            label: &'static str,
            rect: Rect,
            resp: egui::Response,
            active: bool,
        }
        let nav_w = ui.available_width().max(80.0);
        let mut slots = Vec::with_capacity(5);
        let mut active_rect: Option<Rect> = None;
        ui.spacing_mut().item_spacing.y = 2.0;

        for (mode, icon, label, visible) in [
            (ViewMode::Editor, Icon::Notes, s.nav_notes, true),
            (ViewMode::Tasks, Icon::Task, s.nav_tasks, modules.nav_tasks),
            (ViewMode::Table, Icon::Table, s.nav_table, modules.nav_table),
            (ViewMode::Board, Icon::Board, s.nav_board, modules.nav_board),
            (ViewMode::Graph, Icon::Graph, s.nav_graph, modules.nav_graph),
            (ViewMode::Overview, Icon::LayoutGrid, s.nav_overview, modules.nav_overview),
        ] {
            if !visible {
                continue;
            }
            let (rect, resp) = ui.allocate_exact_size(vec2(nav_w, 28.0), Sense::click());
            let active = view_mode == mode
                || (mode == ViewMode::Editor
                    && matches!(view_mode, ViewMode::Collection | ViewMode::Canvas));
            if active {
                active_rect = Some(rect);
            }
            slots.push(NavSlot {
                mode,
                icon,
                label,
                rect,
                resp,
                active,
            });
        }
        ui.spacing_mut().item_spacing.y = 6.0;

        if let Some(target) = active_rect {
            let pill_y = anim_value_smooth(
                ctx,
                egui::Id::new("sb_nav_pill_y"),
                target.top(),
                0.16,
            );
            let pill_r = Rect::from_min_size(pos2(target.left(), pill_y), target.size());
            ui.painter()
                .rect_filled(pill_r, Rounding::same(6.0), p.bg_selected);
            let bar_r = Rect::from_center_size(
                pos2(pill_r.left() + 3.0, pill_r.center().y),
                vec2(2.5, 14.0),
            );
            ui.painter()
                .rect_filled(bar_r, Rounding::same(1.5), p.accent);
        }

        for slot in slots {
            let hov = anim_bool_eased(
                ctx,
                ui.make_persistent_id(("nav_view", slot.label)),
                slot.resp.hovered(),
                0.10,
            );
            if !slot.active && hov > 0.01 {
                ui.painter().rect_filled(
                    slot.rect,
                    Rounding::same(6.0),
                    with_alpha(p.bg_hover, hov),
                );
            }
            let icon_col = if slot.active {
                p.text_primary
            } else {
                lerp_color(p.text_muted, p.text_secondary, hov)
            };
            let text_col = if slot.active {
                p.text_primary
            } else {
                lerp_color(p.text_secondary, p.text_primary, hov)
            };
            let ir = Rect::from_center_size(
                pos2(slot.rect.left() + 16.0, slot.rect.center().y),
                vec2(14.0, 14.0),
            );
            draw_icon(ui.painter(), ir, slot.icon, icon_col);
            let g = ui.painter().layout_no_wrap(
                slot.label.to_string(),
                if slot.active {
                    font_medium(13.0)
                } else {
                    font_body(13.0)
                },
                text_col,
            );
            ui.painter().galley(
                pos2(slot.rect.left() + 30.0, slot.rect.center().y - g.size().y * 0.5),
                g,
                text_col,
            );
            if slot.mode == ViewMode::Tasks && due_tasks_badge > 0 {
                let badge = due_tasks_badge.to_string();
                let bg = ui.painter().layout_no_wrap(badge, font_mono(10.5), p.accent_text);
                let bw = (bg.size().x + 10.0).max(18.0);
                let br = Rect::from_center_size(
                    pos2(slot.rect.right() - bw * 0.5 - 6.0, slot.rect.center().y),
                    vec2(bw, 17.0),
                );
                ui.painter().rect_filled(br, Rounding::same(8.5), p.accent_soft);
                ui.painter().galley(
                    pos2(br.center().x - bg.size().x * 0.5, br.center().y - bg.size().y * 0.5),
                    bg,
                    p.accent_text,
                );
            }
            if slot.resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                if slot.mode == ViewMode::Editor && !slot.active {
                    actions.push(UiAction::ReturnToNotesTab);
                } else {
                    actions.push(UiAction::SetViewMode(slot.mode));
                }
            }
        }
    }

    ui.add_space(10.0);
    hairline_h(ui, p);
    ui.add_space(10.0);

    // 3b. Configurable Quick Modules (Media Library / English)
    if modules.show_block {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(s.sb_modules_title)
                    .font(font_medium(11.0))
                    .color(p.text_muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cfg_pop = ui.make_persistent_id("sb_modules_cfg_pop");
                let cfg_btn = icon_button(ui, p, Icon::Settings, s.sb_modules_title, false);
                if cfg_btn.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(cfg_pop));
                }
                egui::popup_below_widget(
                    ui,
                    cfg_pop,
                    &cfg_btn,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_min_width(205.0);
                        let mut next = modules.clone();
                        let mut changed = false;
                        if ui.checkbox(&mut next.show_media_library, s.sb_module_media).changed() {
                            changed = true;
                        }
                        if ui.checkbox(&mut next.show_english, s.sb_module_english).changed() {
                            changed = true;
                        }
                        if next.show_english {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(s.settings_study_lang_label)
                                    .font(font_body(11.0))
                                    .color(p.text_muted),
                            );
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                                for (id, ru, en) in crate::config::STUDY_LANGUAGE_PRESETS {
                                    let lbl = if lang == Lang::Ru { *ru } else { *en };
                                    let active = next.study_language.eq_ignore_ascii_case(id)
                                        || next.study_language.eq_ignore_ascii_case(ru);
                                    if ui.selectable_label(active, lbl).clicked() {
                                        next.study_language = (*id).to_string();
                                        changed = true;
                                    }
                                }
                            });
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut next.study_language)
                                    .hint_text(s.settings_study_lang_hint)
                                    .desired_width(185.0),
                            );
                            if r.changed() {
                                changed = true;
                            }
                        }
                        ui.separator();
                        if ui.button(s.sb_modules_hide).clicked() {
                            next.show_block = false;
                            changed = true;
                            ui.memory_mut(|m| m.close_popup());
                        }
                        if changed {
                            actions.push(UiAction::SetSidebarModules(next));
                        }
                    },
                );
            });
        });
        ui.add_space(4.0);
        if modules.show_media_library {
            if sidebar_row(ui, p, "sb_mod_media", Icon::Image, s.sb_module_media, None, false, 0.0)
                .clicked()
            {
                actions.push(UiAction::OpenSidebarModule(SidebarModule::MediaLibrary));
            }
        }
        if modules.show_english {
            let disp_lang = crate::config::STUDY_LANGUAGE_PRESETS
                .iter()
                .find(|(id, ru, en)| {
                    modules.study_language.eq_ignore_ascii_case(id)
                        || modules.study_language.eq_ignore_ascii_case(ru)
                        || modules.study_language.eq_ignore_ascii_case(en)
                })
                .map(|(_, ru, en)| if lang == Lang::Ru { *ru } else { *en })
                .unwrap_or_else(|| {
                    let t = modules.study_language.trim();
                    if t.is_empty() { "English" } else { t }
                });
            let lang_label = format!("{}: {}", s.sb_module_english, disp_lang);
            if sidebar_row(ui, p, "sb_mod_english", Icon::Globe, &lang_label, None, false, 0.0)
                .clicked()
            {
                actions.push(UiAction::OpenSidebarModule(SidebarModule::English));
            }
        }
        ui.add_space(10.0);
        hairline_h(ui, p);
        ui.add_space(10.0);
    }

    // 4. Diary Calendar + Pages Tree + Tag Cloud
    let scroll_h = (ui.available_height() - 48.0).max(80.0);
    egui::ScrollArea::vertical()
        .id_salt("sidebar_tree_scroll")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .max_height(scroll_h)
        .show(ui, |ui| {
            render_diary_calendar(
                ctx,
                ui,
                p,
                lang,
                cal.open,
                cal.year,
                cal.month,
                cal.today,
                selected_path,
                cal.daily_dates,
                false,
                actions,
            );
            ui.add_space(10.0);
            hairline_h(ui, p);
            ui.add_space(10.0);
            render_pages_and_tags_tree(
                ui,
                p,
                lang,
                notes,
                collections,
                vault_folders,
                tag_counts,
                selected_path,
                selected_collection_path,
                selected_canvas_path,
                active_tag,
                expanded_folders,
                pinned_folders,
                folder_order,
                view_mode,
                true,
                false,
                pages_ui,
                actions,
            );
        });

    // 5. Pinned Footer (Sync + Language + Theme + Settings)
    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let sync_col = if sync.busy {
                p.accent_text
            } else if sync.configured {
                p.text_secondary
            } else {
                p.text_muted
            };
            let max_sync_w = (ui.available_width() - 96.0).max(50.0);
            let (rect, resp) =
                ui.allocate_exact_size(vec2(max_sync_w, 28.0), Sense::click());
            let hov = anim_bool_eased(ctx, resp.id.with("hov"), resp.hovered(), 0.10);
            if hov > 0.01 {
                ui.painter().rect_filled(
                    rect,
                    Rounding::same(6.0),
                    with_alpha(p.bg_hover, hov),
                );
            }

            if sync.busy {
                let ir = Rect::from_center_size(
                    pos2(rect.left() + 12.0, rect.center().y),
                    vec2(13.0, 13.0),
                );
                draw_sync_icon(ui.painter(), ir, sync_col, sync.angle);
            } else {
                let dot_col = if !sync.configured {
                    p.text_muted
                } else if sync.pending_changes > 0 {
                    p.warning
                } else {
                    p.success
                };
                paint_badge_dot(ui.painter(), pos2(rect.left() + 11.0, rect.center().y), dot_col);
            }

            let footer_sync_label = if sync.configured && !sync.busy && sync.pending_changes > 0 {
                format!(
                    "{} {} · {}",
                    sync.pending_changes, s.sync_pending_short, sync.status_text
                )
            } else {
                sync.status_text.to_string()
            };

            let mut job = egui::text::LayoutJob::single_section(
                footer_sync_label,
                egui::TextFormat {
                    font_id: font_medium(12.0),
                    color: lerp_color(sync_col, p.text_primary, hov),
                    ..Default::default()
                },
            );
            job.wrap = egui::text::TextWrapping {
                max_width: (max_sync_w - 24.0).max(20.0),
                max_rows: 1,
                break_anywhere: true,
                overflow_character: Some('…'),
            };
            let g = ui.fonts(|f| f.layout_job(job));
            ui.painter().galley(
                pos2(rect.left() + 21.0, rect.center().y - g.size().y * 0.5),
                g,
                sync_col,
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                if sync.configured {
                    actions.push(UiAction::SyncNow);
                } else {
                    actions.push(UiAction::OpenSettings(SettingsTab::GitHub));
                }
            }

            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if icon_button(
                        ui,
                        p,
                        Icon::Settings,
                        s.tooltip_settings,
                        view_mode == ViewMode::Settings,
                    )
                    .clicked()
                    {
                        if view_mode == ViewMode::Settings {
                            actions.push(UiAction::CloseSettings);
                        } else {
                            actions.push(UiAction::OpenSettings(SettingsTab::Appearance));
                        }
                    }
                    let theme_icon = if p.is_dark { Icon::Sun } else { Icon::Moon };
                    if icon_button(ui, p, theme_icon, s.tooltip_theme, false).clicked() {
                        actions.push(UiAction::ToggleTheme);
                    }
                    // Quiet RU/EN switcher
                    let (lrect, lresp) =
                        ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
                    let lhov = anim_bool_eased(ctx, lresp.id.with("hov"), lresp.hovered(), 0.10);
                    if lhov > 0.01 {
                        ui.painter().rect_filled(
                            lrect,
                            Rounding::same(6.0),
                            with_alpha(p.bg_hover, lhov),
                        );
                    }
                    let lcol = lerp_color(p.text_secondary, p.text_primary, lhov);
                    let g = ui.painter().layout_no_wrap(
                        lang.code().to_string(),
                        font_mono(11.0),
                        lcol,
                    );
                    ui.painter().galley(
                        pos2(
                            lrect.center().x - g.size().x * 0.5,
                            lrect.center().y - g.size().y * 0.5,
                        ),
                        g,
                        lcol,
                    );
                    if lresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        actions.push(UiAction::ToggleLang);
                    }
                },
            );
        });
        ui.add_space(6.0);
        hairline_h(ui, p);
    });
}

pub(crate) fn render_search_trigger(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    show_shortcut: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let search_w = ui.available_width().max(60.0);
    let row_h = if show_shortcut { 30.0 } else { 34.0 };
    let (srect, sresp) = ui.allocate_exact_size(vec2(search_w, row_h), Sense::click());
    let s_hov = anim_bool_eased(ctx, sresp.id.with("hov"), sresp.hovered(), 0.11);
    ui.painter().rect_filled(
        srect,
        Rounding::same(if show_shortcut { 6.0 } else { 8.0 }),
        lerp_color(p.bg_elevated, p.bg_hover, s_hov),
    );
    ui.painter().rect_stroke(
        srect,
        Rounding::same(if show_shortcut { 6.0 } else { 8.0 }),
        Stroke::new(
            1.0_f32,
            lerp_color(p.border_hairline, p.border_subtle, s_hov),
        ),
    );
    let ir = Rect::from_center_size(
        pos2(srect.left() + 13.0, srect.center().y),
        vec2(13.0, 13.0),
    );
    draw_icon(ui.painter(), ir, Icon::Search, p.text_muted);
    let sg = ui.painter().layout_no_wrap(
        s.search_placeholder.to_string(),
        font_body(if show_shortcut { 12.5 } else { 13.5 }),
        p.text_muted,
    );
    ui.painter().galley(
        pos2(srect.left() + 25.0, srect.center().y - sg.size().y * 0.5),
        sg,
        p.text_muted,
    );
    if show_shortcut {
        let sc_g = ui.painter().layout_no_wrap(
            shortcut_label("K"),
            font_mono(10.5),
            p.text_muted,
        );
        ui.painter().galley(
            pos2(
                srect.right() - 10.0 - sc_g.size().x,
                srect.center().y - sc_g.size().y * 0.5,
            ),
            sc_g,
            p.text_muted,
        );
    }
    if sresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        actions.push(UiAction::OpenPalette);
    }
}

fn entry_matches_filter(entry: TreeEntry<'_>, filter: PagesKindFilter) -> bool {
    match filter {
        PagesKindFilter::All => true,
        PagesKindFilter::Notes => matches!(entry, TreeEntry::Note(_)),
        PagesKindFilter::Boards => {
            matches!(entry, TreeEntry::Collection(c) if c.kind == VaultFileKind::Board)
        }
        PagesKindFilter::Tables => {
            matches!(entry, TreeEntry::Collection(c) if c.kind == VaultFileKind::Table)
        }
        PagesKindFilter::Canvases => {
            matches!(entry, TreeEntry::Collection(c) if c.kind == VaultFileKind::Canvas)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_pages_and_tags_tree(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    notes: &[NoteMeta],
    collections: &[CollectionMeta],
    vault_folders: &[String],
    tag_counts: &[(String, usize)],
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    active_tag: Option<&str>,
    expanded_folders: &HashSet<String>,
    pinned_folders: &[String],
    folder_order: &[String],
    view_mode: ViewMode,
    show_tags_cloud: bool,
    compact_touch: bool,
    pages_ui: &mut PagesTreeUiState<'_>,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);

    let mut pinned_entries: Vec<TreeEntry<'_>> = Vec::new();
    for n in notes {
        if n.pinned
            && active_tag.is_none_or(|tf| n.tags.iter().any(|t| t.eq_ignore_ascii_case(tf)))
        {
            pinned_entries.push(TreeEntry::Note(n));
        }
    }
    if active_tag.is_none() {
        for c in collections {
            if c.pinned {
                pinned_entries.push(TreeEntry::Collection(c));
            }
        }
    }
    pinned_entries.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));

    if !pinned_entries.is_empty() {
        ui.horizontal(|ui| {
            section_heading(ui, p, s.pinned_section);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(pinned_entries.len().to_string())
                        .font(font_mono(11.0))
                        .color(p.text_muted),
                );
            });
        });
        ui.add_space(4.0);
        for entry in pinned_entries {
            render_entry_row_with_menu(
                ui,
                p,
                lang,
                entry,
                ("pin_row", entry.path()),
                true,
                entry.is_selected(
                    view_mode,
                    selected_path,
                    selected_collection_path,
                    selected_canvas_path,
                ),
                0.0,
                compact_touch,
                actions,
            );
        }
        ui.add_space(12.0);
    }

    let mut root = FolderNode::default();
    let mut notes_n = 0usize;
    let mut boards_n = 0usize;
    let mut tables_n = 0usize;
    let mut canvases_n = 0usize;
    let mut filtered_count = 0usize;
    for note in notes {
        if let Some(tag_filter) = active_tag {
            if !note.tags.iter().any(|t| t.eq_ignore_ascii_case(tag_filter)) {
                continue;
            }
        }
        notes_n += 1;
        let entry = TreeEntry::Note(note);
        if !entry_matches_filter(entry, *pages_ui.filter) {
            continue;
        }
        let parts: Vec<&str> = note.path.split('/').collect();
        insert_into_tree(&mut root, &parts, entry);
        filtered_count += 1;
    }
    if active_tag.is_none() {
        for col in collections {
            match col.kind {
                VaultFileKind::Board => boards_n += 1,
                VaultFileKind::Table => tables_n += 1,
                VaultFileKind::Canvas => canvases_n += 1,
                _ => {}
            }
            let entry = TreeEntry::Collection(col);
            if !entry_matches_filter(entry, *pages_ui.filter) {
                continue;
            }
            let parts: Vec<&str> = col.path.split('/').collect();
            insert_into_tree(&mut root, &parts, entry);
            filtered_count += 1;
        }
        if *pages_ui.filter == PagesKindFilter::All {
            for folder in vault_folders {
                ensure_folder_path(&mut root, folder);
            }
        }
    }
    let total_all = notes_n + boards_n + tables_n + canvases_n;
    let counts_breakdown = format!(
        "{}: {notes_n} · {}: {boards_n} · {}: {tables_n} · {}: {canvases_n}",
        s.stat_notes_count, s.stat_boards_count, s.stat_tables_count, s.stat_canvases_count,
    );

    ui.horizontal(|ui| {
        section_heading(ui, p, s.pages_section);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(4.0);

            let filter_active = *pages_ui.filter_open || *pages_ui.filter != PagesKindFilter::All;
            let (fr, fresp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            if filter_active || fresp.hovered() {
                ui.painter().rect_filled(
                    fr,
                    Rounding::same(5.0),
                    if *pages_ui.filter != PagesKindFilter::All {
                        p.accent_soft
                    } else {
                        p.bg_hover
                    },
                );
            }
            draw_icon(
                ui.painter(),
                Rect::from_center_size(fr.center(), vec2(12.0, 12.0)),
                Icon::Filter,
                if filter_active { p.accent_text } else { p.text_muted },
            );
            if fresp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(s.pages_filter_tooltip)
                .clicked()
            {
                *pages_ui.filter_open = !*pages_ui.filter_open;
            }

            let sort_lbl = match *pages_ui.sort {
                PagesSortOrder::NameAsc => s.sort_az_short,
                PagesSortOrder::ModifiedDesc => s.sort_recent_short,
                PagesSortOrder::NameDesc => s.sort_za_short,
            };
            let sort_fg = if *pages_ui.sort != PagesSortOrder::NameAsc {
                p.accent_text
            } else {
                p.text_muted
            };
            let sg = ui
                .painter()
                .layout_no_wrap(sort_lbl.to_string(), font_mono(10.0), sort_fg);
            let (sr, sresp) = ui.allocate_exact_size(vec2(sg.size().x + 22.0, 19.0), Sense::click());
            if sresp.hovered() || *pages_ui.sort != PagesSortOrder::NameAsc {
                ui.painter().rect_filled(
                    sr,
                    Rounding::same(5.0),
                    if *pages_ui.sort != PagesSortOrder::NameAsc {
                        p.accent_soft
                    } else {
                        p.bg_hover
                    },
                );
            }
            draw_icon(
                ui.painter(),
                Rect::from_center_size(pos2(sr.left() + 9.0, sr.center().y), vec2(11.0, 11.0)),
                Icon::Sort,
                sort_fg,
            );
            ui.painter().galley(
                pos2(sr.left() + 17.0, sr.center().y - sg.size().y * 0.5),
                sg,
                sort_fg,
            );
            if sresp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(s.pages_sort_tooltip)
                .clicked()
            {
                *pages_ui.sort = match *pages_ui.sort {
                    PagesSortOrder::NameAsc => PagesSortOrder::ModifiedDesc,
                    PagesSortOrder::ModifiedDesc => PagesSortOrder::NameDesc,
                    PagesSortOrder::NameDesc => PagesSortOrder::NameAsc,
                };
            }

            if !expanded_folders.is_empty() {
                let (cr, cresp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                if cresp.hovered() {
                    ui.painter()
                        .rect_filled(cr, Rounding::same(4.0), p.bg_hover);
                }
                let cc = cr.center();
                let col = if cresp.hovered() {
                    p.text_primary
                } else {
                    p.text_muted
                };
                // Two inward chevrons ("collapse all")
                ui.painter().line_segment(
                    [pos2(cc.x - 3.5, cc.y - 3.5), pos2(cc.x, cc.y - 0.8)],
                    Stroke::new(1.3_f32, col),
                );
                ui.painter().line_segment(
                    [pos2(cc.x + 3.5, cc.y - 3.5), pos2(cc.x, cc.y - 0.8)],
                    Stroke::new(1.3_f32, col),
                );
                ui.painter().line_segment(
                    [pos2(cc.x - 3.5, cc.y + 3.5), pos2(cc.x, cc.y + 0.8)],
                    Stroke::new(1.3_f32, col),
                );
                ui.painter().line_segment(
                    [pos2(cc.x + 3.5, cc.y + 3.5), pos2(cc.x, cc.y + 0.8)],
                    Stroke::new(1.3_f32, col),
                );
                let col_tip = if lang == Lang::Ru {
                    "Свернуть все папки"
                } else {
                    "Collapse all folders"
                };
                if cresp
                    .on_hover_cursor(CursorIcon::PointingHand)
                    .on_hover_text(col_tip)
                    .clicked()
                {
                    actions.push(UiAction::CollapseAllFolders);
                }
            }

            ui.label(
                egui::RichText::new(filtered_count.to_string())
                    .font(font_mono(11.0))
                    .color(p.text_muted),
            )
            .on_hover_text(&counts_breakdown);

            let (pr, presp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            if presp.hovered() {
                ui.painter().rect_filled(pr, Rounding::same(4.0), p.bg_hover);
            }
            draw_icon(
                ui.painter(),
                Rect::from_center_size(pr.center(), vec2(12.0, 12.0)),
                Icon::Plus,
                if presp.hovered() { p.accent_text } else { p.text_secondary },
            );
            if presp
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(s.action_new_note)
                .clicked()
            {
                actions.push(UiAction::OpenCreateModal(Some(CreateItemKind::Note)));
            }
        });
    });

    if *pages_ui.filter_open || *pages_ui.filter != PagesKindFilter::All {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
            for (kind, lbl, cnt) in [
                (PagesKindFilter::All, s.filter_all, total_all),
                (PagesKindFilter::Notes, s.filter_notes, notes_n),
                (PagesKindFilter::Boards, s.filter_boards, boards_n),
                (PagesKindFilter::Tables, s.filter_tables, tables_n),
                (PagesKindFilter::Canvases, s.filter_canvases, canvases_n),
            ] {
                let sel = *pages_ui.filter == kind;
                let fg = if sel { p.accent_text } else { p.text_secondary };
                let g = ui.painter().layout_no_wrap(
                    format!("{lbl} {cnt}"),
                    font_medium(10.5),
                    fg,
                );
                let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 11.0, 19.0), Sense::click());
                ui.painter().rect_filled(
                    r,
                    Rounding::same(5.0),
                    if sel {
                        p.accent_soft
                    } else if resp.hovered() {
                        p.bg_hover
                    } else {
                        p.bg_elevated
                    },
                );
                if sel {
                    ui.painter()
                        .rect_stroke(r, Rounding::same(5.0), Stroke::new(1.0_f32, p.accent));
                }
                ui.painter()
                    .galley(pos2(r.center().x - g.size().x * 0.5, r.center().y - g.size().y * 0.5), g, fg);
                if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    *pages_ui.filter = kind;
                }
            }
        });
    }
    ui.add_space(4.0);

    render_folder_node(
        ui,
        p,
        lang,
        "",
        &root,
        0,
        selected_path,
        selected_collection_path,
        selected_canvas_path,
        view_mode,
        expanded_folders,
        pinned_folders,
        folder_order,
        *pages_ui.sort,
        compact_touch,
        actions,
    );

    if show_tags_cloud && !tag_counts.is_empty() {
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            section_heading(ui, p, s.tags_section);
            if active_tag.is_some() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(6.0);
                    let resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(s.all_tags_clear)
                                .font(font_medium(11.5))
                                .color(p.accent_text),
                        )
                        .sense(Sense::click()),
                    );
                    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        actions.push(UiAction::SetTagFilter(None));
                    }
                });
            }
        });
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
            for (tag, _count) in tag_counts {
                let selected = active_tag.is_some_and(|at| at.eq_ignore_ascii_case(tag));
                let (resp, _) = tag_chip(ui, p, tag, false);
                if selected {
                    ui.painter().rect_stroke(
                        resp.rect,
                        Rounding::same(5.0),
                        Stroke::new(1.5_f32, p.accent),
                    );
                }
                if resp.clicked() {
                    if selected {
                        actions.push(UiAction::SetTagFilter(None));
                    } else {
                        actions.push(UiAction::SetTagFilter(Some(tag.clone())));
                    }
                }
                let tag_ctx = tag.clone();
                resp.context_menu(|ui| {
                    if context_menu_row(ui, p, Icon::Sparkles, s.tag_rename_ctx, false).clicked() {
                        actions.push(UiAction::OpenTagRename(Some(tag_ctx)));
                        ui.close_menu();
                    }
                });
            }
        });
    }
}

#[derive(Clone, Debug)]
struct FolderDragState {
    parent_prefix: String,
    folder_path: String,
    label: String,
    is_daily_root: bool,
    is_pinned: bool,
    start_pos: egui::Pos2,
    grab_offset_y: f32,
    row_rect: Rect,
    target_in_group: usize,
    active: bool,
}

#[allow(clippy::too_many_arguments)]
fn render_folder_node(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    prefix: &str,
    node: &FolderNode<'_>,
    depth: usize,
    selected_path: Option<&str>,
    selected_collection_path: Option<&str>,
    selected_canvas_path: Option<&str>,
    view_mode: ViewMode,
    expanded_folders: &HashSet<String>,
    pinned_folders: &[String],
    folder_order: &[String],
    pages_sort: PagesSortOrder,
    compact_touch: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let indent = (depth as f32) * 14.0;
    let folder_h = if compact_touch { 34.0 } else { 27.0 };
    let is_diary_root_lvl = prefix.eq_ignore_ascii_case("daily");
    let is_diary_boards_lvl = prefix.eq_ignore_ascii_case("daily/boards")
        || prefix.starts_with("daily/boards/");
    let in_daily_notes = is_diary_root_lvl
        || (prefix.starts_with("daily/") && !is_diary_boards_lvl);

    let mut subfolders: Vec<(&String, &FolderNode<'_>)> = node.subfolders.iter().collect();
    if is_diary_root_lvl {
        subfolders.sort_by(|(a, _), (b, _)| {
            let a_boards = a.eq_ignore_ascii_case("boards");
            let b_boards = b.eq_ignore_ascii_case("boards");
            a_boards.cmp(&b_boards).then_with(|| b.cmp(a))
        });
    } else if in_daily_notes || is_diary_boards_lvl {
        subfolders.reverse();
    } else {
        subfolders.sort_by(|(a, _), (b, _)| {
            let pa = if prefix.is_empty() {
                (*a).clone()
            } else {
                format!("{prefix}/{a}")
            };
            let pb = if prefix.is_empty() {
                (*b).clone()
            } else {
                format!("{prefix}/{b}")
            };
            let a_pin = pinned_folders.contains(&pa);
            let b_pin = pinned_folders.contains(&pb);
            let a_ord = folder_order.iter().position(|x| x == &pa).unwrap_or(usize::MAX);
            let b_ord = folder_order.iter().position(|x| x == &pb).unwrap_or(usize::MAX);
            b_pin
                .cmp(&a_pin)
                .then_with(|| a_ord.cmp(&b_ord))
                .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
        });
    }

    let folder_drag_id = egui::Id::new("krono_folder_drag");
    let mut drag_state = ui
        .ctx()
        .data(|d| d.get_temp::<FolderDragState>(folder_drag_id));
    let mut sibling_rects: Vec<(usize, String, bool, Rect)> = Vec::new();
    let active_drag = drag_state
        .as_ref()
        .filter(|st| st.active && st.parent_prefix == prefix)
        .cloned();
    let src_group_pos = active_drag.as_ref().map(|st| {
        subfolders
            .iter()
            .take_while(|(fname, _)| {
                let fp = if prefix.is_empty() {
                    (*fname).clone()
                } else {
                    format!("{prefix}/{fname}")
                };
                fp != st.folder_path
            })
            .filter(|(fname, _)| {
                let fp = if prefix.is_empty() {
                    (*fname).clone()
                } else {
                    format!("{prefix}/{fname}")
                };
                let is_inner = is_diary_root_lvl && fname.eq_ignore_ascii_case("boards")
                    || in_daily_notes
                    || is_diary_boards_lvl;
                !is_inner && pinned_folders.contains(&fp) == st.is_pinned
            })
            .count()
    });
    let mut group_idx_counter = [0usize, 0usize];

    for (sib_idx, (folder_name, child)) in subfolders.into_iter().enumerate() {
        let folder_path = if prefix.is_empty() {
            folder_name.clone()
        } else {
            format!("{prefix}/{folder_name}")
        };
        let is_open = expanded_folders.contains(&folder_path);
        let is_daily_root = prefix.is_empty() && folder_name.eq_ignore_ascii_case("daily");
        let is_daily_boards = is_diary_root_lvl && folder_name.eq_ignore_ascii_case("boards");
        let is_daily_inner = is_daily_boards || in_daily_notes || is_diary_boards_lvl;
        let is_pinned = !is_daily_inner && pinned_folders.contains(&folder_path);
        let my_g_idx = if !is_daily_inner {
            let slot = usize::from(is_pinned);
            let cur = group_idx_counter[slot];
            group_idx_counter[slot] += 1;
            cur
        } else {
            0
        };
        let display_folder_name = if is_daily_root {
            s.diary_section.to_string()
        } else if is_daily_boards {
            s.diary_boards_folder.to_string()
        } else if prefix.starts_with("daily/") && folder_name.len() == 2 {
            if let Ok(m) = folder_name.parse::<u32>() {
                if (1..=12).contains(&m) {
                    format!("{folder_name} · {}", month_name(lang, m))
                } else {
                    folder_name.clone()
                }
            } else {
                folder_name.clone()
            }
        } else {
            folder_name.clone()
        };

        let w = ui.available_width().max(60.0);
        let sense = if is_daily_inner || compact_touch {
            Sense::click()
        } else {
            Sense::click_and_drag()
        };
        let (base_rect, resp) = ui.allocate_exact_size(vec2(w, folder_h), sense);
        if !is_daily_inner {
            sibling_rects.push((sib_idx, folder_path.clone(), is_pinned, base_rect));
            if resp.drag_started() {
                if let Some(pos) = ui.ctx().pointer_interact_pos() {
                    drag_state = Some(FolderDragState {
                        parent_prefix: prefix.to_string(),
                        folder_path: folder_path.clone(),
                        label: display_folder_name.clone(),
                        is_daily_root,
                        is_pinned,
                        start_pos: pos,
                        grab_offset_y: (pos.y - base_rect.top()).clamp(4.0, folder_h - 4.0),
                        row_rect: base_rect,
                        target_in_group: my_g_idx,
                        active: false,
                    });
                }
            }
        }
        let is_dragging_this = active_drag
            .as_ref()
            .is_some_and(|st| st.folder_path == folder_path);
        let target_shift_y = if let (Some(st), Some(src_g)) = (&active_drag, src_group_pos) {
            if !is_daily_inner && is_pinned == st.is_pinned && !is_dragging_this {
                let shift_amt = folder_h + ui.spacing().item_spacing.y;
                if st.target_in_group > src_g && my_g_idx > src_g && my_g_idx <= st.target_in_group {
                    -shift_amt
                } else if st.target_in_group < src_g && my_g_idx >= st.target_in_group && my_g_idx < src_g {
                    shift_amt
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };
        let shift_y = crate::ui::motion::anim_value_smooth(
            ui.ctx(),
            ui.make_persistent_id(("fld_shift_y", &folder_path)),
            target_shift_y,
            0.11,
        );
        let rect = base_rect.translate(vec2(0.0, shift_y));

        let open_t = anim_bool_eased(
            ui.ctx(),
            ui.make_persistent_id(("fld_open", &folder_path)),
            is_open,
            0.12,
        );
        if !is_dragging_this {
            let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
            if hov > 0.01 {
                ui.painter().rect_filled(
                    rect,
                    Rounding::same(6.0),
                    with_alpha(p.bg_hover, hov),
                );
            }
            let chev_center = pos2(rect.left() + 8.0 + indent, rect.center().y);
            let angle = open_t * std::f32::consts::FRAC_PI_2;
            let (sin_a, cos_a) = angle.sin_cos();
            let rot_pt = |lx: f32, ly: f32| -> egui::Pos2 {
                pos2(
                    chev_center.x + lx * cos_a - ly * sin_a,
                    chev_center.y + lx * sin_a + ly * cos_a,
                )
            };
            let chev_col = lerp_color(p.text_muted, p.text_secondary, hov);
            ui.painter().add(egui::Shape::line(
                vec![rot_pt(-1.8, -3.2), rot_pt(1.8, 0.0), rot_pt(-1.8, 3.2)],
                Stroke::new(1.4_f32, chev_col),
            ));
            let f_rect = Rect::from_center_size(
                pos2(rect.left() + 23.0 + indent, rect.center().y),
                vec2(14.0, 14.0),
            );
            let f_icon = if is_daily_root {
                Icon::CalendarDays
            } else if is_daily_boards {
                Icon::Board
            } else if open_t >= 0.45 {
                Icon::FolderOpen
            } else {
                Icon::Folder
            };
            draw_icon(
                ui.painter(),
                f_rect,
                f_icon,
                if is_daily_root || is_daily_boards {
                    p.accent_text
                } else {
                    p.text_secondary
                },
            );
            let txt_col = lerp_color(p.text_secondary, p.text_primary, hov);
            let g = ui
                .painter()
                .layout_no_wrap(display_folder_name, font_medium(12.5), txt_col);
            ui.painter().galley(
                pos2(rect.left() + 35.0 + indent, rect.center().y - g.size().y * 0.5),
                g,
                txt_col,
            );
            if is_pinned {
                draw_icon(
                    ui.painter(),
                    Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(11.0, 11.0)),
                    Icon::Pin,
                    p.accent_text,
                );
            }
        }
        let resp = resp.on_hover_cursor(if is_dragging_this {
            CursorIcon::Grabbing
        } else {
            CursorIcon::PointingHand
        });
        if resp.clicked() {
            actions.push(UiAction::ToggleFolder(folder_path.clone()));
        }
        if !is_daily_inner {
            let folder_ctx = folder_path.clone();
            resp.context_menu(|ui| {
                ui.set_min_width(172.0);
                let pin_lbl = if is_pinned {
                    s.folder_ctx_unpin
                } else {
                    s.folder_ctx_pin
                };
                let pin_ic = if is_pinned { Icon::PinOff } else { Icon::Pin };
                if context_menu_row(ui, p, pin_ic, pin_lbl, false).clicked() {
                    actions.push(UiAction::TogglePinFolder(folder_ctx.clone()));
                    ui.close_menu();
                }
                if !is_daily_root {
                    ui.add_space(3.0);
                    hairline_h(ui, p);
                    ui.add_space(3.0);
                    if context_menu_row(ui, p, Icon::Document, s.folder_ctx_new_note, false).clicked() {
                        actions.push(UiAction::CreateInFolder {
                            folder: folder_ctx.clone(),
                            kind: CreateItemKind::Note,
                        });
                        ui.close_menu();
                    }
                    if context_menu_row(ui, p, Icon::Board, s.folder_ctx_new_board, false).clicked() {
                        actions.push(UiAction::CreateInFolder {
                            folder: folder_ctx.clone(),
                            kind: CreateItemKind::Board,
                        });
                        ui.close_menu();
                    }
                    if context_menu_row(ui, p, Icon::Table, s.folder_ctx_new_table, false).clicked() {
                        actions.push(UiAction::CreateInFolder {
                            folder: folder_ctx.clone(),
                            kind: CreateItemKind::Table,
                        });
                        ui.close_menu();
                    }
                    if context_menu_row(ui, p, Icon::Canvas, s.folder_ctx_new_canvas, false).clicked() {
                        actions.push(UiAction::CreateInFolder {
                            folder: folder_ctx.clone(),
                            kind: CreateItemKind::Canvas,
                        });
                        ui.close_menu();
                    }
                    ui.add_space(3.0);
                    hairline_h(ui, p);
                    ui.add_space(3.0);
                    if context_menu_row(ui, p, Icon::Folder, s.folder_ctx_new_subfolder, false)
                        .clicked()
                    {
                        actions.push(UiAction::StartCreateSubfolder(folder_ctx));
                        ui.close_menu();
                    }
                }
            });
        }

        if open_t > 0.005 {
            let height_id = ui.make_persistent_id(("fld_h", &folder_path, compact_touch));
            let guide_x = (rect.left() + 8.0 + indent).round();

            if open_t >= 0.995 {
                let inner = ui.vertical(|ui| {
                    render_folder_node(
                        ui,
                        p,
                        lang,
                        &folder_path,
                        child,
                        depth + 1,
                        selected_path,
                        selected_collection_path,
                        selected_canvas_path,
                        view_mode,
                        expanded_folders,
                        pinned_folders,
                        folder_order,
                        pages_sort,
                        compact_touch,
                        actions,
                    );
                });
                let r = inner.response.rect;
                ui.data_mut(|d| d.insert_temp(height_id, r.height()));
                if r.height() >= 8.0 {
                    ui.painter().line_segment(
                        [pos2(guide_x, r.top() + 2.0), pos2(guide_x, r.bottom() - 4.0)],
                        Stroke::new(1.0_f32, p.border_hairline),
                    );
                }
            } else {
                let est_h = estimate_folder_height(
                    child,
                    &folder_path,
                    expanded_folders,
                    ui.spacing().item_spacing.y,
                    compact_touch,
                );
                let full_h = ui
                    .data(|d| d.get_temp::<f32>(height_id))
                    .unwrap_or(est_h);
                let visible_h = (full_h * open_t).round().max(0.0);
                if visible_h > 0.5 {
                    let (clip_rect, _) =
                        ui.allocate_exact_size(vec2(w, visible_h), Sense::hover());
                    let content_rect = Rect::from_min_size(
                        clip_rect.min,
                        vec2(w, full_h.max(visible_h) + 8.0),
                    );
                    let mut child_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(content_rect)
                            .layout(egui::Layout::top_down(egui::Align::LEFT)),
                    );
                    child_ui.set_clip_rect(ui.clip_rect().intersect(clip_rect));
                    child_ui.multiply_opacity(open_t);
                    render_folder_node(
                        &mut child_ui,
                        p,
                        lang,
                        &folder_path,
                        child,
                        depth + 1,
                        selected_path,
                        selected_collection_path,
                        selected_canvas_path,
                        view_mode,
                        expanded_folders,
                        pinned_folders,
                        folder_order,
                        pages_sort,
                        compact_touch,
                        actions,
                    );
                    let measured_h = child_ui.min_rect().height();
                    if measured_h > 1.0 {
                        ui.data_mut(|d| d.insert_temp(height_id, measured_h));
                    }
                    if clip_rect.height() >= 8.0 {
                        ui.painter().line_segment(
                            [
                                pos2(guide_x, clip_rect.top() + 2.0),
                                pos2(guide_x, clip_rect.bottom() - 4.0),
                            ],
                            Stroke::new(1.0_f32, with_alpha(p.border_hairline, open_t)),
                        );
                    }
                }
            }
        }
    }

    if let Some(mut st) = drag_state.take() {
        if st.parent_prefix == prefix && !sibling_rects.is_empty() {
            let primary_down = ui.input(|i| i.pointer.primary_down());
            let ptr_pos = ui
                .ctx()
                .pointer_interact_pos()
                .or_else(|| ui.ctx().pointer_hover_pos());
            if primary_down {
                if let Some(pos) = ptr_pos {
                    if !st.active && (pos - st.start_pos).length() >= 5.0 {
                        st.active = true;
                    }
                    if st.active {
                        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
                        ui.ctx().request_repaint();
                        let group: Vec<(usize, Rect)> = sibling_rects
                            .iter()
                            .filter(|&&(_, _, pin, _)| pin == st.is_pinned)
                            .map(|&(i, _, _, r)| (i, r))
                            .collect();
                        if !group.is_empty() {
                            let mut new_target = 0usize;
                            for (g_idx, &(_, r)) in group.iter().enumerate() {
                                let thresh = if g_idx < st.target_in_group {
                                    r.center().y - 8.0
                                } else {
                                    r.center().y + 8.0
                                };
                                if pos.y >= thresh {
                                    new_target = g_idx + 1;
                                }
                            }
                            st.target_in_group = new_target.min(group.len().saturating_sub(1));

                            // Target slot placeholder in the vacated gap
                            if let Some((_, target_r)) = group.get(st.target_in_group) {
                                let ph_y = crate::ui::motion::anim_value_smooth(
                                    ui.ctx(),
                                    egui::Id::new("fld_ph_y"),
                                    target_r.top(),
                                    0.11,
                                );
                                let ph_r = Rect::from_min_size(
                                    pos2(st.row_rect.left(), ph_y),
                                    st.row_rect.size(),
                                );
                                ui.painter().rect_filled(
                                    ph_r,
                                    Rounding::same(6.0),
                                    with_alpha(p.accent, 0.10),
                                );
                                ui.painter().rect_stroke(
                                    ph_r,
                                    Rounding::same(6.0),
                                    Stroke::new(1.0_f32, with_alpha(p.accent, 0.45)),
                                );
                            }

                            // Floating lifted ghost folder row under cursor
                            let min_y = group.first().map(|(_, r)| r.top()).unwrap_or(st.row_rect.top());
                            let max_y = group.last().map(|(_, r)| r.top()).unwrap_or(st.row_rect.top());
                            let ghost_y = (pos.y - st.grab_offset_y).clamp(min_y - 4.0, max_y + 4.0);
                            let ghost_r = Rect::from_min_size(
                                pos2(st.row_rect.left() + 2.0, ghost_y),
                                st.row_rect.size(),
                            );
                            let gp = ui.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Tooltip,
                                egui::Id::new("krono_folder_drag_ghost"),
                            ));
                            gp.rect_filled(
                                ghost_r.translate(vec2(0.0, 3.0)),
                                Rounding::same(7.0),
                                Color32::from_rgba_unmultiplied(0, 0, 0, 55),
                            );
                            gp.rect_filled(ghost_r, Rounding::same(6.0), p.bg_elevated);
                            gp.rect_stroke(
                                ghost_r,
                                Rounding::same(6.0),
                                Stroke::new(1.2_f32, p.accent),
                            );
                            draw_icon(
                                &gp,
                                Rect::from_center_size(
                                    pos2(ghost_r.left() + 23.0 + indent, ghost_r.center().y),
                                    vec2(14.0, 14.0),
                                ),
                                if st.is_daily_root {
                                    Icon::CalendarDays
                                } else {
                                    Icon::Folder
                                },
                                p.accent_text,
                            );
                            let gg = ui.ctx().fonts(|f| {
                                f.layout_no_wrap(st.label.clone(), font_medium(12.5), p.text_primary)
                            });
                            gp.galley(
                                pos2(
                                    ghost_r.left() + 35.0 + indent,
                                    ghost_r.center().y - gg.size().y * 0.5,
                                ),
                                gg,
                                p.text_primary,
                            );
                        }
                    }
                }
                ui.ctx().data_mut(|d| d.insert_temp(folder_drag_id, st));
            } else {
                if st.active {
                    let group: Vec<(usize, Rect)> = sibling_rects
                        .iter()
                        .filter(|&&(_, _, pin, _)| pin == st.is_pinned)
                        .map(|&(i, _, _, r)| (i, r))
                        .collect();
                    if let Some(&(global_to_index, _)) = group.get(st.target_in_group) {
                        actions.push(UiAction::ReorderFolder {
                            parent_prefix: st.parent_prefix,
                            folder_path: st.folder_path,
                            to_index: global_to_index,
                        });
                    }
                }
                ui.ctx()
                    .data_mut(|d| d.remove::<FolderDragState>(folder_drag_id));
            }
        }
    }

    let mut sorted_entries = node.entries.clone();
    if in_daily_notes {
        sorted_entries.sort_by(|a, b| {
            let ah = is_daily_hub_path(a.path());
            let bh = is_daily_hub_path(b.path());
            bh.cmp(&ah).then_with(|| b.path().cmp(a.path()))
        });
    } else {
        match pages_sort {
            PagesSortOrder::NameAsc => {
                sorted_entries.sort_by(|a, b| a.title().to_lowercase().cmp(&b.title().to_lowercase()));
            }
            PagesSortOrder::ModifiedDesc => {
                sorted_entries.sort_by(|a, b| {
                    b.modified()
                        .cmp(&a.modified())
                        .then_with(|| a.title().to_lowercase().cmp(&b.title().to_lowercase()))
                });
            }
            PagesSortOrder::NameDesc => {
                sorted_entries.sort_by(|a, b| b.title().to_lowercase().cmp(&a.title().to_lowercase()));
            }
        }
    }

    for entry in sorted_entries {
        let is_selected = entry.is_selected(
            view_mode,
            selected_path,
            selected_collection_path,
            selected_canvas_path,
        );
        render_entry_row_with_menu(
            ui,
            p,
            lang,
            entry,
            ("tree_row", entry.path()),
            false,
            is_selected,
            indent,
            compact_touch,
            actions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_entry_row_with_menu(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    entry: TreeEntry<'_>,
    id_salt: (&str, &str),
    in_pinned: bool,
    is_selected: bool,
    indent: f32,
    compact_touch: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let resp = if compact_touch {
        touch_entry_row(ui, p, lang, entry, is_selected, indent)
    } else {
        sidebar_row(
            ui,
            p,
            id_salt,
            entry.icon(in_pinned),
            entry.title(),
            None,
            is_selected,
            indent,
        )
    };
    if resp.clicked() {
        entry.push_open(actions);
    }
    resp.context_menu(|ui| {
        ui.set_min_width(164.0);
        ui.spacing_mut().item_spacing.y = 2.0;
        if context_menu_row(ui, p, entry.icon(false), s.open_btn, false).clicked() {
            entry.push_open(actions);
            ui.close_menu();
        }
        let split_target = match entry {
            TreeEntry::Note(n) => Some(crate::ui::NavTarget::Note(n.path.clone())),
            TreeEntry::Collection(c) if c.kind != VaultFileKind::Canvas => {
                Some(crate::ui::NavTarget::Collection(c.path.clone()))
            }
            _ => None,
        };
        if let Some(st) = split_target {
            if context_menu_row(ui, p, Icon::Split, s.sidebar_ctx_open_right, false).clicked() {
                actions.push(UiAction::OpenInSplit(st));
                ui.close_menu();
            }
        }
        let pinned = entry.pinned();
        if context_menu_row(
            ui,
            p,
            if pinned { Icon::PinOff } else { Icon::Pin },
            if pinned {
                s.unpin_note_btn
            } else {
                s.pin_note_btn
            },
            false,
        )
        .clicked()
        {
            actions.push(UiAction::TogglePin(entry.path().to_string()));
            ui.close_menu();
        }
        if context_menu_row(ui, p, Icon::Edit, s.rename_btn, false).clicked() {
            actions.push(UiAction::StartRename(entry.path().to_string()));
            ui.close_menu();
        }
        if context_menu_row(ui, p, Icon::Copy, s.duplicate_btn, false).clicked() {
            actions.push(UiAction::DuplicateNote(entry.path().to_string()));
            ui.close_menu();
        }
        ui.add_space(2.0);
        hairline_h(ui, p);
        ui.add_space(2.0);
        if context_menu_row(ui, p, Icon::Trash, s.delete_btn, true).clicked() {
            actions.push(UiAction::RequestDelete(entry.path().to_string()));
            ui.close_menu();
        }
    });
}

fn touch_entry_row(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    entry: TreeEntry<'_>,
    selected: bool,
    indent_px: f32,
) -> egui::Response {
    let w = ui.available_width().max(80.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 38.0), Sense::click());
    let id = ui.make_persistent_id(("touch_entry_row", entry.path()));
    let hov = anim_bool_eased(ui.ctx(), id.with("hov"), resp.hovered(), 0.10);

    let bg = if selected {
        p.bg_selected
    } else if hov > 0.01 {
        with_alpha(p.bg_hover, hov)
    } else {
        Color32::TRANSPARENT
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, Rounding::same(7.0), bg);
    }

    let icon_x = rect.left() + 8.0 + indent_px;
    let ir = Rect::from_center_size(pos2(icon_x + 7.5, rect.center().y), vec2(15.0, 15.0));
    draw_icon(
        ui.painter(),
        ir,
        entry.icon(false),
        if selected { p.accent_text } else { p.text_muted },
    );

    let time_str = format_relative_time(lang, entry.modified());
    let time_g = ui
        .painter()
        .layout_no_wrap(time_str, font_mono(11.0), p.text_muted);
    let right_reserve = time_g.size().x + 28.0;

    let text_left = icon_x + 22.0;
    let max_text_w = (rect.right() - right_reserve - text_left).max(40.0);

    let mut job = egui::text::LayoutJob::single_section(
        entry.title().to_string(),
        egui::TextFormat {
            font_id: font_medium(14.0),
            color: p.text_primary,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_text_w,
        max_rows: 1,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    let title_g = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(
        pos2(text_left, rect.center().y - title_g.size().y * 0.5),
        title_g,
        p.text_primary,
    );

    match entry {
        TreeEntry::Note(note) => {
            if let Some(st) = note.status {
                let (_, dot_col, _) = p.status_colors(st);
                ui.painter()
                    .circle_filled(pos2(rect.right() - 12.0, rect.center().y), 3.2, dot_col);
            }
        }
        TreeEntry::Collection(_) => {
            ui.painter()
                .circle_filled(pos2(rect.right() - 12.0, rect.center().y), 3.2, p.accent);
        }
    }
    ui.painter().galley(
        pos2(
            rect.right() - 22.0 - time_g.size().x,
            rect.center().y - time_g.size().y * 0.5,
        ),
        time_g,
        p.text_muted,
    );

    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn estimate_folder_height(
    node: &FolderNode<'_>,
    prefix: &str,
    expanded_folders: &HashSet<String>,
    spacing_y: f32,
    compact_touch: bool,
) -> f32 {
    let fh = if compact_touch { 34.0 } else { 27.0 };
    let nh = if compact_touch { 38.0 } else { 28.0 };
    let mut total = 0.0_f32;
    let mut count = 0usize;
    for (folder_name, child) in &node.subfolders {
        if count > 0 {
            total += spacing_y;
        }
        total += fh;
        count += 1;
        let folder_path = if prefix.is_empty() {
            folder_name.clone()
        } else {
            format!("{prefix}/{folder_name}")
        };
        if expanded_folders.contains(&folder_path) {
            let sub_h =
                estimate_folder_height(child, &folder_path, expanded_folders, spacing_y, compact_touch);
            if sub_h > 0.0 {
                total += spacing_y + sub_h;
            }
        }
    }
    for _ in &node.entries {
        if count > 0 {
            total += spacing_y;
        }
        total += nh;
        count += 1;
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use krono_core::note::NoteStatus;

    fn dummy_note(path: &str) -> NoteMeta {
        NoteMeta {
            path: path.into(),
            title: path.into(),
            status: Some(NoteStatus::Todo),
            tags: Vec::new(),
            excerpt: String::new(),
            modified: 0,
            word_count: 0,
            pinned: false,
            board_order: 0,
        }
    }

    #[test]
    fn estimates_folder_heights_accurately() {
        let empty = FolderNode::default();
        let expanded = HashSet::new();
        assert_eq!(estimate_folder_height(&empty, "", &expanded, 6.0, false), 0.0);

        let a = dummy_note("projects/a.md");
        let b = dummy_note("projects/sub/b.md");
        let c = dummy_note("projects/sub/c.md");

        let mut root = FolderNode::default();
        for note in [&a, &b, &c] {
            let parts: Vec<&str> = note.path.split('/').collect();
            insert_into_tree(&mut root, &parts, TreeEntry::Note(note));
        }

        let projects = root.subfolders.get("projects").unwrap();
        assert_eq!(
            estimate_folder_height(projects, "projects", &expanded, 6.0, false),
            27.0 + 6.0 + 28.0
        );

        let mut expanded_sub = HashSet::new();
        expanded_sub.insert("projects/sub".to_string());
        assert_eq!(
            estimate_folder_height(projects, "projects", &expanded_sub, 6.0, false),
            27.0 + 6.0 + (28.0 + 6.0 + 28.0) + 6.0 + 28.0
        );
        assert_eq!(
            estimate_folder_height(projects, "projects", &expanded_sub, 6.0, true),
            34.0 + 6.0 + (38.0 + 6.0 + 38.0) + 6.0 + 38.0
        );
    }
}
