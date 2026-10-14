import "server-only";
import { Octokit } from "@octokit/rest";
import { parseReport, REPORT_KINDS, type ReportKind } from "../schema";
import type { ChangedFile, FileStatus, PullRequestBundle, ReportEnvelope } from "../types";
import { extractReport } from "./extract";

/**
 * Comment authors whose reports are trusted. Anyone else could post a fake
 * manifest that steers reviewers away from code. Bots are not trusted as a
 * class because some of them echo other people's comments.
 */
const TRUSTED_ASSOCIATIONS = new Set(["OWNER", "MEMBER", "COLLABORATOR"]);
const TRUSTED_BOTS = new Set(["github-actions[bot]"]);
const MAX_FILES = 3000;

export class GitHubConfigError extends Error {}

export function hasGitHubToken(): boolean {
  return Boolean(process.env.GITHUB_TOKEN);
}

function client(): Octokit {
  const auth = process.env.GITHUB_TOKEN;
  if (!auth) throw new GitHubConfigError("GITHUB_TOKEN is not set. Add it to .env.local to load pull requests from GitHub.");
  return new Octokit({ auth, baseUrl: process.env.GITHUB_API_URL || undefined, userAgent: "reviewhub" });
}

function extraTrustedAuthors(): Set<string> {
  return new Set(
    (process.env.REVIEWHUB_TRUSTED_AUTHORS ?? "")
      .split(",")
      .map((s) => s.trim().toLowerCase())
      .filter(Boolean),
  );
}

export async function fetchPullRequestBundle(owner: string, repo: string, number: number): Promise<PullRequestBundle> {
  const octokit = client();
  const { data: pr } = await octokit.pulls.get({ owner, repo, pull_number: number });

  const [rawFiles, comments] = await Promise.all([
    octokit.paginate(octokit.pulls.listFiles, { owner, repo, pull_number: number, per_page: 100 }, (res, done) => {
      if (res.data.length < 100) done();
      return res.data;
    }),
    octokit.paginate(octokit.issues.listComments, { owner, repo, issue_number: number, per_page: 100 }),
  ]);

  const files: ChangedFile[] = rawFiles.slice(0, MAX_FILES).map((f) => ({
    path: f.filename,
    previousPath: f.previous_filename,
    status: f.status as FileStatus,
    additions: f.additions,
    deletions: f.deletions,
    patch: f.patch,
  }));

  const reports: PullRequestBundle["reports"] = {};
  const trusted = extraTrustedAuthors();
  const prAuthor = pr.user?.login.toLowerCase();

  // Newest comment of each kind wins.
  const sorted = [...comments].sort((a, b) => b.updated_at.localeCompare(a.updated_at));
  for (const comment of sorted) {
    if (!comment.body) continue;
    const login = comment.user?.login.toLowerCase() ?? "";
    const isTrusted =
      TRUSTED_ASSOCIATIONS.has(comment.author_association) ||
      TRUSTED_BOTS.has(login) ||
      login === prAuthor ||
      trusted.has(login);
    if (!isTrusted) continue;

    const extracted = extractReport(comment.body);
    if (!extracted || reports[extracted.kind]) continue;
    const envelope: ReportEnvelope<unknown> = {
      source: { type: "comment", url: comment.html_url, author: comment.user?.login ?? "unknown", updatedAt: comment.updated_at },
      data: extracted.data,
      error: extracted.error,
    };
    (reports as Record<ReportKind, ReportEnvelope<unknown>>)[extracted.kind] = envelope;
  }

  // Fall back to report files committed on the head branch.
  const missing = REPORT_KINDS.filter((kind) => !reports[kind]);
  await Promise.all(
    missing.map(async (kind) => {
      const path = `.reviewhub/${kind}.json`;
      const text = await readFile(octokit, pr.head.repo?.owner.login ?? owner, pr.head.repo?.name ?? repo, path, pr.head.sha);
      if (text === null) return;
      let envelope: ReportEnvelope<unknown>;
      try {
        const parsed = parseReport(kind, JSON.parse(text));
        envelope = { source: { type: "file", path, ref: pr.head.sha }, ...(parsed.ok ? { data: parsed.data } : { error: parsed.error }) };
      } catch (err) {
        envelope = { source: { type: "file", path, ref: pr.head.sha }, error: `Invalid JSON: ${(err as Error).message}` };
      }
      (reports as Record<ReportKind, ReportEnvelope<unknown>>)[kind] = envelope;
    }),
  );

  // Report files are tooling output, not part of the change under review.
  const reviewable = files.filter((f) => !f.path.startsWith(".reviewhub/"));

  return {
    pr: {
      owner,
      repo,
      number,
      title: pr.title,
      body: pr.body ?? "",
      state: pr.merged_at ? "merged" : (pr.state as "open" | "closed"),
      draft: Boolean(pr.draft),
      author: { login: pr.user?.login ?? "unknown", avatarUrl: pr.user?.avatar_url ?? "" },
      baseRef: pr.base.ref,
      headRef: pr.head.ref,
      headSha: pr.head.sha,
      htmlUrl: pr.html_url,
      createdAt: pr.created_at,
      additions: pr.additions,
      deletions: pr.deletions,
      changedFiles: pr.changed_files,
      commits: pr.commits,
    },
    files: reviewable,
    reports,
  };
}

async function readFile(octokit: Octokit, owner: string, repo: string, path: string, ref: string): Promise<string | null> {
  try {
    const { data } = await octokit.repos.getContent({ owner, repo, path, ref });
    if (Array.isArray(data) || data.type !== "file" || !("content" in data)) return null;
    return Buffer.from(data.content, "base64").toString("utf8");
  } catch (err) {
    if ((err as { status?: number }).status === 404) return null;
    throw err;
  }
}
