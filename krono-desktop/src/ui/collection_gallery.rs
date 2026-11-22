use crate::i18n::{strings, Lang};
use crate::storage::cover_cache::{CoverCache, CoverLookup};
use crate::sync::tmdb::{BookSearchItem, TmdbMediaKind, TmdbSearchItem, TMDB_GENRES};
use crate::ui::collection_view::{CollectionEditorState, CollectionSort, CollectionSortDir};
use crate::ui::components::{
    primary_button, secondary_button, segmented_control, styled_text_input,
};
use crate::ui::icons::{draw_icon, paint_star, Icon};
use crate::ui::motion::lerp_color;
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
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MediaAddTab {
    #[default]
    MoviesTv,
    Books,
    Manual,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TmdbRecsMode {
    #[default]
    Favorites8Plus,
    SimilarTo,
    ByGenres,
}

pub struct TmdbUiState {
    pub search_open: bool,
    pub active_tab: MediaAddTab,
    pub search_focus_requested: bool,
    pub search_query: String,
    pub last_dispatched_query: String,
    pub last_input_at: Option<Instant>,
    pub search_busy: bool,
    pub search_error: Option<String>,
    pub search_results: Vec<TmdbSearchItem>,

    pub book_query: String,
    pub last_dispatched_book_query: String,
    pub book_last_input_at: Option<Instant>,
    pub book_search_busy: bool,
    pub book_search_error: Option<String>,
    pub book_search_results: Vec<BookSearchItem>,

    pub manual_type: String,
    pub manual_title: String,
    pub manual_year: String,
    pub manual_creator: String,
    pub manual_cover_url: String,

    pub pending_add_keys: HashSet<String>,

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
            active_tab: MediaAddTab::MoviesTv,
            search_focus_requested: false,
            search_query: String::new(),
            last_dispatched_query: String::new(),
            last_input_at: None,
            search_busy: false,
            search_error: None,
            search_results: Vec::new(),
            book_query: String::new(),
            last_dispatched_book_query: String::new(),
            book_last_input_at: None,
            book_search_busy: false,
            book_search_error: None,
            book_search_results: Vec::new(),
            manual_type: "movie".to_string(),
            manual_title: String::new(),
            manual_year: String::new(),
            manual_creator: String::new(),
            manual_cover_url: String::new(),
            pending_add_keys: HashSet::new(),
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
        self.active_tab = MediaAddTab::MoviesTv;
        self.search_focus_requested = false;
        self.search_query.clear();
        self.last_dispatched_query.clear();
        self.last_input_at = None;
        self.search_busy = false;
        self.search_error = None;
        self.search_results.clear();
        self.book_query.clear();
        self.last_dispatched_book_query.clear();
        self.book_last_input_at = None;
        self.book_search_busy = false;
        self.book_search_error = None;
        self.book_search_results.clear();
        self.manual_type = "movie".to_string();
        self.manual_title.clear();
        self.manual_year.clear();
        self.manual_creator.clear();
        self.manual_cover_url.clear();
        self.pending_add_keys.clear();
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
    let is_media_col = has_media_columns(&state.doc);
    let is_ru = lang == Lang::Ru;

    // For non-media collections keep the compact gallery toolbar;
    // media libraries use the 2-row header in collection_view.rs.
    if !is_media_col {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            if primary_button(ui, p, Some(Icon::Plus), s.col_new_card, false).clicked() {
                add_manual_record_and_edit(lang, state, actions);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                render_gallery_sort_picker(ui, p, lang, state);
            });
        });
        ui.add_space(10.0);
    }

    // Filter + sort records
    let genres_col = find_col_id(&state.doc, &["genres", "genre", "жанр"]);
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
    let cat_f = state.media_category_filter.as_deref();
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
            ) && record_matches_min_number(rec, min_num_f)
                && record_matches_media_category(rec, cat_f);
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

    let avail_w = ui.available_width().max(180.0);
    let cols = (((avail_w + gap) / (186.0 + gap)).floor() as usize).max(1);
    let card_w = ((avail_w - gap * (cols.saturating_sub(1) as f32)) / cols as f32).clamp(140.0, 240.0);
    let poster_h = (card_w * 1.5).round();

    let mut kb_moved = false;
    let modal_open = state.editing_record_id.is_some()
        || state.tmdb_ui.search_open
        || state.tmdb_ui.recs_open
        || state.editing_column_id.is_some()
        || state.adding_field_open;
    if !modal_open && !ctx.wants_keyboard_input() && !records.is_empty() {
        let cur_idx = state
            .kb_selected_record_id
            .as_deref()
            .and_then(|id| records.iter().position(|r| r.id == id));
        let (left, right, up, down, enter, esc) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if esc && state.kb_selected_record_id.is_some() {
            state.kb_selected_record_id = None;
        } else if enter {
            if let Some(idx) = cur_idx {
                open_record_id = Some(records[idx].id.clone());
            }
        } else if left || right || up || down {
            let len = records.len();
            let next_idx = match cur_idx {
                None => 0,
                Some(idx) => {
                    if right {
                        (idx + 1).min(len - 1)
                    } else if left {
                        idx.saturating_sub(1)
                    } else if down {
                        (idx + cols).min(len - 1)
                    } else {
                        idx.saturating_sub(cols)
                    }
                }
            };
            state.kb_selected_record_id = Some(records[next_idx].id.clone());
            kb_moved = true;
        }
    }
    let kb_selected_id = state.kb_selected_record_id.clone();

    egui::ScrollArea::vertical()
        .id_salt("c_gallery_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for row in records.chunks(cols) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for rec in row {
                        let media_type = rec
                            .values
                            .get("media_type")
                            .map(FieldValue::as_text)
                            .unwrap_or_default();
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
                            progress: if media_type == "movie" {
                                String::new()
                            } else {
                                progress_col
                                    .as_deref()
                                    .and_then(|c| rec.values.get(c))
                                    .map(|v| v.as_text())
                                    .unwrap_or_default()
                            },
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
                        let is_kb_sel = kb_selected_id.as_deref() == Some(rec.id.as_str());
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
                            is_kb_sel,
                            kb_moved && is_kb_sel,
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
    let is_ru = lang == Lang::Ru;
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
        .anchor(Align2::CENTER_TOP, vec2(0.0, 44.0))
        .show(ctx, |ui| {
            let modal_w = 700.0_f32.min((screen.width() - 32.0).max(320.0));
            egui::Frame::none()
                .fill(p.bg_elevated)
                .stroke(Stroke::new(1.0_f32, p.border_strong))
                .rounding(Rounding::same(12.0))
                .shadow(p.shadow_floating)
                .inner_margin(Margin::same(16.0))
                .show(ui, |ui| {
                    ui.set_width(modal_w);
                    ui.horizontal(|ui| {
                        let tab_changed = segmented_control(
                            ui,
                            p,
                            "spotlight_media_tabs",
                            &mut state.tmdb_ui.active_tab,
                            &[
                                (MediaAddTab::MoviesTv, None, s.media_tab_movies_tv),
                                (MediaAddTab::Books, None, s.media_tab_books),
                                (MediaAddTab::Manual, None, s.media_tab_manual),
                            ],
                        );
                        if tab_changed {
                            state.tmdb_ui.search_focus_requested = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if crate::ui::components::icon_button(ui, p, Icon::Close, s.tooltip_close, false)
                                .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                    ui.add_space(10.0);

                    match state.tmdb_ui.active_tab {
                        MediaAddTab::MoviesTv => {
                            if !has_tmdb_token {
                                render_no_tmdb_key_banner(ui, p, s, &mut close, actions);
                                return;
                            }

                            let resp = styled_text_input(
                                ui,
                                p,
                                &mut state.tmdb_ui.search_query,
                                s.tmdb_search_hint,
                                false,
                                modal_w,
                            );
                            if state.tmdb_ui.search_focus_requested {
                                resp.request_focus();
                                state.tmdb_ui.search_focus_requested = false;
                            }
                            if resp.changed() {
                                state.tmdb_ui.last_input_at = Some(Instant::now());
                            }
                            let enter = (resp.has_focus() || resp.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if enter && !state.tmdb_ui.search_query.trim().is_empty() {
                                let q = state.tmdb_ui.search_query.trim().to_string();
                                state.tmdb_ui.last_dispatched_query = q.clone();
                                state.tmdb_ui.last_input_at = None;
                                actions.push(UiAction::TmdbSearch(q));
                            }

                            if let Some(t) = state.tmdb_ui.last_input_at {
                                let q = state.tmdb_ui.search_query.trim().to_string();
                                if q.chars().count() >= 2 && q != state.tmdb_ui.last_dispatched_query {
                                    if t.elapsed() >= Duration::from_millis(400) {
                                        state.tmdb_ui.last_dispatched_query = q.clone();
                                        state.tmdb_ui.last_input_at = None;
                                        actions.push(UiAction::TmdbSearch(q));
                                    } else {
                                        ctx.request_repaint_after(Duration::from_millis(100));
                                    }
                                }
                            }

                            if state.tmdb_ui.search_busy {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("TMDB...")
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                            }
                            if let Some(err) = &state.tmdb_ui.search_error {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(err)
                                        .font(font_body(12.5))
                                        .color(p.danger),
                                );
                            }

                            let results = state.tmdb_ui.search_results.clone();
                            if !results.is_empty() {
                                ui.add_space(10.0);
                                let list_h = (screen.height() - 210.0).clamp(260.0, 500.0);
                                let desired_h = ((results.len() as f32) * 90.0).clamp(96.0, list_h);
                                egui::ScrollArea::vertical()
                                    .id_salt("tmdb_search_scroll")
                                    .auto_shrink([false, false])
                                    .min_scrolled_height(desired_h)
                                    .max_height(list_h)
                                    .show(ui, |ui| {
                                        for item in results {
                                            let is_dup = state.tmdb_ui.pending_add_keys.contains(&item.tmdb_id)
                                                || is_media_duplicate(
                                                    &state.doc,
                                                    &item.tmdb_id,
                                                    &item.title,
                                                    item.year,
                                                );
                                            if let Some(status_opt) = render_tmdb_result_row(
                                                ctx,
                                                ui,
                                                p,
                                                lang,
                                                cover_cache,
                                                &item,
                                                is_dup,
                                            ) {
                                                state.tmdb_ui.pending_add_keys.insert(item.tmdb_id.clone());
                                                actions.push(UiAction::TmdbAddMedia {
                                                    item,
                                                    status_opt: status_opt.to_string(),
                                                });
                                            }
                                            ui.add_space(6.0);
                                        }
                                    });
                            } else if !state.tmdb_ui.search_busy
                                && state.tmdb_ui.search_error.is_none()
                                && !state.tmdb_ui.last_dispatched_query.is_empty()
                                && state.tmdb_ui.last_dispatched_query == state.tmdb_ui.search_query.trim()
                            {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(s.media_no_results)
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                            }
                        }
                        MediaAddTab::Books => {
                            let resp = styled_text_input(
                                ui,
                                p,
                                &mut state.tmdb_ui.book_query,
                                s.media_book_search_hint,
                                false,
                                modal_w,
                            );
                            if state.tmdb_ui.search_focus_requested {
                                resp.request_focus();
                                state.tmdb_ui.search_focus_requested = false;
                            }
                            if resp.changed() {
                                state.tmdb_ui.book_last_input_at = Some(Instant::now());
                            }
                            let enter = (resp.has_focus() || resp.lost_focus())
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if enter && !state.tmdb_ui.book_query.trim().is_empty() {
                                let q = state.tmdb_ui.book_query.trim().to_string();
                                state.tmdb_ui.last_dispatched_book_query = q.clone();
                                state.tmdb_ui.book_last_input_at = None;
                                actions.push(UiAction::BookSearch(q));
                            }

                            if let Some(t) = state.tmdb_ui.book_last_input_at {
                                let q = state.tmdb_ui.book_query.trim().to_string();
                                if q.chars().count() >= 2 && q != state.tmdb_ui.last_dispatched_book_query {
                                    if t.elapsed() >= Duration::from_millis(400) {
                                        state.tmdb_ui.last_dispatched_book_query = q.clone();
                                        state.tmdb_ui.book_last_input_at = None;
                                        actions.push(UiAction::BookSearch(q));
                                    } else {
                                        ctx.request_repaint_after(Duration::from_millis(100));
                                    }
                                }
                            }

                            if state.tmdb_ui.book_search_busy {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("OpenLibrary...")
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                            }
                            if let Some(err) = &state.tmdb_ui.book_search_error {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(err)
                                        .font(font_body(12.5))
                                        .color(p.danger),
                                );
                            }

                            let books = state.tmdb_ui.book_search_results.clone();
                            if !books.is_empty() {
                                ui.add_space(10.0);
                                let list_h = (screen.height() - 210.0).clamp(260.0, 500.0);
                                let desired_h = ((books.len() as f32) * 90.0).clamp(96.0, list_h);
                                egui::ScrollArea::vertical()
                                    .id_salt("openlib_search_scroll")
                                    .auto_shrink([false, false])
                                    .min_scrolled_height(desired_h)
                                    .max_height(list_h)
                                    .show(ui, |ui| {
                                        for item in books {
                                            let is_dup = state.tmdb_ui.pending_add_keys.contains(&item.external_id)
                                                || is_media_duplicate(
                                                    &state.doc,
                                                    &item.external_id,
                                                    &item.title,
                                                    item.year,
                                                );
                                            if let Some(status_opt) = render_book_result_row(
                                                ctx,
                                                ui,
                                                p,
                                                s,
                                                cover_cache,
                                                &item,
                                                is_dup,
                                            ) {
                                                state.tmdb_ui.pending_add_keys.insert(item.external_id.clone());
                                                actions.push(UiAction::AddBookMedia {
                                                    item,
                                                    status_opt: status_opt.to_string(),
                                                });
                                            }
                                            ui.add_space(6.0);
                                        }
                                    });
                            } else if !state.tmdb_ui.book_search_busy
                                && state.tmdb_ui.book_search_error.is_none()
                                && !state.tmdb_ui.last_dispatched_book_query.is_empty()
                                && state.tmdb_ui.last_dispatched_book_query == state.tmdb_ui.book_query.trim()
                            {
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(s.media_no_results)
                                        .font(font_body(12.5))
                                        .color(p.text_muted),
                                );
                            }
                        }
                        MediaAddTab::Manual => {
                            let types = [
                                ("movie", s.media_cat_movies),
                                ("tv", s.media_cat_tv),
                                ("book", s.media_cat_books),
                                ("game", if is_ru { "Игра" } else { "Game" }),
                                ("course", if is_ru { "Курс" } else { "Course" }),
                                ("other", s.media_cat_other),
                            ];
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = vec2(6.0, 5.0);
                                for (tid, lbl) in types {
                                    let active = state.tmdb_ui.manual_type == tid;
                                    let btn = egui::Button::new(
                                        egui::RichText::new(lbl)
                                            .font(font_medium(11.5))
                                            .color(if active { p.accent_text } else { p.text_secondary }),
                                    )
                                    .fill(if active { p.accent_soft } else { p.bg_sidebar })
                                    .stroke(Stroke::new(1.0_f32, if active { p.accent } else { p.border_hairline }))
                                    .rounding(Rounding::same(12.0));
                                    if ui.add(btn).clicked() {
                                        state.tmdb_ui.manual_type = tid.to_string();
                                    }
                                }
                            });
                            ui.add_space(10.0);

                            let creator_lbl = media_creator_label(&state.tmdb_ui.manual_type, s);
                            let cover_preview_url = state.tmdb_ui.manual_cover_url.trim().to_string();
                            ui.horizontal_top(|ui| {
                                let left_w = (modal_w - 110.0).max(200.0);
                                ui.allocate_ui_with_layout(
                                    vec2(left_w, 136.0),
                                    Layout::top_down(Align::LEFT),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.y = 8.0;
                                        let r_title = styled_text_input(
                                            ui,
                                            p,
                                            &mut state.tmdb_ui.manual_title,
                                            s.media_manual_title,
                                            false,
                                            left_w,
                                        );
                                        if state.tmdb_ui.search_focus_requested {
                                            r_title.request_focus();
                                            state.tmdb_ui.search_focus_requested = false;
                                        }
                                        ui.horizontal(|ui| {
                                            styled_text_input(
                                                ui,
                                                p,
                                                &mut state.tmdb_ui.manual_year,
                                                s.media_manual_year,
                                                false,
                                                88.0,
                                            );
                                            styled_text_input(
                                                ui,
                                                p,
                                                &mut state.tmdb_ui.manual_creator,
                                                creator_lbl,
                                                false,
                                                (left_w - 96.0).max(100.0),
                                            );
                                        });
                                        styled_text_input(
                                            ui,
                                            p,
                                            &mut state.tmdb_ui.manual_cover_url,
                                            s.media_manual_cover_url,
                                            false,
                                            left_w,
                                        );
                                    },
                                );
                                ui.add_space(12.0);
                                render_poster_box(ctx, ui, p, cover_cache, &cover_preview_url, vec2(92.0, 138.0), 8.0);
                            });

                            ui.add_space(10.0);
                            let can_add = !state.tmdb_ui.manual_title.trim().is_empty();
                            let mut add_status: Option<&str> = None;
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        can_add,
                                        egui::Button::new(
                                            egui::RichText::new(format!("+ {}", s.media_btn_watchlist))
                                                .font(font_medium(12.5))
                                                .color(if can_add { p.accent_on_fill } else { p.text_muted }),
                                        )
                                        .fill(if can_add { p.accent } else { p.bg_input })
                                        .rounding(Rounding::same(7.0))
                                        .min_size(vec2(104.0, 30.0)),
                                    )
                                    .clicked()
                                {
                                    add_status = Some("watchlist");
                                }
                                if ui
                                    .add_enabled(
                                        can_add,
                                        egui::Button::new(
                                            egui::RichText::new(format!("✓ {}", s.media_btn_watched))
                                                .font(font_medium(12.5))
                                                .color(if can_add { p.text_primary } else { p.text_muted }),
                                        )
                                        .fill(p.bg_sidebar)
                                        .stroke(Stroke::new(1.0_f32, p.border_subtle))
                                        .rounding(Rounding::same(7.0))
                                        .min_size(vec2(104.0, 30.0)),
                                    )
                                    .clicked()
                                {
                                    add_status = Some("watched");
                                }
                            });
                            if let Some(status_opt) = add_status {
                                let title = state.tmdb_ui.manual_title.trim().to_string();
                                let mt = state.tmdb_ui.manual_type.clone();
                                ensure_media_type_option(&mut state.doc, &mt, is_ru);
                                let rec_id = state.doc.add_record(
                                    &title,
                                    Some(("status", status_opt)),
                                    chrono::Utc::now().timestamp(),
                                );
                                if let Some(rec) = state.doc.records.iter_mut().find(|r| r.id == rec_id) {
                                    rec.values.insert("media_type".into(), FieldValue::Text(mt));
                                    if let Ok(y) = state.tmdb_ui.manual_year.trim().parse::<f64>() {
                                        if y > 0.0 {
                                            rec.values.insert("year".into(), FieldValue::Number(y));
                                        }
                                    }
                                    if !state.tmdb_ui.manual_creator.trim().is_empty() {
                                        rec.values.insert(
                                            "director".into(),
                                            FieldValue::Text(state.tmdb_ui.manual_creator.trim().to_string()),
                                        );
                                    }
                                    if !cover_preview_url.is_empty() {
                                        rec.values.insert(
                                            "poster_path".into(),
                                            FieldValue::Text(cover_preview_url),
                                        );
                                    }
                                }
                                actions.push(UiAction::MarkCollectionDirty);
                                state.tmdb_ui.manual_title.clear();
                                state.tmdb_ui.manual_year.clear();
                                state.tmdb_ui.manual_creator.clear();
                                state.tmdb_ui.manual_cover_url.clear();
                                close = true;
                            }
                        }
                    }
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
                                let is_dup = state.tmdb_ui.pending_add_keys.contains(&item.tmdb_id)
                                    || is_media_duplicate(&state.doc, &item.tmdb_id, &item.title, item.year);
                                if let Some(status_opt) = render_tmdb_result_row(
                                    ctx,
                                    ui,
                                    p,
                                    lang,
                                    cover_cache,
                                    &item,
                                    is_dup,
                                ) {
                                    remove_id = Some(item.tmdb_id.clone());
                                    actions.push(UiAction::TmdbAddMedia {
                                        item,
                                        status_opt: status_opt.to_string(),
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
    is_duplicate: bool,
) -> Option<&'static str> {
    let s = strings(lang);
    let mut picked_status: Option<&'static str> = None;
    egui::Frame::none()
        .fill(p.bg_sidebar)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row_inner_w = ui.available_width();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let (thumb_rect, _) = ui.allocate_exact_size(vec2(42.0, 62.0), Sense::hover());
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
                let right_col_w = if is_duplicate { 118.0 } else { 196.0 };
                let text_w = (row_inner_w - 42.0 - 10.0 - 10.0 - right_col_w).max(140.0);
                ui.allocate_ui_with_layout(
                    vec2(text_w, 62.0),
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
                                    egui::RichText::new(truncate_chars(&item.overview, 135))
                                        .font(font_body(11.5))
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        }
                    },
                );

                ui.add_space(10.0);
                ui.allocate_ui_with_layout(
                    vec2(right_col_w, 62.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if is_duplicate {
                            render_in_library_badge(ui, p, s.media_in_library);
                        } else {
                            if render_spotlight_action_btn(
                                ui,
                                p,
                                &format!("✓ {}", s.media_btn_watched),
                                false,
                            )
                            .clicked()
                            {
                                picked_status = Some("watched");
                            }
                            if render_spotlight_action_btn(
                                ui,
                                p,
                                &format!("+ {}", s.media_btn_watchlist),
                                true,
                            )
                            .clicked()
                            {
                                picked_status = Some("watchlist");
                            }
                        }
                    },
                );
            });
        });
    picked_status
}

