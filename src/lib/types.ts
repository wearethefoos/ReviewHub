import type { MetricsReport, ReviewManifest, SecurityReport } from "./schema";

export interface PullRequestInfo {
  owner: string;
  repo: string;
  number: number;
  title: string;
  body: string;
  state: "open" | "closed" | "merged";
  draft: boolean;
  author: { login: string; avatarUrl: string };
  baseRef: string;
  headRef: string;
  headSha: string;
  htmlUrl: string;
  createdAt: string;
  additions: number;
  deletions: number;
  changedFiles: number;
  commits: number;
}

export type FileStatus = "added" | "modified" | "removed" | "renamed" | "copied" | "changed" | "unchanged";

export interface ChangedFile {
  path: string;
  previousPath?: string;
  status: FileStatus;
  additions: number;
  deletions: number;
  /** Unified diff hunks for this file. Missing for binary or very large files. */
  patch?: string;
}

export type ReportSource =
  | { type: "comment"; url: string; author: string; updatedAt: string }
  | { type: "file"; path: string; ref: string }
  | { type: "fixture" };

export interface ReportEnvelope<T> {
  source: ReportSource;
  data?: T;
  /** Set when a report was found but did not validate. */
  error?: string;
}

export interface PullRequestBundle {
  pr: PullRequestInfo;
  files: ChangedFile[];
  reports: {
    manifest?: ReportEnvelope<ReviewManifest>;
    security?: ReportEnvelope<SecurityReport>;
    metrics?: ReportEnvelope<MetricsReport>;
  };
}
