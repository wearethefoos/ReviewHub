import { ShieldCheckIcon, ShieldIcon } from "@primer/octicons-react";
import { Link } from "@/components/Link";
import { splitDiff } from "@/lib/core";
import { fileAnchor, tabHref } from "@/lib/router";
import type { SecurityReport, Severity } from "@/lib/types";
import type { PullRequestInfo } from "@/lib/types";
import { CopyButton } from "../CopyButton";
import { PatchView } from "../FileDiff";
import { Markdown } from "../Markdown";
import { Box, BoxHeader, Code, EmptyState, Label, SectionHeading, SeverityLabel } from "../ui";

const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

function combinedPrompt(report: SecurityReport, pr: PullRequestInfo): string {
  const items = report.findings
    .filter((f) => f.severity !== "info")
    .sort((a, b) => SEVERITIES.indexOf(a.severity) - SEVERITIES.indexOf(b.severity))
    .map((f, i) => {
      const where = f.lines ? `${f.file}:${f.lines[0]}-${f.lines[1]}` : f.file;
      return `${i + 1}. [${f.severity}] ${f.title} (${where})\n   ${f.suggestion.prompt ?? f.suggestion.explanation}`;
    });
  return [
    `Fix these security findings on ${pr.owner}/${pr.repo}#${pr.number} (branch ${pr.headRef}), most severe first. Add a regression test for each fix.`,
    ...items,
  ].join("\n\n");
}

export function Security({ pr, report, error }: { pr: PullRequestInfo; report?: SecurityReport; error?: string }) {
  if (!report) {
    return (
      <EmptyState icon={<ShieldIcon size={24} />} title={error ? "The security report could not be read" : "No security report yet"}>
        {error ? (
          <pre className="mt-2 whitespace-pre-wrap text-left font-mono text-xs">{error}</pre>
        ) : (
          <>
            Run an independent security agent over this pull request, for example <Code>/reviewhub:security-review</Code> with the{" "}
            <a href="https://github.com/wearethefoos/ReviewHub#claude-code-plugin" target="_blank" rel="noreferrer">
              ReviewHub plugin
            </a>{" "}
            for Claude Code.
          </>
        )}
      </EmptyState>
    );
  }

  const findings = [...report.findings].sort((a, b) => SEVERITIES.indexOf(a.severity) - SEVERITIES.indexOf(b.severity));
  const counts = SEVERITIES.map((s) => [s, findings.filter((f) => f.severity === s).length] as const).filter(([, n]) => n > 0);

  return (
    <div className="space-y-6">
      <Box>
        <BoxHeader>
          <ShieldIcon size={16} className="text-fg-muted" />
          <span className="font-semibold">{report.scanner.name}</span>
          {report.commit && <span className="font-mono text-xs text-fg-muted">{report.commit.slice(0, 7)}</span>}
          {report.commit && report.commit !== pr.headSha && (
            <Label tone="attention" className="ml-auto">
              scanned an older commit
            </Label>
          )}
        </BoxHeader>
        <div className="space-y-3 px-4 py-3">
          {report.summary && <Markdown>{report.summary}</Markdown>}
          <div className="flex flex-wrap items-center gap-2">
            {counts.length === 0 ? (
              <span className="flex items-center gap-2 text-success">
                <ShieldCheckIcon size={16} /> No findings.
              </span>
            ) : (
              counts.map(([severity, n]) => (
                <span key={severity} className="flex items-center gap-1">
                  <SeverityLabel severity={severity} /> <span className="text-xs text-fg-muted">×{n}</span>
                </span>
              ))
            )}
            {findings.some((f) => f.severity !== "info") && <CopyButton text={combinedPrompt(report, pr)} label="Copy prompt to fix all" className="ml-auto" />}
          </div>
        </div>
      </Box>

      <section>
        <SectionHeading title="Findings" count={findings.length} />
        <div className="space-y-4">
          {findings.map((f) => {
            const patches = f.suggestion.diff ? splitDiff(f.suggestion.diff).map(([path, hunks]) => ({ path, hunks })) : [];
            return (
              <Box key={f.id} id={`security-${f.id}`}>
                <BoxHeader className="flex-wrap">
                  <SeverityLabel severity={f.severity} />
                  <span className="font-semibold">{f.title}</span>
                  <span className="ml-auto flex items-center gap-2 text-xs text-fg-muted">
                    {f.category && <Label>{f.category}</Label>}
                    <span>{f.confidence} confidence</span>
                  </span>
                </BoxHeader>
                <div className="space-y-3 px-4 py-3">
                  <Link href={tabHref(pr, "changes", fileAnchor(f.file))} className="font-mono text-xs hover:underline">
                    {f.file}
                    {f.lines && (f.lines[0] === f.lines[1] ? `:${f.lines[0]}` : `:${f.lines[0]}-${f.lines[1]}`)}
                  </Link>
                  <Markdown>{f.description}</Markdown>
                  <div className="rounded-md border border-success/40 bg-success-subtle px-3 py-2">
                    <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-success">Suggested fix</p>
                    <Markdown>{f.suggestion.explanation}</Markdown>
                  </div>
                  {patches.length > 0 && (
                    <div className="space-y-2">
                      {patches.map((p, i) => (
                        <div key={i}>
                          <p className="mb-1 font-mono text-xs text-fg-muted">{p.path}</p>
                          <PatchView hunks={p.hunks} />
                        </div>
                      ))}
                    </div>
                  )}
                  {f.suggestion.prompt && (
                    <details>
                      <summary className="text-xs text-accent hover:underline">Agent prompt</summary>
                      <pre className="mt-1 whitespace-pre-wrap rounded-md bg-canvas-subtle px-3 py-2 font-mono text-xs">{f.suggestion.prompt}</pre>
                    </details>
                  )}
                </div>
                {(f.suggestion.diff || f.suggestion.prompt) && (
                  <div className="flex flex-wrap gap-2 border-t border-border px-4 py-2">
                    {f.suggestion.diff && <CopyButton text={f.suggestion.diff.endsWith("\n") ? f.suggestion.diff : `${f.suggestion.diff}\n`} label="Copy patch (git apply)" />}
                    {f.suggestion.prompt && <CopyButton text={f.suggestion.prompt} label="Copy agent prompt" />}
                  </div>
                )}
              </Box>
            );
          })}
        </div>
      </section>
    </div>
  );
}
