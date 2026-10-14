import { CodeReviewIcon, GearIcon, HubotIcon, SyncIcon } from "@primer/octicons-react";
import { type FormEvent, useEffect, useState } from "react";
import { Link } from "@/components/Link";
import { OpenOnHash } from "@/components/OpenOnHash";
import { PrHeader, type TabCounts } from "@/components/PrHeader";
import { ReportSources } from "@/components/ReportSources";
import { Code, EmptyState } from "@/components/ui";
import { Briefing } from "@/components/views/Briefing";
import { Changes } from "@/components/views/Changes";
import { Decisions } from "@/components/views/Decisions";
import { Rollout } from "@/components/views/Rollout";
import { Security } from "@/components/views/Security";
import { Verification } from "@/components/views/Verification";
import type { Review } from "@/lib/core";
import { formatPrRef, parsePrRef } from "@/lib/github";
import { navigate, routeHref, useRoute, type TabId } from "@/lib/router";
import { EXTENSION_URL, isExtension, loadSettings, openSettings } from "@/lib/settings";
import { useReview } from "./useReview";

const openOptions = openSettings;

function TopBar() {
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const input = new FormData(event.currentTarget).get("pr")?.toString() ?? "";
    const ref = parsePrRef(input);
    if (ref) navigate(routeHref({ pr: formatPrRef(ref) }));
  }
  return (
    <header className="bg-header text-header-fg">
      <div className="mx-auto flex max-w-[1280px] items-center gap-4 px-4 py-3 md:px-6">
        <Link href="?" className="flex items-center gap-2 font-semibold text-header-fg">
          <CodeReviewIcon size={24} />
          ReviewHub
        </Link>
        <form onSubmit={submit} className="ml-auto w-full max-w-sm">
          <input
            name="pr"
            placeholder="Open a pull request: owner/repo#123 or URL"
            aria-label="Pull request URL or owner/repo#number"
            className="w-full rounded-md border border-white/25 bg-transparent px-3 py-1 text-sm text-header-fg placeholder:text-white/60 focus:border-white/60 focus:outline-none"
          />
        </form>
        <a href="options.html" onClick={openOptions} title="Settings" className="text-header-fg/80 hover:text-header-fg">
          <GearIcon size={18} />
        </a>
      </div>
    </header>
  );
}

function Home() {
  const [hasToken, setHasToken] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    loadSettings().then((s) => setHasToken(Boolean(s.token)));
  }, []);

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const input = new FormData(event.currentTarget).get("pr")?.toString() ?? "";
    const ref = parsePrRef(input);
    if (ref) navigate(routeHref({ pr: formatPrRef(ref) }));
    else setError(input);
  }

  return (
    <main className="mx-auto max-w-[1012px] px-4 py-10 md:px-6">
      <h1 className="text-3xl font-semibold">Review what matters.</h1>
      <p className="mt-2 max-w-2xl text-base text-fg-muted">
        Agents write more code than people can read line by line. ReviewHub combines the authoring agent&apos;s own account of the work, an independent security
        review and test coverage into one view, so you spend your attention on the few parts that need it.
      </p>
      <div className="mt-8 grid gap-4 md:grid-cols-2">
        <div className="rounded-md border border-border p-5">
          <h2 className="font-semibold">Open a pull request</h2>
          <form onSubmit={submit} className="mt-2 flex gap-2">
            <input name="pr" placeholder="owner/repo#123" aria-label="Pull request" className="min-w-0 flex-1 rounded-md border border-border bg-canvas px-3 py-1.5 focus:border-accent focus:outline-none" />
            <button type="submit" className="rounded-md border border-border bg-canvas-subtle px-4 py-1.5 font-medium hover:bg-neutral-subtle">
              Open
            </button>
          </form>
          {error !== null && (
            <p className="mt-2 text-sm text-danger">
              Couldn&apos;t read <Code>{error || "(empty)"}</Code> as a pull request.
            </p>
          )}
          <p className="mt-3 text-xs text-fg-muted">
            {isExtension() ? (
              <>
                Or click <span className="font-semibold">Open in ReviewHub</span> on any pull request on github.com.{" "}
              </>
            ) : (
              <>
                On a computer, the{" "}
                <a href={EXTENSION_URL} target="_blank" rel="noreferrer">
                  Chrome extension
                </a>{" "}
                adds an Open in ReviewHub button to pull requests on github.com.{" "}
              </>
            )}
            {hasToken === false && (
              <>
                Public repositories work without a token;{" "}
                <a href="options.html" onClick={openOptions}>
                  add one
                </a>{" "}
                for private repositories and a higher rate limit.
              </>
            )}
          </p>
        </div>
        <div className="rounded-md border border-border p-5">
          <h2 className="font-semibold">Try the demo</h2>
          <p className="mt-1 text-fg-muted">An agent-authored pull request that adds partial refunds to a payments API, with a manifest, security report and coverage.</p>
          <Link href={routeHref({ demo: true })} className="mt-4 inline-block rounded-md bg-success-emphasis px-4 py-1.5 font-medium text-white hover:opacity-90">
            Open demo pull request
          </Link>
        </div>
      </div>
    </main>
  );
}

