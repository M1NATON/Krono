use crate::config::open_in_file_manager;
use crate::i18n::Lang;
use eframe::egui;
use serde::Deserialize;
use std::{
    cmp::Ordering,
    fs,
    io::Read,
    path::PathBuf,
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    time::Duration,
};

pub const DEFAULT_UPDATE_REPO: &str = "M1NATON/Krono";
pub const CURRENT_APP_VERSION: &str = env!("KRONO_APP_VERSION");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// `None` means a stable final release (ranks higher than `Some(...)` of the same M.m.p).
    pub pre: Option<String>,
}

impl SemVer {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim().trim_start_matches(['v', 'V']);
        if trimmed.is_empty() {
            return None;
        }
        let (core, pre) = match trimmed.split_once('-') {
            Some((c, p)) => (c, Some(p.trim().to_lowercase())),
            None => (trimmed, None),
        };
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u32>().ok()?;
        let minor = parts.next()?.parse::<u32>().ok()?;
        let patch = parts.next().unwrap_or("0").parse::<u32>().ok()?;
        Some(Self {
            major,
            minor,
            patch,
            pre: pre.filter(|s| !s.is_empty()),
        })
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            Ordering::Equal => match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                // Stable release > pre-release of the same major.minor.patch
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => compare_prerelease_strings(a, b),
            },
            ord => ord,
        }
    }
}

fn compare_prerelease_strings(a: &str, b: &str) -> Ordering {
    for (pa, pb) in a.split('.').zip(b.split('.')) {
        let ord = match (pa.parse::<u32>(), pb.parse::<u32>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => pa.cmp(pb),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.split('.').count().cmp(&b.split('.').count())
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AvailableUpdate {
    pub tag_name: String,
    pub title: String,
    pub body: String,
    pub prerelease: bool,
    pub html_url: String,
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
    pub asset_sha256: Option<String>,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum UpdateApplyOutcome {
    /// Windows installer spawned; app should flush & exit so installer can replace executable.
    LaunchedInstallerExitApp(String),
    /// Linux AppImage replaced in-place & relaunched; app should exit.
    ReplacedAppImageExitApp(String),
    /// Opened DMG / folder for user confirmation.
    OpenedPackage(String),
}

#[derive(Clone, Debug)]
pub enum UpdateEvent {
    CheckFinished {
        update: Result<Option<AvailableUpdate>, String>,
        silent: bool,
    },
    InstallFinished(Result<UpdateApplyOutcome, String>),
}

pub struct UpdateWorker {
    tx: Sender<UpdateEvent>,
    rx: Receiver<UpdateEvent>,
    checking: bool,
    installing: bool,
}

impl UpdateWorker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            checking: false,
            installing: false,
        }
    }

    pub fn is_checking(&self) -> bool {
        self.checking
    }

    pub fn is_installing(&self) -> bool {
        self.installing
    }

    pub fn start_check(&mut self, include_prerelease: bool, silent: bool, lang: Lang, ctx: egui::Context) {
        if self.checking || self.installing {
            return;
        }
        self.checking = true;
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("krono-update-check".into())
            .spawn(move || {
                let res = check_latest_release(DEFAULT_UPDATE_REPO, include_prerelease, lang);
                let _ = tx.send(UpdateEvent::CheckFinished {
                    update: res,
                    silent,
                });
                ctx.request_repaint();
            })
            .ok();
    }

    pub fn start_install(&mut self, update: AvailableUpdate, lang: Lang, ctx: egui::Context) {
        if self.installing {
            return;
        }
        self.installing = true;
        let tx = self.tx.clone();
        std::thread::Builder::new()
            .name("krono-update-install".into())
            .spawn(move || {
                let res = download_and_apply_update(&update, lang);
                let _ = tx.send(UpdateEvent::InstallFinished(res));
                ctx.request_repaint();
            })
            .ok();
    }

    pub fn poll(&mut self) -> Vec<UpdateEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.rx.try_recv() {
            match &ev {
                UpdateEvent::CheckFinished { .. } => self.checking = false,
                UpdateEvent::InstallFinished(_) => self.installing = false,
            }
            out.push(ev);
        }
        out
    }
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Clone, Deserialize)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub digest: Option<String>,
}

pub fn pick_platform_asset<'a>(assets: &'a [GhAsset], os: &str, arch: &str) -> Option<&'a GhAsset> {
    let find_suffix = |suf: &str| -> Option<&'a GhAsset> {
        assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(&suf.to_lowercase()))
    };
    match os {
        "windows" => find_suffix("-windows-x86_64-setup.exe")
            .or_else(|| find_suffix("-windows-x86_64.exe"))
            .or_else(|| find_suffix("-windows-x86_64.zip")),
        "linux" => find_suffix("-linux-x86_64.AppImage")
            .or_else(|| find_suffix("-linux-x86_64.tar.gz")),
        "macos" => {
            if arch == "aarch64" {
                find_suffix("-macos-arm64.dmg").or_else(|| find_suffix("-macos-x86_64.dmg"))
            } else {
                find_suffix("-macos-x86_64.dmg").or_else(|| find_suffix("-macos-arm64.dmg"))
            }
        }
        _ => None,
    }
}

