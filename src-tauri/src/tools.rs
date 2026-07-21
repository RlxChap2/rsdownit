//! Finds and provisions yt-dlp and FFmpeg with publisher checksum verification.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, Runtime};

pub const SETUP_EVENT: &str = "setup://progress";

const YTDLP_WINDOWS_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";
const YTDLP_MACOS_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_macos";
const YTDLP_LINUX_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux";
const YTDLP_CHECKSUMS_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS";
// ffmpeg.org's recommended Windows builds; "essentials" carries everything
// yt-dlp needs for merging and audio conversion.
const FFMPEG_WINDOWS_ZIP_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
const FFMPEG_WINDOWS_SHA256_URL: &str =
    "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip.sha256";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub path: Option<String>,
    pub managed: bool,
    pub version: Option<String>,
    pub verified: bool,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsReport {
    pub yt_dlp: ToolStatus,
    pub ffmpeg: ToolStatus,
    pub ready: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdateStatus {
    pub name: String,
    pub managed: bool,
    pub update_available: Option<bool>,
    pub current_version: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUpdatesReport {
    pub yt_dlp: ToolUpdateStatus,
    pub ffmpeg: ToolUpdateStatus,
    pub updates_available: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupProgress {
    pub tool: String,
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

/// Progress callback for provisioning; wired to `setup://progress` in the app.
pub type SetupSink<'a> = &'a (dyn Fn(SetupProgress) + Send + Sync);

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn probe_version(command: &str, args: &[&str]) -> Option<String> {
    let mut cmd = std::process::Command::new(command);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
}

fn checksum_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tool");
    path.with_file_name(format!("{file_name}.sha256"))
}

fn ffmpeg_release_checksum_path(tools_dir: &Path) -> PathBuf {
    tools_dir.join("ffmpeg-release.sha256")
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    use std::fmt::Write as _;

    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("Could not read downloaded tool: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = std::io::Read::read(&mut file, &mut buffer)
            .map_err(|error| format!("Could not verify downloaded tool: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn managed_integrity(path: &Path) -> Option<String> {
    let expected = std::fs::read_to_string(checksum_path(path)).ok()?;
    let expected = expected.split_whitespace().next()?.to_ascii_lowercase();
    let actual = sha256_file(path).ok()?;
    (actual == expected).then_some(actual)
}

fn write_local_checksum(path: &Path) -> Result<String, String> {
    let digest = sha256_file(path)?;
    std::fs::write(checksum_path(path), format!("{digest}\n"))
        .map_err(|error| format!("Could not store tool checksum: {error}"))?;
    Ok(digest)
}

/// Finds a tool, preferring PATH, then the managed copy in `tools_dir`.
pub fn locate_tool_in(tools_dir: Option<&Path>, base_name: &str) -> ToolStatus {
    let version_args: &[&str] = if base_name == "ffmpeg" {
        &["-version"]
    } else {
        &["--version"]
    };

    if let Some(version) = probe_version(base_name, version_args) {
        return ToolStatus {
            name: base_name.to_string(),
            available: true,
            path: Some(base_name.to_string()),
            managed: false,
            version: Some(version),
            verified: false,
            sha256: None,
        };
    }

    if let Some(dir) = tools_dir {
        let managed = dir.join(exe_name(base_name));
        if managed.exists() {
            let checksum = managed_integrity(&managed);
            if checksum.is_none() {
                return ToolStatus {
                    name: base_name.to_string(),
                    available: false,
                    path: None,
                    managed: true,
                    version: None,
                    verified: false,
                    sha256: None,
                };
            }
            let version = probe_version(&managed.to_string_lossy(), version_args);
            return ToolStatus {
                name: base_name.to_string(),
                available: true,
                path: Some(managed.to_string_lossy().to_string()),
                managed: true,
                version,
                verified: true,
                sha256: checksum,
            };
        }
    }

    ToolStatus {
        name: base_name.to_string(),
        available: false,
        path: None,
        managed: false,
        version: None,
        verified: false,
        sha256: None,
    }
}

async fn fetch_text(url: &str, label: &str) -> Result<String, String> {
    reqwest::Client::builder()
        .user_agent("rsdownit")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| format!("HTTP client error: {error}"))?
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Could not download {label}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download of {label} failed: {error}"))?
        .text()
        .await
        .map_err(|error| format!("Could not read {label}: {error}"))
}

fn checksum_for_asset(checksums: &str, asset_name: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let digest = fields.next()?;
        let name = fields.next()?.trim_start_matches('*');
        (name == asset_name && digest.len() == 64).then(|| digest.to_ascii_lowercase())
    })
}

fn report(
    sink: SetupSink<'_>,
    tool: &str,
    phase: &str,
    downloaded: u64,
    total: Option<u64>,
    message: &str,
) {
    sink(SetupProgress {
        tool: tool.to_string(),
        phase: phase.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total,
        message: message.to_string(),
    });
}

async fn download_file(
    sink: SetupSink<'_>,
    tool: &str,
    url: &str,
    destination: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("rsdownit")
        .build()
        .map_err(|error| format!("HTTP client error: {error}"))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| format!("Could not download {tool}: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download of {tool} failed: {error}"))?;

    let total = response.content_length();
    let temp_path = destination.with_extension("part");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .map_err(|error| format!("Could not write {tool}: {error}"))?;

    let mut downloaded: u64 = 0;
    let mut hasher = Sha256::new();
    let mut last_emit = std::time::Instant::now();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("Download of {tool} interrupted: {error}"))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|error| format!("Could not write {tool}: {error}"))?;
        downloaded += chunk.len() as u64;
        hasher.update(&chunk);
        if last_emit.elapsed().as_millis() > 250 {
            report(sink, tool, "downloading", downloaded, total, "Downloading");
            last_emit = std::time::Instant::now();
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|error| format!("Could not finish {tool}: {error}"))?;
    drop(file);

    report(
        sink,
        tool,
        "verifying",
        downloaded,
        total,
        "Verifying SHA-256",
    );
    let actual_sha256 = hex_digest(hasher.finalize());
    if actual_sha256 != expected_sha256.to_ascii_lowercase() {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!(
            "{tool} checksum did not match its publisher. The file was deleted."
        ));
    }

    if destination.exists() {
        tokio::fs::remove_file(destination)
            .await
            .map_err(|error| format!("Could not replace old {tool}: {error}"))?;
    }

    tokio::fs::rename(&temp_path, destination)
        .await
        .map_err(|error| format!("Could not finalize {tool}: {error}"))?;
    tokio::fs::write(checksum_path(destination), format!("{actual_sha256}\n"))
        .await
        .map_err(|error| format!("Could not store {tool} checksum: {error}"))?;
    report(
        sink,
        tool,
        "downloaded",
        downloaded,
        total,
        "Download complete",
    );
    Ok(())
}