function tabCounts(review: Review): TabCounts {
  const { bundle } = review;
  const manifest = bundle.reports.manifest?.data;
  const security = bundle.reports.security?.data;
  const serious = (security?.findings ?? []).filter((f) => f.severity === "critical" || f.severity === "high").length;
  return {
    changes: { count: bundle.files.length },
    decisions: manifest ? { count: manifest.deviations.length + manifest.decisions.length } : undefined,
    verification: manifest ? { count: manifest.gaps.length, alert: manifest.validation.some((v) => v.result === "failed") } : undefined,
    security: security ? { count: security.findings.filter((f) => f.severity !== "info").length, alert: serious > 0 } : undefined,
    rollout: manifest ? { count: manifest.rollout.length } : undefined,
  };
}

function ReviewView({ review, tab, onReload }: { review: Review; tab: TabId; onReload: () => void }) {
  const { bundle, focus } = review;
  const { pr, reports } = bundle;
  const manifest = reports.manifest?.data;
  const security = reports.security?.data;
  const metrics = reports.metrics?.data;

  useEffect(() => {
    document.title = `${pr.title} · ${pr.owner}/${pr.repo}#${pr.number} · ReviewHub`;
  }, [pr]);

  return (
    <>
      <PrHeader pr={pr} active={tab} counts={tabCounts(review)} />
      <OpenOnHash />
      <main className="mx-auto max-w-[1280px] px-4 py-6 md:px-6">
        {tab === "briefing" && (
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
                    <a href={manifest.agent.sessionUrl} target="_blank" rel="noreferrer" className="mt-1 block text-xs hover:underline">
                      Agent session
                    </a>
                  )}
                </div>
              )}
              <ReportSources reports={reports} />
              <button type="button" onClick={onReload} className="flex items-center gap-1.5 text-xs text-accent hover:underline">
                <SyncIcon size={12} /> Reload from GitHub
              </button>
              <div className="border-t border-border pt-4 text-xs text-fg-muted">
                Files are ranked by a focus score built from security findings, the agent&apos;s decisions and deviations, validation gaps, test coverage of the
                changed lines, complexity and sensitive paths. The agent can ask for a file to be skipped, but only independent signals can confirm that.
              </div>
            </aside>
          </div>
        )}
        {tab === "changes" && <Changes pr={pr} focus={focus} />}
        {tab === "decisions" && <Decisions pr={pr} manifest={manifest} />}
        {tab === "verification" && <Verification pr={pr} manifest={manifest} metrics={metrics} focus={focus} />}
        {tab === "security" && <Security pr={pr} report={security} error={reports.security?.error ?? undefined} />}
        {tab === "rollout" && <Rollout pr={pr} manifest={manifest} />}
      </main>
    </>
  );
}

export function App() {
  const route = useRoute();
  const [state, reload] = useReview(route.pr, route.demo);

  return (
    <div className="min-h-screen">
      <TopBar />
      {state.status === "idle" && <Home />}
      {state.status === "loading" && <p className="mx-auto max-w-[1280px] px-6 py-10 text-fg-muted">Loading {route.demo ? "the demo" : route.pr} from GitHub…</p>}
      {state.status === "error" && (
        <main className="mx-auto max-w-[1012px] px-4 py-10 md:px-6">
          <EmptyState title={`Couldn't load ${state.key}`}>
            <p>{state.message}</p>
            <p className="mt-3 flex justify-center gap-4">
              <button type="button" onClick={reload} className="text-accent hover:underline">
                Try again
              </button>
              <a href="options.html" onClick={openOptions}>
                Settings
              </a>
            </p>
          </EmptyState>
        </main>
      )}
      {state.status === "ready" && <ReviewView review={state.review} tab={route.tab} onReload={reload} />}
    </div>
  );
}
