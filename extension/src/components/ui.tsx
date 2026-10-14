import type { ReactNode } from "react";
import type { Severity } from "@/lib/types";
import type { Tier } from "@/lib/types";

export function cx(...classes: (string | false | null | undefined)[]): string {
  return classes.filter(Boolean).join(" ");
}

export type Tone = "neutral" | "accent" | "success" | "danger" | "attention" | "severe" | "done";

const LABEL_TONES: Record<Tone, string> = {
  neutral: "border-border text-fg-muted",
  accent: "border-accent text-accent",
  success: "border-success text-success",
  danger: "border-danger text-danger",
  attention: "border-attention text-attention",
  severe: "border-severe text-severe",
  done: "border-done text-done",
};

/** Primer-style outlined label. */
export function Label({ tone = "neutral", children, className }: { tone?: Tone; children: ReactNode; className?: string }) {
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1 whitespace-nowrap rounded-full border px-2 text-xs font-medium leading-[18px]",
        LABEL_TONES[tone],
        className,
      )}
    >
      {children}
    </span>
  );
}

export function Counter({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "danger" | "accent" }) {
  return (
    <span
      className={cx(
        "inline-block min-w-5 rounded-full px-1.5 text-center text-xs font-medium leading-[18px]",
        tone === "danger" ? "bg-danger-emphasis text-white" : tone === "accent" ? "bg-accent-emphasis text-white" : "bg-neutral-subtle text-fg",
      )}
    >
      {children}
    </span>
  );
}

export const SEVERITY_TONE: Record<Severity, Tone> = {
  critical: "danger",
  high: "severe",
  medium: "attention",
  low: "neutral",
  info: "accent",
};

export function SeverityLabel({ severity }: { severity: Severity }) {
  return <Label tone={SEVERITY_TONE[severity]}>{severity}</Label>;
}

export const TIER_META: Record<Tier, { label: string; tone: Tone; description: string }> = {
  focus: { label: "Focus", tone: "danger", description: "Read these line by line." },
  skim: { label: "Skim", tone: "attention", description: "Check the shape, not every line." },
  skip: { label: "Skip", tone: "neutral", description: "Generated, mechanical or low risk." },
};

export function TierLabel({ tier }: { tier: Tier }) {
  return <Label tone={TIER_META[tier].tone}>{TIER_META[tier].label}</Label>;
}

export function Box({ children, className, id }: { children: ReactNode; className?: string; id?: string }) {
  return (
    <div id={id} className={cx("rounded-md border border-border bg-canvas", className)}>
      {children}
    </div>
  );
}

export function BoxHeader({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cx("flex items-center gap-2 rounded-t-md border-b border-border bg-canvas-subtle px-4 py-2.5", className)}>
      {children}
    </div>
  );
}

export function SectionHeading({ icon, title, count, action, id }: { icon?: ReactNode; title: string; count?: number; action?: ReactNode; id?: string }) {
  return (
    <div id={id} className="mb-3 flex items-center gap-2">
      {icon && <span className="text-fg-muted">{icon}</span>}
      <h2 className="text-base font-semibold">{title}</h2>
      {count !== undefined && <Counter>{count}</Counter>}
      {action && <div className="ml-auto">{action}</div>}
    </div>
  );
}

export function EmptyState({ icon, title, children }: { icon?: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="rounded-md border border-dashed border-border px-6 py-10 text-center">
      {icon && <div className="mb-2 flex justify-center text-fg-muted">{icon}</div>}
      <p className="font-semibold">{title}</p>
      {children && <div className="mx-auto mt-1 max-w-xl text-fg-muted">{children}</div>}
    </div>
  );
}

export function Code({ children }: { children: ReactNode }) {
  return <code className="rounded-md bg-neutral-subtle px-1.5 py-0.5 font-mono text-[85%]">{children}</code>;
}
