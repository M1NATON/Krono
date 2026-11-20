use crate::i18n::Lang;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreset {
    #[default]
    #[serde(alias = "Dark")]
    Graphite,
    ObsidianOled,
    Everforest,
    WarmEspresso,
    #[serde(alias = "Light")]
    Paper,
    EditorialSand,
}

impl ThemePreset {
    pub const ALL: [ThemePreset; 6] = [
        Self::Graphite,
        Self::ObsidianOled,
        Self::Everforest,
        Self::WarmEspresso,
        Self::Paper,
        Self::EditorialSand,
    ];

    pub fn is_dark(self) -> bool {
        matches!(
            self,
            Self::Graphite | Self::ObsidianOled | Self::Everforest | Self::WarmEspresso
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccentPreset {
    #[default]
    SteelBlue,
    Sage,
    Amber,
    Terracotta,
    GraphiteMono,
}

impl AccentPreset {
    pub const ALL: [AccentPreset; 5] = [
        Self::SteelBlue,
        Self::Sage,
        Self::Amber,
        Self::Terracotta,
        Self::GraphiteMono,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EditorWidth {
    Compact,
    #[default]
    Normal,
    Wide,
}

impl EditorWidth {
    pub fn max_px(self) -> f32 {
        match self {
            Self::Compact => 640.0,
            Self::Normal => 720.0,
            Self::Wide => 860.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlashTrigger {
    #[default]
    Slash,
    Backslash,
}

impl SlashTrigger {
    pub fn as_char(self) -> char {
        match self {
            Self::Slash => '/',
            Self::Backslash => '\\',
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomSlashSnippet {
    pub trigger: String,
    pub label: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub vault_dir: PathBuf,
    pub language: Lang,
    pub theme: ThemePreset,
    pub last_dark_theme: ThemePreset,
    pub last_light_theme: ThemePreset,
    pub accent: AccentPreset,
    pub editor_font_size: f32,
    pub heading_serif: bool,
    pub editor_width: EditorWidth,
    pub show_editor_footer: bool,
    pub auto_save_enabled: bool,
    pub md_slash_menu: bool,
    pub md_wikilink_autocomplete: bool,
    pub md_auto_lists: bool,
    pub md_auto_pairs: bool,
    pub md_show_toolbar: bool,
    pub md_slash_trigger: SlashTrigger,
    pub custom_slash_snippets: Vec<CustomSlashSnippet>,
    pub calendar_open: bool,
    pub auto_sync_interval_mins: u32,
    pub github_repo: String,
    pub github_branch: String,
    pub github_token: String,
    pub auto_sync_on_start: bool,
    #[serde(default)]
    pub auto_sync_after_save: bool,
    pub sidebar_open: bool,
    pub inspector_open: bool,
    pub sidebar_width: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            vault_dir: default_vault_dir(),
            language: Lang::Ru,
            theme: ThemePreset::Graphite,
            last_dark_theme: ThemePreset::Graphite,
            last_light_theme: ThemePreset::Paper,
            accent: AccentPreset::SteelBlue,
            editor_font_size: 15.0,
            heading_serif: true,
            editor_width: EditorWidth::Normal,
            show_editor_footer: true,
            auto_save_enabled: true,
            md_slash_menu: true,
            md_wikilink_autocomplete: true,
            md_auto_lists: true,
            md_auto_pairs: true,
            md_show_toolbar: true,
            md_slash_trigger: SlashTrigger::Slash,
            custom_slash_snippets: Vec::new(),
            calendar_open: false,
            auto_sync_interval_mins: 0,
            github_repo: String::new(),
            github_branch: "main".to_string(),
            github_token: String::new(),
            auto_sync_on_start: false,
            auto_sync_after_save: false,
            sidebar_open: true,
            inspector_open: false,
            sidebar_width: 244.0,
        }
    }
}

pub fn default_vault_dir() -> PathBuf {
    let cwd_vault = PathBuf::from("./krono_vault");
    if cwd_vault.is_dir() {
        return cwd_vault.canonicalize().unwrap_or(cwd_vault);
    }
    if let Some(proj) = ProjectDirs::from("io", "krono", "Krono") {
        return proj.data_local_dir().join("vault");
    }
    PathBuf::from("./krono_vault")
}

pub fn config_file_path() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("io", "krono", "Krono") {
        return proj.config_dir().join("config.json");
    }
    PathBuf::from("./krono_config.json")
}

pub fn crash_log_path() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("io", "krono", "Krono") {
        return proj.config_dir().join("crash.log");
    }
    PathBuf::from("./krono_crash.log")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_file_path();
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut cfg) = serde_json::from_str::<AppConfig>(&raw) {
                if cfg.github_branch.trim().is_empty() {
                    cfg.github_branch = "main".to_string();
                }
                cfg.sidebar_width = cfg.sidebar_width.clamp(200.0, 360.0);
                cfg.editor_font_size = cfg.editor_font_size.clamp(14.0, 18.0);
                if !cfg.last_dark_theme.is_dark() {
                    cfg.last_dark_theme = ThemePreset::Graphite;
                }
                if cfg.last_light_theme.is_dark() {
                    cfg.last_light_theme = ThemePreset::Paper;
                }
                if cfg.theme.is_dark() {
                    cfg.last_dark_theme = cfg.theme;
                } else {
                    cfg.last_light_theme = cfg.theme;
                }
                if !matches!(cfg.auto_sync_interval_mins, 0 | 5 | 15) {
                    cfg.auto_sync_interval_mins = 0;
                }
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        write_file_atomic(&path, json.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn select_theme(&mut self, preset: ThemePreset) {
        self.theme = preset;
        if preset.is_dark() {
            self.last_dark_theme = preset;
        } else {
            self.last_light_theme = preset;
        }
    }

    pub fn toggle_theme_polarity(&mut self) {
        if self.theme.is_dark() {
            let next = if !self.last_light_theme.is_dark() {
                self.last_light_theme
            } else {
                ThemePreset::Paper
            };
            self.theme = next;
        } else {
            let next = if self.last_dark_theme.is_dark() {
                self.last_dark_theme
            } else {
                ThemePreset::Graphite
            };
            self.theme = next;
        }
    }

    pub fn is_github_configured(&self) -> bool {
        normalize_github_repo(&self.github_repo).is_some()
            && !self.github_token.trim().is_empty()
            && !self.github_branch.trim().is_empty()
    }
}

pub fn open_in_file_manager(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir_all(path).map_err(|e| format!("create dir: {e}"))?;
    }
    #[cfg(target_os = "windows")]
    let res = Command::new("explorer").arg(path).spawn();

    #[cfg(target_os = "macos")]
    let res = Command::new("open").arg(path).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let res = Command::new("xdg-open").arg(path).spawn();

    res.map(|_| ())
        .map_err(|e| format!("open folder failed: {e}"))
}

pub fn write_file_atomic(target: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create parent dir: {e}"))?;
    }
    let tmp = target.with_extension("krono.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("write temp file: {e}"))?;
    if fs::rename(&tmp, target).is_err() {
        if target.exists() {
            let _ = fs::remove_file(target);
        }
        fs::rename(&tmp, target).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("rename temp file: {e}")
        })?;
    }
    Ok(())
}

/// Accepts `"owner/repo"` or `"https://github.com/owner/repo(.git)"`.
pub fn normalize_github_repo(input: &str) -> Option<(String, String)> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let stripped = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .unwrap_or(trimmed);
    let stripped = stripped.strip_suffix(".git").unwrap_or(stripped);
    let mut parts = stripped.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    if owner.contains(' ') || repo.contains(' ') {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_github_repo_formats() {
        assert_eq!(
            normalize_github_repo("alice/krono-vault"),
            Some(("alice".into(), "krono-vault".into()))
        );
        assert_eq!(
            normalize_github_repo("https://github.com/alice/krono-vault.git/"),
            Some(("alice".into(), "krono-vault".into()))
        );
        assert_eq!(normalize_github_repo("invalid"), None);
        assert_eq!(normalize_github_repo("a/b/c"), None);
    }

    #[test]
    fn migrates_legacy_dark_light_config() {
        let dark_cfg: AppConfig = serde_json::from_str(r#"{"theme":"Dark"}"#).unwrap();
        assert_eq!(dark_cfg.theme, ThemePreset::Graphite);

        let light_cfg: AppConfig = serde_json::from_str(r#"{"theme":"Light"}"#).unwrap();
        assert_eq!(light_cfg.theme, ThemePreset::Paper);
        assert!(light_cfg.md_slash_menu);
        assert!(light_cfg.md_wikilink_autocomplete);
        assert!(light_cfg.md_auto_lists);
        assert!(light_cfg.md_auto_pairs);
        assert!(light_cfg.md_show_toolbar);
        assert_eq!(light_cfg.md_slash_trigger, SlashTrigger::Slash);
        assert!(!light_cfg.calendar_open);
        assert!(light_cfg.custom_slash_snippets.is_empty());
    }
}
