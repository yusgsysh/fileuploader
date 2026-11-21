import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  ArrowUpFromLine,
  CheckCircle2,
  CircleX,
  Loader2,
  RefreshCw,
  Search,
  Timer,
  XCircle,
} from "lucide-react";
import { api, queryKeys } from "@/lib/api";
import type { Task } from "@/types";
import { useTaskActions } from "@/hooks/useTaskActions";
import { TaskCard } from "@/components/TaskCard";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

type Filter = "all" | "active" | "completed" | "failed";

export function TasksPage() {
  const [filter, setFilter] = useState<Filter>("all");
  const [query, setQuery] = useState("");
  const { data: tasks, isLoading, isFetching, refetch } = useQuery({
    queryKey: queryKeys.tasks,
    queryFn: () => api.listTasks(300),
    refetchInterval: 5000,
  });
  const actions = useTaskActions();

  const counts = useMemo(() => {
    const list = tasks ?? [];
    return {
      all: list.length,
      active: list.filter((t) => t.status === "running" || t.status === "pending").length,
      completed: list.filter((t) => t.status === "completed").length,
      failed: list.filter((t) => t.status === "failed" || t.status === "cancelled").length,
    };
  }, [tasks]);

  const filtered = useMemo(() => {
    let list = tasks ?? [];
    if (filter === "active") {
      list = list.filter((t) => t.status === "running" || t.status === "pending");
    } else if (filter === "completed") {
      list = list.filter((t) => t.status === "completed");
    } else if (filter === "failed") {
      list = list.filter((t) => t.status === "failed" || t.status === "cancelled");
    }
    const needle = query.trim().toLowerCase();
    if (needle) {
      list = list.filter(
        (t) =>
          t.filename.toLowerCase().includes(needle) ||
          t.id.toLowerCase().includes(needle) ||
          t.bucket.toLowerCase().includes(needle) ||
          (t.s3_key ?? "").toLowerCase().includes(needle) ||
          t.url.toLowerCase().includes(needle),
      );
    }
    return list;
  }, [tasks, filter, query]);

  return (
    <div className="space-y-6 animate-fade-up">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="mb-1 text-[11px] uppercase tracking-[0.2em] text-cyan-700/70">
            传输队列
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-slate-900 sm:text-3xl">任务列表</h1>
          <p className="mt-1 text-sm text-slate-600">
            SSE 实时推送进度，断线时自动轮询。取消会中止当前分片上传。
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="md" onClick={() => void refetch()} disabled={isFetching}>
            {isFetching ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4" />
            )}
            刷新
          </Button>
          <Link to="/">
            <Button size="md">
              <ArrowUpFromLine className="h-4 w-4" />
              新建任务
            </Button>
          </Link>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <SummaryTile
          active={filter === "all"}
          label="全部任务"
          value={counts.all}
          icon={Timer}
          onClick={() => setFilter("all")}
        />
        <SummaryTile
          active={filter === "active"}
          label="进行中"
          value={counts.active}
          icon={Loader2}
          tone="cyan"
          pulse={counts.active > 0}
          onClick={() => setFilter("active")}
        />
        <SummaryTile
          active={filter === "completed"}
          label="已完成"
          value={counts.completed}
          icon={CheckCircle2}
          tone="green"
          onClick={() => setFilter("completed")}
        />
        <SummaryTile
          active={filter === "failed"}
          label="失败 / 已取消"
          value={counts.failed}
          icon={XCircle}
          tone="red"
          onClick={() => setFilter("failed")}
        />
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-1.5 rounded-xl border border-slate-200 bg-slate-50 p-1">
          {(
            [
              ["all", "全部", counts.all],
              ["active", "进行中", counts.active],
              ["completed", "已完成", counts.completed],
              ["failed", "异常", counts.failed],
            ] as const
          ).map(([key, label, count]) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                filter === key
                  ? "bg-white text-slate-900 shadow-sm"
                  : "text-slate-500 hover:text-slate-700",
              )}
            >
              {label}
              <span className="ml-1.5 font-mono text-[10px] text-slate-500">{count}</span>
            </button>
          ))}
        </div>
        <div className="relative w-full sm:w-72">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-500" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索文件名、ID、存储桶…"
            className="h-9 pl-9 text-xs"
          />
        </div>
      </div>

      {isLoading ? (
        <div className="space-y-3">
          {Array.from({ length: 3 }).map((_, index) => (
            <div
              key={index}
              className="h-44 animate-pulse rounded-2xl border border-slate-200 bg-slate-50"
            />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="grid place-items-center rounded-2xl border border-dashed border-slate-200 bg-slate-50/60 px-6 py-20 text-center">
          <div className="grid h-14 w-14 place-items-center rounded-2xl border border-slate-200 bg-white">
            <CircleX className="h-6 w-6 text-slate-600" />
          </div>
          <h2 className="mt-4 text-sm font-medium text-slate-800">
            {tasks?.length ? "当前筛选条件下没有任务" : "暂无传输任务"}
          </h2>
          <p className="mt-1 max-w-sm text-xs leading-relaxed text-slate-500">
            {tasks?.length
              ? "请调整筛选条件或搜索关键词。"
              : "粘贴 HTTP 或 MEGA 分享链接，并选择 S3 目标即可开始。"}
          </p>
          {!tasks?.length ? (
            <Link to="/" className="mt-5">
              <Button size="sm">开始新任务</Button>
            </Link>
          ) : null}
        </div>
      ) : (
        <div className="space-y-3">
          {filtered.map((task: Task) => (
            <TaskCard
              key={task.id}
              task={task}
              busy={actions.busyId === task.id}
              onCancel={actions.cancel}
              onRetry={actions.retry}
              onDelete={actions.deleteTask}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SummaryTile({
  label,
  value,
  icon: Icon,
  active,
  onClick,
  tone = "default",
  pulse,
}: {
  label: string;
  value: number;
  icon: typeof Timer;
  active: boolean;
  onClick: () => void;
  tone?: "default" | "cyan" | "green" | "red";
  pulse?: boolean;
}) {
  const tones = {
    default: "text-slate-600",
    cyan: "text-cyan-700",
    green: "text-emerald-700",
    red: "text-rose-600",
  } as const;
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all",
        active
          ? "border-cyan-200 bg-cyan-50"
          : "border-slate-200 bg-slate-50 hover:border-slate-300 hover:bg-slate-100",
      )}
    >
      <div className={cn("grid h-8 w-8 place-items-center rounded-lg bg-slate-50", tones[tone])}>
        <Icon className={cn("h-4 w-4", pulse && "animate-spin")} />
      </div>
      <div>
        <div className="font-mono text-lg font-semibold text-slate-900">{value}</div>
        <div className="text-[10px] uppercase tracking-wider text-slate-500">{label}</div>
      </div>
    </button>
  );
}
