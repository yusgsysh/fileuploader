import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { Task, TaskDetail } from "@/types";
import { queryKeys } from "@/lib/api";

export function useEventStream() {
  const queryClient = useQueryClient();
  const [connected, setConnected] = useState(false);

  useEffect(() => {
    const source = new EventSource("/api/events");

    source.addEventListener("open", () => setConnected(true));
    source.addEventListener("hello", () => setConnected(true));
    source.addEventListener("error", () => setConnected(false));

    source.addEventListener("task", (event) => {
      try {
        const task = JSON.parse((event as MessageEvent).data) as Task;
        queryClient.setQueryData<Task[]>(queryKeys.tasks, (old) => {
          if (!old) return old;
          const index = old.findIndex((item) => item.id === task.id);
          if (index === -1) return [task, ...old];
          const next = [...old];
          next[index] = task;
          return next;
        });
        const detailKey = queryKeys.task(task.id);
        if (queryClient.getQueryData<TaskDetail>(detailKey)) {
          queryClient.setQueryData<TaskDetail>(detailKey, (old) =>
            old ? { ...old, ...task, history: old.history } : old,
          );
        }
        if (task.status !== "running" && task.status !== "pending") {
          void queryClient.invalidateQueries({ queryKey: queryKeys.system });
        }
      } catch {
        // Ignore malformed events; the next poll will reconcile state.
      }
    });

    source.addEventListener("deleted", (event) => {
      const id = (event as MessageEvent).data as string;
      queryClient.setQueryData<Task[]>(queryKeys.tasks, (old) =>
        old ? old.filter((task) => task.id !== id) : old,
      );
      queryClient.removeQueries({ queryKey: queryKeys.task(id) });
    });

    source.addEventListener("resync", () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.tasks });
      void queryClient.invalidateQueries({ queryKey: queryKeys.system });
    });

    return () => {
      source.close();
      setConnected(false);
    };
  }, [queryClient]);

  return connected;
}