async fn ensure_yt_dlp(
    tools_dir: &Path,
    sink: SetupSink<'_>,
    refresh_managed: bool,
) -> Result<ToolStatus, String> {
    let existing = locate_tool_in(Some(tools_dir), "yt-dlp");
    if existing.available && (!refresh_managed || !existing.managed) {
        return Ok(existing);
    }

    let (url, asset_name) = if cfg!(windows) {
        (YTDLP_WINDOWS_URL, "yt-dlp.exe")
    } else if cfg!(target_os = "macos") {
        (YTDLP_MACOS_URL, "yt-dlp_macos")
    } else {
        (YTDLP_LINUX_URL, "yt-dlp_linux")
    };

    report(
        sink,
        "yt-dlp",
        "verifying",
        0,
        None,
        "Fetching publisher checksum",
    );
    let checksums = fetch_text(YTDLP_CHECKSUMS_URL, "yt-dlp checksums").await?;
    let expected = checksum_for_asset(&checksums, asset_name).ok_or_else(|| {
        "The yt-dlp release did not publish a checksum for this platform.".to_string()
    })?;

    let destination = tools_dir.join(exe_name("yt-dlp"));
    download_file(sink, "yt-dlp", url, &destination, &expected).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755));
    }

    report(sink, "yt-dlp", "ready", 0, None, "yt-dlp is ready");
    Ok(locate_tool_in(Some(tools_dir), "yt-dlp"))
}

