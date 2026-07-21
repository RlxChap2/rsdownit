//! Download jobs, provider fallback, cancellation, and progress events.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::models::{DownloadMode, DownloadRequest, ProviderKind};
use crate::providers::cobalt::{
    build_cobalt_request, rank_instances, CobaltResponse, InstanceDirectory,
    INSTANCE_DIRECTORY_URL, SEED_INSTANCES,
};
use crate::providers::direct::{
    extract_media_links_from_html, is_direct_media_url, is_stream_manifest_url,
};
use crate::providers::yt_dlp::build_ytdlp_args;
use crate::security::{
    is_safe_download_filename, secure_get, secure_redirect_policy, validate_api_endpoint,
    validate_remote_target, validate_remote_url,
};
use crate::settings::AppSettings;
use crate::storage::{next_available_path, sanitize_filename};
use crate::tools;

pub const DOWNLOAD_EVENT: &str = "download://update";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobUpdate {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JobUpdate {
    fn new(id: &str, status: &str) -> Self {
        Self {
            id: id.to_string(),
            status: status.to_string(),
            title: None,
            provider: None,
            progress: None,
            speed: None,
            eta: None,
            detail: None,
            file_path: None,
            error: None,
        }
    }
}

/// Job progress callback; wired to the `download://update` event in the app.
pub type JobSink<'a> = &'a (dyn Fn(JobUpdate) + Send + Sync);

/// Resolved external tool locations for one job run.
pub struct EngineTools {
    pub yt_dlp_path: Option<String>,
    /// Directory containing a managed ffmpeg.exe, when the system has none.
    pub ffmpeg_dir: Option<String>,
}

#[derive(Clone)]
pub struct JobManager {
    cancel_flags: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
    slots: Arc<tokio::sync::Semaphore>,
}

impl Default for JobManager {
    fn default() -> Self {
        Self {
            cancel_flags: Arc::new(Mutex::new(HashMap::new())),
            slots: Arc::new(tokio::sync::Semaphore::new(2)),
        }
    }
}

impl JobManager {
    pub fn register(&self, id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut flags) = self.cancel_flags.lock() {
            flags.insert(id.to_string(), flag.clone());
        }
        flag
    }

    pub fn cancel(&self, id: &str) -> bool {
        if let Ok(flags) = self.cancel_flags.lock() {
            if let Some(flag) = flags.get(id) {
                flag.store(true, Ordering::SeqCst);
                return true;
            }
        }
        false
    }

    pub fn finish(&self, id: &str) {
        if let Ok(mut flags) = self.cancel_flags.lock() {
            flags.remove(id);
        }
    }

    pub async fn wait_for_slot(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.slots
            .clone()
            .acquire_owned()
            .await
            .expect("download semaphore remains open")
    }
}

struct Cancelled;

fn check_cancel(cancel: &AtomicBool) -> Result<(), Cancelled> {
    if cancel.load(Ordering::SeqCst) {
        Err(Cancelled)
    } else {
        Ok(())
    }
}

fn format_bytes_per_sec(bytes_per_sec: f64) -> String {
    if bytes_per_sec >= 1_048_576.0 {
        format!("{:.1} MB/s", bytes_per_sec / 1_048_576.0)
    } else if bytes_per_sec >= 1024.0 {
        format!("{:.0} KB/s", bytes_per_sec / 1024.0)
    } else {
        format!("{bytes_per_sec:.0} B/s")
    }
}

fn format_eta(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "--".to_string();
    }
    let seconds = seconds.round() as u64;
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
}

fn filename_from_url(url: &str) -> String {
    let base = url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed.path_segments().and_then(|mut segments| {
                segments
                    .rfind(|segment| !segment.is_empty())
                    .map(str::to_string)
            })
        })
        .map(|segment| percent_decode_str(&segment).decode_utf8_lossy().to_string())
        .unwrap_or_default();
    let cleaned = sanitize_filename(&base);
    if cleaned == "download" && !cleaned.contains('.') {
        "download.bin".to_string()
    } else {
        cleaned
    }
}

