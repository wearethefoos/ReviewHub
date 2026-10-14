"use client";

import { useEffect, useState } from "react";
import type { RolloutStep } from "@/lib/schema";
import { CopyButton } from "./CopyButton";
import { InlineMarkdown, Markdown } from "./Markdown";
import { Box, BoxHeader, Counter, Label } from "./ui";

interface Phase {
  when: RolloutStep["when"];
  title: string;
  steps: RolloutStep[];
}

/** Rollout steps as a checklist. Checked state is a per-viewer convenience kept in localStorage. */
export function RolloutChecklist({ storageKey, phases }: { storageKey: string; phases: Phase[] }) {
  const [done, setDone] = useState<Record<string, boolean>>({});

  useEffect(() => {
    try {
      const saved = window.localStorage.getItem(storageKey);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- hydrating from storage after mount
      if (saved) setDone(JSON.parse(saved));
    } catch {
      // Storage can be unavailable (private mode, blocked site data).
    }
  }, [storageKey]);

  function toggle(id: string) {
    setDone((prev) => {
      const next = { ...prev, [id]: !prev[id] };
      try {
        window.localStorage.setItem(storageKey, JSON.stringify(next));
      } catch {
        // Ignore: the checklist still works for this page view.
      }
      return next;
    });
  }

  return (
    <div className="space-y-6">
      {phases.map((phase) => {
        const completed = phase.steps.filter((s) => done[s.id]).length;
        return (
          <Box key={phase.when}>
            <BoxHeader>
              <span className="font-semibold">{phase.title}</span>
              <Counter>
                {completed}/{phase.steps.length}
              </Counter>
            </BoxHeader>
            <ul className="divide-y divide-border">
              {phase.steps.map((step) => (
                <li key={step.id} id={`rollout-${step.id}`} className="flex gap-3 px-4 py-3">
                  <input
                    type="checkbox"
                    checked={Boolean(done[step.id])}
                    onChange={() => toggle(step.id)}
                    aria-label={`Mark "${step.title}" as done`}
                    className="mt-1 size-4 accent-[var(--success-emphasis)]"
                  />
                  <div className="min-w-0 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className={done[step.id] ? "font-medium text-fg-muted line-through" : "font-medium"}>
                        <InlineMarkdown>{step.title}</InlineMarkdown>
                      </span>
                      {step.blocking && <Label tone="danger">blocking</Label>}
                      <Label tone="accent">{step.owner}</Label>
                      {step.prompt && <CopyButton text={step.prompt} label="Copy agent prompt" className="ml-auto" />}
                    </div>
                    {step.description && <Markdown className="mt-1 text-fg-muted">{step.description}</Markdown>}
                  </div>
                </li>
              ))}
            </ul>
          </Box>
        );
      })}
    </div>
  );
}
