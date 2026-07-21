import {
  ArrowDownToLine,
  ClipboardPaste,
  FolderOpen,
  Link2,
  Music,
  Video,
  VolumeX,
  X,
} from "lucide-react";
import { useId, type ReactNode } from "react";

import {
  AUDIO_BITRATES,
  AUDIO_FORMATS,
  VIDEO_QUALITIES,
} from "../../lib/download-options";
import type {
  AudioBitrate,
  AudioFormat,
  DownloadMode,
  VideoQuality,
} from "../../lib/types";
import { isProbablyUrl } from "../../lib/utils";
import { Select } from "../../components/ui/select";

type DownloadComposerProps = {
  url: string;
  mode: DownloadMode;
  videoQuality: VideoQuality;
  audioFormat: AudioFormat;
  audioBitrate: AudioBitrate;
  outputFolder: string;
  showAdvanced: boolean;
  activity: ReactNode;
  onUrlChange: (url: string) => void;
  onModeChange: (mode: DownloadMode) => void;
  onVideoQualityChange: (quality: VideoQuality) => void;
  onAudioFormatChange: (format: AudioFormat) => void;
  onAudioBitrateChange: (bitrate: AudioBitrate) => void;
  onPaste: () => void;
  onChooseFolder: () => void;
  onDownload: () => void;
};

const MODES = [
  { value: "video", label: "Video", description: "Picture and sound", icon: Video },
  { value: "audio", label: "Audio", description: "Sound only", icon: Music },
  { value: "muted-video", label: "Muted", description: "Picture only", icon: VolumeX },
] satisfies Array<{
  value: DownloadMode;
  label: string;
  description: string;
  icon: typeof Video;
}>;

export function DownloadComposer({
  url,
  mode,
  videoQuality,
  audioFormat,
  audioBitrate,
  outputFolder,
  showAdvanced,
  activity,
  onUrlChange,
  onModeChange,
  onVideoQualityChange,
  onAudioFormatChange,
  onAudioBitrateChange,
  onPaste,
  onChooseFolder,
  onDownload,
}: DownloadComposerProps) {
  const urlId = useId();
  const qualityId = useId();
  const formatId = useId();
  const bitrateId = useId();
  const validUrl = isProbablyUrl(url);
  const ready = validUrl && Boolean(outputFolder.trim());
  const invalid = url.length > 0 && !validUrl;

  return (
    <section className="download-stage" aria-labelledby="download-title">
      <header className="hero-copy">
        <p className="eyebrow">Local-first media downloader</p>
        <h2 id="download-title">
          Download from <span>almost anywhere.</span>
        </h2>
        <p>
          Paste a public media link. rsdownit finds the best available video or audio
          without sending usage data anywhere.
        </p>
      </header>

      <form
        className="download-composer"
        onSubmit={(event) => {
          event.preventDefault();
          if (ready) onDownload();
        }}
      >
        <div className={invalid ? "url-control is-error" : "url-control"}>
          <Link2 aria-hidden="true" />
          <label className="sr-only" htmlFor={urlId}>Media link</label>
          <input
            id={urlId}
            value={url}
            onChange={(event) => onUrlChange(event.currentTarget.value)}
            placeholder="Paste a video or audio link"
            spellCheck={false}
            autoComplete="url"
            inputMode="url"
            aria-invalid={invalid}
            aria-describedby="url-help"
          />
          {url && (
            <button
              type="button"
              className="icon-button quiet clear-link"
              aria-label="Clear link"
              onClick={() => onUrlChange("")}
            >
              <X aria-hidden="true" />
            </button>
          )}
          <button type="button" className="paste-button" onClick={onPaste}>
            <ClipboardPaste aria-hidden="true" />
            <span>Paste</span>
          </button>
        </div>

        <p className={invalid ? "field-help error" : "field-help"} id="url-help">
          {invalid
            ? "Use a complete http or https link."
            : "Public links only. DRM and paywall bypass are not supported."}
        </p>

        <div className="download-workbench">
          <div className="download-options">
            <fieldset className="mode-fieldset">
              <legend>Save as</legend>
              <div className="mode-switcher" role="group" aria-label="Download mode">
                {MODES.map((option) => {
                  const selected = mode === option.value;
                  return (
                    <button
                      key={option.value}
                      type="button"
                      className={selected ? "mode-button selected" : "mode-button"}
                      aria-pressed={selected}
                      onClick={() => onModeChange(option.value)}
                    >
                      <option.icon aria-hidden="true" />
                      <span>
                        <strong>{option.label}</strong>
                        <small>{option.description}</small>
                      </span>
                    </button>
                  );
                })}
              </div>
            </fieldset>

            <div className="format-row">
              {mode === "video" && (
                <label className="field-group compact" htmlFor={qualityId}>
                  <span>Video quality</span>
                  <Select
                    id={qualityId}
                    value={videoQuality}
                    onChange={(event) =>
                      onVideoQualityChange(event.currentTarget.value as VideoQuality)
                    }
                  >
                    {VIDEO_QUALITIES.map((quality) => (
                      <option key={quality.value} value={quality.value}>{quality.label}</option>
                    ))}
                  </Select>
                </label>
              )}

              {mode === "audio" && (
                <>
                  <label className="field-group compact" htmlFor={formatId}>
                    <span>Audio format</span>
                    <Select
                      id={formatId}
                      value={audioFormat}
                      onChange={(event) =>
                        onAudioFormatChange(event.currentTarget.value as AudioFormat)
                      }
                    >
                      {AUDIO_FORMATS.map((format) => (
                        <option key={format.value} value={format.value}>{format.label}</option>
                      ))}
                    </Select>
                  </label>
                  {showAdvanced && (
                    <label className="field-group compact" htmlFor={bitrateId}>
                      <span>Bitrate</span>
                      <Select
                        id={bitrateId}
                        value={audioBitrate}
                        onChange={(event) =>
                          onAudioBitrateChange(event.currentTarget.value as AudioBitrate)
                        }
                      >
                        {AUDIO_BITRATES.map((bitrate) => (
                          <option key={bitrate.value} value={bitrate.value}>{bitrate.label}</option>
                        ))}
                      </Select>
                    </label>
                  )}
                </>
              )}

              {mode === "muted-video" && (
                <p className="mode-note">Best video stream without an audio track.</p>
              )}
            </div>

            <div className="destination-row">
              <div>
                <span>Save to</span>
                <strong title={outputFolder}>{outputFolder || "Downloads"}</strong>
              </div>
              <button type="button" className="secondary-button" onClick={onChooseFolder}>
                <FolderOpen aria-hidden="true" />
                <span>Choose folder</span>
              </button>
            </div>

            <button type="submit" className="primary-button download-button" disabled={!ready}>
              <ArrowDownToLine aria-hidden="true" />
              {mode === "audio" ? "Download audio" : "Download video"}
            </button>
          </div>

          {activity}
        </div>
      </form>
    </section>
  );
}