fn render_book_result_row(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    s: &crate::i18n::Strings,
    cover_cache: &mut CoverCache,
    item: &BookSearchItem,
    is_duplicate: bool,
) -> Option<&'static str> {
    let mut picked_status: Option<&'static str> = None;
    egui::Frame::none()
        .fill(p.bg_sidebar)
        .stroke(Stroke::new(1.0_f32, p.border_hairline))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let row_inner_w = ui.available_width();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let (thumb_rect, _) = ui.allocate_exact_size(vec2(42.0, 62.0), Sense::hover());
                let lookup = item
                    .cover_url
                    .as_deref()
                    .map(|url| cover_cache.get_or_request(ctx, url))
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
                        draw_icon(ui.painter(), ir, Icon::Document, p.text_muted);
                    }
                }

                ui.add_space(10.0);
                let right_col_w = if is_duplicate { 118.0 } else { 196.0 };
                let text_w = (row_inner_w - 42.0 - 10.0 - 10.0 - right_col_w).max(140.0);
                ui.allocate_ui_with_layout(
                    vec2(text_w, 62.0),
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
                        });
                        if !item.author.trim().is_empty() {
                            ui.label(
                                egui::RichText::new(&item.author)
                                    .font(font_medium(11.5))
                                    .color(p.accent_text),
                            );
                        }
                        if !item.summary.trim().is_empty() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(truncate_chars(&item.summary, 120))
                                        .font(font_body(11.5))
                                        .color(p.text_secondary),
                                )
                                .wrap(),
                            );
                        }
                    },
                );

                ui.add_space(10.0);
                ui.allocate_ui_with_layout(
                    vec2(right_col_w, 62.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if is_duplicate {
                            render_in_library_badge(ui, p, s.media_in_library);
                        } else {
                            if render_spotlight_action_btn(
                                ui,
                                p,
                                &format!("✓ {}", s.media_btn_watched),
                                false,
                            )
                            .clicked()
                            {
                                picked_status = Some("watched");
                            }
                            if render_spotlight_action_btn(
                                ui,
                                p,
                                &format!("+ {}", s.media_btn_watchlist),
                                true,
                            )
                            .clicked()
                            {
                                picked_status = Some("watchlist");
                            }
                        }
                    },
                );
            });
        });
    picked_status
}

