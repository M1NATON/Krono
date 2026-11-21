use crate::i18n::{strings, Lang};
use crate::storage::index::SearchHit;
use crate::ui::components::{focus_once, hairline_h, keycap_badge, reset_focus_gate, section_heading};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, ThemePalette};
use crate::ui::{CreateItemKind, SettingsTab, UiAction, ViewMode};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CursorIcon, Margin, Rect, Rounding, Sense, Stroke,
};
use krono_core::path::VaultFileKind;

#[derive(Clone)]
struct PaletteCommand {
    icon: Icon,
    label: &'static str,
    action: UiAction,
}

enum PaletteEntry {
    Note(SearchHit),
    Command(PaletteCommand),
}

#[allow(clippy::too_many_arguments)]
pub fn render_command_palette(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    open: &mut bool,
    query: &mut String,
    selected_idx: &mut usize,
    hits: &[SearchHit],
) -> (bool /* query_changed */, Vec<UiAction>) {
    let anim_t = anim_bool_eased(ctx, egui::Id::new("palette_open_anim"), *open, 0.14);
    if !*open && anim_t < 0.01 {
        reset_focus_gate(ctx, egui::Id::new("palette_foc_gate"));
        return (false, Vec::new());
    }

    let s = strings(lang);
    let mut actions = Vec::new();
    let mut query_changed = false;

    if *open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
        return (false, actions);
    }

    let screen = ctx.screen_rect();
    let backdrop_alpha = if p.is_dark { 145.0 } else { 85.0 };
    egui::Area::new(egui::Id::new("palette_backdrop"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let resp = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, (backdrop_alpha * anim_t) as u8),
            );
            if resp.clicked() {
                *open = false;
            }
        });

    if !*open {
        return (false, actions);
    }

    let all_commands = [
        PaletteCommand {
            icon: Icon::Plus,
            label: s.action_new_note,
            action: UiAction::OpenCreateModal(Some(CreateItemKind::Note)),
        },
        PaletteCommand {
            icon: Icon::Board,
            label: s.action_new_custom_board,
            action: UiAction::OpenCreateModal(Some(CreateItemKind::Board)),
        },
        PaletteCommand {
            icon: Icon::Table,
            label: s.action_new_custom_table,
            action: UiAction::OpenCreateModal(Some(CreateItemKind::Table)),
        },
        PaletteCommand {
            icon: Icon::Canvas,
            label: s.action_new_canvas,
            action: UiAction::OpenCreateModal(Some(CreateItemKind::Canvas)),
        },
        PaletteCommand {
            icon: Icon::Sparkles,
            label: s.action_scratchpad,
            action: UiAction::OpenScratchpad,
        },
        PaletteCommand {
            icon: Icon::LayoutGrid,
            label: s.action_toggle_sb_modules,
            action: UiAction::ToggleSidebarModules,
        },
        PaletteCommand {
            icon: Icon::CalendarDays,
            label: s.action_daily_note,
            action: UiAction::OpenTodayNote,
        },
        PaletteCommand {
            icon: Icon::Task,
            label: s.tasks_title,
            action: UiAction::SetViewMode(ViewMode::Tasks),
        },
        PaletteCommand {
            icon: Icon::CalendarDays,
            label: s.tasks_carry_over_btn,
            action: UiAction::CarryOverDailyTasks,
        },
        PaletteCommand {
            icon: Icon::Notes,
            label: s.weekly_review_action,
            action: UiAction::GenerateWeeklyReview,
        },
        PaletteCommand {
            icon: Icon::CircleHelp,
            label: s.action_md_help,
            action: UiAction::OpenSettings(SettingsTab::MarkdownHelp),
        },
        PaletteCommand {
            icon: Icon::Sync,
            label: s.action_sync_github,
            action: UiAction::SyncNow,
        },
        PaletteCommand {
            icon: if p.is_dark { Icon::Sun } else { Icon::Moon },
            label: s.action_toggle_theme,
            action: UiAction::ToggleTheme,
        },
        PaletteCommand {
            icon: Icon::Globe,
            label: s.action_toggle_lang,
            action: UiAction::ToggleLang,
        },
        PaletteCommand {
            icon: Icon::SidebarToggle,
            label: s.action_toggle_sidebar,
            action: UiAction::ToggleSidebar,
        },
        PaletteCommand {
            icon: Icon::InspectorToggle,
            label: s.action_toggle_inspector,
            action: UiAction::ToggleInspector,
        },
        PaletteCommand {
            icon: Icon::Notes,
            label: s.action_view_editor,
            action: UiAction::SetViewMode(ViewMode::Editor),
        },
        PaletteCommand {
            icon: Icon::Table,
            label: s.action_view_table,
            action: UiAction::SetViewMode(ViewMode::Table),
        },
        PaletteCommand {
            icon: Icon::Board,
            label: s.action_view_board,
            action: UiAction::SetViewMode(ViewMode::Board),
        },
        PaletteCommand {
            icon: Icon::Graph,
            label: s.action_view_graph,
            action: UiAction::SetViewMode(ViewMode::Graph),
        },
        PaletteCommand {
            icon: Icon::Settings,
            label: s.action_open_settings,
            action: UiAction::OpenSettings(SettingsTab::Appearance),
        },
        PaletteCommand {
            icon: Icon::Search,
            label: s.action_global_replace,
            action: UiAction::OpenGlobalReplace,
        },
        PaletteCommand {
            icon: Icon::Sparkles,
            label: s.action_rename_tag,
            action: UiAction::OpenTagRename(None),
        },
        PaletteCommand {
            icon: Icon::Maximize,
            label: s.action_toggle_zen,
            action: UiAction::ToggleZenMode,
        },
        PaletteCommand {
            icon: Icon::Split,
            label: s.action_toggle_split,
            action: UiAction::ToggleSplitRight,
        },
    ];

    let parsed_query = krono_core::search::parse_search_query(query);

    let q_lower = query.trim().to_lowercase();
    let matching_cmds: Vec<PaletteCommand> = all_commands
        .into_iter()
        .filter(|c| q_lower.is_empty() || c.label.to_lowercase().contains(&q_lower))
        .collect();

    let mut entries: Vec<PaletteEntry> = Vec::new();
    for hit in hits.iter().take(8) {
        entries.push(PaletteEntry::Note(hit.clone()));
    }
    for cmd in matching_cmds.into_iter().take(6) {
        entries.push(PaletteEntry::Command(cmd));
    }

    if entries.is_empty() {
        *selected_idx = 0;
    } else if *selected_idx >= entries.len() {
        *selected_idx = entries.len() - 1;
    }

    let mut nav_moved = false;
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) && !entries.is_empty() {
        *selected_idx = (*selected_idx + 1) % entries.len();
        nav_moved = true;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) && !entries.is_empty() {
        *selected_idx = (*selected_idx + entries.len() - 1) % entries.len();
        nav_moved = true;
    }
    let sel_mem_id = egui::Id::new("palette_prev_selected_idx");
    let prev_sel = ctx.data(|d| d.get_temp::<usize>(sel_mem_id));
    if prev_sel != Some(*selected_idx) {
        ctx.data_mut(|d| d.insert_temp(sel_mem_id, *selected_idx));
        nav_moved = true;
    }
    let enter_trigger = ctx.input(|i| i.key_pressed(egui::Key::Enter));

    let glide_y = (if screen.width() < 600.0 { 44.0 } else { 86.0 }) - (1.0 - anim_t) * 12.0;

    egui::Area::new(egui::Id::new("palette_modal"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, glide_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(with_alpha(p.bg_elevated, anim_t))
                .stroke(Stroke::new(1.0_f32, with_alpha(p.border_subtle, anim_t)))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(0.0))
                .show(ui, |ui| {
                    ui.set_width(540.0_f32.min((screen.width() - 24.0).max(280.0)));

                    egui::Frame::none()
                        .inner_margin(Margin::symmetric(14.0, 12.0))
                        .show(ui, |ui| {
                            let mut palette_input_id: Option<egui::Id> = None;
                            ui.horizontal(|ui| {
                                let (ir, _) =
                                    ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                                draw_icon(
                                    ui.painter(),
                                    Rect::from_center_size(ir.center(), vec2(15.0, 15.0)),
                                    Icon::Search,
                                    p.accent_text,
                                );
                                let input_resp = ui.add(
                                    egui::TextEdit::singleline(query)
                                        .hint_text(s.palette_placeholder)
                                        .frame(false)
                                        .font(font_body(15.0))
                                        .desired_width(ui.available_width() - 44.0),
                                );
                                palette_input_id = Some(input_resp.id);
                                focus_once(
                                    ui.ctx(),
                                    egui::Id::new("palette_foc_gate"),
                                    *open,
                                    input_resp.id,
                                );
                                if input_resp.changed() {
                                    query_changed = true;
                                    *selected_idx = 0;
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        keycap_badge(ui, p, "ESC");
                                    },
                                );
                            });

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                ui.label(
                                    egui::RichText::new(s.palette_ops_hint)
                                        .font(font_body(11.0))
                                        .color(p.text_muted),
                                );
                                for op in ["tag:", "status:", "path:"] {
                                    let chip = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(op)
                                                .font(font_mono(11.0))
                                                .color(p.accent_text),
                                        )
                                        .fill(with_alpha(p.accent, 0.10))
                                        .rounding(Rounding::same(5.0))
                                        .min_size(vec2(0.0, 18.0)),
                                    );
                                    if chip.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                        if !query.is_empty() && !query.ends_with(char::is_whitespace) {
                                            query.push(' ');
                                        }
                                        query.push_str(op);
                                        query_changed = true;
                                        *selected_idx = 0;
                                        if let Some(id) = palette_input_id {
                                            ctx.memory_mut(|m| m.request_focus(id));
                                        }
                                    }
                                }
                            });
                        });

                    hairline_h(ui, p);

                    egui::ScrollArea::vertical()
                        .id_salt("palette_list_scroll")
                        .max_height(360.0)
                        .show(ui, |ui| {
                            egui::Frame::none()
                                .inner_margin(Margin::symmetric(10.0, 8.0))
                                .show(ui, |ui| {
                                    if entries.is_empty() {
                                        ui.add_space(18.0);
                                        ui.vertical_centered(|ui| {
                                            ui.label(
                                                egui::RichText::new(s.palette_no_results)
                                                    .font(font_body(13.0))
                                                    .color(p.text_muted),
                                            );
                                        });
                                        ui.add_space(18.0);
                                        return;
                                    }

                                    enum SlotKind<'a> {
                                        Note(&'a SearchHit),
                                        Command(&'a PaletteCommand),
                                    }
                                    struct PaletteSlot<'a> {
                                        is_sel: bool,
                                        rect: Rect,
                                        resp: egui::Response,
                                        kind: SlotKind<'a>,
                                    }

                                    let content_top = ui.cursor().top();
                                    let mut shown_notes_hdr = false;
                                    let mut shown_cmds_hdr = false;
                                    let mut slots: Vec<PaletteSlot<'_>> =
                                        Vec::with_capacity(entries.len());
                                    let mut active_rect: Option<Rect> = None;

                                    for (idx, entry) in entries.iter().enumerate() {
                                        let is_sel = idx == *selected_idx;
                                        match entry {
                                            PaletteEntry::Note(hit) => {
                                                if !shown_notes_hdr {
                                                    shown_notes_hdr = true;
                                                    ui.add_space(2.0);
                                                    section_heading(ui, p, s.palette_notes_header);
                                                    ui.add_space(4.0);
                                                }
                                                let w = ui.available_width();
                                                let row_h = if hit.snippet.is_empty() {
                                                    32.0
                                                } else {
                                                    46.0
                                                };
                                                let (rect, resp) = ui.allocate_exact_size(
                                                    vec2(w, row_h),
                                                    Sense::click(),
                                                );
                                                if is_sel && nav_moved {
                                                    resp.scroll_to_me(Some(egui::Align::Center));
                                                }
                                                if is_sel {
                                                    active_rect = Some(rect);
                                                }
                                                slots.push(PaletteSlot {
                                                    is_sel,
                                                    rect,
                                                    resp,
                                                    kind: SlotKind::Note(hit),
                                                });
                                            }
                                            PaletteEntry::Command(cmd) => {
                                                if !shown_cmds_hdr {
                                                    shown_cmds_hdr = true;
                                                    if shown_notes_hdr {
                                                        ui.add_space(8.0);
                                                    }
                                                    section_heading(
                                                        ui,
                                                        p,
                                                        s.palette_actions_header,
                                                    );
                                                    ui.add_space(4.0);
                                                }
                                                let w = ui.available_width();
                                                let (rect, resp) = ui.allocate_exact_size(
                                                    vec2(w, 30.0),
                                                    Sense::click(),
                                                );
                                                if is_sel && nav_moved {
                                                    resp.scroll_to_me(Some(egui::Align::Center));
                                                }
                                                if is_sel {
                                                    active_rect = Some(rect);
                                                }
                                                slots.push(PaletteSlot {
                                                    is_sel,
                                                    rect,
                                                    resp,
                                                    kind: SlotKind::Command(cmd),
                                                });
                                            }
                                        }
                                    }

                                    // Paint smoothly gliding active selection pill behind rows
                                    if let Some(target) = active_rect {
                                        let rel_y = target.top() - content_top;
                                        let pill_rel_y = anim_value_smooth(
                                            ctx,
                                            egui::Id::new("palette_pill_rel_y"),
                                            rel_y,
                                            0.10,
                                        );
                                        let pill_h = anim_value_smooth(
                                            ctx,
                                            egui::Id::new("palette_pill_h"),
                                            target.height(),
                                            0.10,
                                        );
                                        let pill_r = Rect::from_min_size(
                                            pos2(target.left(), content_top + pill_rel_y),
                                            vec2(target.width(), pill_h.max(24.0)),
                                        );
                                        ui.painter().rect_filled(
                                            pill_r,
                                            Rounding::same(7.0),
                                            p.bg_selected,
                                        );
                                    }

                                    for slot in slots {
                                        let hov_t = anim_bool_eased(
                                            ctx,
                                            slot.resp.id.with("hov"),
                                            slot.resp.hovered(),
                                            0.08,
                                        );
                                        if !slot.is_sel && hov_t > 0.01 {
                                            ui.painter().rect_filled(
                                                slot.rect,
                                                Rounding::same(7.0),
                                                with_alpha(p.bg_hover, hov_t),
                                            );
                                        }

                                        let rect = slot.rect;
                                        let is_sel = slot.is_sel;
                                        match slot.kind {
                                            SlotKind::Note(hit) => {
                                                let hit_icon = match hit.kind {
                                                    VaultFileKind::Board => Icon::Board,
                                                    VaultFileKind::Table => Icon::Table,
                                                    VaultFileKind::Canvas => Icon::Canvas,
                                                    _ => Icon::Document,
                                                };
                                                draw_icon(
                                                    ui.painter(),
                                                    Rect::from_center_size(
                                                        pos2(rect.left() + 16.0, rect.top() + 16.0),
                                                        vec2(14.0, 14.0),
                                                    ),
                                                    hit_icon,
                                                    if is_sel {
                                                        p.text_primary
                                                    } else {
                                                        p.text_muted
                                                    },
                                                );
                                                let path_hint = if rect.width() >= 390.0 {
                                                    Some(hit.path.clone())
                                                } else {
                                                    hit.path
                                                        .rsplit_once('/')
                                                        .map(|(folder, _)| format!("{folder}/"))
                                                };
                                                let path_g = path_hint.map(|ph| {
                                                    let mut pjob = egui::text::LayoutJob::single_section(
                                                        ph,
                                                        egui::TextFormat {
                                                            font_id: font_mono(10.5),
                                                            color: p.text_muted,
                                                            ..Default::default()
                                                        },
                                                    );
                                                    pjob.wrap = egui::text::TextWrapping {
                                                        max_width: (rect.width() * 0.36).max(60.0),
                                                        max_rows: 1,
                                                        break_anywhere: false,
                                                        overflow_character: Some('…'),
                                                    };
                                                    ui.fonts(|f| f.layout_job(pjob))
                                                });
                                                let title_max_w = (rect.width()
                                                    - if let Some(ref pg) = path_g {
                                                        pg.size().x + 52.0
                                                    } else {
                                                        44.0
                                                    })
                                                .max(80.0);
                                                let mut tjob =
                                                    egui::text::LayoutJob::single_section(
                                                        hit.title.clone(),
                                                        egui::TextFormat {
                                                            font_id: font_medium(13.5),
                                                            color: p.text_primary,
                                                            ..Default::default()
                                                        },
                                                    );
                                                tjob.wrap = egui::text::TextWrapping {
                                                    max_width: title_max_w,
                                                    max_rows: 1,
                                                    break_anywhere: false,
                                                    overflow_character: Some('…'),
                                                };
                                                let title_g = ui.fonts(|f| f.layout_job(tjob));
                                                ui.painter().galley(
                                                    pos2(rect.left() + 34.0, rect.top() + 7.0),
                                                    title_g,
                                                    p.text_primary,
                                                );
                                                if let Some(path_g) = path_g {
                                                    ui.painter().galley(
                                                        pos2(
                                                            rect.right() - 10.0 - path_g.size().x,
                                                            rect.top() + 9.0,
                                                        ),
                                                        path_g,
                                                        p.text_muted,
                                                    );
                                                }
                                                if !hit.snippet.is_empty() {
                                                    let mut sjob = egui::text::LayoutJob::default();
                                                    let normal_fmt = egui::TextFormat {
                                                        font_id: font_body(11.5),
                                                        color: p.text_secondary,
                                                        ..Default::default()
                                                    };
                                                    let hl_fmt = egui::TextFormat {
                                                        font_id: font_medium(11.5),
                                                        color: p.accent_text,
                                                        background: with_alpha(p.accent, 0.18),
                                                        ..Default::default()
                                                    };
                                                    for part in krono_core::search::highlight_snippet(
                                                        &hit.snippet,
                                                        &parsed_query.terms,
                                                    ) {
                                                        sjob.append(
                                                            &part.text,
                                                            0.0,
                                                            if part.highlighted {
                                                                hl_fmt.clone()
                                                            } else {
                                                                normal_fmt.clone()
                                                            },
                                                        );
                                                    }
                                                    sjob.wrap = egui::text::TextWrapping {
                                                        max_width: (rect.width() - 48.0).max(40.0),
                                                        max_rows: 1,
                                                        break_anywhere: true,
                                                        overflow_character: Some('…'),
                                                    };
                                                    let sg = ui.fonts(|f| f.layout_job(sjob));
                                                    ui.painter().galley(
                                                        pos2(rect.left() + 34.0, rect.top() + 25.0),
                                                        sg,
                                                        p.text_secondary,
                                                    );
                                                }

                                                if slot
                                                    .resp
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .clicked()
                                                    || (enter_trigger && is_sel)
                                                {
                                                    match hit.kind {
                                                        VaultFileKind::Board | VaultFileKind::Table => {
                                                            actions.push(UiAction::OpenCollection(
                                                                hit.path.clone(),
                                                            ));
                                                        }
                                                        VaultFileKind::Canvas => {
                                                            actions.push(UiAction::OpenCanvas(
                                                                hit.path.clone(),
                                                            ));
                                                        }
                                                        _ => {
                                                            actions.push(UiAction::OpenNote(
                                                                hit.path.clone(),
                                                            ));
                                                            actions.push(UiAction::SetViewMode(
                                                                ViewMode::Editor,
                                                            ));
                                                        }
                                                    }
                                                    *open = false;
                                                }
                                            }
                                            SlotKind::Command(cmd) => {
                                                draw_icon(
                                                    ui.painter(),
                                                    Rect::from_center_size(
                                                        pos2(rect.left() + 16.0, rect.center().y),
                                                        vec2(14.0, 14.0),
                                                    ),
                                                    cmd.icon,
                                                    if is_sel {
                                                        p.text_primary
                                                    } else {
                                                        p.text_secondary
                                                    },
                                                );
                                                let g = ui.painter().layout_no_wrap(
                                                    cmd.label.to_string(),
                                                    font_medium(13.0),
                                                    p.text_primary,
                                                );
                                                ui.painter().galley(
                                                    pos2(
                                                        rect.left() + 34.0,
                                                        rect.center().y - g.size().y * 0.5,
                                                    ),
                                                    g,
                                                    p.text_primary,
                                                );
                                                if slot
                                                    .resp
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .clicked()
                                                    || (enter_trigger && is_sel)
                                                {
                                                    actions.push(cmd.action.clone());
                                                    *open = false;
                                                }
                                            }
                                        }
                                    }
                                });
                        });

                    // Dedicated rounded footer bar BELOW ScrollArea so selected rows never bleed past 12px corners
                    hairline_h(ui, p);
                    egui::Frame::none()
                        .fill(with_alpha(p.bg_sidebar, anim_t))
                        .rounding(Rounding {
                            nw: 0.0,
                            ne: 0.0,
                            sw: 12.0,
                            se: 12.0,
                        })
                        .inner_margin(Margin::symmetric(14.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                                keycap_badge(ui, p, "↑↓");
                                ui.label(
                                    egui::RichText::new(s.palette_hint_nav)
                                        .font(font_body(11.5))
                                        .color(p.text_muted),
                                );
                                ui.add_space(8.0);
                                keycap_badge(ui, p, "↵");
                                ui.label(
                                    egui::RichText::new(s.palette_hint_open)
                                        .font(font_body(11.5))
                                        .color(p.text_muted),
                                );
                                ui.add_space(8.0);
                                keycap_badge(ui, p, "Esc");
                                ui.label(
                                    egui::RichText::new(s.palette_hint_close)
                                        .font(font_body(11.5))
                                        .color(p.text_muted),
                                );
                            });
                        });
                });
        });

    (query_changed, actions)
}