/// Streams a URL straight to the output folder. Used by the direct provider
/// and to fetch resolved links from the API and HTML providers.
pub async fn stream_to_file(
    sink: JobSink<'_>,
    id: &str,
    provider: ProviderKind,
    url: &str,
    output_dir: &str,
    file_name_hint: Option<String>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let response = secure_get(
        url,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) rsdownit",
        std::time::Duration::from_secs(30),
    )
    .await?
    .error_for_status()
    .map_err(|error| format!("Server rejected the download: {error}"))?;

    if response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            let value = value.to_ascii_lowercase();
            value.starts_with("text/html") || value.starts_with("application/xhtml")
        })
    {
        return Err("The resolved link returned a web page instead of media.".to_string());
    }

    let total = response.content_length();
    let name = file_name_hint
        .map(|hint| sanitize_filename(&hint))
        .unwrap_or_else(|| filename_from_url(url));
    if !is_safe_download_filename(&name) {
        return Err("The server suggested an unsafe executable filename.".to_string());
    }
    let target = next_available_path(Path::new(output_dir).join(name), |path| path.exists());
    let temp = target.with_extension(format!(
        "{}.part",
        target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin")
    ));

    let mut file = tokio::fs::File::create(&temp)
        .await
        .map_err(|error| format!("Could not create file: {error}"))?;

    let started = Instant::now();
    let mut downloaded: u64 = 0;
    let mut last_emit = Instant::now();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if check_cancel(cancel).is_err() {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err("cancelled".to_string());
        }
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                drop(file);
                let _ = tokio::fs::remove_file(&temp).await;
                return Err(format!("Connection interrupted: {error}"));
            }
        };
        if let Err(error) = file.write_all(&chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(format!("Could not write file: {error}"));
        }
        downloaded += chunk.len() as u64;

        if last_emit.elapsed().as_millis() > 250 {
            let elapsed = started.elapsed().as_secs_f64().max(0.001);
            let speed = downloaded as f64 / elapsed;
            let mut update = JobUpdate::new(id, "downloading");
            update.provider = Some(provider);
            update.speed = Some(format_bytes_per_sec(speed));
            if let Some(total) = total {
                update.progress = Some((downloaded as f64 / total as f64) * 100.0);
                update.eta = Some(format_eta((total - downloaded) as f64 / speed));
            }
            sink(update);
            last_emit = Instant::now();
        }
    }

    if let Err(error) = file.flush().await {
        drop(file);
        let _ = tokio::fs::remove_file(&temp).await;
        return Err(format!("Could not finish file: {error}"));
    }
    drop(file);
    tokio::fs::rename(&temp, &target)
        .await
        .map_err(|error| format!("Could not finalize file: {error}"))?;
    Ok(target)
}

fn parse_progress_line(line: &str) -> Option<(f64, String, String)> {
    let mut parts = line.trim().split('|');
    let percent = parts.next()?.trim().trim_end_matches('%').trim();
    let percent: f64 = percent.parse().ok()?;
    let speed = parts.next().unwrap_or("--").trim().to_string();
    let eta = parts.next().unwrap_or("--").trim().to_string();
    Some((percent, speed, eta))
}

