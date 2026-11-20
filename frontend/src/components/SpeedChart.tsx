import { useId } from "react";
import { formatSpeed } from "@/lib/utils";

export interface SpeedSample {
  at: number;
  download: number;
  upload: number;
}

export function SpeedChart({ samples }: { samples: SpeedSample[] }) {
  const id = useId().replace(/:/g, "");
  const width = 720;
  const height = 170;
  const padX = 8;
  const padY = 14;
  const data = samples.slice(-120);
  const max = Math.max(1, ...data.flatMap((s) => [s.download, s.upload]));
  const x = (index: number) =>
    data.length <= 1
      ? padX
      : padX + (index / (data.length - 1)) * (width - padX * 2);
  const y = (value: number) =>
    height - padY - (Math.max(0, value) / max) * (height - padY * 2);

  const line = (key: "download" | "upload") =>
    data.map((sample, index) => `${x(index)},${y(sample[key])}`).join(" ");
  const area = (key: "download" | "upload") => {
    if (!data.length) return "";
    return `${padX},${height - padY} ${line(key)} ${x(data.length - 1)},${height - padY}`;
  };

  return (
    <div className="w-full">
      <div className="mb-2 flex items-center justify-between text-[11px] text-slate-500">
        <div className="flex items-center gap-4">
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-3 rounded-full bg-cyan-400" /> Download
          </span>
          <span className="inline-flex items-center gap-1.5">
            <span className="h-1.5 w-3 rounded-full bg-emerald-400" /> Upload
          </span>
        </div>
        <span className="font-mono">peak {formatSpeed(max)}</span>
      </div>
      <div className="relative overflow-hidden rounded-xl border border-white/[0.06] bg-black/25">
        {!data.length ? (
          <div className="grid h-[170px] place-items-center text-xs text-slate-600">
            Waiting for transfer samples…
          </div>
        ) : (
          <svg viewBox={`0 0 ${width} ${height}`} className="h-[170px] w-full" preserveAspectRatio="none">
            <defs>
              <linearGradient id={`down-${id}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#22d3ee" stopOpacity="0.24" />
                <stop offset="100%" stopColor="#22d3ee" stopOpacity="0" />
              </linearGradient>
              <linearGradient id={`up-${id}`} x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stopColor="#34d399" stopOpacity="0.2" />
                <stop offset="100%" stopColor="#34d399" stopOpacity="0" />
              </linearGradient>
            </defs>
            {[0.25, 0.5, 0.75].map((ratio) => (
              <line
                key={ratio}
                x1={padX}
                x2={width - padX}
                y1={padY + ratio * (height - padY * 2)}
                y2={padY + ratio * (height - padY * 2)}
                stroke="rgba(148,163,184,0.08)"
                strokeWidth="1"
              />
            ))}
            <polygon points={area("download")} fill={`url(#down-${id})`} />
            <polygon points={area("upload")} fill={`url(#up-${id})`} />
            <polyline
              points={line("download")}
              fill="none"
              stroke="#22d3ee"
              strokeWidth="2"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            <polyline
              points={line("upload")}
              fill="none"
              stroke="#34d399"
              strokeWidth="2"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
          </svg>
        )}
      </div>
    </div>
  );
}
