import { cn } from "@/lib/utils";

export function Logo({
  className,
  compact = false,
}: {
  className?: string;
  compact?: boolean;
}) {
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <div className="relative grid h-10 w-10 place-items-center overflow-hidden rounded-xl border border-cyan-200 bg-gradient-to-br from-cyan-100 via-sky-50 to-indigo-100 shadow-glow">
        <svg viewBox="0 0 32 32" className="h-6 w-6" aria-hidden="true">
          <defs>
            <linearGradient id="logo-gradient" x1="4" y1="4" x2="28" y2="28">
              <stop offset="0" stopColor="#0891b2" />
              <stop offset="0.55" stopColor="#0284c7" />
              <stop offset="1" stopColor="#4f46e5" />
            </linearGradient>
          </defs>
          <path
            d="M8 20.5a5 5 0 0 1 .6-9.96A7 7 0 0 1 22.6 9.8a4.5 4.5 0 0 1 1.9 8.6"
            fill="none"
            stroke="url(#logo-gradient)"
            strokeWidth="2.2"
            strokeLinecap="round"
          />
          <path
            d="M16 13v9m0 0-3.6-3.6M16 22l3.6-3.6"
            fill="none"
            stroke="#0e7490"
            strokeWidth="2.2"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <circle cx="24.5" cy="23.5" r="2.2" fill="#10b981" />
        </svg>
      </div>
      {!compact ? (
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-[15px] font-semibold tracking-tight text-slate-900">
              File<span className="gradient-text">Uploader</span>
            </span>
            <span className="rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-widest text-slate-600">
              自托管
            </span>
          </div>
          <div className="whitespace-nowrap text-[9px] uppercase tracking-[0.14em] text-slate-500">
           流式 · 分片 · S3
          </div>
        </div>
      ) : null}
    </div>
  );
}
