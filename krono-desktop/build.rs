use std::path::Path;
use std::process::Command;

fn normalize_version(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn git_latest_tag(repo_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    normalize_version(&text)
}

fn main() {
    println!("cargo:rerun-if-env-changed=KRONO_VERSION");

    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let repo_root = manifest_dir.parent().unwrap_or(&manifest_dir);

    for rel in [".git/HEAD", ".git/refs/tags", ".git/packed-refs"] {
        let p = repo_root.join(rel);
        if p.exists() {
            println!("cargo:rerun-if-changed={}", p.display());
        }
    }

    let version = std::env::var("KRONO_VERSION")
        .ok()
        .and_then(|v| normalize_version(&v))
        .or_else(|| git_latest_tag(repo_root))
        .or_else(|| {
            std::env::var("CARGO_PKG_VERSION")
                .ok()
                .and_then(|v| normalize_version(&v))
        })
        .unwrap_or_else(|| "0.3.1".to_string());

    println!("cargo:rustc-env=KRONO_APP_VERSION={version}");
}
