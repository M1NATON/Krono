use crate::config::normalize_github_repo;
use crate::i18n::Lang;
use crate::storage::vault::{delete_note, list_vault_files, write_note_bytes_atomic};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use krono_core::{
    hash::{git_blob_sha1, normalize_lf_bytes},
    path::{conflict_path, encode_url_path, validate_vault_path, VaultFileKind},
    sync_plan::{plan_sync, SyncOp},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
    time::Duration,
};

const BLOB_CONCURRENCY: usize = 4;

#[derive(Clone, Debug)]
pub struct RepoInfo {
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
}

#[derive(Clone, Debug)]
pub struct SyncSummary {
    pub new_base_hashes: HashMap<String, String>,
    pub pushed: usize,
    pub pulled: usize,
    pub deleted_local: usize,
    pub deleted_remote: usize,
    pub conflicts: Vec<String>,
    pub head_sha: String,
}

#[derive(Deserialize)]
struct GhRepoResp {
    full_name: String,
    private: bool,
    default_branch: String,
    permissions: Option<GhPermissions>,
}

#[derive(Deserialize)]
struct GhPermissions {
    push: bool,
}

#[derive(Deserialize)]
struct GhRefResp {
    object: GhObject,
}

#[derive(Deserialize)]
struct GhObject {
    sha: String,
}

#[derive(Deserialize)]
struct GhCommitResp {
    tree: GhObject,
}

#[derive(Deserialize)]
struct GhTreeResp {
    tree: Vec<GhTreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Deserialize)]
struct GhTreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
    sha: String,
}

#[derive(Deserialize)]
struct GhBlobResp {
    content: String,
    encoding: String,
}

#[derive(Serialize)]
struct GhCreateBlobReq<'a> {
    content: &'a str,
    encoding: &'a str,
}

#[derive(Deserialize)]
struct GhShaResp {
    sha: String,
}

#[derive(Serialize)]
struct GhCreateTreeReq<'a> {
    base_tree: &'a str,
    tree: Vec<GhTreeUpdateEntry<'a>>,
}

#[derive(Serialize)]
struct GhTreeUpdateEntry<'a> {
    path: &'a str,
    mode: &'a str,
    #[serde(rename = "type")]
    entry_type: &'a str,
    sha: Option<&'a str>,
}

#[derive(Serialize)]
struct GhCreateCommitReq<'a> {
    message: &'a str,
    tree: &'a str,
    parents: Vec<&'a str>,
}

#[derive(Serialize)]
struct GhUpdateRefReq<'a> {
    sha: &'a str,
    force: bool,
}

#[derive(Serialize)]
struct GhPutContentsReq<'a> {
    message: &'a str,
    content: &'a str,
    branch: &'a str,
}

#[derive(Deserialize)]
struct GhPutContentsResp {
    commit: GhObject,
}

#[derive(Deserialize)]
struct GhErrBody {
    message: Option<String>,
}

enum PendingDiskOp {
    Write { rel_path: String, bytes: Vec<u8> },
    SoftDelete { rel_path: String },
}

fn build_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(12))
        .timeout(Duration::from_secs(35))
        .build()
}

fn apply_headers(req: ureq::Request, token: &str) -> ureq::Request {
    let auth = zeroize::Zeroizing::new(format!("Bearer {}", token.trim()));
    req.set("Authorization", &auth)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "Krono-Desktop/0.2")
}

