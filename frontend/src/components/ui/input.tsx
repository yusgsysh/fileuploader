import * as React from "react";
import { cn } from "@/lib/utils";

export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, type, ...props }, ref) => (
  <input
    ref={ref}
    type={type}
    className={cn(
      "flex h-11 w-full rounded-xl border border-white/[0.08] bg-black/20 px-3.5 text-sm text-foreground shadow-inner shadow-black/20 transition-colors placeholder:text-muted-foreground/60 focus:border-cyan-400/45 focus:outline-none focus:ring-2 focus:ring-cyan-400/15 disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  />
));
Input.displayName = "Input";

export const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...props }, ref) => (
  <textarea
    ref={ref}
    className={cn(
      "flex min-h-[88px] w-full resize-y rounded-xl border border-white/[0.08] bg-black/20 px-3.5 py-3 text-sm text-foreground shadow-inner shadow-black/20 transition-colors placeholder:text-muted-foreground/60 focus:border-cyan-400/45 focus:outline-none focus:ring-2 focus:ring-cyan-400/15 disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  />
));
Textarea.displayName = "Textarea";

export const Select = React.forwardRef<
  HTMLSelectElement,
  React.SelectHTMLAttributes<HTMLSelectElement>
>(({ className, children, ...props }, ref) => (
  <select
    ref={ref}
    className={cn(
      "flex h-11 w-full appearance-none rounded-xl border border-white/[0.08] bg-black/20 px-3.5 text-sm text-foreground transition-colors focus:border-cyan-400/45 focus:outline-none focus:ring-2 focus:ring-cyan-400/15 disabled:cursor-not-allowed disabled:opacity-50",
      className,
    )}
    {...props}
  >
    {children}
  </select>
));
Select.displayName = "Select";

export function Label({
  className,
  ...props
}: React.LabelHTMLAttributes<HTMLLabelElement>) {
  return <label className={cn("field-label", className)} {...props} />;
}

export function Field({
  label,
  hint,
  children,
  className,
}: {
  label: string;
  hint?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("space-y-0", className)}>
      <Label>{label}</Label>
      {children}
      {hint ? (
        <p className="mt-1.5 text-[11px] leading-relaxed text-muted-foreground/75">{hint}</p>
      ) : null}
    </div>
  );
}
