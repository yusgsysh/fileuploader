import { useEffect, useState } from "react";
import type React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  Boxes,
  Check,
  CheckCircle2,
  Cloud,
  CloudCog,
  Cpu,
  Database,
  Gauge,
  HardDrive,
  KeyRound,
  Loader2,
  Pencil,
  Plus,
  RefreshCw,
  Save,
  ServerCog,
  Shield,
  Trash2,
  X,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, queryKeys } from "@/lib/api";
import type {
  AppSettings,
  MegaCheck,
  S3Config,
  S3ConfigInput,
  S3TestResult,
} from "@/types";
import {
  cn,
  formatBytes,
  formatDate,
  parseHeadersJson,
} from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Field, Input, Textarea } from "@/components/ui/input";
import { Badge, Switch } from "@/components/ui/primitives";

const MiB = 1024 * 1024;
const GiB = 1024 * MiB;

type Tab = "transfer" | "mega" | "s3";

export function SettingsPage() {
  const [tab, setTab] = useState<Tab>("transfer");
  const settingsState = useSettings();
  return (
    <div className="space-y-6 animate-fade-up">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="mb-1 text-[11px] uppercase tracking-[0.2em] text-cyan-300/70">
            System configuration
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-white sm:text-3xl">Settings</h1>
          <p className="mt-1 text-sm text-slate-400">
            Transfer limits, MEGAcmd paths and S3-compatible destinations. Secrets are never
            returned by the API.
          </p>
        </div>
        <Badge tone="cyan" className="px-3 py-1">
          <Shield className="h-3 w-3" />
          server-side credentials
        </Badge>
      </div>

      <div className="flex flex-wrap gap-1.5 rounded-xl border border-white/[0.06] bg-white/[0.02] p-1">
        {(
          [
            ["transfer", "Transfer engine", Gauge],
            ["mega", "MEGA / MEGAcmd", Cloud],
            ["s3", "S3 & R2", Database],
          ] as const
        ).map(([key, label, Icon]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-medium transition-colors",
              tab === key
                ? "bg-white/[0.08] text-slate-100 shadow-sm"
                : "text-slate-500 hover:text-slate-300",
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            {label}
          </button>
        ))}
      </div>

      {tab === "transfer" ? <TransferSettings state={settingsState} /> : null}
      {tab === "mega" ? <MegaSettings state={settingsState} /> : null}
      {tab === "s3" ? <S3Settings /> : null}
    </div>
  );
}

function useSettings() {
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: queryKeys.settings, queryFn: api.getSettings });
  const [settings, setSettings] = useState<AppSettings | null>(null);

  useEffect(() => {
    if (query.data) setSettings(query.data);
  }, [query.data]);

  const save = useMutation({
    mutationFn: api.updateSettings,
    onSuccess: (value) => {
      setSettings(value);
      queryClient.setQueryData(queryKeys.settings, value);
      void queryClient.invalidateQueries({ queryKey: queryKeys.system });
      toast.success("Settings saved");
    },
    onError: (error) =>
      toast.error("Could not save settings", {
        description: error instanceof ApiError ? error.message : "Request failed",
      }),
  });

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSettings((current) => (current ? { ...current, [key]: value } : current));
  };

  return { settings, update, save, isLoading: query.isLoading };
}

type SettingsState = ReturnType<typeof useSettings>;

