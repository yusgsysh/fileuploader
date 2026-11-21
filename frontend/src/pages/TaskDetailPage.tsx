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
  Trash2,
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
        <div className="h-10 w-36 animate-pulse rounded-lg bg-slate-100" />
        <div className="h-48 animate-pulse rounded-2xl border border-slate-200 bg-slate-50" />
        <div className="h-64 animate-pulse rounded-2xl border border-slate-200 bg-slate-50" />
      </div>
    );
  }

  if (error || !task) {
    return (
      <div className="grid place-items-center rounded-2xl border border-dashed border-slate-200 px-6 py-24 text-center">
        <FileWarning className="h-9 w-9 text-slate-600" />
        <h1 className="mt-4 text-lg font-medium text-slate-800">任务不存在</h1>
        <p className="mt-1 text-sm text-slate-500">
          {error instanceof Error ? error.message : "该任务可能已被删除。"}
        </p>
        <Link to="/tasks" className="mt-5">
          <Button variant="outline">
            <ArrowLeft className="h-4 w-4" />
            返回任务列表
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
      toast.success(`${label} 已复制`);
    } catch {
      toast.error("当前环境无法访问剪贴板");
    }
  };

  return (
    <div className="space-y-6 animate-fade-up">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Link
          to="/tasks"
          className="inline-flex items-center gap-1.5 text-xs text-slate-500 transition-colors hover:text-slate-800"
        >
          <ArrowLeft className="h-3.5 w-3.5" />
          全部任务
        </Link>
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void refetch()}
            disabled={isFetching}
            title="刷新"
          >
            {isFetching ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RefreshCw className="h-3.5 w-3.5" />
            )}
            刷新
          </Button>
          {active ? (
            <Button
              size="sm"
              variant="outline"
              disabled={actions.busyId === task.id}
              onClick={() => actions.cancel(task.id)}
            >
              <XCircle className="h-3.5 w-3.5" />
              取消
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
              重试
            </Button>
          ) : null}
          {!active ? (
            <Button
              size="sm"
              variant="ghost"
              disabled={actions.busyId === task.id}
              onClick={() => actions.deleteTask(task.id, () => navigate("/tasks"))}
              className="text-slate-500 hover:text-rose-600"
            >
              <Trash2 className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </div>
      </div>

      <section className="relative overflow-hidden rounded-2xl border border-slate-200 bg-card/70 p-5 shadow-panel sm:p-6">
        <div className="pointer-events-none absolute -right-10 -top-24 h-64 w-64 rounded-full bg-cyan-50 blur-3xl" />
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
              {task.range_supported ? <Badge tone="green">支持断点续传</Badge> : null}
              {task.attempt > 1 ? <Badge tone="amber">第 {task.attempt} 次尝试</Badge> : null}
            </div>
            <h1 className="mt-2 break-all text-xl font-semibold tracking-tight text-slate-900 sm:text-2xl">
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
              {task.total_bytes ? <span>{formatBytes(task.total_bytes)}</span> : <span>大小未知</span>}
              {elapsed !== null ? <span>耗时 {formatDuration(elapsed)}</span> : null}
            </div>
          </div>
          <div className="grid shrink-0 grid-cols-3 gap-2 sm:grid-cols-1">
            <Metric label="下载" value={formatSpeed(task.progress.download_speed_bps)} />
            <Metric label="上传" value={formatSpeed(task.progress.upload_speed_bps)} />
            <Metric
              label="剩余"
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
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                <Layers3 className="h-4 w-4 text-cyan-700" />
                传输进度
              </div>
              <div className="flex items-center gap-3 text-[11px] text-slate-500">
                <span>总进度 {percentLabel(task.progress.overall_progress)}</span>
                <span>
                  分片 {task.progress.parts_completed}
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
              <div className="mb-4 flex items-center gap-2 text-sm font-semibold text-slate-900">
                <HardDrive className="h-4 w-4 text-violet-700" />
                MEGA 下载
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                <Info label="进程状态" value={task.progress.mega.process_state} />
                <Info
                  label="mega-get 进度"
                  value={
                    task.progress.mega.percentage !== null &&
                    task.progress.mega.percentage !== undefined
                      ? `${task.progress.mega.percentage.toFixed(1)}%`
                      : "按文件大小估算"
                  }
                />
                <Info label="临时占用" value={formatBytes(task.progress.mega.temp_bytes)} />
                <Info
                  label="文件数"
                  value={`${task.progress.mega.files_done} / ${task.progress.mega.files_total || "—"}`}
                />
                <Info
                  label="当前文件"
                  value={task.progress.mega.current_file ?? "—"}
                  className="sm:col-span-2"
                />
                <Info
                  label="临时目录"
                  value={task.progress.mega.temp_dir ?? "—"}
                  mono
                  className="sm:col-span-2"
                />
              </div>
            </section>
          ) : null}

          <section className="panel overflow-hidden">
            <div className="flex flex-wrap items-center justify-between gap-2 border-b border-slate-200 px-5 py-4">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                <Database className="h-4 w-4 text-emerald-700" />
                分片明细
              </div>
              <Badge tone="green">
                已上传 {task.progress.parts_completed}
                {task.progress.parts_total ? ` / ${task.progress.parts_total}` : ""}
              </Badge>
            </div>
            {parts.length === 0 ? (
              <div className="px-5 py-10 text-center text-xs text-slate-500">
                {active
                  ? "S3 确认每个 UploadPart 后，分片会显示在这里。"
                  : "暂无分片信息。"}
              </div>
            ) : (
              <div className="max-h-[360px] overflow-auto">
                <table className="w-full text-left text-[11px]">
                  <thead className="sticky top-0 bg-card/95 text-[10px] uppercase tracking-wider text-slate-500 backdrop-blur">
                    <tr>
                      <th className="px-5 py-2.5 font-medium">分片</th>
                      <th className="px-3 py-2.5 font-medium">大小</th>
                      <th className="px-3 py-2.5 font-medium">ETag</th>
                      <th className="px-5 py-2.5 text-right font-medium">状态</th>
                    </tr>
                  </thead>
                  <tbody>
                    {parts.map((part) => (
                      <tr key={part.part_number} className="border-t border-slate-200">
                        <td className="px-5 py-2.5 font-mono text-slate-700">
                          #{part.part_number}
                        </td>
                        <td className="px-3 py-2.5 font-mono text-slate-600">
                          {formatBytes(part.size)}
                        </td>
                        <td className="max-w-[220px] truncate px-3 py-2.5 font-mono text-slate-600">
                          {part.etag ?? "—"}
                        </td>
                        <td className="px-5 py-2.5 text-right">
                          <span className="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2 py-0.5 text-emerald-700">
                            <Check className="h-3 w-3" />
                            {part.status === "uploaded" ? "已上传" : part.status}
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
            <section className="rounded-2xl border border-rose-200 bg-rose-50 p-5">
              <div className="flex items-center gap-2 text-sm font-semibold text-rose-700">
                <FileWarning className="h-4 w-4" />
                错误日志
              </div>
              <pre className="code-scroll mt-3 max-h-52 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-rose-100 bg-slate-50 p-3 font-mono text-[11px] leading-relaxed text-rose-800/80">
                {task.error}
              </pre>
            </section>
          ) : null}

          <section className="panel overflow-hidden">
            <div className="border-b border-slate-200 px-5 py-4">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                <History className="h-4 w-4 text-slate-600" />
                执行历史
              </div>
            </div>
            {task.history.length === 0 ? (
              <div className="px-5 py-8 text-center text-xs text-slate-500">
                任务开始运行后会记录执行历史。
              </div>
            ) : (
              <div className="divide-y divide-slate-100">
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
                            ? "bg-emerald-50 text-emerald-700"
                            : item.status === "running"
                              ? "bg-cyan-50 text-cyan-700"
                              : "bg-rose-50 text-rose-600",
                        )}
                      >
                        {statusLabel[item.status as keyof typeof statusLabel] ?? item.status}
                      </span>
                      <span className="text-slate-600">{stageLabel[item.stage as keyof typeof stageLabel] ?? item.stage}</span>
                    </div>
                    <div className="text-[10px] text-slate-500">
                      {formatDate(item.started_at)}
                      {item.finished_at ? ` → ${formatDate(item.finished_at)}` : ""}
                    </div>
                    {item.error ? (
                      <div className="w-full break-words rounded-lg bg-slate-50 px-3 py-2 font-mono text-[10px] text-slate-500">
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
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <DownloadCloud className="h-4 w-4 text-cyan-700" />
              来源
            </div>
            <div className="mt-4 space-y-3">
              <Info label="识别来源" value={(task.resolved_source ?? "—").toUpperCase()} />
              <Info label="主机" value={hostnameOf(task.url)} />
              <div>
                <div className="field-label">URL</div>
                <div className="flex items-start gap-2 rounded-xl border border-slate-200 bg-slate-50 p-3">
                  <code className="min-w-0 flex-1 break-all text-[11px] leading-relaxed text-slate-600">
                    {task.url}
                  </code>
                  <button
                    onClick={() => void copy(task.url, "URL")}
                    className="shrink-0 rounded p-1 text-slate-500 hover:bg-slate-100 hover:text-slate-800"
                    title="复制链接"
                  >
                    <Clipboard className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
              <Info label="内容类型" value={task.content_type ?? "—"} />
              <Info label="自定义请求头" value={task.has_custom_headers ? "已配置" : "无"} />
              {task.auth_username ? <Info label="Basic Auth" value={task.auth_username} /> : null}
            </div>
          </section>

          <section className="panel p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <Server className="h-4 w-4 text-emerald-700" />
              目标位置
            </div>
            <div className="mt-4 space-y-3">
              <Info label="S3 配置" value={task.s3_config_id} mono />
              <Info label="存储桶" value={task.bucket} mono />
              <div>
                <div className="field-label">对象 Key</div>
                <div className="flex items-start gap-2 rounded-xl border border-slate-200 bg-slate-50 p-3">
                  <code className="min-w-0 flex-1 break-all text-[11px] leading-relaxed text-emerald-700/80">
                    s3://{task.bucket}/{task.s3_key ?? task.object_key ?? "auto"}
                  </code>
                  <button
                    onClick={() =>
                      void copy(
                        `s3://${task.bucket}/${task.s3_key ?? task.object_key ?? ""}`,
                        "S3 URI",
                      )
                    }
                    className="shrink-0 rounded p-1 text-slate-500 hover:bg-slate-100 hover:text-slate-800"
                    title="复制 S3 URI"
                  >
                    <Clipboard className="h-3.5 w-3.5" />
                  </button>
                </div>
              </div>
              <Info label="覆盖策略" value={task.allow_overwrite ? "允许覆盖" : "保护已有对象"} />
              {task.s3_upload_id ? (
                <Info label="分片上传 ID" value={task.s3_upload_id} mono />
              ) : null}
            </div>
          </section>

          <section className="panel p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <Cpu className="h-4 w-4 text-slate-600" />
              任务信息
            </div>
            <div className="mt-4 space-y-3">
              <Info label="任务 ID" value={task.id} mono />
              <Info label="尝试次数" value={String(task.attempt)} />
              <Info label="阶段" value={stageLabel[task.stage]} />
              <Info label="状态" value={statusLabel[task.status]} />
              <Info label="创建时间" value={formatDate(task.created_at)} />
              <Info label="开始时间" value={formatDate(task.started_at)} />
              <Info label="结束时间" value={formatDate(task.finished_at)} />
              <Info
                label="断点续传"
                value={
                  task.range_supported === null || task.range_supported === undefined
                    ? "未知"
                    : task.range_supported
                      ? "支持"
                      : "不支持"
                }
              />
              <Info label="临时数据量" value={formatBytes(task.temp_bytes)} />
            </div>
          </section>

        </div>
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-right sm:min-w-[112px]">
      <div className="text-[9px] uppercase tracking-wider text-slate-600">{label}</div>
      <div className="font-mono text-xs text-slate-800">{value}</div>
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
          "break-words text-xs text-slate-700",
          mono && "font-mono text-[11px] text-slate-600",
        )}
      >
        {value}
      </div>
    </div>
  );
}