fn extract_ffmpeg_from_zip(zip_path: &Path, target_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path)
        .map_err(|error| format!("Could not open FFmpeg archive: {error}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("Could not read FFmpeg archive: {error}"))?;

    let wanted = ["ffmpeg.exe", "ffprobe.exe"];
    let mut extracted = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("Bad FFmpeg archive entry: {error}"))?;
        let name = entry.name().replace('\\', "/");
        let Some(file_name) = name.rsplit('/').next() else {
            continue;
        };
        if !name.contains("/bin/") || !wanted.contains(&file_name) {
            continue;
        }
        if entry.size() > 300 * 1024 * 1024 {
            return Err(format!(
                "FFmpeg archive entry {file_name} is unexpectedly large."
            ));
        }
        let destination = target_dir.join(file_name);
        let mut output = std::fs::File::create(&destination)
            .map_err(|error| format!("Could not write {file_name}: {error}"))?;
        std::io::copy(&mut entry, &mut output)
            .map_err(|error| format!("Could not extract {file_name}: {error}"))?;
        extracted += 1;
    }

    if extracted != wanted.len() {
        return Err("FFmpeg archive did not contain expected binaries.".to_string());
    }
    for file_name in wanted {
        let path = target_dir.join(file_name);
        if path.exists() {
            write_local_checksum(&path)?;
        }
    }
    Ok(())
}

async fn ensure_ffmpeg(
    tools_dir: &Path,
    sink: SetupSink<'_>,
    refresh_managed: bool,
) -> Result<ToolStatus, String> {
    let existing = locate_tool_in(Some(tools_dir), "ffmpeg");
    if existing.available && (!refresh_managed || !existing.managed) {
        return Ok(existing);
    }

    if !cfg!(windows) {
        // Cross-platform managed FFmpeg lands with the packaging work; on
        // macOS/Linux we rely on the system package manager for now.
        return Ok(existing);
    }

    let zip_path = tools_dir.join("ffmpeg-download.zip");
    report(
        sink,
        "ffmpeg",
        "verifying",
        0,
        None,
        "Fetching publisher checksum",
    );
    let checksum_text = fetch_text(FFMPEG_WINDOWS_SHA256_URL, "FFmpeg checksum").await?;
    let expected = checksum_text
        .split_whitespace()
        .find(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
        .ok_or_else(|| "FFmpeg publisher checksum was invalid.".to_string())?;
    download_file(sink, "ffmpeg", FFMPEG_WINDOWS_ZIP_URL, &zip_path, expected).await?;

    report(sink, "ffmpeg", "extracting", 0, None, "Unpacking FFmpeg");
    let dir_clone = tools_dir.to_path_buf();
    let zip_clone = zip_path.clone();
    tokio::task::spawn_blocking(move || extract_ffmpeg_from_zip(&zip_clone, &dir_clone))
        .await
        .map_err(|error| format!("FFmpeg extraction crashed: {error}"))??;
    let _ = tokio::fs::remove_file(&zip_path).await;
    let _ = tokio::fs::remove_file(checksum_path(&zip_path)).await;
    tokio::fs::write(
        ffmpeg_release_checksum_path(tools_dir),
        format!("{}\n", expected.to_ascii_lowercase()),
    )
    .await
    .map_err(|error| format!("Could not store FFmpeg release checksum: {error}"))?;

    report(sink, "ffmpeg", "ready", 0, None, "FFmpeg is ready");
    Ok(locate_tool_in(Some(tools_dir), "ffmpeg"))
}

/// Makes sure every tool is present, downloading missing ones into
/// `tools_dir`. Safe to call repeatedly; only missing tools trigger downloads.
pub async fn ensure_tools_in(tools_dir: &Path, sink: SetupSink<'_>) -> Result<ToolsReport, String> {
    std::fs::create_dir_all(tools_dir)
        .map_err(|error| format!("Could not create tools dir: {error}"))?;
    let yt_dlp = ensure_yt_dlp(tools_dir, sink, false).await?;
    let ffmpeg = ensure_ffmpeg(tools_dir, sink, false)
        .await
        .unwrap_or_else(|error| {
            // FFmpeg failing to provision must not block plain downloads.
            report(sink, "ffmpeg", "error", 0, None, &error);
            locate_tool_in(Some(tools_dir), "ffmpeg")
        });
    let ready = yt_dlp.available;
    Ok(ToolsReport {
        yt_dlp,
        ffmpeg,
        ready,
    })
}

pub async fn refresh_tools_in(
    tools_dir: &Path,
    sink: SetupSink<'_>,
) -> Result<ToolsReport, String> {
    std::fs::create_dir_all(tools_dir)
        .map_err(|error| format!("Could not create tools dir: {error}"))?;
    let yt_dlp = ensure_yt_dlp(tools_dir, sink, true).await?;
    let ffmpeg = ensure_ffmpeg(tools_dir, sink, true)
        .await
        .unwrap_or_else(|error| {
            report(sink, "ffmpeg", "error", 0, None, &error);
            locate_tool_in(Some(tools_dir), "ffmpeg")
        });
    Ok(ToolsReport {
        ready: yt_dlp.available,
        yt_dlp,
        ffmpeg,
    })
}

pub async fn check_tool_updates_in(tools_dir: &Path) -> Result<ToolUpdatesReport, String> {
    let yt_dlp = locate_tool_in(Some(tools_dir), "yt-dlp");
    let ffmpeg = locate_tool_in(Some(tools_dir), "ffmpeg");

    let yt_update = if yt_dlp.managed && yt_dlp.available {
        let (_, asset_name) = if cfg!(windows) {
            (YTDLP_WINDOWS_URL, "yt-dlp.exe")
        } else if cfg!(target_os = "macos") {
            (YTDLP_MACOS_URL, "yt-dlp_macos")
        } else {
            (YTDLP_LINUX_URL, "yt-dlp_linux")
        };
        let checksums = fetch_text(YTDLP_CHECKSUMS_URL, "yt-dlp checksums").await?;
        let latest = checksum_for_asset(&checksums, asset_name).ok_or_else(|| {
            "The yt-dlp release did not publish a checksum for this platform.".to_string()
        })?;
        yt_dlp.sha256.as_ref().map(|current| current != &latest)
    } else {
        None
    };

    let ffmpeg_update = if cfg!(windows) && ffmpeg.managed && ffmpeg.available {
        let latest = fetch_text(FFMPEG_WINDOWS_SHA256_URL, "FFmpeg checksum").await?;
        let latest = latest
            .split_whitespace()
            .find(|value| value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()))
            .ok_or_else(|| "FFmpeg publisher checksum was invalid.".to_string())?;
        std::fs::read_to_string(ffmpeg_release_checksum_path(tools_dir))
            .ok()
            .and_then(|value| value.split_whitespace().next().map(str::to_owned))
            .map(|current| !current.eq_ignore_ascii_case(latest))
    } else {
        None
    };

    Ok(ToolUpdatesReport {
        updates_available: yt_update == Some(true) || ffmpeg_update == Some(true),
        yt_dlp: ToolUpdateStatus {
            name: "yt-dlp".to_string(),
            managed: yt_dlp.managed,
            update_available: yt_update,
            current_version: yt_dlp.version,
        },
        ffmpeg: ToolUpdateStatus {
            name: "FFmpeg".to_string(),
            managed: ffmpeg.managed,
            update_available: ffmpeg_update,
            current_version: ffmpeg.version,
        },
    })
}

