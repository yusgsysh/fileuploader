import * as React from "react";
import { cn } from "@/lib/utils";

export function Badge({
  className,
  tone = "default",
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & {
  tone?: "default" | "cyan" | "green" | "red" | "amber" | "violet";
}) {
  const tones = {
    default: "border-white/10 bg-white/[0.04] text-slate-300",
    cyan: "border-cyan-400/25 bg-cyan-400/10 text-cyan-200",
    green: "border-emerald-400/25 bg-emerald-400/10 text-emerald-200",
    red: "border-rose-400/25 bg-rose-400/10 text-rose-200",
    amber: "border-amber-400/25 bg-amber-400/10 text-amber-200",
    violet: "border-violet-400/25 bg-violet-400/10 text-violet-200",
  } as const;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-[11px] font-medium tracking-wide",
        tones[tone],
        className,
      )}
      {...props}
    />
  );
}

export function Progress({
  value,
  className,
  barClassName,
  indeterminate = false,
}: {
  value?: number;
  className?: string;
  barClassName?: string;
  indeterminate?: boolean;
}) {
  const width = Math.min(100, Math.max(0, value ?? 0));
  return (
    <div
      className={cn(
        "relative h-1.5 w-full overflow-hidden rounded-full bg-white/[0.07]",
        className,
      )}
    >
      <div
        className={cn(
          "h-full rounded-full bg-gradient-to-r from-cyan-400 to-sky-400 transition-[width] duration-500",
          indeterminate && "shimmer w-1/2 animate-pulse-soft",
          barClassName,
        )}
        style={indeterminate ? undefined : { width: `${width}%` }}
      />
    </div>
  );
}

export function Switch({
  checked,
  onCheckedChange,
  disabled,
  label,
}: {
  checked: boolean;
  onCheckedChange: (value: boolean) => void;
  disabled?: boolean;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-white/10 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:opacity-40",
        checked ? "bg-cyan-400/80" : "bg-white/10",
      )}
    >
      <span
        className={cn(
          "inline-block h-4.5 w-4.5 translate-x-1 rounded-full bg-white shadow transition-transform",
          checked && "translate-x-[22px]",
        )}
        style={{ width: 18, height: 18 }}
      />
    </button>
  );
}
