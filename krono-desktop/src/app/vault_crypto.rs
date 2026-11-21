use super::KronoApp;
use crate::i18n::Lang;
use crate::storage::vault::{delete_note, full_vault_path, write_note_bytes_atomic};
use crate::ui::components::ToastKind;
use crate::ui::{NavTarget, ViewMode};
use eframe::egui;
use krono_core::{
    crypto::{decrypt_note_str, encrypt_note_str},
    hash::{git_blob_sha1, normalize_lf_bytes},
    note::NoteDocument,
    path::{is_encrypted_note, slugify_title, vault_file_stem},
};
use std::{
    fs,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CryptoModalKind {
    Unlock { target_path: String },
    CreateEncrypted,
    EncryptExisting { path: String },
}

pub struct VaultCryptoState {
    master_password: Option<Zeroizing<String>>,
    pub last_activity_at: Instant,
    pub locked_note_path: Option<String>,
    pub modal: Option<CryptoModalKind>,
    pub password_draft: Zeroizing<String>,
    pub confirm_draft: Zeroizing<String>,
    pub title_draft: String,
    pub modal_error: Option<String>,
    pub clipboard_clear_deadline: Option<Instant>,
    clipboard_gen: Arc<AtomicU64>,
}

impl Default for VaultCryptoState {
    fn default() -> Self {
        Self {
            master_password: None,
            last_activity_at: Instant::now(),
            locked_note_path: None,
            modal: None,
            password_draft: Zeroizing::new(String::new()),
            confirm_draft: Zeroizing::new(String::new()),
            title_draft: String::new(),
            modal_error: None,
            clipboard_clear_deadline: None,
            clipboard_gen: Arc::new(AtomicU64::new(0)),
        }
    }
}

impl VaultCryptoState {
    pub fn is_unlocked(&self) -> bool {
        self.master_password.is_some()
    }

    pub fn master_password(&self) -> Option<&str> {
        self.master_password.as_deref().map(String::as_str)
    }

    pub fn clear_modal(&mut self) {
        self.modal = None;
        self.password_draft.zeroize();
        self.confirm_draft.zeroize();
        self.title_draft.clear();
        self.modal_error = None;
    }

    pub fn clipboard_seconds_left(&self) -> Option<u64> {
        let deadline = self.clipboard_clear_deadline?;
        let now = Instant::now();
        if now >= deadline {
            None
        } else {
            Some(((deadline - now).as_secs_f32().ceil() as u64).max(1))
        }
    }
}

impl KronoApp {
    pub fn is_current_note_locked(&self) -> bool {
        self.selected_path
            .as_deref()
            .is_some_and(is_encrypted_note)
            && !self.vault_crypto.is_unlocked()
    }

    pub(crate) fn open_unlock_modal(&mut self, path: &str) {
        self.vault_crypto.clear_modal();
        self.vault_crypto.modal = Some(CryptoModalKind::Unlock {
            target_path: path.to_string(),
        });
    }

    pub(crate) fn open_create_encrypted_modal(&mut self) {
        self.vault_crypto.clear_modal();
        self.vault_crypto.title_draft = if self.config.language == Lang::Ru {
            "Секретная заметка".into()
        } else {
            "Secret Note".into()
        };
        self.vault_crypto.modal = Some(CryptoModalKind::CreateEncrypted);
    }

    pub(crate) fn open_encrypt_existing_modal(&mut self, path: &str) {
        if is_encrypted_note(path) {
            return;
        }
        self.vault_crypto.clear_modal();
        self.vault_crypto.modal = Some(CryptoModalKind::EncryptExisting {
            path: path.to_string(),
        });
    }

    pub(crate) fn lock_vault(&mut self) {
        self.vault_crypto.master_password = None;
        self.vault_crypto.password_draft.zeroize();
        self.vault_crypto.confirm_draft.zeroize();
        self.vault_crypto.modal_error = None;

        if self
            .selected_path
            .as_deref()
            .is_some_and(is_encrypted_note)
        {
            self.current_doc.body.zeroize();
            self.current_doc.frontmatter.title.zeroize();
            self.current_doc = NoteDocument::default();
            self.dirty = false;
            self.vault_crypto.locked_note_path = self.selected_path.clone();
            self.refresh_document_derivations();
        }

        if let Some(crate::app::split::SplitContent::Note(ref mut n)) = self.split.right {
            if is_encrypted_note(&n.path) {
                n.doc.body.zeroize();
                n.doc.frontmatter.title.zeroize();
                n.dirty = false;
                self.split.right = None;
            }
        }

        for (_, (_, data)) in self.embedded_notes_cache.iter_mut() {
            if is_encrypted_note(&data.path) {
                data.body.zeroize();
                data.title.zeroize();
            }
        }
        self.embedded_notes_cache.clear();

        let msg: String = match self.config.language {
            Lang::Ru => "Сейф заблокирован — ключи и открытый текст очищены из памяти".into(),
            Lang::En => "Vault locked — keys and plaintext wiped from RAM".into(),
        };
        self.push_toast(msg, ToastKind::Info);
    }

    pub(super) fn tick_vault_autolock(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.events.is_empty()) {
            self.vault_crypto.last_activity_at = Instant::now();
        }

        if self.vault_crypto.is_unlocked() && self.config.vault_autolock_minutes > 0 {
            let limit = Duration::from_secs(self.config.vault_autolock_minutes as u64 * 60);
            if self.vault_crypto.last_activity_at.elapsed() >= limit {
                self.lock_vault();
            }
        }

        if let Some(deadline) = self.vault_crypto.clipboard_clear_deadline {
            if Instant::now() >= deadline {
                self.vault_crypto.clipboard_clear_deadline = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
    }

    pub(crate) fn copy_secret_with_autoclear(&mut self, secret: String) {
        let lang = self.config.language;
        let Ok(mut cb) = arboard::Clipboard::new() else {
            let msg: String = match lang {
                Lang::Ru => "Не удалось открыть буфер обмена".into(),
                Lang::En => "Failed to open clipboard".into(),
            };
            self.push_toast(msg, ToastKind::Error);
            return;
        };
        if let Err(e) = cb.set_text(secret.clone()) {
            self.push_toast(e.to_string(), ToastKind::Error);
            return;
        }

        let secret_guard = Zeroizing::new(secret);
        let gen = self.vault_crypto.clipboard_gen.fetch_add(1, Ordering::SeqCst) + 1;
        let gen_ref = Arc::clone(&self.vault_crypto.clipboard_gen);
        self.vault_crypto.clipboard_clear_deadline = Some(Instant::now() + Duration::from_secs(15));

        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(15));
            if gen_ref.load(Ordering::SeqCst) != gen {
                return;
            }
            if let Ok(mut cb) = arboard::Clipboard::new() {
                if let Ok(current) = cb.get_text() {
                    if current == *secret_guard {
                        let _ = cb.set_text(String::new());
                    }
                }
            }
        });

        let msg: String = match lang {
            Lang::Ru => "Секрет скопирован (очистится через 15 с)".into(),
            Lang::En => "Secret copied (auto-clears in 15s)".into(),
        };
        self.push_toast(msg, ToastKind::Info);
    }

    pub(crate) fn read_decrypted_note(
        &self,
        rel_path: &str,
        password: &str,
    ) -> Result<(NoteDocument, i64, u64), String> {
        let full = full_vault_path(&self.config.vault_dir, rel_path)?;
        let raw = fs::read_to_string(&full).map_err(|e| format!("read '{rel_path}': {e}"))?;
        let plain = decrypt_note_str(&raw, password)?;
        let stem = vault_file_stem(rel_path);
        let doc = NoteDocument::parse(&plain, stem);
        let meta = fs::metadata(&full).map_err(|e| e.to_string())?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Ok((doc, modified, meta.len()))
    }

    pub(crate) fn save_encrypted_note_doc(
        &mut self,
        rel_path: &str,
        doc: &NoteDocument,
        password: &str,
    ) -> Result<(i64, u64), String> {
        let plain = Zeroizing::new(doc.serialize());
        let env_json = encrypt_note_str(&plain, password)?;
        let norm = normalize_lf_bytes(env_json.as_bytes());
        let blob_sha = git_blob_sha1(&norm);
        self.watcher.mark_internal_change();
        let (modified, size) = write_note_bytes_atomic(&self.config.vault_dir, rel_path, &norm)?;
        let title = vault_file_stem(rel_path);
        self.index
            .upsert_encrypted_note(rel_path, title, modified, size, &blob_sha)?;
        Ok((modified, size))
    }

    pub(crate) fn submit_crypto_modal(&mut self) {
        let lang = self.config.language;
        let Some(kind) = self.vault_crypto.modal.clone() else {
            return;
        };
        let pw = self.vault_crypto.password_draft.clone();
        if pw.trim().is_empty() {
            self.vault_crypto.modal_error = Some(match lang {
                Lang::Ru => "Введите мастер-пароль.".into(),
                Lang::En => "Enter master password.".into(),
            });
            return;
        }

        match kind {
            CryptoModalKind::Unlock { target_path } => {
                match self.read_decrypted_note(&target_path, &pw) {
                    Ok((doc, modified, _)) => {
                        self.vault_crypto.master_password = Some(pw);
                        self.vault_crypto.last_activity_at = Instant::now();
                        self.vault_crypto.locked_note_path = None;
                        self.vault_crypto.clear_modal();

                        self.selected_path = Some(target_path.clone());
                        self.current_doc = doc;
                        self.current_modified = modified;
                        self.dirty = false;
                        self.external_conflict_open_note = false;
                        self.expand_parents(&target_path);
                        self.refresh_document_derivations();
                        self.refresh_embedded_boards();
                        self.record_nav(NavTarget::Note(target_path));
                        self.view_mode = ViewMode::Editor;
                    }
                    Err(_) => {
                        self.vault_crypto.modal_error = Some(match lang {
                            Lang::Ru => "Неверный пароль или повреждённый файл.".into(),
                            Lang::En => "Wrong password or corrupted encrypted file.".into(),
                        });
                    }
                }
            }
            CryptoModalKind::CreateEncrypted => {
                let confirm = &self.vault_crypto.confirm_draft;
                if *pw != **confirm {
                    self.vault_crypto.modal_error = Some(match lang {
                        Lang::Ru => "Пароли не совпадают.".into(),
                        Lang::En => "Passwords do not match.".into(),
                    });
                    return;
                }
                let title = self.vault_crypto.title_draft.trim().to_string();
                let clean_title = if title.is_empty() {
                    if lang == Lang::Ru {
                        "Секретная заметка".to_string()
                    } else {
                        "Secret Note".to_string()
                    }
                } else {
                    title
                };
                let slug = slugify_title(&clean_title);
                let existing = self.existing_paths_set();
                let mut rel_path = format!("{slug}.md.enc");
                let mut n = 2usize;
                while existing.contains(&rel_path) {
                    rel_path = format!("{slug}-{n}.md.enc");
                    n += 1;
                }

                let mut doc = NoteDocument::default();
                doc.frontmatter.title = clean_title.clone();
                doc.body = if lang == Lang::Ru {
                    "```secret\n# Новый секрет\nСервис: Example\nЛогин: user@example.com\nПароль: change-me\n```\n".into()
                } else {
                    "```secret\n# New Secret\nService: Example\nLogin: user@example.com\nPassword: change-me\n```\n".into()
                };

                match self.save_encrypted_note_doc(&rel_path, &doc, &pw) {
                    Ok((modified, _)) => {
                        self.vault_crypto.master_password = Some(pw);
                        self.vault_crypto.last_activity_at = Instant::now();
                        self.vault_crypto.locked_note_path = None;
                        self.vault_crypto.clear_modal();
                        self.reload_index_caches();
                        self.selected_path = Some(rel_path.clone());
                        self.current_doc = doc;
                        self.current_modified = modified;
                        self.dirty = false;
                        self.refresh_document_derivations();
                        self.record_nav(NavTarget::Note(rel_path));
                        self.view_mode = ViewMode::Editor;
                    }
                    Err(e) => {
                        self.vault_crypto.modal_error = Some(e);
                    }
                }
            }
            CryptoModalKind::EncryptExisting { path } => {
                let confirm = &self.vault_crypto.confirm_draft;
                if *pw != **confirm {
                    self.vault_crypto.modal_error = Some(match lang {
                        Lang::Ru => "Пароли не совпадают.".into(),
                        Lang::En => "Passwords do not match.".into(),
                    });
                    return;
                }
                let doc = if self.selected_path.as_deref() == Some(path.as_str()) {
                    self.current_doc.clone()
                } else {
                    match crate::storage::vault::read_note(&self.config.vault_dir, &path) {
                        Ok((d, _, _)) => d,
                        Err(e) => {
                            self.vault_crypto.modal_error = Some(e);
                            return;
                        }
                    }
                };
                let stem_rel = path.strip_suffix(".md").unwrap_or(&path);
                let enc_path = format!("{stem_rel}.md.enc");
                match self.save_encrypted_note_doc(&enc_path, &doc, &pw) {
                    Ok((modified, _)) => {
                        let _ = delete_note(&self.config.vault_dir, &path);
                        let _ = self.index.remove_note(&path);
                        self.vault_crypto.master_password = Some(pw);
                        self.vault_crypto.last_activity_at = Instant::now();
                        self.vault_crypto.locked_note_path = None;
                        self.vault_crypto.clear_modal();
                        self.reload_index_caches();
                        if self.selected_path.as_deref() == Some(path.as_str()) {
                            self.selected_path = Some(enc_path.clone());
                            self.current_doc = doc;
                            self.current_modified = modified;
                            self.dirty = false;
                            self.refresh_document_derivations();
                        }
                        self.record_nav(NavTarget::Note(enc_path));
                        let msg: String = match lang {
                            Lang::Ru => "Заметка зашифрована (.md.enc)".into(),
                            Lang::En => "Note encrypted (.md.enc)".into(),
                        };
                        self.push_toast(msg, ToastKind::Success);
                    }
                    Err(e) => {
                        self.vault_crypto.modal_error = Some(e);
                    }
                }
            }
        }
    }
}