// ---------------------------------------------------------------------------
// Tauri adapters
// ---------------------------------------------------------------------------

pub fn tools_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve app data dir: {error}"))?
        .join("bin");
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("Could not create tools dir: {error}"))?;
    Ok(dir)
}

pub fn locate_tool<R: Runtime>(app: &AppHandle<R>, base_name: &str) -> ToolStatus {
    locate_tool_in(tools_dir(app).ok().as_deref(), base_name)
}

pub fn tools_report<R: Runtime>(app: &AppHandle<R>) -> ToolsReport {
    let yt_dlp = locate_tool(app, "yt-dlp");
    let ffmpeg = locate_tool(app, "ffmpeg");
    let ready = yt_dlp.available;
    ToolsReport {
        yt_dlp,
        ffmpeg,
        ready,
    }
}

pub async fn ensure_tools<R: Runtime>(app: &AppHandle<R>) -> Result<ToolsReport, String> {
    let dir = tools_dir(app)?;
    let sink = |progress: SetupProgress| {
        let _ = app.emit(SETUP_EVENT, progress);
    };
    ensure_tools_in(&dir, &sink).await
}

pub async fn refresh_tools<R: Runtime>(app: &AppHandle<R>) -> Result<ToolsReport, String> {
    let dir = tools_dir(app)?;
    let sink = |progress: SetupProgress| {
        let _ = app.emit(SETUP_EVENT, progress);
    };
    refresh_tools_in(&dir, &sink).await
}

pub async fn check_tool_updates<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<ToolUpdatesReport, String> {
    check_tool_updates_in(&tools_dir(app)?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_digest_bytes_as_lowercase_hex() {
        assert_eq!(hex_digest([0x00, 0x01, 0xab, 0xff]), "0001abff");
    }

    #[test]
    fn selects_only_the_exact_release_asset_checksum() {
        let checksums = concat!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  yt-dlp\n",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb *yt-dlp.exe\n",
        );
        assert_eq!(
            checksum_for_asset(checksums, "yt-dlp.exe").as_deref(),
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert!(checksum_for_asset(checksums, "missing.exe").is_none());
    }
}
