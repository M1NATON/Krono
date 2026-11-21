use super::KronoApp;
use crate::i18n::Lang;
use crate::storage::vault::write_note_bytes_atomic;
use crate::sync::{
    history::{fetch_file_at_ref, fetch_file_commits, FileCommit},
    secrets::load_github_token,
};
use crate::ui::{components::ToastKind, history_view::InspectorTab};
use eframe::egui;
use krono_core::{note::NoteDocument, path::vault_file_stem};
use std::sync::mpsc;

#[derive(Default)]
pub struct HistoryState {
    pub active_tab: InspectorTab,
    pub loaded_for_path: Option<String>,
    pub commits: Vec<FileCommit>,
    pub loading_commits: bool,
    pub commits_error: Option<String>,
    pub selected_sha: Option<String>,
    pub loading_content: bool,
    pub content_error: Option<String>,
    pub selected_content: Option<String>,
    pub confirm_restore_sha: Option<String>,
    commits_rx: Option<mpsc::Receiver<(String, Result<Vec<FileCommit>, String>)>>,
    content_rx: Option<mpsc::Receiver<(String, String, Result<String, String>)>>,
}

impl HistoryState {
    pub fn reset_for_path(&mut self, path: Option<&str>) {
        if self.loaded_for_path.as_deref() != path {
            self.loaded_for_path = None;
            self.commits.clear();
            self.loading_commits = false;
            self.commits_error = None;
            self.selected_sha = None;
            self.loading_content = false;
            self.content_error = None;
            self.selected_content = None;
            self.confirm_restore_sha = None;
            self.commits_rx = None;
            self.content_rx = None;
        }
    }
}

impl KronoApp {
    pub(super) fn poll_history_channels(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.history.commits_rx {
            if let Ok((path, res)) = rx.try_recv() {
                self.history.loading_commits = false;
                self.history.commits_rx = None;
                if self.selected_path.as_deref() == Some(path.as_str()) {
                    self.history.loaded_for_path = Some(path);
                    match res {
                        Ok(list) => {
                            self.history.commits = list;
                            self.history.commits_error = None;
                        }
                        Err(e) => {
                            self.history.commits.clear();
                            self.history.commits_error = Some(e);
                        }
                    }
                }
            }
        }

        if let Some(rx) = &self.history.content_rx {
            if let Ok((path, sha, res)) = rx.try_recv() {
                self.history.loading_content = false;
                self.history.content_rx = None;
                if self.selected_path.as_deref() == Some(path.as_str())
                    && self.history.selected_sha.as_deref() == Some(sha.as_str())
                {
                    match res {
                        Ok(text) => {
                            self.history.selected_content = Some(text);
                            self.history.content_error = None;
                        }
                        Err(e) => {
                            self.history.selected_content = None;
                            self.history.content_error = Some(e);
                        }
                    }
                }
            }
        }

        if self.history.loading_commits || self.history.loading_content {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
    }

    pub(crate) fn request_note_history(&mut self, path: &str) {
        let lang = self.config.language;
        let repo = self.config.github_repo.trim().to_string();
        if repo.is_empty() {
            self.history.loaded_for_path = Some(path.to_string());
            self.history.commits.clear();
            self.history.loading_commits = false;
            self.history.commits_error = Some(match lang {
                Lang::Ru => "Репозиторий GitHub не настроен.".into(),
                Lang::En => "GitHub repository is not configured.".into(),
            });
            return;
        }
        let Ok(Some(token)) = load_github_token() else {
            self.history.loaded_for_path = Some(path.to_string());
            self.history.commits.clear();
            self.history.loading_commits = false;
            self.history.commits_error = Some(match lang {
                Lang::Ru => "Токен GitHub не найден в связке ключей.".into(),
                Lang::En => "GitHub token not found in keychain.".into(),
            });
            return;
        };

        self.history.loading_commits = true;
        self.history.commits_error = None;
        self.history.selected_sha = None;
        self.history.selected_content = None;
        self.history.content_error = None;
        self.history.confirm_restore_sha = None;

        let (tx, rx) = mpsc::channel();
        self.history.commits_rx = Some(rx);
        let path_owned = path.to_string();
        std::thread::spawn(move || {
            let res = fetch_file_commits(&repo, &token, &path_owned, 20, lang);
            let _ = tx.send((path_owned, res));
        });
    }

    pub(crate) fn request_history_version(&mut self, path: &str, sha: &str) {
        let lang = self.config.language;
        let repo = self.config.github_repo.trim().to_string();
        let Ok(Some(token)) = load_github_token() else {
            self.history.content_error = Some(match lang {
                Lang::Ru => "Токен GitHub не найден.".into(),
                Lang::En => "GitHub token not found.".into(),
            });
            return;
        };

        self.history.selected_sha = Some(sha.to_string());
        self.history.loading_content = true;
        self.history.content_error = None;
        self.history.selected_content = None;
        self.history.confirm_restore_sha = None;

        let (tx, rx) = mpsc::channel();
        self.history.content_rx = Some(rx);
        let path_owned = path.to_string();
        let sha_owned = sha.to_string();
        std::thread::spawn(move || {
            let res = fetch_file_at_ref(&repo, &token, &path_owned, &sha_owned, lang).and_then(
                |bytes| {
                    String::from_utf8(bytes).map_err(|_| match lang {
                        Lang::Ru => "Файл в этой ревизии не является UTF-8 текстом.".into(),
                        Lang::En => "File at this revision is not valid UTF-8 text.".into(),
                    })
                },
            );
            let _ = tx.send((path_owned, sha_owned, res));
        });
    }

    pub(crate) fn restore_history_version(&mut self, path: &str, content: &str) {
        let lang = self.config.language;
        let stem = vault_file_stem(path).to_string();
        let norm = content.replace("\r\n", "\n");
        let doc = NoteDocument::parse(&norm, &stem);
        let vault_dir = self.config.vault_dir.clone();
        match write_note_bytes_atomic(&vault_dir, path, norm.as_bytes()) {
            Ok((modified, size)) => {
                if let Err(e) = self.index.upsert_note(path, &doc, modified, size) {
                    self.push_toast(e, ToastKind::Error);
                    return;
                }
                if self.selected_path.as_deref() == Some(path) {
                    self.current_doc = doc;
                    self.current_modified = modified;
                    self.dirty = false;
                    self.refresh_document_derivations();
                }
                self.reload_index_caches();
                self.history.confirm_restore_sha = None;
                let msg: String = match lang {
                    Lang::Ru => "Версия заметки восстановлена".into(),
                    Lang::En => "Note version restored".into(),
                };
                self.push_toast(msg, ToastKind::Success);
            }
            Err(e) => {
                self.push_toast(e, ToastKind::Error);
            }
        }
    }
}