fn check_latest_release(
    repo: &str,
    include_prerelease: bool,
    lang: Lang,
) -> Result<Option<AvailableUpdate>, String> {
    let cur_ver = SemVer::parse(CURRENT_APP_VERSION).ok_or_else(|| "invalid current version".to_string())?;
    let url = format!("https://api.github.com/repos/{repo}/releases?per_page=10");
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(15))
        .build();

    let resp = agent
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("User-Agent", "Krono-Desktop-Updater")
        .call()
        .map_err(|e| match lang {
            Lang::Ru => format!("Не удалось проверить обновления: {e}"),
            Lang::En => format!("Failed to check for updates: {e}"),
        })?;

    let releases: Vec<GhRelease> = resp.into_json().map_err(|e| format!("parse releases JSON: {e}"))?;
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    let mut best: Option<(SemVer, GhRelease, GhAsset)> = None;
    for rel in releases {
        if rel.draft || (!include_prerelease && rel.prerelease) {
            continue;
        }
        let Some(ver) = SemVer::parse(&rel.tag_name) else {
            continue;
        };
        if ver <= cur_ver {
            continue;
        }
        let Some(asset) = pick_platform_asset(&rel.assets, os, arch).cloned() else {
            continue;
        };
        if best.as_ref().is_none_or(|(bver, _, _)| &ver > bver) {
            best = Some((ver, rel, asset));
        }
    }

    Ok(best.map(|(_, rel, asset)| {
        let sha = asset
            .digest
            .as_deref()
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(|s| s.trim().to_lowercase());
        AvailableUpdate {
            tag_name: rel.tag_name.clone(),
            title: if rel.name.trim().is_empty() {
                rel.tag_name
            } else {
                rel.name
            },
            body: rel.body,
            prerelease: rel.prerelease,
            html_url: rel.html_url,
            asset_name: asset.name,
            asset_url: asset.browser_download_url,
            asset_size: asset.size,
            asset_sha256: sha,
        }
    }))
}

fn download_and_apply_update(
    update: &AvailableUpdate,
    lang: Lang,
) -> Result<UpdateApplyOutcome, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(120))
        .build();

    let resp = agent
        .get(&update.asset_url)
        .set("User-Agent", "Krono-Desktop-Updater")
        .call()
        .map_err(|e| match lang {
            Lang::Ru => format!("Ошибка скачивания обновления: {e}"),
            Lang::En => format!("Failed to download update: {e}"),
        })?;

    let mut bytes = Vec::with_capacity(update.asset_size.min(64 * 1024 * 1024) as usize);
    resp.into_reader()
        .take(128 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read update stream: {e}"))?;

    if bytes.is_empty() {
        return Err(match lang {
            Lang::Ru => "Скачанный файл обновления пуст".into(),
            Lang::En => "Downloaded update file is empty".into(),
        });
    }

    if let Some(expected_sha) = &update.asset_sha256 {
        let actual_sha = sha256_hex(&bytes);
        if &actual_sha != expected_sha {
            return Err(match lang {
                Lang::Ru => format!(
                    "Контрольная сумма SHA-256 не совпала (ожидалось {}, получено {})",
                    &expected_sha[..12.min(expected_sha.len())],
                    &actual_sha[..12.min(actual_sha.len())]
                ),
                Lang::En => format!(
                    "SHA-256 checksum mismatch (expected {}, got {})",
                    &expected_sha[..12.min(expected_sha.len())],
                    &actual_sha[..12.min(actual_sha.len())]
                ),
            });
        }
    }

    let update_dir = std::env::temp_dir().join("krono_updates");
    fs::create_dir_all(&update_dir).map_err(|e| format!("create update dir: {e}"))?;
    let safe_name = update
        .asset_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .collect::<String>();
    let dest_path = update_dir.join(if safe_name.is_empty() {
        "krono_update.bin"
    } else {
        &safe_name
    });
    fs::write(&dest_path, &bytes).map_err(|e| format!("write update file: {e}"))?;

    apply_downloaded_package(&dest_path, &bytes, lang)
}