fn render_in_library_badge(ui: &mut Ui, p: &ThemePalette, label: &str) {
    let fg = if p.is_dark {
        Color32::from_rgb(74, 222, 128)
    } else {
        Color32::from_rgb(22, 163, 74)
    };
    let bg = Color32::from_rgba_unmultiplied(34, 197, 94, 34);
    let border = Color32::from_rgba_unmultiplied(34, 197, 94, 76);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_medium(12.0), fg);
    let size = vec2(galley.size().x + 20.0, 25.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(6.0), bg);
    ui.painter()
        .rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, border));
    let text_pos = pos2(
        rect.center().x - galley.size().x * 0.5,
        rect.center().y - galley.size().y * 0.5,
    );
    ui.painter().galley(text_pos, galley, fg);
}

fn render_spotlight_action_btn(
    ui: &mut Ui,
    p: &ThemePalette,
    label: &str,
    accent: bool,
) -> egui::Response {
    let fg = if accent { p.accent_on_fill } else { p.text_primary };
    let bg = if accent { p.accent } else { p.bg_elevated };
    let stroke = if accent {
        Stroke::NONE
    } else {
        Stroke::new(1.0_f32, p.border_subtle)
    };
    ui.add(
        egui::Button::new(egui::RichText::new(label).font(font_medium(12.0)).color(fg))
            .fill(bg)
            .stroke(stroke)
            .rounding(Rounding::same(6.0))
            .min_size(vec2(0.0, 27.0)),
    )
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
    kb_selected: bool,
    scroll_into_view: bool,
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

    if scroll_into_view {
        resp.response.scroll_to_me(Some(Align::Center));
    }
    if kb_selected {
        ui.painter().rect_stroke(
            resp.response.rect,
            Rounding::same(10.0),
            Stroke::new(2.0_f32, p.accent),
        );
    } else if resp.response.hovered() {
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

pub(crate) fn render_gallery_sort_picker(
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

pub(crate) fn record_matches_media_category(rec: &RecordItem, category: Option<&str>) -> bool {
    let Some(cat) = category.map(str::trim).filter(|c| !c.is_empty()) else {
        return true;
    };
    let mt = rec
        .values
        .get("media_type")
        .map(FieldValue::as_text)
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    match cat {
        "movie" | "tv" | "book" => mt == cat,
        "other" => mt != "movie" && mt != "tv" && mt != "book",
        _ => true,
    }
}

pub(crate) fn is_media_duplicate(
    doc: &CollectionDoc,
    external_id: &str,
    title: &str,
    year: Option<i32>,
) -> bool {
    let ext = external_id.trim();
    let title_low = title.trim().to_lowercase();
    doc.records.iter().any(|rec| {
        if !ext.is_empty()
            && rec
                .values
                .get("tmdb_id")
                .map(FieldValue::as_text)
                .is_some_and(|v| v.trim() == ext)
        {
            return true;
        }
        if !title_low.is_empty() && rec.title.trim().to_lowercase() == title_low {
            return match (
                year,
                rec.values
                    .get("year")
                    .and_then(FieldValue::as_number)
                    .map(|n| n.round() as i32),
            ) {
                (Some(a), Some(b)) => a == b,
                (None, _) | (_, None) => true,
            };
        }
        false
    })
}

pub(crate) fn ensure_media_type_option(doc: &mut CollectionDoc, opt_id: &str, is_ru: bool) {
    let Some(col) = doc.columns.iter_mut().find(|c| c.id == "media_type") else {
        return;
    };
    if col.options.iter().any(|o| o.id == opt_id) {
        return;
    }
    let (label, color) = match opt_id {
        "book" => (if is_ru { "Книга" } else { "Book" }, ColorToken::Teal),
        "game" => (if is_ru { "Игра" } else { "Game" }, ColorToken::Amber),
        "course" => (if is_ru { "Курс" } else { "Course" }, ColorToken::Green),
        "other" => (if is_ru { "Другое" } else { "Other" }, ColorToken::Slate),
        _ => (opt_id, ColorToken::Slate),
    };
    col.options.push(SelectOption {
        id: opt_id.to_string(),
        label: label.to_string(),
        color,
    });
}

pub(crate) fn collect_normalized_genres(doc: &CollectionDoc, is_ru: bool) -> Vec<String> {
    let Some(gcol) = find_col_id(doc, &["genres", "genre", "жанр"]) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for rec in &doc.records {
        if let Some(val) = rec.values.get(&gcol) {
            for part in val.as_text().split([',', ';', '/']) {
                let g = krono_core::collection::normalize_genre_label(part, is_ru);
                if !g.is_empty() && !out.iter().any(|x| x.to_lowercase() == g.to_lowercase()) {
                    out.push(g);
                }
            }
        }
    }
    out.sort_by_key(|a| a.to_lowercase());
    out
}

pub(crate) fn pick_random_watchlist_record(
    state: &CollectionEditorState,
    today: SimpleDate,
    is_ru: bool,
) -> Option<String> {
    let genres_col = find_col_id(&state.doc, &["genres", "genre", "жанр"]);
    let q_lower = state.filter_query.trim().to_lowercase();
    let min_num_f = state.min_number_filter.as_ref().map(|(c, v)| (c.as_str(), *v));
    let genre_f = state.genre_filter.as_deref();
    let cat_f = state.media_category_filter.as_deref();
    let status_col = state.doc.effective_group_column_id().unwrap_or("status");

    let candidates: Vec<String> = state
        .doc
        .records
        .iter()
        .filter(|rec| {
            let is_watchlist = rec
                .values
                .get(status_col)
                .map(FieldValue::as_text)
                .is_some_and(|v| v.trim() == "watchlist");
            if !is_watchlist {
                return false;
            }
            if !record_matches_smart_filter(
                rec,
                &state.doc.columns,
                &q_lower,
                state.date_filter,
                state.tag_filter.as_deref(),
                None,
                today,
            ) || !record_matches_min_number(rec, min_num_f)
                || !record_matches_media_category(rec, cat_f)
            {
                return false;
            }
            if let Some(gf) = genre_f {
                let Some(gcol) = genres_col.as_deref() else {
                    return false;
                };
                let gf_low = gf.trim().to_lowercase();
                return rec.values.get(gcol).is_some_and(|v| {
                    v.as_text().split([',', ';', '/']).any(|part| {
                        krono_core::collection::normalize_genre_label(part, is_ru).to_lowercase()
                            == gf_low
                    })
                });
            }
            true
        })
        .map(|r| r.id.clone())
        .collect();

    if candidates.is_empty() {
        return None;
    }
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    let mut idx = nanos % candidates.len();
    if state.last_random_pick_id.as_deref() == Some(candidates[idx].as_str()) {
        idx = (idx + 1) % candidates.len();
    }
    Some(candidates[idx].clone())
}

pub(crate) fn media_creator_label(media_type: &str, s: &crate::i18n::Strings) -> &'static str {
    match media_type.trim() {
        "book" => s.media_creator_author,
        "movie" => s.media_creator_director,
        "game" => s.media_creator_studio,
        "course" => s.media_creator_course,
        _ => s.media_manual_creator,
    }
}

pub(crate) fn render_poster_box(
    ctx: &egui::Context,
    ui: &mut Ui,
    p: &ThemePalette,
    cover_cache: &mut CoverCache,
    cover_url: &str,
    size: egui::Vec2,
    rounding: f32,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let round = Rounding::same(rounding);
    match cover_cache.get_or_request(ctx, cover_url) {
        CoverLookup::Ready(tex_id) => {
            let tex_size = ctx
                .tex_manager()
                .read()
                .meta(tex_id)
                .map(|m| m.size)
                .unwrap_or([2, 3]);
            let uv = aspect_fill_uv(tex_size, rect.size());
            ui.painter().image(tex_id, rect, uv, Color32::WHITE);
            ui.painter()
                .rect_stroke(rect, round, Stroke::new(1.0_f32, p.border_subtle));
        }
        _ => {
            ui.painter().rect_filled(rect, round, p.bg_sidebar);
            ui.painter()
                .rect_stroke(rect, round, Stroke::new(1.0_f32, p.border_hairline));
            let ir = Rect::from_center_size(rect.center(), vec2(24.0, 24.0));
            draw_icon(ui.painter(), ir, Icon::Image, p.text_muted);
        }
    }
    resp
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
    let bg = Color32::from_rgba_unmultiplied(
        base.r(),
        base.g(),
        base.b(),
        if p.is_dark { 36 } else { 30 },
    );
    let fg = lerp_color(base, p.text_secondary, 0.25);
    (bg, fg)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_media_categories_and_other() {
        let mut doc = CollectionDoc::default();
        let m1 = doc.add_record("Dune", None, 1);
        let b1 = doc.add_record("Dune Book", None, 2);
        let g1 = doc.add_record("Elden Ring", None, 3);
        doc.records[0].values.insert("media_type".into(), FieldValue::Text("movie".into()));
        doc.records[1].values.insert("media_type".into(), FieldValue::Text("book".into()));
        doc.records[2].values.insert("media_type".into(), FieldValue::Text("game".into()));
        let _ = (m1, b1, g1);

        assert!(record_matches_media_category(&doc.records[0], None));
        assert!(record_matches_media_category(&doc.records[0], Some("movie")));
        assert!(!record_matches_media_category(&doc.records[0], Some("book")));
        assert!(record_matches_media_category(&doc.records[1], Some("book")));
        assert!(record_matches_media_category(&doc.records[2], Some("other")));
        assert!(!record_matches_media_category(&doc.records[0], Some("other")));
    }

    #[test]
    fn detects_media_duplicates_by_id_or_title_and_year() {
        let mut doc = CollectionDoc::default();
        let id = doc.add_record("Inception", None, 1);
        if let Some(rec) = doc.records.iter_mut().find(|r| r.id == id) {
            rec.values.insert("tmdb_id".into(), FieldValue::Text("27205".into()));
            rec.values.insert("year".into(), FieldValue::Number(2010.0));
        }
        assert!(is_media_duplicate(&doc, "27205", "Начало", Some(2010)));
        assert!(is_media_duplicate(&doc, "other", "inception", Some(2010)));
        assert!(!is_media_duplicate(&doc, "other", "inception", Some(1999)));
    }
}
