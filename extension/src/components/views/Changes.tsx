import { Link } from "@/components/Link";
import type { FocusAnalysis, Tier } from "@/lib/types";
import { fileAnchor, tabHref } from "@/lib/router";
import type { PullRequestInfo } from "@/lib/types";
import { FileDiff } from "../FileDiff";
import { ReviewBudget } from "../ReviewBudget";
import { Counter, TIER_META, TierLabel } from "../ui";

const TIERS: Tier[] = ["focus", "skim", "skip"];

export function Changes({ pr, focus }: { pr: PullRequestInfo; focus: FocusAnalysis }) {
  return (
    <div className="flex gap-6">
      <nav aria-label="Files by review tier" className="sticky top-4 hidden max-h-[calc(100vh-2rem)] w-72 shrink-0 self-start overflow-y-auto lg:block">
        {TIERS.map((tier) => {
          const files = focus.files.filter((f) => f.tier === tier);
          if (files.length === 0) return null;
          return (
            <div key={tier} className="mb-4">
              <a href={`#tier-${tier}`} className="mb-1 flex items-center gap-2 text-xs font-semibold text-fg">
                <TierLabel tier={tier} /> <Counter>{files.length}</Counter>
              </a>
              <ul>
                {files.map((f) => (
                  <li key={f.file.path}>
                    <Link
                      href={tabHref(pr, "changes", fileAnchor(f.file.path))}
                      title={f.file.path}
                      className="flex items-center gap-2 rounded-md px-2 py-1 text-xs text-fg hover:bg-neutral-subtle"
                    >
                      <span className="min-w-0 flex-1 truncate font-mono">
                        {f.file.path}
                      </span>
                      <span className="font-mono text-fg-muted">{f.score}</span>
                    </Link>
                  </li>
                ))}
              </ul>
            </div>
          );
        })}
      </nav>

      <div className="min-w-0 flex-1 space-y-8">
        <ReviewBudget focus={focus} />
        {focus.danglingRefs.length > 0 && (
          <p className="rounded-md border border-attention bg-attention-subtle px-4 py-2 text-sm">
            The reports mention files that are not in this diff, so they may be out of date: {focus.danglingRefs.join(", ")}
          </p>
        )}
        {TIERS.map((tier) => {
          const files = focus.files.filter((f) => f.tier === tier);
          if (files.length === 0) return null;
          return (
            <section key={tier} id={`tier-${tier}`} className="space-y-3">
              <div className="flex items-baseline gap-2">
                <h2 className="text-base font-semibold">{TIER_META[tier].label}</h2>
                <Counter>{files.length}</Counter>
                <span className="text-fg-muted">{TIER_META[tier].description}</span>
              </div>
              {files.map((f) => (
                <FileDiff key={f.file.path} file={f} pr={pr} defaultOpen={tier !== "skip"} />
              ))}
            </section>
          );
        })}
      </div>
    </div>
  );
}
