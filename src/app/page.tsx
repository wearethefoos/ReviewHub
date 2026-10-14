import {
  AlertIcon,
  BeakerIcon,
  CheckCircleFillIcon,
  CommentDiscussionIcon,
  GitBranchIcon,
  GraphIcon,
  LightBulbIcon,
  RocketIcon,
  ShieldIcon,
} from "@primer/octicons-react";
import Link from "next/link";
import type { ReactNode } from "react";
import { Box, Code } from "@/components/ui";
import { prPath } from "@/lib/refs";
import { DEMO_NUMBER, DEMO_OWNER, DEMO_REPO, hasGitHubToken } from "@/lib/sources";

const FEATURES: { icon: ReactNode; title: string; body: string }[] = [
  { icon: <CommentDiscussionIcon size={16} />, title: "The original ask", body: "The prompt and follow-ups next to what was delivered." },
  { icon: <GitBranchIcon size={16} />, title: "Deviations", body: "Where the agent did something other than what was asked, and why." },
  { icon: <LightBulbIcon size={16} />, title: "Decisions", body: "Choices the agent made on its own, with alternatives and ready-to-use prompts." },
  { icon: <CheckCircleFillIcon size={16} />, title: "Validation", body: "What the agent ran to prove the change works, with evidence." },
  { icon: <AlertIcon size={16} />, title: "Gaps", body: "What could not be validated and how to close each gap." },
  { icon: <RocketIcon size={16} />, title: "Rollout", body: "What humans or other agents still need to do to ship it." },
  { icon: <ShieldIcon size={16} />, title: "Security review", body: "Findings from a separate agent, each with a patch or a fix prompt." },
  { icon: <GraphIcon size={16} />, title: "Focus ranking", body: "Coverage, complexity and risk signals decide which files you read closely." },
];

export default async function Home({ searchParams }: { searchParams: Promise<{ error?: string }> }) {
  const { error } = await searchParams;
  const tokenSet = hasGitHubToken();

  return (
    <main className="mx-auto max-w-[1012px] px-4 py-10 md:px-6">
      <h1 className="text-3xl font-semibold">Review what matters.</h1>
      <p className="mt-2 max-w-2xl text-base text-fg-muted">
        Agents write more code than people can read line by line. ReviewHub combines the authoring agent&apos;s own account of the work, an independent security
        review and test coverage into one view, so you spend your attention on the few parts that need it.
      </p>

      {error !== undefined && (
        <p className="mt-6 rounded-md border border-danger bg-danger-subtle px-4 py-2">
          Couldn&apos;t read <Code>{error || "(empty)"}</Code> as a pull request. Use <Code>owner/repo#123</Code> or a GitHub pull request URL.
        </p>
      )}

      <div className="mt-8 grid gap-4 md:grid-cols-2">
        <Box className="p-5">
          <h2 className="font-semibold">Try the demo</h2>
          <p className="mt-1 text-fg-muted">An agent-authored pull request that adds partial refunds to a payments API, with a manifest, security report and coverage.</p>
          <Link
            href={prPath({ owner: DEMO_OWNER, repo: DEMO_REPO, number: DEMO_NUMBER })}
            className="mt-4 inline-block rounded-md bg-success-emphasis px-4 py-1.5 font-medium text-white hover:opacity-90"
          >
            Open demo pull request
          </Link>
        </Box>
        <Box className="p-5">
          <h2 className="font-semibold">Open a pull request</h2>
          <form action="/open" className="mt-2 flex gap-2">
            <input
              name="pr"
              placeholder="owner/repo#123"
              aria-label="Pull request"
              className="min-w-0 flex-1 rounded-md border border-border bg-canvas px-3 py-1.5 focus:border-accent focus:outline-none"
            />
            <button type="submit" className="rounded-md border border-border bg-canvas-subtle px-4 py-1.5 font-medium hover:bg-neutral-subtle">
              Open
            </button>
          </form>
          <p className="mt-3 text-xs text-fg-muted">
            {tokenSet ? (
              <>Using the server&apos;s GitHub token.</>
            ) : (
              <>
                Set <Code>GITHUB_TOKEN</Code> in <Code>.env.local</Code> to load real pull requests.
              </>
            )}
          </p>
        </Box>
      </div>

      <h2 className="mb-3 mt-12 text-base font-semibold">What ReviewHub shows</h2>
      <ul className="grid gap-x-6 gap-y-4 sm:grid-cols-2">
        {FEATURES.map((f) => (
          <li key={f.title} className="flex gap-3">
            <span className="mt-0.5 text-fg-muted">{f.icon}</span>
            <span>
              <span className="font-semibold">{f.title}.</span> <span className="text-fg-muted">{f.body}</span>
            </span>
          </li>
        ))}
      </ul>

      <h2 className="mb-3 mt-12 text-base font-semibold">How it gets the data</h2>
      <Box className="p-5 text-fg-muted">
        <p>
          Agents publish three JSON reports as pull request comments (or as files under <Code>.reviewhub/</Code> on the branch):
        </p>
        <ul className="mt-2 list-disc space-y-1 pl-6">
          <li>
            <Code>manifest</Code>: written by the authoring agent: prompt, deviations, decisions, validation, gaps and rollout.
          </li>
          <li>
            <Code>security</Code>: written by a separate security agent that did not author the change.
          </li>
          <li>
            <Code>metrics</Code>: coverage and complexity from CI.
          </li>
        </ul>
        <p className="mt-3 flex items-center gap-2">
          <BeakerIcon size={16} /> The schemas, the agent instructions and the CLI are in the repository under <Code>docs/</Code>, <Code>plugin/</Code> (a Claude Code plugin) and{" "}
          <Code>cli/</Code>.
        </p>
      </Box>
    </main>
  );
}
