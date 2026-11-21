import { useState } from "react";
import type React from "react";
import { useMutation } from "@tanstack/react-query";
import {
  Eye,
  EyeOff,
  Loader2,
  LockKeyhole,
  LogIn,
  ShieldCheck,
  UserPlus,
  UserRound,
} from "lucide-react";
import { toast } from "sonner";
import { api, ApiError } from "@/lib/api";
import { useAuth } from "@/hooks/useAuth";
import { Logo } from "@/components/Logo";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/input";

export function LoginPage() {
  const { status, refresh } = useAuth();
  const setupRequired = status?.setup_required ?? false;

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPassword, setShowPassword] = useState(false);

  const submit = useMutation({
    mutationFn: () =>
      setupRequired
        ? api.setup(username.trim(), password)
        : api.login(username.trim(), password),
    onSuccess: async () => {
      toast.success(setupRequired ? "管理员账号创建成功" : "登录成功");
      await refresh();
    },
    onError: (error) => {
      toast.error(setupRequired ? "无法创建账号" : "登录失败", {
        description: error instanceof ApiError ? error.message : "请求失败，请稍后再试",
      });
    },
  });

  const onSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!username.trim()) {
      toast.error("请输入用户名");
      return;
    }
    if (!password) {
      toast.error("请输入密码");
      return;
    }
    if (setupRequired) {
      if (password.length < 8) {
        toast.error("密码至少需要 8 个字符");
        return;
      }
      if (password !== confirm) {
        toast.error("两次输入的密码不一致");
        return;
      }
    }
    submit.mutate();
  };

  return (
    <div className="relative flex min-h-screen items-center justify-center overflow-hidden px-4 py-10">
      <div className="pointer-events-none absolute inset-0 -z-10">
        <div className="absolute left-1/2 top-0 h-[420px] w-[720px] -translate-x-1/2 rounded-full bg-cyan-100/60 blur-3xl" />
        <div className="absolute bottom-0 right-1/4 h-[360px] w-[520px] rounded-full bg-indigo-100/50 blur-3xl" />
      </div>

      <div className="w-full max-w-[420px] animate-fade-up">
        <div className="mb-7 flex justify-center">
          <Logo />
        </div>

        <div className="panel overflow-hidden p-6 shadow-panel sm:p-8">
          <div className="mb-6 flex items-start gap-3">
            <div className="grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-cyan-200 bg-cyan-50 text-cyan-700">
              {setupRequired ? (
                <UserPlus className="h-5 w-5" />
              ) : (
                <LogIn className="h-5 w-5" />
              )}
            </div>
            <div>
              <h1 className="text-lg font-semibold tracking-tight text-slate-900">
                {setupRequired ? "初始化管理员账号" : "登录 FileUploader"}
              </h1>
              <p className="mt-1 text-xs leading-relaxed text-slate-600">
                {setupRequired
                  ? "首次使用，请设置管理员用户名和密码。账号创建后即可进入控制台。"
                  : "请输入管理员账号继续访问传输控制台。"}
              </p>
            </div>
          </div>

          <form onSubmit={onSubmit} className="space-y-4">
            <Field label="用户名">
              <div className="relative">
                <UserRound className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                <Input
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                  placeholder="admin"
                  className="pl-10"
                  autoComplete="username"
                  autoFocus
                  spellCheck={false}
                />
              </div>
            </Field>

            <Field
              label={setupRequired ? "设置密码" : "密码"}
              hint={setupRequired ? "至少 8 个字符，请妥善保存。" : undefined}
            >
              <div className="relative">
                <LockKeyhole className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                <Input
                  type={showPassword ? "text" : "password"}
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder="••••••••"
                  className="pl-10 pr-11"
                  autoComplete={setupRequired ? "new-password" : "current-password"}
                />
                <button
                  type="button"
                  onClick={() => setShowPassword((value) => !value)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 rounded p-1 text-slate-500 transition-colors hover:text-slate-700"
                  aria-label={showPassword ? "隐藏密码" : "显示密码"}
                >
                  {showPassword ? (
                    <EyeOff className="h-4 w-4" />
                  ) : (
                    <Eye className="h-4 w-4" />
                  )}
                </button>
              </div>
            </Field>

            {setupRequired ? (
              <Field label="确认密码">
                <div className="relative">
                  <LockKeyhole className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" />
                  <Input
                    type={showPassword ? "text" : "password"}
                    value={confirm}
                    onChange={(event) => setConfirm(event.target.value)}
                    placeholder="再次输入密码"
                    className="pl-10"
                    autoComplete="new-password"
                  />
                </div>
              </Field>
            ) : null}

            <Button
              type="submit"
              size="lg"
              className="mt-1 w-full"
              disabled={submit.isPending}
            >
              {submit.isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : setupRequired ? (
                <UserPlus className="h-4 w-4" />
              ) : (
                <LogIn className="h-4 w-4" />
              )}
              {submit.isPending
                ? setupRequired
                  ? "正在创建…"
                  : "正在登录…"
                : setupRequired
                  ? "创建账号并进入"
                  : "登录"}
            </Button>
          </form>

          <div className="mt-6 flex items-start gap-2 rounded-xl border border-emerald-100 bg-emerald-50 px-3 py-2.5 text-[11px] leading-relaxed text-emerald-800/80">
            <ShieldCheck className="mt-0.5 h-3.5 w-3.5 shrink-0 text-emerald-600/80" />
            <span>
              密码使用加盐哈希保存在服务器本地数据库中，登录状态通过 HttpOnly Cookie 维护。
            </span>
          </div>
        </div>

        <p className="mt-5 text-center text-[11px] text-slate-600">
          自托管 · 流式下载 · S3 兼容存储
        </p>
      </div>
    </div>
  );
}
