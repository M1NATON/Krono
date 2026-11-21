use crate::i18n::{column_type_label, strings, Lang};
use crate::ui::collection_board::render_custom_board;
use crate::ui::components::{
    focus_once, hairline_h, icon_button, normalize_tag_input, primary_button, reset_focus_gate,
    secondary_button, styled_text_input, tag_autocomplete_input, tag_chip,
};
use crate::ui::icons::{draw_icon, Icon};
use crate::ui::motion::{anim_bool_eased, ease_in_out_cubic, lerp_color, with_alpha};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{LayoutClass, UiAction};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, CursorIcon, Layout, Margin, PopupCloseBehavior, Rect,
    Rounding, Sense, Stroke, Ui,
};
use krono_core::{
    collection::{
        record_matches_smart_filter, strip_link_target, ChecklistItem, CollectionDoc,
        CollectionViewKind, ColorToken, ColumnDef, ColumnType, DateFilterPreset, FieldValue,
        RecordItem, SavedCollectionView,
    },
    daily::SimpleDate,
    note::NoteMeta,
};
use std::collections::BTreeSet;
use std::hash::Hash;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionSortDir {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectionSort {
    pub column_key: String,
    pub dir: CollectionSortDir,
}

pub struct CollectionEditorState {
    pub path: Option<String>,
    pub doc: CollectionDoc,
    pub modified: i64,
    pub dirty: bool,
    pub last_edit_at: Option<Instant>,
    pub external_conflict: bool,
    pub active_view: CollectionViewKind,

    pub filter_query: String,
    pub date_filter: DateFilterPreset,
    pub tag_filter: Option<String>,
    pub select_filter: Option<(String, String)>,
    pub smart_filter_open: bool,
    pub active_saved_view_id: Option<String>,
    pub new_view_name: String,
    pub sort: Option<CollectionSort>,
    pub title_col_width: f32,
    pub mobile_active_opt_id: String,
    pub quick_add_col_opt: Option<String>,
    pub quick_add_at_top: bool,
    pub quick_add_title: String,
    pub quick_add_tags: Vec<String>,
    pub quick_add_tag_input: String,
    pub adding_board_stage: bool,
    pub new_stage_name: String,
    pub new_stage_color: ColorToken,

    pub editing_record_id: Option<String>,
    pub modal_new_tag: String,
    pub modal_new_check_item: String,
    pub modal_note_search: String,

    pub editing_column_id: Option<String>,
    pub new_option_label: String,
    pub new_option_color: ColorToken,
    pub adding_field_open: bool,
    pub new_field_name: String,
    pub new_field_type: ColumnType,
}

impl Default for CollectionEditorState {
    fn default() -> Self {
        Self {
            path: None,
            doc: CollectionDoc::default(),
            modified: 0,
            dirty: false,
            last_edit_at: None,
            external_conflict: false,
            active_view: CollectionViewKind::Board,
            filter_query: String::new(),
            date_filter: DateFilterPreset::All,
            tag_filter: None,
            select_filter: None,
            smart_filter_open: false,
            active_saved_view_id: None,
            new_view_name: String::new(),
            sort: None,
            title_col_width: 240.0,
            mobile_active_opt_id: String::new(),
            quick_add_col_opt: None,
            quick_add_at_top: false,
            quick_add_title: String::new(),
            quick_add_tags: Vec::new(),
            quick_add_tag_input: String::new(),
            adding_board_stage: false,
            new_stage_name: String::new(),
            new_stage_color: ColorToken::Blue,
            editing_record_id: None,
            modal_new_tag: String::new(),
            modal_new_check_item: String::new(),
            modal_note_search: String::new(),
            editing_column_id: None,
            new_option_label: String::new(),
            new_option_color: ColorToken::Blue,
            adding_field_open: false,
            new_field_name: String::new(),
            new_field_type: ColumnType::Text,
        }
    }
}

impl CollectionEditorState {
    pub fn reset_transient(&mut self) {
        self.external_conflict = false;
        self.filter_query.clear();
        self.date_filter = DateFilterPreset::All;
        self.tag_filter = None;
        self.select_filter = None;
        self.smart_filter_open = false;
        self.active_saved_view_id = None;
        self.new_view_name.clear();
        self.sort = None;
        self.mobile_active_opt_id.clear();
        self.quick_add_col_opt = None;
        self.quick_add_at_top = false;
        self.quick_add_title.clear();
        self.quick_add_tags.clear();
        self.quick_add_tag_input.clear();
        self.adding_board_stage = false;
        self.new_stage_name.clear();
        self.editing_record_id = None;
        self.modal_new_tag.clear();
        self.modal_new_check_item.clear();
        self.modal_note_search.clear();
        self.editing_column_id = None;
        self.new_option_label.clear();
        self.adding_field_open = false;
        self.new_field_name.clear();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_collection_view(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    state: &mut CollectionEditorState,
    all_notes: &[NoteMeta],
) -> Vec<UiAction> {
    let mut actions = Vec::new();
    let s = strings(lang);

    let Some(rel_path) = state.path.clone() else {
        return actions;
    };

    if state.external_conflict {
        render_external_conflict_banner(ui, p, lang, &mut actions);
        ui.add_space(10.0);
    }

    // Header: Title + Pin + Item count + View Switcher + Filter + Add Field/Stage
    render_collection_header(ui, p, lang, layout, &rel_path, state, &mut actions);
    ui.add_space(10.0);

    let available_tags = collect_available_tags(&state.doc, all_notes);

    match state.active_view {
        CollectionViewKind::Board => {
            render_custom_board(ui, p, lang, layout, today, state, &available_tags, &mut actions);
        }
        CollectionViewKind::Table => {
            render_custom_table(ui, p, lang, layout, today, state, all_notes, &mut actions);
        }
    }

    // Modals (Card detail & Column schema)
    render_record_detail_modal(ctx, p, lang, state, all_notes, &available_tags, &mut actions);
    render_column_schema_modal(ctx, p, lang, state, &mut actions);
    render_add_field_modal(ctx, p, lang, state, &mut actions);

    let _ = s;
    actions
}

fn render_external_conflict_banner(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    egui::Frame::none()
        .fill(p.warning_soft)
        .stroke(Stroke::new(1.0_f32, p.warning))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(12.0, 9.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(15.0, 16.0), Sense::hover());
                draw_icon(ui.painter(), ir, Icon::Alert, p.warning);
                ui.label(
                    egui::RichText::new(s.ext_conflict_msg)
                        .font(font_medium(12.5))
                        .color(p.text_primary),
                );
                if secondary_button(ui, p, None, s.ext_conflict_reload).clicked() {
                    actions.push(UiAction::CollectionExternalReload);
                }
                if secondary_button(ui, p, None, s.ext_conflict_keep).clicked() {
                    actions.push(UiAction::CollectionExternalKeepMine);
                }
                if secondary_button(ui, p, None, s.ext_conflict_copy).clicked() {
                    actions.push(UiAction::CollectionExternalSaveConflictCopy);
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn render_collection_header(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    rel_path: &str,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let is_mobile = layout.is_mobile();

    ui.horizontal(|ui| {
        let mut title = state.doc.title.clone();
        let title_font = font_semibold(if is_mobile { 18.0 } else { 22.0 });
        let measure_txt = if title.trim().is_empty() {
            s.untitled_hint
        } else {
            title.as_str()
        };
        let measured_w = ui
            .painter()
            .layout_no_wrap(measure_txt.to_string(), title_font.clone(), p.text_primary)
            .size()
            .x;
        let edit_w = if is_mobile {
            (ui.available_width() - 120.0).max(110.0)
        } else {
            let max_w = (ui.available_width() * 0.45).clamp(140.0, 360.0);
            (measured_w + 18.0).clamp(96.0, max_w)
        };
        let resp = ui.add(
            egui::TextEdit::singleline(&mut title)
                .frame(false)
                .font(title_font)
                .text_color(p.text_primary)
                .hint_text(s.untitled_hint)
                .desired_width(edit_w),
        );
        if resp.changed() {
            state.doc.title = title;
            actions.push(UiAction::MarkCollectionDirty);
        }

        if icon_button(
            ui,
            p,
            if state.doc.pinned { Icon::Pin } else { Icon::PinOff },
            s.tooltip_pin,
            state.doc.pinned,
        )
        .clicked()
        {
            actions.push(UiAction::TogglePin(rel_path.to_string()));
        }

        ui.add_space(2.0);
        egui::Frame::none()
            .fill(p.bg_elevated)
            .stroke(Stroke::new(1.0_f32, p.border_hairline))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(8.0, 2.0))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("{} {}", state.doc.records.len(), s.col_items_unit))
                        .font(font_mono(11.0))
                        .color(p.text_secondary),
                );
            });
    });

    ui.add_space(8.0);
    let has_active_filters = state.date_filter != DateFilterPreset::All
        || state.tag_filter.is_some()
        || state.select_filter.is_some()
        || !state.filter_query.trim().is_empty();
    let filter_sub_active = state.date_filter != DateFilterPreset::All
        || state.tag_filter.is_some()
        || state.select_filter.is_some();

    let render_left_toolbar = |ui: &mut Ui, state: &mut CollectionEditorState| {
        let search_w = if is_mobile {
            (ui.available_width() - 150.0).max(110.0)
        } else {
            210.0
        };
        if styled_text_input(
            ui,
            p,
            &mut state.filter_query,
            s.col_search_hint,
            false,
            search_w,
        )
        .changed()
        {
            state.active_saved_view_id = None;
        }

        if smart_filter_pill(
            ui,
            p,
            Some(Icon::Filter),
            s.col_filters_btn,
            state.smart_filter_open || filter_sub_active,
        )
        .clicked()
        {
            state.smart_filter_open = !state.smart_filter_open;
        }

        if has_active_filters
            && smart_filter_pill(ui, p, Some(Icon::Close), s.col_clear_filters, false).clicked()
        {
            state.filter_query.clear();
            state.date_filter = DateFilterPreset::All;
            state.tag_filter = None;
            state.select_filter = None;
            state.active_saved_view_id = None;
        }
    };

    let render_right_toolbar = |ui: &mut Ui, state: &mut CollectionEditorState, actions: &mut Vec<UiAction>| {
        if secondary_button(ui, p, Some(Icon::Plus), s.col_add_field).clicked() {
            state.adding_field_open = true;
            state.new_field_name.clear();
            state.new_field_type = ColumnType::Text;
        }

        let select_cols = state.doc.select_columns();
        if state.active_view == CollectionViewKind::Board && select_cols.len() > 1 {
            let current_gid = state
                .doc
                .effective_group_column_id()
                .unwrap_or("")
                .to_string();
            let cur_name = select_cols
                .iter()
                .find(|c| c.id == current_gid)
                .map(|c| c.name.as_str())
                .unwrap_or("");
            let gid_pop = ui.make_persistent_id("group_col_picker");
            let g_btn = secondary_button(
                ui,
                p,
                Some(Icon::ChevronDown),
                &format!("{}: {}", s.col_group_by, cur_name),
            );
            if g_btn.clicked() {
                ui.memory_mut(|m| m.toggle_popup(gid_pop));
            }
            let mut chosen_gid: Option<String> = None;
            let opts: Vec<(String, String)> = select_cols
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect();
            egui::popup_below_widget(
                ui,
                gid_pop,
                &g_btn,
                PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(160.0);
                    for (cid, cname) in opts {
                        let sel = cid == current_gid;
                        if crate::ui::components::context_menu_row(
                            ui,
                            p,
                            if sel { Icon::Check } else { Icon::List },
                            &cname,
                            false,
                        )
                        .clicked()
                        {
                            chosen_gid = Some(cid);
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                },
            );
            if let Some(cid) = chosen_gid {
                state.doc.group_by_column = Some(cid);
                actions.push(UiAction::MarkCollectionDirty);
            }
        }

        // Sort picker for both Board and Table views
        {
            let sort_pop = ui.make_persistent_id("collection_sort_picker");
            let sort_lbl = match &state.sort {
                None => {
                    if lang == Lang::Ru {
                        "Сортировка".to_string()
                    } else {
                        "Sort".to_string()
                    }
                }
                Some(sr) => {
                    let col_name = if sr.column_key == "__title" {
                        s.table_col_name
                    } else {
                        state
                            .doc
                            .columns
                            .iter()
                            .find(|c| c.id == sr.column_key)
                            .map(|c| c.name.as_str())
                            .unwrap_or("")
                    };
                    let arrow = if sr.dir == CollectionSortDir::Asc { "↑" } else { "↓" };
                    format!("{col_name} {arrow}")
                }
            };
            if let Some(ref mut sr) = state.sort {
                if icon_button(ui, p, Icon::Close, s.col_clear_filters, false).clicked() {
                    state.sort = None;
                } else {
                    let dir_lbl = if sr.dir == CollectionSortDir::Asc { "↑" } else { "↓" };
                    if secondary_button(ui, p, None, dir_lbl).clicked() {
                        sr.dir = match sr.dir {
                            CollectionSortDir::Asc => CollectionSortDir::Desc,
                            CollectionSortDir::Desc => CollectionSortDir::Asc,
                        };
                    }
                }
            }
            let s_btn = secondary_button(ui, p, Some(Icon::ChevronDown), &sort_lbl);
            if s_btn.clicked() {
                ui.memory_mut(|m| m.toggle_popup(sort_pop));
            }
            let mut chosen_sort: Option<Option<String>> = None;
            let cols_list: Vec<(String, String)> = state
                .doc
                .columns
                .iter()
                .map(|c| (c.id.clone(), c.name.clone()))
                .collect();
            let cur_key = state.sort.as_ref().map(|sr| sr.column_key.clone());
            egui::popup_below_widget(
                ui,
                sort_pop,
                &s_btn,
                PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(170.0);
                    let manual_lbl = if lang == Lang::Ru { "Ручной порядок" } else { "Manual order" };
                    if crate::ui::components::context_menu_row(
                        ui,
                        p,
                        if cur_key.is_none() { Icon::Check } else { Icon::List },
                        manual_lbl,
                        false,
                    )
                    .clicked()
                    {
                        chosen_sort = Some(None);
                        ui.memory_mut(|m| m.close_popup());
                    }
                    if crate::ui::components::context_menu_row(
                        ui,
                        p,
                        if cur_key.as_deref() == Some("__title") { Icon::Check } else { Icon::Document },
                        s.table_col_name,
                        false,
                    )
                    .clicked()
                    {
                        chosen_sort = Some(Some("__title".to_string()));
                        ui.memory_mut(|m| m.close_popup());
                    }
                    for (cid, cname) in cols_list {
                        let sel = cur_key.as_deref() == Some(cid.as_str());
                        if crate::ui::components::context_menu_row(
                            ui,
                            p,
                            if sel { Icon::Check } else { Icon::List },
                            &cname,
                            false,
                        )
                        .clicked()
                        {
                            chosen_sort = Some(Some(cid));
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                },
            );
            if let Some(pick) = chosen_sort {
                state.sort = pick.map(|column_key| CollectionSort {
                    column_key,
                    dir: CollectionSortDir::Asc,
                });
            }
        }
    };

    if !is_mobile && ui.available_width() >= 760.0 {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            render_left_toolbar(ui, state);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                render_right_toolbar(ui, state, actions);
            });
        });
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            render_left_toolbar(ui, state);
            render_right_toolbar(ui, state, actions);
        });
    }
    ui.add_space(4.0);

    if !state.doc.saved_views.is_empty() {
        ui.add_space(6.0);
        let views_snap = state.doc.saved_views.clone();
        let mut remove_view_id: Option<String> = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
            ui.label(
                egui::RichText::new(s.col_saved_views)
                    .font(font_medium(11.5))
                    .color(p.text_muted),
            );
            for sv in &views_snap {
                let is_cur = state.active_saved_view_id.as_deref() == Some(sv.id.as_str());
                let (clicked, del_clicked) = saved_view_chip(ui, p, &sv.name, is_cur);
                if clicked {
                    state.filter_query = sv.query.clone();
                    state.date_filter = sv.date_preset;
                    state.tag_filter = sv.tag.clone();
                    state.select_filter = sv
                        .select_col_id
                        .clone()
                        .zip(sv.select_opt_id.clone());
                    state.active_view = sv.view_kind;
                    state.active_saved_view_id = Some(sv.id.clone());
                }
                if del_clicked {
                    remove_view_id = Some(sv.id.clone());
                }
            }
        });
        if let Some(rm_id) = remove_view_id {
            state.doc.saved_views.retain(|v| v.id != rm_id);
            if state.active_saved_view_id.as_deref() == Some(rm_id.as_str()) {
                state.active_saved_view_id = None;
            }
            actions.push(UiAction::MarkCollectionDirty);
        }
    }

    if state.smart_filter_open {
        ui.add_space(6.0);
        egui::Frame::none()
            .fill(p.bg_sidebar)
            .stroke(Stroke::new(1.0_f32, p.border_hairline))
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(10.0, 8.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;

                    // Date preset row
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(5.0, 4.0);
                        for (preset, lbl) in [
                            (DateFilterPreset::All, s.col_date_all),
                            (DateFilterPreset::Overdue, s.col_date_overdue),
                            (DateFilterPreset::Today, s.col_date_today),
                            (DateFilterPreset::Next7Days, s.col_date_week),
                            (DateFilterPreset::NoDate, s.col_date_none),
                        ] {
                            let active = state.date_filter == preset;
                            if smart_filter_pill(ui, p, None, lbl, active).clicked() {
                                state.date_filter = preset;
                                state.active_saved_view_id = None;
                            }
                        }
                    });

                    // Tags & optional non-group select column filter row
                    let mut all_tags: BTreeSet<String> = BTreeSet::new();
                    for rec in &state.doc.records {
                        for t in &rec.tags {
                            if !t.trim().is_empty() {
                                all_tags.insert(t.clone());
                            }
                        }
                    }
                    let group_cid = state
                        .doc
                        .effective_group_column_id()
                        .unwrap_or("")
                        .to_string();
                    let extra_select = state
                        .doc
                        .columns
                        .iter()
                        .find(|c| {
                            c.col_type == ColumnType::Select
                                && c.id != group_cid
                                && !c.options.is_empty()
                        })
                        .cloned();

                    if !all_tags.is_empty() || extra_select.is_some() {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(5.0, 4.0);
                            for tag in &all_tags {
                                let is_sel = state
                                    .tag_filter
                                    .as_deref()
                                    .is_some_and(|t| t.eq_ignore_ascii_case(tag));
                                let (resp, _) = tag_chip(ui, p, tag, is_sel);
                                if resp.clicked() {
                                    state.tag_filter = if is_sel { None } else { Some(tag.clone()) };
                                    state.active_saved_view_id = None;
                                }
                            }
                            if let Some(ref sel_col) = extra_select {
                                ui.label(
                                    egui::RichText::new(format!("{}:", sel_col.name))
                                        .font(font_medium(11.0))
                                        .color(p.text_muted),
                                );
                                for opt in &sel_col.options {
                                    let active = state.select_filter.as_ref().is_some_and(
                                        |(cid, oid)| cid == &sel_col.id && oid == &opt.id,
                                    );
                                    if smart_filter_pill(ui, p, None, &opt.label, active).clicked() {
                                        state.select_filter = if active {
                                            None
                                        } else {
                                            Some((sel_col.id.clone(), opt.id.clone()))
                                        };
                                        state.active_saved_view_id = None;
                                    }
                                }
                            }
                        });
                    }

                    // Save as SavedView
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        styled_text_input(
                            ui,
                            p,
                            &mut state.new_view_name,
                            s.col_save_view_hint,
                            false,
                            if is_mobile { 150.0 } else { 200.0 },
                        );
                        if primary_button(ui, p, Some(Icon::Plus), s.col_save_view_btn, false)
                            .clicked()
                            && !state.new_view_name.trim().is_empty()
                        {
                            let ts = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0);
                            let vid = format!("sv_{}_{}", ts, state.doc.saved_views.len() + 1);
                            state.doc.saved_views.push(SavedCollectionView {
                                id: vid.clone(),
                                name: state.new_view_name.trim().to_string(),
                                view_kind: state.active_view,
                                query: state.filter_query.clone(),
                                date_preset: state.date_filter,
                                tag: state.tag_filter.clone(),
                                select_col_id: state
                                    .select_filter
                                    .as_ref()
                                    .map(|(c, _)| c.clone()),
                                select_opt_id: state
                                    .select_filter
                                    .as_ref()
                                    .map(|(_, o)| o.clone()),
                            });
                            state.active_saved_view_id = Some(vid);
                            state.new_view_name.clear();
                            actions.push(UiAction::MarkCollectionDirty);
                        }
                    });
                });
            });
    }
}

