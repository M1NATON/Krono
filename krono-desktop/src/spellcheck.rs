use spellbook::Dictionary;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::mpsc,
};

static EN_AFF: &str = include_str!("../assets/dicts/en_US.aff");
static EN_DIC: &str = include_str!("../assets/dicts/en_US.dic");
static RU_AFF: &str = include_str!("../assets/dicts/ru_RU.aff");
static RU_DIC: &str = include_str!("../assets/dicts/ru_RU.dic");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellWordSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub word: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WordScript {
    Latin,
    Cyrillic,
    MixedOrOther,
}

pub struct SpellChecker {
    en_dict: Option<Dictionary>,
    ru_dict: Option<Dictionary>,
    load_rx: Option<mpsc::Receiver<(Option<Dictionary>, Option<Dictionary>)>>,
    ready: bool,
    check_cache: HashMap<String, bool>,
    user_words: HashSet<String>,
}

impl SpellChecker {
    pub fn new(vault_dir: &Path) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let en = Dictionary::new(EN_AFF, EN_DIC).ok();
            let ru = Dictionary::new(RU_AFF, RU_DIC).ok();
            let _ = tx.send((en, ru));
        });
        let user_words = load_user_dictionary(vault_dir);
        Self {
            en_dict: None,
            ru_dict: None,
            load_rx: Some(rx),
            ready: false,
            check_cache: HashMap::new(),
            user_words,
        }
    }

    pub fn poll_ready(&mut self) -> bool {
        let Some(rx) = self.load_rx.as_ref() else {
            return false;
        };
        if let Ok((en, ru)) = rx.try_recv() {
            self.en_dict = en;
            self.ru_dict = ru;
            self.load_rx = None;
            self.ready = true;
            self.check_cache.clear();
            return true;
        }
        false
    }

    pub fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn reload_user_dict(&mut self, vault_dir: &Path) {
        self.user_words = load_user_dictionary(vault_dir);
        self.check_cache.clear();
    }

    pub fn add_user_word(&mut self, vault_dir: &Path, word: &str) -> Result<(), String> {
        let clean = word.trim().to_lowercase();
        if clean.is_empty() {
            return Ok(());
        }
        self.user_words.insert(clean);
        self.check_cache.clear();
        save_user_dictionary(vault_dir, &self.user_words)
    }

    pub fn is_word_correct(&mut self, word: &str) -> bool {
        if !self.ready {
            return true;
        }
        let low = word.to_lowercase();
        if self.user_words.contains(&low) || self.user_words.contains(word) {
            return true;
        }
        if let Some(&cached) = self.check_cache.get(word) {
            return cached;
        }

        let ok = match classify_script(word) {
            WordScript::Latin => self.en_dict.as_ref().is_none_or(|d| d.check(word)),
            WordScript::Cyrillic => self.ru_dict.as_ref().is_none_or(|d| {
                d.check(word) || (word.contains('ё') && d.check(&word.replace('ё', "е")))
                    || (word.contains('Ё') && d.check(&word.replace('Ё', "Е")))
            }),
            WordScript::MixedOrOther => true,
        };

        if self.check_cache.len() > 16_384 {
            self.check_cache.clear();
        }
        self.check_cache.insert(word.to_string(), ok);
        ok
    }

    pub fn suggest(&self, word: &str, max_results: usize) -> Vec<String> {
        if !self.ready || max_results == 0 {
            return Vec::new();
        }
        let mut raw = Vec::new();
        match classify_script(word) {
            WordScript::Latin => {
                if let Some(d) = &self.en_dict {
                    d.suggest(word, &mut raw);
                }
            }
            WordScript::Cyrillic => {
                if let Some(d) = &self.ru_dict {
                    d.suggest(word, &mut raw);
                }
            }
            WordScript::MixedOrOther => {}
        }
        raw.into_iter().take(max_results).collect()
    }
}

fn user_dict_path(vault_dir: &Path) -> PathBuf {
    vault_dir.join(".krono").join("dictionary.txt")
}

fn load_user_dictionary(vault_dir: &Path) -> HashSet<String> {
    let path = user_dict_path(vault_dir);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return HashSet::new();
    };
    raw.lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| !l.is_empty())
        .collect()
}

