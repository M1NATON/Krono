use crate::i18n::{strings, Lang};
use crate::storage::cover_cache::{CoverCache, CoverLookup};
use crate::sync::tmdb::{TmdbMediaKind, TmdbSearchItem, TMDB_GENRES};
use crate::ui::collection_view::{CollectionEditorState, CollectionSort, CollectionSortDir};
use crate::ui::components::{
    primary_button, secondary_button, segmented_control, styled_text_input,
};
use crate::ui::icons::{draw_icon, paint_star, Icon};
use crate::ui::theme::{font_body, font_medium, font_mono, font_semibold, ThemePalette};
use crate::ui::{SettingsTab, UiAction};
use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, Layout, Margin, Pos2, Rect, Rounding, Sense, Stroke,
    Ui,
};
use krono_core::{
    collection::{
        compare_records, record_matches_min_number, record_matches_smart_filter, strip_link_target,
        CollectionDoc, ColorToken, ColumnType, FieldValue, RecordItem, SelectOption,
        SortDirection,
    },
    daily::SimpleDate,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TmdbRecsMode {
    #[default]
    Favorites8Plus,
    SimilarTo,
    ByGenres,
}

pub struct TmdbUiState {
    pub search_open: bool,
    pub search_query: String,
    pub last_dispatched_query: String,
    pub last_input_at: Option<Instant>,
    pub search_busy: bool,
    pub search_error: Option<String>,
    pub search_results: Vec<TmdbSearchItem>,

    pub recs_open: bool,
    pub recs_mode: TmdbRecsMode,
    pub recs_similar_record_id: Option<String>,
    pub recs_genre_kind: TmdbMediaKind,
    pub recs_selected_genres: Vec<u32>,
    pub recs_busy: bool,
    pub recs_error: Option<String>,
    pub recs_results: Vec<TmdbSearchItem>,
}

impl Default for TmdbUiState {
    fn default() -> Self {
        Self {
            search_open: false,
            search_query: String::new(),
            last_dispatched_query: String::new(),
            last_input_at: None,
            search_busy: false,
            search_error: None,
            search_results: Vec::new(),
            recs_open: false,
            recs_mode: TmdbRecsMode::Favorites8Plus,
            recs_similar_record_id: None,
            recs_genre_kind: TmdbMediaKind::Movie,
            recs_selected_genres: vec![1, 2],
            recs_busy: false,
            recs_error: None,
            recs_results: Vec::new(),
        }
    }
}

impl TmdbUiState {
    pub fn reset(&mut self) {
        self.search_open = false;
        self.search_query.clear();
        self.last_dispatched_query.clear();
        self.last_input_at = None;
        self.search_busy = false;
        self.search_error = None;
        self.search_results.clear();
        self.recs_open = false;
        self.recs_mode = TmdbRecsMode::Favorites8Plus;
        self.recs_similar_record_id = None;
        self.recs_busy = false;
        self.recs_error = None;
        self.recs_results.clear();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_custom_gallery(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    today: SimpleDate,
    state: &mut CollectionEditorState,
    cover_cache: &mut CoverCache,
    has_tmdb_token: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);

    // Toolbar: for media_library, "+ Movie / TV" is the primary CTA (manual add lives inside the search modal)
    let is_media_col = has_media_columns(&state.doc);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
        if is_media_col {
            if primary_button(ui, p, Some(Icon::Search), s.tmdb_add_media_btn, false).clicked() {
                actions.push(UiAction::OpenTmdbSearch);
            }
            if secondary_button(ui, p, Some(Icon::Sparkles), s.tmdb_recommend_btn).clicked() {
                actions.push(UiAction::RequestTmdbRecommendations);
            }
        } else if primary_button(ui, p, Some(Icon::Plus), s.col_new_card, false).clicked() {
            add_manual_record_and_edit(lang, state, actions);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            render_gallery_sort_picker(ui, p, lang, state);
        });
    });
    ui.add_space(10.0);

    // Quick genre filter chips (when a "genres" column exists)
    let is_ru = lang == Lang::Ru;
    let genres_col = find_col_id(&state.doc, &["genres", "genre", "жанр"]);
    if let Some(gcol) = genres_col.as_deref() {
        let mut all_genres: Vec<String> = Vec::new();
        for rec in &state.doc.records {
            if let Some(val) = rec.values.get(gcol) {
                for part in val.as_text().split([',', ';', '/']) {
                    let g = krono_core::collection::normalize_genre_label(part, is_ru);
                    if !g.is_empty()
                        && !all_genres.iter().any(|x| x.to_lowercase() == g.to_lowercase())
                    {
                        all_genres.push(g);
                    }
                }
            }
        }
        all_genres.sort_by_key(|a| a.to_lowercase());
        if !all_genres.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
                let all_active = state.genre_filter.is_none();
                let all_btn = egui::Button::new(
                    egui::RichText::new(s.col_date_all)
                        .font(font_medium(11.5))
                        .color(if all_active { p.accent_text } else { p.text_secondary }),
                )
                .fill(if all_active { p.accent_soft } else { p.bg_sidebar })
                .stroke(Stroke::new(
                    1.0_f32,
                    if all_active { p.accent } else { p.border_hairline },
                ))
                .rounding(Rounding::same(12.0));
                if ui.add(all_btn).clicked() {
                    state.genre_filter = None;
                }
                for g in all_genres {
                    let active = state
                        .genre_filter
                        .as_deref()
                        .is_some_and(|f| f.to_lowercase() == g.to_lowercase());
                    let btn = egui::Button::new(
                        egui::RichText::new(&g)
                            .font(font_medium(11.5))
                            .color(if active { p.accent_text } else { p.text_secondary }),
                    )
                    .fill(if active { p.accent_soft } else { p.bg_sidebar })
                    .stroke(Stroke::new(
                        1.0_f32,
                        if active { p.accent } else { p.border_hairline },
                    ))
                    .rounding(Rounding::same(12.0));
                    if ui.add(btn).clicked() {
                        if active {
                            state.genre_filter = None;
                        } else {
                            state.genre_filter = Some(g);
                        }
                    }
                }
            });
            ui.add_space(10.0);
        }
    }

    // Filter + sort records
    let q_lower = state.filter_query.trim().to_lowercase();
    let select_f = state
        .select_filter
        .as_ref()
        .map(|(c, o)| (c.as_str(), o.as_str()));
    let min_num_f = state
        .min_number_filter
        .as_ref()
        .map(|(c, v)| (c.as_str(), *v));
    let genre_f = state.genre_filter.as_deref();
    let mut records: Vec<&RecordItem> = state
        .doc
        .records
        .iter()
        .filter(|rec| {
            let base_ok = record_matches_smart_filter(
                rec,
                &state.doc.columns,
                &q_lower,
                state.date_filter,
                state.tag_filter.as_deref(),
                select_f,
                today,
            ) && record_matches_min_number(rec, min_num_f);
            if !base_ok {
                return false;
            }
            if let Some(gf) = genre_f {
                let Some(gcol) = genres_col.as_deref() else {
                    return false;
                };
                let gf_low = gf.trim().to_lowercase();
                let matches_genre = rec.values.get(gcol).is_some_and(|v| {
                    v.as_text().split([',', ';', '/']).any(|part| {
                        krono_core::collection::normalize_genre_label(part, is_ru).to_lowercase()
                            == gf_low
                    })
                });
                if !matches_genre {
                    return false;
                }
            }
            true
        })
        .collect();

    if let Some(sort) = &state.sort {
        let dir = match sort.dir {
            CollectionSortDir::Asc => SortDirection::Asc,
            CollectionSortDir::Desc => SortDirection::Desc,
        };
        records.sort_by(|a, b| compare_records(a, b, &sort.column_key, dir, &state.doc.columns));
    } else {
        records.sort_by_key(|r| (r.order, r.created_at));
    }

    if records.is_empty() {
        egui::Frame::none()
            .fill(p.bg_sidebar)
            .stroke(Stroke::new(1.0_f32, p.border_hairline))
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::same(24.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(s.col_gallery_empty)
                            .font(font_body(13.5))
                            .color(p.text_muted),
                    );
                });
            });
        return;
    }

    let poster_col = find_col_id(&state.doc, &["poster_path", "poster", "cover"]);
    let rating_col = find_col_id(&state.doc, &["rating", "score", "оценка"]);
    let year_col = find_col_id(&state.doc, &["year", "год"]);
    let progress_col = find_col_id(&state.doc, &["progress", "прогресс"]);
    let review_col = find_col_id(&state.doc, &["review", "мини-отзыв", "notes"]);
    let status_col = state.doc.effective_group_column_id().map(str::to_string);
    let note_col = state
        .doc
        .columns
        .iter()
        .find(|c| c.col_type == ColumnType::NoteLink)
        .map(|c| c.id.clone());

    let status_options: Vec<SelectOption> = status_col
        .as_deref()
        .and_then(|cid| state.doc.columns.iter().find(|c| c.id == cid))
        .map(|c| c.options.clone())
        .unwrap_or_default();

    let gap = 14.0_f32;
    let mut open_record_id: Option<String> = None;
    let mut ctx_action: Option<GalleryCardContextAction> = None;

    egui::ScrollArea::vertical()
        .id_salt("c_gallery_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let avail_w = ui.available_width().max(180.0);
            let cols = (((avail_w + gap) / (186.0 + gap)).floor() as usize).max(1);
            let card_w = ((avail_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32)
                .clamp(140.0, 240.0);
            let poster_h = (card_w * 1.5).round();

            for row in records.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for rec in row {
                        let meta = GalleryCardMeta {
                            poster: poster_col
                                .as_deref()
                                .and_then(|c| rec.values.get(c))
                                .map(|v| v.as_text())
                                .unwrap_or_default(),
                            rating: rating_col
                                .as_deref()
                                .and_then(|c| rec.values.get(c))
                                .and_then(|v| v.as_number()),
                            year: year_col
                                .as_deref()
                                .and_then(|c| rec.values.get(c))
                                .map(|v| v.as_text())
                                .unwrap_or_default(),
                            progress: progress_col
                                .as_deref()
                                .and_then(|c| rec.values.get(c))
                                .map(|v| v.as_text())
                                .unwrap_or_default(),
                            review: review_col
                                .as_deref()
                                .and_then(|c| rec.values.get(c))
                                .map(|v| v.as_text())
                                .filter(|t| !t.trim().is_empty())
                                .unwrap_or_else(|| rec.description.clone()),
                            status_opt: status_col
                                .as_deref()
                                .and_then(|cid| resolve_select_label(&state.doc, rec, cid)),
                            note_path: note_col
                                .as_deref()
                                .and_then(|cid| rec.values.get(cid))
                                .map(|v| strip_link_target(&v.as_text()).to_string())
                                .filter(|s| !s.trim().is_empty()),
                        };
                        let ctx_cfg = GalleryCardCtxConfig {
                            status_col: status_col.as_deref(),
                            status_options: &status_options,
                            rating_col: rating_col.as_deref(),
                        };
                        if render_gallery_card(
                            ctx,
                            ui,
                            p,
                            lang,
                            card_w,
                            poster_h,
                            rec,
                            &meta,
                            &ctx_cfg,
                            cover_cache,
                            actions,
                            &mut ctx_action,
                        ) {
                            open_record_id = Some(rec.id.clone());
                        }
                    }
                });
                ui.add_space(gap);
            }
        });

    let _ = has_tmdb_token;
    if let Some(id) = open_record_id {
        state.editing_record_id = Some(id);
    }
    if let Some(act) = ctx_action {
        match act {
            GalleryCardContextAction::Edit(id) => {
                state.editing_record_id = Some(id);
            }
            GalleryCardContextAction::SetStatus {
                rec_id,
                col_id,
                opt_id,
            } => {
                if let Some(rec) = state.doc.records.iter_mut().find(|r| r.id == rec_id) {
                    rec.values.insert(col_id, FieldValue::Text(opt_id));
                    actions.push(UiAction::MarkCollectionDirty);
                }
            }
            GalleryCardContextAction::SetRating {
                rec_id,
                col_id,
                rating,
            } => {
                if let Some(rec) = state.doc.records.iter_mut().find(|r| r.id == rec_id) {
                    rec.values.insert(col_id, FieldValue::Number(rating));
                    actions.push(UiAction::MarkCollectionDirty);
                }
            }
            GalleryCardContextAction::Delete(id) => {
                state.doc.remove_record(&id);
                if state.editing_record_id.as_deref() == Some(id.as_str()) {
                    state.editing_record_id = None;
                }
                actions.push(UiAction::MarkCollectionDirty);
            }
        }
    }
}

