use crate::i18n::Lang;
use crate::sync::github::{sync_with_github, test_connection, RepoInfo, SyncSummary};
use eframe::egui;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

#[derive(Clone, Debug)]
pub enum SyncEvent {
    TestResult(Result<RepoInfo, String>),
    SyncResult(Result<SyncSummary, String>),
}

pub struct SyncWorker {
    tx: Sender<SyncEvent>,
    rx: Receiver<SyncEvent>,
    busy: bool,
}

impl SyncWorker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            busy: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn start_test(
        &mut self,
        repo: String,
        token: String,
        lang: Lang,
        ctx: egui::Context,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("krono-github-test".into())
            .spawn(move || {
                let res = test_connection(&repo, &token, lang);
                let _ = tx.send(SyncEvent::TestResult(res));
                ctx.request_repaint();
            })
            .ok();
    }

    pub fn start_sync(
        &mut self,
        vault_dir: PathBuf,
        base_hashes: HashMap<String, String>,
        repo: String,
        branch: String,
        token: String,
        lang: Lang,
        ctx: egui::Context,
    ) {
        if self.busy {
            return;
        }
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("krono-github-sync".into())
            .spawn(move || {
                let res =
                    sync_with_github(&vault_dir, &base_hashes, &repo, &branch, &token, lang);
                let _ = tx.send(SyncEvent::SyncResult(res));
                ctx.request_repaint();
            })
            .ok();
    }

    pub fn poll(&mut self) -> Vec<SyncEvent> {
        let mut events = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            self.busy = false;
            events.push(ev);
        }
        events
    }
}