fn smart_filter_pill(
    ui: &mut Ui,
    p: &ThemePalette,
    icon: Option<Icon>,
    label: &str,
    active: bool,
) -> egui::Response {
    let icon_pad = if icon.is_some() { 18.0 } else { 0.0 };
    let text_col = if active { p.accent_text } else { p.text_secondary };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(11.5), text_col);
    let w = (g.size().x + icon_pad + 16.0).max(32.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 25.0), Sense::click());
    let fill = if active {
        p.accent_soft
    } else if resp.hovered() {
        p.bg_hover
    } else {
        p.bg_card
    };
    let border = if active { p.accent } else { p.border_hairline };
    ui.painter().rect_filled(rect, Rounding::same(6.0), fill);
    ui.painter()
        .rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, border));
    if let Some(ic) = icon {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(12.0, 12.0)),
            ic,
            text_col,
        );
    }
    ui.painter().galley(
        pos2(
            rect.left() + 8.0 + icon_pad,
            rect.center().y - g.size().y * 0.5,
        ),
        g,
        text_col,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn saved_view_chip(ui: &mut Ui, p: &ThemePalette, name: &str, active: bool) -> (bool, bool) {
    let fg = if active { p.accent_text } else { p.text_primary };
    let g = ui
        .painter()
        .layout_no_wrap(name.to_string(), font_medium(11.5), fg);
    let w = g.size().x + 30.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    let del_rect = Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(14.0, 14.0));
    let ptr = ui.ctx().pointer_interact_pos();
    let del_hov = resp.hovered() && ptr.is_some_and(|pt| del_rect.contains(pt));
    let fill = if active {
        p.accent_soft
    } else if resp.hovered() {
        p.bg_hover
    } else {
        p.bg_card
    };
    ui.painter().rect_filled(rect, Rounding::same(6.0), fill);
    ui.painter().rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(1.0_f32, if active { p.accent } else { p.border_hairline }),
    );
    ui.painter()
        .galley(pos2(rect.left() + 8.0, rect.center().y - g.size().y * 0.5), g, fg);
    draw_icon(
        ui.painter(),
        Rect::from_center_size(del_rect.center(), vec2(10.0, 10.0)),
        Icon::Close,
        if del_hov { p.danger } else { p.text_muted },
    );
    let clicked = resp.clicked();
    resp.on_hover_cursor(CursorIcon::PointingHand);
    if clicked && del_hov {
        (false, true)
    } else if clicked {
        (true, false)
    } else {
        (false, false)
    }
}

