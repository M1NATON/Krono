use crate::i18n::Lang;
use eframe::egui;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";
const TMDB_IMAGE_W342_BASE: &str = "https://image.tmdb.org/t/p/w342";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TmdbMediaKind {
    Movie,
    Tv,
}

impl TmdbMediaKind {
    pub fn as_select_opt(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Tv => "tv",
        }
    }

    pub fn from_select_opt(s: &str) -> Option<Self> {
        match s.trim() {
            "movie" => Some(Self::Movie),
            "tv" => Some(Self::Tv),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TmdbSearchItem {
    pub tmdb_id: String,
    pub kind: TmdbMediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub vote_average: Option<f64>,
    pub poster_path: Option<String>,
    pub overview: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TmdbMediaDetails {
    pub item: TmdbSearchItem,
    pub genres: Vec<String>,
    pub director: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BookSearchItem {
    pub external_id: String,
    pub title: String,
    pub author: String,
    pub year: Option<i32>,
    pub cover_url: Option<String>,
    pub summary: String,
}

pub enum TmdbEvent {
    SearchDone {
        query: String,
        result: Result<Vec<TmdbSearchItem>, String>,
    },
    BookSearchDone {
        query: String,
        result: Result<Vec<BookSearchItem>, String>,
    },
    DetailsDone {
        target_status: String,
        fallback: TmdbSearchItem,
        result: Result<TmdbMediaDetails, String>,
    },
    RecommendationsDone(Result<Vec<TmdbSearchItem>, String>),
}

pub fn poster_w342_url(poster_path: &str) -> String {
    let trimmed = poster_path.trim();
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return trimmed.to_string();
    }
    let normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    format!("{TMDB_IMAGE_W342_BASE}{normalized}")
}

pub fn is_v4_bearer_token(token: &str) -> bool {
    let trimmed = token.trim();
    trimmed.contains('.') && trimmed.len() > 60
}

pub fn encode_query_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

pub fn deduplicate_recommendations(
    items: Vec<TmdbSearchItem>,
    existing_tmdb_ids: &HashSet<String>,
) -> Vec<TmdbSearchItem> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut unique: HashMap<String, TmdbSearchItem> = HashMap::new();

    for item in items {
        if item.tmdb_id.trim().is_empty() || existing_tmdb_ids.contains(&item.tmdb_id) {
            continue;
        }
        *counts.entry(item.tmdb_id.clone()).or_insert(0) += 1;
        unique.entry(item.tmdb_id.clone()).or_insert(item);
    }

    let mut result: Vec<TmdbSearchItem> = unique.into_values().collect();
    result.sort_by(|a, b| {
        let ca = counts.get(&a.tmdb_id).copied().unwrap_or(0);
        let cb = counts.get(&b.tmdb_id).copied().unwrap_or(0);
        cb.cmp(&ca).then_with(|| {
            let va = a.vote_average.unwrap_or(0.0);
            let vb = b.vote_average.unwrap_or(0.0);
            vb.partial_cmp(&va).unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    result.truncate(24);
    result
}

pub fn spawn_search_multi(
    ctx: egui::Context,
    tx: mpsc::Sender<TmdbEvent>,
    token: String,
    query: String,
    lang: Lang,
    tmdb_lang: &'static str,
) {
    let _ = thread::Builder::new()
        .name("krono-tmdb-search".into())
        .spawn(move || {
            let res = search_multi_sync(&token, &query, lang, tmdb_lang);
            let _ = tx.send(TmdbEvent::SearchDone {
                query,
                result: res,
            });
            ctx.request_repaint();
        });
}

pub fn spawn_fetch_details(
    ctx: egui::Context,
    tx: mpsc::Sender<TmdbEvent>,
    token: String,
    item: TmdbSearchItem,
    target_status: String,
    lang: Lang,
    tmdb_lang: &'static str,
) {
    let _ = thread::Builder::new()
        .name("krono-tmdb-details".into())
        .spawn(move || {
            let res = fetch_details_sync(&token, &item, lang, tmdb_lang);
            let _ = tx.send(TmdbEvent::DetailsDone {
                target_status,
                fallback: item,
                result: res,
            });
            ctx.request_repaint();
        });
}

pub fn spawn_fetch_recommendations(
    ctx: egui::Context,
    tx: mpsc::Sender<TmdbEvent>,
    token: String,
    seeds: Vec<(TmdbMediaKind, String)>,
    existing_ids: HashSet<String>,
    lang: Lang,
    tmdb_lang: &'static str,
) {
    let _ = thread::Builder::new()
        .name("krono-tmdb-recs".into())
        .spawn(move || {
            let res = fetch_recommendations_sync(&token, &seeds, &existing_ids, lang, tmdb_lang);
            let _ = tx.send(TmdbEvent::RecommendationsDone(res));
            ctx.request_repaint();
        });
}

pub fn spawn_search_books(
    ctx: egui::Context,
    tx: mpsc::Sender<TmdbEvent>,
    query: String,
    lang: Lang,
) {
    let _ = thread::Builder::new()
        .name("krono-openlib-search".into())
        .spawn(move || {
            let res = search_books_sync(&query, lang);
            let _ = tx.send(TmdbEvent::BookSearchDone {
                query,
                result: res,
            });
            ctx.request_repaint();
        });
}

fn build_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build()
}

fn tmdb_get_json<T: serde::de::DeserializeOwned>(
    agent: &ureq::Agent,
    path_and_query: &str,
    token: &str,
    lang: Lang,
) -> Result<T, String> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(if lang == Lang::Ru {
            "TMDB ключ не настроен".into()
        } else {
            "TMDB API key is not configured".into()
        });
    }
    let sep = if path_and_query.contains('?') { '&' } else { '?' };
    let url = if is_v4_bearer_token(trimmed) {
        format!("{TMDB_API_BASE}{path_and_query}")
    } else {
        format!(
            "{TMDB_API_BASE}{path_and_query}{sep}api_key={}",
            encode_query_component(trimmed)
        )
    };
    let mut req = agent.get(&url).set("Accept", "application/json");
    if is_v4_bearer_token(trimmed) {
        req = req.set("Authorization", &format!("Bearer {trimmed}"));
    }
    let resp = req.call().map_err(|e| format_ureq_error(e, lang))?;
    resp.into_json::<T>()
        .map_err(|e| format!("TMDB parse error: {e}"))
}

fn format_ureq_error(err: ureq::Error, lang: Lang) -> String {
    match err {
        ureq::Error::Status(401, _) => {
            if lang == Lang::Ru {
                "Неверный ключ TMDB (401). Проверьте Настройки → Интеграции.".into()
            } else {
                "Invalid TMDB key (401). Check Settings → Integrations.".into()
            }
        }
        ureq::Error::Status(code, _) => {
            if lang == Lang::Ru {
                format!("Ошибка TMDB API (HTTP {code})")
            } else {
                format!("TMDB API error (HTTP {code})")
            }
        }
        ureq::Error::Transport(t) => {
            if lang == Lang::Ru {
                format!("Сеть TMDB недоступна: {t}")
            } else {
                format!("TMDB network error: {t}")
            }
        }
    }
}

#[derive(Deserialize)]
struct SearchMultiResp {
    #[serde(default)]
    results: Vec<RawSearchResult>,
}

#[derive(Deserialize)]
struct RawSearchResult {
    id: i64,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    first_air_date: Option<String>,
    #[serde(default)]
    vote_average: Option<f64>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    overview: Option<String>,
}

impl RawSearchResult {
    fn into_item(self, forced_kind: Option<TmdbMediaKind>) -> Option<TmdbSearchItem> {
        let kind = match forced_kind {
            Some(k) => k,
            None => match self.media_type.as_deref() {
                Some("movie") => TmdbMediaKind::Movie,
                Some("tv") => TmdbMediaKind::Tv,
                _ => return None,
            },
        };
        let title = self
            .title
            .or(self.name)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())?;
        let date_str = self.release_date.or(self.first_air_date).unwrap_or_default();
        let year = date_str
            .split('-')
            .next()
            .and_then(|y| y.parse::<i32>().ok());
        Some(TmdbSearchItem {
            tmdb_id: self.id.to_string(),
            kind,
            title,
            year,
            vote_average: self.vote_average.filter(|v| *v > 0.0),
            poster_path: self.poster_path.filter(|p| !p.trim().is_empty()),
            overview: self.overview.unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct RawDetailsResp {
    #[serde(default)]
    genres: Vec<RawGenre>,
    #[serde(default)]
    created_by: Vec<RawPerson>,
    #[serde(default)]
    credits: Option<RawCredits>,
}

#[derive(Deserialize)]
struct RawGenre {
    name: String,
}

#[derive(Deserialize)]
struct RawCredits {
    #[serde(default)]
    crew: Vec<RawCrew>,
}

#[derive(Deserialize)]
struct RawCrew {
    name: String,
    #[serde(default)]
    job: String,
}

#[derive(Deserialize)]
struct RawPerson {
    name: String,
}

fn search_multi_sync(
    token: &str,
    query: &str,
    lang: Lang,
    tmdb_lang: &str,
) -> Result<Vec<TmdbSearchItem>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let agent = build_agent();
    let path = format!(
        "/search/multi?query={}&language={}&include_adult=false",
        encode_query_component(trimmed),
        tmdb_lang
    );
    let resp: SearchMultiResp = tmdb_get_json(&agent, &path, token, lang)?;
    Ok(resp
        .results
        .into_iter()
        .filter_map(|r| r.into_item(None))
        .take(20)
        .collect())
}

fn fetch_details_sync(
    token: &str,
    item: &TmdbSearchItem,
    lang: Lang,
    tmdb_lang: &str,
) -> Result<TmdbMediaDetails, String> {
    let agent = build_agent();
    let kind_seg = match item.kind {
        TmdbMediaKind::Movie => "movie",
        TmdbMediaKind::Tv => "tv",
    };
    let path = format!(
        "/{kind_seg}/{}?language={}&append_to_response=credits",
        encode_query_component(&item.tmdb_id),
        tmdb_lang
    );
    let details: RawDetailsResp = tmdb_get_json(&agent, &path, token, lang)?;
    let genres: Vec<String> = details
        .genres
        .into_iter()
        .map(|g| g.name.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();
    let director = match item.kind {
        TmdbMediaKind::Movie => details
            .credits
            .and_then(|c| {
                c.crew
                    .into_iter()
                    .find(|p| p.job.eq_ignore_ascii_case("Director"))
                    .map(|p| p.name)
            })
            .unwrap_or_default(),
        TmdbMediaKind::Tv => details
            .created_by
            .into_iter()
            .map(|p| p.name)
            .find(|n| !n.trim().is_empty())
            .unwrap_or_default(),
    };

    Ok(TmdbMediaDetails {
        item: item.clone(),
        genres,
        director,
    })
}

fn fetch_recommendations_sync(
    token: &str,
    seeds: &[(TmdbMediaKind, String)],
    existing_ids: &HashSet<String>,
    lang: Lang,
    tmdb_lang: &str,
) -> Result<Vec<TmdbSearchItem>, String> {
    if seeds.is_empty() {
        return Ok(Vec::new());
    }
    let agent = build_agent();
    let mut collected = Vec::new();
    let mut last_err: Option<String> = None;

    for (kind, id) in seeds.iter().take(5) {
        let kind_seg = match kind {
            TmdbMediaKind::Movie => "movie",
            TmdbMediaKind::Tv => "tv",
        };
        let path = format!(
            "/{kind_seg}/{}/recommendations?language={}",
            encode_query_component(id),
            tmdb_lang
        );
        match tmdb_get_json::<SearchMultiResp>(&agent, &path, token, lang) {
            Ok(resp) => {
                for r in resp.results.into_iter().take(10) {
                    if let Some(item) = r.into_item(Some(*kind)) {
                        collected.push(item);
                    }
                }
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    if collected.is_empty() {
        if let Some(err) = last_err {
            return Err(err);
        }
    }
    Ok(deduplicate_recommendations(collected, existing_ids))
}

pub struct TmdbGenre {
    pub id: u32,
    pub movie_id: u32,
    pub tv_id: u32,
    pub label_ru: &'static str,
    pub label_en: &'static str,
}

pub const TMDB_GENRES: &[TmdbGenre] = &[
    TmdbGenre { id: 1, movie_id: 878, tv_id: 10765, label_ru: "Фантастика", label_en: "Sci-Fi" },
    TmdbGenre { id: 2, movie_id: 53, tv_id: 9648, label_ru: "Триллер", label_en: "Thriller" },
    TmdbGenre { id: 3, movie_id: 18, tv_id: 18, label_ru: "Драма", label_en: "Drama" },
    TmdbGenre { id: 4, movie_id: 35, tv_id: 35, label_ru: "Комедия", label_en: "Comedy" },
    TmdbGenre { id: 5, movie_id: 28, tv_id: 10759, label_ru: "Боевик", label_en: "Action" },
    TmdbGenre { id: 6, movie_id: 80, tv_id: 80, label_ru: "Криминал", label_en: "Crime" },
    TmdbGenre { id: 7, movie_id: 16, tv_id: 16, label_ru: "Анимация / Аниме", label_en: "Animation / Anime" },
    TmdbGenre { id: 8, movie_id: 14, tv_id: 10765, label_ru: "Фэнтези", label_en: "Fantasy" },
    TmdbGenre { id: 9, movie_id: 9648, tv_id: 9648, label_ru: "Детектив", label_en: "Mystery" },
    TmdbGenre { id: 10, movie_id: 99, tv_id: 99, label_ru: "Документальное", label_en: "Documentary" },
];

pub fn resolve_tmdb_genre_ids(kind: TmdbMediaKind, selected_keys: &[u32]) -> Vec<u32> {
    let mut ids = Vec::new();
    for key in selected_keys {
        if let Some(g) = TMDB_GENRES.iter().find(|g| g.id == *key) {
            let gid = match kind {
                TmdbMediaKind::Movie => g.movie_id,
                TmdbMediaKind::Tv => g.tv_id,
            };
            if !ids.contains(&gid) {
                ids.push(gid);
            }
        }
    }
    ids
}

pub fn build_discover_path(kind: TmdbMediaKind, tmdb_genre_ids: &[u32], tmdb_lang: &str) -> String {
    let kind_seg = match kind {
        TmdbMediaKind::Movie => "movie",
        TmdbMediaKind::Tv => "tv",
    };
    let joined = tmdb_genre_ids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if joined.is_empty() {
        format!(
            "/discover/{kind_seg}?sort_by=vote_average.desc&vote_count.gte=200&language={tmdb_lang}&include_adult=false"
        )
    } else {
        format!(
            "/discover/{kind_seg}?with_genres={joined}&sort_by=vote_average.desc&vote_count.gte=150&language={tmdb_lang}&include_adult=false"
        )
    }
}

pub fn spawn_discover_by_genres(
    ctx: egui::Context,
    tx: mpsc::Sender<TmdbEvent>,
    token: String,
    kind: TmdbMediaKind,
    selected_genre_keys: Vec<u32>,
    existing_ids: HashSet<String>,
    lang: Lang,
    tmdb_lang: &'static str,
) {
    let _ = thread::Builder::new()
        .name("krono-tmdb-discover".into())
        .spawn(move || {
            let res = discover_by_genres_sync(&token, kind, &selected_genre_keys, &existing_ids, lang, tmdb_lang);
            let _ = tx.send(TmdbEvent::RecommendationsDone(res));
            ctx.request_repaint();
        });
}

fn discover_by_genres_sync(
    token: &str,
    kind: TmdbMediaKind,
    selected_genre_keys: &[u32],
    existing_ids: &HashSet<String>,
    lang: Lang,
    tmdb_lang: &str,
) -> Result<Vec<TmdbSearchItem>, String> {
    let agent = build_agent();
    let tmdb_ids = resolve_tmdb_genre_ids(kind, selected_genre_keys);
    let path = build_discover_path(kind, &tmdb_ids, tmdb_lang);
    let resp: SearchMultiResp = tmdb_get_json(&agent, &path, token, lang)?;
    let items: Vec<TmdbSearchItem> = resp
        .results
        .into_iter()
        .filter_map(|r| r.into_item(Some(kind)))
        .collect();
    Ok(deduplicate_recommendations(items, existing_ids))
}

#[derive(Deserialize)]
struct OpenLibrarySearchResp {
    #[serde(default)]
    docs: Vec<RawOpenLibraryDoc>,
}

#[derive(Deserialize)]
struct RawOpenLibraryDoc {
    #[serde(default)]
    key: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author_name: Vec<String>,
    #[serde(default)]
    first_publish_year: Option<i32>,
    #[serde(default)]
    cover_i: Option<i64>,
    #[serde(default)]
    subject: Vec<String>,
}

pub fn parse_openlibrary_json(raw: &str) -> Result<Vec<BookSearchItem>, String> {
    let parsed: OpenLibrarySearchResp =
        serde_json::from_str(raw).map_err(|e| format!("OpenLibrary parse error: {e}"))?;
    let mut out = Vec::new();
    for doc in parsed.docs {
        let Some(title) = doc
            .title
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let key = doc.key.trim();
        let external_id = if key.is_empty() {
            format!("ol:title:{}", title.to_lowercase())
        } else {
            format!("ol:{key}")
        };
        let author = doc
            .author_name
            .into_iter()
            .map(|a| a.trim().to_string())
            .find(|a| !a.is_empty())
            .unwrap_or_default();
        let cover_url = doc
            .cover_i
            .filter(|id| *id > 0)
            .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
        let summary = doc
            .subject
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .take(4)
            .collect::<Vec<_>>()
            .join(", ");
        out.push(BookSearchItem {
            external_id,
            title,
            author,
            year: doc.first_publish_year.filter(|y| *y > 0),
            cover_url,
            summary,
        });
    }
    Ok(out)
}

fn search_books_sync(query: &str, lang: Lang) -> Result<Vec<BookSearchItem>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let agent = build_agent();
    let url = format!(
        "https://openlibrary.org/search.json?q={}&limit=12&fields=key,title,author_name,first_publish_year,cover_i,subject",
        encode_query_component(trimmed)
    );
    let resp = agent
        .get(&url)
        .set("Accept", "application/json")
        .set(
            "User-Agent",
            concat!("Krono/", env!("KRONO_APP_VERSION"), " (Desktop Knowledge App)"),
        )
        .call()
        .map_err(|err| match err {
            ureq::Error::Status(code, _) => {
                if lang == Lang::Ru {
                    format!("Ошибка OpenLibrary (HTTP {code})")
                } else {
                    format!("OpenLibrary error (HTTP {code})")
                }
            }
            ureq::Error::Transport(t) => {
                if lang == Lang::Ru {
                    format!("OpenLibrary недоступен: {t}")
                } else {
                    format!("OpenLibrary network error: {t}")
                }
            }
        })?;
    let body = resp
        .into_string()
        .map_err(|e| format!("OpenLibrary read error: {e}"))?;
    parse_openlibrary_json(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_w342_poster_urls() {
        assert_eq!(
            poster_w342_url("/abc.jpg"),
            "https://image.tmdb.org/t/p/w342/abc.jpg"
        );
        assert_eq!(
            poster_w342_url("abc.jpg"),
            "https://image.tmdb.org/t/p/w342/abc.jpg"
        );
    }

    #[test]
    fn detects_v4_bearer_vs_v3_key() {
        assert!(!is_v4_bearer_token("a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6"));
        let jwt = format!("{}.{}.{}", "a".repeat(30), "b".repeat(30), "c".repeat(20));
        assert!(is_v4_bearer_token(&jwt));
    }

    #[test]
    fn encodes_query_utf8() {
        assert_eq!(
            encode_query_component("Дюна 2"),
            "%D0%94%D1%8E%D0%BD%D0%B0%202"
        );
    }

    #[test]
    fn deduplicates_recommendations_excluding_existing() {
        let mk = |id: &str, vote: f64| TmdbSearchItem {
            tmdb_id: id.into(),
            kind: TmdbMediaKind::Movie,
            title: format!("Movie {id}"),
            year: Some(2020),
            vote_average: Some(vote),
            poster_path: None,
            overview: String::new(),
        };
        let existing: HashSet<String> = ["10".to_string()].into_iter().collect();
        let res = deduplicate_recommendations(
            vec![mk("10", 9.0), mk("20", 7.5), mk("30", 8.0), mk("20", 7.5)],
            &existing,
        );
        assert_eq!(
            res.iter().map(|i| i.tmdb_id.as_str()).collect::<Vec<_>>(),
            vec!["20", "30"]
        );
    }

    #[test]
    fn builds_discover_path_with_mapped_genre_ids() {
        let movie_ids = resolve_tmdb_genre_ids(TmdbMediaKind::Movie, &[1, 3]);
        assert_eq!(movie_ids, vec![878, 18]);
        let tv_ids = resolve_tmdb_genre_ids(TmdbMediaKind::Tv, &[1, 5]);
        assert_eq!(tv_ids, vec![10765, 10759]);

        let p = build_discover_path(TmdbMediaKind::Movie, &movie_ids, "ru-RU");
        assert!(p.starts_with("/discover/movie?with_genres=878,18"));
        assert!(p.contains("language=ru-RU"));
    }

    #[test]
    fn parses_openlibrary_json_with_covers_and_fallbacks() {
        let raw = r#"{
            "docs": [
                {
                    "key": "/works/OL123W",
                    "title": "Dune",
                    "author_name": ["Frank Herbert"],
                    "first_publish_year": 1965,
                    "cover_i": 823456,
                    "subject": ["Science fiction", "Desert"]
                },
                {
                    "key": "/works/OL999W",
                    "title": "No Cover Book"
                }
            ]
        }"#;
        let items = parse_openlibrary_json(raw).expect("valid json");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].external_id, "ol:/works/OL123W");
        assert_eq!(items[0].author, "Frank Herbert");
        assert_eq!(items[0].year, Some(1965));
        assert_eq!(
            items[0].cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/823456-M.jpg")
        );
        assert_eq!(items[0].summary, "Science fiction, Desert");
        assert_eq!(items[1].cover_url, None);
        assert_eq!(items[1].author, "");
    }
}