#[allow(unused_variables)]
fn apply_downloaded_package(
    dest_path: &PathBuf,
    bytes: &[u8],
    lang: Lang,
) -> Result<UpdateApplyOutcome, String> {
    let fname = dest_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    #[cfg(target_os = "windows")]
    {
        if fname.ends_with(".exe") {
            Command::new(dest_path)
                .arg("/SILENT")
                .spawn()
                .map_err(|e| format!("launch installer: {e}"))?;
            return Ok(UpdateApplyOutcome::LaunchedInstallerExitApp(
                match lang {
                    Lang::Ru => "Запущен установщик обновления…".into(),
                    Lang::En => "Launching update installer…".into(),
                },
            ));
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(dest_path, fs::Permissions::from_mode(0o755));
        if fname.ends_with(".appimage") {
            if let Some(cur_appimage) = std::env::var_os("APPIMAGE").map(PathBuf::from) {
                let tmp_replace = cur_appimage.with_extension("AppImage.new");
                if fs::write(&tmp_replace, bytes).is_ok() {
                    let _ = fs::set_permissions(&tmp_replace, fs::Permissions::from_mode(0o755));
                    if fs::rename(&tmp_replace, &cur_appimage).is_ok() {
                        let _ = Command::new(&cur_appimage).spawn();
                        return Ok(UpdateApplyOutcome::ReplacedAppImageExitApp(
                            match lang {
                                Lang::Ru => "AppImage обновлён, перезапуск…".into(),
                                Lang::En => "AppImage updated, restarting…".into(),
                            },
                        ));
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if fname.ends_with(".dmg") {
            Command::new("open")
                .arg(dest_path)
                .spawn()
                .map_err(|e| format!("open dmg: {e}"))?;
            return Ok(UpdateApplyOutcome::OpenedPackage(match lang {
                Lang::Ru => "Образ .dmg открыт в Finder — перетащите Krono.app в Applications".into(),
                Lang::En => "Opened .dmg in Finder — drag Krono.app into Applications".into(),
            }));
        }
    }

    if let Some(parent) = dest_path.parent() {
        let _ = open_in_file_manager(parent);
    }
    Ok(UpdateApplyOutcome::OpenedPackage(match lang {
        Lang::Ru => format!("Обновление скачано: {}", dest_path.display()),
        Lang::En => format!("Update downloaded: {}", dest_path.display()),
    }))
}

/// Pure Rust FIPS 180-4 SHA-256 implementation for verifying GitHub Release asset digests
/// without adding extra external crate dependencies.
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(((data.len() + 9 + 63) / 64) * 64);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    format!(
        "{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
        h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_and_prereleases_correctly() {
        let b1 = SemVer::parse("v0.2.0-beta.1").unwrap();
        let b2 = SemVer::parse("v0.2.0-beta.2").unwrap();
        let b10 = SemVer::parse("0.2.0-beta.10").unwrap();
        let stable = SemVer::parse("v0.2.0").unwrap();
        let next = SemVer::parse("v0.2.1").unwrap();

        assert!(b2 > b1);
        assert!(b10 > b2);
        assert!(stable > b10);
        assert!(next > stable);
    }

    #[test]
    fn computes_fips_sha256_accurately() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn picks_preferred_installer_assets_per_platform() {
        let assets = vec![
            GhAsset {
                name: "krono-v0.2.0-beta.1-windows-x86_64.zip".into(),
                browser_download_url: "https://example.com/win.zip".into(),
                size: 10,
                digest: None,
            },
            GhAsset {
                name: "krono-v0.2.0-beta.1-windows-x86_64-setup.exe".into(),
                browser_download_url: "https://example.com/setup.exe".into(),
                size: 20,
                digest: None,
            },
            GhAsset {
                name: "krono-v0.2.0-beta.1-linux-x86_64.tar.gz".into(),
                browser_download_url: "https://example.com/lin.tar.gz".into(),
                size: 30,
                digest: None,
            },
            GhAsset {
                name: "krono-v0.2.0-beta.1-linux-x86_64.AppImage".into(),
                browser_download_url: "https://example.com/lin.AppImage".into(),
                size: 40,
                digest: None,
            },
            GhAsset {
                name: "krono-v0.2.0-beta.1-macos-arm64.dmg".into(),
                browser_download_url: "https://example.com/arm.dmg".into(),
                size: 50,
                digest: None,
            },
            GhAsset {
                name: "krono-v0.2.0-beta.1-macos-x86_64.dmg".into(),
                browser_download_url: "https://example.com/intel.dmg".into(),
                size: 60,
                digest: None,
            },
        ];

        assert_eq!(
            pick_platform_asset(&assets, "windows", "x86_64").unwrap().name,
            "krono-v0.2.0-beta.1-windows-x86_64-setup.exe"
        );
        assert_eq!(
            pick_platform_asset(&assets, "linux", "x86_64").unwrap().name,
            "krono-v0.2.0-beta.1-linux-x86_64.AppImage"
        );
        assert_eq!(
            pick_platform_asset(&assets, "macos", "aarch64").unwrap().name,
            "krono-v0.2.0-beta.1-macos-arm64.dmg"
        );
        assert_eq!(
            pick_platform_asset(&assets, "macos", "x86_64").unwrap().name,
            "krono-v0.2.0-beta.1-macos-x86_64.dmg"
        );
    }

    #[test]
    fn current_app_version_is_valid_semver() {
        assert!(SemVer::parse(CURRENT_APP_VERSION).is_some());
    }
}