fn format_ureq_err(err: ureq::Error, lang: Lang) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let gh_msg = serde_json::from_str::<GhErrBody>(&body)
                .ok()
                .and_then(|b| b.message)
                .unwrap_or(body);
            match code {
                401 => match lang {
                    Lang::Ru => "Ошибка 401: неверный или истёкший GitHub Token.".into(),
                    Lang::En => "HTTP 401: Invalid or expired GitHub token.".into(),
                },
                403 => match lang {
                    Lang::Ru => format!("Ошибка 403: нет прав доступа (проверьте Contents: Read and write). {gh_msg}"),
                    Lang::En => format!("HTTP 403: Access denied (verify Contents: Read and write). {gh_msg}"),
                },
                404 => match lang {
                    Lang::Ru => format!("Ошибка 404: репозиторий или ветка не найдены (либо нет доступа у токена). {gh_msg}"),
                    Lang::En => format!("HTTP 404: Repository or branch not found (or token lacks access). {gh_msg}"),
                },
                _ => format!("GitHub HTTP {code}: {gh_msg}"),
            }
        }
        ureq::Error::Transport(t) => match lang {
            Lang::Ru => format!("Сетевая ошибка: {t}"),
            Lang::En => format!("Network error: {t}"),
        },
    }
}

fn parse_owner_repo(repo_input: &str, lang: Lang) -> Result<(String, String), String> {
    normalize_github_repo(repo_input).ok_or_else(|| match lang {
        Lang::Ru => "Укажите репозиторий в формате owner/repo или ссылку GitHub.".into(),
        Lang::En => "Specify repository as owner/repo or a GitHub URL.".into(),
    })
}

pub fn test_connection(repo_input: &str, token: &str, lang: Lang) -> Result<RepoInfo, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err(match lang {
            Lang::Ru => "Укажите GitHub Personal Access Token.".into(),
            Lang::En => "Please provide a GitHub Personal Access Token.".into(),
        });
    }
    let (owner, repo) = parse_owner_repo(repo_input, lang)?;
    let agent = build_agent();
    let url = format!("https://api.github.com/repos/{owner}/{repo}");

    let resp = apply_headers(agent.get(&url), token)
        .call()
        .map_err(|e| format_ureq_err(e, lang))?;

    let info: GhRepoResp = resp.into_json().map_err(|e| e.to_string())?;
    let can_push = info.permissions.map(|p| p.push).unwrap_or(true);
    if !can_push {
        return Err(match lang {
            Lang::Ru => "У токена нет прав на запись (push) в этот репозиторий.".into(),
            Lang::En => "This token does not have push permission to the repository.".into(),
        });
    }

    Ok(RepoInfo {
        full_name: info.full_name,
        private: info.private,
        default_branch: info.default_branch,
    })
}

/// Detects remote pull paths that would collide with an active local/remote file differing only by case.
/// Returns a map from original remote `path` -> safe `conflict_path` fallback for colliding pulls.
pub(crate) fn compute_case_collision_redirects(
    surviving_local_paths: &[String],
    ops: &[SyncOp],
    conflict_ts: &str,
) -> HashMap<String, String> {
    let mut seen_lower: HashMap<String, String> = HashMap::new();
    for p in surviving_local_paths {
        seen_lower.entry(p.to_lowercase()).or_insert_with(|| p.clone());
    }

    let mut redirects = HashMap::new();
    for op in ops {
        if let SyncOp::Pull { path, .. } = op {
            let key = path.to_lowercase();
            if let Some(existing) = seen_lower.get(&key) {
                if existing != path {
                    redirects.insert(path.clone(), conflict_path(path, conflict_ts));
                }
            } else {
                seen_lower.insert(key, path.clone());
            }
        }
    }
    redirects
}

