use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{AudioFormat, DownloadMode};

/// Live directory of open community instances (no auth required).
pub const INSTANCE_DIRECTORY_URL: &str = "https://cobalt.directory/api/working?type=api";

/// Known-open instances used when the directory is unreachable. Refreshed
/// from the live directory at runtime whenever possible.
pub const SEED_INSTANCES: [&str; 5] = [
    "https://api.qwkuns.me",
    "https://cobalt.alpha.wolfy.love",
    "https://subito-c.meowing.de",
    "https://lime.clxxped.lol",
    "https://nuko-c.meowing.de",
];

/// Response shape of the cobalt.directory "working" endpoint:
/// `{ lastUpdatedUTC, data: { "<service>": ["https://instance", ...] } }`.
#[derive(Clone, Debug, Deserialize)]
pub struct InstanceDirectory {
    #[serde(default)]
    pub data: HashMap<String, Vec<String>>,
}

/// Ranks instances by how many services they currently handle, so the most
/// capable community servers are tried first.
pub fn rank_instances(directory: &InstanceDirectory) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (service, urls) in &directory.data {
        if service.eq_ignore_ascii_case("frontend") {
            continue;
        }
        for url in urls {
            if url.starts_with("https://") {
                *counts.entry(url.trim_end_matches('/')).or_default() += 1;
            }
        }
    }
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    ranked.into_iter().map(|(url, _)| url.to_string()).collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CobaltRequest {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bitrate: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CobaltStatus {
    Tunnel,
    Redirect,
    Picker,
    LocalProcessing,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltPickerItem {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub r#type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltError {
    pub code: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltResponse {
    pub status: CobaltStatus,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub picker: Vec<CobaltPickerItem>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub error: Option<CobaltError>,
}

impl CobaltResponse {
    pub fn is_downloadable(&self) -> bool {
        matches!(self.status, CobaltStatus::Tunnel | CobaltStatus::Redirect)
            && self.url.as_deref().is_some_and(|url| !url.is_empty())
    }
}

pub fn build_cobalt_request(
    url: &str,
    mode: DownloadMode,
    audio_format: AudioFormat,
    audio_bitrate: Option<&str>,
) -> CobaltRequest {
    let is_audio = mode == DownloadMode::Audio;

    CobaltRequest {
        url: url.to_string(),
        download_mode: is_audio.then(|| "audio".to_string()),
        audio_format: is_audio
            .then(|| audio_format.as_cobalt_value().map(str::to_string))
            .flatten(),
        audio_bitrate: is_audio
            .then(|| {
                audio_bitrate
                    .filter(|value| *value != "best")
                    .map(str::to_string)
            })
            .flatten(),
    }
}
