import { Link } from "react-router-dom";
import { ChevronRight, RotateCcw, Trash2, XCircle } from "lucide-react";
import type { Task } from "@/types";
import { FileIcon } from "@/components/FileIcon";
import { StatusBadge, SourceBadge } from "@/components/StatusBadge";
import { TaskProgress } from "@/components/TaskProgress";
import { Button } from "@/components/ui/button";
import {
  cn,
  formatBytes,
  hostnameOf,
  shortId,
  timeAgo,
} from "@/lib/utils";

export function TaskCard({
  task,
  onCancel,
  onRetry,
  onDelete,
  busy,
}: {
  task: Task;
  onCancel: (id: string) => void;
  onRetry: (id: string) => void;
  onDelete: (id: string) => void;
  busy?: boolean;
}) {
  const active = task.status === "running" || task.status === "pending";
  const terminal = !active;
  const canRetry = task.status === "failed" || task.status === "cancelled";

  return (
    <div
      className={cn(
        "group relative overflow-hidden rounded-2xl border border-slate-200 bg-card/65 p-4 transition-all duration-200 hover:border-slate-300 hover:bg-card/85 sm:p-5",
        task.status === "failed" && "border-rose-200",
        task.status === "completed" && "border-emerald-100",
      )}
    >
      <div className="flex items-start gap-3 sm:gap-4">
        <FileIcon name={task.filename} contentType={task.content_type} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="min-w-0">
              <Link
                to={`/tasks/${task.id}`}
                className="group/link inline-flex max-w-full items-center gap-1.5 text-[15px] font-medium text-slate-900 transition-colors hover:text-cyan-700"
              >
                <span className="truncate">{task.filename || "未命名下载"}</span>
                <ChevronRight className="h-3.5 w-3.5 shrink-0 text-slate-600 transition-transform group-hover/link:translate-x-0.5 group-hover/link:text-cyan-700" />
              </Link>
              <div className="mt-1 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-slate-500">
                <SourceBadge source={task.resolved_source} />
                <span className="font-mono text-slate-500">{shortId(task.id)}</span>
                <span className="truncate">{hostnameOf(task.url)}</span>
                {task.total_bytes ? <span>{formatBytes(task.total_bytes)}</span> : null}
                {task.attempt > 1 ? (
                  <span className="text-amber-700/80">第 {task.attempt} 次尝试</span>
                ) : null}
              </div>
            </div>
            <div className="flex items-center gap-2">
              <StatusBadge status={task.status} />
            </div>
          </div>

          <div className="mt-4">
            <TaskProgress task={task} compact />
          </div>

          <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-slate-200 pt-3">
            <div className="flex min-w-0 items-center gap-2 text-[11px] text-slate-500">
              <span className="truncate font-mono text-slate-500">
                {task.bucket}/{task.s3_key ?? task.object_key ?? "auto"}
              </span>
              <span className="hidden text-slate-700 sm:inline">·</span>
              <span className="hidden shrink-0 sm:inline">{timeAgo(task.created_at)}</span>
            </div>
            <div className="flex items-center gap-1">
              {active ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onCancel(task.id)}
                  title="取消任务"
                  className="text-slate-600 hover:text-amber-700"
                >
                  <XCircle className="h-3.5 w-3.5" />
                  <span className="hidden sm:inline">取消</span>
                </Button>
              ) : null}
              {canRetry ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onRetry(task.id)}
                  title="重试任务"
                  className="text-slate-600 hover:text-cyan-700"
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  <span className="hidden sm:inline">重试</span>
                </Button>
              ) : null}
              {terminal ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onDelete(task.id)}
                  title="删除记录"
                  className="text-slate-500 hover:text-rose-600"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              ) : null}
            </div>
          </div>

          {task.error ? (
            <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-[11px] leading-relaxed text-rose-700/90">
              <span className="mr-1.5 font-medium text-rose-600">
                {task.status === "cancelled" ? "已取消" : "错误"}：
              </span>
              {task.error}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
