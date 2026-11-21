import { NavLink, Outlet } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  ArrowUpFromLine,
  ListChecks,
  LogOut,
  Settings2,
  Wifi,
  WifiOff,
} from "lucide-react";
import { Logo } from "@/components/Logo";
import { useAuth } from "@/hooks/useAuth";
import { useEventStream } from "@/hooks/useEventStream";
import { api, queryKeys } from "@/lib/api";
import { cn, formatBytes } from "@/lib/utils";

const navItems = [
  { to: "/", label: "新建任务", icon: ArrowUpFromLine, end: true },
  { to: "/tasks", label: "任务列表", icon: ListChecks },
  { to: "/settings", label: "设置", icon: Settings2 },
];

export function AppShell() {
  const connected = useEventStream();
  const { status, logout } = useAuth();
  const { data: system } = useQuery({
    queryKey: queryKeys.system,
    queryFn: api.systemStatus,
    refetchInterval: 12_000,
  });

  return (
    <div className="min-h-screen">
      <div className="pointer-events-none fixed inset-0 -z-10 bg-grid-faint bg-[size:56px_56px] opacity-40 [mask-image:radial-gradient(ellipse_at_center,black,transparent_75%)]" />
      <div className="mx-auto flex w-full max-w-[1500px]">
        <aside className="sticky top-0 hidden h-screen w-[260px] shrink-0 flex-col border-r border-slate-200 px-5 py-6 lg:flex">
          <Logo />
          <nav className="mt-10 flex flex-col gap-1.5">
            {navItems.map((item) => (
              <NavLink
                key={item.to}
                to={item.to}
                end={item.end}
                className={({ isActive }) =>
                  cn(
                    "group flex items-center gap-3 rounded-xl px-3 py-2.5 text-sm font-medium transition-all",
                    isActive
                      ? "border border-cyan-200 bg-cyan-50 text-cyan-800 shadow-sm"
                      : "border border-transparent text-slate-600 hover:bg-slate-100 hover:text-slate-800",
                  )
                }
              >
                <item.icon className="h-4 w-4" strokeWidth={1.9} />
                {item.label}
              </NavLink>
            ))}
          </nav>

          <div className="mt-8 rounded-2xl border border-slate-200 bg-slate-50 p-4">
            <div className="flex items-center justify-between text-[11px] uppercase tracking-wider text-slate-500">
              <span>运行状态</span>
              <span
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px]",
                  connected
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-amber-50 text-amber-700",
                )}
              >
                {connected ? <Wifi className="h-3 w-3" /> : <WifiOff className="h-3 w-3" />}
                {connected ? "实时" : "轮询"}
              </span>
            </div>
            <div className="mt-3 space-y-2 text-xs text-slate-600">
              <div className="flex items-center justify-between">
                <span>进行中任务</span>
                <span className="font-mono text-slate-800">
                  {system ? system.active_tasks : "—"}
                  <span className="text-slate-500">
                    {" "}
                    / {(system?.tasks.pending ?? 0) + (system?.active_tasks ?? 0)}
                  </span>
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>MEGA 临时空间</span>
                <span className="font-mono text-slate-800">
                  {system ? formatBytes(system.mega_temp_bytes) : "—"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>MEGAcmd</span>
                <span
                  className={cn(
                    "inline-flex items-center gap-1",
                    system?.mega.server_running ? "text-emerald-700" : "text-slate-500",
                  )}
                >
                  <span
                    className={cn(
                      "h-1.5 w-1.5 rounded-full",
                      system?.mega.server_running ? "bg-emerald-500" : "bg-slate-300",
                    )}
                  />
                  {system?.mega.server_running ? "已就绪" : "未启动"}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-auto space-y-2">
            <div className="flex items-center gap-2 rounded-xl border border-slate-200 bg-slate-50 px-3 py-2.5 text-[11px] text-slate-500">
              <Activity className="h-3.5 w-3.5 text-cyan-600/70" />
              <span>
                v{system?.version ?? "0.1.0"} ·{" "}
                <span className="text-slate-600">
                  {(system?.settings.max_http_tasks ?? 3) + (system?.settings.max_mega_tasks ?? 1)}{" "}
                  个并发槽位
                </span>
              </span>
            </div>
            <div className="flex items-center justify-between gap-2 rounded-xl border border-slate-200 bg-slate-50 px-3 py-2 text-[11px] text-slate-500">
              <span className="truncate">
                账号：<span className="text-slate-700">{status?.username ?? "管理员"}</span>
              </span>
              <button
                type="button"
                onClick={() => void logout()}
                className="inline-flex shrink-0 items-center gap-1 rounded-lg px-1.5 py-1 text-slate-500 transition-colors hover:bg-slate-100 hover:text-rose-600"
                title="退出登录"
              >
                <LogOut className="h-3.5 w-3.5" />
                退出
              </button>
            </div>
          </div>
        </aside>

        <div className="min-w-0 flex-1">
          <header className="sticky top-0 z-30 border-b border-slate-200 bg-background/75 px-4 py-3 backdrop-blur-xl lg:hidden">
            <div className="flex items-center justify-between gap-3">
              <Logo compact />
              <nav className="flex items-center gap-1">
                {navItems.map((item) => (
                  <NavLink
                    key={item.to}
                    to={item.to}
                    end={item.end}
                    className={({ isActive }) =>
                      cn(
                        "rounded-lg p-2 text-slate-600",
                        isActive && "bg-cyan-50 text-cyan-700",
                      )
                    }
                    aria-label={item.label}
                  >
                    <item.icon className="h-5 w-5" />
                  </NavLink>
                ))}
                <button
                  type="button"
                  onClick={() => void logout()}
                  className="rounded-lg p-2 text-slate-500 transition-colors hover:text-rose-600"
                  aria-label="退出登录"
                >
                  <LogOut className="h-5 w-5" />
                </button>
              </nav>
            </div>
          </header>

          <main className="mx-auto w-full max-w-[1240px] px-4 py-6 sm:px-6 lg:px-10 lg:py-9">
            <Outlet />
          </main>
          <footer className="mx-auto flex w-full max-w-[1240px] flex-wrap items-center justify-between gap-2 px-4 pb-8 text-[11px] text-slate-600 sm:px-6 lg:px-10">
            <span>流式下载 → 有界分片上传 → S3 兼容对象存储</span>
            <span className="inline-flex items-center gap-1.5">
              <span className={cn("h-1.5 w-1.5 rounded-full", connected ? "bg-emerald-500" : "bg-amber-500")} />
              {connected ? "SSE 已连接" : "SSE 重连中"}
            </span>
          </footer>
        </div>
      </div>
    </div>
  );
}
