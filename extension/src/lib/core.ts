// Typed wrapper around the reviewhub-core WebAssembly module. All scoring,
// validation and trust rules live in Rust (crates/core); this only converts JSON.
import init, * as wasm from "@/wasm/reviewhub_wasm.js";
import type { FocusAnalysis } from "@/generated/FocusAnalysis";
import type { Hunk } from "@/generated/Hunk";
import type { PullRequestBundle } from "@/generated/PullRequestBundle";

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