async fn terminate_process_tree(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    if let Some(pid) = child.id() {
        let mut command = tokio::process::Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x0800_0000);
        let _ = command.status().await;
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// Runs yt-dlp as a child process and translates its output into job events.
pub async fn ytdlp_download(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    engine_tools: &EngineTools,
    concurrent_fragments: u8,
    cookie_browser: Option<&str>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let Some(tool_path) = engine_tools.yt_dlp_path.as_ref() else {
        return Err("yt-dlp is not installed yet.".to_string());
    };

    let output_template = Path::new(&request.output_dir)
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string();
    let mut args = build_ytdlp_args(
        &request.url,
        request.mode.clone(),
        request.video_quality,
        request.audio_format,
        &request.audio_bitrate,
        &output_template,
        concurrent_fragments,
    );

    if let Some(browser) = cookie_browser {
        let insert_at = args.len().saturating_sub(1);
        args.insert(insert_at, "--cookies-from-browser".to_string());
        args.insert(insert_at + 1, browser.to_string());
    }

    // Surface the media title and final file path over stdout markers.
    let extra = [
        "--no-simulate",
        "--no-warnings",
        "--print",
        "before_dl:rsd_title:%(title)s",
        "--print",
        "after_move:rsd_path:%(filepath)s",
    ];
    for (index, value) in extra.iter().enumerate() {
        args.insert(index, value.to_string());
    }

    // Point yt-dlp at the managed FFmpeg when the system has none.
    if let Some(dir) = engine_tools.ffmpeg_dir.as_ref() {
        args.insert(0, "--ffmpeg-location".to_string());
        args.insert(1, dir.clone());
    }

    let mut command = tokio::process::Command::new(tool_path);
    command
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start yt-dlp: {error}"))?;

    let stdout = child.stdout.take().ok_or("yt-dlp produced no output.")?;
    let stderr = child.stderr.take();

    let stderr_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if !line.trim().is_empty() {
                    lines.push(line);
                }
            }
        }
        lines
    });

    let mut final_path: Option<PathBuf> = None;
    let mut reader = BufReader::new(stdout).lines();
    loop {
        let line = tokio::select! {
            line = reader.next_line() => line.map_err(|error| format!("yt-dlp output error: {error}"))?,
            _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
                if check_cancel(cancel).is_err() {
                    terminate_process_tree(&mut child).await;
                    return Err("cancelled".to_string());
                }
                continue;
            }
        };
        let Some(line) = line else {
            break;
        };
        if check_cancel(cancel).is_err() {
            terminate_process_tree(&mut child).await;
            return Err("cancelled".to_string());
        }

        if let Some(title) = line.strip_prefix("rsd_title:") {
            let mut update = JobUpdate::new(id, "downloading");
            update.provider = Some(ProviderKind::YtDlp);
            update.title = Some(title.trim().to_string());
            sink(update);
        } else if let Some(path) = line.strip_prefix("rsd_path:") {
            final_path = Some(PathBuf::from(path.trim()));
        } else if line.contains("[ExtractAudio]") || line.contains("[Merger]") {
            let mut update = JobUpdate::new(id, "converting");
            update.provider = Some(ProviderKind::YtDlp);
            update.detail = Some("Processing media".to_string());
            sink(update);
        } else if let Some((percent, speed, eta)) = parse_progress_line(&line) {
            let mut update = JobUpdate::new(id, "downloading");
            update.provider = Some(ProviderKind::YtDlp);
            update.progress = Some(percent);
            update.speed = Some(speed);
            update.eta = Some(eta);
            sink(update);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|error| format!("yt-dlp crashed: {error}"))?;
    if !status.success() {
        let stderr_lines = stderr_task.await.unwrap_or_default();
        let reason = stderr_lines
            .iter()
            .rev()
            .find(|line| line.contains("ERROR"))
            .or_else(|| stderr_lines.last())
            .cloned()
            .unwrap_or_else(|| "yt-dlp could not process this link.".to_string());
        return Err(reason);
    }

    final_path.ok_or_else(|| "yt-dlp finished without a file.".to_string())
}

/// Calls one Cobalt-compatible endpoint and streams the file it resolves.
struct CobaltEndpoint<'a> {
    provider: ProviderKind,
    base_url: &'a str,
    auth_header: Option<String>,
    timeout_seconds: u64,
}

async fn cobalt_fetch(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    endpoint: CobaltEndpoint<'_>,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    validate_api_endpoint(endpoint.base_url)?;
    let payload = build_cobalt_request(
        &request.url,
        request.mode.clone(),
        request.audio_format,
        Some(request.audio_bitrate.as_str()),
    );

    let client = reqwest::Client::builder()
        .user_agent("rsdownit")
        .redirect(secure_redirect_policy())
        .timeout(std::time::Duration::from_secs(
            endpoint.timeout_seconds.max(5),
        ))
        .build()
        .map_err(|error| format!("HTTP client error: {error}"))?;

    let mut http_request = client
        .post(endpoint.base_url)
        .header("Accept", "application/json")
        .json(&payload);
    if let Some(header_value) = endpoint.auth_header {
        http_request = http_request.header("Authorization", header_value);
    }

    let response: CobaltResponse = http_request
        .send()
        .await
        .map_err(|error| format!("API request failed: {error}"))?
        .json()
        .await
        .map_err(|error| format!("API returned an unexpected response: {error}"))?;

    let media_url = if response.is_downloadable() {
        response.url.clone().unwrap_or_default()
    } else if let Some(first) = response.picker.first() {
        first.url.clone()
    } else {
        let code = response
            .error
            .map(|error| error.code)
            .unwrap_or_else(|| "api.unsupported".to_string());
        return Err(format!("API could not handle this link ({code})."));
    };
    if media_url.is_empty() {
        return Err("API returned an empty media URL.".to_string());
    }
    validate_remote_url(&media_url)?;

    stream_to_file(
        sink,
        id,
        endpoint.provider,
        &media_url,
        &request.output_dir,
        response.filename.clone(),
        cancel,
    )
    .await
}

