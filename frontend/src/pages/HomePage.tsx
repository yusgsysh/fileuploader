import { useEffect, useMemo, useState } from "react";
import type React from "react";
import { Link, useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  ArrowUpFromLine,
  CheckCircle2,
  ChevronDown,
  DownloadCloud,
  Gauge,
  HardDrive,
  KeyRound,
  Link2,
  Loader2,
  ServerCog,
} from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, queryKeys } from "@/lib/api";
import type { CreateTaskInput, SourceType } from "@/types";
import {
  cn,
  detectSource,
  formatBytes,
  parseHeadersJson,
  shortId,
  statusLabel,
  timeAgo,
} from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Field, Input, Select, Textarea } from "@/components/ui/input";
import { Badge, Switch } from "@/components/ui/primitives";
import { Card } from "@/components/ui/card";
import { SourceBadge, StatusBadge } from "@/components/StatusBadge";
import { TaskProgress } from "@/components/TaskProgress";

interface HomeForm {
  url: string;
  source_type: SourceType;
  s3_config_id: string;
  bucket: string;
  object_key: string;
  filename: string;
  user_agent: string;
  referer: string;
  cookie: string;
  auth_username: string;
  auth_password: string;
  headers_text: string;
  allow_overwrite: boolean;
}

const initialForm: HomeForm = {
  url: "",
  source_type: "auto",
  s3_config_id: "",
  bucket: "",
  object_key: "",
  filename: "",
  user_agent: "",
  referer: "",
  cookie: "",
  auth_username: "",
  auth_password: "",
  headers_text: "",
  allow_overwrite: false,
};

