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
        "group relative overflow-hidden rounded-2xl border border-white/[0.07] bg-card/65 p-4 transition-all duration-200 hover:border-white/[0.12] hover:bg-card/85 sm:p-5",
        task.status === "failed" && "border-rose-400/15",
        task.status === "completed" && "border-emerald-400/10",
      )}
    >
      <div className="flex items-start gap-3 sm:gap-4">
        <FileIcon name={task.filename} contentType={task.content_type} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div className="min-w-0">
              <Link
                to={`/tasks/${task.id}`}
                className="group/link inline-flex max-w-full items-center gap-1.5 text-[15px] font-medium text-slate-100 transition-colors hover:text-cyan-200"
              >
                <span className="truncate">{task.filename || "Untitled download"}</span>
                <ChevronRight className="h-3.5 w-3.5 shrink-0 text-slate-600 transition-transform group-hover/link:translate-x-0.5 group-hover/link:text-cyan-300" />
              </Link>
              <div className="mt-1 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[11px] text-slate-500">
                <SourceBadge source={task.resolved_source} />
                <span className="font-mono text-slate-500">{shortId(task.id)}</span>
                <span className="truncate">{hostnameOf(task.url)}</span>
                {task.total_bytes ? <span>{formatBytes(task.total_bytes)}</span> : null}
                {task.attempt > 1 ? (
                  <span className="text-amber-300/80">attempt {task.attempt}</span>
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

          <div className="mt-3 flex flex-wrap items-center justify-between gap-2 border-t border-white/[0.05] pt-3">
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
                  title="Cancel task"
                  className="text-slate-400 hover:text-amber-200"
                >
                  <XCircle className="h-3.5 w-3.5" />
                  <span className="hidden sm:inline">Cancel</span>
                </Button>
              ) : null}
              {canRetry ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onRetry(task.id)}
                  title="Retry task"
                  className="text-slate-400 hover:text-cyan-200"
                >
                  <RotateCcw className="h-3.5 w-3.5" />
                  <span className="hidden sm:inline">Retry</span>
                </Button>
              ) : null}
              {terminal ? (
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={busy}
                  onClick={() => onDelete(task.id)}
                  title="Delete record"
                  className="text-slate-500 hover:text-rose-300"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              ) : null}
            </div>
          </div>

          {task.error ? (
            <div className="mt-3 rounded-lg border border-rose-400/15 bg-rose-400/[0.05] px-3 py-2 text-[11px] leading-relaxed text-rose-200/90">
              <span className="mr-1.5 font-medium text-rose-300">
                {task.status === "cancelled" ? "Cancelled" : "Error"}:
              </span>
              {task.error}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
