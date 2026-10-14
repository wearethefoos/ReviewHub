import { AlertIcon, BeakerIcon, CheckCircleFillIcon, GraphIcon, SkipIcon, XCircleFillIcon } from "@primer/octicons-react";
import { Link } from "@/components/Link";
import type { ReactNode } from "react";
import type { FocusAnalysis } from "@/lib/types";
import { fileAnchor, tabHref } from "@/lib/router";
import type { Gap, MetricsReport, ReviewManifest, Validation } from "@/lib/types";
import type { PullRequestInfo } from "@/lib/types";
import { CopyButton } from "../CopyButton";
import { FileRefs } from "../FileRefs";
import { InlineMarkdown, Markdown } from "../Markdown";
import { Box, BoxHeader, Code, EmptyState, Label, SectionHeading, TierLabel } from "../ui";

const RESULT_ICON: Record<Validation["result"], ReactNode> = {
  passed: <CheckCircleFillIcon size={16} className="text-success" />,
  failed: <XCircleFillIcon size={16} className="text-danger" />,
  partial: <AlertIcon size={16} className="text-attention" />,
  skipped: <SkipIcon size={16} className="text-fg-muted" />,
};

const RESULT_ORDER: Validation["result"][] = ["failed", "partial", "skipped", "passed"];

export function gapPrompt(gap: Gap, pr: PullRequestInfo): string {
  const files = gap.files.map((f) => f.path).join(", ");
  return [
    `Close this validation gap on ${pr.owner}/${pr.repo}#${pr.number} (branch ${pr.headRef}): ${gap.title}.`,
    `Why it is open: ${gap.why}`,
    `What to do: ${gap.howToValidate}`,
    files ? `Relevant files: ${files}.` : "",
    "Report what you ran and the result.",
  ]
    .filter(Boolean)
    .join("\n\n");
}

