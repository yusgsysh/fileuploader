import { useEffect, useMemo, useState } from "react";
import type React from "react";
import { Link, useNavigate } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  ArrowUpFromLine,
  CheckCircle2,
  ChevronDown,
  Cloud,
  DatabaseZap,
  DownloadCloud,
  FolderTree,
  Gauge,
  HardDrive,
  KeyRound,
  Link2,
  Loader2,
  ServerCog,
  ShieldCheck,
  Sparkles,
  Wand2,
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
      toast.success("Transfer queued", {
        description: `${task.filename} · ${task.bucket}`,
      });
      navigate(`/tasks/${task.id}`);
    },
    onError: (error) => {
      toast.error("Could not create task", {
        description: error instanceof ApiError ? error.message : "Request failed",
      });
    },
  });

  const update = <K extends keyof HomeForm>(key: K, value: HomeForm[K]) => {
    setForm((current) => ({ ...current, [key]: value }));
  };

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!form.url.trim()) {
      toast.error("Paste a download URL first");
      return;
    }
    if (!form.s3_config_id) {
      toast.error("Choose an S3 configuration");
      return;
    }
    if (!form.bucket.trim()) {
      toast.error("Bucket is required");
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
      <section className="relative overflow-hidden rounded-3xl border border-white/[0.07] bg-gradient-to-br from-white/[0.045] via-card/50 to-card/20 px-6 py-8 sm:px-9 sm:py-10">
        <div className="pointer-events-none absolute -right-20 -top-28 h-80 w-80 rounded-full bg-cyan-400/10 blur-3xl" />
        <div className="pointer-events-none absolute -bottom-36 left-1/3 h-80 w-80 rounded-full bg-indigo-500/10 blur-3xl" />
        <div className="relative flex flex-col justify-between gap-7 lg:flex-row lg:items-end">
          <div className="max-w-2xl">
            <div className="mb-4 inline-flex items-center gap-2 rounded-full border border-cyan-400/20 bg-cyan-400/[0.07] px-3 py-1 text-[11px] text-cyan-200">
              <Sparkles className="h-3.5 w-3.5" />
              Streaming-first transfer engine
            </div>
            <h1 className="text-balance text-3xl font-semibold tracking-tight text-white sm:text-[40px] sm:leading-[1.08]">
              Pull a link. Stream it.
              <br />
              <span className="gradient-text">Store it in S3.</span>
            </h1>
            <p className="mt-4 max-w-xl text-sm leading-relaxed text-slate-400">
              HTTP downloads never touch local disk. MEGA links go through official{" "}
              <code className="rounded bg-white/[0.06] px-1 py-0.5 text-[11px] text-slate-300">mega-get</code>{" "}
              and upload from an isolated temp directory. Both share one bounded multipart engine.
            </p>
          </div>
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-4 lg:min-w-[370px]">
            {[
              { label: "Active", value: system?.active_tasks ?? "—", icon: Gauge },
              { label: "Queued", value: system?.tasks.pending ?? "—", icon: DownloadCloud },
              { label: "Completed", value: system?.tasks.completed ?? "—", icon: CheckCircle2 },
              {
                label: "MEGA temp",
                value: system ? formatBytes(system.mega_temp_bytes) : "—",
                icon: HardDrive,
              },
            ].map((stat) => (
              <div
                key={stat.label}
                className="rounded-xl border border-white/[0.06] bg-black/15 px-3 py-3"
              >
                <stat.icon className="mb-2 h-3.5 w-3.5 text-cyan-400/80" />
                <div className="truncate font-mono text-lg font-semibold text-slate-100">
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
          <div className="flex items-center justify-between border-b border-white/[0.06] px-5 py-4 sm:px-6">
            <div className="flex items-center gap-2.5">
              <div className="grid h-8 w-8 place-items-center rounded-lg bg-cyan-400/10 text-cyan-300">
                <Link2 className="h-4 w-4" />
              </div>
              <div>
                <h2 className="text-sm font-semibold text-slate-100">New transfer</h2>
                <p className="text-[11px] text-slate-500">
                  Source, destination and transfer policy
                </p>
              </div>
            </div>
            <Badge tone="cyan">{detected ? detected.toUpperCase() : "AUTO DETECT"}</Badge>
          </div>

          <form onSubmit={submit} className="space-y-6 p-5 sm:p-6">
            <div>
              <Field
                label="Download URL"
                hint={
                  <>
                    HTTP/HTTPS links are fetched with SSRF protection. MEGA share links use official
                    MEGAcmd. The full share key is preserved.
                  </>
                }
              >
                <div className="relative">
                  <Link2 className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                  <Input
                    value={form.url}
                    onChange={(event) => update("url", event.target.value)}
                    placeholder="https://example.com/archive.zip or https://mega.nz/file/..."
                    className="h-12 pl-10 pr-24 text-sm"
                    autoFocus
                    spellCheck={false}
                  />
                  <div className="absolute right-2 top-1/2 -translate-y-1/2">
                    {detected ? (
                      <SourceBadge source={detected} />
                    ) : (
                      <span className="rounded-full border border-white/10 px-2 py-1 text-[10px] text-slate-500">
                        waiting
                      </span>
                    )}
                  </div>
                </div>
              </Field>
              <div className="mt-3 flex flex-wrap items-center gap-2">
                <span className="text-[11px] text-slate-500">Source</span>
                {(["auto", "http", "mega"] as SourceType[]).map((type) => (
                  <button
                    key={type}
                    type="button"
                    onClick={() => update("source_type", type)}
                    className={cn(
                      "rounded-full border px-3 py-1 text-[11px] font-medium transition-colors",
                      form.source_type === type
                        ? "border-cyan-400/30 bg-cyan-400/10 text-cyan-200"
                        : "border-white/10 bg-white/[0.02] text-slate-400 hover:text-slate-200",
                    )}
                  >
                    {type === "auto" ? "Auto" : type.toUpperCase()}
                  </button>
                ))}
                {detected && form.source_type !== "auto" && detected !== form.source_type ? (
                  <span className="text-[11px] text-amber-300">
                    URL looks like {detected.toUpperCase()}
                  </span>
                ) : null}
              </div>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <Field label="S3 configuration" hint="Credentials stay on the server.">
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
                    {configsLoading ? "Loading…" : "Select configuration"}
                  </option>
                  {configs?.map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name} {item.endpoint ? `· ${item.endpoint}` : "· AWS S3"}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="Bucket" hint="Type any bucket or pick one visible to these credentials.">
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
                label="Object path"
                hint="Optional. End with / to append the detected filename. For MEGA folders this is a prefix."
              >
                <Input
                  value={form.object_key}
                  onChange={(event) => update("object_key", event.target.value)}
                  placeholder={config?.default_prefix ? `${config.default_prefix}/` : "archive/file.zip"}
                  spellCheck={false}
                />
              </Field>
              <Field label="Custom filename" hint="Optional. Content-Disposition is used otherwise.">
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
              className="flex w-full items-center justify-between rounded-xl border border-white/[0.06] bg-white/[0.02] px-4 py-3 text-left transition-colors hover:bg-white/[0.04]"
            >
              <span className="flex items-center gap-2 text-xs font-medium text-slate-300">
                <KeyRound className="h-3.5 w-3.5 text-slate-500" />
                Request options & credentials
                <span className="text-[10px] font-normal text-slate-600">
                  User-Agent, cookies, headers, Basic Auth
                </span>
              </span>
              <ChevronDown
                className={cn(
                  "h-4 w-4 text-slate-500 transition-transform",
                  advanced && "rotate-180",
                )}
              />
            </button>

            {advanced ? (
              <div className="grid gap-4 rounded-2xl border border-white/[0.06] bg-black/15 p-4 sm:grid-cols-2">
                <Field label="User-Agent">
                  <Input
                    value={form.user_agent}
                    onChange={(event) => update("user_agent", event.target.value)}
                    placeholder="Default FileUploader agent"
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
                <Field label="Basic Auth username">
                  <Input
                    value={form.auth_username}
                    onChange={(event) => update("auth_username", event.target.value)}
                    placeholder="username"
                    autoComplete="off"
                  />
                </Field>
                <Field label="Basic Auth password">
                  <Input
                    type="password"
                    value={form.auth_password}
                    onChange={(event) => update("auth_password", event.target.value)}
                    placeholder="••••••••"
                    autoComplete="new-password"
                  />
                </Field>
                <Field label="Custom headers" hint="One per line: Name: value">
                  <Textarea
                    value={form.headers_text}
                    onChange={(event) => update("headers_text", event.target.value)}
                    placeholder={"X-Api-Key: …\nAccept-Language: en-US"}
                    className="min-h-[96px] font-mono text-xs"
                    spellCheck={false}
                  />
                </Field>
                <div className="sm:col-span-2 flex items-center justify-between rounded-xl border border-amber-400/15 bg-amber-400/[0.04] px-4 py-3">
                  <div>
                    <div className="text-xs font-medium text-amber-100">Allow object overwrite</div>
                    <p className="mt-0.5 text-[11px] text-amber-200/50">
                      Off by default. Retries of the same task may replace its own partial objects.
                    </p>
                  </div>
                  <Switch
                    checked={form.allow_overwrite}
                    onCheckedChange={(value) => update("allow_overwrite", value)}
                    label="Allow overwrite"
                  />
                </div>
              </div>
            ) : null}

            {configs && configs.length === 0 ? (
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-amber-400/20 bg-amber-400/[0.05] px-4 py-3 text-xs text-amber-100">
                <span className="inline-flex items-center gap-2">
                  <ServerCog className="h-4 w-4" />
                  No S3 configuration yet. Add R2, MinIO or AWS S3 credentials in Settings.
                </span>
                <Link to="/settings" className="font-medium text-amber-200 underline-offset-4 hover:underline">
                  Open settings
                </Link>
              </div>
            ) : null}

            <div className="flex flex-wrap items-center justify-between gap-3 border-t border-white/[0.06] pt-5">
              <div className="flex items-center gap-2 text-[11px] text-slate-500">
                <ShieldCheck className="h-3.5 w-3.5 text-emerald-400/70" />
                Private IPs and metadata endpoints are blocked on every redirect.
              </div>
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
                {create.isPending ? "Starting…" : "Start transfer"}
                {!create.isPending ? <ArrowRight className="h-4 w-4" /> : null}
              </Button>
            </div>
          </form>
        </Card>

        <div className="space-y-5">
          <Card className="p-5">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
              <Wand2 className="h-4 w-4 text-cyan-300" />
              Transfer pipeline
            </div>
            <div className="mt-4 space-y-4">
              <PipelineStep
                icon={Cloud}
                title="HTTP / HTTPS"
                detail="Reqwest stream → fixed buffer → bounded queue. Zero local file."
                tone="cyan"
              />
              <PipelineStep
                icon={FolderTree}
                title="MEGA share"
                detail="Official mega-get → task-scoped temp dir → async file stream."
                tone="violet"
              />
              <PipelineStep
                icon={DatabaseZap}
                title="S3 multipart"
                detail="16 MiB parts, bounded concurrency, aborts on cancel/failure."
                tone="emerald"
              />
            </div>
          </Card>

          <Card className="p-5">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
                <Gauge className="h-4 w-4 text-cyan-300" />
                Runtime
              </div>
              <Link to="/settings" className="text-[11px] text-cyan-300 hover:text-cyan-200">
                Configure
              </Link>
            </div>
            <div className="mt-4 grid grid-cols-2 gap-3 text-xs">
              <RuntimeStat label="HTTP slots" value={system?.settings.max_http_tasks ?? "—"} />
              <RuntimeStat label="MEGA slots" value={system?.settings.max_mega_tasks ?? "—"} />
              <RuntimeStat
                label="Part size"
                value={system ? formatBytes(system.settings.part_size_bytes, 0) : "—"}
              />
              <RuntimeStat label="Upload lanes" value={system?.settings.upload_concurrency ?? "—"} />
            </div>
            <div className="mt-4 flex items-center justify-between rounded-xl border border-white/[0.06] bg-black/15 px-3 py-2.5 text-[11px]">
              <span className="text-slate-500">MEGAcmd server</span>
              <span
                className={cn(
                  "inline-flex items-center gap-1.5",
                  system?.mega.server_running ? "text-emerald-300" : "text-slate-500",
                )}
              >
                <span
                  className={cn(
                    "h-1.5 w-1.5 rounded-full",
                    system?.mega.server_running ? "bg-emerald-400" : "bg-slate-600",
                  )}
                />
                {system?.mega.server_running
                  ? system.mega.logged_in
                    ? system.mega.account ?? "session active"
                    : "public links ready"
                  : "not running"}
              </span>
            </div>
          </Card>

          <Card className="p-5">
            <div className="flex items-center justify-between">
              <div className="text-sm font-semibold text-slate-100">Recent activity</div>
              <Link to="/tasks" className="text-[11px] text-cyan-300 hover:text-cyan-200">
                View all
              </Link>
            </div>
            <div className="mt-3 space-y-2">
              {recent.length === 0 ? (
                <div className="rounded-xl border border-dashed border-white/10 px-4 py-6 text-center text-[11px] text-slate-500">
                  No transfers yet. Your first task will appear here.
                </div>
              ) : (
                recent.map((task) => (
                  <Link
                    key={task.id}
                    to={`/tasks/${task.id}`}
                    className="block rounded-xl border border-white/[0.05] bg-white/[0.02] px-3 py-2.5 transition-colors hover:border-white/[0.1] hover:bg-white/[0.04]"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-xs text-slate-200">{task.filename}</span>
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

function PipelineStep({
  icon: Icon,
  title,
  detail,
  tone,
}: {
  icon: typeof Cloud;
  title: string;
  detail: string;
  tone: "cyan" | "violet" | "emerald";
}) {
  const tones = {
    cyan: "bg-cyan-400/10 text-cyan-300",
    violet: "bg-violet-400/10 text-violet-300",
    emerald: "bg-emerald-400/10 text-emerald-300",
  };
  return (
    <div className="flex gap-3">
      <div className={cn("grid h-8 w-8 shrink-0 place-items-center rounded-lg", tones[tone])}>
        <Icon className="h-4 w-4" />
      </div>
      <div>
        <div className="text-xs font-medium text-slate-200">{title}</div>
        <p className="mt-0.5 text-[11px] leading-relaxed text-slate-500">{detail}</p>
      </div>
    </div>
  );
}

function RuntimeStat({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-white/[0.05] bg-black/15 px-3 py-2">
      <div className="text-[10px] uppercase tracking-wider text-slate-600">{label}</div>
      <div className="mt-0.5 font-mono text-slate-200">{value}</div>
    </div>
  );
}
