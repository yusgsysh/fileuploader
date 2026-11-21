import { useEffect } from "react";
import { Link, Route, Routes, useLocation } from "react-router-dom";
import { ArrowLeft, Compass, Loader2 } from "lucide-react";
import { AppShell } from "@/components/AppShell";
import { Logo } from "@/components/Logo";
import { AuthProvider, useAuth } from "@/hooks/useAuth";
import { HomePage } from "@/pages/HomePage";
import { TasksPage } from "@/pages/TasksPage";
import { TaskDetailPage } from "@/pages/TaskDetailPage";
import { SettingsPage } from "@/pages/SettingsPage";
import { LoginPage } from "@/pages/LoginPage";
import { Button } from "@/components/ui/button";

function ScrollToTop() {
  const location = useLocation();
  useEffect(() => {
    window.scrollTo({ top: 0, behavior: "instant" as ScrollBehavior });
  }, [location.pathname]);
  return null;
}

export default function App() {
  return (
    <AuthProvider>
      <ScrollToTop />
      <AuthGate />
    </AuthProvider>
  );
}

function AuthGate() {
  const { status, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="flex min-h-screen flex-col items-center justify-center gap-6">
        <Logo />
        <div className="flex items-center gap-2 text-xs text-slate-500">
          <Loader2 className="h-4 w-4 animate-spin" />
          正在检查登录状态…
        </div>
      </div>
    );
  }

  if (!status?.authenticated) {
    return <LoginPage />;
  }

  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<HomePage />} />
        <Route path="/tasks" element={<TasksPage />} />
        <Route path="/tasks/:id" element={<TaskDetailPage />} />
        <Route path="/settings" element={<SettingsPage />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  );
}

function NotFound() {
  return (
    <div className="grid min-h-[60vh] place-items-center text-center">
      <div>
        <div className="mx-auto grid h-14 w-14 place-items-center rounded-2xl border border-slate-200 bg-white">
          <Compass className="h-6 w-6 text-slate-500" />
        </div>
        <h1 className="mt-4 text-lg font-medium text-slate-800">页面不存在</h1>
        <Link to="/" className="mt-5 inline-block">
          <Button variant="outline">
            <ArrowLeft className="h-4 w-4" />
            返回新建任务
          </Button>
        </Link>
      </div>
    </div>
  );
}