#[allow(clippy::too_many_arguments)]
fn render_custom_table(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    layout: LayoutClass,
    today: SimpleDate,
    state: &mut CollectionEditorState,
    all_notes: &[NoteMeta],
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let q_lower = state.filter_query.trim().to_lowercase();
    let cols_snapshot = state.doc.columns.clone();
    let sel_filter = state
        .select_filter
        .as_ref()
        .map(|(c, o)| (c.as_str(), o.as_str()));

    let mut visible_indices: Vec<usize> = state
        .doc
        .records
        .iter()
        .enumerate()
        .filter(|(_, rec)| {
            record_matches_smart_filter(
                rec,
                &cols_snapshot,
                &q_lower,
                state.date_filter,
                state.tag_filter.as_deref(),
                sel_filter,
                today,
            )
        })
        .map(|(i, _)| i)
        .collect();

    if let Some(ref sort) = state.sort {
        let dir = match sort.dir {
            CollectionSortDir::Asc => krono_core::collection::SortDirection::Asc,
            CollectionSortDir::Desc => krono_core::collection::SortDirection::Desc,
        };
        visible_indices.sort_by(|&ai, &bi| {
            krono_core::collection::compare_records(
                &state.doc.records[ai],
                &state.doc.records[bi],
                &sort.column_key,
                dir,
                &cols_snapshot,
            )
        });
    }

    if layout.is_mobile() || ui.available_width() < 560.0 {
        render_mobile_custom_table(
            ui,
            p,
            lang,
            &cols_snapshot,
            &visible_indices,
            state,
            all_notes,
            actions,
        );
        return;
    }

    let avail_h = (ui.available_height() - 12.0).max(200.0);
    let title_w = state.title_col_width.clamp(120.0, 560.0);
    let ops_w = 64.0_f32;
    let total_w: f32 =
        title_w + ops_w + cols_snapshot.iter().map(|c| c.width.clamp(90.0, 560.0)).sum::<f32>();

    let mut toggle_sort_col: Option<String> = None;
    let mut delete_rec_id: Option<String> = None;
    let mut open_rec_id: Option<String> = None;
    let mut dirty = false;
    let mut title_width_delta: f32 = 0.0;
    let mut col_width_delta: Option<(usize, f32)> = None;
    let mut resize_stopped = false;

    egui::ScrollArea::horizontal()
        .id_salt("custom_table_h_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let table_w = total_w.max(ui.available_width());
            egui::Frame::none()
                .fill(p.bg_sidebar)
                .stroke(Stroke::new(1.0_f32, p.border_hairline))
                .rounding(Rounding::same(10.0))
                .inner_margin(Margin::symmetric(0.0, 4.0))
                .show(ui, |ui| {
                    ui.set_width(table_w);

                    // Header row
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if render_table_header_cell(
                            ui,
                            p,
                            title_w - 6.0,
                            Icon::Document,
                            s.table_col_name,
                            state.sort.as_ref().filter(|sr| sr.column_key == "__title"),
                            false,
                        )
                        .0
                        {
                            toggle_sort_col = Some("__title".to_string());
                        }
                        let (tdx, tstop) = render_col_resize_handle(ui, p, "ctbl_resz_title", 34.0);
                        title_width_delta += tdx;
                        resize_stopped |= tstop;

                        for (ci, col) in cols_snapshot.iter().enumerate() {
                            let cw = col.width.clamp(90.0, 560.0);
                            let (sort_hit, cfg_hit) = render_table_header_cell(
                                ui,
                                p,
                                cw - 6.0,
                                column_type_icon(col.col_type),
                                &col.name,
                                state.sort.as_ref().filter(|sr| sr.column_key == col.id),
                                true,
                            );
                            if sort_hit {
                                toggle_sort_col = Some(col.id.clone());
                            }
                            if cfg_hit {
                                state.editing_column_id = Some(col.id.clone());
                            }
                            let (cdx, cstop) = render_col_resize_handle(
                                ui,
                                p,
                                ("ctbl_resz", &col.id),
                                34.0,
                            );
                            if cdx.abs() > 0.0 {
                                col_width_delta = Some((ci, cdx));
                            }
                            resize_stopped |= cstop;
                        }
                        let (ar, aresp) = ui.allocate_exact_size(vec2(ops_w, 34.0), Sense::click());
                        let ig = Rect::from_center_size(ar.center(), vec2(14.0, 14.0));
                        draw_icon(ui.painter(), ig, Icon::Plus, p.text_muted);
                        if aresp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(s.col_add_field)
                            .clicked()
                        {
                            state.adding_field_open = true;
                            state.new_field_name.clear();
                        }
                    });
                    hairline_h(ui, p);

                    // Body rows
                    egui::ScrollArea::vertical()
                        .id_salt("custom_table_v_scroll")
                        .max_height((avail_h - 86.0).max(140.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(table_w);
                            for &idx in &visible_indices {
                                let rec = &mut state.doc.records[idx];
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 0.0;
                                    // Title + detail modal trigger
                                    ui.allocate_ui_with_layout(
                                        vec2(title_w, 36.0),
                                        Layout::left_to_right(Align::Center),
                                        |ui| {
                                            ui.add_space(10.0);
                                            let tr = ui.add(
                                                egui::TextEdit::singleline(&mut rec.title)
                                                    .frame(false)
                                                    .hint_text(s.col_row_title_hint)
                                                    .font(font_medium(13.0))
                                                    .text_color(p.text_primary)
                                                    .desired_width(title_w - 44.0),
                                            );
                                            if tr.changed() {
                                                dirty = true;
                                            }
                                            if icon_button(ui, p, Icon::OutLink, s.open_btn, false)
                                                .clicked()
                                            {
                                                open_rec_id = Some(rec.id.clone());
                                            }
                                        },
                                    );

                                    for col in &cols_snapshot {
                                        let cw = col.width.clamp(90.0, 560.0);
                                        ui.allocate_ui_with_layout(
                                            vec2(cw, 36.0),
                                            Layout::left_to_right(Align::Center),
                                            |ui| {
                                                ui.add_space(6.0);
                                                if render_inline_field_cell(
                                                    ui,
                                                    p,
                                                    lang,
                                                    "tbl",
                                                    col,
                                                    rec,
                                                    cw - 12.0,
                                                    all_notes,
                                                    &mut state.modal_note_search,
                                                    actions,
                                                ) {
                                                    dirty = true;
                                                }
                                            },
                                        );
                                    }

                                    ui.allocate_ui_with_layout(
                                        vec2(ops_w, 36.0),
                                        Layout::centered_and_justified(egui::Direction::LeftToRight),
                                        |ui| {
                                            if icon_button(
                                                ui,
                                                p,
                                                Icon::Trash,
                                                s.delete_btn,
                                                false,
                                            )
                                            .clicked()
                                            {
                                                delete_rec_id = Some(rec.id.clone());
                                            }
                                        },
                                    );
                                });
                                hairline_h(ui, p);
                            }

                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.add_space(10.0);
                                if secondary_button(ui, p, Some(Icon::Plus), s.col_new_row).clicked()
                                {
                                    let row_num = state.doc.records.len() + 1;
                                    let title = if lang == Lang::Ru {
                                        format!("Запись {row_num}")
                                    } else {
                                        format!("Item {row_num}")
                                    };
                                    state.doc.add_record(&title, None, chrono::Utc::now().timestamp());
                                    dirty = true;
                                }
                            });
                            ui.add_space(6.0);
                        });
                });
        });

    if let Some(key) = toggle_sort_col {
        state.sort = match &state.sort {
            Some(cur) if cur.column_key == key && cur.dir == CollectionSortDir::Asc => {
                Some(CollectionSort {
                    column_key: key,
                    dir: CollectionSortDir::Desc,
                })
            }
            Some(cur) if cur.column_key == key => None,
            _ => Some(CollectionSort {
                column_key: key,
                dir: CollectionSortDir::Asc,
            }),
        };
    }
    if let Some(rid) = delete_rec_id {
        state.doc.remove_record(&rid);
        dirty = true;
    }
    if let Some(rid) = open_rec_id {
        state.modal_new_tag.clear();
        state.editing_record_id = Some(rid);
    }
    if title_width_delta.abs() > 0.0 {
        state.title_col_width = (state.title_col_width + title_width_delta).clamp(120.0, 560.0);
    }
    if let Some((ci, dx)) = col_width_delta {
        if let Some(col) = state.doc.columns.get_mut(ci) {
            col.width = (col.width + dx).clamp(90.0, 560.0);
        }
    }
    if resize_stopped {
        dirty = true;
    }
    if dirty {
        actions.push(UiAction::MarkCollectionDirty);
    }
}

