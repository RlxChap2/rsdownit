use serde::{Deserialize, Serialize};

use crate::models::{AudioFormat, DownloadMode, ProviderKind};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApiProviderSettings {
    pub enabled: bool,
    pub base_url: String,
    pub auth_type: String,
    /// Kept in memory for the current session. Never written to settings.json.
    #[serde(default)]
    pub token: String,
    pub timeout_seconds: u64,
}

impl Default for ApiProviderSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            auth_type: "none".to_string(),
            token: String::new(),
            timeout_seconds: 20,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub output_folder: String,
    pub mode: DownloadMode,
    pub audio_format: AudioFormat,
    pub audio_bitrate: String,
    pub provider_order: Vec<ProviderKind>,
    pub concurrency: u8,
    pub cookies_from_browser: bool,
    pub cookie_browser: String,
    pub ffmpeg_path: String,
    pub api_provider: ApiProviderSettings,
    /// Third-party community Cobalt servers. Disabled unless the user opts in.
    pub community_fallback: bool,
}

impl AppSettings {
    /// Loads settings from disk, falling back to defaults (with the user's
    /// Downloads folder pre-selected) so the app works with zero setup.
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Could not create settings dir: {error}"))?;
        }
        let mut persisted = self.clone();
        persisted.api_provider.token.clear();
        let raw = serde_json::to_string_pretty(&persisted)
            .map_err(|error| format!("Could not serialize settings: {error}"))?;
        std::fs::write(path, raw).map_err(|error| format!("Could not save settings: {error}"))
    }
}

pub fn default_output_folder() -> String {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .map(|dir| dir.to_string_lossy().to_string())
        .unwrap_or_default()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_folder: default_output_folder(),
            mode: DownloadMode::Video,
            audio_format: AudioFormat::Best,
            audio_bitrate: "best".to_string(),
            provider_order: vec![
                ProviderKind::Direct,
                ProviderKind::YtDlp,
                ProviderKind::ConfiguredApi,
                ProviderKind::HtmlProbe,
            ],
            concurrency: 2,
            cookies_from_browser: false,
            cookie_browser: "edge".to_string(),
            ffmpeg_path: String::new(),
            api_provider: ApiProviderSettings::default(),
            community_fallback: false,
        }
    }
}

impl AppSettings {
    pub fn normalize(&mut self) {
        self.concurrency = self.concurrency.clamp(1, 8);
        if !matches!(
            self.cookie_browser.as_str(),
            "edge" | "chrome" | "firefox" | "brave" | "vivaldi" | "opera"
        ) {
            self.cookie_browser = "edge".to_string();
        }
        self.api_provider.timeout_seconds = self.api_provider.timeout_seconds.clamp(5, 120);
        if !matches!(
            self.api_provider.auth_type.as_str(),
            "none" | "api-key" | "bearer"
        ) {
            self.api_provider.auth_type = "none".to_string();
        }
        self.api_provider.base_url = self
            .api_provider
            .base_url
            .trim()
            .trim_end_matches('/')
            .to_string();
        if self.api_provider.base_url.is_empty() {
            self.api_provider.enabled = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_untrusted_settings() {
        let mut settings = AppSettings {
            concurrency: 99,
            cookie_browser: "unknown".to_string(),
            api_provider: ApiProviderSettings {
                auth_type: "custom".to_string(),
                timeout_seconds: 999,
                ..ApiProviderSettings::default()
            },
            ..AppSettings::default()
        };
        settings.normalize();
        assert_eq!(settings.concurrency, 8);
        assert_eq!(settings.cookie_browser, "edge");
        assert_eq!(settings.api_provider.auth_type, "none");
        assert_eq!(settings.api_provider.timeout_seconds, 120);
    }

    #[test]
    fn never_persists_api_tokens() {
        let path =
            std::env::temp_dir().join(format!("rsdownit-settings-{}.json", uuid::Uuid::new_v4()));
        let mut settings = AppSettings::default();
        settings.api_provider.token = "session-secret".to_string();
        settings.save(&path).expect("settings save");
        let raw = std::fs::read_to_string(&path).expect("settings file");
        assert!(!raw.contains("session-secret"));
        let _ = std::fs::remove_file(path);
    }
}
