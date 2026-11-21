use crate::config::{
    AccentPreset, AppConfig, CustomSlashSnippet, EditorWidth, SlashTrigger, ThemePreset,
};
use crate::i18n::{strings, Lang};
use crate::storage::vault::{list_custom_templates, AssetAuditEntry};
use crate::sync::updater::{AvailableUpdate, CURRENT_APP_VERSION};
use crate::ui::components::{
    hairline_h, icon_button, keycap_badge, paint_badge_dot, primary_button, secondary_button,
    segmented_control, shortcut_label, styled_text_input, toggle_switch,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::image_cache::ImageTextureCache;
use crate::ui::motion::{anim_bool_eased, anim_value_smooth, ease_out_back, lerp_color, with_alpha};
use crate::ui::theme::{
    accent_swatch_color, font_body, font_bold, font_medium, font_mono, font_semibold,
    font_serif_bold, ThemePalette,
};
use crate::ui::{SettingsTab, UiAction, VaultCounts};
use eframe::egui::{
    self, pos2, vec2, Align, CursorIcon, Layout, Margin, Rect, Rounding, Sense, Stroke, Ui,
};
use std::path::Path;

#[derive(Clone, Copy)]
pub struct UpdateUiState<'a> {
    pub checking: bool,
    pub installing: bool,
    pub available: Option<&'a AvailableUpdate>,
    pub status_msg: Option<&'a str>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_settings_view(
    ui: &mut Ui,
    p: &ThemePalette,
    cfg: &mut AppConfig,
    tab: &mut SettingsTab,
    tab_anim_t: f32,
    vault_draft: &mut String,
    has_saved_token: bool,
    token_replace_mode: &mut bool,
    token_draft: &mut String,
    test_feedback: &Option<(bool, String)>,
    last_sync_info: Option<&str>,
    sync_busy: bool,
    counts: VaultCounts,
    snippet_trigger: &mut String,
    snippet_label: &mut String,
    snippet_content: &mut String,
    asset_audit: &[AssetAuditEntry],
    asset_audit_only_unused: &mut bool,
    vault_dir: &Path,
    image_cache: &mut ImageTextureCache,
    update_ui: UpdateUiState<'_>,
) -> Vec<UiAction> {
    let lang = cfg.language;
    let s = strings(lang);
    let mut actions = Vec::new();

    let avail = ui.available_width();
    let max_w = 880.0_f32.min((avail - 8.0).max(260.0));
    let side_pad = ((avail - max_w) * 0.5).max(4.0);
    let compact_tabs = max_w < 640.0;

    ui.horizontal_top(|ui| {
        ui.add_space(side_pad);
        ui.vertical(|ui| {
            ui.set_width(max_w);
            ui.add_space(6.0);

            if compact_tabs {
                let nav_items = [
                    (SettingsTab::Appearance, Some(Icon::Palette), s.settings_tab_appearance),
                    (SettingsTab::Editor, Some(Icon::Type), s.settings_tab_editor),
                    (SettingsTab::MarkdownHelp, Some(Icon::CircleHelp), s.settings_tab_md_help),
                    (SettingsTab::Storage, Some(Icon::HardDrive), s.settings_tab_storage),
                    (SettingsTab::GitHub, Some(Icon::GitHub), s.settings_tab_github),
                    (SettingsTab::Shortcuts, Some(Icon::Keyboard), s.settings_tab_shortcuts),
                ];
                egui::ScrollArea::horizontal()
                    .id_salt("settings_top_tabs")
                    .show(ui, |ui| {
                        segmented_control(ui, p, "settings_top_tabs_seg", tab, &nav_items);
                    });
                ui.add_space(16.0);
                render_tab_scroll(
                    ui,
                    p,
                    cfg,
                    *tab,
                    tab_anim_t,
                    vault_draft,
                    has_saved_token,
                    token_replace_mode,
                    token_draft,
                    test_feedback,
                    last_sync_info,
                    sync_busy,
                    counts,
                    snippet_trigger,
                    snippet_label,
                    snippet_content,
                    asset_audit,
                    asset_audit_only_unused,
                    vault_dir,
                    image_cache,
                    update_ui,
                    &mut actions,
                );
            } else {
                let nav_w = 194.0;
                let content_w = (max_w - nav_w - 28.0).max(320.0);

                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(nav_w);
                        ui.spacing_mut().item_spacing.y = 4.0;
                        let nav_items = [
                            (SettingsTab::Appearance, Icon::Palette, s.settings_tab_appearance),
                            (SettingsTab::Editor, Icon::Type, s.settings_tab_editor),
                            (SettingsTab::MarkdownHelp, Icon::CircleHelp, s.settings_tab_md_help),
                            (SettingsTab::Storage, Icon::HardDrive, s.settings_tab_storage),
                            (SettingsTab::GitHub, Icon::GitHub, s.settings_tab_github),
                            (SettingsTab::Shortcuts, Icon::Keyboard, s.settings_tab_shortcuts),
                        ];

                        struct NavSlot {
                            tab: SettingsTab,
                            icon: Icon,
                            label: &'static str,
                            rect: Rect,
                            resp: egui::Response,
                            active: bool,
                        }

                        let mut slots = Vec::with_capacity(nav_items.len());
                        let mut active_rect: Option<Rect> = None;
                        for (t_val, icon, label) in nav_items {
                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(nav_w, 34.0), Sense::click());
                            let active = *tab == t_val;
                            if active {
                                active_rect = Some(rect);
                            }
                            slots.push(NavSlot {
                                tab: t_val,
                                icon,
                                label,
                                rect,
                                resp,
                                active,
                            });
                        }

                        if let Some(target) = active_rect {
                            let pill_y = anim_value_smooth(
                                ui.ctx(),
                                egui::Id::new("settings_nav_pill_y"),
                                target.top(),
                                0.16,
                            );
                            let pill_r =
                                Rect::from_min_size(pos2(target.left(), pill_y), target.size());
                            ui.painter()
                                .rect_filled(pill_r, Rounding::same(7.0), p.bg_selected);
                            let bar_r = Rect::from_center_size(
                                pos2(pill_r.left() + 3.0, pill_r.center().y),
                                vec2(2.5, 16.0),
                            );
                            ui.painter()
                                .rect_filled(bar_r, Rounding::same(1.5), p.accent);
                        }

                        for slot in slots {
                            let hov_t = anim_bool_eased(
                                ui.ctx(),
                                ui.make_persistent_id(("settings_nav", slot.label)),
                                slot.resp.hovered(),
                                0.10,
                            );
                            if !slot.active && hov_t > 0.01 {
                                ui.painter().rect_filled(
                                    slot.rect,
                                    Rounding::same(7.0),
                                    with_alpha(p.bg_hover, hov_t),
                                );
                            }
                            let icon_col = if slot.active {
                                p.accent_text
                            } else {
                                lerp_color(p.text_muted, p.text_secondary, hov_t)
                            };
                            let text_col = if slot.active {
                                p.text_primary
                            } else {
                                lerp_color(p.text_secondary, p.text_primary, hov_t)
                            };
                            let ir = Rect::from_center_size(
                                pos2(slot.rect.left() + 17.0, slot.rect.center().y),
                                vec2(15.0, 15.0),
                            );
                            draw_icon(ui.painter(), ir, slot.icon, icon_col);
                            let g = ui.painter().layout_no_wrap(
                                slot.label.to_string(),
                                if slot.active {
                                    font_medium(13.5)
                                } else {
                                    font_body(13.5)
                                },
                                text_col,
                            );
                            ui.painter().galley(
                                pos2(
                                    slot.rect.left() + 34.0,
                                    slot.rect.center().y - g.size().y * 0.5,
                                ),
                                g,
                                text_col,
                            );
                            if slot.resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                *tab = slot.tab;
                            }
                        }
                    });

                    ui.add_space(28.0);

                    ui.vertical(|ui| {
                        ui.set_width(content_w);
                        render_tab_scroll(
                            ui,
                            p,
                            cfg,
                            *tab,
                            tab_anim_t,
                            vault_draft,
                            has_saved_token,
                            token_replace_mode,
                            token_draft,
                            test_feedback,
                            last_sync_info,
                            sync_busy,
                            counts,
                            snippet_trigger,
                            snippet_label,
                            snippet_content,
                            asset_audit,
                            asset_audit_only_unused,
                            vault_dir,
                            image_cache,
                            update_ui,
                            &mut actions,
                        );
                    });
                });
            }
        });
    });

    actions
}