fn render_col_resize_handle(
    ui: &mut Ui,
    p: &ThemePalette,
    id_salt: impl Hash,
    height: f32,
) -> (f32, bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(6.0, height), Sense::hover());
    let resp = ui.interact(rect, ui.make_persistent_id(id_salt), Sense::drag());
    let active = resp.hovered() || resp.dragged();
    let line_col = if active { p.accent } else { p.border_hairline };
    ui.painter().line_segment(
        [
            pos2(rect.center().x, rect.top() + 5.0),
            pos2(rect.center().x, rect.bottom() - 5.0),
        ],
        Stroke::new(if active { 2.0_f32 } else { 1.0_f32 }, line_col),
    );
    if active {
        resp.clone().on_hover_cursor(CursorIcon::ResizeHorizontal);
    }
    let dx = if resp.dragged() { resp.drag_delta().x } else { 0.0 };
    (dx, resp.drag_stopped())
}

fn render_table_header_cell(
    ui: &mut Ui,
    p: &ThemePalette,
    width: f32,
    icon: Icon,
    name: &str,
    active_sort: Option<&CollectionSort>,
    show_cfg: bool,
) -> (bool, bool) {
    let mut sort_clicked = false;
    let mut cfg_clicked = false;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hdr"), resp.hovered(), 0.10);
    if hov > 0.01 {
        ui.painter()
            .rect_filled(rect, Rounding::same(4.0), with_alpha(p.bg_hover, hov));
    }

    draw_icon(
        ui.painter(),
        Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(12.5, 12.5)),
        icon,
        p.text_muted,
    );
    let label = match active_sort {
        Some(sr) if sr.dir == CollectionSortDir::Asc => format!("{name} ^"),
        Some(_) => format!("{name} v"),
        None => name.to_string(),
    };
    let max_lbl_w = (width - if show_cfg { 52.0 } else { 32.0 }).max(24.0);
    let mut job = egui::text::LayoutJob::single_section(
        label,
        egui::TextFormat {
            font_id: font_medium(12.0),
            color: p.text_secondary,
            ..Default::default()
        },
    );
    job.wrap = egui::text::TextWrapping {
        max_width: max_lbl_w,
        max_rows: 1,
        break_anywhere: false,
        overflow_character: Some('…'),
    };
    let g = ui.fonts(|f| f.layout_job(job));
    ui.painter().galley(
        pos2(rect.left() + 28.0, rect.center().y - g.size().y * 0.5),
        g,
        p.text_secondary,
    );

    if show_cfg {
        let cfg_r = Rect::from_center_size(pos2(rect.right() - 14.0, rect.center().y), vec2(20.0, 20.0));
        let cfg_resp = ui.interact(cfg_r, resp.id.with("cfg"), Sense::click());
        draw_icon(
            ui.painter(),
            Rect::from_center_size(cfg_r.center(), vec2(11.5, 11.5)),
            Icon::Settings,
            if cfg_resp.hovered() {
                p.text_primary
            } else {
                p.text_muted
            },
        );
        if cfg_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
            cfg_clicked = true;
        }
    }
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        sort_clicked = true;
    }
    (sort_clicked, cfg_clicked)
}

