import { Badge } from "@/components/ui/primitives";
import type { TaskStatus } from "@/types";
import { statusLabel, statusTone } from "@/lib/utils";
import { cn } from "@/lib/utils";
import { CircleCheck, CircleX, Loader2, PauseCircle, Timer } from "lucide-react";

export function StatusBadge({ status }: { status: TaskStatus }) {
  const Icon =
    status === "completed"
      ? CircleCheck
      : status === "failed"
        ? CircleX
        : status === "cancelled"
          ? PauseCircle
          : status === "pending"
            ? Timer
            : Loader2;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium",
        statusTone(status),
      )}
    >
      <Icon
        className={cn("h-3.5 w-3.5", status === "running" && "animate-spin")}
        strokeWidth={2}
      />
      {statusLabel[status]}
    </span>
  );
}

export function SourceBadge({ source }: { source?: "http" | "mega" | null }) {
  if (source === "mega") {
    return (
      <Badge tone="violet" className="uppercase">
        MEGA
      </Badge>
    );
  }
  if (source === "http") {
    return (
      <Badge tone="cyan" className="uppercase">
        HTTP
      </Badge>
    );
  }
  return <Badge>auto</Badge>;
}
