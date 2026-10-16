import { GitMergeIcon, GitPullRequestClosedIcon, GitPullRequestDraftIcon, GitPullRequestIcon, LinkExternalIcon } from "@primer/octicons-react";
import { Link } from "@/components/Link";
import type { TabId } from "@/lib/router";
import { TABS, tabHref } from "@/lib/router";
import type { PullRequestInfo } from "@/lib/types";
import { ReviewPanel } from "./ReviewComments";
import { Counter, cx } from "./ui";

function StateBadge({ pr }: { pr: PullRequestInfo }) {
  const meta = pr.draft
    ? { label: "Draft", icon: <GitPullRequestDraftIcon size={16} />, cls: "bg-neutral-emphasis" }
    : pr.state === "merged"
      ? { label: "Merged", icon: <GitMergeIcon size={16} />, cls: "bg-done-emphasis" }
      : pr.state === "closed"
        ? { label: "Closed", icon: <GitPullRequestClosedIcon size={16} />, cls: "bg-danger-emphasis" }
        : { label: "Open", icon: <GitPullRequestIcon size={16} />, cls: "bg-success-emphasis" };
  return (
    <span className={cx("inline-flex items-center gap-1 rounded-full px-3 py-1 text-sm font-medium text-white", meta.cls)}>
      {meta.icon}
      {meta.label}
    </span>
  );
}

export interface TabCounts {
  [tab: string]: { count?: number; alert?: boolean } | undefined;
}

export function PrHeader({ pr, active, counts }: { pr: PullRequestInfo; active: TabId; counts: TabCounts }) {
  return (
    <div className="border-b border-border bg-canvas">
      <div className="mx-auto max-w-[1280px] px-4 pt-4 md:px-6">
        <p className="text-sm text-fg-muted">
          {pr.owner} / <span className="font-semibold text-fg">{pr.repo}</span>
        </p>
        <h1 className="mt-2 text-2xl font-normal md:text-[32px] md:leading-tight">
          {pr.title} <span className="font-light text-fg-muted">#{pr.number}</span>
        </h1>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-fg-muted">
          <StateBadge pr={pr} />
          <span>
            <span className="font-semibold text-fg">{pr.author.login}</span> wants to merge {pr.commits} {pr.commits === 1 ? "commit" : "commits"} into{" "}
            <span className="rounded-md bg-accent-subtle px-1.5 py-0.5 font-mono text-xs text-accent">{pr.baseRef}</span> from{" "}
            <span className="rounded-md bg-accent-subtle px-1.5 py-0.5 font-mono text-xs text-accent">{pr.headRef}</span>
          </span>
          <span className="font-mono text-xs">
            <span className="text-success">+{pr.additions.toLocaleString("en-US")}</span>{" "}
            <span className="text-danger">−{pr.deletions.toLocaleString("en-US")}</span>
          </span>
          <span className="ml-auto flex items-center gap-4">
            {pr.htmlUrl && (
              <a href={pr.htmlUrl} className="inline-flex items-center gap-1 text-sm" target="_blank" rel="noreferrer">
                View on GitHub <LinkExternalIcon size={14} />
              </a>
            )}
            <ReviewPanel pr={pr} />
          </span>
        </div>
        <nav aria-label="Review sections" className="-mb-px mt-4 flex gap-1 overflow-x-auto">
          {TABS.map((tab) => {
            const c = counts[tab.id];
            const isActive = tab.id === active;
            return (
              <Link
                key={tab.id}
                href={tabHref(pr, tab.id)}
                aria-current={isActive ? "page" : undefined}
                className={cx(
                  "flex items-center gap-2 whitespace-nowrap border-b-2 px-3 py-2 text-sm text-fg hover:border-border",
                  isActive ? "border-severe font-semibold" : "border-transparent",
                )}
              >
                {tab.label}
                {c?.count !== undefined && <Counter tone={c.alert ? "danger" : "neutral"}>{c.count}</Counter>}
              </Link>
            );
          })}
        </nav>
      </div>
    </div>
  );
}