#[allow(clippy::too_many_arguments)]
fn render_mobile_custom_table(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    cols: &[ColumnDef],
    indices: &[usize],
    state: &mut CollectionEditorState,
    all_notes: &[NoteMeta],
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let mut dirty = false;
    let mut open_id: Option<String> = None;

    egui::ScrollArea::vertical()
        .id_salt("mob_ctable_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for &idx in indices {
                let rec = &mut state.doc.records[idx];
                egui::Frame::none()
                    .fill(p.bg_card)
                    .stroke(Stroke::new(1.0_f32, p.border_hairline))
                    .rounding(Rounding::same(9.0))
                    .inner_margin(Margin::same(12.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&rec.title)
                                    .font(font_medium(14.0))
                                    .color(p.text_primary),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if icon_button(ui, p, Icon::OutLink, s.open_btn, false).clicked() {
                                    open_id = Some(rec.id.clone());
                                }
                            });
                        });
                        if !cols.is_empty() {
                            ui.add_space(6.0);
                        }
                        for col in cols.iter().take(4) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("{}:", col.name))
                                        .font(font_body(12.0))
                                        .color(p.text_muted),
                                );
                                if render_inline_field_cell(
                                    ui,
                                    p,
                                    lang,
                                    "mob",
                                    col,
                                    rec,
                                    170.0,
                                    all_notes,
                                    &mut state.modal_note_search,
                                    actions,
                                ) {
                                    dirty = true;
                                }
                            });
                        }
                    });
                ui.add_space(8.0);
            }
            if secondary_button(ui, p, Some(Icon::Plus), s.col_new_row).clicked() {
                let title = if lang == Lang::Ru {
                    format!("Запись {}", state.doc.records.len() + 1)
                } else {
                    format!("Item {}", state.doc.records.len() + 1)
                };
                state.doc.add_record(&title, None, chrono::Utc::now().timestamp());
                dirty = true;
            }
        });

    if let Some(id) = open_id {
        state.modal_new_tag.clear();
        state.editing_record_id = Some(id);
    }
    if dirty {
        actions.push(UiAction::MarkCollectionDirty);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_inline_field_cell(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    scope: &str,
    col: &ColumnDef,
    rec: &mut RecordItem,
    max_w: f32,
    all_notes: &[NoteMeta],
    note_filter: &mut String,
    actions: &mut Vec<UiAction>,
) -> bool {
    let cell_id = ui.make_persistent_id((scope, &rec.id, &col.id));
    let mut changed = false;
    match col.col_type {
        ColumnType::Text | ColumnType::Date => {
            let mut val = rec.values.get(&col.id).map(FieldValue::as_text).unwrap_or_default();
            let hint = if col.col_type == ColumnType::Date {
                "YYYY-MM-DD"
            } else {
                "—"
            };
            let font = if col.col_type == ColumnType::Date {
                font_mono(12.0)
            } else {
                font_body(12.5)
            };
            let r = ui.add(
                egui::TextEdit::singleline(&mut val)
                    .id(cell_id.with("txt"))
                    .frame(false)
                    .hint_text(hint)
                    .font(font)
                    .text_color(p.text_primary)
                    .desired_width(max_w),
            );
            if r.changed() {
                if val.is_empty() {
                    rec.values.remove(&col.id);
                } else {
                    rec.values.insert(col.id.clone(), FieldValue::Text(val));
                }
                changed = true;
            }
        }
        ColumnType::Number => {
            let mut txt = rec.values.get(&col.id).map(FieldValue::as_text).unwrap_or_default();
            let r = ui.add(
                egui::TextEdit::singleline(&mut txt)
                    .id(cell_id.with("num"))
                    .frame(false)
                    .hint_text("0")
                    .font(font_mono(12.5))
                    .text_color(p.text_primary)
                    .desired_width(max_w),
            );
            if r.changed() {
                let clean = txt.trim();
                if clean.is_empty() {
                    rec.values.remove(&col.id);
                } else if let Ok(n) = clean.replace(',', ".").parse::<f64>() {
                    rec.values.insert(col.id.clone(), FieldValue::Number(n));
                } else {
                    rec.values
                        .insert(col.id.clone(), FieldValue::Text(clean.to_string()));
                }
                changed = true;
            }
        }
        ColumnType::Checkbox => {
            let mut val = rec
                .values
                .get(&col.id)
                .map(FieldValue::as_bool)
                .unwrap_or(false);
            if render_custom_checkbox(ui, p, cell_id.with("chk"), &mut val) {
                rec.values.insert(col.id.clone(), FieldValue::Bool(val));
                changed = true;
            }
        }
        ColumnType::Select => {
            let cur = rec.values.get(&col.id).map(FieldValue::as_text).unwrap_or_default();
            if let Some(next_id) = render_select_pill_picker(ui, p, lang, cell_id.with("sel"), col, &cur) {
                rec.values.insert(col.id.clone(), FieldValue::Text(next_id));
                changed = true;
            }
        }
        ColumnType::NoteLink => {
            let cur = rec.values.get(&col.id).map(FieldValue::as_text).unwrap_or_default();
            if let Some(next_val) = render_note_link_picker(
                ui,
                p,
                lang,
                cell_id.with("nl"),
                &cur,
                max_w,
                all_notes,
                note_filter,
                actions,
            ) {
                if next_val.is_empty() {
                    rec.values.remove(&col.id);
                } else {
                    rec.values
                        .insert(col.id.clone(), FieldValue::Text(next_val));
                }
                changed = true;
            }
        }
    }
    changed
}

pub(crate) fn render_select_pill_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    id_salt: impl Hash,
    col: &ColumnDef,
    current_opt_id: &str,
) -> Option<String> {
    let opt = col.options.iter().find(|o| o.id == current_opt_id);
    let (pill_bg, dot_col, text_col) = opt
        .map(|o| p.token_colors(o.color))
        .unwrap_or((p.bg_input, p.text_muted, p.text_muted));
    let label = opt
        .map(|o| o.label.as_str())
        .unwrap_or(if lang == Lang::Ru { "Выбрать…" } else { "Select…" });

    let g = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(11.5), text_col);
    let w = (g.size().x + 32.0).clamp(64.0, 148.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 23.0), Sense::click());
    let hov = anim_bool_eased(ui.ctx(), resp.id.with("hov"), resp.hovered(), 0.10);
    ui.painter().rect_filled(
        rect,
        Rounding::same(11.0),
        lerp_color(pill_bg, p.bg_hover, hov * 0.35),
    );
    ui.painter()
        .circle_filled(pos2(rect.left() + 10.0, rect.center().y), 3.0, dot_col);
    ui.painter().galley(
        pos2(rect.left() + 17.0, rect.center().y - g.size().y * 0.5),
        g,
        text_col,
    );
    draw_icon(
        ui.painter(),
        Rect::from_center_size(pos2(rect.right() - 10.0, rect.center().y), vec2(10.0, 10.0)),
        Icon::ChevronDown,
        text_col,
    );

    let pop_id = ui.make_persistent_id(("sel_pop", id_salt));
    let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pop_id));
    }

    let mut chosen = None;
    egui::popup_below_widget(
        ui,
        pop_id,
        &resp,
        PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(148.0);
            for o in &col.options {
                let (obg, odot, otxt) = p.token_colors(o.color);
                let is_sel = o.id == current_opt_id;
                let (r, b) = ui.allocate_exact_size(vec2(ui.available_width().max(136.0), 26.0), Sense::click());
                if b.hovered() || is_sel {
                    ui.painter()
                        .rect_filled(r, Rounding::same(5.0), if is_sel { obg } else { p.bg_hover });
                }
                ui.painter()
                    .circle_filled(pos2(r.left() + 11.0, r.center().y), 3.2, odot);
                let og = ui.painter().layout_no_wrap(
                    o.label.clone(),
                    font_medium(12.0),
                    if is_sel { otxt } else { p.text_primary },
                );
                ui.painter().galley(
                    pos2(r.left() + 21.0, r.center().y - og.size().y * 0.5),
                    og,
                    p.text_primary,
                );
                if b.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                    chosen = Some(o.id.clone());
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        },
    );
    chosen
}