/// The user-configured Cobalt-compatible endpoint (highest API priority).
async fn cobalt_download(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    settings: &AppSettings,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let api = &settings.api_provider;
    let base_url = api.base_url.trim().trim_end_matches('/').to_string();
    if !api.enabled || base_url.is_empty() {
        return Err("No API endpoint configured.".to_string());
    }

    let auth_header = (!api.token.trim().is_empty()).then(|| match api.auth_type.as_str() {
        "bearer" => format!("Bearer {}", api.token.trim()),
        _ => format!("Api-Key {}", api.token.trim()),
    });

    cobalt_fetch(
        sink,
        id,
        request,
        CobaltEndpoint {
            provider: ProviderKind::ConfiguredApi,
            base_url: &base_url,
            auth_header,
            timeout_seconds: api.timeout_seconds,
        },
        cancel,
    )
    .await
}

/// Fetches the live list of open community instances, falling back to a
/// bundled seed list. Cached for the lifetime of the process.
async fn public_instances() -> Vec<String> {
    static CACHE: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    if let Some(cached) = CACHE.get() {
        return cached.clone();
    }

    let fetched: Option<Vec<String>> = async {
        let client = reqwest::Client::builder()
            .user_agent("rsdownit (github.com/RlxChap2/rsdownit)")
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .ok()?;
        let directory: InstanceDirectory = client
            .get(INSTANCE_DIRECTORY_URL)
            .send()
            .await
            .ok()?
            .json()
            .await
            .ok()?;
        let ranked = rank_instances(&directory);
        (!ranked.is_empty()).then_some(ranked)
    }
    .await;

    let instances =
        fetched.unwrap_or_else(|| SEED_INSTANCES.iter().map(|url| url.to_string()).collect());
    CACHE.get_or_init(|| instances).clone()
}

/// Free community Cobalt servers, tried in ranked order without any auth.
async fn public_api_download(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let instances = public_instances().await;
    let mut last_error = "No community servers are reachable right now.".to_string();

    for base_url in instances.iter().take(4) {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".to_string());
        }
        let mut update = JobUpdate::new(id, "downloading");
        update.provider = Some(ProviderKind::PublicApi);
        update.detail = Some(format!(
            "Trying community server {}",
            base_url.trim_start_matches("https://")
        ));
        sink(update);

        match cobalt_fetch(
            sink,
            id,
            request,
            CobaltEndpoint {
                provider: ProviderKind::PublicApi,
                base_url,
                auth_header: None,
                timeout_seconds: 30,
            },
            cancel,
        )
        .await
        {
            Ok(path) => return Ok(path),
            Err(error) if error == "cancelled" => return Err(error),
            Err(error) => last_error = error,
        }
    }

    Err(last_error)
}

/// Fetches the page HTML and downloads the first direct media link found.
async fn html_probe_download(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    engine_tools: &EngineTools,
    settings: &AppSettings,
    cancel: &AtomicBool,
) -> Result<PathBuf, String> {
    let html = secure_get(
        &request.url,
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) rsdownit",
        std::time::Duration::from_secs(20),
    )
    .await
    .map_err(|error| format!("Could not fetch the page: {error}"))?
    .text()
    .await
    .map_err(|error| format!("Could not read the page: {error}"))?;

    let links = extract_media_links_from_html(&request.url, &html);
    let Some(media_url) = links.first() else {
        return Err("No media links found in the page.".to_string());
    };

    if is_stream_manifest_url(media_url) {
        let mut manifest_request = request.clone();
        manifest_request.url = media_url.clone();
        let cookie_browser = settings
            .cookies_from_browser
            .then_some(settings.cookie_browser.as_str());
        ytdlp_download(
            sink,
            id,
            &manifest_request,
            engine_tools,
            settings.concurrency,
            cookie_browser,
            cancel,
        )
        .await
    } else {
        stream_to_file(
            sink,
            id,
            ProviderKind::HtmlProbe,
            media_url,
            &request.output_dir,
            None,
            cancel,
        )
        .await
    }
}