pub fn sync_with_github(
    vault_dir: &Path,
    base_hashes: &HashMap<String, String>,
    repo_input: &str,
    branch: &str,
    token: &str,
    lang: Lang,
) -> Result<SyncSummary, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err(match lang {
            Lang::Ru => "Не задан GitHub Token в настройках.".into(),
            Lang::En => "GitHub token is not configured in settings.".into(),
        });
    }
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(match lang {
            Lang::Ru => "Не задана ветка GitHub.".into(),
            Lang::En => "GitHub branch cannot be empty.".into(),
        });
    }

    let (owner, repo) = parse_owner_repo(repo_input, lang)?;
    let agent = build_agent();
    let api_base = format!("https://api.github.com/repos/{owner}/{repo}");

    // 1. Collect all local vault files (normalized LF for text; raw bytes for binary assets)
    let mut local_bytes: HashMap<String, Vec<u8>> = HashMap::new();
    let mut local_hashes: HashMap<String, String> = HashMap::new();
    for info in list_vault_files(vault_dir) {
        if let Ok(raw) = fs::read(&info.full_path) {
            let prepared = if info.kind.is_binary() {
                raw
            } else {
                normalize_lf_bytes(&raw)
            };
            let sha = git_blob_sha1(&prepared);
            local_hashes.insert(info.rel_path.clone(), sha);
            local_bytes.insert(info.rel_path, prepared);
        }
    }

    let encoded_branch = encode_url_path(branch);
    let conflict_ts = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();

    for attempt in 0..2 {
        // 2. Fetch or bootstrap branch reference
        let ref_url = format!("{api_base}/git/ref/heads/{encoded_branch}");
        let head_commit_sha = match apply_headers(agent.get(&ref_url), token).call() {
            Ok(resp) => {
                let parsed: GhRefResp = resp.into_json().map_err(|e| e.to_string())?;
                parsed.object.sha
            }
            Err(ureq::Error::Status(404 | 409, _)) => {
                let _repo_check: GhRepoResp = apply_headers(agent.get(&api_base), token)
                    .call()
                    .map_err(|e| format_ureq_err(e, lang))?
                    .into_json()
                    .map_err(|e| e.to_string())?;

                let (boot_path, boot_bytes) = if let Some((p, b)) = local_bytes.iter().next() {
                    (p.clone(), b.clone())
                } else {
                    let starter = b"---\ntitle: Welcome\nstatus: Todo\ntags: krono\n---\n\n# Krono Vault\n".to_vec();
                    let _ = write_note_bytes_atomic(vault_dir, "welcome.md", &starter);
                    local_hashes.insert("welcome.md".into(), git_blob_sha1(&starter));
                    local_bytes.insert("welcome.md".into(), starter.clone());
                    ("welcome.md".to_string(), starter)
                };

                let encoded_boot = encode_url_path(&boot_path);
                let put_url = format!("{api_base}/contents/{encoded_boot}");
                let b64 = BASE64.encode(&boot_bytes);
                let put_resp: GhPutContentsResp = apply_headers(agent.put(&put_url), token)
                    .send_json(GhPutContentsReq {
                        message: "Initialize Krono vault",
                        content: &b64,
                        branch,
                    })
                    .map_err(|e| format_ureq_err(e, lang))?
                    .into_json()
                    .map_err(|e| e.to_string())?;
                put_resp.commit.sha
            }
            Err(e) => return Err(format_ureq_err(e, lang)),
        };

        // 3. Read head commit -> tree SHA -> recursive tree
        let commit_url = format!("{api_base}/git/commits/{head_commit_sha}");
        let commit_resp: GhCommitResp = apply_headers(agent.get(&commit_url), token)
            .call()
            .map_err(|e| format_ureq_err(e, lang))?
            .into_json()
            .map_err(|e| e.to_string())?;
        let base_tree_sha = commit_resp.tree.sha;

        let tree_url = format!("{api_base}/git/trees/{base_tree_sha}?recursive=1");
        let tree_resp: GhTreeResp = apply_headers(agent.get(&tree_url), token)
            .call()
            .map_err(|e| format_ureq_err(e, lang))?
            .into_json()
            .map_err(|e| e.to_string())?;

        if tree_resp.truncated {
            return Err(match lang {
                Lang::Ru => "Репозиторий слишком велик (GitHub API обрезал дерево файлов).".into(),
                Lang::En => "Repository tree was truncated by GitHub API.".into(),
            });
        }

        let mut remote_hashes: HashMap<String, String> = HashMap::new();
        for entry in tree_resp.tree {
            if entry.entry_type == "blob" {
                if let Ok((clean_path, _kind)) = validate_vault_path(&entry.path) {
                    remote_hashes.insert(clean_path, entry.sha);
                }
            }
        }

        // 4. Plan 3-way merge
        let plan = plan_sync(base_hashes, &local_hashes, &remote_hashes);
        let mut new_base = plan.unchanged.clone();

        // Guard against case-insensitive collisions on Windows / macOS
        let case_redirects = if cfg!(any(target_os = "windows", target_os = "macos")) {
            let deleted_set: HashSet<&str> = plan
                .ops
                .iter()
                .filter_map(|op| match op {
                    SyncOp::DeleteLocal(p) => Some(p.as_str()),
                    _ => None,
                })
                .collect();
            let surviving: Vec<String> = local_hashes
                .keys()
                .filter(|p| !deleted_set.contains(p.as_str()))
                .cloned()
                .collect();
            compute_case_collision_redirects(&surviving, &plan.ops, &conflict_ts)
        } else {
            HashMap::new()
        };

        // 5. Download remote blobs IN MEMORY first (parallel across up to 4 threads)
        let mut needed_shas = BTreeSet::new();
        for op in &plan.ops {
            if let SyncOp::Pull { remote_sha, .. } | SyncOp::Conflict { remote_sha, .. } = op {
                needed_shas.insert(remote_sha.clone());
            }
        }
        let downloaded = fetch_blobs_parallel(
            &agent,
            &api_base,
            &needed_shas.into_iter().collect::<Vec<_>>(),
            token,
            lang,
        )?;

        // Prepare buffered disk operations + remote push/delete queues
        let mut pulled = 0usize;
        let mut deleted_local = 0usize;
        let mut conflicts = Vec::new();
        let mut push_queue: Vec<(String, Vec<u8>)> = Vec::new();
        let mut delete_remote_queue: Vec<String> = Vec::new();
        let mut pending_disk: Vec<PendingDiskOp> = Vec::new();

        for op in plan.ops {
            match op {
                SyncOp::Push(path) => {
                    if let Some(bytes) = local_bytes.get(&path) {
                        push_queue.push((path, bytes.clone()));
                    }
                }
                SyncOp::Pull { path, remote_sha } => {
                    let raw = downloaded
                        .get(&remote_sha)
                        .ok_or_else(|| format!("missing downloaded blob {remote_sha}"))?;
                    let is_bin = VaultFileKind::from_path(&path).is_some_and(VaultFileKind::is_binary);
                    let bytes = if is_bin {
                        raw.clone()
                    } else {
                        normalize_lf_bytes(raw)
                    };
                    if let Some(safe_conflict) = case_redirects.get(&path) {
                        pending_disk.push(PendingDiskOp::Write {
                            rel_path: safe_conflict.clone(),
                            bytes: bytes.clone(),
                        });
                        push_queue.push((safe_conflict.clone(), bytes));
                        conflicts.push(safe_conflict.clone());
                    } else {
                        pending_disk.push(PendingDiskOp::Write {
                            rel_path: path.clone(),
                            bytes,
                        });
                        new_base.insert(path, remote_sha);
                        pulled += 1;
                    }
                }
                SyncOp::Conflict { path, remote_sha } => {
                    if let Some(local_data) = local_bytes.get(&path) {
                        let backup = conflict_path(&path, &conflict_ts);
                        pending_disk.push(PendingDiskOp::Write {
                            rel_path: backup.clone(),
                            bytes: local_data.clone(),
                        });
                        push_queue.push((backup.clone(), local_data.clone()));
                        conflicts.push(backup);
                    }
                    let raw = downloaded
                        .get(&remote_sha)
                        .ok_or_else(|| format!("missing downloaded blob {remote_sha}"))?;
                    let is_bin = VaultFileKind::from_path(&path).is_some_and(VaultFileKind::is_binary);
                    let remote_bytes = if is_bin {
                        raw.clone()
                    } else {
                        normalize_lf_bytes(raw)
                    };
                    pending_disk.push(PendingDiskOp::Write {
                        rel_path: path.clone(),
                        bytes: remote_bytes,
                    });
                    new_base.insert(path, remote_sha);
                    pulled += 1;
                }
                SyncOp::DeleteLocal(path) => {
                    pending_disk.push(PendingDiskOp::SoftDelete { rel_path: path });
                    deleted_local += 1;
                }
                SyncOp::DeleteRemote(path) => {
                    delete_remote_queue.push(path);
                }
            }
        }

        // 6. Commit remote changes BEFORE mutating local disk (retrying once on 422 non-fast-forward)
        let mut pushed = 0usize;
        let deleted_remote = delete_remote_queue.len();
        let mut final_head = head_commit_sha.clone();

        if !push_queue.is_empty() || !delete_remote_queue.is_empty() {
            let created_blobs =
                upload_blobs_parallel(&agent, &api_base, &push_queue, token, lang)?;
            for (path, sha) in &created_blobs {
                new_base.insert(path.clone(), sha.clone());
                pushed += 1;
            }

            let mut tree_entries: Vec<GhTreeUpdateEntry<'_>> = Vec::new();
            for (path, sha) in &created_blobs {
                tree_entries.push(GhTreeUpdateEntry {
                    path: path.as_str(),
                    mode: "100644",
                    entry_type: "blob",
                    sha: Some(sha.as_str()),
                });
            }
            for path in &delete_remote_queue {
                tree_entries.push(GhTreeUpdateEntry {
                    path: path.as_str(),
                    mode: "100644",
                    entry_type: "blob",
                    sha: None,
                });
            }

            let new_tree_url = format!("{api_base}/git/trees");
            let new_tree: GhShaResp = apply_headers(agent.post(&new_tree_url), token)
                .send_json(GhCreateTreeReq {
                    base_tree: &base_tree_sha,
                    tree: tree_entries,
                })
                .map_err(|e| format_ureq_err(e, lang))?
                .into_json()
                .map_err(|e| e.to_string())?;

            let commit_msg = format!(
                "Krono sync (+{pushed} updated, -{deleted_remote} removed)"
            );
            let new_commit_url = format!("{api_base}/git/commits");
            let new_commit: GhShaResp = apply_headers(agent.post(&new_commit_url), token)
                .send_json(GhCreateCommitReq {
                    message: &commit_msg,
                    tree: &new_tree.sha,
                    parents: vec![head_commit_sha.as_str()],
                })
                .map_err(|e| format_ureq_err(e, lang))?
                .into_json()
                .map_err(|e| e.to_string())?;

            let update_ref_url = format!("{api_base}/git/refs/heads/{encoded_branch}");
            match apply_headers(agent.patch(&update_ref_url), token)
                .send_json(GhUpdateRefReq {
                    sha: &new_commit.sha,
                    force: false,
                }) {
                Ok(_) => {
                    final_head = new_commit.sha;
                }
                Err(ureq::Error::Status(422, _)) if attempt == 0 => {
                    // Remote branch advanced concurrently; re-plan safely before touching disk
                    continue;
                }
                Err(e) => return Err(format_ureq_err(e, lang)),
            }
        }

        // 7. Apply buffered local disk writes/deletes strictly after remote commit succeeds
        for op in pending_disk {
            match op {
                PendingDiskOp::Write { rel_path, bytes } => {
                    write_note_bytes_atomic(vault_dir, &rel_path, &bytes)?;
                }
                PendingDiskOp::SoftDelete { rel_path } => {
                    delete_note(vault_dir, &rel_path)?;
                }
            }
        }

        return Ok(SyncSummary {
            new_base_hashes: new_base,
            pushed,
            pulled,
            deleted_local,
            deleted_remote,
            conflicts,
            head_sha: final_head,
        });
    }

    Err(match lang {
        Lang::Ru => "Ветка на GitHub изменилась во время синхронизации. Повторите попытку.".into(),
        Lang::En => "GitHub branch moved concurrently during sync. Please retry.".into(),
    })
}