fn add_manual_record_and_edit(
    lang: Lang,
    state: &mut CollectionEditorState,
    actions: &mut Vec<UiAction>,
) {
    let title = if lang == Lang::Ru {
        "Новая запись"
    } else {
        "New Item"
    };
    let stage = state.doc.effective_group_column_id().and_then(|gid| {
        state
            .doc
            .columns
            .iter()
            .find(|c| c.id == gid)
            .and_then(|c| c.options.first())
            .map(|o| (gid.to_string(), o.id.clone()))
    });
    let stage_ref = stage.as_ref().map(|(c, o)| (c.as_str(), o.as_str()));
    let new_id = state
        .doc
        .add_record(title, stage_ref, chrono::Utc::now().timestamp());
    state.editing_record_id = Some(new_id);
    actions.push(UiAction::MarkCollectionDirty);
}

enum GalleryCardContextAction {
    Edit(String),
    SetStatus {
        rec_id: String,
        col_id: String,
        opt_id: String,
    },
    SetRating {
        rec_id: String,
        col_id: String,
        rating: f64,
    },
    Delete(String),
}

struct GalleryCardCtxConfig<'a> {
    status_col: Option<&'a str>,
    status_options: &'a [SelectOption],
    rating_col: Option<&'a str>,
}

#[allow(clippy::too_many_arguments)]
pub fn render_tmdb_modals(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    cover_cache: &mut CoverCache,
    has_tmdb_token: bool,
    actions: &mut Vec<UiAction>,
) {
    if state.tmdb_ui.search_open {
        render_tmdb_search_modal(ctx, p, lang, state, cover_cache, has_tmdb_token, actions);
    } else if state.tmdb_ui.recs_open {
        render_tmdb_recs_modal(ctx, p, lang, state, cover_cache, has_tmdb_token, actions);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tmdb_search_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    cover_cache: &mut CoverCache,
    has_tmdb_token: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135_u8 } else { 75_u8 };
    let mut close = false;

    egui::Area::new(egui::Id::new("tmdb_search_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha));
            if r.clicked() {
                close = true;
            }
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    egui::Area::new(egui::Id::new("tmdb_search_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 48.0))
        .show(ctx, |ui| {
            let modal_w = 580.0_f32.min((screen.width() - 32.0).max(280.0));
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(18.0))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(s.tmdb_search_title)
                                .font(font_semibold(16.0))
                                .color(p.text_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::ui::components::icon_button(ui, p, Icon::Close, s.tooltip_close, false)
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(10.0);

                    if !has_tmdb_token {
                        render_no_tmdb_key_banner(ui, p, s, &mut close, actions);
                        ui.add_space(10.0);
                        if secondary_button(ui, p, Some(Icon::Plus), s.tmdb_add_manual_btn).clicked()
                        {
                            add_manual_record_and_edit(lang, state, actions);
                            close = true;
                        }
                        return;
                    }

                    ui.horizontal(|ui| {
                        let inp_w = (modal_w - 110.0).max(160.0);
                        let resp = styled_text_input(
                            ui,
                            p,
                            &mut state.tmdb_ui.search_query,
                            s.tmdb_search_hint,
                            false,
                            inp_w,
                        );
                        if resp.changed() {
                            state.tmdb_ui.last_input_at = Some(Instant::now());
                        }
                        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if (primary_button(ui, p, Some(Icon::Search), s.tmdb_search_btn, false)
                            .clicked()
                            || enter)
                            && !state.tmdb_ui.search_query.trim().is_empty()
                        {
                            let q = state.tmdb_ui.search_query.trim().to_string();
                            state.tmdb_ui.last_dispatched_query = q.clone();
                            state.tmdb_ui.last_input_at = None;
                            actions.push(UiAction::TmdbSearch(q));
                        }
                    });

                    // Debounced live search (>= 2 chars after 350 ms)
                    if let Some(t) = state.tmdb_ui.last_input_at {
                        let q = state.tmdb_ui.search_query.trim().to_string();
                        if q.chars().count() >= 2 && q != state.tmdb_ui.last_dispatched_query {
                            if t.elapsed() >= Duration::from_millis(350) {
                                state.tmdb_ui.last_dispatched_query = q.clone();
                                state.tmdb_ui.last_input_at = None;
                                actions.push(UiAction::TmdbSearch(q));
                            } else {
                                ctx.request_repaint_after(Duration::from_millis(120));
                            }
                        }
                    }

                    ui.add_space(10.0);
                    if state.tmdb_ui.search_busy {
                        ui.label(
                            egui::RichText::new("TMDB...")
                                .font(font_body(12.5))
                                .color(p.text_muted),
                        );
                        ui.add_space(6.0);
                    }
                    if let Some(err) = &state.tmdb_ui.search_error {
                        ui.label(
                            egui::RichText::new(err)
                                .font(font_body(12.5))
                                .color(p.danger),
                        );
                        ui.add_space(6.0);
                    }

                    let results = state.tmdb_ui.search_results.clone();
                    let list_h = (screen.height() - 280.0).clamp(180.0, 420.0);
                    egui::ScrollArea::vertical()
                        .id_salt("tmdb_search_scroll")
                        .auto_shrink([false, false])
                        .max_height(list_h)
                        .show(ui, |ui| {
                            for item in results {
                                if render_tmdb_result_row(
                                    ctx,
                                    ui,
                                    p,
                                    lang,
                                    cover_cache,
                                    &item,
                                    s.tmdb_add_btn,
                                ) {
                                    actions.push(UiAction::TmdbAddMedia {
                                        item,
                                        status_opt: "watchlist".to_string(),
                                    });
                                    close = true;
                                }
                                ui.add_space(8.0);
                            }
                        });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if secondary_button(ui, p, Some(Icon::Plus), s.tmdb_add_manual_btn).clicked()
                        {
                            add_manual_record_and_edit(lang, state, actions);
                            close = true;
                        }
                    });
                });
        });

    if close {
        state.tmdb_ui.search_open = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tmdb_recs_modal(
    ctx: &egui::Context,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
    cover_cache: &mut CoverCache,
    has_tmdb_token: bool,
    actions: &mut Vec<UiAction>,
) {
    let s = strings(lang);
    let screen = ctx.screen_rect();
    let max_alpha = if p.is_dark { 135_u8 } else { 75_u8 };
    let mut close = false;

    egui::Area::new(egui::Id::new("tmdb_recs_bg"))
        .order(egui::Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let r = ui.allocate_rect(screen, Sense::click());
            ui.painter()
                .rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, max_alpha));
            if r.clicked() {
                close = true;
            }
        });

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close = true;
    }

    egui::Area::new(egui::Id::new("tmdb_recs_win"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_TOP, vec2(0.0, 36.0))
        .show(ctx, |ui| {
            let modal_w = 640.0_f32.min((screen.width() - 32.0).max(300.0));
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_subtle))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(18.0))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(s.tmdb_recs_title)
                                .font(font_semibold(16.0))
                                .color(p.text_primary),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::ui::components::icon_button(ui, p, Icon::Close, s.tooltip_close, false)
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(10.0);

                    if !has_tmdb_token {
                        render_no_tmdb_key_banner(ui, p, s, &mut close, actions);
                        return;
                    }

                    // Mode switcher: Favorites (8+) / Similar to / By genres
                    let mut mode = state.tmdb_ui.recs_mode;
                    if segmented_control(
                        ui,
                        p,
                        "tmdb_recs_mode_seg",
                        &mut mode,
                        &[
                            (TmdbRecsMode::Favorites8Plus, Some(Icon::Star), s.tmdb_recs_mode_favs),
                            (TmdbRecsMode::SimilarTo, Some(Icon::Sparkles), s.tmdb_recs_mode_similar),
                            (TmdbRecsMode::ByGenres, Some(Icon::Filter), s.tmdb_recs_mode_genres),
                        ],
                    ) {
                        state.tmdb_ui.recs_mode = mode;
                        state.tmdb_ui.recs_error = None;
                        if mode == TmdbRecsMode::Favorites8Plus {
                            actions.push(UiAction::RequestTmdbRecommendations);
                        }
                    }
                    ui.add_space(10.0);

                    match state.tmdb_ui.recs_mode {
                        TmdbRecsMode::Favorites8Plus => {}
                        TmdbRecsMode::SimilarTo => {
                            let tmdb_records: Vec<(String, String, TmdbMediaKind, String)> = state
                                .doc
                                .records
                                .iter()
                                .filter_map(|r| {
                                    let tid = r
                                        .values
                                        .get("tmdb_id")
                                        .map(FieldValue::as_text)
                                        .map(|t| t.trim().to_string())
                                        .filter(|t| !t.is_empty())?;
                                    let kind = r
                                        .values
                                        .get("media_type")
                                        .map(FieldValue::as_text)
                                        .and_then(|s| TmdbMediaKind::from_select_opt(&s))
                                        .unwrap_or(TmdbMediaKind::Movie);
                                    Some((r.id.clone(), r.title.clone(), kind, tid))
                                })
                                .collect();
                            if tmdb_records.is_empty() {
                                ui.label(
                                    egui::RichText::new(s.tmdb_recs_no_titles)
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                            } else {
                                let selected_label = state
                                    .tmdb_ui
                                    .recs_similar_record_id
                                    .as_deref()
                                    .and_then(|sid| tmdb_records.iter().find(|(id, ..)| id == sid))
                                    .map(|(_, title, ..)| title.as_str())
                                    .unwrap_or(s.tmdb_recs_pick_title);
                                let mut picked: Option<(String, TmdbMediaKind, String)> = None;
                                egui::ComboBox::from_id_salt("tmdb_similar_picker")
                                    .width((modal_w - 40.0).max(200.0))
                                    .selected_text(selected_label)
                                    .show_ui(ui, |ui| {
                                        for (rid, title, kind, tid) in &tmdb_records {
                                            let is_sel = state.tmdb_ui.recs_similar_record_id.as_deref()
                                                == Some(rid.as_str());
                                            if ui.selectable_label(is_sel, title).clicked() {
                                                picked = Some((rid.clone(), *kind, tid.clone()));
                                            }
                                        }
                                    });
                                if let Some((rid, kind, tid)) = picked {
                                    state.tmdb_ui.recs_similar_record_id = Some(rid);
                                    actions.push(UiAction::RequestTmdbSimilar { kind, tmdb_id: tid });
                                }
                            }
                            ui.add_space(8.0);
                        }
                        TmdbRecsMode::ByGenres => {
                            ui.horizontal(|ui| {
                                segmented_control(
                                    ui,
                                    p,
                                    "tmdb_genre_kind_seg",
                                    &mut state.tmdb_ui.recs_genre_kind,
                                    &[
                                        (TmdbMediaKind::Movie, None, s.tmdb_kind_movie),
                                        (TmdbMediaKind::Tv, None, s.tmdb_kind_tv),
                                    ],
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if primary_button(
                                        ui,
                                        p,
                                        Some(Icon::Sparkles),
                                        s.tmdb_recs_find_btn,
                                        false,
                                    )
                                    .clicked()
                                    {
                                        actions.push(UiAction::RequestTmdbByGenres {
                                            kind: state.tmdb_ui.recs_genre_kind,
                                            genre_keys: state.tmdb_ui.recs_selected_genres.clone(),
                                        });
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                                for g in TMDB_GENRES {
                                    let active = state.tmdb_ui.recs_selected_genres.contains(&g.id);
                                    let lbl = if lang == Lang::Ru { g.label_ru } else { g.label_en };
                                    let fill = if active { p.accent_soft } else { p.bg_sidebar };
                                    let stroke = if active { p.accent } else { p.border_hairline };
                                    let fg = if active { p.accent_text } else { p.text_secondary };
                                    let btn = egui::Button::new(
                                        egui::RichText::new(lbl).font(font_medium(11.5)).color(fg),
                                    )
                                    .fill(fill)
                                    .stroke(Stroke::new(1.0_f32, stroke))
                                    .rounding(Rounding::same(6.0));
                                    if ui.add(btn).clicked() {
                                        if active {
                                            state.tmdb_ui.recs_selected_genres.retain(|x| *x != g.id);
                                        } else {
                                            state.tmdb_ui.recs_selected_genres.push(g.id);
                                        }
                                    }
                                }
                            });
                            ui.add_space(8.0);
                        }
                    }

                    if state.tmdb_ui.recs_busy {
                        ui.label(
                            egui::RichText::new("TMDB...")
                                .font(font_body(12.5))
                                .color(p.text_muted),
                        );
                        ui.add_space(6.0);
                    }
                    if let Some(err) = &state.tmdb_ui.recs_error {
                        ui.label(
                            egui::RichText::new(err)
                                .font(font_body(12.5))
                                .color(p.danger),
                        );
                        ui.add_space(6.0);
                    }

                    if !state.tmdb_ui.recs_busy
                        && state.tmdb_ui.recs_error.is_none()
                        && state.tmdb_ui.recs_results.is_empty()
                    {
                        ui.label(
                            egui::RichText::new(s.tmdb_recs_empty)
                                .font(font_body(13.0))
                                .color(p.text_muted),
                        );
                    }

                    let results = state.tmdb_ui.recs_results.clone();
                    let mut remove_id: Option<String> = None;
                    let list_h = (screen.height() - 290.0).clamp(240.0, 440.0);
                    egui::ScrollArea::vertical()
                        .id_salt("tmdb_recs_scroll")
                        .auto_shrink([false, false])
                        .min_scrolled_height(260.0)
                        .max_height(list_h)
                        .show(ui, |ui| {
                            for item in results {
                                if render_tmdb_result_row(
                                    ctx,
                                    ui,
                                    p,
                                    lang,
                                    cover_cache,
                                    &item,
                                    s.tmdb_add_watchlist_btn,
                                ) {
                                    remove_id = Some(item.tmdb_id.clone());
                                    actions.push(UiAction::TmdbAddMedia {
                                        item,
                                        status_opt: "watchlist".to_string(),
                                    });
                                }
                                ui.add_space(8.0);
                            }
                        });
                    if let Some(rid) = remove_id {
                        state.tmdb_ui.recs_results.retain(|r| r.tmdb_id != rid);
                    }
                });
        });

    if close {
        state.tmdb_ui.recs_open = false;
    }
}

