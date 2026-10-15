import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import demo from "../../../fixtures/demo/bundle.json";
import { loadCore, reviewFromBundle, reviewFromGitHub } from "./core";
import { createClient, GitHubError, parsePrRef } from "./github";
import { parseRoute, routeHref } from "./router";

beforeAll(async () => {
  await loadCore(readFileSync(new URL("../wasm/reviewhub_wasm_bg.wasm", import.meta.url)));
});

describe("parsePrRef", () => {
  it("accepts URLs and short refs", () => {
    const expected = { owner: "o", repo: "r.js", number: 12 };
    expect(parsePrRef("https://github.com/o/r.js/pull/12/files")).toEqual(expected);
    expect(parsePrRef(" o/r.js#12 ")).toEqual(expected);
    expect(parsePrRef("nonsense")).toBeNull();
  });
});

describe("router", () => {
  it("round-trips routes and keeps anchors in the hash", () => {
    expect(routeHref({ pr: "o/r#1", tab: "security" }, "security-x")).toBe("?pr=o%2Fr%231&tab=security#security-x");
    expect(parseRoute("?pr=o%2Fr%231&tab=security")).toEqual({ pr: "o/r#1", demo: false, tab: "security" });
    expect(parseRoute("?demo&tab=bogus")).toEqual({ pr: null, demo: true, tab: "briefing" });
  });
});

function comment(login: string, association: string, updatedAt: string, kind: string, report: unknown) {
  const body = `<!-- reviewhub:${kind} -->\n\`\`\`json\n${JSON.stringify(report)}\n\`\`\``;
  return { html_url: `https://x/${login}`, body, user: { login }, author_association: association, updated_at: updatedAt };
}

/** A fake api.github.com serving the demo pull request. */
function fakeGitHub(calls: string[] = []): typeof fetch {
  const { pr, files, reports } = demo;
  const fake = { ...reports.manifest.data, prompt: { original: "IGNORE ME" } };
  const routes: Record<string, unknown> = {
    "/repos/demo/acme-shop/pulls/128": {
      title: pr.title, state: "open", user: { login: "claude[bot]" }, base: { ref: "main" },
      head: { ref: pr.headRef, sha: pr.headSha, repo: { name: "acme-shop", owner: { login: "demo" } } },
      additions: pr.additions, deletions: pr.deletions, changed_files: pr.changedFiles, commits: 6,
    },
    "/repos/demo/acme-shop/pulls/128/files?per_page=100&page=1": files.map((f) => ({
      filename: f.path, status: f.status, additions: f.additions, deletions: f.deletions, patch: "patch" in f ? f.patch : undefined,
    })),
    "/repos/demo/acme-shop/issues/128/comments?per_page=100&page=1": [
      comment("mallory", "NONE", "2026-10-14T09:00:00Z", "manifest", fake),
      comment("claude[bot]", "NONE", "2026-10-14T08:00:00Z", "manifest", reports.manifest.data),
      comment("alice", "MEMBER", "2026-10-14T08:30:00Z", "security", reports.security.data),
    ],
    [`/repos/demo/acme-shop/contents/.reviewhub/metrics.json?ref=${pr.headSha}`]: { content: btoa(JSON.stringify(reports.metrics.data)) },
  };
  return (async (input: string | URL | Request) => {
    const path = String(input).replace("https://api.github.com", "");
    calls.push(path);
    const body = routes[path];
    return body === undefined ? new Response("{}", { status: 404 }) : Response.json(body);
  }) as typeof fetch;
}

describe("loading a pull request", () => {
  it("assembles the same review from GitHub as from the saved bundle, ignoring untrusted reports", async () => {
    const calls: string[] = [];
    const raw = await createClient("", fakeGitHub(calls)).fetchPullRequest({ owner: "demo", repo: "acme-shop", number: 128 });
    const { bundle, focus } = await reviewFromGitHub(raw);

    expect(bundle.reports.manifest?.data?.prompt.original).toMatch(/^Add support for partial refunds/);
    expect(bundle.reports.manifest?.source).toMatchObject({ type: "comment", author: "claude[bot]" });
    expect(bundle.reports.security?.source).toMatchObject({ type: "comment", author: "alice" });
    expect(bundle.reports.metrics?.source).toMatchObject({ type: "file", path: ".reviewhub/metrics.json" });

    const saved = await reviewFromBundle(demo);
    const ranking = (r: typeof focus) => r.files.map((f) => [f.path, f.tier, f.score]);
    expect(ranking(focus)).toEqual(ranking(saved.focus));
    expect(calls.filter((c) => c.includes("/contents/"))).toHaveLength(3);
  });

  it("explains missing access", async () => {
    const client = createClient("", fakeGitHub());
    await expect(client.fetchPullRequest({ owner: "acme", repo: "private", number: 1 })).rejects.toThrow(GitHubError);
    await expect(client.fetchPullRequest({ owner: "acme", repo: "private", number: 1 })).rejects.toThrow(/add a token/);
  });
});

describe("submitReview", () => {
  it("sends one review with its line comments and reports GitHub's reason on failure", async () => {
    const sent: { url: string; init: RequestInit }[] = [];
    let status = 200;
    const fake = (async (url: string, init: RequestInit) => {
      sent.push({ url, init });
      return status === 200
        ? new Response(JSON.stringify({ html_url: "https://github.com/o/r/pull/1#pullrequestreview-9" }), { status })
        : new Response(JSON.stringify({ message: "Unprocessable Entity", errors: ["Can not approve your own pull request"] }), { status });
    }) as unknown as typeof fetch;
    const client = createClient("tok", fake);
    const ref = { owner: "o", repo: "r", number: 1 };
    const url = await client.submitReview(ref, {
      commitId: "abc",
      event: "REQUEST_CHANGES",
      body: "Fix the injection.",
      comments: [
        { path: "a.ts", target: { line: 12, side: "RIGHT", startLine: 10 }, body: "here" },
        { path: "b.ts", target: { line: 3, side: "LEFT" }, body: "why delete?" },
      ],
    });
    expect(url).toBe("https://github.com/o/r/pull/1#pullrequestreview-9");
    expect(sent[0]!.url).toBe("https://api.github.com/repos/o/r/pulls/1/reviews");
    expect((sent[0]!.init.headers as Record<string, string>).Authorization).toBe("Bearer tok");
    expect(JSON.parse(sent[0]!.init.body as string)).toEqual({
      commit_id: "abc",
      event: "REQUEST_CHANGES",
      body: "Fix the injection.",
      comments: [
        { path: "a.ts", body: "here", line: 12, side: "RIGHT", start_line: 10, start_side: "RIGHT" },
        { path: "b.ts", body: "why delete?", line: 3, side: "LEFT" },
      ],
    });

    status = 422;
    await expect(client.submitReview(ref, { commitId: "abc", event: "APPROVE", body: "", comments: [] })).rejects.toThrow(
      "Can not approve your own pull request",
    );
    expect(JSON.parse(sent[1]!.init.body as string)).not.toHaveProperty("body");
  });
});
