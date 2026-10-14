import { HubotIcon } from "@primer/octicons-react";
import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { OpenOnHash } from "@/components/OpenOnHash";
import { PrHeader, type TabCounts } from "@/components/PrHeader";
import { ReportSources } from "@/components/ReportSources";
import { EmptyState } from "@/components/ui";
import { Briefing } from "@/components/views/Briefing";
import { Changes } from "@/components/views/Changes";
import { Decisions } from "@/components/views/Decisions";
import { Rollout } from "@/components/views/Rollout";
import { Security } from "@/components/views/Security";
import { Verification } from "@/components/views/Verification";
import { analyzeFocus } from "@/lib/focus/score";
import { isTab, type TabId } from "@/lib/refs";
import { GitHubConfigError, loadPullRequest } from "@/lib/sources";
import type { PullRequestBundle } from "@/lib/types";

type Params = { owner: string; repo: string; number: string };

type LoadError = { title: string; message: string };

async function load(params: Params): Promise<PullRequestBundle | LoadError> {
  const number = Number(params.number);
  if (!Number.isInteger(number) || number <= 0) notFound();
  try {
    return await loadPullRequest(params.owner, params.repo, number);
  } catch (err) {
    if (err instanceof GitHubConfigError) {
      return { title: "ReviewHub isn't connected to GitHub", message: `${err.message} Copy .env.example to .env.local, set GITHUB_TOKEN and restart the server.` };
    }
    const status = (err as { status?: number }).status;
    if (status === 404) notFound();
    if (status === 401) return { title: "GitHub rejected the token", message: "GITHUB_TOKEN is invalid or expired. Create a new token and restart the server." };
    if (status === 403) {
      return { title: "GitHub refused the request", message: "The token lacks access to this repository, or the API rate limit was hit. It needs read access to pull requests and contents." };
    }
    throw err;
  }
}

export async function generateMetadata({ params }: { params: Promise<Params> }): Promise<Metadata> {
  const p = await params;
  return { title: `${p.owner}/${p.repo}#${p.number}` };
}

export default async function PullRequestPage({ params, searchParams }: { params: Promise<Params>; searchParams: Promise<{ tab?: string }> }) {
  const [p, { tab }] = await Promise.all([params, searchParams]);
  const active: TabId = isTab(tab) ? tab : "briefing";
  const bundle = await load(p);

  if (!("pr" in bundle)) {
    return (
      <main className="mx-auto max-w-[1012px] px-4 py-10 md:px-6">
        <EmptyState title={bundle.title}>{bundle.message}</EmptyState>
      </main>
    );
  }

  const { pr, reports } = bundle;
  const manifest = reports.manifest?.data;
  const security = reports.security?.data;
  const metrics = reports.metrics?.data;
  const focus = analyzeFocus({ files: bundle.files, manifest, security, metrics });

  const serious = (security?.findings ?? []).filter((f) => f.severity === "critical" || f.severity === "high").length;
  const counts: TabCounts = {
    changes: { count: bundle.files.length },
    decisions: manifest ? { count: manifest.deviations.length + manifest.decisions.length } : undefined,
    verification: manifest ? { count: manifest.gaps.length, alert: manifest.validation.some((v) => v.result === "failed") } : undefined,
    security: security ? { count: security.findings.filter((f) => f.severity !== "info").length, alert: serious > 0 } : undefined,
    rollout: manifest ? { count: manifest.rollout.length } : undefined,
  };

  return (
    <>
      <PrHeader pr={pr} active={active} counts={counts} />
      <OpenOnHash />
      <main className="mx-auto max-w-[1280px] px-4 py-6 md:px-6">
        {active === "briefing" && (
          <div className="flex flex-col gap-8 lg:flex-row">
            <div className="min-w-0 flex-1">
              <Briefing bundle={bundle} focus={focus} />
            </div>
            <aside className="space-y-6 lg:w-72 lg:shrink-0">
              {manifest && (
                <div>
                  <h3 className="mb-2 text-xs font-semibold text-fg-muted">Authored by</h3>
                  <p className="flex items-center gap-2 text-sm">
                    <HubotIcon size={16} className="text-fg-muted" />
                    <span className="font-medium">{manifest.agent.name}</span>
                  </p>
                  {manifest.agent.sessionUrl && (
                    <a href={manifest.agent.sessionUrl} className="mt-1 block text-xs hover:underline">
                      Agent session
                    </a>
                  )}
                </div>
              )}
              <ReportSources reports={reports} />
              <div className="border-t border-border pt-4 text-xs text-fg-muted">
                Files are ranked by a focus score built from security findings, the agent&apos;s decisions and deviations, validation gaps, test coverage of the
                changed lines, complexity and sensitive paths. The agent can ask for a file to be skipped, but only independent signals can confirm that.
              </div>
            </aside>
          </div>
        )}
        {active === "changes" && <Changes pr={pr} focus={focus} />}
        {active === "decisions" && <Decisions pr={pr} manifest={manifest} />}
        {active === "verification" && <Verification pr={pr} manifest={manifest} metrics={metrics} focus={focus} />}
        {active === "security" && <Security pr={pr} report={security} error={reports.security?.error} />}
        {active === "rollout" && <Rollout pr={pr} manifest={manifest} />}
      </main>
    </>
  );
}