function TransferSettings({ state }: { state: SettingsState }) {
  const { settings, update, save, isLoading } = state;
  if (isLoading || !settings) return <SettingsSkeleton />;

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_320px]">
      <div className="space-y-5 pb-28">
        <SettingsSection
          icon={Boxes}
          title="Multipart upload"
          description="Part size and upload lanes define memory pressure and request parallelism."
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="Part size"
              suffix="MiB"
              value={settings.part_size_bytes / MiB}
              min={5}
              max={5120}
              step={1}
              onChange={(value) => update("part_size_bytes", Math.round(value * MiB))}
              hint="S3 minimum is 5 MiB. 16 MiB is a balanced default."
            />
            <NumberField
              label="Upload concurrency"
              value={settings.upload_concurrency}
              min={1}
              max={16}
              onChange={(value) => update("upload_concurrency", value)}
              hint="Maximum in-flight UploadPart requests per task."
            />
            <NumberField
              label="Maximum pending parts"
              value={settings.max_pending_parts}
              min={1}
              max={32}
              onChange={(value) => update("max_pending_parts", value)}
              hint="Bounded queue between downloader and uploader."
            />
            <NumberField
              label="HTTP buffer"
              suffix="KiB"
              value={settings.http_buffer_bytes / 1024}
              min={64}
              max={16384}
              step={64}
              onChange={(value) => update("http_buffer_bytes", Math.round(value * 1024))}
              hint="Chunk processing window for the HTTP stream."
            />
          </div>
          <div className="rounded-xl border border-cyan-400/10 bg-cyan-400/[0.035] px-4 py-3 text-[11px] leading-relaxed text-cyan-100/60">
            Approximate worst-case streaming buffer:{" "}
            <span className="font-mono text-cyan-200">
              {formatBytes(
                settings.part_size_bytes *
                  (settings.max_pending_parts + settings.upload_concurrency + 2),
              )}
            </span>{" "}
            plus SDK/TLS overhead. Memory is bounded regardless of file size.
          </div>
        </SettingsSection>

        <SettingsSection
          icon={Gauge}
          title="Scheduling & resilience"
          description="Concurrency gates, retry budgets and timeout guardrails."
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="HTTP tasks"
              value={settings.max_http_tasks}
              min={1}
              max={32}
              onChange={(value) => update("max_http_tasks", value)}
            />
            <NumberField
              label="MEGA tasks"
              value={settings.max_mega_tasks}
              min={1}
              max={16}
              onChange={(value) => update("max_mega_tasks", value)}
            />
            <NumberField
              label="Queued task limit"
              value={settings.max_queued_tasks}
              min={1}
              max={1000}
              onChange={(value) => update("max_queued_tasks", value)}
              hint="New tasks are rejected when the queue is full."
            />
            <NumberField
              label="Retry attempts"
              value={settings.max_retries}
              min={0}
              max={10}
              onChange={(value) => update("max_retries", value)}
            />
            <NumberField
              label="HTTP timeout"
              suffix="sec"
              value={settings.http_timeout_secs}
              min={1}
              max={3600}
              onChange={(value) => update("http_timeout_secs", value)}
              hint="Connect and per-chunk idle timeout."
            />
            <NumberField
              label="Maximum redirects"
              value={settings.max_redirects}
              min={0}
              max={20}
              onChange={(value) => update("max_redirects", value)}
            />
            <NumberField
              label="Maximum file size"
              suffix="GiB"
              value={settings.max_file_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("max_file_bytes", Math.round(value * GiB))}
              hint="0 disables the limit. Applies to HTTP and MEGA totals."
            />
            <NumberField
              label="Task runtime limit"
              suffix="hours"
              value={settings.task_timeout_secs / 3600}
              min={1}
              max={720}
              step={0.5}
              onChange={(value) => update("task_timeout_secs", Math.round(value * 3600))}
            />
          </div>
        </SettingsSection>

        <SaveBar pending={save.isPending} onClick={() => save.mutate(settings)} />
      </div>

      <aside className="space-y-5">
        <InfoCard
          icon={Cpu}
          title="Backpressure model"
          items={[
            "HTTP stream → fixed part buffer",
            "Bounded mpsc queue (pending parts)",
            "Finite UploadPart worker lanes",
            "Multipart abort on failure/cancel",
          ]}
        />
        <InfoCard
          icon={HardDrive}
          title="Memory envelope"
          items={[
            `Part buffer: ${formatBytes(settings.part_size_bytes)}`,
            `Pending queue: ${settings.max_pending_parts} parts`,
            `Upload lanes: ${settings.upload_concurrency}`,
            `HTTP concurrency: ${settings.max_http_tasks} tasks`,
          ]}
        />
        <div className="rounded-2xl border border-amber-400/15 bg-amber-400/[0.04] p-4 text-[11px] leading-relaxed text-amber-100/70">
          <div className="mb-1 flex items-center gap-2 font-medium text-amber-200">
            <AlertTriangle className="h-3.5 w-3.5" />
            Changing limits
          </div>
          New values apply to newly scheduled work immediately. Running transfers keep their
          existing part size for the current attempt.
        </div>
      </aside>
    </div>
  );
}