pub(crate) fn render_custom_checkbox(
    ui: &mut Ui,
    p: &ThemePalette,
    id_salt: impl Hash,
    checked: &mut bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
    let id = ui.make_persistent_id(id_salt);
    let raw = ui.ctx().animate_bool_with_time(id, *checked, 0.14);
    let t = ease_in_out_cubic(raw);

    let bg = lerp_color(p.bg_input, p.accent, t);
    let stroke = lerp_color(p.border_strong, p.accent, t);
    ui.painter().rect_filled(rect, Rounding::same(4.5), bg);
    ui.painter()
        .rect_stroke(rect, Rounding::same(4.5), Stroke::new(1.1_f32, stroke));
    if t > 0.25 {
        draw_icon(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(11.5, 11.5)),
            Icon::Check,
            with_alpha(p.accent_on_fill, t),
        );
    }
    if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
        *checked = !*checked;
        true
    } else {
        false
    }
}

pub(crate) fn render_color_token_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    current: &mut ColorToken,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for tok in ColorToken::ALL {
            let (_, dot, _) = p.token_colors(tok);
            let sel = *current == tok;
            let (rect, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            if sel {
                ui.painter()
                    .circle_stroke(rect.center(), 8.5, Stroke::new(1.6_f32, p.text_primary));
            }
            ui.painter().circle_filled(rect.center(), 6.0, dot);
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && !sel {
                *current = tok;
                changed = true;
            }
        }
    });
    changed
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_note_link_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    id_salt: impl Hash,
    current_val: &str,
    max_w: f32,
    all_notes: &[NoteMeta],
    filter_draft: &mut String,
    actions: &mut Vec<UiAction>,
) -> Option<String> {
    let s = strings(lang);
    let target = strip_link_target(current_val);
    let has_val = !target.is_empty();
    let pop_id = ui.make_persistent_id(("nl_pop", id_salt));
    let mut result: Option<String> = None;

    let mut trigger_rect_resp: Option<egui::Response> = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let label = if has_val { target } else { s.col_record_pick_note };
        let fg = if has_val { p.accent_text } else { p.text_muted };
        let bg = if has_val { p.accent_soft } else { p.bg_input };
        let g = ui.painter().layout_no_wrap(label.to_string(), font_medium(11.5), fg);
        let pill_w = (g.size().x + 26.0).min((max_w - if has_val { 24.0 } else { 0.0 }).max(64.0));
        let (r, resp) = ui.allocate_exact_size(vec2(pill_w, 23.0), Sense::click());
        ui.painter().rect_filled(r, Rounding::same(6.0), bg);
        draw_icon(
            ui.painter(),
            Rect::from_center_size(pos2(r.left() + 10.0, r.center().y), vec2(11.5, 11.5)),
            Icon::Link,
            fg,
        );
        ui.painter().galley(
            pos2(r.left() + 20.0, r.center().y - g.size().y * 0.5),
            g,
            fg,
        );
        let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        if resp.clicked() {
            ui.memory_mut(|m| m.toggle_popup(pop_id));
        }
        trigger_rect_resp = Some(resp);

        if has_val
            && icon_button(ui, p, Icon::OutLink, s.col_record_open_note, false).clicked()
        {
            actions.push(UiAction::OpenOrCreateWikilink(target.to_string()));
        }
    });

    if let Some(ref anchor) = trigger_rect_resp {
        egui::popup_below_widget(
            ui,
            pop_id,
            anchor,
            PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                ui.set_min_width(220.0);
                ui.set_max_width(250.0);
                styled_text_input(
                    ui,
                    p,
                    filter_draft,
                    s.search_placeholder,
                    false,
                    f32::INFINITY,
                );
                ui.add_space(6.0);
                let fq = filter_draft.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt(pop_id.with("sc"))
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for n in all_notes
                            .iter()
                            .filter(|n| fq.is_empty() || n.title.to_lowercase().contains(&fq))
                            .take(24)
                        {
                            if crate::ui::components::context_menu_row(
                                ui,
                                p,
                                Icon::Document,
                                &n.title,
                                false,
                            )
                            .clicked()
                            {
                                result = Some(n.title.clone());
                                filter_draft.clear();
                                ui.memory_mut(|m| m.close_popup());
                            }
                        }
                    });
                if has_val {
                    ui.add_space(4.0);
                    hairline_h(ui, p);
                    ui.add_space(4.0);
                    if crate::ui::components::context_menu_row(
                        ui,
                        p,
                        Icon::Close,
                        s.col_record_clear_note,
                        true,
                    )
                    .clicked()
                    {
                        result = Some(String::new());
                        ui.memory_mut(|m| m.close_popup());
                    }
                }
            },
        );
    }

    result
}

pub(crate) fn collect_available_tags(doc: &CollectionDoc, all_notes: &[NoteMeta]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for rec in &doc.records {
        for t in &rec.tags {
            let norm = normalize_tag_input(t);
            if !norm.is_empty() && seen.insert(norm) {
                out.push(t.trim().to_string());
            }
        }
    }
    for note in all_notes {
        for t in &note.tags {
            let norm = normalize_tag_input(t);
            if !norm.is_empty() && seen.insert(norm) {
                out.push(t.trim().to_string());
            }
        }
    }
    out.sort_by_key(|a| a.to_lowercase());
    out
}

