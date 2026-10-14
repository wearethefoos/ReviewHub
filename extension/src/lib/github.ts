// Fetches the raw GitHub responses for a pull request. What to make of them
// (which comments to trust, which reports are valid) is decided in Rust.
import { headRepo, reportFilePaths, type RawPullRequest } from "./core";

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

  return { get, fetchPullRequest };
}
