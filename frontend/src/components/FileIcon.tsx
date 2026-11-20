import {
  Archive,
  File,
  FileAudio,
  FileCode2,
  FileImage,
  FileText,
  FileVideo,
  Package,
} from "lucide-react";
import { cn } from "@/lib/utils";

export function FileIcon({
  name,
  contentType,
  className,
}: {
  name?: string | null;
  contentType?: string | null;
  className?: string;
}) {
  const ext = (name ?? "").split(".").pop()?.toLowerCase() ?? "";
  const type = contentType ?? "";
  let Icon = File;
  let tone = "text-slate-300";

  if (type.startsWith("image/") || ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"].includes(ext)) {
    Icon = FileImage;
    tone = "text-violet-300";
  } else if (type.startsWith("video/") || ["mp4", "mkv", "mov", "webm", "avi"].includes(ext)) {
    Icon = FileVideo;
    tone = "text-sky-300";
  } else if (type.startsWith("audio/") || ["mp3", "flac", "wav", "m4a", "opus"].includes(ext)) {
    Icon = FileAudio;
    tone = "text-fuchsia-300";
  } else if (
    ["zip", "7z", "rar", "tar", "gz", "xz", "bz2", "zst"].includes(ext) ||
    type.includes("zip") ||
    type.includes("compressed")
  ) {
    Icon = Archive;
    tone = "text-amber-300";
  } else if (["apk", "deb", "rpm", "dmg", "exe", "appimage"].includes(ext)) {
    Icon = Package;
    tone = "text-emerald-300";
  } else if (
    ["js", "ts", "tsx", "rs", "py", "go", "json", "toml", "yaml", "yml", "html", "css", "sh"].includes(ext)
  ) {
    Icon = FileCode2;
    tone = "text-cyan-300";
  } else if (["txt", "md", "pdf", "doc", "docx", "csv", "log"].includes(ext) || type.startsWith("text/")) {
    Icon = FileText;
    tone = "text-indigo-300";
  }

  return (
    <div
      className={cn(
        "grid h-11 w-11 shrink-0 place-items-center rounded-xl border border-white/[0.07] bg-white/[0.035]",
        className,
      )}
    >
      <Icon className={cn("h-5 w-5", tone)} strokeWidth={1.7} />
    </div>
  );
}
