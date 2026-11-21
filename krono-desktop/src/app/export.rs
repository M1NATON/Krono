use super::KronoApp;
use crate::config::write_file_atomic;
use crate::i18n::Lang;
use crate::storage::vault::{full_vault_path, read_note};
use crate::ui::components::ToastKind;
use krono_core::{
    export::export_note_to_html,
    path::{is_encrypted_note, slugify_title},
};
use std::{fs, path::Path};

fn mime_for_image(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

fn resolve_vault_image_bytes(
    vault_dir: &Path,
    note_rel: Option<&str>,
    raw_img: &str,
) -> Option<(String, Vec<u8>)> {
    let clean = raw_img.trim().replace('\\', "/");
    if clean.starts_with("http://") || clean.starts_with("https://") || clean.starts_with("data:") {
        return None;
    }
    let mut candidates = vec![clean.clone()];
    if let Some(rel) = note_rel {
        if let Some((dir, _)) = rel.rsplit_once('/') {
            candidates.push(format!("{dir}/{clean}"));
        }
    }
    for cand in candidates {
        if let Ok(full) = full_vault_path(vault_dir, &cand) {
            if let Ok(bytes) = fs::read(&full) {
                return Some((mime_for_image(&cand).to_string(), bytes));
            }
        }
    }
    None
}

impl KronoApp {
    fn resolve_transcluded_note_for_export(&self, target: &str) -> Option<String> {
        let rel = self.find_note_by_wikilink(target)?;
        if is_encrypted_note(&rel) {
            return None;
        }
        read_note(&self.config.vault_dir, &rel)
            .ok()
            .map(|(doc, _, _)| doc.body)
    }

    pub(crate) fn export_current_note_html(&mut self) {
        let lang = self.config.language;
        if self.selected_path.is_none() || self.is_current_note_locked() {
            return;
        }
        let title = self.current_doc.frontmatter.title.clone();
        let slug = slugify_title(&title);
        let Some(dest) = rfd::FileDialog::new()
            .set_file_name(format!("{slug}.html"))
            .add_filter("HTML", &["html", "htm"])
            .save_file()
        else {
            return;
        };

        let vault_dir = self.config.vault_dir.clone();
        let cur_rel = self.selected_path.clone();
        let html = export_note_to_html(
            &title,
            &self.current_doc.body,
            |target| self.resolve_transcluded_note_for_export(target),
            |img| resolve_vault_image_bytes(&vault_dir, cur_rel.as_deref(), img),
        );

        match write_file_atomic(&dest, html.as_bytes()) {
            Ok(()) => {
                let msg: String = match lang {
                    Lang::Ru => format!("HTML сохранён: {}", dest.display()),
                    Lang::En => format!("HTML exported: {}", dest.display()),
                };
                self.push_toast(msg, ToastKind::Success);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }

    pub(crate) fn export_current_note_pdf(&mut self) {
        let lang = self.config.language;
        if self.selected_path.is_none() || self.is_current_note_locked() {
            return;
        }
        let title = self.current_doc.frontmatter.title.clone();
        let slug = slugify_title(&title);
        let Some(dest) = rfd::FileDialog::new()
            .set_file_name(format!("{slug}.pdf"))
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return;
        };

        let vault_dir = self.config.vault_dir.clone();
        let cur_rel = self.selected_path.clone();
        let res = crate::export_pdf::render_markdown_to_pdf_bytes(
            &title,
            &self.current_doc.body,
            |target| self.resolve_transcluded_note_for_export(target),
            |img| resolve_vault_image_bytes(&vault_dir, cur_rel.as_deref(), img).map(|(_, b)| b),
        );

        match res.and_then(|bytes| write_file_atomic(&dest, &bytes)) {
            Ok(()) => {
                let msg: String = match lang {
                    Lang::Ru => format!("PDF сохранён: {}", dest.display()),
                    Lang::En => format!("PDF exported: {}", dest.display()),
                };
                self.push_toast(msg, ToastKind::Success);
            }
            Err(e) => self.push_toast(e, ToastKind::Error),
        }
    }
}
