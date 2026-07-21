import type { CSSProperties } from "react";

type ProgressProps = {
  value: number;
  label?: string;
};

export function Progress({ value, label }: ProgressProps) {
  const boundedValue = Math.min(100, Math.max(0, value));

  return (
    <div
      className="ui-progress"
      role="progressbar"
      aria-label={label}
      aria-valuenow={boundedValue}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div
        className="ui-progress-fill"
        style={{ "--progress": boundedValue / 100 } as CSSProperties}
      />
    </div>
  );
}