fn has_audio_extension(url: &str) -> bool {
    let audio = [".mp3", ".m4a", ".opus", ".wav"];
    url::Url::parse(url)
        .map(|parsed| {
            let path = parsed.path().to_ascii_lowercase();
            audio.iter().any(|ext| path.ends_with(ext))
        })
        .unwrap_or(false)
}

fn provider_chain(request: &DownloadRequest, settings: &AppSettings) -> Vec<ProviderKind> {
    let wants_audio = request.mode == DownloadMode::Audio;
    // A direct file is only a valid fast path when it already matches the
    // requested mode; extracting audio from a raw file needs yt-dlp/FFmpeg.
    let direct_ok = is_direct_media_url(&request.url)
        && !is_stream_manifest_url(&request.url)
        && (!wants_audio || has_audio_extension(&request.url));

    let mut chain = Vec::new();
    if direct_ok {
        chain.push(ProviderKind::Direct);
    }
    chain.push(ProviderKind::YtDlp);
    let api = &settings.api_provider;
    if api.enabled && !api.base_url.trim().is_empty() {
        chain.push(ProviderKind::ConfiguredApi);
    }
    if settings.community_fallback {
        chain.push(ProviderKind::PublicApi);
    }
    if !wants_audio {
        chain.push(ProviderKind::HtmlProbe);
    }
    chain
}

/// Runs the provider chain for one job, reporting through `sink`.
pub async fn run_chain(
    sink: JobSink<'_>,
    id: &str,
    request: &DownloadRequest,
    settings: &AppSettings,
    engine_tools: &EngineTools,
    cancel: &AtomicBool,
) {
    let chain = provider_chain(request, settings);
    let mut failures: Vec<String> = Vec::new();

    for provider in chain {
        if cancel.load(Ordering::SeqCst) {
            sink(JobUpdate::new(id, "cancelled"));
            return;
        }

        let mut update = JobUpdate::new(id, "downloading");
        update.provider = Some(provider);
        update.progress = Some(0.0);
        sink(update);

        let result = match provider {
            ProviderKind::Direct => {
                stream_to_file(
                    sink,
                    id,
                    ProviderKind::Direct,
                    &request.url,
                    &request.output_dir,
                    None,
                    cancel,
                )
                .await
            }
            ProviderKind::ConfiguredApi => {
                cobalt_download(sink, id, request, settings, cancel).await
            }
            ProviderKind::PublicApi => public_api_download(sink, id, request, cancel).await,
            ProviderKind::YtDlp => {
                let cookie_browser = settings
                    .cookies_from_browser
                    .then_some(settings.cookie_browser.as_str());
                ytdlp_download(
                    sink,
                    id,
                    request,
                    engine_tools,
                    settings.concurrency,
                    cookie_browser,
                    cancel,
                )
                .await
            }
            ProviderKind::HtmlProbe => {
                html_probe_download(sink, id, request, engine_tools, settings, cancel).await
            }
        };

        match result {
            Ok(path) => {
                let mut update = JobUpdate::new(id, "complete");
                update.provider = Some(provider);
                update.progress = Some(100.0);
                update.file_path = Some(path.to_string_lossy().to_string());
                update.title = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string());
                sink(update);
                return;
            }
            Err(error) if error == "cancelled" => {
                sink(JobUpdate::new(id, "cancelled"));
                return;
            }
            Err(error) => {
                failures.push(format!("{provider:?}: {error}"));
                let mut update = JobUpdate::new(id, "probing");
                update.detail = Some("Trying the next provider".to_string());
                sink(update);
            }
        }
    }

    let mut update = JobUpdate::new(id, "failed");
    update.error = Some(failures.join(" • "));
    sink(update);
}

