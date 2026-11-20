export type SourceType = "auto" | "http" | "mega";
export type ResolvedSource = "http" | "mega";
export type TaskStatus = "pending" | "running" | "completed" | "failed" | "cancelled";
export type TaskStage =
  | "pending"
  | "downloading"
  | "preparing_upload"
  | "uploading"
  | "finalizing"
  | "completed"
  | "failed"
  | "cancelled";

export interface PartInfo {
  part_number: number;
  size: number;
  etag?: string | null;
  status: string;
}

export interface MegaProgress {
  process_state: string;
  temp_bytes: number;
  temp_dir?: string | null;
  files_total: number;
  files_done: number;
  current_file?: string | null;
  percentage?: number | null;
}

export interface Progress {
  download_speed_bps: number;
  upload_speed_bps: number;
  download_progress?: number | null;
  upload_progress?: number | null;
  overall_progress?: number | null;
  eta_secs?: number | null;
  parts_completed: number;
  parts_total?: number | null;
  parts: PartInfo[];
  mega?: MegaProgress | null;
}

export interface Task {
  id: string;
  url: string;
  source_type: SourceType;
  resolved_source?: ResolvedSource | null;
  s3_config_id: string;
  bucket: string;
  object_key?: string | null;
  filename: string;
  content_type?: string | null;
  total_bytes?: number | null;
  downloaded_bytes: number;
  uploaded_bytes: number;
  status: TaskStatus;
  stage: TaskStage;
  error?: string | null;
  s3_upload_id?: string | null;
  s3_key?: string | null;
  temp_bytes: number;
  temp_complete: boolean;
  allow_overwrite: boolean;
  attempt: number;
  range_supported?: boolean | null;
  created_at: string;
  updated_at: string;
  started_at?: string | null;
  finished_at?: string | null;
  has_custom_headers: boolean;
  auth_username?: string | null;
  progress: Progress;
}

export interface TaskHistoryItem {
  id: number;
  attempt: number;
  status: string;
  stage: string;
  error?: string | null;
  started_at: string;
  finished_at?: string | null;
}

export interface TaskDetail extends Task {
  history: TaskHistoryItem[];
}

export interface CreateTaskInput {
  url: string;
  source_type: SourceType;
  s3_config_id?: string;
  bucket?: string;
  object_key?: string;
  filename?: string;
  user_agent?: string;
  referer?: string;
  cookie?: string;
  headers?: Record<string, string>;
  auth_username?: string;
  auth_password?: string;
  allow_overwrite?: boolean;
}

export interface AppSettings {
  part_size_bytes: number;
  upload_concurrency: number;
  http_buffer_bytes: number;
  max_pending_parts: number;
  max_http_tasks: number;
  max_mega_tasks: number;
  max_queued_tasks: number;
  max_retries: number;
  http_timeout_secs: number;
  max_redirects: number;
  max_file_bytes: number;
  task_timeout_secs: number;
  mega_get_path: string;
  mega_server_path: string;
  mega_whoami_path: string;
  mega_home: string;
  mega_temp_dir: string;
  mega_max_task_bytes: number;
  mega_temp_max_bytes: number;
  mega_retention_secs: number;
  mega_max_downloads: number;
  mega_auto_start_server: boolean;
  mega_download_timeout_secs: number;
}

export interface S3Config {
  id: string;
  name: string;
  endpoint: string;
  region: string;
  access_key: string;
  bucket: string;
  default_prefix: string;
  path_style: boolean;
  use_tls: boolean;
  metadata: Record<string, string>;
  secret_set: boolean;
  created_at: string;
  updated_at: string;
}

export interface S3ConfigInput {
  name: string;
  endpoint: string;
  region: string;
  access_key: string;
  secret_key?: string;
  bucket: string;
  default_prefix: string;
  path_style: boolean;
  use_tls: boolean;
  metadata: Record<string, string>;
}

export interface BucketInfo {
  name: string;
  creation_date?: string | null;
}

export interface S3TestResult {
  ok: boolean;
  message: string;
  bucket_count?: number | null;
  latency_ms: number;
}

export interface TaskCounts {
  total: number;
  pending: number;
  running: number;
  completed: number;
  failed: number;
  cancelled: number;
}

export interface MegaStatus {
  installed: boolean;
  server_running: boolean;
  logged_in: boolean;
  account?: string | null;
  version?: string | null;
  get_path: string;
  server_path: string;
  home: string;
  message?: string | null;
}

export interface SystemStatus {
  version: string;
  uptime_secs: number;
  database_ok: boolean;
  tasks: TaskCounts;
  active_tasks: number;
  queued_tasks: number;
  mega_temp_bytes: number;
  mega_temp_dirs: number;
  settings: {
    max_http_tasks: number;
    max_mega_tasks: number;
    upload_concurrency: number;
    part_size_bytes: number;
    http_buffer_bytes: number;
    max_pending_parts: number;
  };
  mega: MegaStatus;
}

export interface MegaCheck {
  name: string;
  ok: boolean;
  detail: string;
}

export interface MegaTestResult {
  ok: boolean;
  checks: MegaCheck[];
}