fn fetch_blobs_parallel(
    agent: &ureq::Agent,
    api_base: &str,
    shas: &[String],
    token: &str,
    lang: Lang,
) -> Result<HashMap<String, Vec<u8>>, String> {
    if shas.is_empty() {
        return Ok(HashMap::new());
    }
    let workers = BLOB_CONCURRENCY.min(shas.len());
    let chunk_sz = shas.len().div_ceil(workers);
    let results: Vec<Result<Vec<(String, Vec<u8>)>, String>> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in shas.chunks(chunk_sz) {
            handles.push(scope.spawn(move || {
                let mut local = Vec::with_capacity(chunk.len());
                for sha in chunk {
                    let bytes = fetch_blob(agent, api_base, sha, token, lang)?;
                    local.push((sha.clone(), bytes));
                }
                Ok::<_, String>(local)
            }));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("download worker panicked".to_string()))
            })
            .collect()
    });

    let mut out = HashMap::with_capacity(shas.len());
    for res in results {
        for (sha, bytes) in res? {
            out.insert(sha, bytes);
        }
    }
    Ok(out)
}

fn upload_blobs_parallel(
    agent: &ureq::Agent,
    api_base: &str,
    items: &[(String, Vec<u8>)],
    token: &str,
    lang: Lang,
) -> Result<Vec<(String, String)>, String> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let workers = BLOB_CONCURRENCY.min(items.len());
    let chunk_sz = items.len().div_ceil(workers);
    let results: Vec<Result<Vec<(String, String)>, String>> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in items.chunks(chunk_sz) {
            handles.push(scope.spawn(move || {
                let blob_url = format!("{api_base}/git/blobs");
                let mut local = Vec::with_capacity(chunk.len());
                for (path, bytes) in chunk {
                    let b64 = BASE64.encode(bytes);
                    let blob_resp: GhShaResp = apply_headers(agent.post(&blob_url), token)
                        .send_json(GhCreateBlobReq {
                            content: &b64,
                            encoding: "base64",
                        })
                        .map_err(|e| format_ureq_err(e, lang))?
                        .into_json()
                        .map_err(|e| e.to_string())?;
                    local.push((path.clone(), blob_resp.sha));
                }
                Ok::<_, String>(local)
            }));
        }
        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err("upload worker panicked".to_string()))
            })
            .collect()
    });

    let mut out = Vec::with_capacity(items.len());
    for res in results {
        out.extend(res?);
    }
    Ok(out)
}

