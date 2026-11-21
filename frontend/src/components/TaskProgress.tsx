import { ArrowDownToLine, ArrowUpFromLine } from "lucide-react";
import type { Task } from "@/types";
import {
  cn,
  formatBytes,
  formatDuration,
  formatSpeed,
  percentLabel,
  progressValue,
} from "@/lib/utils";
import { Progress } from "@/components/ui/primitives";

export function TaskProgress({
  task,
  compact = false,
}: {
  task: Task;
  compact?: boolean;
}) {
  const download = task.progress.download_progress ?? task.progress.mega?.percentage ?? null;
  const upload = task.progress.upload_progress ?? null;
  const active = task.status === "running";
  const unknownDownload = active && download === null && task.downloaded_bytes > 0;
  const unknownUpload =
    active && upload === null && task.uploaded_bytes > 0 && task.downloaded_bytes > 0;

  return (
    <div className={cn("space-y-3", compact && "space-y-2")}>
      <div>
        <div className="mb-1.5 flex items-center justify-between gap-3 text-[11px]">
          <span className="inline-flex items-center gap-1.5 text-slate-600">
            <ArrowDownToLine className="h-3.5 w-3.5 text-cyan-600" />
            下载
          </span>
          <span className="flex items-center gap-2 font-mono text-slate-600">
            <span className="text-slate-700">
              {download === null ? formatBytes(task.downloaded_bytes) : percentLabel(download)}
            </span>
            <span className="text-[10px] text-slate-500">
              {formatSpeed(task.progress.download_speed_bps)}
            </span>
          </span>
        </div>
        {unknownDownload ? (
          <Progress indeterminate />
        ) : (
          <Progress
            value={progressValue(download)}
            barClassName="from-cyan-500 to-sky-500"
            className={cn(download === null && "opacity-40")}
          />
        )}
      </div>
      <div>
        <div className="mb-1.5 flex items-center justify-between gap-3 text-[11px]">
          <span className="inline-flex items-center gap-1.5 text-slate-600">
            <ArrowUpFromLine className="h-3.5 w-3.5 text-emerald-600" />
            上传
          </span>
          <span className="flex items-center gap-2 font-mono text-slate-600">
            <span className="text-slate-700">
              {upload === null ? formatBytes(task.uploaded_bytes) : percentLabel(upload)}
            </span>
            <span className="text-[10px] text-slate-500">
              {formatSpeed(task.progress.upload_speed_bps)}
            </span>
          </span>
        </div>
        {unknownUpload ? (
          <Progress
            indeterminate
            barClassName="from-emerald-500 to-teal-500"
          />
        ) : (
          <Progress
            value={progressValue(upload)}
            barClassName="from-emerald-500 to-teal-500"
            className={cn(upload === null && "opacity-40")}
          />
        )}
      </div>
      {!compact ? (
        <div className="flex flex-wrap items-center justify-between gap-2 border-t border-slate-200 pt-2 text-[11px] text-slate-500">
          <span>
            已下载 {formatBytes(task.downloaded_bytes)} · 已上传{" "}
            {formatBytes(task.uploaded_bytes)}
          </span>
          <span>
            {task.total_bytes ? `共 ${formatBytes(task.total_bytes)}` : "大小未知"}
            {task.progress.eta_secs !== null && task.progress.eta_secs !== undefined
              ? ` · 剩余 ${formatDuration(task.progress.eta_secs)}`
              : ""}
          </span>
        </div>
      ) : null}
    </div>
  );
}