/// Tauri adapter: resolves tools, wires events, and runs the chain.
pub async fn run_job<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    request: DownloadRequest,
    settings: AppSettings,
    cancel: Arc<AtomicBool>,
) {
    let sink = |update: JobUpdate| {
        let _ = app.emit(DOWNLOAD_EVENT, update);
    };

    if cancel.load(Ordering::SeqCst) {
        sink(JobUpdate::new(&id, "cancelled"));
        return;
    }

    if let Err(error) = validate_remote_target(&request.url).await {
        let mut update = JobUpdate::new(&id, "failed");
        update.error = Some(error);
        sink(update);
        return;
    }

    let mut update = JobUpdate::new(&id, "probing");
    update.detail = Some("Resolving the best way to download".to_string());
    sink(update);

    // Make sure the engine tools exist before the first real download.
    if !tools::tools_report(&app).ready {
        let mut update = JobUpdate::new(&id, "probing");
        update.detail = Some("Setting up the download engine".to_string());
        sink(update);
        if let Err(error) = tools::ensure_tools(&app).await {
            let mut update = JobUpdate::new(&id, "failed");
            update.error = Some(format!("Could not set up the download engine: {error}"));
            sink(update);
            return;
        }
    }

    let yt_dlp = tools::locate_tool(&app, "yt-dlp");
    let ffmpeg = tools::locate_tool(&app, "ffmpeg");
    let engine_tools = EngineTools {
        yt_dlp_path: yt_dlp.path,
        ffmpeg_dir: match (ffmpeg.managed, ffmpeg.path) {
            (true, Some(path)) => Path::new(&path)
                .parent()
                .map(|dir| dir.to_string_lossy().to_string()),
            _ => None,
        },
    };

    run_chain(&sink, &id, &request, &settings, &engine_tools, &cancel).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ytdlp_progress_lines() {
        let parsed = parse_progress_line("  12.3%|2.35MiB/s|00:12");
        let (percent, speed, eta) = parsed.expect("parses");
        assert!((percent - 12.3).abs() < f64::EPSILON);
        assert_eq!(speed, "2.35MiB/s");
        assert_eq!(eta, "00:12");
        assert!(parse_progress_line("[download] Destination: x").is_none());
    }

    #[test]
    fn derives_filenames_from_urls() {
        assert_eq!(
            filename_from_url("https://cdn.example.com/media/My%20Clip.mp4?sig=1"),
            "My Clip.mp4"
        );
        assert_eq!(filename_from_url("https://example.com/"), "download.bin");
    }

    #[test]
    fn formats_speed_and_eta() {
        assert_eq!(format_bytes_per_sec(2_097_152.0), "2.0 MB/s");
        assert_eq!(format_eta(75.0), "1m 15s");
        assert_eq!(format_eta(f64::NAN), "--");
    }

    #[test]
    fn skips_direct_provider_when_audio_wanted_from_video_file() {
        let settings = AppSettings::default();
        let mut request = DownloadRequest {
            url: "https://cdn.example.com/clip.mp4".to_string(),
            output_dir: "C:/Downloads".to_string(),
            mode: DownloadMode::Audio,
            video_quality: crate::models::VideoQuality::Best,
            audio_format: crate::models::AudioFormat::Mp3,
            audio_bitrate: "best".to_string(),
        };
        assert_eq!(
            provider_chain(&request, &settings),
            vec![ProviderKind::YtDlp]
        );

        request.mode = DownloadMode::Video;
        assert_eq!(
            provider_chain(&request, &settings),
            vec![
                ProviderKind::Direct,
                ProviderKind::YtDlp,
                ProviderKind::HtmlProbe
            ]
        );

        request.url = "https://cdn.example.com/master.m3u8".to_string();
        assert_eq!(
            provider_chain(&request, &settings),
            vec![ProviderKind::YtDlp, ProviderKind::HtmlProbe]
        );

        let with_fallback = AppSettings {
            community_fallback: true,
            ..AppSettings::default()
        };
        assert!(provider_chain(&request, &with_fallback).contains(&ProviderKind::PublicApi));
    }
}