export function Verification({
  pr,
  manifest,
  metrics,
  focus,
}: {
  pr: PullRequestInfo;
  manifest?: ReviewManifest;
  metrics?: MetricsReport;
  focus: FocusAnalysis;
}) {
  const validations = [...(manifest?.validation ?? [])].sort((a, b) => RESULT_ORDER.indexOf(a.result) - RESULT_ORDER.indexOf(b.result));
  const coverage = metrics?.coverage;
  const quality = metrics?.quality;
  const metricRows = focus.files.filter(
    (f) => coverage?.files.some((c) => c.path === f.file.path) || quality?.files.some((q) => q.path === f.file.path) || f.signals.some((s) => s.kind === "coverage"),
  );

  return (
    <div className="space-y-10">
      <section>
        <SectionHeading icon={<CheckCircleFillIcon size={16} />} title="What the agent validated" count={validations.length} />
        {validations.length === 0 ? (
          <EmptyState title="No validation reported">The agent did not report running tests, builds or manual checks.</EmptyState>
        ) : (
          <Box>
            <ul className="divide-y divide-border">
              {validations.map((v) => (
                <li key={v.id} id={`validation-${v.id}`} className="px-4 py-3">
                  <div className="flex flex-wrap items-center gap-2">
                    {RESULT_ICON[v.result]}
                    <span className="font-medium">
                      <InlineMarkdown>{v.description}</InlineMarkdown>
                    </span>
                    <Label className="ml-auto">{v.kind}</Label>
                  </div>
                  {v.command && (
                    <p className="ml-6 mt-1 text-xs text-fg-muted">
                      <Code>{v.command}</Code>
                    </p>
                  )}
                  {v.evidence && (
                    <details className="ml-6 mt-2">
                      <summary className="text-xs text-accent hover:underline">Evidence</summary>
                      <pre className="mt-1 overflow-x-auto rounded-md bg-canvas-subtle px-3 py-2 font-mono text-xs">{v.evidence}</pre>
                    </details>
                  )}
                </li>
              ))}
            </ul>
          </Box>
        )}
      </section>

      <section>
        <SectionHeading icon={<AlertIcon size={16} />} title="What still needs validating" count={manifest?.gaps.length ?? 0} />
        <p className="-mt-1 mb-4 text-fg-muted">Things the agent could not verify, with the steps to close each gap.</p>
        {!manifest || manifest.gaps.length === 0 ? (
          <EmptyState title="No open validation gaps reported." />
        ) : (
          <div className="grid gap-4 lg:grid-cols-2">
            {manifest.gaps.map((g) => (
              <Box key={g.id} id={`gap-${g.id}`} className="flex flex-col border-l-4 border-l-attention">
                <BoxHeader>
                  <span className="font-semibold">
                    <InlineMarkdown>{g.title}</InlineMarkdown>
                  </span>
                </BoxHeader>
                <div className="flex-1 space-y-3 px-4 py-3">
                  <div className="flex flex-wrap gap-2">
                    <Label tone="accent">owner: {g.owner}</Label>
                    {g.effort && <Label>effort: {g.effort}</Label>}
                  </div>
                  <div>
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">Why it is open</p>
                    <Markdown>{g.why}</Markdown>
                  </div>
                  <div>
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">How to validate</p>
                    <Markdown>{g.howToValidate}</Markdown>
                  </div>
                  <FileRefs refs={g.files} pr={pr} />
                </div>
                {g.owner === "agent" && (
                  <div className="border-t border-border px-4 py-2">
                    <CopyButton text={gapPrompt(g, pr)} label="Copy as agent prompt" />
                  </div>
                )}
              </Box>
            ))}
          </div>
        )}
      </section>

      <section>
        <SectionHeading icon={<GraphIcon size={16} />} title="Coverage and quality" />
        {!metrics ? (
          <EmptyState icon={<BeakerIcon size={24} />} title="No metrics report">
            Publish coverage from CI with <Code>reviewhub metrics --lcov coverage/lcov.info</Code> to see which changed lines no test exercises.
          </EmptyState>
        ) : (
          <Box>
            {coverage?.lines != null && (
              <BoxHeader>
                <span>
                  Line coverage <span className="font-semibold">{coverage.lines.toFixed(1)}%</span>
                  {coverage.baseLines != null && (
                    <span className="text-fg-muted">
                      {" "}
                      on this branch, {coverage.baseLines.toFixed(1)}% on {pr.baseRef}
                    </span>
                  )}
                </span>
                {coverage.tool && <span className="ml-auto text-xs text-fg-muted">{coverage.tool}</span>}
              </BoxHeader>
            )}
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead className="border-b border-border text-left text-xs text-fg-muted">
                  <tr>
                    <th className="px-4 py-2 font-medium">File</th>
                    <th className="px-4 py-2 font-medium">Tier</th>
                    <th className="px-4 py-2 text-right font-medium">Coverage</th>
                    <th className="px-4 py-2 text-right font-medium">Uncovered changed lines</th>
                    <th className="px-4 py-2 text-right font-medium">Complexity</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {metricRows.map((f) => {
                    const cov = coverage?.files.find((c) => c.path === f.file.path);
                    const q = quality?.files.find((x) => x.path === f.file.path);
                    return (
                      <tr key={f.file.path}>
                        <td className="px-4 py-2">
                          <Link href={tabHref(pr, "changes", fileAnchor(f.file.path))} className="font-mono text-xs text-fg hover:text-accent hover:underline">
                            {f.file.path}
                          </Link>
                        </td>
                        <td className="px-4 py-2">
                          <TierLabel tier={f.tier} />
                        </td>
                        <td className="px-4 py-2 text-right font-mono text-xs">
                          {cov ? `${cov.lines.toFixed(0)}%` : <span className="text-danger">not run</span>}
                        </td>
                        <td className={`px-4 py-2 text-right font-mono text-xs ${f.uncoveredChangedLines > 0 ? "text-danger" : ""}`}>
                          {cov ? f.uncoveredChangedLines : "–"}
                        </td>
                        <td className="px-4 py-2 text-right font-mono text-xs">
                          {q?.complexity ?? "–"}
                          {q?.complexityDelta ? <span className="text-fg-muted"> (+{q.complexityDelta})</span> : null}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </Box>
        )}
      </section>
    </div>
  );
}
