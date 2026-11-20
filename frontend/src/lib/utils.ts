import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import type { TaskStage, TaskStatus } from "@/types";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBytes(value?: number | null, precision = 1): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  if (value === 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const negative = value < 0;
  let size = Math.abs(value);
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : precision;
  return `${negative ? "-" : ""}${size.toFixed(digits)} ${units[unit]}`;
}

export function formatSpeed(bps?: number | null): string {
  if (!bps || bps <= 0) return "—";
  return `${formatBytes(bps)}/s`;
}

export function formatDuration(seconds?: number | null): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) {
    return "—";
  }
  if (seconds < 1) return "<1s";
  const total = Math.round(seconds);
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

export function formatDate(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

export function timeAgo(value?: string | null): string {
  if (!value) return "—";
  const timestamp = new Date(value).getTime();
  if (Number.isNaN(timestamp)) return "—";
  const diff = Math.max(0, Date.now() - timestamp);
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export const statusLabel: Record<TaskStatus, string> = {
  pending: "Queued",
  running: "Running",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export const stageLabel: Record<TaskStage, string> = {
  pending: "Pending",
  downloading: "Downloading",
  preparing_upload: "Preparing upload",
  uploading: "Uploading",
  finalizing: "Finalizing",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function statusTone(status: TaskStatus) {
  switch (status) {
    case "running":
      return "border-cyan-400/25 bg-cyan-400/10 text-cyan-200";
    case "completed":
      return "border-emerald-400/25 bg-emerald-400/10 text-emerald-200";
    case "failed":
      return "border-rose-400/25 bg-rose-400/10 text-rose-200";
    case "cancelled":
      return "border-amber-400/25 bg-amber-400/10 text-amber-200";
    default:
      return "border-slate-400/20 bg-slate-400/10 text-slate-300";
  }
}

export function progressValue(value?: number | null): number {
  if (value === null || value === undefined || !Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

export function percentLabel(value?: number | null): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "—";
  }
  return `${value.toFixed(value >= 99.95 ? 0 : 1)}%`;
}

export function shortId(id: string): string {
  return id.slice(0, 8);
}

export function parseHeadersJson(input: string): Record<string, string> {
  const headers: Record<string, string> = {};
  for (const line of input.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const idx = trimmed.indexOf(":");
    if (idx <= 0) continue;
    headers[trimmed.slice(0, idx).trim()] = trimmed.slice(idx + 1).trim();
  }
  return headers;
}

export function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return "—";
  }
}

export function detectSource(url: string): "mega" | "http" | null {
  try {
    const parsed = new URL(url);
    if (
      [
        "mega.nz",
        "www.mega.nz",
        "mega.co.nz",
        "www.mega.co.nz",
        "mega.io",
        "www.mega.io",
      ].includes(parsed.hostname.toLowerCase())
    ) {
      return "mega";
    }
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return "http";
    }
  } catch {
    return null;
  }
  return null;
}

export function isMegaShare(url: string): boolean {
  return detectSource(url) === "mega";
}
