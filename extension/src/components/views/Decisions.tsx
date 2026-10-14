import { GitBranchIcon, LightBulbIcon } from "@primer/octicons-react";
import type { ReviewManifest } from "@/lib/types";
import type { PullRequestInfo } from "@/lib/types";
import { CopyButton } from "../CopyButton";
import { FileRefs } from "../FileRefs";
import { InlineMarkdown, Markdown } from "../Markdown";
import { Box, BoxHeader, EmptyState, Label, SectionHeading } from "../ui";

const IMPACT_ORDER = ["high", "medium", "low"] as const;

export function Decisions({ pr, manifest }: { pr: PullRequestInfo; manifest?: ReviewManifest }) {
  if (!manifest) {
    return (
      <EmptyState icon={<LightBulbIcon size={24} />} title="No review manifest">
        Deviations and decisions come from the authoring agent&apos;s review manifest.
      </EmptyState>
    );
  }
  const decisions = [...manifest.decisions].sort((a, b) => IMPACT_ORDER.indexOf(a.impact) - IMPACT_ORDER.indexOf(b.impact));

  return (
    <div className="space-y-10">
      <section>
        <SectionHeading icon={<GitBranchIcon size={16} />} title="Deviations from the prompt" count={manifest.deviations.length} />
        <p className="-mt-1 mb-4 text-fg-muted">Places where the agent delivered something other than what was asked. Confirm each one is acceptable.</p>
        {manifest.deviations.length === 0 ? (
          <EmptyState title="The agent reports no deviations." />
        ) : (
          <div className="space-y-4">
            {manifest.deviations.map((d) => (
              <Box key={d.id} id={`deviation-${d.id}`} className={d.severity === "major" ? "border-l-4 border-l-severe" : ""}>
                <BoxHeader>
                  <span className="font-semibold">
                    <InlineMarkdown>{d.title}</InlineMarkdown>
                  </span>
                  <Label tone={d.severity === "major" ? "severe" : "neutral"} className="ml-auto">
                    {d.severity}
                  </Label>
                </BoxHeader>
                <div className="grid border-b border-border md:grid-cols-2">
                  <div className="border-border px-4 py-3 md:border-r">
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">Requested</p>
                    <Markdown>{d.requested}</Markdown>
                  </div>
                  <div className="border-t border-border px-4 py-3 md:border-t-0">
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">Delivered</p>
                    <Markdown>{d.delivered}</Markdown>
                  </div>
                </div>
                <div className="space-y-3 px-4 py-3">
                  <div>
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">Why</p>
                    <Markdown>{d.reason}</Markdown>
                  </div>
                  <FileRefs refs={d.files} pr={pr} />
                </div>
              </Box>
            ))}
          </div>
        )}
      </section>

      <section>
        <SectionHeading icon={<LightBulbIcon size={16} />} title="Decisions the agent made" count={decisions.length} />
        <p className="-mt-1 mb-4 text-fg-muted">
          Questions the prompt left open. High-impact decisions are expensive to change later; each alternative comes with a prompt you can hand to an agent.
        </p>
        {decisions.length === 0 ? (
          <EmptyState title="The agent reports no decisions of its own." />
        ) : (
          <div className="space-y-4">
            {decisions.map((d) => (
              <Box key={d.id} id={`decision-${d.id}`} className={d.impact === "high" ? "border-l-4 border-l-done" : ""}>
                <BoxHeader>
                  <span className="font-semibold">
                    <InlineMarkdown>{d.title}</InlineMarkdown>
                  </span>
                  <Label tone={d.impact === "high" ? "done" : d.impact === "medium" ? "accent" : "neutral"} className="ml-auto">
                    {d.impact} impact
                  </Label>
                </BoxHeader>
                <div className="space-y-3 px-4 py-3">
                  <div>
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">The open question</p>
                    <Markdown>{d.context}</Markdown>
                  </div>
                  <div className="rounded-md border border-done/40 bg-done-subtle px-3 py-2">
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-done">Chosen</p>
                    <Markdown>{d.chosen}</Markdown>
                    <Markdown className="mt-2 text-fg-muted">{d.rationale}</Markdown>
                  </div>
                  <FileRefs refs={d.files} pr={pr} />
                </div>
                {d.alternatives.length > 0 && (
                  <div className="border-t border-border">
                    <p className="px-4 pt-3 text-xs font-semibold uppercase tracking-wide text-fg-muted">Alternatives</p>
                    <ul className="divide-y divide-border">
                      {d.alternatives.map((alt, i) => (
                        <li key={i} className="px-4 py-3">
                          <div className="flex flex-wrap items-start gap-2">
                            <p className="font-semibold">{alt.title}</p>
                            {alt.prompt && <CopyButton text={alt.prompt} label="Copy prompt to switch" className="ml-auto" />}
                          </div>
                          <Markdown className="mt-1">{alt.description}</Markdown>
                          {alt.tradeoffs && <Markdown className="mt-1 text-fg-muted">{`**Trade-offs:** ${alt.tradeoffs}`}</Markdown>}
                        </li>
                      ))}
                    </ul>
                  </div>
                )}
              </Box>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
