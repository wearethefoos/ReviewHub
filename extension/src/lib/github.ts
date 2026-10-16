// Fetches the raw GitHub responses for a pull request, and submits reviews.
// What to make of the responses (which comments to trust, which reports are
// valid) is decided in Rust.
import { headRepo, reportFilePaths, type RawPullRequest } from "./core";
import type { CommentTarget } from "./types";

const API = "https://api.github.com";
const MAX_FILE_PAGES = 30; // GitHub returns at most 3000 files

export class GitHubError extends Error {
  constructor(
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

export interface PrRef {
  owner: string;
  repo: string;
  number: number;
}

/** Parses "https://github.com/o/r/pull/1", "o/r#1" or "o/r/pull/1". */
export function parsePrRef(input: string): PrRef | null {
  const text = input.trim();
  const match =
    /^(?:https?:\/\/[^/]+\/)?([\w.-]+)\/([\w.-]+)\/pulls?\/(\d+)/.exec(text) ?? /^([\w.-]+)\/([\w.-]+)#(\d+)$/.exec(text);
  return match ? { owner: match[1]!, repo: match[2]!, number: Number(match[3]) } : null;
}

export function formatPrRef(ref: PrRef): string {
  return `${ref.owner}/${ref.repo}#${ref.number}`;
}

type Fetch = typeof fetch;

export function createClient(token: string, fetchImpl: Fetch = fetch) {
  async function get(path: string, allow404 = false): Promise<unknown | null> {
    const headers: Record<string, string> = { Accept: "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28" };
    if (token) headers.Authorization = `Bearer ${token}`;
    const res = await fetchImpl(`${API}${path}`, { headers });
    if (res.status === 404 && allow404) return null;
    if (res.ok) return res.json();
    if (res.status === 401) throw new GitHubError("GitHub rejected the token. Update it in the ReviewHub options.", 401);
    if (res.status === 403 || res.status === 429) {
      const limited = res.headers.get("x-ratelimit-remaining") === "0";
      throw new GitHubError(
        limited
          ? token
            ? "GitHub's API rate limit is used up. Try again later."
            : "GitHub's rate limit for anonymous requests is used up. Add a token in the ReviewHub options."
          : "GitHub refused the request. The token may lack access to this repository.",
        res.status,
      );
    }
    if (res.status === 404) {
      throw new GitHubError(
        token ? "Pull request not found, or the token can't see it." : "Pull request not found. For private repositories, add a token in the ReviewHub options.",
        404,
      );
    }
    throw new GitHubError(`GitHub returned ${res.status} for ${path}`, res.status);
  }

  async function pages(path: string, maxPages: number): Promise<unknown[]> {
    const all: unknown[] = [];
    for (let page = 1; page <= maxPages; page++) {
      const items = (await get(`${path}?per_page=100&page=${page}`)) as unknown[];
      all.push(...items);
      if (items.length < 100) break;
    }
    return all;
  }

  async function fetchPullRequest(ref: PrRef, trustedAuthors: string[] = []): Promise<RawPullRequest> {
    const { owner, repo, number } = ref;
    const base = `/repos/${owner}/${repo}`;
    const [pull, files, comments] = await Promise.all([
      get(`${base}/pulls/${number}`),
      pages(`${base}/pulls/${number}/files`, MAX_FILE_PAGES),
      pages(`${base}/issues/${number}/comments`, 100),
    ]);
    const [headOwner, headName, sha] = await headRepo(pull, owner, repo);
    const reportFiles: Record<string, unknown> = {};
    await Promise.all(
      (await reportFilePaths()).map(async (path) => {
        const content = await get(`/repos/${headOwner}/${headName}/contents/${path}?ref=${sha}`, true);
        if (content) reportFiles[path] = content;
      }),
    );
    return { owner, repo, number, pull, files, comments, reportFiles, trustedAuthors };
  }

  /** Submits a review with its line comments in one request. Returns the review's URL. */
  async function submitReview(ref: PrRef, review: ReviewSubmission): Promise<string> {
    const { owner, repo, number } = ref;
    const res = await fetchImpl(`${API}/repos/${owner}/${repo}/pulls/${number}/reviews`, {
      method: "POST",
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
        ...(token && { Authorization: `Bearer ${token}` }),
      },
      body: JSON.stringify({
        commit_id: review.commitId,
        event: review.event,
        ...(review.body.trim() && { body: review.body }),
        comments: review.comments.map((c) => ({
          path: c.path,
          body: c.body,
          line: c.target.line,
          side: c.target.side,
          ...(c.target.startLine !== undefined && { start_line: c.target.startLine, start_side: c.target.startSide ?? c.target.side }),
        })),
      }),
    });
    if (res.ok) return ((await res.json()) as { html_url?: string }).html_url ?? "";
    if (res.status === 401) throw new GitHubError("GitHub rejected the token. Update it in the ReviewHub options.", 401);
    if (res.status === 403 || res.status === 404) {
      throw new GitHubError(
        "GitHub refused the review. The token needs Pull requests: Read and write access to this repository (see the ReviewHub options).",
        res.status,
      );
    }
    const details = (await res.json().catch(() => ({}))) as { message?: string; errors?: unknown[] };
    const errors = (details.errors ?? []).map((e) => (typeof e === "string" ? e : JSON.stringify(e))).join("; ");
    throw new GitHubError(`GitHub didn't accept the review: ${[details.message, errors].filter(Boolean).join(": ") || res.status}`, res.status);
  }

  /** The login the token belongs to. */
  async function viewer(): Promise<string> {
    return ((await get("/user")) as { login: string }).login;
  }

  /** The merge methods the repository allows, in GitHub's order. */
  async function mergeMethods(ref: PrRef): Promise<MergeMethod[]> {
    const repo = (await get(`/repos/${ref.owner}/${ref.repo}`)) as Record<string, unknown>;
    return (["merge", "squash", "rebase"] as const).filter((m) => repo[`allow_${m === "merge" ? "merge_commit" : `${m}_merge`}`] !== false);
  }

  /** Merges the pull request if its head is still `sha` (so nothing unreviewed gets merged). */
  async function merge(ref: PrRef, sha: string, method: MergeMethod): Promise<void> {
    const { owner, repo, number } = ref;
    const res = await fetchImpl(`${API}/repos/${owner}/${repo}/pulls/${number}/merge`, {
      method: "PUT",
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "Content-Type": "application/json",
        ...(token && { Authorization: `Bearer ${token}` }),
      },
      body: JSON.stringify({ sha, merge_method: method }),
    });
    if (res.ok) return;
    if (res.status === 401) throw new GitHubError("GitHub rejected the token. Update it in the ReviewHub options.", 401);
    if (res.status === 403 || res.status === 404) {
      throw new GitHubError(
        "GitHub refused the merge. The token needs Contents: Read and write access to this repository (see the ReviewHub options).",
        res.status,
      );
    }
    if (res.status === 409) throw new GitHubError("The pull request got new commits since you reviewed it. Reload it, then review the new commits.", 409);
    const details = (await res.json().catch(() => ({}))) as { message?: string };
    throw new GitHubError(`GitHub didn't merge the pull request: ${details.message ?? res.status}`, res.status);
  }

  return { get, fetchPullRequest, submitReview, viewer, mergeMethods, merge };
}

export type ReviewEvent = "COMMENT" | "APPROVE" | "REQUEST_CHANGES";

export type MergeMethod = "merge" | "squash" | "rebase";

export interface ReviewSubmission {
  commitId: string;
  event: ReviewEvent;
  body: string;
  comments: { path: string; target: CommentTarget; body: string }[];
}