fn render_no_tmdb_key_banner(
    ui: &mut Ui,
    p: &ThemePalette,
    s: &crate::i18n::Strings,
    close: &mut bool,
    actions: &mut Vec<UiAction>,
) {
    ui.add(
        egui::Label::new(
            egui::RichText::new(s.tmdb_no_key_msg)
                .font(font_body(13.0))
                .color(p.text_secondary),
        )
        .wrap(),
    );
    ui.add_space(10.0);
    if primary_button(ui, p, Some(Icon::Settings), s.tmdb_open_settings_btn, false).clicked() {
        *close = true;
        actions.push(UiAction::OpenSettings(SettingsTab::GitHub));
    }
}

fn render_tmdb_result_row(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    cover_cache: &mut CoverCache,
    item: &TmdbSearchItem,
    action_label: &str,
) -> bool {
    let mut clicked = false;
    egui::Frame::none()
        .fill(p.bg_sidebar)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (thumb_rect, _) = ui.allocate_exact_size(vec2(46.0, 69.0), Sense::hover());
                let lookup = item
                    .poster_path
                    .as_deref()
                    .map(|pp| cover_cache.get_or_request(ctx, pp))
                    .unwrap_or(CoverLookup::Empty);
                match lookup {
                    CoverLookup::Ready(tex_id) => {
                        ui.painter().image(
                            tex_id,
                            thumb_rect,
                            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                    }
                    _ => {
                        ui.painter()
                            .rect_filled(thumb_rect, Rounding::same(4.0), p.bg_elevated);
                        let ir = Rect::from_center_size(thumb_rect.center(), vec2(18.0, 18.0));
                        draw_icon(ui.painter(), ir, Icon::Image, p.text_muted);
                    }
                }

                ui.add_space(10.0);
                let text_w = (ui.available_width() - 155.0).max(140.0);
                ui.allocate_ui_with_layout(
                    vec2(text_w, 69.0),
                    Layout::top_down(Align::LEFT),
                    |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                egui::RichText::new(&item.title)
                                    .font(font_semibold(13.5))
                                    .color(p.text_primary),
                            );
                            if let Some(y) = item.year {
                                ui.label(
                                    egui::RichText::new(format!("({y})"))
                                        .font(font_mono(12.0))
                                        .color(p.text_muted),
                                );
                            }
                            let kind_lbl = match (item.kind, lang) {
                                (TmdbMediaKind::Movie, Lang::Ru) => "Фильм",
                                (TmdbMediaKind::Movie, Lang::En) => "Movie",
                                (TmdbMediaKind::Tv, Lang::Ru) => "Сериал",
                                (TmdbMediaKind::Tv, Lang::En) => "TV",
                            };
                            ui.label(
                                egui::RichText::new(kind_lbl)
                                    .font(font_medium(11.0))
                                    .color(p.accent_text),
                            );
                            if let Some(v) = item.vote_average {
                                let (sr, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
                                paint_star(
                                    ui.painter(),
                                    sr.center(),
                                    5.0,
                                    Color32::from_rgb(245, 158, 11),
                                    None,
                                );
                                ui.label(
                                    egui::RichText::new(format!("{v:.1}"))
                                        .font(font_mono(11.5))
                                        .color(Color32::from_rgb(245, 158, 11)),
                                );
                            }
                        });
                        if !item.overview.trim().is_empty() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(truncate_chars(&item.overview, 115))
                                        .font(font_body(11.5))
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        }
                    },
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if primary_button(ui, p, Some(Icon::Plus), action_label, false).clicked() {
                        clicked = true;
                    }
                });
            });
        });
    clicked
}