fn render_record_detail_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    all_notes: &[NoteMeta],
    available_tags: &[String],
    actions: &mut Vec<UiAction>,
) {
    let Some(rec_id) = state.editing_record_id.clone() else {
        return;
    };
    let s = strings(lang);

    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    let mut close_modal = false;

    egui::Area::new(egui::Id::new("c_rec_modal_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha as u8),
            );
            if r.clicked() {
                close_modal = true;
            }
        });

    let cols_snapshot = state.doc.columns.clone();
    let mut dirty = false;
    let mut delete_me = false;

    let top_y = ((screen.height() - 560.0) * 0.5).clamp(20.0, 96.0);

    egui::Area::new(egui::Id::new("c_rec_detail_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, top_y))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(20.0))
                .show(ui, |ui| {
                    let modal_w = 560.0_f32.min((screen.width() - 36.0).max(260.0));
                    let scroll_max_h = (screen.height() - top_y - 24.0 - 156.0).max(140.0);
                    ui.set_width(modal_w);

                    let Some(rec) = state.doc.records.iter_mut().find(|r| r.id == rec_id) else {
                        close_modal = true;
                        return;
                    };

                    ui.horizontal(|ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut rec.title)
                                .frame(false)
                                .font(font_semibold(19.0))
                                .text_color(p.text_primary)
                                .hint_text(s.col_card_title_hint)
                                .desired_width((modal_w - 42.0).max(140.0)),
                        );
                        if r.changed() {
                            dirty = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                                close_modal = true;
                            }
                            if icon_button(ui, p, Icon::FilePlus, s.tasks_convert_to_note, false)
                                .clicked()
                            {
                                actions.push(UiAction::ConvertRecordToNote {
                                    record_id: rec_id.clone(),
                                });
                            }
                        });
                    });
                    ui.add_space(10.0);
                    hairline_h(ui, p);
                    ui.add_space(12.0);

                    egui::ScrollArea::vertical()
                        .id_salt("c_rec_modal_scroll")
                        .auto_shrink([false, true])
                        .max_height(scroll_max_h)
                        .show(ui, |ui| {

                    // Properties grid
                    ui.label(
                        egui::RichText::new(s.col_record_props)
                            .font(font_medium(12.0))
                            .color(p.text_muted),
                    );
                    ui.add_space(6.0);
                    for col in &cols_snapshot {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                vec2(120.0, 26.0),
                                Layout::left_to_right(Align::Center),
                                |ui| {
                                    let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                    draw_icon(ui.painter(), ir, column_type_icon(col.col_type), p.text_muted);
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(&col.name)
                                            .font(font_body(12.5))
                                            .color(p.text_secondary),
                                    );
                                },
                            );
                            if render_inline_field_cell(
                                ui,
                                p,
                                lang,
                                "mdl",
                                col,
                                rec,
                                (modal_w - 140.0).max(120.0),
                                all_notes,
                                &mut state.modal_note_search,
                                actions,
                            ) {
                                dirty = true;
                            }
                        });
                        ui.add_space(4.0);
                    }

                    ui.add_space(10.0);
                    // Tags row
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                        let mut remove_tag: Option<usize> = None;
                        for (ti, tag) in rec.tags.iter().enumerate() {
                            let (_, rm) = tag_chip(ui, p, tag, true);
                            if rm {
                                remove_tag = Some(ti);
                            }
                        }
                        if let Some(ti) = remove_tag {
                            rec.tags.remove(ti);
                            dirty = true;
                        }
                        if let Some(new_tag) = tag_autocomplete_input(
                            ui,
                            p,
                            lang,
                            ("c_rec_tag_inp", &rec_id),
                            &mut state.modal_new_tag,
                            s.add_tag_hint,
                            128.0,
                            available_tags,
                            &rec.tags,
                        ) {
                            let clean = normalize_tag_input(&new_tag);
                            if !clean.is_empty()
                                && !rec.tags.iter().any(|t| normalize_tag_input(t) == clean)
                            {
                                rec.tags.push(clean);
                                dirty = true;
                            }
                            state.modal_new_tag.clear();
                        }
                    });

                    ui.add_space(12.0);
                    // Description
                    ui.label(
                        egui::RichText::new(s.col_record_desc)
                            .font(font_medium(12.0))
                            .color(p.text_muted),
                    );
                    ui.add_space(4.0);
                    egui::Frame::none()
                        .fill(p.bg_input)
                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            let dr = ui.add(
                                egui::TextEdit::multiline(&mut rec.description)
                                    .frame(false)
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY)
                                    .hint_text(s.col_record_desc_hint)
                                    .font(font_body(13.0))
                                    .text_color(p.text_primary),
                            );
                            if dr.changed() {
                                dirty = true;
                            }
                        });

                    ui.add_space(12.0);
                    // Checklist
                    ui.label(
                        egui::RichText::new(s.col_record_checklist)
                            .font(font_medium(12.0))
                            .color(p.text_muted),
                    );
                    ui.add_space(4.0);
                    let mut rm_chk: Option<usize> = None;
                    for (ci, item) in rec.checklist.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if render_custom_checkbox(ui, p, ("mdl_chk", &rec.id, ci), &mut item.done) {
                                dirty = true;
                            }
                            let cr = ui.add(
                                egui::TextEdit::singleline(&mut item.text)
                                    .frame(false)
                                    .font(font_body(13.0))
                                    .text_color(if item.done { p.text_muted } else { p.text_primary })
                                    .desired_width((modal_w - 86.0).max(120.0)),
                            );
                            if cr.changed() {
                                dirty = true;
                            }
                            if icon_button(ui, p, Icon::Close, s.delete_btn, false).clicked() {
                                rm_chk = Some(ci);
                            }
                        });
                    }
                    if let Some(ci) = rm_chk {
                        rec.checklist.remove(ci);
                        dirty = true;
                    }
                    ui.horizontal(|ui| {
                        let ir = styled_text_input(
                            ui,
                            p,
                            &mut state.modal_new_check_item,
                            s.col_record_item_hint,
                            false,
                            (modal_w - 110.0).max(140.0),
                        );
                        let enter = ir.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (secondary_button(ui, p, Some(Icon::Plus), s.col_record_add_item).clicked()
                            || enter)
                            && !state.modal_new_check_item.trim().is_empty()
                        {
                            rec.checklist.push(ChecklistItem {
                                text: state.modal_new_check_item.trim().to_string(),
                                done: false,
                            });
                            state.modal_new_check_item.clear();
                            dirty = true;
                        }
                    });
                    });

                    ui.add_space(14.0);
                    hairline_h(ui, p);
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, p, Some(Icon::Trash), s.col_record_delete).clicked() {
                            delete_me = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if primary_button(ui, p, None, s.apply_btn, false).clicked() {
                                close_modal = true;
                            }
                        });
                    });
                });
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close_modal = true;
    }

    if delete_me {
        state.doc.remove_record(&rec_id);
        state.editing_record_id = None;
        state.modal_new_tag.clear();
        dirty = true;
    } else if close_modal {
        state.editing_record_id = None;
        state.modal_new_tag.clear();
    }
    if dirty {
        actions.push(UiAction::MarkCollectionDirty);
    }
}

