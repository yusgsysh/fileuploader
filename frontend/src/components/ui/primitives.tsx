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
    default: "border-slate-200 bg-slate-100 text-slate-700",
    cyan: "border-cyan-200 bg-cyan-50 text-cyan-700",
    green: "border-emerald-200 bg-emerald-50 text-emerald-700",
    red: "border-rose-200 bg-rose-50 text-rose-700",
    amber: "border-amber-200 bg-amber-50 text-amber-700",
    violet: "border-violet-200 bg-violet-50 text-violet-700",
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
        "relative h-1.5 w-full overflow-hidden rounded-full bg-slate-100",
        className,
      )}
    >
      <div
        className={cn(
          "h-full rounded-full bg-gradient-to-r from-cyan-500 to-sky-500 transition-[width] duration-500",
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
        "relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border border-slate-200 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50 disabled:opacity-40",
        checked ? "bg-cyan-600" : "bg-slate-200",
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