#[allow(clippy::too_many_arguments)]
fn render_tab_scroll(
    ui: &mut Ui,
    p: &ThemePalette,
    cfg: &mut AppConfig,
    tab: SettingsTab,
    tab_anim_t: f32,
    vault_draft: &mut String,
    has_saved_token: bool,
    token_replace_mode: &mut bool,
    token_draft: &mut String,
    test_feedback: &Option<(bool, String)>,
    last_sync_info: Option<&str>,
    sync_busy: bool,
    counts: VaultCounts,
    snippet_trigger: &mut String,
    snippet_label: &mut String,
    snippet_content: &mut String,
    asset_audit: &[AssetAuditEntry],
    asset_audit_only_unused: &mut bool,
    vault_dir: &Path,
    image_cache: &mut ImageTextureCache,
    update_ui: UpdateUiState<'_>,
    actions: &mut Vec<UiAction>,
) {
    let lang = cfg.language;
    let s = strings(lang);

    egui::ScrollArea::vertical()
        .id_salt(("settings_scroll", tab as u8))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let content_w = ui.available_width();
            ui.set_width(content_w);
            if tab_anim_t < 0.99 {
                ui.add_space((1.0 - tab_anim_t) * 7.0);
            }

            match tab {
                SettingsTab::Appearance => {
                    section_intro(ui, p, s.settings_tab_appearance, s.settings_appearance_desc);
                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.settings_themes_heading)
                                .font(font_medium(13.5))
                                .color(p.text_primary),
                        );
                        ui.add_space(12.0);

                        let theme_cols = if ui.available_width() < 420.0 { 2 } else { 3 };
                        for row in ThemePreset::ALL.chunks(theme_cols) {
                            ui.columns(row.len(), |cols| {
                                for (idx, &preset) in row.iter().enumerate() {
                                    if theme_preview_card(
                                        &mut cols[idx],
                                        p,
                                        lang,
                                        preset,
                                        cfg.accent,
                                        cfg.theme == preset,
                                    )
                                    .clicked()
                                    {
                                        cfg.select_theme(preset);
                                        actions.push(UiAction::ConfigChanged);
                                    }
                                }
                            });
                            ui.add_space(10.0);
                        }
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        setting_row(
                            ui,
                            p,
                            s.settings_accent_heading,
                            s.settings_accent_desc,
                            170.0,
                            |ui| {
                                ui.allocate_ui_with_layout(
                                    vec2(162.0, 26.0),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        for acc in AccentPreset::ALL {
                                            if accent_swatch_button(
                                                ui,
                                                p,
                                                lang,
                                                acc,
                                                cfg.accent == acc,
                                            )
                                            .clicked()
                                            {
                                                cfg.accent = acc;
                                                actions.push(UiAction::ConfigChanged);
                                            }
                                        }
                                    },
                                );
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_language,
                            s.settings_language_desc,
                            210.0,
                            |ui| {
                                let mut chosen = cfg.language;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_lang_seg",
                                    &mut chosen,
                                    &[(Lang::Ru, None, "Русский"), (Lang::En, None, "English")],
                                ) {
                                    cfg.language = chosen;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });
                }

                SettingsTab::Editor => {
                    section_intro(ui, p, s.settings_tab_editor, s.settings_editor_desc);
                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        setting_row(
                            ui,
                            p,
                            s.settings_font_size,
                            s.settings_font_size_desc,
                            230.0,
                            |ui| {
                                let mut sz = cfg.editor_font_size.round() as i32;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_fontsize_seg",
                                    &mut sz,
                                    &[
                                        (14, None, "14"),
                                        (15, None, "15"),
                                        (16, None, "16"),
                                        (17, None, "17"),
                                        (18, None, "18"),
                                    ],
                                ) {
                                    cfg.editor_font_size = sz as f32;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_heading_style,
                            s.settings_heading_style_desc,
                            180.0,
                            |ui| {
                                let mut serif = cfg.heading_serif;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_heading_style_seg",
                                    &mut serif,
                                    &[
                                        (true, None, s.heading_serif),
                                        (false, None, s.heading_sans),
                                    ],
                                ) {
                                    cfg.heading_serif = serif;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_column_width,
                            s.settings_column_width_desc,
                            250.0,
                            |ui| {
                                let mut width = cfg.editor_width;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_width_seg",
                                    &mut width,
                                    &[
                                        (EditorWidth::Compact, None, s.width_compact),
                                        (EditorWidth::Normal, None, s.width_normal),
                                        (EditorWidth::Wide, None, s.width_wide),
                                    ],
                                ) {
                                    cfg.editor_width = width;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        setting_row(
                            ui,
                            p,
                            s.settings_auto_save,
                            s.settings_auto_save_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_auto_save",
                                    &mut cfg.auto_save_enabled,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_task_notifications,
                            s.settings_task_notifications_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_task_notifications",
                                    &mut cfg.task_notifications,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_show_footer,
                            s.settings_show_footer_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_show_footer",
                                    &mut cfg.show_editor_footer,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_show_inspector,
                            s.settings_show_inspector_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_show_inspector",
                                    &mut cfg.inspector_open,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });

                    ui.add_space(14.0);

                    // Markdown Assist toggles & trigger config
                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.md_assist_heading)
                                .font(font_medium(14.0))
                                .color(p.text_primary),
                        );
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(s.md_assist_desc)
                                    .font(font_body(12.0))
                                    .color(p.text_muted),
                            )
                            .wrap(),
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_slash_menu,
                            s.md_slash_menu_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(ui, p, "sw_md_slash", &mut cfg.md_slash_menu) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_wikilink_ac,
                            s.md_wikilink_ac_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_md_wiki",
                                    &mut cfg.md_wikilink_autocomplete,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_auto_lists,
                            s.md_auto_lists_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(ui, p, "sw_md_lists", &mut cfg.md_auto_lists) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_auto_pairs,
                            s.md_auto_pairs_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(ui, p, "sw_md_pairs", &mut cfg.md_auto_pairs) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_show_toolbar,
                            s.md_show_toolbar_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(ui, p, "sw_md_toolbar", &mut cfg.md_show_toolbar) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.md_slash_trigger,
                            s.md_slash_trigger_desc,
                            120.0,
                            |ui| {
                                let mut trig = cfg.md_slash_trigger;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_slash_trig_seg",
                                    &mut trig,
                                    &[
                                        (SlashTrigger::Slash, None, "/"),
                                        (SlashTrigger::Backslash, None, "\\"),
                                    ],
                                ) {
                                    cfg.md_slash_trigger = trig;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });

                    ui.add_space(14.0);

                    // Daily Note Template selector (.krono/templates/*.md)
                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.settings_daily_template)
                                .font(font_medium(14.0))
                                .color(p.text_primary),
                        );
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(s.settings_daily_template_desc)
                                    .font(font_body(12.0))
                                    .color(p.text_muted),
                            )
                            .wrap(),
                        );
                        ui.add_space(8.0);
                        let custom_tpls = list_custom_templates(vault_dir);
                        let current_label = if cfg.daily_template.is_empty() {
                            s.settings_daily_template_builtin.to_string()
                        } else {
                            custom_tpls
                                .iter()
                                .find(|t| t.id == cfg.daily_template)
                                .map(|t| t.name.clone())
                                .unwrap_or_else(|| cfg.daily_template.clone())
                        };
                        egui::ComboBox::from_id_salt("settings_daily_template_combo")
                            .selected_text(current_label)
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        cfg.daily_template.is_empty(),
                                        s.settings_daily_template_builtin,
                                    )
                                    .clicked()
                                {
                                    cfg.daily_template.clear();
                                    actions.push(UiAction::ConfigChanged);
                                }
                                for tpl in &custom_tpls {
                                    let active = cfg.daily_template == tpl.id;
                                    if ui.selectable_label(active, &tpl.name).clicked() {
                                        cfg.daily_template = tpl.id.clone();
                                        actions.push(UiAction::ConfigChanged);
                                    }
                                }
                            });
                    });

                    ui.add_space(14.0);

                    // Custom Slash Commands / Snippets editor
                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.md_snippets_heading)
                                .font(font_medium(14.0))
                                .color(p.text_primary),
                        );
                        ui.add_space(2.0);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(s.md_snippets_desc)
                                    .font(font_body(12.0))
                                    .color(p.text_muted),
                            )
                            .wrap(),
                        );
                        ui.add_space(10.0);

                        if cfg.custom_slash_snippets.is_empty() {
                            ui.label(
                                egui::RichText::new(s.md_snippet_empty)
                                    .font(font_body(12.0))
                                    .color(p.text_muted),
                            );
                        } else {
                            let mut remove_idx = None;
                            let trig_ch = cfg.md_slash_trigger.as_char();
                            for (idx, snip) in cfg.custom_slash_snippets.iter().enumerate() {
                                if idx > 0 {
                                    ui.add_space(6.0);
                                    hairline_h(ui, p);
                                    ui.add_space(6.0);
                                }
                                ui.horizontal(|ui| {
                                    keycap_badge(ui, p, &format!("{trig_ch}{}", snip.trigger));
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(&snip.label)
                                            .font(font_medium(12.5))
                                            .color(p.text_primary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if icon_button(ui, p, Icon::Trash, s.delete_btn, false)
                                            .clicked()
                                        {
                                            remove_idx = Some(idx);
                                        }
                                    });
                                });
                            }
                            if let Some(i) = remove_idx {
                                cfg.custom_slash_snippets.remove(i);
                                actions.push(UiAction::ConfigChanged);
                            }
                        }

                        ui.add_space(10.0);
                        hairline_h(ui, p);
                        ui.add_space(10.0);

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                            let half_w = ((ui.available_width() - 12.0) * 0.5).max(120.0);
                            styled_text_input(
                                ui,
                                p,
                                snippet_trigger,
                                s.md_snippet_trigger_hint,
                                false,
                                half_w.min(180.0),
                            );
                            styled_text_input(
                                ui,
                                p,
                                snippet_label,
                                s.md_snippet_label_hint,
                                false,
                                half_w,
                            );
                        });
                        ui.add_space(6.0);
                        egui::Frame::none()
                            .fill(p.bg_input)
                            .stroke(Stroke::new(1.0_f32, p.border_subtle))
                            .rounding(Rounding::same(7.0))
                            .inner_margin(Margin::symmetric(10.0, 6.0))
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(snippet_content)
                                        .hint_text(s.md_snippet_content_hint)
                                        .frame(false)
                                        .font(font_mono(12.0))
                                        .desired_rows(3)
                                        .desired_width(f32::INFINITY),
                                );
                            });
                        ui.add_space(8.0);
                        if primary_button(ui, p, Some(Icon::Plus), s.md_snippet_add_btn, false)
                            .clicked()
                        {
                            let trig = snippet_trigger
                                .trim()
                                .trim_start_matches('/')
                                .trim_start_matches('\\')
                                .to_string();
                            let content = snippet_content.clone();
                            if !trig.is_empty() && !content.trim().is_empty() {
                                let label = if snippet_label.trim().is_empty() {
                                    trig.clone()
                                } else {
                                    snippet_label.trim().to_string()
                                };
                                cfg.custom_slash_snippets.push(CustomSlashSnippet {
                                    trigger: trig,
                                    label,
                                    content,
                                });
                                snippet_trigger.clear();
                                snippet_label.clear();
                                snippet_content.clear();
                                actions.push(UiAction::ConfigChanged);
                            }
                        }
                    });
                }

                SettingsTab::MarkdownHelp => {
                    render_markdown_cheatsheet_tab(ui, p, cfg, actions);
                }

                SettingsTab::Storage => {
                    section_intro(ui, p, s.settings_tab_storage, s.settings_storage_desc);
                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.settings_vault_dir)
                                .font(font_medium(13.5))
                                .color(p.text_primary),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(s.settings_vault_dir_desc)
                                .font(font_body(12.0))
                                .color(p.text_muted),
                        );
                        ui.add_space(10.0);

                        styled_text_input(ui, p, vault_draft, "", false, f32::INFINITY);
                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                            if secondary_button(
                                ui,
                                p,
                                Some(Icon::ExternalLink),
                                s.settings_open_folder,
                            )
                            .clicked()
                            {
                                actions.push(UiAction::OpenVaultFolder);
                            }
                            if primary_button(ui, p, None, s.settings_apply_vault, false).clicked()
                            {
                                actions.push(UiAction::ApplyVaultDir(
                                    vault_draft.trim().to_string(),
                                ));
                            }
                        });
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.settings_index_heading)
                                .font(font_medium(13.5))
                                .color(p.text_primary),
                        );
                        ui.add_space(12.0);

                        ui.columns(3, |cols| {
                            index_stat_tile(&mut cols[0], p, counts.notes, s.stat_notes_count);
                            index_stat_tile(&mut cols[1], p, counts.boards, s.stat_boards_count);
                            index_stat_tile(&mut cols[2], p, counts.tables, s.stat_tables_count);
                        });
                        ui.add_space(8.0);
                        ui.columns(3, |cols| {
                            index_stat_tile(&mut cols[0], p, counts.canvases, s.stat_canvases_count);
                            index_stat_tile(&mut cols[1], p, counts.tags, s.stat_tags_count);
                            index_stat_tile(&mut cols[2], p, counts.links, s.stat_links_count);
                        });

                        ui.add_space(14.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_rebuild_index,
                            s.settings_rebuild_index_desc,
                            180.0,
                            |ui| {
                                if secondary_button(
                                    ui,
                                    p,
                                    Some(Icon::RotateCcw),
                                    s.settings_rebuild_index,
                                )
                                .clicked()
                                {
                                    actions.push(UiAction::RebuildIndex);
                                }
                            },
                        );
                    });

                    ui.add_space(16.0);
                    settings_card(ui, p, |ui| {
                        let total_bytes: u64 = asset_audit.iter().map(|a| a.size).sum();
                        let unused_count = asset_audit.iter().filter(|a| a.ref_count == 0).count();

                        ui.horizontal(|ui| {
                            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                            draw_icon(ui.painter(), ir, Icon::Image, p.accent);
                            ui.label(
                                egui::RichText::new(s.assets_section_title)
                                    .font(font_medium(13.5))
                                    .color(p.text_primary),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                if unused_count > 0
                                    && primary_button(
                                        ui,
                                        p,
                                        Some(Icon::Trash),
                                        s.assets_clean_unused_btn,
                                        false,
                                    )
                                    .clicked()
                                {
                                    actions.push(UiAction::DeleteUnusedAssets);
                                }
                                if secondary_button(
                                    ui,
                                    p,
                                    Some(Icon::RotateCcw),
                                    s.assets_refresh_btn,
                                )
                                .clicked()
                                {
                                    actions.push(UiAction::RefreshAssetAudit);
                                }
                            });
                        });

                        ui.add_space(10.0);
                        ui.columns(3, |cols| {
                            index_stat_tile(&mut cols[0], p, asset_audit.len(), s.assets_total_count);
                            asset_size_tile(&mut cols[1], p, total_bytes, s.assets_total_size);
                            index_stat_tile(&mut cols[2], p, unused_count, s.assets_unused_count);
                        });

                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if segmented_filter_btn(
                                ui,
                                p,
                                s.assets_filter_all,
                                !*asset_audit_only_unused,
                            )
                            .clicked()
                            {
                                *asset_audit_only_unused = false;
                            }
                            if segmented_filter_btn(
                                ui,
                                p,
                                &format!("{} ({})", s.assets_filter_unused, unused_count),
                                *asset_audit_only_unused,
                            )
                            .clicked()
                            {
                                *asset_audit_only_unused = true;
                            }
                        });

                        ui.add_space(10.0);
                        hairline_h(ui, p);
                        ui.add_space(8.0);

                        let filtered: Vec<&AssetAuditEntry> = asset_audit
                            .iter()
                            .filter(|a| !*asset_audit_only_unused || a.ref_count == 0)
                            .collect();
                        if filtered.is_empty() {
                            ui.label(
                                egui::RichText::new(s.assets_empty)
                                    .font(font_body(12.5))
                                    .color(p.text_muted),
                            );
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("settings_assets_scroll")
                                .max_height(260.0)
                                .show(ui, |ui| {
                                    for entry in filtered {
                                        let orphan = entry.ref_count == 0;
                                        egui::Frame::none()
                                            .fill(p.bg_elevated)
                                            .stroke(Stroke::new(1.0_f32, p.border_hairline))
                                            .rounding(Rounding::same(8.0))
                                            .inner_margin(Margin::symmetric(8.0, 6.0))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.spacing_mut().item_spacing.x = 8.0;
                                                    let (trect, tresp) = ui.allocate_exact_size(
                                                        vec2(36.0, 36.0),
                                                        Sense::click(),
                                                    );
                                                    ui.painter().rect_filled(
                                                        trect,
                                                        Rounding::same(5.0),
                                                        p.bg_input,
                                                    );
                                                    if let Some((tex_id, _)) =
                                                        image_cache.get_or_load(
                                                            ui.ctx(),
                                                            vault_dir,
                                                            &entry.rel_path,
                                                        )
                                                    {
                                                        ui.painter().image(
                                                            tex_id,
                                                            trect,
                                                            Rect::from_min_max(
                                                                pos2(0.0, 0.0),
                                                                pos2(1.0, 1.0),
                                                            ),
                                                            egui::Color32::WHITE,
                                                        );
                                                    } else {
                                                        draw_icon(
                                                            ui.painter(),
                                                            Rect::from_center_size(
                                                                trect.center(),
                                                                vec2(16.0, 16.0),
                                                            ),
                                                            Icon::Image,
                                                            p.text_muted,
                                                        );
                                                    }
                                                    if tresp
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        actions.push(UiAction::OpenImageLightbox(
                                                            entry.rel_path.clone(),
                                                        ));
                                                    }

                                                    ui.vertical(|ui| {
                                                        ui.spacing_mut().item_spacing.y = 2.0;
                                                        ui.add(
                                                            egui::Label::new(
                                                                egui::RichText::new(&entry.rel_path)
                                                                    .font(font_medium(12.0))
                                                                    .color(p.text_primary),
                                                            )
                                                            .truncate(),
                                                        );
                                                        ui.label(
                                                            egui::RichText::new(format_bytes_short(
                                                                entry.size,
                                                            ))
                                                            .font(font_mono(11.0))
                                                            .color(p.text_muted),
                                                        );
                                                    });

                                                    ui.with_layout(
                                                        Layout::right_to_left(Align::Center),
                                                        |ui| {
                                                            ui.spacing_mut().item_spacing.x = 6.0;
                                                            if icon_button(
                                                                ui,
                                                                p,
                                                                Icon::Trash,
                                                                s.delete_btn,
                                                                false,
                                                            )
                                                            .clicked()
                                                            {
                                                                actions.push(
                                                                    UiAction::DeleteSingleAsset(
                                                                        entry.rel_path.clone(),
                                                                    ),
                                                                );
                                                            }
                                                            let (bg, fg, lbl) = if orphan {
                                                                (
                                                                    p.danger_soft,
                                                                    p.danger,
                                                                    s.assets_unused_badge
                                                                        .to_string(),
                                                                )
                                                            } else {
                                                                (
                                                                    p.success_soft,
                                                                    p.success,
                                                                    format!(
                                                                        "{} {}",
                                                                        entry.ref_count,
                                                                        s.assets_used_badge
                                                                    ),
                                                                )
                                                            };
                                                            let g = ui.painter().layout_no_wrap(
                                                                lbl,
                                                                font_medium(11.0),
                                                                fg,
                                                            );
                                                            let (br, bresp) = ui.allocate_exact_size(
                                                                vec2(g.size().x + 12.0, 20.0),
                                                                Sense::hover(),
                                                            );
                                                            ui.painter().rect_filled(
                                                                br,
                                                                Rounding::same(5.0),
                                                                bg,
                                                            );
                                                            ui.painter().galley(
                                                                pos2(
                                                                    br.left() + 6.0,
                                                                    br.center().y - g.size().y * 0.5,
                                                                ),
                                                                g,
                                                                fg,
                                                            );
                                                            if !entry.referenced_by.is_empty() {
                                                                bresp.on_hover_text(
                                                                    entry.referenced_by.join("\n"),
                                                                );
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(5.0);
                                    }
                                });
                        }
                    });

                    ui.add_space(16.0);
                    settings_card(ui, p, |ui| {
                        ui.horizontal(|ui| {
                            let (ir, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                            draw_icon(ui.painter(), ir, Icon::Sparkles, p.accent);
                            ui.label(
                                egui::RichText::new(s.update_section_title)
                                    .font(font_medium(13.5))
                                    .color(p.text_primary),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_enabled_ui(
                                    !update_ui.checking && !update_ui.installing,
                                    |ui| {
                                        let check_lbl = if update_ui.checking {
                                            s.update_checking
                                        } else {
                                            s.update_check_btn
                                        };
                                        if secondary_button(
                                            ui,
                                            p,
                                            Some(Icon::RotateCcw),
                                            check_lbl,
                                        )
                                        .clicked()
                                        {
                                            actions.push(UiAction::CheckForUpdates);
                                        }
                                    },
                                );
                            });
                        });

                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 8.0;
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: v{}",
                                    s.update_current_ver, CURRENT_APP_VERSION
                                ))
                                .font(font_mono(12.0))
                                .color(p.text_secondary),
                            );
                            if let Some(msg) = update_ui.status_msg {
                                ui.label(
                                    egui::RichText::new(format!("· {}", msg))
                                        .font(font_body(12.0))
                                        .color(p.text_muted),
                                );
                            }
                        });

                        if let Some(upd) = update_ui.available {
                            ui.add_space(10.0);
                            egui::Frame::none()
                                .fill(p.accent_soft)
                                .stroke(Stroke::new(1.0_f32, p.accent))
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::symmetric(12.0, 10.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}: {}",
                                                    s.update_available_badge, upd.tag_name
                                                ))
                                                .font(font_semibold(13.0))
                                                .color(p.accent),
                                            );
                                            ui.label(
                                                egui::RichText::new(&upd.asset_name)
                                                    .font(font_mono(11.5))
                                                    .color(p.text_secondary),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                ui.add_enabled_ui(!update_ui.installing, |ui| {
                                                    let inst_lbl = if update_ui.installing {
                                                        s.update_installing
                                                    } else {
                                                        s.update_install_btn
                                                    };
                                                    if primary_button(
                                                        ui,
                                                        p,
                                                        Some(Icon::Sparkles),
                                                        inst_lbl,
                                                        false,
                                                    )
                                                    .clicked()
                                                    {
                                                        actions.push(UiAction::InstallUpdate);
                                                    }
                                                });
                                            },
                                        );
                                    });
                                });
                        }

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.update_auto_check,
                            s.update_auto_check_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_auto_check_updates",
                                    &mut cfg.auto_check_updates,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.update_prerelease,
                            s.update_prerelease_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_include_prerelease_updates",
                                    &mut cfg.include_prerelease_updates,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });
                }

                SettingsTab::GitHub => {
                    section_intro(ui, p, s.settings_tab_github, s.settings_github_desc);
                    ui.add_space(14.0);

                    // Connection summary card
                    let configured = cfg.is_github_configured(has_saved_token);
                    settings_card(ui, p, |ui| {
                        let narrow = ui.available_width() < 440.0;
                        let sync_text = format!(
                            "{} {}",
                            s.gh_last_sync,
                            last_sync_info.unwrap_or(s.gh_never_synced)
                        );
                        if narrow {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                                let (dot_r, _) =
                                    ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                                paint_badge_dot(
                                    ui.painter(),
                                    dot_r.center(),
                                    if configured { p.success } else { p.text_muted },
                                );
                                ui.label(
                                    egui::RichText::new(if configured {
                                        s.gh_status_connected
                                    } else {
                                        s.gh_status_disconnected
                                    })
                                    .font(font_medium(13.5))
                                    .color(p.text_primary),
                                );
                                if configured {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "· {} ({})",
                                            cfg.github_repo.trim(),
                                            cfg.github_branch.trim()
                                        ))
                                        .font(font_mono(12.0))
                                        .color(p.text_secondary),
                                    );
                                }
                            });
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(sync_text)
                                    .font(font_body(12.0))
                                    .color(p.text_muted),
                            );
                        } else {
                            ui.horizontal(|ui| {
                                let (dot_r, _) =
                                    ui.allocate_exact_size(vec2(10.0, 16.0), Sense::hover());
                                paint_badge_dot(
                                    ui.painter(),
                                    dot_r.center(),
                                    if configured { p.success } else { p.text_muted },
                                );
                                ui.label(
                                    egui::RichText::new(if configured {
                                        s.gh_status_connected
                                    } else {
                                        s.gh_status_disconnected
                                    })
                                    .font(font_medium(13.5))
                                    .color(p.text_primary),
                                );
                                if configured {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "· {} ({})",
                                            cfg.github_repo.trim(),
                                            cfg.github_branch.trim()
                                        ))
                                        .font(font_mono(12.0))
                                        .color(p.text_secondary),
                                    );
                                }
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new(sync_text)
                                            .font(font_body(12.0))
                                            .color(p.text_muted),
                                    );
                                });
                            });
                        }
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        ui.label(
                            egui::RichText::new(s.gh_repo_label)
                                .font(font_medium(12.5))
                                .color(p.text_secondary),
                        );
                        ui.add_space(5.0);
                        if styled_text_input(
                            ui,
                            p,
                            &mut cfg.github_repo,
                            s.gh_repo_hint,
                            false,
                            f32::INFINITY,
                        )
                        .changed()
                        {
                            actions.push(UiAction::ConfigChanged);
                        }

                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(s.gh_branch_label)
                                .font(font_medium(12.5))
                                .color(p.text_secondary),
                        );
                        ui.add_space(5.0);
                        if styled_text_input(
                            ui,
                            p,
                            &mut cfg.github_branch,
                            "main",
                            false,
                            190.0,
                        )
                        .changed()
                        {
                            actions.push(UiAction::ConfigChanged);
                        }

                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(s.gh_token_label)
                                .font(font_medium(12.5))
                                .color(p.text_secondary),
                        );
                        ui.add_space(5.0);
                        if has_saved_token && !*token_replace_mode {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                                egui::Frame::none()
                                    .fill(p.success_soft)
                                    .rounding(Rounding::same(6.0))
                                    .inner_margin(Margin::symmetric(10.0, 5.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (ir, _) = ui.allocate_exact_size(
                                                vec2(12.0, 12.0),
                                                Sense::hover(),
                                            );
                                            draw_icon(ui.painter(), ir, Icon::Check, p.success);
                                            ui.label(
                                                egui::RichText::new(s.gh_token_saved)
                                                    .font(font_medium(12.0))
                                                    .color(p.success),
                                            );
                                        });
                                    });
                                if secondary_button(ui, p, Some(Icon::Edit), s.gh_token_replace)
                                    .clicked()
                                {
                                    token_draft.clear();
                                    *token_replace_mode = true;
                                }
                                if secondary_button(ui, p, Some(Icon::Trash), s.gh_token_delete)
                                    .clicked()
                                {
                                    actions.push(UiAction::DeleteGitHubToken);
                                }
                            });
                        } else {
                            if !has_saved_token {
                                ui.label(
                                    egui::RichText::new(s.gh_token_missing)
                                        .font(font_body(12.0))
                                        .color(p.text_muted),
                                );
                                ui.add_space(4.0);
                            }
                            let tok_w = (ui.available_width() - 180.0).max(160.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                                styled_text_input(
                                    ui,
                                    p,
                                    token_draft,
                                    s.gh_token_hint,
                                    true,
                                    tok_w,
                                );
                                ui.add_enabled_ui(!token_draft.trim().is_empty(), |ui| {
                                    if primary_button(
                                        ui,
                                        p,
                                        Some(Icon::Check),
                                        s.gh_token_save,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::SaveGitHubToken);
                                    }
                                });
                                if *token_replace_mode
                                    && secondary_button(ui, p, Some(Icon::Close), s.cancel_btn)
                                        .clicked()
                                {
                                    token_draft.clear();
                                    *token_replace_mode = false;
                                }
                            });
                        }

                        ui.add_space(14.0);
                        ui.add_enabled_ui(!sync_busy, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
                                if secondary_button(ui, p, Some(Icon::GitHub), s.gh_test_btn)
                                    .clicked()
                                {
                                    actions.push(UiAction::TestGitHubConnection);
                                }
                                if primary_button(
                                    ui,
                                    p,
                                    Some(Icon::Sync),
                                    if sync_busy {
                                        s.syncing_label
                                    } else {
                                        s.gh_sync_now_btn
                                    },
                                    false,
                                )
                                .clicked()
                                {
                                    actions.push(UiAction::SyncNow);
                                }
                            });
                        });

                        if let Some((ok, msg)) = test_feedback {
                            ui.add_space(12.0);
                            egui::Frame::none()
                                .fill(if *ok { p.success_soft } else { p.danger_soft })
                                .rounding(Rounding::same(7.0))
                                .inner_margin(Margin::symmetric(12.0, 8.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let (ir, _) = ui
                                            .allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                        draw_icon(
                                            ui.painter(),
                                            ir,
                                            if *ok { Icon::Check } else { Icon::Alert },
                                            if *ok { p.success } else { p.danger },
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(msg)
                                                    .font(font_medium(12.5))
                                                    .color(if *ok { p.success } else { p.danger }),
                                            )
                                            .wrap(),
                                        );
                                    });
                                });
                        }
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        setting_row(
                            ui,
                            p,
                            s.settings_auto_sync,
                            s.settings_auto_sync_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_auto_sync_start",
                                    &mut cfg.auto_sync_on_start,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_auto_sync_after_save,
                            s.settings_auto_sync_after_save_desc,
                            56.0,
                            |ui| {
                                if toggle_switch(
                                    ui,
                                    p,
                                    "sw_auto_sync_after_save",
                                    &mut cfg.auto_sync_after_save,
                                ) {
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );

                        ui.add_space(12.0);
                        hairline_h(ui, p);
                        ui.add_space(12.0);

                        setting_row(
                            ui,
                            p,
                            s.settings_sync_interval,
                            s.settings_sync_interval_desc,
                            210.0,
                            |ui| {
                                let mut interval = cfg.auto_sync_interval_mins;
                                if segmented_control(
                                    ui,
                                    p,
                                    "settings_sync_interval_seg",
                                    &mut interval,
                                    &[
                                        (0u32, None, s.sync_interval_off),
                                        (5u32, None, s.sync_interval_5m),
                                        (15u32, None, s.sync_interval_15m),
                                    ],
                                ) {
                                    cfg.auto_sync_interval_mins = interval;
                                    actions.push(UiAction::ConfigChanged);
                                }
                            },
                        );
                    });

                    ui.add_space(14.0);

                    settings_card(ui, p, |ui| {
                        egui::CollapsingHeader::new(
                            egui::RichText::new(s.gh_guide_title)
                                .font(font_medium(13.0))
                                .color(p.accent_text),
                        )
                        .show(ui, |ui| {
                            ui.add_space(4.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(s.gh_guide_body)
                                        .font(font_body(12.5))
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        });
                    });
                }

                SettingsTab::Shortcuts => {
                    section_intro(ui, p, s.settings_tab_shortcuts, s.settings_shortcuts_desc);
                    ui.add_space(14.0);

                    let rows = [
                        (s.sc_palette, shortcut_label("K")),
                        (s.sc_new_note, shortcut_label("N")),
                        (s.sc_daily_note, shortcut_label("D")),
                        (s.sc_save_note, shortcut_label("S")),
                        (
                            s.sc_format,
                            if cfg!(target_os = "macos") {
                                "⌘B / I / E".to_string()
                            } else {
                                "Ctrl+B / I / E".to_string()
                            },
                        ),
                        (
                            s.sc_sync_now,
                            if cfg!(target_os = "macos") {
                                "⇧⌘S".to_string()
                            } else {
                                "Ctrl+Shift+S".to_string()
                            },
                        ),
                        (s.sc_toggle_sidebar, shortcut_label("\\")),
                        (
                            s.sc_toggle_inspector,
                            if cfg!(target_os = "macos") {
                                "⇧⌘I".to_string()
                            } else {
                                "Ctrl+Shift+I".to_string()
                            },
                        ),
                        (s.sc_open_graph, shortcut_label("G")),
                        (s.sc_open_settings, shortcut_label(",")),
                        (s.sc_close_overlay, "Esc".to_string()),
                    ];

                    settings_card(ui, p, |ui| {
                        for (idx, (desc, keys)) in rows.iter().enumerate() {
                            if idx > 0 {
                                ui.add_space(9.0);
                                hairline_h(ui, p);
                                ui.add_space(9.0);
                            }
                            let left_w = (ui.available_width() - 104.0).max(120.0);
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(left_w);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(*desc)
                                                .font(font_body(13.0))
                                                .color(p.text_primary),
                                        )
                                        .wrap(),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    keycap_badge(ui, p, keys);
                                });
                            });
                        }
                    });
                }
            }

            ui.add_space(28.0);
        });
}

