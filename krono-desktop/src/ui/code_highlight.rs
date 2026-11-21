use crate::ui::theme::{font_mono, ThemePalette};
use eframe::egui::{text::LayoutJob, Color32, TextFormat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodeLang {
    Rust,
    Python,
    JsTs,
    Json,
    Toml,
    Shell,
    Sql,
    Plain,
}

fn normalize_lang(raw: &str) -> CodeLang {
    match raw.trim().to_ascii_lowercase().as_str() {
        "rs" | "rust" => CodeLang::Rust,
        "py" | "python" => CodeLang::Python,
        "js" | "javascript" | "ts" | "typescript" | "jsx" | "tsx" => CodeLang::JsTs,
        "json" | "jsonc" => CodeLang::Json,
        "toml" => CodeLang::Toml,
        "sh" | "bash" | "zsh" | "shell" => CodeLang::Shell,
        "sql" => CodeLang::Sql,
        _ => CodeLang::Plain,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    Normal,
    Keyword,
    String,
    Number,
    Comment,
    Key,
}

struct CodePalette {
    normal: Color32,
    keyword: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    key: Color32,
}

impl CodePalette {
    fn from_theme(p: &ThemePalette) -> Self {
        if p.is_dark {
            Self {
                normal: p.text_primary,
                keyword: Color32::from_rgb(198, 138, 255),
                string: Color32::from_rgb(152, 209, 132),
                number: Color32::from_rgb(235, 179, 108),
                comment: p.text_muted,
                key: Color32::from_rgb(116, 195, 245),
            }
        } else {
            Self {
                normal: p.text_primary,
                keyword: Color32::from_rgb(138, 43, 196),
                string: Color32::from_rgb(34, 128, 62),
                number: Color32::from_rgb(176, 98, 18),
                comment: p.text_muted,
                key: Color32::from_rgb(18, 112, 179),
            }
        }
    }

    fn color(&self, kind: TokenKind) -> Color32 {
        match kind {
            TokenKind::Normal => self.normal,
            TokenKind::Keyword => self.keyword,
            TokenKind::String => self.string,
            TokenKind::Number => self.number,
            TokenKind::Comment => self.comment,
            TokenKind::Key => self.key,
        }
    }
}

pub fn highlight_code_layout(
    code: &str,
    lang: &str,
    font_size: f32,
    p: &ThemePalette,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    let code_lang = normalize_lang(lang);
    let cp = CodePalette::from_theme(p);
    let font = font_mono(font_size);

    if code_lang == CodeLang::Plain {
        job.append(
            code,
            0.0,
            TextFormat {
                font_id: font,
                color: cp.normal,
                ..Default::default()
            },
        );
        return job;
    }

    let tokens = tokenize_code(code, code_lang);
    for (text, kind) in tokens {
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: cp.color(kind),
                italics: kind == TokenKind::Comment,
                ..Default::default()
            },
        );
    }
    job
}

