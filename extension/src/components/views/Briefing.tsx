import {
  AlertIcon,
  CheckCircleFillIcon,
  CommentDiscussionIcon,
  FileDiffIcon,
  GitBranchIcon,
  LightBulbIcon,
  PersonIcon,
  RocketIcon,
  ShieldIcon,
  XCircleFillIcon,
} from "@primer/octicons-react";
import { Link } from "@/components/Link";
import type { ReactNode } from "react";
import type { FocusAnalysis } from "@/lib/types";
import { fileAnchor, tabHref } from "@/lib/router";
import type { Severity } from "@/lib/types";
import type { PullRequestBundle } from "@/lib/types";
import { InlineMarkdown, Markdown } from "../Markdown";
import { ReviewBudget } from "../ReviewBudget";
import { Box, BoxHeader, Counter, EmptyState, Label, SectionHeading, SeverityLabel, TierLabel } from "../ui";

const SEVERITIES: Severity[] = ["critical", "high", "medium", "low", "info"];

function AttentionCard({
  icon,
  title,
  count,
  href,
  tone,
  children,
}: {
  icon: ReactNode;
  title: string;
  count: number;
  href: string;
  tone: "danger" | "severe" | "done" | "attention" | "accent";
  children: ReactNode;
}) {
  const border = {
    danger: "border-t-danger",
    severe: "border-t-severe",
    done: "border-t-done",
    attention: "border-t-attention",
    accent: "border-t-accent",
  }[tone];
  return (
    <Box className={`border-t-4 ${border}`}>
      <div className="flex items-center gap-2 px-4 pb-1 pt-3">
        <span className="text-fg-muted">{icon}</span>
        <Link href={href} className="font-semibold text-fg hover:text-accent hover:underline">
          {title}
        </Link>
        <Counter>{count}</Counter>
      </div>
      <div className="px-4 pb-3 text-sm">{children}</div>
    </Box>
  );
}

function ItemList({ items }: { items: { key: string; href: string; label: ReactNode; meta?: ReactNode }[] }) {
  if (items.length === 0) return <p className="text-fg-muted">None.</p>;
  return (
    <ul className="space-y-1.5">
      {items.map((item) => (
        <li key={item.key} className="flex items-start gap-2">
          <Link href={item.href} className="min-w-0 flex-1 text-fg hover:text-accent hover:underline">
            {item.label}
          </Link>
          {item.meta}
        </li>
      ))}
    </ul>
  );
}

