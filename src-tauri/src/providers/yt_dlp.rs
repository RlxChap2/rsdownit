use crate::models::{AudioFormat, DownloadMode, VideoQuality};

pub fn requires_ffmpeg_for_audio(format: AudioFormat) -> bool {
    matches!(format, AudioFormat::Mp3 | AudioFormat::Wav)
}

pub fn build_ytdlp_args(
    url: &str,
    mode: DownloadMode,
    video_quality: VideoQuality,
    audio_format: AudioFormat,
    audio_bitrate: &str,
    output_template: &str,
    concurrent_fragments: u8,
) -> Vec<String> {
    let mut args = vec![
        "--no-playlist".to_string(),
        "--continue".to_string(),
        "--no-overwrites".to_string(),
        "--retries".to_string(),
        "10".to_string(),
        "--fragment-retries".to_string(),
        "10".to_string(),
        "--socket-timeout".to_string(),
        "20".to_string(),
        "--concurrent-fragments".to_string(),
        concurrent_fragments.clamp(1, 8).to_string(),
        "--newline".to_string(),
        "--progress-template".to_string(),
        "download:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s"
            .to_string(),
        "-o".to_string(),
        output_template.to_string(),
    ];

    match mode {
        DownloadMode::Video => {
            args.push("-f".to_string());
            args.push(match video_quality.max_height() {
                Some(height) => {
                    format!("bestvideo*[height<={height}]+bestaudio/best[height<={height}]/best")
                }
                None => "bestvideo*+bestaudio/best".to_string(),
            });
        }
        DownloadMode::Audio => {
            args.push("--extract-audio".to_string());
            if let Some(format) = audio_format.as_ytdlp_value() {
                args.push("--audio-format".to_string());
                args.push(format.to_string());
            }
            if audio_bitrate != "best" {
                args.push("--audio-quality".to_string());
                args.push(format!("{audio_bitrate}K"));
            }
        }
        DownloadMode::MutedVideo => {
            args.push("-f".to_string());
            args.push("bestvideo*".to_string());
        }
    }

    args.push(url.to_string());
    args
}