fn tokenize_code<'a>(code: &'a str, lang: CodeLang) -> Vec<(&'a str, TokenKind)> {
    let mut out = Vec::new();
    let bytes = code.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    let push_tok = |out: &mut Vec<(&'a str, TokenKind)>, slice: &'a str, kind: TokenKind| {
        if slice.is_empty() {
            return;
        }
        if let Some((prev, prev_kind)) = out.last_mut() {
            if *prev_kind == kind && prev.as_ptr() as usize + prev.len() == slice.as_ptr() as usize {
                let merged_len = prev.len() + slice.len();
                let start = prev.as_ptr() as usize - code.as_ptr() as usize;
                *prev = &code[start..start + merged_len];
                return;
            }
        }
        out.push((slice, kind));
    };

    while i < len {
        // Line comments
        let is_slash_comment =
            matches!(lang, CodeLang::Rust | CodeLang::JsTs | CodeLang::Json)
                && code[i..].starts_with("//");
        let is_hash_comment =
            matches!(lang, CodeLang::Python | CodeLang::Toml | CodeLang::Shell)
                && bytes[i] == b'#';
        let is_sql_comment = lang == CodeLang::Sql && code[i..].starts_with("--");
        if is_slash_comment || is_hash_comment || is_sql_comment {
            let end = code[i..].find('\n').map(|rel| i + rel).unwrap_or(len);
            push_tok(&mut out, &code[i..end], TokenKind::Comment);
            i = end;
            continue;
        }

        // Block comments /* ... */
        if matches!(lang, CodeLang::Rust | CodeLang::JsTs | CodeLang::Sql)
            && code[i..].starts_with("/*")
        {
            let end = code[i + 2..]
                .find("*/")
                .map(|rel| i + 2 + rel + 2)
                .unwrap_or(len);
            push_tok(&mut out, &code[i..end], TokenKind::Comment);
            i = end;
            continue;
        }

        // TOML [section] headers at line start
        if lang == CodeLang::Toml
            && bytes[i] == b'['
            && (i == 0 || bytes[i - 1] == b'\n')
        {
            let line_end = code[i..].find('\n').map(|rel| i + rel).unwrap_or(len);
            if let Some(close) = code[i..line_end].find(']') {
                push_tok(&mut out, &code[i..=i + close], TokenKind::Key);
                i = i + close + 1;
                continue;
            }
        }

        // Strings: ", ', ` (JS/TS template)
        let b = bytes[i];
        if b == b'"' || b == b'\'' || (b == b'`' && lang == CodeLang::JsTs) {
            let quote = b;
            let mut j = i + 1;
            while j < len {
                if bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                    continue;
                }
                if bytes[j] == quote {
                    j += 1;
                    break;
                }
                if bytes[j] == b'\n' && quote != b'`' {
                    break;
                }
                j += 1;
            }
            let kind = if lang == CodeLang::Json && quote == b'"' && is_followed_by_colon(&code[j..]) {
                TokenKind::Key
            } else {
                TokenKind::String
            };
            push_tok(&mut out, &code[i..j], kind);
            i = j;
            continue;
        }

        // Numbers
        if b.is_ascii_digit()
            && (i == 0 || !is_ident_continue(code[..i].chars().next_back().unwrap_or(' ')))
        {
            let mut j = i + 1;
            while j < len
                && (bytes[j].is_ascii_alphanumeric()
                    || bytes[j] == b'_'
                    || bytes[j] == b'.')
            {
                j += 1;
            }
            push_tok(&mut out, &code[i..j], TokenKind::Number);
            i = j;
            continue;
        }

        // Identifiers / Keywords
        let ch = code[i..].chars().next().unwrap_or('\0');
        if is_ident_start(ch) {
            let mut j = i + ch.len_utf8();
            while j < len {
                let next_ch = code[j..].chars().next().unwrap_or('\0');
                if is_ident_continue(next_ch) {
                    j += next_ch.len_utf8();
                } else {
                    break;
                }
            }
            let ident = &code[i..j];
            let kind = if is_keyword(lang, ident) {
                TokenKind::Keyword
            } else if lang == CodeLang::Toml && is_followed_by_equals(&code[j..]) {
                TokenKind::Key
            } else {
                TokenKind::Normal
            };
            push_tok(&mut out, ident, kind);
            i = j;
            continue;
        }

        let end = i + ch.len_utf8();
        push_tok(&mut out, &code[i..end], TokenKind::Normal);
        i = end;
    }

    out
}

fn is_followed_by_colon(rest: &str) -> bool {
    rest.trim_start_matches([' ', '\t']).starts_with(':')
}

