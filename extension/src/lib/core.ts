// Typed wrapper around the reviewhub-core WebAssembly module. All scoring,
// validation and trust rules live in Rust (crates/core); this only converts JSON.
import init, * as wasm from "@/wasm/reviewhub_wasm.js";
import type { Annotation } from "@/generated/Annotation";
import type { AuthorVerdict } from "@/generated/AuthorVerdict";
import type { CommentTarget } from "@/generated/CommentTarget";
import type { DiffLine } from "@/generated/DiffLine";
import type { DraftComment } from "@/generated/DraftComment";
import type { FocusAnalysis } from "@/generated/FocusAnalysis";
import type { Hunk } from "@/generated/Hunk";
import type { PullRequestBundle } from "@/generated/PullRequestBundle";
import type { SecurityFinding } from "@/generated/SecurityFinding";

export interface Review {
  bundle: PullRequestBundle;
  focus: FocusAnalysis;
}

/** The raw GitHub responses `reviewhub-core` assembles a bundle from (Rust: `RawPullRequest`). */
export interface RawPullRequest {
  owner: string;
  repo: string;
  number: number;
  pull: unknown;
  files: unknown[];
  comments: unknown[];
  reportFiles: Record<string, unknown>;
  trustedAuthors: string[];
}

let ready: Promise<unknown> | null = null;

/** Loads the wasm module once. Pass the module bytes outside the browser (tests). */
export function loadCore(module?: BufferSource): Promise<unknown> {
  ready ??= module ? init({ module_or_path: module }) : init();
  return ready;
}

export async function reviewFromGitHub(raw: RawPullRequest): Promise<Review> {
  await loadCore();
  return JSON.parse(wasm.review(JSON.stringify(raw)));
}

export async function reviewFromBundle(bundle: unknown): Promise<Review> {
  await loadCore();
  return JSON.parse(wasm.reviewBundle(JSON.stringify(bundle)));
}

export async function headRepo(pull: unknown, owner: string, repo: string): Promise<[string, string, string]> {
  await loadCore();
  return wasm.headRepo(JSON.stringify(pull), owner, repo) as [string, string, string];
}

export async function reportFilePaths(): Promise<string[]> {
  await loadCore();
  return wasm.reportFilePaths();
}

/** Synchronous; call after the review has loaded. */
export function splitDiff(diff: string): [string, Hunk[]][] {
  return JSON.parse(wasm.splitDiff(diff));
}

/** Synchronous; call after the review has loaded. Where a comment on this diff line goes. */
export function lineTarget(line: DiffLine): CommentTarget {
  return JSON.parse(wasm.lineTarget(JSON.stringify(line)));
}

/** Synchronous. The comment target for rows `from..to` (either order) of a hunk, as selected on the diff. */
export function selectionTarget(hunk: Hunk, from: number, to: number): CommentTarget | null {
  return JSON.parse(wasm.selectionTarget(JSON.stringify(hunk), from, to));
}

/** Synchronous. A draft review comment for a security finding, or null when its lines aren't in the diff. */
export function findingComment(finding: SecurityFinding, hunks: Hunk[]): DraftComment | null {
  return JSON.parse(wasm.findingComment(JSON.stringify(finding), JSON.stringify(hunks)));
}

/** Synchronous. A draft review comment for an annotation on the diff, or null when its lines aren't in the diff. */
export function annotationComment(path: string, annotation: Annotation, hunks: Hunk[]): DraftComment | null {
  return JSON.parse(wasm.annotationComment(path, JSON.stringify(annotation), JSON.stringify(hunks)));
}

/** Synchronous. The comment for a review by the pull request's author, who GitHub won't let approve or request changes. */
export function authorReviewBody(verdict: AuthorVerdict, body: string, lineComments: number): string {
  return wasm.authorReviewBody(JSON.stringify(verdict), body, lineComments);
}
