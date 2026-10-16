// Test doubles: the demo pull request served as GitHub's API would, and the bits of `chrome.*` the extension uses.
import { readFileSync } from "node:fs";
import demo from "../../../fixtures/demo/bundle.json";
import { loadCore } from "@/lib/core";

export { demo };

export function loadWasm() {
  return loadCore(readFileSync("src/wasm/reviewhub_wasm_bg.wasm"));
}

export interface Call {
  url: string;
  method: string;
  headers: Record<string, string>;
  body?: unknown;
}

function reportComment(id: number, kind: string, report: unknown) {
  return {
    id,
    html_url: `https://github.com/o/r/pull/7#issuecomment-${id}`,
    body: `<!-- reviewhub:${kind} -->\n\`\`\`json\n${JSON.stringify(report)}\n\`\`\``,
    user: { login: "agent-author" },
    author_association: "OWNER",
    updated_at: "2026-10-01T00:00:00Z",
  };
}

/** A `fetch` serving the demo as o/r#7 (and 404 for anything else), recording every call. `routes` override it. */
export function fakeGitHub(routes: Record<string, (call: Call) => Response> = {}) {
  const calls: Call[] = [];
  const base = "https://api.github.com/repos/o/r";
  const json = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { "content-type": "application/json" } });
  const pull = {
    number: 7,
    title: demo.pr.title,
    body: demo.pr.body,
    state: "open",
    draft: false,
    user: { login: "agent-author", avatar_url: "" },
    base: { ref: "main" },
    head: { ref: "feature", sha: demo.pr.headSha, repo: { name: "r", owner: { login: "o" } } },
    html_url: "https://github.com/o/r/pull/7",
    created_at: "2026-10-01T00:00:00Z",
    additions: demo.pr.additions,
    deletions: demo.pr.deletions,
    changed_files: demo.files.length,
    commits: 1,
  };
  const files = demo.files.map((f) => ({ filename: f.path, status: f.status, additions: f.additions, deletions: f.deletions, patch: f.patch }));
  const comments = (["manifest", "security", "metrics"] as const).map((kind, i) => reportComment(i + 1, kind, demo.reports[kind].data));
  const defaults: Record<string, (call: Call) => Response> = {
    [`GET ${base}/pulls/7`]: () => json(pull),
    [`GET ${base}/pulls/7/files?per_page=100&page=1`]: () => json(files),
    [`GET ${base}/issues/7/comments?per_page=100&page=1`]: () => json(comments),
  };
  const fetch = (async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input);
    const method = init?.method ?? "GET";
    const call: Call = { url, method, headers: { ...(init?.headers as Record<string, string>) }, body: init?.body ? JSON.parse(String(init.body)) : undefined };
    calls.push(call);
    const handler = routes[`${method} ${url}`] ?? defaults[`${method} ${url}`];
    return handler ? handler(call) : json({ message: "Not Found" }, 404);
  }) as typeof globalThis.fetch;
  return { fetch, calls, json };
}

/** A `chrome` with in-memory storage, as an extension page sees it. */
export function fakeChrome(stored: Record<string, unknown> = {}, installType = "normal") {
  const store: Record<string, unknown> = { ...stored };
  const pick = (keys: string | string[]) => Object.fromEntries((Array.isArray(keys) ? keys : [keys]).filter((k) => k in store).map((k) => [k, store[k]]));
  const chrome = {
    runtime: { id: "test", getManifest: () => ({ version: "0.6.0" }), openOptionsPage: () => {} },
    storage: { local: { get: async (keys: string | string[]) => pick(keys), set: async (items: Record<string, unknown>) => void Object.assign(store, items) } },
    management: { getSelf: async () => ({ installType }) },
  };
  return { chrome, store };
}