fn save_user_dictionary(vault_dir: &Path, words: &HashSet<String>) -> Result<(), String> {
    let path = user_dict_path(vault_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut sorted: Vec<&str> = words.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let content = if sorted.is_empty() {
        String::new()
    } else {
        format!("{}\n", sorted.join("\n"))
    };
    let tmp = path.with_extension("txt.tmp");
    std::fs::write(&tmp, content.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn classify_script(word: &str) -> WordScript {
    let mut has_lat = false;
    let mut has_cyr = false;
    for ch in word.chars() {
        if !ch.is_alphabetic() {
            continue;
        }
        if ch.is_ascii_alphabetic() {
            has_lat = true;
        } else if ('\u{0400}'..='\u{04FF}').contains(&ch) {
            has_cyr = true;
        } else {
            return WordScript::MixedOrOther;
        }
    }
    match (has_lat, has_cyr) {
        (true, false) => WordScript::Latin,
        (false, true) => WordScript::Cyrillic,
        _ => WordScript::MixedOrOther,
    }
}

/// Extracts checkable word spans from Markdown text while skipping fenced code,
/// inline code, URLs, wikilinks/embeds, Markdown link targets, acronyms, and numbers.
pub fn extract_spell_spans(text: &str) -> Vec<SpellWordSpan> {
    let mut out = Vec::new();
    let mut in_fence: Option<char> = None;
    let mut byte_offset = 0usize;
    let mut char_offset = 0usize;

    for segment in text.split_inclusive('\n') {
        let line_no_nl = segment.strip_suffix('\n').unwrap_or(segment);
        let trimmed = line_no_nl.trim_start();
        if let Some(ch) = fence_marker(trimmed) {
            if in_fence == Some(ch) {
                in_fence = None;
            } else if in_fence.is_none() {
                in_fence = Some(ch);
            }
            byte_offset += segment.len();
            char_offset += segment.chars().count();
            continue;
        }

        if in_fence.is_none() {
            extract_line_spans(line_no_nl, byte_offset, char_offset, &mut out);
        }

        byte_offset += segment.len();
        char_offset += segment.chars().count();
    }

    out
}

fn fence_marker(trimmed: &str) -> Option<char> {
    if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

fn extract_line_spans(
    line: &str,
    base_byte: usize,
    base_char: usize,
    out: &mut Vec<SpellWordSpan>,
) {
    let len = line.len();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut c_idx = 0usize;

    while i < len {
        // Inline code: `...`
        if bytes[i] == b'`' {
            if let Some(rel) = line[i + 1..].find('`') {
                let end = i + 1 + rel + 1;
                c_idx += line[i..end].chars().count();
                i = end;
                continue;
            }
        }
        // Wikilink / embed: [[...]] or ![[...]]
        if line[i..].starts_with("![[") || line[i..].starts_with("[[") {
            let skip = if line[i..].starts_with("![[") { 3 } else { 2 };
            if let Some(rel) = line[i + skip..].find("]]") {
                let end = i + skip + rel + 2;
                c_idx += line[i..end].chars().count();
                i = end;
                continue;
            }
        }
        // Markdown link target: ](...)
        if line[i..].starts_with("](") {
            if let Some(rel) = line[i + 2..].find(')') {
                let end = i + 2 + rel + 1;
                c_idx += line[i..end].chars().count();
                i = end;
                continue;
            }
        }
        // URL: http:// or https://
        if line[i..].starts_with("http://") || line[i..].starts_with("https://") {
            let end = line[i..]
                .find(char::is_whitespace)
                .map(|r| i + r)
                .unwrap_or(len);
            c_idx += line[i..end].chars().count();
            i = end;
            continue;
        }

        let ch = line[i..].chars().next().unwrap_or('\0');
        if is_word_char(ch) {
            let start_b = i;
            let start_c = c_idx;
            let mut end_b = i + ch.len_utf8();
            let mut end_c = c_idx + 1;
            let mut has_digit_or_underscore = ch.is_ascii_digit() || ch == '_';

            while end_b < len {
                let next_ch = line[end_b..].chars().next().unwrap_or('\0');
                if is_word_char(next_ch) {
                    if next_ch.is_ascii_digit() || next_ch == '_' {
                        has_digit_or_underscore = true;
                    }
                    end_b += next_ch.len_utf8();
                    end_c += 1;
                } else if (next_ch == '-' || next_ch == '\'' || next_ch == '’')
                    && line[end_b + next_ch.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(char::is_alphabetic)
                {
                    end_b += next_ch.len_utf8();
                    end_c += 1;
                } else {
                    break;
                }
            }

            let word = &line[start_b..end_b];
            if !has_digit_or_underscore && is_checkable_word(word) {
                out.push(SpellWordSpan {
                    byte_start: base_byte + start_b,
                    byte_end: base_byte + end_b,
                    char_start: base_char + start_c,
                    char_end: base_char + end_c,
                    word: word.to_string(),
                });
            }

            i = end_b;
            c_idx = end_c;
        } else {
            i += ch.len_utf8();
            c_idx += 1;
        }
    }
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphabetic() || ch.is_ascii_digit() || ch == '_'
}

fn is_checkable_word(word: &str) -> bool {
    let alpha_count = word.chars().filter(|c| c.is_alphabetic()).count();
    if alpha_count < 2 {
        return false;
    }
    // Skip ALL-CAPS acronyms (API, HTTP, SQL)
    if word
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase())
    {
        return false;
    }
    // Skip camelCase identifiers (e.g., fooBar)
    let mut chars = word.chars();
    let first_lower = chars.next().is_some_and(|c| c.is_lowercase());
    if first_lower && chars.any(|c| c.is_uppercase()) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_spell_spans_skips_code_urls_wikilinks_and_acronyms() {
        let md = "Привет wrld! Visit https://example.com/test `badcode` [[Ссылко]] HTTP fooBar.\n```rust\nlet x = wrld;\n```\n";
        let spans = extract_spell_spans(md);
        let words: Vec<&str> = spans.iter().map(|s| s.word.as_str()).collect();
        assert_eq!(words, vec!["Привет", "wrld", "Visit"]);
    }

    #[test]
    fn embedded_en_ru_dicts_load_check_suggest_and_persist_user_words() {
        let en = Dictionary::new(EN_AFF, EN_DIC).expect("en_US dictionary must parse");
        let ru = Dictionary::new(RU_AFF, RU_DIC).expect("ru_RU dictionary must parse");

        let tmp = std::env::temp_dir().join("krono_spellcheck_test_vault");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut sc = SpellChecker {
            en_dict: Some(en),
            ru_dict: Some(ru),
            load_rx: None,
            ready: true,
            check_cache: HashMap::new(),
            user_words: HashSet::new(),
        };

        assert!(sc.is_word_correct("hello"));
        assert!(!sc.is_word_correct("helooo"));
        assert!(sc.is_word_correct("привет"));
        assert!(!sc.is_word_correct("преветт"));

        let sug_en = sc.suggest("helo", 3);
        assert!(!sug_en.is_empty());

        sc.add_user_word(&tmp, "Krono").unwrap();
        assert!(sc.is_word_correct("Krono"));
        assert!(sc.is_word_correct("krono"));

        let loaded = load_user_dictionary(&tmp);
        assert!(loaded.contains("krono"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