fn render_column_schema_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let Some(col_id) = state.editing_column_id.clone() else {
        return;
    };
    let s = strings(lang);
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.editing_column_id = None;
        return;
    }

    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    let mut close_modal = false;
    egui::Area::new(egui::Id::new("c_col_modal_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha as u8),
            );
            if r.clicked() {
                close_modal = true;
            }
        });

    let mut dirty = false;
    let mut move_col_dir: Option<isize> = None;
    let mut delete_col = false;
    let mut move_opt: Option<(String, isize)> = None;
    let mut delete_opt: Option<String> = None;
    let mut add_opt: Option<(String, ColorToken)> = None;

    egui::Area::new(egui::Id::new("c_col_schema_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(20.0))
                .show(ui, |ui| {
            let modal_w = 420.0_f32.min((screen.width() - 40.0).max(260.0));
            ui.set_width(modal_w);

            let Some(col) = state.doc.columns.iter_mut().find(|c| c.id == col_id) else {
                close_modal = true;
                return;
            };

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(s.col_schema_title)
                        .font(font_semibold(16.0))
                        .color(p.text_primary),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if icon_button(ui, p, Icon::Close, s.tooltip_close, false).clicked() {
                        close_modal = true;
                    }
                    if icon_button(ui, p, Icon::ChevronRight, "->", false).clicked() {
                        move_col_dir = Some(1);
                    }
                    if icon_button(ui, p, Icon::ChevronLeft, "<-", false).clicked() {
                        move_col_dir = Some(-1);
                    }
                });
            });
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new(s.col_field_name)
                    .font(font_medium(12.0))
                    .color(p.text_secondary),
            );
            ui.add_space(4.0);
            if styled_text_input(ui, p, &mut col.name, "", false, f32::INFINITY).changed() {
                dirty = true;
            }

            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(s.col_field_type)
                    .font(font_medium(12.0))
                    .color(p.text_secondary),
            );
            ui.add_space(4.0);
            let prev_ct = col.col_type;
            if render_column_type_picker(ui, p, lang, 124.0, &mut col.col_type) {
                if col.col_type != prev_ct
                    && col.col_type == ColumnType::Select
                    && col.options.is_empty()
                {
                    add_opt = Some((
                        if lang == Lang::Ru {
                            "Вариант 1".into()
                        } else {
                            "Option 1".into()
                        },
                        ColorToken::Blue,
                    ));
                }
                dirty = true;
            }

            if col.col_type == ColumnType::Select {
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(s.col_options_title)
                        .font(font_medium(12.0))
                        .color(p.text_secondary),
                );
                ui.add_space(6.0);
                let opt_len = col.options.len();
                for (oi, opt) in col.options.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        let (_, dot, _) = p.token_colors(opt.color);
                        let (dr, dresp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                        ui.painter().circle_filled(dr.center(), 6.0, dot);
                        if dresp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            let next_idx = (ColorToken::ALL
                                .iter()
                                .position(|c| *c == opt.color)
                                .unwrap_or(0)
                                + 1)
                                % ColorToken::ALL.len();
                            opt.color = ColorToken::ALL[next_idx];
                            dirty = true;
                        }
                        if styled_text_input(ui, p, &mut opt.label, "", false, modal_w - 134.0)
                            .changed()
                        {
                            dirty = true;
                        }
                        if oi > 0 && icon_button(ui, p, Icon::ChevronLeft, "<-", false).clicked() {
                            move_opt = Some((opt.id.clone(), -1));
                        }
                        if oi + 1 < opt_len
                            && icon_button(ui, p, Icon::ChevronRight, "->", false).clicked()
                        {
                            move_opt = Some((opt.id.clone(), 1));
                        }
                        if opt_len > 1
                            && icon_button(ui, p, Icon::Trash, s.delete_btn, false).clicked()
                        {
                            delete_opt = Some(opt.id.clone());
                        }
                    });
                    ui.add_space(4.0);
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    styled_text_input(
                        ui,
                        p,
                        &mut state.new_option_label,
                        s.col_option_hint,
                        false,
                        modal_w - 140.0,
                    );
                    if secondary_button(ui, p, Some(Icon::Plus), s.col_add_option).clicked()
                        && !state.new_option_label.trim().is_empty()
                    {
                        add_opt = Some((
                            state.new_option_label.trim().to_string(),
                            state.new_option_color,
                        ));
                        state.new_option_label.clear();
                    }
                });
            }

            ui.add_space(16.0);
            hairline_h(ui, p);
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if secondary_button(ui, p, Some(Icon::Trash), s.col_delete_field).clicked() {
                    delete_col = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, p, None, s.apply_btn, false).clicked() {
                        close_modal = true;
                    }
                });
            });
                });
        });

    if let Some((lbl, ctok)) = add_opt {
        state.doc.add_select_option(&col_id, &lbl, ctok);
        dirty = true;
    }
    if let Some((oid, d)) = move_opt {
        state.doc.move_select_option(&col_id, &oid, d);
        dirty = true;
    }
    if let Some(oid) = delete_opt {
        state.doc.remove_select_option(&col_id, &oid);
        dirty = true;
    }
    if let Some(d) = move_col_dir {
        state.doc.move_column(&col_id, d);
        dirty = true;
    }
    if delete_col {
        state.doc.remove_column(&col_id);
        state.editing_column_id = None;
        dirty = true;
    } else if close_modal {
        state.editing_column_id = None;
    }
    if dirty {
        actions.push(UiAction::MarkCollectionDirty);
    }
}

fn render_add_field_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let gate_id = egui::Id::new("col_add_field_foc_gate");
    if !state.adding_field_open {
        reset_focus_gate(ctx, gate_id);
        return;
    }
    let s = strings(lang);
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.adding_field_open = false;
        return;
    }

    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135.0 } else { 75.0 };
    egui::Area::new(egui::Id::new("c_add_field_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter().rect_filled(
                screen,
                Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha as u8),
            );
            if r.clicked() {
                state.adding_field_open = false;
            }
        });

    egui::Area::new(egui::Id::new("c_add_field_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(20.0))
                .show(ui, |ui| {
            let modal_w = 380.0_f32.min((screen.width() - 40.0).max(240.0));
            ui.set_width(modal_w);
            ui.label(
                egui::RichText::new(s.col_add_field)
                    .font(font_semibold(16.0))
                    .color(p.text_primary),
            );
            ui.add_space(10.0);
            let r = styled_text_input(
                ui,
                p,
                &mut state.new_field_name,
                s.col_field_name,
                false,
                f32::INFINITY,
            );
            focus_once(ui.ctx(), gate_id, true, r.id);
            ui.add_space(10.0);
            render_column_type_picker(ui, p, lang, 114.0, &mut state.new_field_type);

            ui.add_space(14.0);
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                if (primary_button(ui, p, None, s.create_btn, false).clicked() || enter)
                    && !state.new_field_name.trim().is_empty()
                {
                    let name = state.new_field_name.trim().to_string();
                    state
                        .doc
                        .add_column(&name, state.new_field_type, lang == Lang::Ru);
                    state.new_field_name.clear();
                    state.adding_field_open = false;
                    actions.push(UiAction::MarkCollectionDirty);
                }
                if secondary_button(ui, p, None, s.cancel_btn).clicked() {
                    state.adding_field_open = false;
                }
            });
                });
        });
}

fn render_column_type_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    tile_w: f32,
    current: &mut ColumnType,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for ct in ColumnType::ALL {
            let active = *current == ct;
            let (rect, resp) = ui.allocate_exact_size(vec2(tile_w, 28.0), Sense::click());
            ui.painter().rect_filled(
                rect,
                Rounding::same(6.0),
                if active { p.accent_soft } else { p.bg_input },
            );
            ui.painter().rect_stroke(
                rect,
                Rounding::same(6.0),
                Stroke::new(1.0_f32, if active { p.accent } else { p.border_hairline }),
            );
            let fg = if active { p.accent_text } else { p.text_secondary };
            draw_icon(
                ui.painter(),
                Rect::from_center_size(pos2(rect.left() + 13.5, rect.center().y), vec2(12.0, 12.0)),
                column_type_icon(ct),
                fg,
            );
            let g = ui.painter().layout_no_wrap(
                column_type_label(lang, ct).to_string(),
                font_medium(11.5),
                fg,
            );
            ui.painter().galley(
                pos2(rect.left() + 25.0, rect.center().y - g.size().y * 0.5),
                g,
                fg,
            );
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() && !active {
                *current = ct;
                changed = true;
            }
        }
    });
    changed
}

fn column_type_icon(col_type: ColumnType) -> Icon {
    match col_type {
        ColumnType::Text => Icon::Type,
        ColumnType::Number => Icon::ListOrdered,
        ColumnType::Select => Icon::List,
        ColumnType::Checkbox => Icon::Task,
        ColumnType::Date => Icon::CalendarDays,
        ColumnType::NoteLink => Icon::Link,
    }
}
