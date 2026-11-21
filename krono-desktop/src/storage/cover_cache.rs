use eframe::egui::{self, ColorImage, TextureHandle, TextureId, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

const MAX_COVER_BYTES: u64 = 5 * 1024 * 1024;
const WORKER_COUNT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverLookup {
    Empty,
    Loading,
    Ready(TextureId),
    Failed,
}

struct CoverJob {
    key: String,
    resolved: ResolvedSource,
    vault_dir: PathBuf,
    ctx: egui::Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ResolvedSource {
    Http(String),
    VaultRelative(String),
}

struct CoverResult {
    key: String,
    vault_dir: PathBuf,
    image: Result<ColorImage, String>,
}

pub struct CoverCache {
    vault_dir: PathBuf,
    textures: HashMap<String, TextureHandle>,
    pending: HashSet<String>,
    failed: HashSet<String>,
    job_tx: mpsc::Sender<CoverJob>,
    res_rx: mpsc::Receiver<CoverResult>,
}

impl CoverCache {
    pub fn new(vault_dir: PathBuf) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<CoverJob>();
        let (res_tx, res_rx) = mpsc::channel::<CoverResult>();
        let shared_rx = Arc::new(Mutex::new(job_rx));

        for idx in 0..WORKER_COUNT {
            let rx = Arc::clone(&shared_rx);
            let tx = res_tx.clone();
            let _ = thread::Builder::new()
                .name(format!("krono-cover-{idx}"))
                .spawn(move || worker_loop(rx, tx));
        }

        Self {
            vault_dir,
            textures: HashMap::new(),
            pending: HashSet::new(),
            failed: HashSet::new(),
            job_tx,
            res_rx,
        }
    }

    pub fn set_vault_dir(&mut self, vault_dir: PathBuf) {
        if self.vault_dir != vault_dir {
            self.vault_dir = vault_dir;
            self.textures.clear();
            self.pending.clear();
            self.failed.clear();
        }
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(res) = self.res_rx.try_recv() {
            if res.vault_dir != self.vault_dir {
                continue;
            }
            self.pending.remove(&res.key);
            match res.image {
                Ok(img) => {
                    let tex = ctx.load_texture(
                        format!("cover:{}", res.key),
                        img,
                        TextureOptions::LINEAR,
                    );
                    self.textures.insert(res.key, tex);
                }
                Err(_) => {
                    self.failed.insert(res.key);
                }
            }
        }
    }

    pub fn get_or_request(&mut self, ctx: &egui::Context, raw_poster: &str) -> CoverLookup {
        let trimmed = raw_poster.trim();
        if trimmed.is_empty() {
            return CoverLookup::Empty;
        }
        let Some(resolved) = resolve_poster_source(trimmed) else {
            return CoverLookup::Failed;
        };
        let key = trimmed.to_string();

        if let Some(tex) = self.textures.get(&key) {
            return CoverLookup::Ready(tex.id());
        }
        if self.failed.contains(&key) {
            return CoverLookup::Failed;
        }
        if self.pending.contains(&key) {
            return CoverLookup::Loading;
        }

        self.pending.insert(key.clone());
        let _ = self.job_tx.send(CoverJob {
            key,
            resolved,
            vault_dir: self.vault_dir.clone(),
            ctx: ctx.clone(),
        });
        CoverLookup::Loading
    }
}

fn worker_loop(
    rx: Arc<Mutex<mpsc::Receiver<CoverJob>>>,
    tx: mpsc::Sender<CoverResult>,
) {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(8))
        .build();

    loop {
        let job = {
            let Ok(guard) = rx.lock() else {
                break;
            };
            guard.recv()
        };
        let Ok(job) = job else {
            break;
        };
        let image = load_cover_image(&agent, &job.vault_dir, &job.resolved);
        let ctx = job.ctx;
        let _ = tx.send(CoverResult {
            key: job.key,
            vault_dir: job.vault_dir,
            image,
        });
        ctx.request_repaint();
    }
}

fn load_cover_image(
    agent: &ureq::Agent,
    vault_dir: &Path,
    source: &ResolvedSource,
) -> Result<ColorImage, String> {
    let bytes = match source {
        ResolvedSource::VaultRelative(rel) => {
            if krono_core::path::validate_rel_path(rel).is_err() {
                return Err("invalid cover path".to_string());
            }
            fs::read(vault_dir.join(rel)).map_err(|e| format!("read cover: {e}"))?
        }
        ResolvedSource::Http(url) => {
            let cache_path = cache_file_path(vault_dir, url);
            if let Ok(cached) = fs::read(&cache_path) {
                if !cached.is_empty() {
                    return decode_rgba_image(&cached);
                }
            }
            let resp = agent
                .get(url)
                .call()
                .map_err(|e| format!("fetch cover: {e}"))?;
            let mut buf = Vec::new();
            resp.into_reader()
                .take(MAX_COVER_BYTES)
                .read_to_end(&mut buf)
                .map_err(|e| format!("read cover body: {e}"))?;
            if buf.is_empty() {
                return Err("empty cover body".to_string());
            }
            if let Some(parent) = cache_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let tmp = cache_path.with_extension("tmp");
            if fs::write(&tmp, &buf).is_ok() {
                let _ = fs::rename(&tmp, &cache_path);
            }
            buf
        }
    };
    decode_rgba_image(&bytes)
}

fn decode_rgba_image(bytes: &[u8]) -> Result<ColorImage, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("decode cover: {e}"))?;
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(ColorImage::from_rgba_unmultiplied(size, rgba.as_raw()))
}

fn resolve_poster_source(raw: &str) -> Option<ResolvedSource> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('/') {
        return Some(ResolvedSource::Http(resolve_tmdb_poster_w342(trimmed)));
    }
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        return Some(ResolvedSource::Http(trimmed.to_string()));
    }
    Some(ResolvedSource::VaultRelative(trimmed.to_string()))
}

pub fn resolve_tmdb_poster_w342(path: &str) -> String {
    crate::sync::tmdb::poster_w342_url(path)
}

pub fn cache_file_path(vault_dir: &Path, key: &str) -> PathBuf {
    vault_dir
        .join(".krono")
        .join("cache")
        .join("covers")
        .join(format!("{:016x}.img", fnv1a_64(key.as_bytes())))
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_tmdb_w342_urls_and_local_paths() {
        assert_eq!(
            resolve_poster_source("/gEU2QniE6E77NI6lCU6MxlNBvIx.jpg"),
            Some(ResolvedSource::Http(
                "https://image.tmdb.org/t/p/w342/gEU2QniE6E77NI6lCU6MxlNBvIx.jpg".into()
            ))
        );
        assert_eq!(
            resolve_poster_source("https://example.com/poster.png"),
            Some(ResolvedSource::Http("https://example.com/poster.png".into()))
        );
        assert_eq!(
            resolve_poster_source("assets/cover.jpg"),
            Some(ResolvedSource::VaultRelative("assets/cover.jpg".into()))
        );
    }

    #[test]
    fn builds_deterministic_cache_path_inside_dot_krono() {
        let vault = Path::new("/tmp/vault");
        let p1 = cache_file_path(vault, "https://image.tmdb.org/t/p/w342/a.jpg");
        let p2 = cache_file_path(vault, "https://image.tmdb.org/t/p/w342/a.jpg");
        let p3 = cache_file_path(vault, "https://image.tmdb.org/t/p/w342/b.jpg");
        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
        let norm = p1.to_string_lossy().replace('\\', "/");
        assert!(norm.contains("/.krono/cache/covers/"));
    }
}
