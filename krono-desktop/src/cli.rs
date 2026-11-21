use crate::config::AppConfig;
use crate::storage::index::VaultIndex;
use crate::storage::vault::{create_daily_note_with_template, read_note, save_note};
use chrono::{Datelike, Local};
use krono_core::capture::{append_timestamped_entry, parse_cli_args, CliCommand};
use krono_core::daily::{daily_legacy_rel_path, daily_note_rel_path, SimpleDate};

#[cfg(target_os = "windows")]
fn attach_parent_console() {
    const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(dwProcessId: u32) -> i32;
    }
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_parent_console() {}

fn today_local_cli() -> SimpleDate {
    let now = Local::now().date_naive();
    SimpleDate::from_ymd(now.year(), now.month(), now.day()).unwrap_or(SimpleDate {
        year: 2026,
        month: 1,
        day: 1,
    })
}

pub fn try_handle_cli() -> Option<i32> {
    match parse_cli_args(std::env::args().skip(1)) {
        Ok(CliCommand::None) => None,
        Ok(CliCommand::Help) => {
            attach_parent_console();
            println!("Krono CLI");
            println!("  krono add \"<text>\"   Append a timestamped entry to today's daily note");
            println!("  krono --help         Show this help message");
            Some(0)
        }
        Err(err) => {
            attach_parent_console();
            eprintln!("krono: {err}");
            eprintln!("Usage: krono add \"<text>\"");
            Some(1)
        }
        Ok(CliCommand::Add(text)) => {
            attach_parent_console();
            match run_add(&text) {
                Ok(rel_path) => {
                    println!("Added to {rel_path}");
                    Some(0)
                }
                Err(err) => {
                    eprintln!("krono add error: {err}");
                    Some(1)
                }
            }
        }
    }
}

fn run_add(text: &str) -> Result<String, String> {
    let config = AppConfig::load();
    let today = today_local_cli();
    let nested = daily_note_rel_path(today);
    let legacy = daily_legacy_rel_path(today);

    let (rel_path, mut doc) = if config.vault_dir.join(&nested).exists() {
        let (doc, _, _) = read_note(&config.vault_dir, &nested)?;
        (nested, doc)
    } else if config.vault_dir.join(&legacy).exists() {
        let (doc, _, _) = read_note(&config.vault_dir, &legacy)?;
        (legacy, doc)
    } else {
        let created = create_daily_note_with_template(
            &config.vault_dir,
            today,
            config.language,
            Some(&config.daily_template),
        )?;
        (created.day_path, created.day_doc)
    };

    let hh_mm = Local::now().format("%H:%M").to_string();
    doc.body = append_timestamped_entry(&doc.body, &hh_mm, text);
    let (modified, size) = save_note(&config.vault_dir, &rel_path, &doc)?;

    if let Ok(mut idx) = VaultIndex::open_for_vault(&config.vault_dir) {
        let _ = idx.upsert_note(&rel_path, &doc, modified, size);
    }

    Ok(rel_path)
}