fn render_markdown_cheatsheet_tab(
    ui: &mut Ui,
    p: &ThemePalette,
    cfg: &AppConfig,
    actions: &mut Vec<UiAction>,
) {
    let lang = cfg.language;
    let s = strings(lang);
    section_intro(ui, p, s.settings_tab_md_help, s.md_help_desc);
    ui.add_space(10.0);

    egui::Frame::none()
        .fill(p.accent_soft)
        .stroke(Stroke::new(1.0_f32, p.accent))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                draw_icon(ui.painter(), ir, Icon::Sparkles, p.accent_text);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(s.md_help_tip)
                            .font(font_body(12.5))
                            .color(p.accent_text),
                    )
                    .wrap(),
                );
            });
        });

    ui.add_space(14.0);

    let items: &[(&str, &str, &str)] = match lang {
        Lang::Ru => &[
            ("Заголовки (H1–H3)", "# Заголовок 1\n## Подзаголовок 2\n### Секция 3", "heading"),
            ("Акценты в тексте", "**Жирный**  *Курсив*  ~~Зачёркнутый~~  `код`", "inline"),
            ("Чек-лист задач", "- [ ] Сделать сегодня\n- [x] Уже готово", "task"),
            ("Списки", "- Пункт списка\n1. Первый шаг", "list"),
            ("Цитата / Выноска", "> Важная мысль или напоминание", "quote"),
            ("Блок кода", "```rust\nprintln!(\"Krono\");\n```", "codeblock"),
            ("Ссылки между заметками", "[[Architecture]] и [[roadmap|План]]\n[GitHub](https://github.com)", "link"),
            ("Таблица", "| Колонка 1 | Колонка 2 |\n| --- | --- |\n| Данные | Значение |", "table"),
            ("Разделитель", "---", "hr"),
        ],
        Lang::En => &[
            ("Headings (H1–H3)", "# Heading 1\n## Heading 2\n### Heading 3", "heading"),
            ("Inline Formatting", "**Bold**  *Italic*  ~~Strike~~  `code`", "inline"),
            ("Task Checklist", "- [ ] Open task\n- [x] Completed item", "task"),
            ("Lists", "- Bullet point\n1. Numbered step", "list"),
            ("Blockquote", "> Highlight an insight or key takeaway", "quote"),
            ("Code Block", "```rust\nprintln!(\"Krono\");\n```", "codeblock"),
            ("Links & Wikilinks", "[[Architecture]] and [[roadmap|Plan]]\n[GitHub](https://github.com)", "link"),
            ("Markdown Table", "| Column 1 | Column 2 |\n| --- | --- |\n| Item | Status |", "table"),
            ("Horizontal Rule", "---", "hr"),
        ],
    };

    for &(title, syntax, kind) in items {
        settings_card(ui, p, |ui| {
            let narrow = ui.available_width() < 460.0;
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .font(font_medium(13.0))
                        .color(p.text_primary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Copy, s.copy_btn, false).clicked() {
                        actions.push(UiAction::CopyText(syntax.to_string()));
                    }
                });
            });
            ui.add_space(8.0);

            if narrow {
                cheat_syntax_box(ui, p, s.md_help_col_syntax, syntax, ui.available_width());
                ui.add_space(8.0);
                cheat_preview_box(ui, p, lang, cfg.heading_serif, s.md_help_col_result, kind, ui.available_width());
            } else {
                let col_w = ((ui.available_width() - 12.0) * 0.5).max(160.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        cheat_syntax_box(ui, p, s.md_help_col_syntax, syntax, col_w);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.set_width(col_w);
                        cheat_preview_box(ui, p, lang, cfg.heading_serif, s.md_help_col_result, kind, col_w);
                    });
                });
            }
        });
        ui.add_space(10.0);
    }
}