export function Briefing({ bundle, focus }: { bundle: PullRequestBundle; focus: FocusAnalysis }) {
  const { pr } = bundle;
  const manifest = bundle.reports.manifest?.data;
  const security = bundle.reports.security?.data;
  const metrics = bundle.reports.metrics?.data;
  const focusFiles = focus.files.filter((f) => f.tier === "focus");

  const findings = [...(security?.findings ?? [])].sort((a, b) => SEVERITIES.indexOf(a.severity) - SEVERITIES.indexOf(b.severity));
  const blocking = findings.filter((f) => f.severity === "critical" || f.severity === "high");
  const decisions = [...(manifest?.decisions ?? [])].sort((a, b) => ["high", "medium", "low"].indexOf(a.impact) - ["high", "medium", "low"].indexOf(b.impact));
  const validations = manifest?.validation ?? [];
  const passed = validations.filter((v) => v.result === "passed").length;
  const notPassed = validations.filter((v) => v.result !== "passed");
  const rolloutBlocking = (manifest?.rollout ?? []).filter((r) => r.blocking);
  const coverage = metrics?.coverage;

  return (
    <div className="space-y-8">
      {manifest ? (
        <section>
          <SectionHeading icon={<CommentDiscussionIcon size={16} />} title="What was asked" />
          <div className="flex gap-3">
            <div className="mt-1 flex size-8 shrink-0 items-center justify-center rounded-full bg-neutral-subtle text-fg-muted">
              <PersonIcon size={16} />
            </div>
            <Box className="min-w-0 flex-1">
              <BoxHeader>
                <span className="font-semibold">The request</span>
                <span className="text-xs text-fg-muted">as summarized by {manifest.agent.name}</span>
                {manifest.prompt.source && (
                  <a href={manifest.prompt.source} className="ml-auto text-xs hover:underline">
                    Source
                  </a>
                )}
              </BoxHeader>
              <div className="px-4 py-3">
                <Markdown>{manifest.prompt.original}</Markdown>
              </div>
              {manifest.prompt.followUps.map((text, i) => (
                <div key={i} className="border-t border-border px-4 py-3">
                  <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-fg-muted">Follow-up {i + 1}</p>
                  <Markdown>{text}</Markdown>
                </div>
              ))}
            </Box>
          </div>
          <div className="mt-4 flex gap-3">
            <div className="mt-1 flex size-8 shrink-0 items-center justify-center rounded-full bg-done-subtle text-done">
              <LightBulbIcon size={16} />
            </div>
            <Box className="min-w-0 flex-1">
              <BoxHeader>
                <span className="font-semibold">What {manifest.agent.name} delivered</span>
                {manifest.agent.model && <Label>{manifest.agent.model}</Label>}
              </BoxHeader>
              <div className="px-4 py-3">
                <Markdown>{manifest.summary}</Markdown>
              </div>
            </Box>
          </div>
        </section>
      ) : (
        <EmptyState icon={<CommentDiscussionIcon size={24} />} title="No review manifest on this pull request">
          Without a manifest, ReviewHub ranks files using coverage, security findings and path heuristics only. With Claude Code, the{" "}
          <a href="https://github.com/wearethefoos/ReviewHub#claude-code-plugin" target="_blank" rel="noreferrer">
            ReviewHub plugin
          </a>{" "}
          publishes one with every pull request; other agents can follow{" "}
          <a href="https://github.com/wearethefoos/ReviewHub/blob/main/docs/reports.md" target="_blank" rel="noreferrer">
            the report format
          </a>
          .
        </EmptyState>
      )}

      <section>
        <SectionHeading icon={<FileDiffIcon size={16} />} title="Where to spend your review" />
        <ReviewBudget focus={focus} />
        {focusFiles.length > 0 && (
          <Box className="mt-4">
            <BoxHeader>
              <span className="font-semibold">Start here</span>
              <span className="text-xs text-fg-muted">sorted by focus score</span>
            </BoxHeader>
            <ul className="divide-y divide-border">
              {focusFiles.map((f) => (
                <li key={f.file.path} className="px-4 py-2.5">
                  <div className="flex items-center gap-3">
                    <TierLabel tier={f.tier} />
                    <Link href={tabHref(pr, "changes", fileAnchor(f.file.path))} className="min-w-0 truncate font-mono text-xs font-semibold text-fg hover:text-accent hover:underline">
                      {f.file.path}
                    </Link>
                    <span className="ml-auto rounded-md bg-neutral-subtle px-1.5 font-mono text-xs font-semibold" title="Focus score (0–100)">
                      {f.score}
                    </span>
                  </div>
                  <p className="mt-1 text-xs text-fg-muted">
                    <InlineMarkdown>
                      {f.signals
                        .filter((s) => s.weight > 0 && s.kind !== "role" && s.kind !== "size")
                        .slice(0, 3)
                        .map((s) => s.label)
                        .join(" · ")}
                    </InlineMarkdown>
                  </p>
                </li>
              ))}
            </ul>
          </Box>
        )}
      </section>

      <section>
        <SectionHeading icon={<AlertIcon size={16} />} title="Needs your judgement" />
        <div className="grid gap-4 md:grid-cols-2">
          <AttentionCard icon={<ShieldIcon size={16} />} title="Security findings" count={findings.length} href={tabHref(pr, "security")} tone="danger">
            {security ? (
              <ItemList
                items={findings.slice(0, 4).map((f) => ({
                  key: f.id,
                  href: tabHref(pr, "security", `security-${f.id}`),
                  label: f.title,
                  meta: <SeverityLabel severity={f.severity} />,
                }))}
              />
            ) : (
              <p className="text-fg-muted">No security report yet.</p>
            )}
            {blocking.length > 0 && (
              <p className="mt-2 text-xs font-semibold text-danger">
                {blocking.length} critical or high finding{blocking.length === 1 ? "" : "s"} should be fixed before merging.
              </p>
            )}
          </AttentionCard>

          <AttentionCard icon={<GitBranchIcon size={16} />} title="Deviations from the prompt" count={manifest?.deviations.length ?? 0} href={tabHref(pr, "decisions")} tone="severe">
            <ItemList
              items={(manifest?.deviations ?? []).map((d) => ({
                key: d.id,
                href: tabHref(pr, "decisions", `deviation-${d.id}`),
                label: <InlineMarkdown>{d.title}</InlineMarkdown>,
                meta: <Label tone={d.severity === "major" ? "severe" : "neutral"}>{d.severity}</Label>,
              }))}
            />
          </AttentionCard>

          <AttentionCard icon={<LightBulbIcon size={16} />} title="Decisions the agent made" count={decisions.length} href={tabHref(pr, "decisions")} tone="done">
            <ItemList
              items={decisions.map((d) => ({
                key: d.id,
                href: tabHref(pr, "decisions", `decision-${d.id}`),
                label: <InlineMarkdown>{d.title}</InlineMarkdown>,
                meta: <Label tone={d.impact === "high" ? "done" : "neutral"}>{d.impact} impact</Label>,
              }))}
            />
          </AttentionCard>

          <AttentionCard icon={<AlertIcon size={16} />} title="Not validated by the agent" count={manifest?.gaps.length ?? 0} href={tabHref(pr, "verification")} tone="attention">
            <ItemList
              items={(manifest?.gaps ?? []).map((g) => ({
                key: g.id,
                href: tabHref(pr, "verification", `gap-${g.id}`),
                label: <InlineMarkdown>{g.title}</InlineMarkdown>,
                meta: <Label>{g.owner}</Label>,
              }))}
            />
          </AttentionCard>
        </div>
      </section>

      <section className="grid gap-4 md:grid-cols-2">
        <Box>
          <BoxHeader>
            <CheckCircleFillIcon size={16} className="text-success" />
            <Link href={tabHref(pr, "verification")} className="font-semibold text-fg hover:text-accent hover:underline">
              Validation
            </Link>
          </BoxHeader>
          <div className="space-y-2 px-4 py-3">
            {validations.length === 0 ? (
              <p className="text-fg-muted">The agent did not report any validation.</p>
            ) : (
              <>
                <p>
                  <span className="font-semibold">{passed}</span> of {validations.length} checks passed.
                </p>
                <ul className="space-y-1 text-sm">
                  {notPassed.map((v) => (
                    <li key={v.id} className="flex items-start gap-2">
                      {v.result === "failed" ? (
                        <XCircleFillIcon size={16} className="mt-0.5 shrink-0 text-danger" />
                      ) : (
                        <AlertIcon size={16} className="mt-0.5 shrink-0 text-attention" />
                      )}
                      <span>
                        <span className="font-medium">{v.result}:</span> {v.description}
                      </span>
                    </li>
                  ))}
                </ul>
              </>
            )}
            {coverage?.lines != null && (
              <p className="border-t border-border pt-2">
                Line coverage <span className="font-semibold">{coverage.lines.toFixed(1)}%</span>
                {coverage.baseLines != null && (
                  <span className={coverage.lines < coverage.baseLines ? "text-danger" : "text-success"}>
                    {" "}
                    ({coverage.lines >= coverage.baseLines ? "+" : ""}
                    {(coverage.lines - coverage.baseLines).toFixed(1)} vs {pr.baseRef})
                  </span>
                )}
              </p>
            )}
          </div>
        </Box>
        <Box>
          <BoxHeader>
            <RocketIcon size={16} className="text-fg-muted" />
            <Link href={tabHref(pr, "rollout")} className="font-semibold text-fg hover:text-accent hover:underline">
              Before this ships
            </Link>
          </BoxHeader>
          <div className="px-4 py-3">
            {rolloutBlocking.length === 0 ? (
              <p className="text-fg-muted">No blocking rollout steps reported.</p>
            ) : (
              <ItemList
                items={rolloutBlocking.map((r) => ({
                  key: r.id,
                  href: tabHref(pr, "rollout", `rollout-${r.id}`),
                  label: <InlineMarkdown>{r.title}</InlineMarkdown>,
                  meta: <Label>{r.when.replace("-", " ")}</Label>,
                }))}
              />
            )}
          </div>
        </Box>
      </section>
    </div>
  );
}
