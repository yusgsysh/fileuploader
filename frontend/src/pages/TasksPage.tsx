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
          <div className="mb-1 text-[11px] uppercase tracking-[0.2em] text-cyan-300/70">
            Transfer queue
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-white sm:text-3xl">Tasks</h1>
          <p className="mt-1 text-sm text-slate-400">
            Live progress from SSE with polling fallback. Cancel aborts the active multipart upload.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="md" onClick={() => void refetch()} disabled={isFetching}>
            {isFetching ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : (
              <RefreshCw className="h-4 w-4" />
            )}
            Refresh
          </Button>
          <Link to="/">
            <Button size="md">
              <ArrowUpFromLine className="h-4 w-4" />
              New transfer
            </Button>
          </Link>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <SummaryTile
          active={filter === "all"}
          label="All tasks"
          value={counts.all}
          icon={Timer}
          onClick={() => setFilter("all")}
        />
        <SummaryTile
          active={filter === "active"}
          label="Active"
          value={counts.active}
          icon={Loader2}
          tone="cyan"
          pulse={counts.active > 0}
          onClick={() => setFilter("active")}
        />
        <SummaryTile
          active={filter === "completed"}
          label="Completed"
          value={counts.completed}
          icon={CheckCircle2}
          tone="green"
          onClick={() => setFilter("completed")}
        />
        <SummaryTile
          active={filter === "failed"}
          label="Failed / cancelled"
          value={counts.failed}
          icon={XCircle}
          tone="red"
          onClick={() => setFilter("failed")}
        />
      </div>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-1.5 rounded-xl border border-white/[0.06] bg-white/[0.02] p-1">
          {(
            [
              ["all", "All", counts.all],
              ["active", "Active", counts.active],
              ["completed", "Completed", counts.completed],
              ["failed", "Issues", counts.failed],
            ] as const
          ).map(([key, label, count]) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              className={cn(
                "rounded-lg px-3 py-1.5 text-xs font-medium transition-colors",
                filter === key
                  ? "bg-white/[0.08] text-slate-100 shadow-sm"
                  : "text-slate-500 hover:text-slate-300",
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
            placeholder="Search name, id, bucket…"
            className="h-9 pl-9 text-xs"
          />
        </div>
      </div>

      {isLoading ? (
        <div className="space-y-3">
          {Array.from({ length: 3 }).map((_, index) => (
            <div
              key={index}
              className="h-44 animate-pulse rounded-2xl border border-white/[0.06] bg-white/[0.025]"
            />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="grid place-items-center rounded-2xl border border-dashed border-white/10 bg-white/[0.015] px-6 py-20 text-center">
          <div className="grid h-14 w-14 place-items-center rounded-2xl border border-white/[0.07] bg-white/[0.03]">
            <CircleX className="h-6 w-6 text-slate-600" />
          </div>
          <h2 className="mt-4 text-sm font-medium text-slate-200">
            {tasks?.length ? "No tasks match this view" : "No transfers yet"}
          </h2>
          <p className="mt-1 max-w-sm text-xs leading-relaxed text-slate-500">
            {tasks?.length
              ? "Adjust the filter or search query."
              : "Paste an HTTP or MEGA share link and choose an S3 destination to start."}
          </p>
          {!tasks?.length ? (
            <Link to="/" className="mt-5">
              <Button size="sm">Start a transfer</Button>
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
    default: "text-slate-400",
    cyan: "text-cyan-300",
    green: "text-emerald-300",
    red: "text-rose-300",
  } as const;
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-3 rounded-xl border px-4 py-3 text-left transition-all",
        active
          ? "border-cyan-400/25 bg-cyan-400/[0.07]"
          : "border-white/[0.06] bg-white/[0.02] hover:border-white/[0.1] hover:bg-white/[0.04]",
      )}
    >
      <div className={cn("grid h-8 w-8 place-items-center rounded-lg bg-black/20", tones[tone])}>
        <Icon className={cn("h-4 w-4", pulse && "animate-spin")} />
      </div>
      <div>
        <div className="font-mono text-lg font-semibold text-slate-100">{value}</div>
        <div className="text-[10px] uppercase tracking-wider text-slate-500">{label}</div>
      </div>
    </button>
  );
}
