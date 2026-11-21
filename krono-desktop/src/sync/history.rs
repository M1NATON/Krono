use crate::i18n::Lang;
use crate::sync::github::{apply_headers, build_agent, format_ureq_err, parse_owner_repo};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use krono_core::path::encode_url_path;
use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct FileCommit {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: String,
    pub date_iso: String,
}

#[derive(Deserialize)]
struct GhCommitListItem {
    sha: String,
    commit: GhCommitDetail,
}

#[derive(Deserialize)]
struct GhCommitDetail {
    message: String,
    author: Option<GhCommitAuthor>,
}

#[derive(Deserialize)]
struct GhCommitAuthor {
    name: Option<String>,
    date: Option<String>,
}

#[derive(Deserialize)]
struct GhContentsResp {
    content: Option<String>,
    encoding: Option<String>,
}

fn percent_encode_query_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

pub fn fetch_file_commits(
    repo_input: &str,
    token: &str,
    rel_path: &str,
    limit: usize,
    lang: Lang,
) -> Result<Vec<FileCommit>, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err(match lang {
            Lang::Ru => "Укажите GitHub Token в настройках синхронизации.".into(),
            Lang::En => "Configure a GitHub Token in Sync settings.".into(),
        });
    }
    let (owner, repo) = parse_owner_repo(repo_input, lang)?;
    let encoded_path = percent_encode_query_value(rel_path);
    let per_page = limit.clamp(1, 50);
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/commits?path={encoded_path}&per_page={per_page}"
    );

    let agent = build_agent();
    let resp = apply_headers(agent.get(&url), token)
        .call()
        .map_err(|e| format_ureq_err(e, lang))?;

    let items: Vec<GhCommitListItem> = resp.into_json().map_err(|e| match lang {
        Lang::Ru => format!("Ошибка разбора ответа GitHub: {e}"),
        Lang::En => format!("Failed to parse GitHub commits response: {e}"),
    })?;

    Ok(items
        .into_iter()
        .map(|item| {
            let short_sha = item.sha.chars().take(7).collect();
            let first_line = item
                .commit
                .message
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let author = item
                .commit
                .author
                .as_ref()
                .and_then(|a| a.name.clone())
                .unwrap_or_default();
            let date_iso = item
                .commit
                .author
                .as_ref()
                .and_then(|a| a.date.clone())
                .unwrap_or_default();
            FileCommit {
                sha: item.sha,
                short_sha,
                message: first_line,
                author,
                date_iso,
            }
        })
        .collect())
}

pub fn fetch_file_at_ref(
    repo_input: &str,
    token: &str,
    rel_path: &str,
    commit_sha: &str,
    lang: Lang,
) -> Result<Vec<u8>, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err(match lang {
            Lang::Ru => "Укажите GitHub Token в настройках синхронизации.".into(),
            Lang::En => "Configure a GitHub Token in Sync settings.".into(),
        });
    }
    let (owner, repo) = parse_owner_repo(repo_input, lang)?;
    let encoded_path = encode_url_path(rel_path);
    let encoded_ref = percent_encode_query_value(commit_sha);
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/contents/{encoded_path}?ref={encoded_ref}"
    );

    let agent = build_agent();
    let resp = apply_headers(agent.get(&url), token)
        .call()
        .map_err(|e| format_ureq_err(e, lang))?;

    let body: GhContentsResp = resp.into_json().map_err(|e| match lang {
        Lang::Ru => format!("Ошибка разбора содержимого файла GitHub: {e}"),
        Lang::En => format!("Failed to parse GitHub contents response: {e}"),
    })?;

    if let Some(enc) = body.encoding.as_deref() {
        if enc != "base64" {
            return Err(match lang {
                Lang::Ru => format!("Неподдерживаемая кодировка GitHub: {enc}"),
                Lang::En => format!("Unsupported GitHub encoding: {enc}"),
            });
        }
    }

    let raw_b64 = body.content.unwrap_or_default();
    let clean_b64 = raw_b64.replace(['\n', '\r'], "");
    BASE64.decode(clean_b64.as_bytes()).map_err(|e| match lang {
        Lang::Ru => format!("Ошибка декодирования base64: {e}"),
        Lang::En => format!("Base64 decode failed: {e}"),
    })
}