struct GalleryCardMeta {
    poster: String,
    rating: Option<f64>,
    year: String,
    progress: String,
    review: String,
    status_opt: Option<(String, ColorToken)>,
    note_path: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn render_gallery_card(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    card_w: f32,
    poster_h: f32,
    rec: &RecordItem,
    meta: &GalleryCardMeta,
    ctx_cfg: &GalleryCardCtxConfig<'_>,
    cover_cache: &mut CoverCache,
    actions: &mut Vec<UiAction>,
    ctx_action: &mut Option<GalleryCardContextAction>,
) -> bool {
    let s = strings(lang);
    let mut clicked_card = false;

    let frame = egui::Frame::none()
        .fill(p.bg_elevated)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(10.0))
        .inner_margin(Margin::same(0.0));

    let resp = frame.show(ui, |ui| {
        ui.set_width(card_w);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            // Poster 2:3 area
            let (poster_rect, poster_resp) =
                ui.allocate_exact_size(vec2(card_w, poster_h), Sense::click());
            if poster_resp.clicked() {
                clicked_card = true;
            }
            attach_card_context_menu(&poster_resp, p, lang, rec, meta, ctx_cfg, actions, ctx_action);
            let hovered = poster_resp.hovered();
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let rounding = Rounding {
                nw: 10.0,
                ne: 10.0,
                sw: 0.0,
                se: 0.0,
            };
            let lookup = cover_cache.get_or_request(ctx, &meta.poster);
            match lookup {
                CoverLookup::Ready(tex_id) => {
                    let tex_size = ctx
                        .tex_manager()
                        .read()
                        .meta(tex_id)
                        .map(|m| m.size)
                        .unwrap_or([2, 3]);
                    let uv = aspect_fill_uv(tex_size, poster_rect.size());
                    ui.painter().image(tex_id, poster_rect, uv, Color32::WHITE);
                }
                _ => {
                    paint_poster_placeholder(ui.painter(), poster_rect, rounding, p, rec, &meta.year);
                }
            }

            // Hover ring over poster
            if hovered {
                ui.painter().rect_stroke(
                    poster_rect.shrink(0.5),
                    rounding,
                    Stroke::new(1.5_f32, p.accent),
                );
            }

            // Rating badge with vector star on semi-transparent dark pill (top-right)
            if let Some(r) = meta.rating {
                let rating_txt = if (r - r.round()).abs() < 0.05 {
                    format!("{:.0}", r)
                } else {
                    format!("{:.1}", r)
                };
                paint_rating_pill_badge(
                    ui.painter(),
                    pos2(poster_rect.right() - 8.0, poster_rect.top() + 8.0),
                    &rating_txt,
                );
            }

            // Progress badge (top-left) if present
            if !meta.progress.trim().is_empty() {
                paint_dark_pill_badge(
                    ui.painter(),
                    pos2(poster_rect.left() + 8.0, poster_rect.top() + 8.0),
                    Align2::LEFT_TOP,
                    meta.progress.trim(),
                    Color32::from_rgb(226, 232, 240),
                );
            }

            // Info footer
            egui::Frame::none()
                .inner_margin(Margin::symmetric(10.0, 9.0))
                .show(ui, |ui| {
                    ui.set_width((card_w - 20.0).max(80.0));
                    ui.spacing_mut().item_spacing.y = 5.0;

                    let title_txt = if rec.title.trim().is_empty() {
                        s.untitled_hint
                    } else {
                        rec.title.trim()
                    };
                    let title_resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(title_txt)
                                .font(font_semibold(13.0))
                                .color(p.text_primary),
                        )
                        .truncate()
                        .sense(Sense::click()),
                    );
                    if title_resp.clicked() {
                        clicked_card = true;
                    }

                    // Year + status pill row
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if !meta.year.trim().is_empty() {
                            ui.label(
                                egui::RichText::new(meta.year.trim())
                                    .font(font_mono(11.0))
                                    .color(p.text_muted),
                            );
                        }
                        if let Some((lbl, tok)) = &meta.status_opt {
                            let (bg, fg) = token_pill_colors(p, *tok);
                            egui::Frame::none()
                                .fill(bg)
                                .rounding(Rounding::same(5.0))
                                .inner_margin(Margin::symmetric(6.0, 1.5))
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(lbl)
                                            .font(font_medium(10.5))
                                            .color(fg),
                                    );
                                });
                        }
                    });

                    // Mini-review preview (truncated)
                    let review_clean = meta.review.trim();
                    if !review_clean.is_empty() {
                        let preview = truncate_chars(review_clean, 68);
                        ui.label(
                            egui::RichText::new(preview)
                                .font(font_body(11.5))
                                .color(p.text_secondary),
                        );
                    }

                    // Quick note link button
                    if let Some(note_path) = &meta.note_path {
                        let btn_resp = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("↗ {}", s.col_open_linked_note))
                                    .font(font_medium(11.0))
                                    .color(p.accent_text),
                            )
                            .fill(p.accent_soft)
                            .stroke(Stroke::NONE)
                            .rounding(Rounding::same(6.0)),
                        );
                        if btn_resp.clicked() {
                            actions.push(UiAction::OpenNote(note_path.clone()));
                        }
                    }
                });
        });
    });

    if resp.response.hovered() {
        ui.painter().rect_stroke(
            resp.response.rect,
            Rounding::same(10.0),
            Stroke::new(1.0_f32, p.accent.linear_multiply(0.55)),
        );
    }
    clicked_card
}