function MegaSettings({ state }: { state: SettingsState }) {
  const { settings, update, save, isLoading } = state;
  const queryClient = useQueryClient();
  const { data: status, refetch, isFetching } = useQuery({
    queryKey: ["mega-status"],
    queryFn: api.megaStatus,
    refetchInterval: 20_000,
  });
  const [checks, setChecks] = useState<MegaCheck[] | null>(null);

  const test = useMutation({
    mutationFn: () => api.megaTest(),
    onSuccess: (result) => {
      setChecks(result.checks);
      toast[result.ok ? "success" : "warning"](
        result.ok ? "MEGAcmd checks passed" : "MEGAcmd needs attention",
      );
      void queryClient.invalidateQueries({ queryKey: ["mega-status"] });
    },
    onError: (error) =>
      toast.error("MEGA test failed", {
        description: error instanceof ApiError ? error.message : "Request failed",
      }),
  });

  if (isLoading || !settings) return <SettingsSkeleton />;

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
      <div className="space-y-5 pb-28">
        <SettingsSection
          icon={ServerCog}
          title="MEGAcmd paths"
          description="Use official MEGAcmd binaries. Commands are executed without a shell."
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="mega-get path" hint="Absolute path or binary on PATH.">
              <Input
                value={settings.mega_get_path}
                onChange={(event) => update("mega_get_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="MEGAcmd server path">
              <Input
                value={settings.mega_server_path}
                onChange={(event) => update("mega_server_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="mega-whoami path" hint="Used for session and server checks.">
              <Input
                value={settings.mega_whoami_path}
                onChange={(event) => update("mega_whoami_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="MEGAcmd HOME" hint="Persisted session lives under HOME/.megaCmd.">
              <Input
                value={settings.mega_home}
                onChange={(event) => update("mega_home", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="Temporary download directory" className="sm:col-span-2">
              <Input
                value={settings.mega_temp_dir}
                onChange={(event) => update("mega_temp_dir", event.target.value)}
                spellCheck={false}
              />
            </Field>
          </div>
          <div className="flex items-center justify-between rounded-xl border border-white/[0.06] bg-black/15 px-4 py-3">
            <div>
              <div className="text-xs font-medium text-slate-200">Auto-start MEGAcmd server</div>
              <p className="mt-0.5 text-[11px] text-slate-500">
                Commands may start the server on demand. Docker can manage it independently.
              </p>
            </div>
            <Switch
              checked={settings.mega_auto_start_server}
              onCheckedChange={(value) => update("mega_auto_start_server", value)}
            />
          </div>
        </SettingsSection>

        <SettingsSection
          icon={HardDrive}
          title="MEGA disk & concurrency"
          description="Each task receives an isolated directory. Failed uploads keep files for retry."
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="MEGA download concurrency"
              value={settings.mega_max_downloads}
              min={1}
              max={16}
              onChange={(value) => update("mega_max_downloads", value)}
            />
            <NumberField
              label="Per-task disk limit"
              suffix="GiB"
              value={settings.mega_max_task_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("mega_max_task_bytes", Math.round(value * GiB))}
              hint="0 disables the limit."
            />
            <NumberField
              label="Total temp storage limit"
              suffix="GiB"
              value={settings.mega_temp_max_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("mega_temp_max_bytes", Math.round(value * GiB))}
            />
            <NumberField
              label="Retention"
              suffix="hours"
              value={settings.mega_retention_secs / 3600}
              min={0}
              max={2160}
              step={1}
              onChange={(value) => update("mega_retention_secs", Math.round(value * 3600))}
              hint="Failed task temp files are cleaned after startup retention checks."
            />
            <NumberField
              label="mega-get timeout"
              suffix="hours"
              value={settings.mega_download_timeout_secs / 3600}
              min={1}
              max={168}
              step={0.5}
              onChange={(value) =>
                update("mega_download_timeout_secs", Math.round(value * 3600))
              }
            />
          </div>
        </SettingsSection>

        <SaveBar pending={save.isPending} onClick={() => save.mutate(settings)} />
      </div>

      <aside className="space-y-5">
        <div className="panel p-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
              <CloudCog className="h-4 w-4 text-violet-300" />
              MEGAcmd status
            </div>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => void refetch()}
              disabled={isFetching}
            >
              <RefreshCw className={cn("h-3.5 w-3.5", isFetching && "animate-spin")} />
            </Button>
          </div>
          <div className="mt-4 space-y-3 text-xs">
            <StatusLine label="Installed" ok={status?.installed} value={status?.version} />
            <StatusLine label="Server" ok={status?.server_running} value={status?.message} />
            <StatusLine
              label="Session"
              ok={status?.logged_in}
              value={status?.logged_in ? status.account : "public links only"}
              neutral={!status?.logged_in}
            />
            <div className="rounded-xl border border-white/[0.06] bg-black/20 p-3 text-[10px] leading-relaxed text-slate-500">
              <div className="font-mono text-slate-400">{settings.mega_get_path}</div>
              <div className="mt-1">HOME: {status?.home ?? settings.mega_home}</div>
            </div>
          </div>
          <Button
            variant="outline"
            size="sm"
            className="mt-4 w-full"
            onClick={() => test.mutate()}
            disabled={test.isPending}
          >
            {test.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <CheckCircle2 className="h-3.5 w-3.5" />
            )}
            Run diagnostics
          </Button>
          {checks ? (
            <div className="mt-4 space-y-2">
              {checks.map((check) => (
                <div
                  key={check.name}
                  className="rounded-lg border border-white/[0.05] bg-black/15 px-3 py-2"
                >
                  <div className="flex items-center gap-2 text-[11px] text-slate-300">
                    {check.ok ? (
                      <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400" />
                    ) : (
                      <XCircle className="h-3.5 w-3.5 text-rose-400" />
                    )}
                    {check.name}
                  </div>
                  <div className="mt-1 break-words text-[10px] text-slate-500">{check.detail}</div>
                </div>
              ))}
            </div>
          ) : null}
        </div>

        <div className="rounded-2xl border border-violet-400/15 bg-violet-400/[0.04] p-4 text-[11px] leading-relaxed text-violet-100/65">
          <div className="mb-1 flex items-center gap-2 font-medium text-violet-200">
            <KeyRound className="h-3.5 w-3.5" />
            Login & sessions
          </div>
          MEGA passwords are never stored by FileUploader. Log in once inside the container with{" "}
          <code className="rounded bg-black/30 px-1 py-0.5 text-[10px] text-violet-100">
            mega-login
          </code>
          . The session persists under the MEGAcmd HOME volume.
        </div>
      </aside>
    </div>
  );
}

interface ConfigFormState {
  id?: string;
  name: string;
  endpoint: string;
  region: string;
  access_key: string;
  secret_key: string;
  bucket: string;
  default_prefix: string;
  path_style: boolean;
  use_tls: boolean;
  metadata_text: string;
  secret_set: boolean;
}

const emptyConfig: ConfigFormState = {
  name: "",
  endpoint: "",
  region: "us-east-1",
  access_key: "",
  secret_key: "",
  bucket: "",
  default_prefix: "",
  path_style: false,
  use_tls: true,
  metadata_text: "",
  secret_set: false,
};

function S3Settings() {
  const queryClient = useQueryClient();
  const { data: configs, isLoading } = useQuery({
    queryKey: queryKeys.s3Configs,
    queryFn: api.listS3Configs,
  });
  const [form, setForm] = useState<ConfigFormState>(emptyConfig);
  const [testResult, setTestResult] = useState<S3TestResult | null>(null);

  const reset = () => {
    setForm(emptyConfig);
    setTestResult(null);
  };

  const edit = (config: S3Config) => {
    setForm({
      id: config.id,
      name: config.name,
      endpoint: config.endpoint,
      region: config.region,
      access_key: config.access_key,
      secret_key: "",
      bucket: config.bucket,
      default_prefix: config.default_prefix,
      path_style: config.path_style,
      use_tls: config.use_tls,
      metadata_text: Object.entries(config.metadata)
        .map(([key, value]) => `${key}: ${value}`)
        .join("\n"),
      secret_set: config.secret_set,
    });
    setTestResult(null);
    window.scrollTo({ top: 120, behavior: "smooth" });
  };

  const toInput = (): S3ConfigInput => ({
    name: form.name,
    endpoint: form.endpoint,
    region: form.region || "us-east-1",
    access_key: form.access_key,
    secret_key: form.secret_key || undefined,
    bucket: form.bucket,
    default_prefix: form.default_prefix,
    path_style: form.path_style,
    use_tls: form.use_tls,
    metadata: parseHeadersJson(form.metadata_text),
  });

  const save = useMutation({
    mutationFn: () =>
      form.id ? api.updateS3Config(form.id, toInput()) : api.createS3Config(toInput()),
    onSuccess: () => {
      toast.success(form.id ? "S3 configuration updated" : "S3 configuration created");
      void queryClient.invalidateQueries({ queryKey: queryKeys.s3Configs });
      reset();
    },
    onError: (error) =>
      toast.error("Could not save S3 configuration", {
        description: error instanceof ApiError ? error.message : "Request failed",
      }),
  });

  const remove = useMutation({
    mutationFn: api.deleteS3Config,
    onSuccess: () => {
      toast.success("S3 configuration deleted");
      void queryClient.invalidateQueries({ queryKey: queryKeys.s3Configs });
      if (form.id) reset();
    },
    onError: (error) =>
      toast.error("Could not delete configuration", {
        description: error instanceof ApiError ? error.message : "Request failed",
      }),
  });

  const test = useMutation({
    mutationFn: () => {
      if (!form.id && !form.secret_key) {
        throw new Error("Enter a secret key before testing a new configuration.");
      }
      return api.testS3({
        config_id: form.id,
        config: toInput(),
      });
    },
    onSuccess: (result) => {
      setTestResult(result);
      toast[result.ok ? "success" : "error"](result.message);
    },
    onError: (error) =>
      setTestResult({
        ok: false,
        message: error instanceof Error ? error.message : "Connection test failed",
        latency_ms: 0,
      }),
  });

  const applyPreset = (preset: "r2" | "minio" | "aws") => {
    setForm((current) => ({
      ...current,
      ...(preset === "r2"
        ? {
            name: current.name || "Cloudflare R2",
            endpoint: current.endpoint || "https://<ACCOUNT_ID>.r2.cloudflarestorage.com",
            region: "auto",
            path_style: true,
            use_tls: true,
          }
        : preset === "minio"
          ? {
              name: current.name || "MinIO",
              endpoint: current.endpoint || "https://minio.example.com",
              region: current.region === "auto" ? "us-east-1" : current.region,
              path_style: true,
              use_tls: true,
            }
          : {
              name: current.name || "AWS S3",
              endpoint: "",
              region: "us-east-1",
              path_style: false,
              use_tls: true,
            }),
    }));
    setTestResult(null);
  };

  return (
    <div className="space-y-6">
      <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_390px]">
        <div className="space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h2 className="text-sm font-semibold text-slate-100">Destinations</h2>
              <p className="text-[11px] text-slate-500">
                AWS S3, Cloudflare R2, MinIO, Backblaze B2, Wasabi and compatible APIs.
              </p>
            </div>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                reset();
                window.scrollTo({ top: 120, behavior: "smooth" });
              }}
            >
              <Plus className="h-3.5 w-3.5" />
              New
            </Button>
          </div>

          {isLoading ? (
            <div className="space-y-3">
              {Array.from({ length: 2 }).map((_, index) => (
                <div key={index} className="h-32 animate-pulse rounded-2xl bg-white/[0.03]" />
              ))}
            </div>
          ) : !configs?.length ? (
            <div className="rounded-2xl border border-dashed border-white/10 bg-white/[0.015] px-5 py-12 text-center">
              <Cloud className="mx-auto h-8 w-8 text-slate-600" />
              <div className="mt-3 text-sm text-slate-300">No S3 configuration yet</div>
              <p className="mt-1 text-xs text-slate-500">
                Add Cloudflare R2 credentials or any S3-compatible endpoint on the right.
              </p>
            </div>
          ) : (
            configs.map((config) => (
              <div
                key={config.id}
                className="rounded-2xl border border-white/[0.07] bg-card/60 p-4 transition-colors hover:border-white/[0.12]"
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="flex min-w-0 items-start gap-3">
                    <div className="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-white/[0.07] bg-white/[0.03]">
                      <Cloud className="h-4 w-4 text-cyan-300" />
                    </div>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm font-medium text-slate-100">
                          {config.name}
                        </span>
                        <Badge tone={config.secret_set ? "green" : "red"}>
                          {config.secret_set ? "credentials set" : "missing secret"}
                        </Badge>
                        {config.path_style ? <Badge>path style</Badge> : null}
                      </div>
                      <div className="mt-1 break-all font-mono text-[10px] text-slate-500">
                        {config.endpoint || "AWS S3 default endpoint"}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-slate-500">
                        <span>region {config.region}</span>
                        <span>bucket {config.bucket || "—"}</span>
                        <span>prefix {config.default_prefix || "—"}</span>
                        <span>updated {formatDate(config.updated_at)}</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button size="sm" variant="ghost" onClick={() => edit(config)}>
                      <Pencil className="h-3.5 w-3.5" />
                      Edit
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => {
                        setTestResult(null);
                        void api
                          .testS3({ config_id: config.id })
                          .then(setTestResult)
                          .catch((error: unknown) =>
                            setTestResult({
                              ok: false,
                              message:
                                error instanceof Error ? error.message : "Connection test failed",
                              latency_ms: 0,
                            }),
                          );
                      }}
                      title="Test saved credentials"
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-slate-500 hover:text-rose-300"
                      disabled={remove.isPending}
                      onClick={() => {
                        if (
                          window.confirm(
                            `Delete "${config.name}"? Configurations referenced by tasks cannot be deleted.`,
                          )
                        ) {
                          remove.mutate(config.id);
                        }
                      }}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                  </div>
                </div>
              </div>
            ))
          )}
          {testResult ? <TestResultCard result={testResult} /> : null}
        </div>

        <div className="space-y-4">
          <div className="panel p-5">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold text-slate-100">
                  {form.id ? "Edit configuration" : "New configuration"}
                </h2>
                <p className="mt-0.5 text-[11px] text-slate-500">
                  Secret keys are write-only from the UI.
                </p>
              </div>
              {form.id ? (
                <Button variant="ghost" size="icon" onClick={reset} title="Cancel editing">
                  <X className="h-4 w-4" />
                </Button>
              ) : null}
            </div>

            <div className="mt-4 flex flex-wrap gap-1.5">
              <PresetButton onClick={() => applyPreset("r2")} label="Cloudflare R2" tone="amber" />
              <PresetButton onClick={() => applyPreset("minio")} label="MinIO" tone="rose" />
              <PresetButton onClick={() => applyPreset("aws")} label="AWS S3" tone="cyan" />
            </div>

            <div className="mt-4 space-y-3.5">
              <Field label="Display name">
                <Input
                  value={form.name}
                  onChange={(event) => setForm({ ...form, name: event.target.value })}
                  placeholder="Cloudflare R2"
                />
              </Field>
              <Field
                label="Endpoint"
                hint="R2: https://<account-id>.r2.cloudflarestorage.com · leave empty for AWS."
              >
                <Input
                  value={form.endpoint}
                  onChange={(event) => setForm({ ...form, endpoint: event.target.value })}
                  placeholder="https://<account-id>.r2.cloudflarestorage.com"
                  spellCheck={false}
                />
              </Field>
              <div className="grid grid-cols-2 gap-3">
                <Field label="Region" hint="R2 uses auto.">
                  <Input
                    value={form.region}
                    onChange={(event) => setForm({ ...form, region: event.target.value })}
                    placeholder="auto"
                    spellCheck={false}
                  />
                </Field>
                <Field label="Default bucket">
                  <Input
                    value={form.bucket}
                    onChange={(event) => setForm({ ...form, bucket: event.target.value })}
                    placeholder="downloads"
                  />
                </Field>
              </div>
              <Field label="Access key ID">
                <Input
                  value={form.access_key}
                  onChange={(event) => setForm({ ...form, access_key: event.target.value })}
                  placeholder="Access Key ID"
                  spellCheck={false}
                  autoComplete="off"
                />
              </Field>
              <Field
                label="Secret access key"
                hint={
                  form.id && form.secret_set
                    ? "Leave blank to keep the existing secret."
                    : "Stored server-side and never returned."
                }
              >
                <Input
                  type="password"
                  value={form.secret_key}
                  onChange={(event) => setForm({ ...form, secret_key: event.target.value })}
                  placeholder={form.id && form.secret_set ? "•••••••• (unchanged)" : "Secret Access Key"}
                  autoComplete="new-password"
                />
              </Field>
              <Field label="Default object prefix">
                <Input
                  value={form.default_prefix}
                  onChange={(event) => setForm({ ...form, default_prefix: event.target.value })}
                  placeholder="downloads/archive"
                  spellCheck={false}
                />
              </Field>
              <Field label="Custom metadata" hint="One per line: key: value">
                <Textarea
                  value={form.metadata_text}
                  onChange={(event) => setForm({ ...form, metadata_text: event.target.value })}
                  placeholder={"owner: ops\nenvironment: production"}
                  className="min-h-[76px] font-mono text-xs"
                  spellCheck={false}
                />
              </Field>
              <div className="grid grid-cols-2 gap-2">
                <ToggleRow
                  label="Path-style URL"
                  checked={form.path_style}
                  onChange={(value) => setForm({ ...form, path_style: value })}
                />
                <ToggleRow
                  label="TLS"
                  checked={form.use_tls}
                  onChange={(value) => setForm({ ...form, use_tls: value })}
                />
              </div>
            </div>

            <div className="mt-5 flex flex-wrap gap-2">
              <Button
                variant="outline"
                className="flex-1"
                onClick={() => test.mutate()}
                disabled={test.isPending || save.isPending}
              >
                {test.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <RefreshCw className="h-4 w-4" />
                )}
                Test
              </Button>
              <Button
                className="flex-1"
                onClick={() => save.mutate()}
                disabled={save.isPending || test.isPending}
              >
                {save.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                {form.id ? "Update" : "Create"}
              </Button>
            </div>
          </div>

          <div className="rounded-2xl border border-amber-400/15 bg-amber-400/[0.04] p-4 text-[11px] leading-relaxed text-amber-100/70">
            <div className="mb-1 flex items-center gap-2 font-medium text-amber-200">
              <AlertTriangle className="h-3.5 w-3.5" />
              Cloudflare R2 specifics
            </div>
            Use region <code className="rounded bg-black/25 px-1">auto</code>, enable path-style
            access, and create an S3 API token from the R2 dashboard. The endpoint is not the public
            r2.dev development URL.
          </div>
        </div>
      </div>
    </div>
  );
}

function TestResultCard({ result }: { result: S3TestResult }) {
  return (
    <div
      className={cn(
        "flex items-start gap-3 rounded-xl border px-4 py-3 text-xs",
        result.ok
          ? "border-emerald-400/20 bg-emerald-400/[0.05] text-emerald-100"
          : "border-rose-400/20 bg-rose-400/[0.05] text-rose-100",
      )}
    >
      {result.ok ? (
        <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-400" />
      ) : (
        <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-rose-400" />
      )}
      <div>
        <div className="font-medium">{result.ok ? "Connection successful" : "Connection failed"}</div>
        <div className="mt-0.5 break-words text-[11px] opacity-75">{result.message}</div>
        {result.ok ? (
          <div className="mt-1 font-mono text-[10px] opacity-60">{result.latency_ms} ms</div>
        ) : null}
      </div>
    </div>
  );
}

function SettingsSection({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: typeof Boxes;
  title: string;
  description: string;
  children: React.ReactNode;
}) {
  return (
    <section className="panel p-5 sm:p-6">
      <div className="mb-5 flex items-start gap-3">
        <div className="grid h-9 w-9 place-items-center rounded-xl border border-white/[0.06] bg-white/[0.03]">
          <Icon className="h-4 w-4 text-cyan-300" />
        </div>
        <div>
          <h2 className="text-sm font-semibold text-slate-100">{title}</h2>
          <p className="mt-0.5 text-[11px] text-slate-500">{description}</p>
        </div>
      </div>
      {children}
    </section>
  );
}

function NumberField({
  label,
  value,
  onChange,
  min,
  max,
  step = 1,
  suffix,
  hint,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
  hint?: string;
}) {
  return (
    <Field label={label} hint={hint}>
      <div className="relative">
        <Input
          type="number"
          value={Number.isFinite(value) ? value : 0}
          min={min}
          max={max}
          step={step}
          onChange={(event) => onChange(Number(event.target.value))}
          className={cn("font-mono", suffix && "pr-14")}
        />
        {suffix ? (
          <span className="pointer-events-none absolute right-3.5 top-1/2 -translate-y-1/2 text-[10px] uppercase tracking-wider text-slate-600">
            {suffix}
          </span>
        ) : null}
      </div>
    </Field>
  );
}

function SaveBar({ pending, onClick }: { pending: boolean; onClick: () => void }) {
  return (
    <div className="sticky bottom-4 z-20 flex items-center justify-between gap-3 rounded-2xl border border-white/[0.08] bg-background/90 px-4 py-3 shadow-panel backdrop-blur-xl">
      <div className="text-[11px] text-slate-500">
        Values are validated by the server before they are persisted.
      </div>
      <Button onClick={onClick} disabled={pending}>
        {pending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
        Save changes
      </Button>
    </div>
  );
}

function InfoCard({
  icon: Icon,
  title,
  items,
}: {
  icon: typeof Cpu;
  title: string;
  items: string[];
}) {
  return (
    <div className="panel p-5">
      <div className="flex items-center gap-2 text-sm font-semibold text-slate-100">
        <Icon className="h-4 w-4 text-cyan-300" />
        {title}
      </div>
      <ul className="mt-3 space-y-2">
        {items.map((item) => (
          <li key={item} className="flex items-start gap-2 text-[11px] text-slate-500">
            <Check className="mt-0.5 h-3 w-3 shrink-0 text-emerald-400/70" />
            {item}
          </li>
        ))}
      </ul>
    </div>
  );
}

function ToggleRow({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between rounded-xl border border-white/[0.06] bg-black/15 px-3 py-2.5">
      <span className="text-[11px] text-slate-400">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} label={label} />
    </div>
  );
}

function PresetButton({
  label,
  onClick,
  tone,
}: {
  label: string;
  onClick: () => void;
  tone: "amber" | "rose" | "cyan";
}) {
  const tones = {
    amber: "border-amber-400/20 bg-amber-400/[0.06] text-amber-200",
    rose: "border-rose-400/20 bg-rose-400/[0.06] text-rose-200",
    cyan: "border-cyan-400/20 bg-cyan-400/[0.06] text-cyan-200",
  };
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn("rounded-full border px-2.5 py-1 text-[10px] font-medium", tones[tone])}
    >
      {label}
    </button>
  );
}

function StatusLine({
  label,
  ok,
  value,
  neutral,
}: {
  label: string;
  ok?: boolean;
  value?: string | null;
  neutral?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-3">
      <span className="text-slate-500">{label}</span>
      <span
        className={cn(
          "inline-flex max-w-[190px] items-center gap-1.5 text-right text-[11px]",
          neutral
            ? "text-slate-400"
            : ok
              ? "text-emerald-300"
              : "text-rose-300",
        )}
      >
        <span
          className={cn(
            "h-1.5 w-1.5 shrink-0 rounded-full",
            neutral ? "bg-slate-500" : ok ? "bg-emerald-400" : "bg-rose-400",
          )}
        />
        <span className="break-all">{value ?? (ok ? "ready" : "not available")}</span>
      </span>
    </div>
  );
}

function SettingsSkeleton() {
  return (
    <div className="space-y-4">
      <div className="h-64 animate-pulse rounded-2xl border border-white/[0.06] bg-white/[0.025]" />
      <div className="h-64 animate-pulse rounded-2xl border border-white/[0.06] bg-white/[0.025]" />
    </div>
  );
}
