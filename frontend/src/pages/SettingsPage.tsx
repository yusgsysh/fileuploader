import { useEffect, useState } from "react";
import type React from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Boxes,
  CheckCircle2,
  Cloud,
  CloudCog,
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
  Trash2,
  UserRound,
  X,
  XCircle,
} from "lucide-react";
import { toast } from "sonner";
import { api, ApiError, queryKeys } from "@/lib/api";
import { useAuth } from "@/hooks/useAuth";
import type {
  AppSettings,
  MegaCheck,
  S3Config,
  S3ConfigInput,
  S3TestResult,
} from "@/types";
import { cn, formatDate, parseHeadersJson } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Field, Input, Textarea } from "@/components/ui/input";
import { Badge, Switch } from "@/components/ui/primitives";

const MiB = 1024 * 1024;
const GiB = 1024 * MiB;

type Tab = "transfer" | "mega" | "s3" | "account";

export function SettingsPage() {
  const [tab, setTab] = useState<Tab>("transfer");
  const settingsState = useSettings();
  return (
    <div className="space-y-6 animate-fade-up">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <h1 className="text-2xl font-semibold tracking-tight text-slate-900 sm:text-3xl">设置</h1>
      </div>

      <div className="flex flex-wrap gap-1.5 rounded-xl border border-slate-200 bg-slate-50 p-1">
        {(
          [
            ["transfer", "传输引擎", Gauge],
            ["mega", "MEGA / MEGAcmd", Cloud],
            ["s3", "S3 与 R2", Database],
            ["account", "账号", UserRound],
          ] as const
        ).map(([key, label, Icon]) => (
          <button
            key={key}
            onClick={() => setTab(key)}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-medium transition-colors",
              tab === key
                ? "bg-white text-slate-900 shadow-sm"
                : "text-slate-500 hover:text-slate-700",
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
      {tab === "account" ? <AccountSettings /> : null}
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
      toast.success("设置已保存");
    },
    onError: (error) =>
      toast.error("保存设置失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
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
    <div className="space-y-5 pb-28">
        <SettingsSection
          icon={Boxes}
          title="分片上传"
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="分片大小"
              suffix="MiB"
              value={settings.part_size_bytes / MiB}
              min={5}
              max={5120}
              step={1}
              onChange={(value) => update("part_size_bytes", Math.round(value * MiB))}
            />
            <NumberField
              label="上传并发数"
              value={settings.upload_concurrency}
              min={1}
              max={16}
              onChange={(value) => update("upload_concurrency", value)}
            />
            <NumberField
              label="最大待上传分片数"
              value={settings.max_pending_parts}
              min={1}
              max={32}
              onChange={(value) => update("max_pending_parts", value)}
            />
            <NumberField
              label="HTTP 缓冲区"
              suffix="KiB"
              value={settings.http_buffer_bytes / 1024}
              min={64}
              max={16384}
              step={64}
              onChange={(value) => update("http_buffer_bytes", Math.round(value * 1024))}
            />
          </div>
        </SettingsSection>

        <SettingsSection
          icon={Gauge}
          title="调度与容错"
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="HTTP 任务并发"
              value={settings.max_http_tasks}
              min={1}
              max={32}
              onChange={(value) => update("max_http_tasks", value)}
            />
            <NumberField
              label="MEGA 任务并发"
              value={settings.max_mega_tasks}
              min={1}
              max={16}
              onChange={(value) => update("max_mega_tasks", value)}
            />
            <NumberField
              label="排队任务上限"
              value={settings.max_queued_tasks}
              min={1}
              max={1000}
              onChange={(value) => update("max_queued_tasks", value)}
            />
            <NumberField
              label="重试次数"
              value={settings.max_retries}
              min={0}
              max={10}
              onChange={(value) => update("max_retries", value)}
            />
            <NumberField
              label="HTTP 超时"
              suffix="秒"
              value={settings.http_timeout_secs}
              min={1}
              max={3600}
              onChange={(value) => update("http_timeout_secs", value)}
            />
            <NumberField
              label="最大重定向次数"
              value={settings.max_redirects}
              min={0}
              max={20}
              onChange={(value) => update("max_redirects", value)}
            />
            <NumberField
              label="最大文件大小"
              suffix="GiB"
              value={settings.max_file_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("max_file_bytes", Math.round(value * GiB))}
            />
            <NumberField
              label="任务最长运行时间"
              suffix="小时"
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
        result.ok ? "MEGAcmd 检查通过" : "MEGAcmd 存在问题",
      );
      void queryClient.invalidateQueries({ queryKey: ["mega-status"] });
    },
    onError: (error) =>
      toast.error("MEGA 检查失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      }),
  });

  if (isLoading || !settings) return <SettingsSkeleton />;

  return (
    <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_340px]">
      <div className="space-y-5 pb-28">
        <SettingsSection
          icon={ServerCog}
          title="MEGAcmd 路径"
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label="mega-get 路径">
              <Input
                value={settings.mega_get_path}
                onChange={(event) => update("mega_get_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="MEGAcmd 服务路径">
              <Input
                value={settings.mega_server_path}
                onChange={(event) => update("mega_server_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="mega-whoami 路径">
              <Input
                value={settings.mega_whoami_path}
                onChange={(event) => update("mega_whoami_path", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="MEGAcmd HOME">
              <Input
                value={settings.mega_home}
                onChange={(event) => update("mega_home", event.target.value)}
                spellCheck={false}
              />
            </Field>
            <Field label="临时下载目录" className="sm:col-span-2">
              <Input
                value={settings.mega_temp_dir}
                onChange={(event) => update("mega_temp_dir", event.target.value)}
                spellCheck={false}
              />
            </Field>
          </div>
          <div className="flex items-center justify-between rounded-xl border border-slate-200 bg-slate-50 px-4 py-3">
            <div className="text-xs font-medium text-slate-800">自动启动 MEGAcmd 服务</div>
            <Switch
              checked={settings.mega_auto_start_server}
              onCheckedChange={(value) => update("mega_auto_start_server", value)}
            />
          </div>
        </SettingsSection>

        <SettingsSection
          icon={HardDrive}
          title="MEGA 磁盘与并发"
        >
          <div className="grid gap-4 sm:grid-cols-2">
            <NumberField
              label="MEGA 下载并发"
              value={settings.mega_max_downloads}
              min={1}
              max={16}
              onChange={(value) => update("mega_max_downloads", value)}
            />
            <NumberField
              label="单任务磁盘上限"
              suffix="GiB"
              value={settings.mega_max_task_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("mega_max_task_bytes", Math.round(value * GiB))}
            />
            <NumberField
              label="临时空间总上限"
              suffix="GiB"
              value={settings.mega_temp_max_bytes / GiB}
              min={0}
              step={0.5}
              onChange={(value) => update("mega_temp_max_bytes", Math.round(value * GiB))}
            />
            <NumberField
              label="保留时长"
              suffix="小时"
              value={settings.mega_retention_secs / 3600}
              min={0}
              max={2160}
              step={1}
              onChange={(value) => update("mega_retention_secs", Math.round(value * 3600))}
            />
            <NumberField
              label="mega-get 超时"
              suffix="小时"
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
            <div className="flex items-center gap-2 text-sm font-semibold text-slate-900">
              <CloudCog className="h-4 w-4 text-violet-700" />
              MEGAcmd 状态
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
            <StatusLine label="已安装" ok={status?.installed} value={status?.version} />
            <StatusLine label="服务" ok={status?.server_running} value={status?.message} />
            <StatusLine
              label="会话"
              ok={status?.logged_in}
              value={status?.logged_in ? status.account : "仅公共链接"}
              neutral={!status?.logged_in}
            />
            <div className="rounded-xl border border-slate-200 bg-slate-50 p-3 text-[10px] leading-relaxed text-slate-500">
              <div className="font-mono text-slate-600">{settings.mega_get_path}</div>
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
            运行诊断
          </Button>
          {checks ? (
            <div className="mt-4 space-y-2">
              {checks.map((check) => (
                <div
                  key={check.name}
                  className="rounded-lg border border-slate-200 bg-slate-50 px-3 py-2"
                >
                  <div className="flex items-center gap-2 text-[11px] text-slate-700">
                    {check.ok ? (
                      <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" />
                    ) : (
                      <XCircle className="h-3.5 w-3.5 text-rose-500" />
                    )}
                    {check.name}
                  </div>
                  <div className="mt-1 break-words text-[10px] text-slate-500">{check.detail}</div>
                </div>
              ))}
            </div>
          ) : null}
        </div>

      </aside>
    </div>
  );
}

function AccountSettings() {
  const { status } = useAuth();
  const queryClient = useQueryClient();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirm, setConfirm] = useState("");

  const change = useMutation({
    mutationFn: () => api.changePassword(current, next),
    onSuccess: (value) => {
      queryClient.setQueryData(queryKeys.auth, value);
      setCurrent("");
      setNext("");
      setConfirm("");
      toast.success("密码已更新", {
        description: "其他设备上的登录状态已失效，需要重新登录。",
      });
    },
    onError: (error) =>
      toast.error("修改密码失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      }),
  });

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!current) {
      toast.error("请输入当前密码");
      return;
    }
    if (next.length < 8) {
      toast.error("新密码至少需要 8 个字符");
      return;
    }
    if (next !== confirm) {
      toast.error("两次输入的新密码不一致");
      return;
    }
    change.mutate();
  };

  return (
    <div className="space-y-5">
        <SettingsSection
          icon={UserRound}
          title="管理员账号"
        >
          <div className="mb-4 rounded-xl border border-slate-200 bg-slate-50 px-4 py-3 text-xs text-slate-600">
            当前账号：<span className="text-slate-800">{status?.username ?? "—"}</span>
          </div>
          <form onSubmit={submit} className="grid gap-4 sm:grid-cols-2">
            <Field label="当前密码" className="sm:col-span-2">
              <Input
                type="password"
                value={current}
                onChange={(event) => setCurrent(event.target.value)}
                placeholder="••••••••"
                autoComplete="current-password"
              />
            </Field>
            <Field label="新密码">
              <Input
                type="password"
                value={next}
                onChange={(event) => setNext(event.target.value)}
                placeholder="••••••••"
                autoComplete="new-password"
              />
            </Field>
            <Field label="确认新密码">
              <Input
                type="password"
                value={confirm}
                onChange={(event) => setConfirm(event.target.value)}
                placeholder="再次输入新密码"
                autoComplete="new-password"
              />
            </Field>
            <div className="sm:col-span-2 flex justify-end">
              <Button type="submit" disabled={change.isPending}>
                {change.isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <KeyRound className="h-4 w-4" />
                )}
                更新密码
              </Button>
            </div>
          </form>
        </SettingsSection>
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
      toast.success(form.id ? "S3 配置已更新" : "S3 配置已创建");
      void queryClient.invalidateQueries({ queryKey: queryKeys.s3Configs });
      reset();
    },
    onError: (error) =>
      toast.error("保存 S3 配置失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      }),
  });

  const remove = useMutation({
    mutationFn: api.deleteS3Config,
    onSuccess: () => {
      toast.success("S3 配置已删除");
      void queryClient.invalidateQueries({ queryKey: queryKeys.s3Configs });
      if (form.id) reset();
    },
    onError: (error) =>
      toast.error("删除配置失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      }),
  });

  const test = useMutation({
    mutationFn: () => {
      if (!form.id && !form.secret_key) {
        throw new Error("测试新配置前请先填写 Secret Access Key。");
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
        message: error instanceof Error ? error.message : "连接测试失败",
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
            <h2 className="text-sm font-semibold text-slate-900">目标存储</h2>
            <Button
              variant="outline"
              size="sm"
              onClick={() => {
                reset();
                window.scrollTo({ top: 120, behavior: "smooth" });
              }}
            >
              <Plus className="h-3.5 w-3.5" />
              新建
            </Button>
          </div>

          {isLoading ? (
            <div className="space-y-3">
              {Array.from({ length: 2 }).map((_, index) => (
                <div key={index} className="h-32 animate-pulse rounded-2xl bg-white" />
              ))}
            </div>
          ) : !configs?.length ? (
            <div className="rounded-2xl border border-dashed border-slate-200 bg-slate-50/60 px-5 py-12 text-center">
              <Cloud className="mx-auto h-8 w-8 text-slate-600" />
              <div className="mt-3 text-sm text-slate-700">尚未配置 S3</div>
            </div>
          ) : (
            configs.map((config) => (
              <div
                key={config.id}
                className="rounded-2xl border border-slate-200 bg-card/60 p-4 transition-colors hover:border-slate-300"
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="flex min-w-0 items-start gap-3">
                    <div className="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-slate-200 bg-white">
                      <Cloud className="h-4 w-4 text-cyan-700" />
                    </div>
                    <div className="min-w-0">
                      <div className="flex flex-wrap items-center gap-2">
                        <span className="truncate text-sm font-medium text-slate-900">
                          {config.name}
                        </span>
                        <Badge tone={config.secret_set ? "green" : "red"}>
                          {config.secret_set ? "凭据已配置" : "缺少密钥"}
                        </Badge>
                        {config.path_style ? <Badge>路径风格</Badge> : null}
                      </div>
                      <div className="mt-1 break-all font-mono text-[10px] text-slate-500">
                        {config.endpoint || "AWS S3 默认端点"}
                      </div>
                      <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-slate-500">
                        <span>区域 {config.region}</span>
                        <span>存储桶 {config.bucket || "—"}</span>
                        <span>前缀 {config.default_prefix || "—"}</span>
                        <span>更新于 {formatDate(config.updated_at)}</span>
                      </div>
                    </div>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button size="sm" variant="ghost" onClick={() => edit(config)}>
                      <Pencil className="h-3.5 w-3.5" />
                      编辑
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
                                error instanceof Error ? error.message : "连接测试失败",
                              latency_ms: 0,
                            }),
                          );
                      }}
                      title="测试已保存的凭据"
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-slate-500 hover:text-rose-600"
                      disabled={remove.isPending}
                      onClick={() => {
                        if (
                          window.confirm(
                            `确定删除“${config.name}”吗？已被任务引用的配置无法删除。`,
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
                <h2 className="text-sm font-semibold text-slate-900">
                  {form.id ? "编辑配置" : "新建配置"}
                </h2>
              </div>
              {form.id ? (
                <Button variant="ghost" size="icon" onClick={reset} title="取消编辑">
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
              <Field label="显示名称">
                <Input
                  value={form.name}
                  onChange={(event) => setForm({ ...form, name: event.target.value })}
                  placeholder="Cloudflare R2"
                />
              </Field>
              <Field
                label="端点"
              >
                <Input
                  value={form.endpoint}
                  onChange={(event) => setForm({ ...form, endpoint: event.target.value })}
                  placeholder="https://<account-id>.r2.cloudflarestorage.com"
                  spellCheck={false}
                />
              </Field>
              <div className="grid grid-cols-2 gap-3">
                <Field label="区域">
                  <Input
                    value={form.region}
                    onChange={(event) => setForm({ ...form, region: event.target.value })}
                    placeholder="auto"
                    spellCheck={false}
                  />
                </Field>
                <Field label="默认存储桶">
                  <Input
                    value={form.bucket}
                    onChange={(event) => setForm({ ...form, bucket: event.target.value })}
                    placeholder="downloads"
                  />
                </Field>
              </div>
              <Field label="Access Key ID">
                <Input
                  value={form.access_key}
                  onChange={(event) => setForm({ ...form, access_key: event.target.value })}
                  placeholder="Access Key ID"
                  spellCheck={false}
                  autoComplete="off"
                />
              </Field>
              <Field
                label="Secret Access Key"
              >
                <Input
                  type="password"
                  value={form.secret_key}
                  onChange={(event) => setForm({ ...form, secret_key: event.target.value })}
                  placeholder={form.id && form.secret_set ? "••••••••（未修改）" : "Secret Access Key"}
                  autoComplete="new-password"
                />
              </Field>
              <Field label="默认对象前缀">
                <Input
                  value={form.default_prefix}
                  onChange={(event) => setForm({ ...form, default_prefix: event.target.value })}
                  placeholder="downloads/archive"
                  spellCheck={false}
                />
              </Field>
              <Field label="自定义元数据">
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
                  label="路径风格 URL"
                  checked={form.path_style}
                  onChange={(value) => setForm({ ...form, path_style: value })}
                />
                <ToggleRow
                  label="TLS 加密"
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
                测试
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
                {form.id ? "更新" : "创建"}
              </Button>
            </div>
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
          ? "border-emerald-200 bg-emerald-50 text-emerald-800"
          : "border-rose-200 bg-rose-50 text-rose-800",
      )}
    >
      {result.ok ? (
        <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-600" />
      ) : (
        <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-rose-500" />
      )}
      <div>
        <div className="font-medium">{result.ok ? "连接成功" : "连接失败"}</div>
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
  children,
}: {
  icon: typeof Boxes;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="panel p-5 sm:p-6">
      <div className="mb-5 flex items-center gap-3">
        <div className="grid h-9 w-9 place-items-center rounded-xl border border-slate-200 bg-white">
          <Icon className="h-4 w-4 text-cyan-700" />
        </div>
        <h2 className="text-sm font-semibold text-slate-900">{title}</h2>
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
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
}) {
  return (
    <Field label={label}>
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
    <div className="sticky bottom-4 z-20 flex items-center justify-end gap-3 rounded-2xl border border-slate-200 bg-background/90 px-4 py-3 shadow-panel backdrop-blur-xl">
      <Button onClick={onClick} disabled={pending}>
        {pending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Save className="h-4 w-4" />}
        保存更改
      </Button>
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
    <div className="flex items-center justify-between rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5">
      <span className="text-[11px] text-slate-600">{label}</span>
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
    amber: "border-amber-200 bg-amber-50 text-amber-700",
    rose: "border-rose-200 bg-rose-50 text-rose-700",
    cyan: "border-cyan-200 bg-cyan-50 text-cyan-700",
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
            ? "text-slate-600"
            : ok
              ? "text-emerald-700"
              : "text-rose-600",
        )}
      >
        <span
          className={cn(
            "h-1.5 w-1.5 shrink-0 rounded-full",
            neutral ? "bg-slate-400" : ok ? "bg-emerald-500" : "bg-rose-500",
          )}
        />
        <span className="break-all">{value ?? (ok ? "已就绪" : "不可用")}</span>
      </span>
    </div>
  );
}

function SettingsSkeleton() {
  return (
    <div className="space-y-4">
      <div className="h-64 animate-pulse rounded-2xl border border-slate-200 bg-slate-50" />
      <div className="h-64 animate-pulse rounded-2xl border border-slate-200 bg-slate-50" />
    </div>
  );
}
