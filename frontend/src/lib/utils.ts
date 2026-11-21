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
  if (seconds < 1) return "<1 秒";
  const total = Math.round(seconds);
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (days > 0) return `${days} 天 ${hours} 小时`;
  if (hours > 0) return `${hours} 小时 ${minutes} 分`;
  if (minutes > 0) return `${minutes} 分 ${secs} 秒`;
  return `${secs} 秒`;
}

export function formatDate(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "—";
  return new Intl.DateTimeFormat("zh-CN", {
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
  if (secs < 60) return `${secs} 秒前`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} 分钟前`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  return `${days} 天前`;
}

export const statusLabel: Record<TaskStatus, string> = {
  pending: "排队中",
  running: "进行中",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export const stageLabel: Record<TaskStage, string> = {
  pending: "等待中",
  downloading: "下载中",
  preparing_upload: "准备上传",
  uploading: "上传中",
  finalizing: "正在完成",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

export function statusTone(status: TaskStatus) {
  switch (status) {
    case "running":
      return "border-cyan-200 bg-cyan-50 text-cyan-700";
    case "completed":
      return "border-emerald-200 bg-emerald-50 text-emerald-700";
    case "failed":
      return "border-rose-200 bg-rose-50 text-rose-700";
    case "cancelled":
      return "border-amber-200 bg-amber-50 text-amber-700";
    default:
      return "border-slate-200 bg-slate-100 text-slate-700";
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
