import { useEffect, useMemo, useRef, useState } from "react";
import type React from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowLeft,
  Check,
  Clipboard,
  Clock3,
  Cpu,
  Database,
  DownloadCloud,
  FileWarning,
  HardDrive,
  History,
  Layers3,
  Loader2,
  RefreshCw,
  RotateCcw,
  Server,
  ShieldCheck,
  Trash2,
  UploadCloud,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { api, queryKeys } from "@/lib/api";
import { useTaskActions } from "@/hooks/useTaskActions";
import { FileIcon } from "@/components/FileIcon";
import { StatusBadge, SourceBadge } from "@/components/StatusBadge";
import { TaskProgress } from "@/components/TaskProgress";
import { SpeedChart, type SpeedSample } from "@/components/SpeedChart";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/primitives";
import {
  cn,
  formatBytes,
  formatDate,
  formatDuration,
  formatSpeed,
  hostnameOf,
  percentLabel,
  shortId,
  stageLabel,
  statusLabel,
} from "@/lib/utils";

export function TaskDetailPage() {
  const { id = "" } = useParams();
  const navigate = useNavigate();
  const actions = useTaskActions();
  const [samples, setSamples] = useState<SpeedSample[]>([]);
  const signature = useRef("");

  const { data: task, isLoading, error, refetch, isFetching } = useQuery({
    queryKey: queryKeys.task(id),
    queryFn: () => api.getTask(id),
    enabled: Boolean(id),
    refetchInterval: 5000,
  });

  useEffect(() => {
    setSamples([]);
    signature.current = "";
  }, [id]);

  useEffect(() => {
    if (!task) return;
    const nextSignature = [
      task.updated_at,
      task.downloaded_bytes,
      task.uploaded_bytes,
      Math.round(task.progress.download_speed_bps),
      Math.round(task.progress.upload_speed_bps),
      task.status,
    ].join(":");
    if (nextSignature === signature.current) return;
    signature.current = nextSignature;
    setSamples((current) =>
      [
        ...current,
        {
          at: Date.now(),
          download: task.progress.download_speed_bps,
          upload: task.progress.upload_speed_bps,
        },
      ].slice(-120),
    );
  }, [task]);

  const elapsed = useMemo(() => {
    if (!task?.started_at) return null;
    const start = new Date(task.started_at).getTime();
    const end = task.finished_at ? new Date(task.finished_at).getTime() : Date.now();
    if (Number.isNaN(start) || Number.isNaN(end)) return null;
    return Math.max(0, (end - start) / 1000);
  }, [task?.started_at, task?.finished_at]);

  if (isLoading) {
    return (
      <div className="space-y-4">
        <div className="h-10 w-36 animate-pulse rounded-lg bg-white/[0.04]" />
        <div className="h-48 animate-pulse rounded-2xl border border-white/[0.06] bg-white/[0.025]" />
        <div className="h-64 animate-pulse rounded-2xl border border-white/[0.06] bg-white/[0.025]" />
      </div>
    );
  }

  if (error || !task) {
    return (
      <div className="grid place-items-center rounded-2xl border border-dashed border-white/10 px-6 py-24 text-center">
        <FileWarning className="h-9 w-9 text-slate-600" />
        <h1 className="mt-4 text-lg font-medium text-slate-200">Task not found</h1>
        <p className="mt-1 text-sm text-slate-500">
          {error instanceof Error ? error.message : "The task may have been deleted."}
        </p>
        <Link to="/tasks" className="mt-5">
          <Button variant="outline">
            <ArrowLeft className="h-4 w-4" />
            Back to tasks
          </Button>
        </Link>
      </div>
    );
  }

  const active = task.status === "running" || task.status === "pending";
  const canRetry = task.status === "failed" || task.status === "cancelled";
  const parts = task.progress.parts.slice(-200).reverse();
  const copy = async (value: string, label: string) => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(`${label} copied`);
    } catch {
      toast.error("Clipboard access is not available");
    }
  };

  return (
    <div className="space-y-6 animate-fade-up">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Link
          to="/tasks"
          className="inline-flex items-center gap-1.5 text-xs text-slate-500 transition-colors hover:text-slate-200"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          All tasks
        </Link>
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refetch()}
            disabled={isFetching}
            title="Refresh"
          >
            {isFetching ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            Refresh
          </Button>
          {active ? (
            <Button
              size="sm"
              variant="outline"
              disabled={actions.busyId === task.id}
              onClick={() => actions.cancel(task.id)}
            >
              <XCircle className="h-3.5 w-3.5" />
              Cancel
            </Button>
          ) : null}
          {canRetry ? (
            <Button
              size="sm"
              variant="subtle"
              disabled={actions.busyId === task.id}
              onClick={() => actions.retry(task.id)}
            >
              <RotateCcw className="h-3.5 w-3.5" />
              Retry
            </Button>
          ) : null}
          {!active ? (
            <Button
              size="sm"
              variant="ghost"
              disabled={actions.busyId === task.id}
              onClick={() => actions.deleteTask(task.id, () => navigate("/tasks"))}
              className="text-slate-500 hover:text-rose-300"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </div>
      </div>

      <section className="relative overflow-hidden rounded-2xl border border-white/[0.07] bg-card/70 p-5 shadow-panel sm:p-6">
        <div className="pointer-events-none absolute -right-10 -top-24 h-64 w-64 rounded-full bg-cyan-400/[0.08] blur-3xl" />
        <div className="relative flex flex-col gap-4 sm:flex-row sm:items-start">
          <FileIcon
            name={task.filename}
            contentType={task.content_type}
            className="h-14 w-14 rounded-2xl"
          />
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <StatusBadge status={task.status} />
              <SourceBadge source={task.resolved_source} />
              <Badge>{stageLabel[task.stage]}</Badge>
              {task.range_supported ? <Badge tone="green">Range resume</Badge> : null}
              {task.attempt > 1 ? <Badge tone="amber">Attempt {task.attempt}</Badge> : null}
            </div>
            <h1 className="mt-2 break-all text-xl font-semibold tracking-tight text-white sm:text-2xl">
              {task.filename}
            </h1>
            <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-1 text-[11px] text-slate-500">
              <span className="inline-flex items-center gap-1 font-mono">
                <Cpu className="h-3 w-3" />
                {shortId(task.id)}
              </span>
              <span className="inline-flex items-center gap-1">
                <Clock3 className="h-3 w-3" />
                {formatDate(task.created_at)}
              </span>
              {task.total_bytes ? <span>{formatBytes(task.total_bytes)}</span> : <span>Size unknown</span>}
              {elapsed !== null ? <span>Elapsed {formatDuration(elapsed)}</span> : null}
            </div>
          </div>
          <div className="grid shrink-0 grid-cols-3 gap-2 sm:grid-cols-1">
            <Metric label="Download" value={formatSpeed(task.progress.download_speed_bps)} />
            <Metric label="Upload" value={formatSpeed(task.progress.upload_speed_bps)} />
            <Metric
              label="ETA"
              value={
                task.progress.eta_secs !== null && task.progress.eta_secs !== undefined
                  ? formatDuration(task.progress.eta_secs)
                  : "—"
              }
            />
          </div>
        </div>
      </section>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="space-y-5">
          <section className="panel p-5 sm:p-6">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
                <Layers3 className="h-4 w-4 text-cyan-300" />
                Transfer progress
              </div>
              <div className="flex items-center gap-3 text-[11px] text-slate-500">
                <span>Overall {percentLabel(task.progress.overall_progress)}</span>
                <span>
                  Parts {task.progress.parts_completed}
                  {task.progress.parts_total ? ` / ${task.progress.parts_total}` : ""}
                </span>
              </div>
            </div>
            <TaskProgress task={task} />
            <div className="mt-5">
              <SpeedChart samples={samples} />
            </div>
          </section>

          {task.progress.mega ? (
            <section className="panel p-5 sm:p-6">
              <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-slate-100">
                <HardDrive className="h-4 w-4 text-violet-300" />
                MEGA download
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <Info label="Process state" value={task.progress.mega.process_state} />
                <Info
                  label="mega-get progress"
                  value={
                    task.progress.mega.percentage !== null &&
                    task.progress.mega.percentage !== undefined
                      ? `${task.progress.mega.percentage.toFixed(1)}%`
                      : "size-based tracking"
                  }
                />
                <Info label="Temporary size" value={formatBytes(task.progress.mega.temp_bytes)} />
                <Info
                  label="Files"
                  value={`${task.progress.mega.files_done} / ${task.progress.mega.files_total || "—"}`}
                />
                <Info
                  label="Current file"
                  value={task.progress.mega.current_file ?? "—"}
                  className="sm:col-span-2"
                />
                <Info
                  label="Temporary directory"
                  value={task.progress.mega.temp_dir ?? "—"}
                  mono
                  className="sm:col-span-2"
                />
              </div>
            </section>
          ) : null}

          <section className="panel overflow-hidden">
            <div className="flex flex-wrap items-center justify-between gap-2 border-b border-white/[0.06] px-5 py-4">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
                <Database className="h-4 w-4 text-emerald-300" />
                Multipart parts
              </div>
              <Badge tone="green">
                {task.progress.parts_completed} uploaded
                {task.progress.parts_total ? ` / ${task.progress.parts_total}` : ""}
              </Badge>
            </div>
            {parts.length === 0 ? (
              <div className="px-5 py-10 text-center text-xs text-slate-500">
                {active
                  ? "Parts will appear as S3 confirms each UploadPart request."
                  : "No multipart part information is available."}
              </div>
            ) : (
              <div className="max-h-[360px] overflow-auto">
                <table className="w-full text-left text-[11px]">
                  <thead className="sticky top-0 bg-card/95 text-[10px] uppercase tracking-wider text-slate-500 backdrop-blur">
                    <tr>
                      <th className="px-5 py-2.5 font-medium">Part</th>
                      <th className="px-3 py-2.5 font-medium">Size</th>
                      <th className="px-3 py-2.5 font-medium">ETag</th>
                      <th className="px-5 py-2.5 text-right font-medium">Status</th>
                    </tr>
                  </thead>
                  <tbody>
                    {parts.map((part) => (
                      <tr key={part.part_number} className="border-t border-white/[0.04]">
                        <td className="px-5 py-2.5 font-mono text-slate-300">
                          #{part.part_number}
                        </td>
                        <td className="px-3 py-2.5 font-mono text-slate-400">
                          {formatBytes(part.size)}
                        </td>
                        <td className="max-w-[220px] truncate px-3 py-2.5 font-mono text-slate-600">
                          {part.etag ?? "—"}
                        </td>
                        <td className="px-5 py-2.5 text-right">
                          <span className="inline-flex items-center gap-1 rounded-full bg-emerald-400/10 px-2 py-0.5 text-emerald-300">
                            <Check className="h-3 w-3" />
                            {part.status}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {task.error ? (
            <section className="rounded-2xl border border-rose-400/20 bg-rose-400/[0.045] p-5">
              <div className="flex items-center gap-2 text-sm font-semibold text-rose-200">
                <FileWarning className="h-4 w-4" />
                Error log
              </div>
              <pre className="code-scroll mt-3 max-h-52 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-rose-400/10 bg-black/25 p-3 font-mono text-[11px] leading-relaxed text-rose-100/80">
                {task.error}
              </pre>
            </section>
          ) : null}

          <section className="panel overflow-hidden">
            <div className="border-b border-white/[0.06] px-5 py-4">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
                <History className="h-4 w-4 text-slate-400" />
                Execution history
              </div>
            </div>
            {task.history.length === 0 ? (
              <div className="px-5 py-8 text-center text-xs text-slate-500">
                History is recorded when a task starts running.
              </div>
            ) : (
              <div className="divide-y divide-white/[0.04]">
                {task.history.map((item) => (
                  <div
                    key={item.id}
                    className="flex flex-wrap items-center justify-between gap-3 px-5 py-3 text-xs"
                  >
                    <div className="flex items-center gap-3">
                      <span className="font-mono text-[10px] text-slate-500">
                        #{item.attempt}
                      </span>
                      <span
                        className={cn(
                          "rounded-full px-2 py-0.5 text-[10px]",
                          item.status === "completed"
                            ? "bg-emerald-400/10 text-emerald-300"
                            : item.status === "running"
                              ? "bg-cyan-400/10 text-cyan-300"
                              : "bg-rose-400/10 text-rose-300",
                        )}
                      >
                        {item.status}
                      </span>
                      <span className="text-slate-400">{stageLabel[item.stage as keyof typeof stageLabel] ?? item.stage}</span>
                    </div>
                    <div className="text-[10px] text-slate-500">
                      {formatDate(item.started_at)}
                      {item.finished_at ? ` → ${formatDate(item.finished_at)}` : ""}
                    </div>
                    {item.error ? (
                      <div className="w-full break-words rounded-lg bg-black/20 px-3 py-2 font-mono text-[10px] text-slate-500">
                        {item.error}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>

        <div className="space-y-5">
          <section className="panel p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
              <DownloadCloud className="h-4 w-4 text-cyan-300" />
              Source
            </div>
            <div className="mt-4 space-y-3">
              <Info label="Resolved source" value={(task.resolved_source ?? "—").toUpperCase()} />
              <Info label="Host" value={hostnameOf(task.url)} />
              <div>
                <div className="field-label">URL</div>
                <div className="flex items-start gap-2 rounded-xl border border-white/[0.06] bg-black/20 p-3">
                  <code className="min-w-0 flex-1 break-all text-[11px] leading-relaxed text-slate-400">
                    {task.url}
                  </code>
                  <button
                    onClick={() => void copy(task.url, "URL")}
                    className="shrink-0 rounded p-1 text-slate-500 hover:bg-white/[0.06] hover:text-slate-200"
                    title="Copy URL"
                  >
                    <Clipboard className="h-3.5 w-3.5" />
                  </button>
                </div>
                {task.resolved_source === "mega" ? (
                  <p className="mt-1.5 flex items-center gap-1 text-[10px] text-slate-600">
                    <ShieldCheck className="h-3 w-3" />
                    Share keys are never written to application logs.
                  </p>
                ) : null}
              </div>
              <Info label="Content type" value={task.content_type ?? "—"} />
              <Info label="Custom headers" value={task.has_custom_headers ? "configured" : "none"} />
              {task.auth_username ? <Info label="Basic Auth" value={task.auth_username} /> : null}
            </div>
          </section>

          <section className="panel p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
              <Server className="h-4 w-4 text-emerald-300" />
              Destination
            </div>
            <div className="mt-4 space-y-3">
              <Info label="S3 configuration" value={task.s3_config_id} mono />
              <Info label="Bucket" value={task.bucket} mono />
              <div>
                <div className="field-label">Object key</div>
                <div className="flex items-start gap-2 rounded-xl border border-white/[0.06] bg-black/20 p-3">
                  <code className="min-w-0 flex-1 break-all text-[11px] leading-relaxed text-emerald-300/80">
                    s3://{task.bucket}/{task.s3_key ?? task.object_key ?? "auto"}
                  </code>
                  <button
                    onClick={() =>
                      void copy(
                        `s3://${task.bucket}/${task.s3_key ?? task.object_key ?? ""}`,
                        "S3 URI",
                      )
                    }
                    className="shrink-0 rounded p-1 text-slate-500 hover:bg-white/[0.06] hover:text-slate-200"
                    title="Copy S3 URI"
                  >
                    <Clipboard className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
              <Info label="Overwrite policy" value={task.allow_overwrite ? "allowed" : "protected"} />
              {task.s3_upload_id ? (
                <Info label="Multipart upload ID" value={task.s3_upload_id} mono />
              ) : null}
            </div>
          </section>

          <section className="panel p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
              <Cpu className="h-4 w-4 text-slate-400" />
              Task metadata
            </div>
            <div className="mt-4 space-y-3">
              <Info label="Task ID" value={task.id} mono />
              <Info label="Attempt" value={String(task.attempt)} />
              <Info label="Stage" value={stageLabel[task.stage]} />
              <Info label="Status" value={statusLabel[task.status]} />
              <Info label="Created" value={formatDate(task.created_at)} />
              <Info label="Started" value={formatDate(task.started_at)} />
              <Info label="Finished" value={formatDate(task.finished_at)} />
              <Info
                label="Range support"
                value={
                  task.range_supported === null || task.range_supported === undefined
                    ? "unknown"
                    : task.range_supported
                      ? "yes"
                      : "no"
                }
              />
              <Info label="Temp bytes" value={formatBytes(task.temp_bytes)} />
            </div>
          </section>

          <section className="rounded-2xl border border-white/[0.06] bg-black/15 p-4 text-[11px] leading-relaxed text-slate-500">
            <div className="flex items-center gap-2 font-medium text-slate-300">
              <UploadCloud className="h-3.5 w-3.5 text-emerald-300" />
              Safety notes
            </div>
            <ul className="mt-2 space-y-1.5">
              <li>HTTP bodies are piped directly into S3 parts, never cached as a whole file.</li>
              <li>Cancel closes active streams and calls AbortMultipartUpload.</li>
              <li>Failed MEGA uploads keep their isolated temp directory for a clean retry.</li>
            </ul>
          </section>
        </div>
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-white/[0.06] bg-black/20 px-3 py-2 text-right sm:min-w-[112px]">
      <div className="text-[9px] uppercase tracking-wider text-slate-600">{label}</div>
      <div className="font-mono text-xs text-slate-200">{value}</div>
    </div>
  );
}

function Info({
  label,
  value,
  mono,
  className,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div className={className}>
      <div className="field-label">{label}</div>
      <div
        className={cn(
          "break-words text-xs text-slate-300",
          mono && "font-mono text-[11px] text-slate-400",
        )}
      >
        {value}
      </div>
    </div>
  );
}
