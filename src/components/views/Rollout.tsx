import { RocketIcon } from "@primer/octicons-react";
import type { ReviewManifest, RolloutStep } from "@/lib/schema";
import type { PullRequestInfo } from "@/lib/types";
import { RolloutChecklist } from "../RolloutChecklist";
import { EmptyState } from "../ui";

const PHASES: { when: RolloutStep["when"]; title: string }[] = [
  { when: "before-merge", title: "Before merging" },
  { when: "after-merge", title: "After merging" },
  { when: "after-deploy", title: "After deploying" },
];

export function Rollout({ pr, manifest }: { pr: PullRequestInfo; manifest?: ReviewManifest }) {
  const steps = manifest?.rollout ?? [];
  if (steps.length === 0) {
    return (
      <EmptyState icon={<RocketIcon size={24} />} title="No rollout steps reported">
        The agent did not list anything that has to happen for this change to ship.
      </EmptyState>
    );
  }
  const phases = PHASES.map((p) => ({ ...p, steps: steps.filter((s) => s.when === p.when) })).filter((p) => p.steps.length > 0);
  return <RolloutChecklist storageKey={`reviewhub:rollout:${pr.owner}/${pr.repo}#${pr.number}`} phases={phases} />;
}