fn is_followed_by_equals(rest: &str) -> bool {
    rest.trim_start_matches([' ', '\t']).starts_with('=')
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_keyword(lang: CodeLang, ident: &str) -> bool {
    match lang {
        CodeLang::Rust => matches!(
            ident,
            "fn" | "let" | "mut" | "const" | "static" | "pub" | "struct" | "enum" | "impl"
                | "trait" | "type" | "where" | "for" | "in" | "while" | "loop" | "if" | "else"
                | "match" | "return" | "break" | "continue" | "use" | "mod" | "crate" | "super"
                | "self" | "Self" | "async" | "await" | "move" | "ref" | "true" | "false"
                | "Some" | "None" | "Ok" | "Err" | "Option" | "Result" | "String" | "Vec"
        ),
        CodeLang::Python => matches!(
            ident,
            "def" | "class" | "if" | "elif" | "else" | "for" | "while" | "break" | "continue"
                | "return" | "yield" | "import" | "from" | "as" | "with" | "try" | "except"
                | "finally" | "raise" | "pass" | "lambda" | "and" | "or" | "not" | "in" | "is"
                | "async" | "await" | "True" | "False" | "None" | "self"
        ),
        CodeLang::JsTs => matches!(
            ident,
            "function" | "const" | "let" | "var" | "if" | "else" | "for" | "while" | "do"
                | "switch" | "case" | "default" | "break" | "continue" | "return" | "throw"
                | "try" | "catch" | "finally" | "class" | "extends" | "new" | "this" | "super"
                | "import" | "export" | "from" | "async" | "await" | "interface" | "type"
                | "enum" | "implements" | "public" | "private" | "readonly" | "true" | "false"
                | "null" | "undefined"
        ),
        CodeLang::Json => matches!(ident, "true" | "false" | "null"),
        CodeLang::Toml => matches!(ident, "true" | "false"),
        CodeLang::Shell => matches!(
            ident,
            "if" | "then" | "else" | "elif" | "fi" | "for" | "in" | "do" | "done" | "while"
                | "case" | "esac" | "function" | "return" | "exit" | "export" | "local"
                | "readonly" | "echo" | "cd" | "true" | "false"
        ),
        CodeLang::Sql => matches!(
            ident.to_ascii_uppercase().as_str(),
            "SELECT" | "FROM" | "WHERE" | "INSERT" | "INTO" | "VALUES" | "UPDATE" | "SET"
                | "DELETE" | "CREATE" | "TABLE" | "DROP" | "ALTER" | "JOIN" | "LEFT" | "RIGHT"
                | "INNER" | "ON" | "AS" | "AND" | "OR" | "NOT" | "NULL" | "IS" | "IN"
                | "ORDER" | "BY" | "GROUP" | "HAVING" | "LIMIT" | "OFFSET" | "ASC" | "DESC"
                | "PRIMARY" | "KEY" | "UNIQUE" | "INDEX" | "DISTINCT" | "COUNT"
        ),
        CodeLang::Plain => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_rust_python_json_sql_and_handles_unclosed_literals() {
        let rust_toks = tokenize_code("fn main() {\n  let x = 42; // hi\n  let s = \"ok\";\n}", CodeLang::Rust);
        assert!(rust_toks.iter().any(|(t, k)| *t == "fn" && *k == TokenKind::Keyword));
        assert!(rust_toks.iter().any(|(t, k)| *t == "42" && *k == TokenKind::Number));
        assert!(rust_toks.iter().any(|(t, k)| t.starts_with("//") && *k == TokenKind::Comment));
        assert!(rust_toks.iter().any(|(t, k)| *t == "\"ok\"" && *k == TokenKind::String));

        let json_toks = tokenize_code("{\"name\": \"krono\", \"v\": 1, \"ok\": true}", CodeLang::Json);
        assert!(json_toks.iter().any(|(t, k)| *t == "\"name\"" && *k == TokenKind::Key));
        assert!(json_toks.iter().any(|(t, k)| *t == "\"krono\"" && *k == TokenKind::String));
        assert!(json_toks.iter().any(|(t, k)| *t == "true" && *k == TokenKind::Keyword));

        let sql_toks = tokenize_code("select id from notes where id = 1 -- end", CodeLang::Sql);
        assert!(sql_toks.iter().any(|(t, k)| *t == "select" && *k == TokenKind::Keyword));
        assert!(sql_toks.iter().any(|(t, k)| t.starts_with("--") && *k == TokenKind::Comment));

        // Must never panic on unclosed strings/comments or UTF-8
        let unclosed = tokenize_code("/* незакрытый \"строка", CodeLang::Rust);
        assert!(!unclosed.is_empty());
    }
}
