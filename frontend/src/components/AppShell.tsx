import { NavLink, Outlet } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  ArrowUpFromLine,
  ListChecks,
  Settings2,
  Wifi,
  WifiOff,
} from "lucide-react";
import { Logo } from "@/components/Logo";
import { useEventStream } from "@/hooks/useEventStream";
import { api, queryKeys } from "@/lib/api";
import { cn, formatBytes } from "@/lib/utils";

const navItems = [
  { to: "/", label: "New transfer", icon: ArrowUpFromLine, end: true },
  { to: "/tasks", label: "Tasks", icon: ListChecks },
  { to: "/settings", label: "Settings", icon: Settings2 },
];

export function AppShell() {
  const connected = useEventStream();
  const { data: system } = useQuery({
    queryKey: queryKeys.system,
    queryFn: api.systemStatus,
    refetchInterval: 12_000,
  });

  return (
    <div className="min-h-screen">
      <div className="pointer-events-none fixed inset-0 -z-10 bg-grid-faint bg-[size:56px_56px] opacity-40 [mask-image:radial-gradient(ellipse_at_center,black,transparent_75%)]" />
      <div className="mx-auto flex w-full max-w-[1500px]">
        <aside className="sticky top-0 hidden h-screen w-[260px] shrink-0 flex-col border-r border-white/[0.06] px-5 py-6 lg:flex">
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
                      ? "border border-cyan-400/20 bg-cyan-400/[0.09] text-cyan-100 shadow-[inset_0_1px_0_rgb(255_255_255/0.04)]"
                      : "border border-transparent text-slate-400 hover:bg-white/[0.04] hover:text-slate-200",
                  )
                }
              >
                <item.icon className="h-4 w-4" strokeWidth={1.9} />
                {item.label}
              </NavLink>
            ))}
          </nav>

          <div className="mt-8 rounded-2xl border border-white/[0.06] bg-white/[0.025] p-4">
            <div className="flex items-center justify-between text-[11px] uppercase tracking-wider text-slate-500">
              <span>Runtime</span>
              <span
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[10px]",
                  connected
                    ? "bg-emerald-400/10 text-emerald-300"
                    : "bg-amber-400/10 text-amber-300",
                )}
              >
                {connected ? <Wifi className="h-3 w-3" /> : <WifiOff className="h-3 w-3" />}
                {connected ? "live" : "polling"}
              </span>
            </div>
            <div className="mt-3 space-y-2 text-xs text-slate-400">
              <div className="flex items-center justify-between">
                <span>Active tasks</span>
                <span className="font-mono text-slate-200">
                  {system ? system.active_tasks : "—"}
                  <span className="text-slate-500">
                    {" "}
                    / {(system?.tasks.pending ?? 0) + (system?.active_tasks ?? 0)}
                  </span>
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>MEGA temp</span>
                <span className="font-mono text-slate-200">
                  {system ? formatBytes(system.mega_temp_bytes) : "—"}
                </span>
              </div>
              <div className="flex items-center justify-between">
                <span>MEGAcmd</span>
                <span
                  className={cn(
                    "inline-flex items-center gap-1",
                    system?.mega.server_running ? "text-emerald-300" : "text-slate-500",
                  )}
                >
                  <span
                    className={cn(
                      "h-1.5 w-1.5 rounded-full",
                      system?.mega.server_running ? "bg-emerald-400" : "bg-slate-600",
                    )}
                  />
                  {system?.mega.server_running ? "ready" : "offline"}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-auto flex items-center gap-2 rounded-xl border border-white/[0.05] bg-black/20 px-3 py-2.5 text-[11px] text-slate-500">
            <Activity className="h-3.5 w-3.5 text-cyan-400/70" />
            <span>
              v{system?.version ?? "0.1.0"} ·{" "}
              <span className="text-slate-400">
                {(system?.settings.max_http_tasks ?? 3) + (system?.settings.max_mega_tasks ?? 1)}{" "}
                slots
              </span>
            </span>
          </div>
        </aside>

        <div className="min-w-0 flex-1">
          <header className="sticky top-0 z-30 border-b border-white/[0.06] bg-background/75 px-4 py-3 backdrop-blur-xl lg:hidden">
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
                        "rounded-lg p-2 text-slate-400",
                        isActive && "bg-cyan-400/10 text-cyan-200",
                      )
                    }
                    aria-label={item.label}
                  >
                    <item.icon className="h-5 w-5" />
                  </NavLink>
                ))}
              </nav>
            </div>
          </header>

          <main className="mx-auto w-full max-w-[1240px] px-4 py-6 sm:px-6 lg:px-10 lg:py-9">
            <Outlet />
          </main>
          <footer className="mx-auto flex w-full max-w-[1240px] flex-wrap items-center justify-between gap-2 px-4 pb-8 text-[11px] text-slate-600 sm:px-6 lg:px-10">
            <span>Streaming downloader → bounded multipart → S3 compatible storage</span>
            <span className="inline-flex items-center gap-1.5">
              <span className={cn("h-1.5 w-1.5 rounded-full", connected ? "bg-emerald-400" : "bg-amber-400")} />
              {connected ? "SSE connected" : "SSE reconnecting"}
            </span>
          </footer>
        </div>
      </div>
    </div>
  );
}