fn paint_poster_placeholder(
    painter: &egui::Painter,
    rect: Rect,
    rounding: Rounding,
    p: &ThemePalette,
    rec: &RecordItem,
    year: &str,
) {
    let base = if p.is_dark {
        Color32::from_rgb(28, 32, 42)
    } else {
        Color32::from_rgb(228, 232, 240)
    };
    painter.rect_filled(rect, rounding, base);
    let icon_rect = Rect::from_center_size(pos2(rect.center().x, rect.center().y - 16.0), vec2(26.0, 26.0));
    draw_icon(painter, icon_rect, Icon::Image, p.text_muted);

    let title = truncate_chars(rec.title.trim(), 24);
    if !title.is_empty() {
        painter.text(
            pos2(rect.center().x, rect.center().y + 14.0),
            Align2::CENTER_CENTER,
            title,
            font_medium(12.0),
            p.text_secondary,
        );
    }
    if !year.trim().is_empty() {
        painter.text(
            pos2(rect.center().x, rect.center().y + 32.0),
            Align2::CENTER_CENTER,
            year.trim(),
            font_mono(11.0),
            p.text_muted,
        );
    }
}

fn paint_dark_pill_badge(
    painter: &egui::Painter,
    anchor_pos: Pos2,
    align: Align2,
    text: &str,
    fg: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_string(), font_semibold(11.5), fg);
    let pad = vec2(7.0, 3.5);
    let size = galley.size() + pad * 2.0;
    let min = match align {
        Align2::RIGHT_TOP => pos2(anchor_pos.x - size.x, anchor_pos.y),
        _ => anchor_pos,
    };
    let rect = Rect::from_min_size(min, size);
    painter.rect_filled(
        rect,
        Rounding::same(6.0),
        Color32::from_rgba_unmultiplied(15, 18, 24, 210),
    );
    painter.rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(0.75_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
    );
    painter.galley(rect.min + pad, galley, fg);
}

