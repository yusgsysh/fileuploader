import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { api, ApiError, queryKeys } from "@/lib/api";
import type { Task, TaskDetail } from "@/types";

function errorMessage(error: unknown): string {
  return error instanceof ApiError ? error.message : "请求失败，请稍后再试";
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
      toast.info("已请求取消", {
        description: "正在关闭活动连接并中止未完成的分片上传。",
      });
    },
    onError: (error) => toast.error("取消任务失败", { description: errorMessage(error) }),
  });

  const retry = useMutation({
    mutationFn: api.retryTask,
    onSuccess: (task) => {
      mergeTask(task);
      toast.success("已加入重试队列", { description: task.filename });
    },
    onError: (error) => toast.error("重试任务失败", { description: errorMessage(error) }),
  });

  const remove = useMutation({
    mutationFn: api.deleteTask,
    onSuccess: (_value, id) => {
      queryClient.setQueryData<Task[]>(queryKeys.tasks, (old) =>
        old ? old.filter((task) => task.id !== id) : old,
      );
      queryClient.removeQueries({ queryKey: queryKeys.task(id) });
      toast.success("任务记录已删除");
    },
    onError: (error) => toast.error("删除任务失败", { description: errorMessage(error) }),
  });

  const deleteTask = (id: string, onDeleted?: () => void) => {
    if (window.confirm("确定删除该任务记录吗？MEGA 临时文件也会一并清理。")) {
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
