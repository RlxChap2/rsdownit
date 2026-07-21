import type { CSSProperties } from "react";
import { Loader2 } from "lucide-react";

import type { SetupProgress } from "../../lib/types";

type SetupBannerProps = {
  progress: SetupProgress;
};

function formatMegabytes(bytes: number) {
  return `${(bytes / 1_048_576).toFixed(1)} MB`;
}

export function SetupStatus({ progress }: SetupBannerProps) {
  const { tool, phase, downloadedBytes, totalBytes } = progress;
  const percent =
    totalBytes && totalBytes > 0 ? Math.round((downloadedBytes / totalBytes) * 100) : null;

  const label =
    phase === "verifying"
      ? `Verifying ${tool} with SHA-256…`
      : phase === "extracting"
        ? `Unpacking ${tool}…`
        : percent !== null
          ? `Downloading ${tool} · ${percent}% · ${formatMegabytes(downloadedBytes)} of ${formatMegabytes(totalBytes!)}`
          : `Downloading ${tool} · ${formatMegabytes(downloadedBytes)}`;

  return (
    <div className="setup-banner" role="status">
      <Loader2 className="spin" aria-hidden="true" />
      <div className="setup-copy">
        <strong>Preparing the local engine</strong>
        <span>{label}</span>
      </div>
      {percent !== null && (
        <div className="setup-progress">
          <div
            className="setup-progress-fill"
            style={{ "--progress": percent / 100 } as CSSProperties}
          />
        </div>
      )}
    </div>
  );
}
