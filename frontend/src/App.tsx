import { useEffect } from "react";
import { Link, Route, Routes, useLocation } from "react-router-dom";
import { ArrowLeft, Compass } from "lucide-react";
import { AppShell } from "@/components/AppShell";
import { HomePage } from "@/pages/HomePage";
import { TasksPage } from "@/pages/TasksPage";
import { TaskDetailPage } from "@/pages/TaskDetailPage";
import { SettingsPage } from "@/pages/SettingsPage";
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
    <>
      <ScrollToTop />
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<HomePage />} />
          <Route path="/tasks" element={<TasksPage />} />
          <Route path="/tasks/:id" element={<TaskDetailPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="*" element={<NotFound />} />
        </Route>
      </Routes>
    </>
  );
}

function NotFound() {
  return (
    <div className="grid min-h-[60vh] place-items-center text-center">
      <div>
        <div className="mx-auto grid h-14 w-14 place-items-center rounded-2xl border border-white/[0.07] bg-white/[0.03]">
          <Compass className="h-6 w-6 text-slate-500" />
        </div>
        <h1 className="mt-4 text-lg font-medium text-slate-200">Page not found</h1>
        <p className="mt-1 text-sm text-slate-500">That route does not exist.</p>
        <Link to="/" className="mt-5 inline-block">
          <Button variant="outline">
            <ArrowLeft className="h-4 w-4" />
            Back to transfers
          </Button>
        </Link>
      </div>
    </div>
  );
}