fn cheat_syntax_box(ui: &mut Ui, p: &ThemePalette, hdr: &str, syntax: &str, width: f32) {
    ui.label(
        egui::RichText::new(hdr)
            .font(font_body(11.0))
            .color(p.text_muted),
    );
    ui.add_space(4.0);
    egui::Frame::none()
        .fill(p.bg_code)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width((width - 20.0).max(80.0));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(syntax)
                        .font(font_mono(11.5))
                        .color(p.text_secondary),
                )
                .wrap(),
            );
        });
}

fn cheat_preview_box(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    heading_serif: bool,
    hdr: &str,
    kind: &str,
    width: f32,
) {
    ui.label(
        egui::RichText::new(hdr)
            .font(font_body(11.0))
            .color(p.text_muted),
    );
    ui.add_space(4.0);
    egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            ui.set_width((width - 20.0).max(80.0));
            match kind {
                "heading" => {
                    ui.label(
                        egui::RichText::new(if lang == Lang::Ru { "Заголовок 1" } else { "Heading 1" })
                            .font(if heading_serif { font_serif_bold(17.0) } else { font_bold(16.5) })
                            .color(p.text_primary),
                    );
                    ui.label(
                        egui::RichText::new(if lang == Lang::Ru { "Подзаголовок 2" } else { "Heading 2" })
                            .font(font_semibold(14.0))
                            .color(p.text_primary),
                    );
                }
                "inline" => {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                        ui.label(egui::RichText::new("Bold").font(font_bold(12.5)).color(p.text_primary));
                        ui.label(egui::RichText::new("Italic").font(font_body(12.5)).italics().color(p.text_primary));
                        ui.label(egui::RichText::new("Strike").font(font_body(12.5)).strikethrough().color(p.text_muted));
                        egui::Frame::none()
                            .fill(p.bg_code)
                            .rounding(Rounding::same(4.0))
                            .inner_margin(Margin::symmetric(5.0, 1.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("code").font(font_mono(11.5)).color(p.accent_text));
                            });
                    });
                }
                "task" => {
                    ui.label(
                        egui::RichText::new(if lang == Lang::Ru { "☐  Сделать сегодня" } else { "☐  Open task" })
                            .font(font_body(12.5))
                            .color(p.text_primary),
                    );
                    ui.label(
                        egui::RichText::new(if lang == Lang::Ru { "☑  Уже готово" } else { "☑  Completed item" })
                            .font(font_body(12.5))
                            .strikethrough()
                            .color(p.text_muted),
                    );
                }
                "list" => {
                    ui.label(egui::RichText::new(if lang == Lang::Ru { "•  Пункт списка\n1.  Первый шаг" } else { "•  Bullet point\n1.  Numbered step" }).font(font_body(12.5)).color(p.text_primary));
                }
                "quote" => {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(3.0, 20.0), Sense::hover());
                        ui.painter().rect_filled(r, Rounding::same(1.5), p.accent);
                        ui.label(
                            egui::RichText::new(if lang == Lang::Ru { "Важная мысль" } else { "Highlighted note" })
                                .font(font_body(12.5))
                                .italics()
                                .color(p.text_secondary),
                        );
                    });
                }
                "codeblock" => {
                    ui.label(
                        egui::RichText::new("println!(\"Krono\");")
                            .font(font_mono(11.5))
                            .color(p.accent_text),
                    );
                }
                "link" => {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                        egui::Frame::none()
                            .fill(p.accent_soft)
                            .rounding(Rounding::same(4.0))
                            .inner_margin(Margin::symmetric(6.0, 2.0))
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new("Architecture").font(font_medium(12.0)).color(p.accent_text));
                            });
                        ui.label(egui::RichText::new("GitHub ↗").font(font_body(12.0)).underline().color(p.accent_text));
                    });
                }
                "table" => {
                    ui.label(
                        egui::RichText::new(if lang == Lang::Ru { "Колонка 1  |  Колонка 2\nДанные      |  Значение" } else { "Column 1   |  Column 2\nItem         |  Status" })
                            .font(font_mono(11.0))
                            .color(p.text_primary),
                    );
                }
                _ => {
                    ui.add_space(6.0);
                    hairline_h(ui, p);
                    ui.add_space(6.0);
                }
            }
        });
}

