use crate::storage::vault::list_vault_files;
use eframe::egui;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

pub struct VaultWatcher {
    rx: Receiver<()>,
    vault_dir: Arc<Mutex<PathBuf>>,
    reset_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
}

impl VaultWatcher {
    pub fn spawn(initial_vault: PathBuf, ctx: egui::Context) -> Self {
        let (tx, rx): (Sender<()>, Receiver<()>) = mpsc::channel();
        let vault_dir = Arc::new(Mutex::new(initial_vault));
        let reset_flag = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let dir_clone = Arc::clone(&vault_dir);
        let reset_clone = Arc::clone(&reset_flag);
        let stop_clone = Arc::clone(&stop_flag);

        std::thread::Builder::new()
            .name("krono-vault-watcher".into())
            .spawn(move || {
                let mut prev = snapshot(&dir_clone);
                while !stop_clone.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(2200));
                    if stop_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    if reset_clone.swap(false, Ordering::SeqCst) {
                        prev = snapshot(&dir_clone);
                        continue;
                    }
                    let next = snapshot(&dir_clone);
                    if next != prev {
                        prev = next;
                        if tx.send(()).is_ok() {
                            ctx.request_repaint();
                        }
                    }
                }
            })
            .ok();

        Self {
            rx,
            vault_dir,
            reset_flag,
            stop_flag,
        }
    }

    pub fn set_vault_dir(&self, new_dir: PathBuf) {
        if let Ok(mut guard) = self.vault_dir.lock() {
            *guard = new_dir;
            self.reset_flag.store(true, Ordering::SeqCst);
        }
    }

    /// Call after an internal save/delete to prevent redundant reload notification.
    pub fn mark_internal_change(&self) {
        self.reset_flag.store(true, Ordering::SeqCst);
    }

    pub fn poll_changed(&self) -> bool {
        let mut changed = false;
        while self.rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }
}

impl Drop for VaultWatcher {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

fn snapshot(dir_lock: &Arc<Mutex<PathBuf>>) -> BTreeMap<String, (i64, u64)> {
    let Ok(dir) = dir_lock.lock() else {
        return BTreeMap::new();
    };
    list_vault_files(&dir)
        .into_iter()
        .map(|f| (f.rel_path, (f.modified, f.size)))
        .collect()
}