export function HomePage() {
  const [form, setForm] = useState<HomeForm>(initialForm);
  const [advanced, setAdvanced] = useState(false);
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data: configs, isLoading: configsLoading } = useQuery({
    queryKey: queryKeys.s3Configs,
    queryFn: api.listS3Configs,
  });
  const { data: system } = useQuery({
    queryKey: queryKeys.system,
    queryFn: api.systemStatus,
    refetchInterval: 12_000,
  });
  const { data: tasks } = useQuery({
    queryKey: queryKeys.tasks,
    queryFn: () => api.listTasks(30),
    refetchInterval: 5000,
  });
  const { data: buckets } = useQuery({
    queryKey: queryKeys.buckets(form.s3_config_id),
    queryFn: () => api.listBuckets(form.s3_config_id || undefined),
    enabled: Boolean(form.s3_config_id),
    retry: false,
    staleTime: 60_000,
  });

  useEffect(() => {
    if (!form.s3_config_id && configs && configs.length > 0) {
      const first = configs[0];
      setForm((current) => ({
        ...current,
        s3_config_id: first.id,
        bucket: current.bucket || first.bucket,
      }));
    }
  }, [configs, form.s3_config_id]);

  const detected = useMemo(() => detectSource(form.url), [form.url]);

  const create = useMutation({
    mutationFn: api.createTask,
    onSuccess: (task) => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.tasks });
      void queryClient.invalidateQueries({ queryKey: queryKeys.system });
      toast.success("已创建传输任务", {
        description: `${task.filename} · ${task.bucket}`,
      });
      navigate(`/tasks/${task.id}`);
    },
    onError: (error) => {
      toast.error("创建任务失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      });
    },
  });

  const update = <K extends keyof HomeForm>(key: K, value: HomeForm[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
  };

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!form.url.trim()) {
      toast.error("请先粘贴下载链接");
      return;
    }
    if (!form.s3_config_id) {
      toast.error("请选择 S3 配置");
      return;
    }
    if (!form.bucket.trim()) {
      toast.error("请填写存储桶名称");
      return;
    }
    const input: CreateTaskInput = {
      url: form.url.trim(),
      source_type: form.source_type,
      s3_config_id: form.s3_config_id,
      bucket: form.bucket.trim(),
      object_key: form.object_key.trim() || undefined,
      filename: form.filename.trim() || undefined,
      user_agent: form.user_agent.trim() || undefined,
      referer: form.referer.trim() || undefined,
      cookie: form.cookie.trim() || undefined,
      auth_username: form.auth_username.trim() || undefined,
      auth_password: form.auth_password || undefined,
      headers: parseHeadersJson(form.headers_text),
      allow_overwrite: form.allow_overwrite,
    };
    create.mutate(input);
  };

  const config = configs?.find((item) => item.id === form.s3_config_id);
  const recent = tasks?.slice(0, 3) ?? [];

  return (
    <div className="space-y-7 animate-fade-up">
      <section className="relative overflow-hidden rounded-3xl border border-slate-200 bg-gradient-to-br from-white via-card/50 to-card/20 px-6 py-8 sm:px-9 sm:py-10">
        <div className="pointer-events-none absolute -right-20 -top-28 h-80 w-80 rounded-full bg-cyan-50 blur-3xl" />
        <div className="pointer-events-none absolute -bottom-36 left-1/3 h-80 w-80 rounded-full bg-indigo-200/30 blur-3xl" />
        <div className="relative flex flex-col justify-between gap-7 lg:flex-row lg:items-end">
          <div className="max-w-2xl">
            <h1 className="text-balance text-3xl font-semibold tracking-tight text-slate-900 sm:text-[40px] sm:leading-[1.08]">
              粘贴链接，立即流式传输。
              <br />
              <span className="gradient-text">直接写入 S3。</span>
            </h1>
          </div>
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4 lg:min-w-[370px]">
            {[
              { label: "进行中", value: system?.active_tasks ?? "—", icon: Gauge },
              { label: "排队中", value: system?.tasks.pending ?? "—", icon: DownloadCloud },
              { label: "已完成", value: system?.tasks.completed ?? "—", icon: CheckCircle2 },
              {
                label: "MEGA 临时空间",
                value: system ? formatBytes(system.mega_temp_bytes) : "—",
                icon: HardDrive,
              },
            ].map((stat) => (
              <div
                key={stat.label}
                className="rounded-xl border border-slate-200 bg-slate-50 px-3 py-3"
              >
                <stat.icon className="mb-2 h-3.5 w-3.5 text-cyan-600/80" />
                <div className="truncate font-mono text-lg font-semibold text-slate-900">
                  {stat.value}
                </div>
                <div className="text-[10px] uppercase tracking-wider text-slate-500">
                  {stat.label}
                </div>
              </div>
            ))}
          </div>
        </div>
      </section>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_330px]">
        <Card className="overflow-hidden">
          <div className="flex items-center justify-between border-b border-slate-200 px-5 py-4 sm:px-6">
            <div className="flex items-center gap-2.5">
              <div className="grid h-8 w-8 place-items-center rounded-lg bg-cyan-50 text-cyan-700">
                <Link2 className="h-4 w-4" />
              </div>
              <h2 className="text-sm font-semibold text-slate-900">新建传输任务</h2>
            </div>
            <Badge tone="cyan">{detected ? detected.toUpperCase() : "自动识别"}</Badge>
          </div>

          <form onSubmit={submit} className="space-y-6 p-5 sm:p-6">
            <div>
              <Field
                label="下载链接"
              >
                <div className="relative">
                  <Link2 className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                  <Input
                    value={form.url}
                    onChange={(event) => update("url", event.target.value)}
                    placeholder="https://example.com/archive.zip 或 https://mega.nz/file/..."
                    className="h-12 pl-10 pr-24 text-sm"
                    autoFocus
                    spellCheck={false}
                  />
                  <div className="absolute right-2 top-1/2 -translate-y-1/2">
                    {detected ? (
                      <SourceBadge source={detected} />
                    ) : (
                      <span className="rounded-full border border-slate-200 px-2 py-1 text-[10px] text-slate-500">
                        等待输入
                      </span>
                    )}
                  </div>
                </div>
              </Field>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <span className="text-[11px] text-slate-500">来源</span>
                {(["auto", "http", "mega"] as SourceType[]).map((type) => (
                  <button
                    key={type}
                    type="button"
                    onClick={() => update("source_type", type)}
                    className={cn(
                      "rounded-full border px-3 py-1 text-[11px] font-medium transition-colors",
                      form.source_type === type
                        ? "border-cyan-200 bg-cyan-50 text-cyan-700"
                        : "border-slate-200 bg-slate-50 text-slate-600 hover:text-slate-800",
                    )}
                  >
                    {type === "auto" ? "自动" : type.toUpperCase()}
                  </button>
                ))}
                {detected && form.source_type !== "auto" && detected !== form.source_type ? (
                  <span className="text-[11px] text-amber-700">
                    链接看起来是 {detected.toUpperCase()}
                  </span>
                ) : null}
              </div>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="S3 配置">
                <Select
                  value={form.s3_config_id}
                  onChange={(event) => {
                    const id = event.target.value;
                    const next = configs?.find((item) => item.id === id);
                    setForm((current) => ({
                      ...current,
                      s3_config_id: id,
                      bucket: next?.bucket || current.bucket,
                    }));
                  }}
                  disabled={configsLoading}
                >
                  <option value="" disabled>
                    {configsLoading ? "加载中…" : "请选择配置"}
                  </option>
                  {configs?.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name} {item.endpoint ? `· ${item.endpoint}` : "· AWS S3"}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="存储桶">
                <Input
                  value={form.bucket}
                  onChange={(event) => update("bucket", event.target.value)}
                  placeholder={config?.bucket || "downloads"}
                  list="bucket-options"
                />
                <datalist id="bucket-options">
                  {buckets?.map((bucket) => (
                    <option key={bucket.name} value={bucket.name} />
                  ))}
                </datalist>
              </Field>
              <Field
                label="对象路径"
              >
                <Input
                  value={form.object_key}
                  onChange={(event) => update("object_key", event.target.value)}
                  placeholder={config?.default_prefix ? `${config.default_prefix}/` : "archive/file.zip"}
                  spellCheck={false}
                />
              </Field>
              <Field label="自定义文件名">
                <Input
                  value={form.filename}
                  onChange={(event) => update("filename", event.target.value)}
                  placeholder="file.zip"
                  spellCheck={false}
                />
              </Field>
            </div>

            <button
              type="button"
              onClick={() => setAdvanced((value) => !value)}
              className="flex w-full items-center justify-between rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-left transition-colors hover:bg-slate-100"
            >
              <span className="flex items-center gap-2 text-xs font-medium text-slate-700">
                <KeyRound className="h-3.5 w-3.5 text-slate-500" />
                请求选项与凭据
              </span>
              <ChevronDown
                className={cn(
                  "h-4 w-4 text-slate-500 transition-transform",
                  advanced && "rotate-180",
                )}
              />
            </button>

            {advanced ? (
              <div className="grid gap-4 rounded-2xl border border-slate-200 bg-slate-50 p-4 sm:grid-cols-2">
                <Field label="User-Agent">
                  <Input
                    value={form.user_agent}
                    onChange={(event) => update("user_agent", event.target.value)}
                    placeholder="默认 FileUploader 客户端"
                  />
                </Field>
                <Field label="Referer">
                  <Input
                    value={form.referer}
                    onChange={(event) => update("referer", event.target.value)}
                    placeholder="https://example.com/"
                  />
                </Field>
                <Field label="Cookie">
                  <Input
                    value={form.cookie}
                    onChange={(event) => update("cookie", event.target.value)}
                    placeholder="session=…"
                  />
                </Field>
                <Field label="Basic Auth 用户名">
                  <Input
                    value={form.auth_username}
                    onChange={(event) => update("auth_username", event.target.value)}
                    placeholder="用户名"
                    autoComplete="off"
                  />
                </Field>
                <Field label="Basic Auth 密码">
                  <Input
                    type="password"
                    value={form.auth_password}
                    onChange={(event) => update("auth_password", event.target.value)}
                    placeholder="••••••••"
                    autoComplete="new-password"
                  />
                </Field>
                <Field label="自定义请求头">
                  <Textarea
                    value={form.headers_text}
                    onChange={(event) => update("headers_text", event.target.value)}
                    placeholder={"X-Api-Key: …\nAccept-Language: en-US"}
                    className="min-h-[96px] font-mono text-xs"
                    spellCheck={false}
                  />
                </Field>
                <div className="sm:col-span-2 flex items-center justify-between rounded-xl border border-amber-200 bg-amber-50 px-4 py-3">
                  <div className="text-xs font-medium text-amber-800">允许覆盖已有对象</div>
                  <Switch
                    checked={form.allow_overwrite}
                    onCheckedChange={(value) => update("allow_overwrite", value)}
                    label="允许覆盖"
                  />
                </div>
              </div>
            ) : null}

            {configs && configs.length === 0 ? (
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-800">
                <span className="inline-flex items-center gap-2">
                  <ServerCog className="h-4 w-4" />
                  尚未配置 S3
                </span>
                <Link to="/settings" className="font-medium text-amber-700 underline-offset-4 hover:underline">
                  前往设置
                </Link>
              </div>
            ) : null}

            <div className="flex flex-wrap items-center justify-end gap-3 border-t border-slate-200 pt-5">
              <Button
                type="submit"
                size="lg"
                disabled={create.isPending || !form.url.trim() || !configs?.length}
                className="min-w-[180px]"
              >
                {create.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <ArrowUpFromLine className="h-4 w-4" />
                )}
                {create.isPending ? "正在启动…" : "开始传输"}
                {!create.isPending ? <ArrowRight className="h-4 w-4" /> : null}
              </Button>
            </div>
          </form>
        </Card>

        <div className="space-y-5">
          <Card className="p-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
                <Gauge className="h-4 w-4 text-cyan-700" />
                运行状态
              </div>
              <Link to="/settings" className="text-[11px] text-cyan-700 hover:text-cyan-700">
                配置
              </Link>
            </div>
            <div className="mt-4 grid grid-cols-2 gap-3 text-xs">
              <RuntimeStat label="HTTP 槽位" value={system?.settings.max_http_tasks ?? "—"} />
              <RuntimeStat label="MEGA 槽位" value={system?.settings.max_mega_tasks ?? "—"} />
              <RuntimeStat
                label="分片大小"
                value={system ? formatBytes(system.settings.part_size_bytes, 0) : "—"}
              />
              <RuntimeStat label="上传并发" value={system?.settings.upload_concurrency ?? "—"} />
            </div>
            <div className="mt-4 flex items-center justify-between rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 text-[11px]">
              <span className="text-slate-500">MEGAcmd 服务</span>
              <span
                className={cn(
                  "inline-flex items-center gap-1.5",
                  system?.mega.server_running ? "text-emerald-700" : "text-slate-500",
                )}
              >
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    system?.mega.server_running ? "bg-emerald-500" : "bg-slate-300",
                  )}
                />
                {system?.mega.server_running
                  ? system.mega.logged_in
                    ? system.mega.account ?? "会话已激活"
                    : "可用公共链接"
                  : "未启动"}
              </span>
            </div>
          </Card>

          <Card className="p-5">
            <div className="flex items-center justify-between">
              <div className="text-sm font-semibold text-slate-900">最近活动</div>
              <Link to="/tasks" className="text-[11px] text-cyan-700 hover:text-cyan-700">
                查看全部
              </Link>
            </div>
            <div className="mt-3 space-y-2">
              {recent.length === 0 ? (
                <div className="rounded-xl border border-dashed border-slate-200 px-4 py-6 text-center text-[11px] text-slate-500">
                  暂无传输记录
                </div>
              ) : (
                recent.map((task) => (
                  <Link
                    key={task.id}
                    to={`/tasks/${task.id}`}
                    className="block rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 transition-colors hover:border-slate-300 hover:bg-slate-100"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-xs text-slate-800">{task.filename}</span>
                      <StatusBadge status={task.status} />
                    </div>
                    <div className="mt-1 flex items-center justify-between text-[10px] text-slate-500">
                      <span className="font-mono">{shortId(task.id)}</span>
                      <span>
                        {statusLabel[task.status]} · {timeAgo(task.created_at)}
                      </span>
                    </div>
                    <div className="mt-2">
                      <TaskProgress task={task} compact />
                    </div>
                  </Link>
                ))
              )}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

function RuntimeStat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2">
      <div className="text-[10px] uppercase tracking-wider text-slate-600">{label}</div>
      <div className="mt-0.5 font-mono text-slate-800">{value}</div>
    </div>
  );
}