fn section_intro(ui: &mut Ui, p: &ThemePalette, title: &str, desc: &str) {
    ui.label(
        egui::RichText::new(title)
            .font(font_semibold(20.0))
            .color(p.text_primary),
    );
    ui.add_space(3.0);
    ui.label(
        egui::RichText::new(desc)
            .font(font_body(13.0))
            .color(p.text_secondary),
    );
}

fn settings_card(ui: &mut Ui, p: &ThemePalette, add_contents: impl FnOnce(&mut Ui)) {
    let pad_x = if ui.available_width() < 400.0 { 12.0 } else { 18.0 };
    egui::Frame::none()
        .fill(p.bg_card)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::symmetric(pad_x, 14.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

fn setting_row(
    ui: &mut Ui,
    p: &ThemePalette,
    title: &str,
    desc: &str,
    right_reserve: f32,
    add_control: impl FnOnce(&mut Ui),
) {
    let total_w = ui.available_width();
    if total_w < right_reserve + 180.0 {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .font(font_medium(13.5))
                    .color(p.text_primary),
            );
            if !desc.is_empty() {
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(desc)
                            .font(font_body(12.0))
                            .color(p.text_muted),
                    )
                    .wrap(),
                );
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                add_control(ui);
            });
        });
        return;
    }

    let left_w = (total_w - right_reserve - 16.0).max(120.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(left_w);
            ui.label(
                egui::RichText::new(title)
                    .font(font_medium(13.5))
                    .color(p.text_primary),
            );
            if !desc.is_empty() {
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(desc)
                            .font(font_body(12.0))
                            .color(p.text_muted),
                    )
                    .wrap(),
                );
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            add_control(ui);
        });
    });
}

