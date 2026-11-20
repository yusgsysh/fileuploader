import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, ApiError, queryKeys } from "@/lib/api";
import type { Task, TaskDetail } from "@/types";

function errorMessage(error: unknown): string {
  return error instanceof ApiError ? error.message : "Request failed";
}

export function useTaskActions() {
  const queryClient = useQueryClient();

  const mergeTask = (task: Task) => {
    queryClient.setQueryData<Task[]>(queryKeys.tasks, (old) => {
      if (!old) return old;
      const index = old.findIndex((item) => item.id === task.id);
      if (index === -1) return [task, ...old];
      const next = [...old];
      next[index] = task;
      return next;
    });
    queryClient.setQueryData<TaskDetail | undefined>(
      queryKeys.task(task.id),
      (old) => (old ? { ...old, ...task, history: old.history } : old),
    );
    void queryClient.invalidateQueries({ queryKey: queryKeys.system });
  };

  const cancel = useMutation({
    mutationFn: api.cancelTask,
    onSuccess: (task) => {
      mergeTask(task);
      toast.info("Cancellation requested", {
        description: "Active connections will be closed and multipart parts aborted.",
      });
    },
    onError: (error) => toast.error("Could not cancel task", { description: errorMessage(error) }),
  });

  const retry = useMutation({
    mutationFn: api.retryTask,
    onSuccess: (task) => {
      mergeTask(task);
      toast.success("Retry queued", { description: task.filename });
    },
    onError: (error) => toast.error("Could not retry task", { description: errorMessage(error) }),
  });

  const remove = useMutation({
    mutationFn: api.deleteTask,
    onSuccess: (_value, id) => {
      queryClient.setQueryData<Task[]>(queryKeys.tasks, (old) =>
        old ? old.filter((task) => task.id !== id) : old,
      );
      queryClient.removeQueries({ queryKey: queryKeys.task(id) });
      toast.success("Task record deleted");
    },
    onError: (error) => toast.error("Could not delete task", { description: errorMessage(error) }),
  });

  const deleteTask = (id: string, onDeleted?: () => void) => {
    if (window.confirm("Delete this task record? Temporary MEGA files will also be removed.")) {
      remove.mutate(id, { onSuccess: () => onDeleted?.() });
    }
  };

  const busyId =
    (cancel.isPending ? cancel.variables : undefined) ??
    (retry.isPending ? retry.variables : undefined) ??
    (remove.isPending ? remove.variables : undefined);

  return {
    cancel: (id: string) => cancel.mutate(id),
    retry: (id: string) => retry.mutate(id),
    deleteTask,
    busyId,
  };
}
