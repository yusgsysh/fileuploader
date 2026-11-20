import type {
  AppSettings,
  BucketInfo,
  CreateTaskInput,
  MegaStatus,
  MegaTestResult,
  S3Config,
  S3ConfigInput,
  S3TestResult,
  SystemStatus,
  Task,
  TaskDetail,
} from "@/types";

const API_BASE = "/api";

export class ApiError extends Error {
  code: string;
  status: number;

  constructor(message: string, code = "request_failed", status = 0) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${API_BASE}${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  });
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    let code = "request_failed";
    try {
      const body = await response.json();
      message = body?.error?.message ?? message;
      code = body?.error?.code ?? code;
    } catch {
      // Keep the status-based fallback.
    }
    throw new ApiError(message, code, response.status);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return (await response.json()) as T;
}

export const api = {
  listTasks: (limit = 200) => request<Task[]>(`/tasks?limit=${limit}`),
  getTask: (id: string) => request<TaskDetail>(`/tasks/${id}`),
  createTask: (input: CreateTaskInput) =>
    request<Task>("/tasks", { method: "POST", body: JSON.stringify(input) }),
  cancelTask: (id: string) =>
    request<Task>(`/tasks/${id}/cancel`, { method: "POST" }),
  retryTask: (id: string) =>
    request<Task>(`/tasks/${id}/retry`, { method: "POST" }),
  deleteTask: (id: string) =>
    request<void>(`/tasks/${id}`, { method: "DELETE" }),

  getSettings: () => request<AppSettings>("/settings"),
  updateSettings: (settings: AppSettings) =>
    request<AppSettings>("/settings", {
      method: "PUT",
      body: JSON.stringify(settings),
    }),

  listS3Configs: () => request<S3Config[]>("/s3/configs"),
  createS3Config: (input: S3ConfigInput) =>
    request<S3Config>("/s3/configs", {
      method: "POST",
      body: JSON.stringify(input),
    }),
  updateS3Config: (id: string, input: S3ConfigInput) =>
    request<S3Config>(`/s3/configs/${id}`, {
      method: "PUT",
      body: JSON.stringify(input),
    }),
  deleteS3Config: (id: string) =>
    request<void>(`/s3/configs/${id}`, { method: "DELETE" }),
  listBuckets: (configId?: string) =>
    request<BucketInfo[]>(
      `/s3/buckets${configId ? `?config_id=${encodeURIComponent(configId)}` : ""}`,
    ),
  testS3: (body: { config_id?: string; config?: S3ConfigInput }) =>
    request<S3TestResult>("/s3/test", {
      method: "POST",
      body: JSON.stringify(body),
    }),

  systemStatus: () => request<SystemStatus>("/system/status"),
  megaStatus: () => request<MegaStatus>("/system/mega/status"),
  megaTest: (url?: string) =>
    request<MegaTestResult>("/system/mega/test", {
      method: "POST",
      body: JSON.stringify({ url: url || null }),
    }),
};

export const queryKeys = {
  tasks: ["tasks"] as const,
  task: (id: string) => ["task", id] as const,
  settings: ["settings"] as const,
  s3Configs: ["s3-configs"] as const,
  system: ["system"] as const,
  buckets: (configId?: string) => ["buckets", configId ?? "default"] as const,
};