fn theme_name(lang: Lang, preset: ThemePreset) -> &'static str {
    let s = strings(lang);
    match preset {
        ThemePreset::Graphite => s.theme_graphite,
        ThemePreset::ObsidianOled => s.theme_obsidian,
        ThemePreset::Everforest => s.theme_everforest,
        ThemePreset::WarmEspresso => s.theme_espresso,
        ThemePreset::Paper => s.theme_paper,
        ThemePreset::EditorialSand => s.theme_sand,
    }
}

fn accent_name(lang: Lang, accent: AccentPreset) -> &'static str {
    let s = strings(lang);
    match accent {
        AccentPreset::SteelBlue => s.accent_steel_blue,
        AccentPreset::Sage => s.accent_sage,
        AccentPreset::Amber => s.accent_amber,
        AccentPreset::Terracotta => s.accent_terracotta,
        AccentPreset::GraphiteMono => s.accent_mono,
    }
}

fn theme_preview_card(
    ui: &mut Ui,
    ui_p: &ThemePalette,
    lang: Lang,
    preset: ThemePreset,
    accent: AccentPreset,
    selected: bool,
) -> egui::Response {
    let tp = ThemePalette::for_preset(preset, accent);
    let card_w = ui.available_width().max(120.0);
    let (slot_rect, resp) = ui.allocate_exact_size(vec2(card_w, 118.0), Sense::click());
    let rest_rect = slot_rect.shrink2(vec2(0.0, 2.0));
    let sel_t = anim_bool_eased(
        ui.ctx(),
        ui.make_persistent_id(("theme_card_sel", preset as u8)),
        selected,
        0.16,
    );
    let hov_t = anim_bool_eased(
        ui.ctx(),
        ui.make_persistent_id(("theme_card", preset as u8)),
        resp.hovered(),
        0.11,
    );
    let press_t = anim_bool_eased(
        ui.ctx(),
        ui.make_persistent_id(("theme_card_prs", preset as u8)),
        resp.is_pointer_button_down_on(),
        0.07,
    );
    let lift = hov_t * 2.0 - press_t * 1.2;
    let rect = rest_rect.translate(vec2(0.0, -lift));

    let sh_factor = hov_t.max(sel_t * 0.55);
    if sh_factor > 0.02 {
        let mut sh = ui_p.shadow_elevated;
        sh.color = with_alpha(sh.color, sh_factor);
        ui.painter().add(sh.as_shape(rect, Rounding::same(9.0)));
    }

    let outer_bg = lerp_color(
        lerp_color(ui_p.bg_input, ui_p.bg_hover, hov_t),
        ui_p.bg_elevated,
        sel_t,
    );
    let border_col = lerp_color(
        lerp_color(ui_p.border_subtle, ui_p.border_strong, hov_t),
        ui_p.accent,
        sel_t,
    );
    let border_w = 1.0 + 0.8 * sel_t;

    ui.painter()
        .rect_filled(rect, Rounding::same(9.0), outer_bg);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(9.0),
        Stroke::new(border_w, border_col),
    );

    // Miniature Krono UI window preview inside 6px padding
    let preview_rect = Rect::from_min_max(
        pos2(rect.left() + 6.0, rect.top() + 6.0),
        pos2(rect.right() - 6.0, rect.bottom() - 32.0),
    );
    ui.painter()
        .rect_filled(preview_rect, Rounding::same(5.5), tp.bg_canvas);

    let sb_w = (preview_rect.width() * 0.29).max(24.0);
    let sb_rect = Rect::from_min_max(
        preview_rect.min,
        pos2(preview_rect.left() + sb_w, preview_rect.bottom()),
    );
    ui.painter().rect_filled(
        sb_rect,
        Rounding {
            nw: 5.5,
            ne: 0.0,
            sw: 5.5,
            se: 0.0,
        },
        tp.bg_sidebar,
    );
    let div_x = sb_rect.right().round();
    ui.painter().line_segment(
        [pos2(div_x, preview_rect.top()), pos2(div_x, preview_rect.bottom())],
        Stroke::new(1.0_f32, tp.border_hairline),
    );

    // Sidebar skeleton rows
    for (row_i, fill) in [tp.text_muted, tp.bg_selected, tp.text_muted]
        .into_iter()
        .enumerate()
    {
        let y = sb_rect.top() + 12.0 + (row_i as f32) * 11.0;
        let row_r = Rect::from_min_size(
            pos2(sb_rect.left() + 6.0, y),
            vec2((sb_w - 12.0).max(8.0), 5.0),
        );
        ui.painter().rect_filled(row_r, Rounding::same(2.0), fill);
    }

    // Canvas skeleton content (title bar, accent pill, 2 lines)
    let cx = sb_rect.right() + 10.0;
    let cw = (preview_rect.right() - 10.0 - cx).max(20.0);
    let title_r = Rect::from_min_size(pos2(cx, preview_rect.top() + 12.0), vec2(cw * 0.56, 6.0));
    ui.painter()
        .rect_filled(title_r, Rounding::same(2.0), tp.text_primary);

    let pill_r = Rect::from_min_size(pos2(cx, preview_rect.top() + 24.0), vec2(cw * 0.30, 8.0));
    ui.painter()
        .rect_filled(pill_r, Rounding::same(4.0), tp.accent_soft);
    ui.painter()
        .circle_filled(pos2(pill_r.left() + 4.5, pill_r.center().y), 2.0, tp.accent);

    let line1_r = Rect::from_min_size(pos2(cx, preview_rect.top() + 40.0), vec2(cw * 0.88, 3.5));
    let line2_r = Rect::from_min_size(pos2(cx, preview_rect.top() + 48.0), vec2(cw * 0.68, 3.5));
    ui.painter()
        .rect_filled(line1_r, Rounding::same(1.5), tp.text_secondary);
    ui.painter()
        .rect_filled(line2_r, Rounding::same(1.5), tp.text_muted);

    ui.painter().rect_stroke(
        preview_rect,
        Rounding::same(5.5),
        Stroke::new(1.0_f32, tp.border_subtle),
    );

    // Bottom label & animated check indicator
    let label_y = preview_rect.bottom() + 16.0;
    let label_col = lerp_color(ui_p.text_secondary, ui_p.text_primary, sel_t.max(hov_t * 0.6));
    let g = ui.painter().layout_no_wrap(
        theme_name(lang, preset).to_string(),
        if selected {
            font_medium(12.5)
        } else {
            font_body(12.5)
        },
        label_col,
    );
    ui.painter().galley(
        pos2(rect.left() + 10.0, label_y - g.size().y * 0.5),
        g,
        label_col,
    );

    if sel_t > 0.02 {
        let chk_c = pos2(rect.right() - 16.0, label_y);
        let pop = ease_out_back(sel_t).max(0.0);
        ui.painter()
            .circle_filled(chk_c, 7.5 * pop, with_alpha(ui_p.accent, sel_t));
        draw_icon(
            ui.painter(),
            Rect::from_center_size(chk_c, vec2(10.0 * pop, 10.0 * pop)),
            Icon::Check,
            with_alpha(ui_p.accent_on_fill, sel_t),
        );
    }

    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn accent_swatch_button(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    accent: AccentPreset,
    selected: bool,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
    let fill = accent_swatch_color(accent, p.is_dark);
    let sel_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("sel"),
        selected,
        0.16,
    );
    let hov_t = anim_bool_eased(
        ui.ctx(),
        resp.id.with("hov"),
        resp.hovered(),
        0.10,
    );

    let ring_alpha = sel_t.max(hov_t * 0.75);
    if ring_alpha > 0.02 {
        let ring_col = lerp_color(
            with_alpha(p.border_strong, hov_t),
            with_alpha(p.text_primary, sel_t),
            sel_t,
        );
        ui.painter().circle_stroke(
            rect.center(),
            12.0,
            Stroke::new(1.2 + 0.4 * sel_t, ring_col),
        );
    }

    ui.painter()
        .circle_filled(rect.center(), 9.0 + hov_t * 1.1, fill);
    if sel_t > 0.05 {
        let on_sw = if p.is_dark && accent == AccentPreset::GraphiteMono {
            egui::Color32::from_rgb(22, 22, 20)
        } else {
            egui::Color32::WHITE
        };
        let pop = ease_out_back(sel_t).max(0.1);
        draw_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(10.0 * pop, 10.0 * pop)),
            Icon::Check,
            with_alpha(on_sw, sel_t),
        );
    }

    resp.on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(accent_name(lang, accent))
}