fn fetch_blob(
    agent: &ureq::Agent,
    api_base: &str,
    blob_sha: &str,
    token: &str,
    lang: Lang,
) -> Result<Vec<u8>, String> {
    let url = format!("{api_base}/git/blobs/{blob_sha}");
    let blob: GhBlobResp = apply_headers(agent.get(&url), token)
        .call()
        .map_err(|e| format_ureq_err(e, lang))?
        .into_json()
        .map_err(|e| e.to_string())?;

    if blob.encoding == "base64" {
        let compact: String = blob
            .content
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        BASE64
            .decode(compact.as_bytes())
            .map_err(|e| format!("base64 decode blob {blob_sha}: {e}"))
    } else {
        Ok(blob.content.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirects_case_colliding_pulls() {
        let surviving = vec!["docs/Note.md".to_string()];
        let ops = vec![
            SyncOp::Pull {
                path: "docs/note.md".to_string(),
                remote_sha: "abc".to_string(),
            },
            SyncOp::Pull {
                path: "docs/other.md".to_string(),
                remote_sha: "def".to_string(),
            },
        ];
        let redirects = compute_case_collision_redirects(&surviving, &ops, "20261119-2300");
        assert_eq!(
            redirects.get("docs/note.md").map(String::as_str),
            Some("docs/note.conflict-20261119-2300.md")
        );
        assert!(!redirects.contains_key("docs/other.md"));
    }
}