fn paint_rating_pill_badge(painter: &egui::Painter, top_right: Pos2, score_text: &str) {
    let fg = Color32::from_rgb(251, 191, 36);
    let galley = painter.layout_no_wrap(score_text.to_string(), font_semibold(11.5), fg);
    let pad = vec2(7.0, 3.5);
    let star_w = 13.0_f32;
    let size = vec2(star_w + galley.size().x + pad.x * 2.0, galley.size().y + pad.y * 2.0);
    let rect = Rect::from_min_size(pos2(top_right.x - size.x, top_right.y), size);
    painter.rect_filled(
        rect,
        Rounding::same(6.0),
        Color32::from_rgba_unmultiplied(15, 18, 24, 210),
    );
    painter.rect_stroke(
        rect,
        Rounding::same(6.0),
        Stroke::new(0.75_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
    );
    paint_star(
        painter,
        pos2(rect.left() + pad.x + 4.5, rect.center().y),
        4.6,
        fg,
        None,
    );
    painter.galley(pos2(rect.left() + pad.x + star_w, rect.top() + pad.y), galley, fg);
}

#[allow(clippy::too_many_arguments)]
fn attach_card_context_menu(
    resp: &egui::Response,
    p: &ThemePalette,
    lang: Lang,
    rec: &RecordItem,
    meta: &GalleryCardMeta,
    ctx_cfg: &GalleryCardCtxConfig<'_>,
    actions: &mut Vec<UiAction>,
    ctx_action: &mut Option<GalleryCardContextAction>,
) {
    let s = strings(lang);
    resp.context_menu(|ui| {
        ui.set_min_width(180.0);
        if ui.button(s.col_ctx_edit).clicked() {
            *ctx_action = Some(GalleryCardContextAction::Edit(rec.id.clone()));
            ui.close_menu();
        }
        if let Some(scol) = ctx_cfg.status_col {
            if !ctx_cfg.status_options.is_empty() {
                ui.menu_button(s.col_ctx_status, |ui| {
                    for opt in ctx_cfg.status_options {
                        if ui.button(&opt.label).clicked() {
                            *ctx_action = Some(GalleryCardContextAction::SetStatus {
                                rec_id: rec.id.clone(),
                                col_id: scol.to_string(),
                                opt_id: opt.id.clone(),
                            });
                            ui.close_menu();
                        }
                    }
                });
            }
        }
        if let Some(rcol) = ctx_cfg.rating_col {
            ui.menu_button(s.col_ctx_rating, |ui| {
                for score in (1..=10).rev() {
                    ui.horizontal(|ui| {
                        let (sr, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                        paint_star(
                            ui.painter(),
                            sr.center(),
                            5.0,
                            Color32::from_rgb(245, 158, 11),
                            None,
                        );
                        if ui.button(format!("{score}/10")).clicked() {
                            *ctx_action = Some(GalleryCardContextAction::SetRating {
                                rec_id: rec.id.clone(),
                                col_id: rcol.to_string(),
                                rating: f64::from(score),
                            });
                            ui.close_menu();
                        }
                    });
                }
            });
        }
        ui.separator();
        if let Some(note_path) = &meta.note_path {
            if ui.button(s.col_ctx_open_note).clicked() {
                actions.push(UiAction::OpenNote(note_path.clone()));
                ui.close_menu();
            }
        }
        if ui.button(s.col_ctx_create_review).clicked() {
            actions.push(UiAction::CreateMediaReviewNote(rec.id.clone()));
            ui.close_menu();
        }
        ui.separator();
        if ui
            .button(egui::RichText::new(s.col_ctx_delete).color(p.danger))
            .clicked()
        {
            *ctx_action = Some(GalleryCardContextAction::Delete(rec.id.clone()));
            ui.close_menu();
        }
    });
}

fn aspect_fill_uv(tex_size: [usize; 2], target: egui::Vec2) -> Rect {
    let tw = tex_size[0].max(1) as f32;
    let th = tex_size[1].max(1) as f32;
    let tex_aspect = tw / th;
    let box_aspect = (target.x / target.y.max(1.0)).max(0.01);
    if tex_aspect > box_aspect {
        let keep = box_aspect / tex_aspect;
        let dx = (1.0 - keep) * 0.5;
        Rect::from_min_max(pos2(dx, 0.0), pos2(1.0 - dx, 1.0))
    } else {
        let keep = tex_aspect / box_aspect;
        let dy = (1.0 - keep) * 0.5;
        Rect::from_min_max(pos2(0.0, dy), pos2(1.0, 1.0 - dy))
    }
}

fn render_gallery_sort_picker(
    ui: &mut Ui,
    p: &ThemePalette,
    lang: Lang,
    state: &mut CollectionEditorState,
) {
    let s = strings(lang);
    let current_label = match &state.sort {
        None => s.col_sort_manual.to_string(),
        Some(cs) if cs.column_key == "__title" => {
            if cs.dir == CollectionSortDir::Asc {
                s.col_sort_title_asc.to_string()
            } else {
                format!("{} ↓", s.col_sort_title_asc)
            }
        }
        Some(cs) => {
            let name = state
                .doc
                .columns
                .iter()
                .find(|c| c.id == cs.column_key)
                .map(|c| c.name.as_str())
                .unwrap_or(cs.column_key.as_str());
            let arrow = if cs.dir == CollectionSortDir::Desc { "↓" } else { "↑" };
            format!("{name} {arrow}")
        }
    };

    let pop_id = ui.make_persistent_id("gallery_sort_popup");
    let btn = secondary_button(ui, p, Some(Icon::Sort), &current_label);
    if btn.clicked() {
        ui.memory_mut(|m| m.toggle_popup(pop_id));
    }

    let mut choices: Vec<(Option<CollectionSort>, String)> = vec![
        (None, s.col_sort_manual.to_string()),
        (
            Some(CollectionSort {
                column_key: "__title".into(),
                dir: CollectionSortDir::Asc,
            }),
            s.col_sort_title_asc.to_string(),
        ),
    ];
    for col in &state.doc.columns {
        if col.col_type == ColumnType::Number {
            choices.push((
                Some(CollectionSort {
                    column_key: col.id.clone(),
                    dir: CollectionSortDir::Desc,
                }),
                format!("{} ↓", col.name),
            ));
            choices.push((
                Some(CollectionSort {
                    column_key: col.id.clone(),
                    dir: CollectionSortDir::Asc,
                }),
                format!("{} ↑", col.name),
            ));
        }
    }

    egui::popup_below_widget(
        ui,
        pop_id,
        &btn,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(170.0);
            for (opt, label) in choices {
                let active = state.sort == opt;
                if crate::ui::components::context_menu_row(
                    ui,
                    p,
                    if active { Icon::Check } else { Icon::Sort },
                    &label,
                    false,
                )
                .clicked()
                {
                    state.sort = opt;
                    ui.memory_mut(|m| m.close_popup());
                }
            }
        },
    );
}

pub(crate) fn has_media_columns(doc: &CollectionDoc) -> bool {
    doc.columns
        .iter()
        .any(|c| c.id == "tmdb_id" || c.id == "poster_path")
}

fn find_col_id(doc: &CollectionDoc, candidates: &[&str]) -> Option<String> {
    for cand in candidates {
        if let Some(c) = doc.columns.iter().find(|col| col.id == *cand) {
            return Some(c.id.clone());
        }
    }
    for cand in candidates {
        if let Some(c) = doc
            .columns
            .iter()
            .find(|col| col.name.to_lowercase().contains(cand))
        {
            return Some(c.id.clone());
        }
    }
    None
}

fn resolve_select_label(
    doc: &CollectionDoc,
    rec: &RecordItem,
    col_id: &str,
) -> Option<(String, ColorToken)> {
    let col = doc.columns.iter().find(|c| c.id == col_id)?;
    let opt_id = rec.values.get(col_id)?.as_text();
    let opt = col.options.iter().find(|o| o.id == opt_id)?;
    Some((opt.label.clone(), opt.color))
}

fn token_pill_colors(p: &ThemePalette, token: ColorToken) -> (Color32, Color32) {
    let base = match token {
        ColorToken::Slate => Color32::from_rgb(100, 116, 139),
        ColorToken::Blue => Color32::from_rgb(59, 130, 246),
        ColorToken::Green => Color32::from_rgb(34, 197, 94),
        ColorToken::Amber => Color32::from_rgb(245, 158, 11),
        ColorToken::Coral => Color32::from_rgb(249, 115, 22),
        ColorToken::Violet => Color32::from_rgb(139, 92, 246),
        ColorToken::Teal => Color32::from_rgb(20, 184, 166),
        ColorToken::Rose => Color32::from_rgb(244, 63, 94),
    };
    let bg = if p.is_dark {
        Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 44)
    } else {
        Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 28)
    };
    (bg, base)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let single = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single.chars();
    let taken: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{taken}…")
    } else {
        taken
    }
}