fn stat_tile_str(ui: &mut Ui, p: &ThemePalette, value: &str, label: &str) {
    egui::Frame::none()
        .fill(p.bg_input)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(value)
                    .font(font_mono(20.0))
                    .color(p.text_primary),
            );
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(label)
                    .font(font_body(12.0))
                    .color(p.text_muted),
            );
        });
}

fn index_stat_tile(ui: &mut Ui, p: &ThemePalette, value: usize, label: &str) {
    stat_tile_str(ui, p, &value.to_string(), label);
}

fn asset_size_tile(ui: &mut Ui, p: &ThemePalette, bytes: u64, label: &str) {
    stat_tile_str(ui, p, &format_bytes_short(bytes), label);
}

fn format_bytes_short(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn segmented_filter_btn(
    ui: &mut Ui,
    p: &ThemePalette,
    label: &str,
    active: bool,
) -> egui::Response {
    let fg = if active { p.accent_text } else { p.text_secondary };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.0), fg);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 18.0, 26.0), Sense::click());
    let bg = if active {
        p.accent_soft
    } else if resp.hovered() {
        p.bg_hover
    } else {
        p.bg_input
    };
    ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(1.0_f32, if active { p.accent } else { p.border_hairline }),
    );
    ui.painter().galley(
        pos2(rect.left() + 9.0, rect.center().y - g.size().y * 0.5),
        g,
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}
