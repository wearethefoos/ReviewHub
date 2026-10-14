import "server-only";
import type { PullRequestBundle } from "../types";
import { DEMO_NUMBER, DEMO_OWNER, DEMO_REPO, demoBundle } from "./demo";
import { fetchPullRequestBundle } from "./github";

export { GitHubConfigError, hasGitHubToken } from "./github";
export { DEMO_NUMBER, DEMO_OWNER, DEMO_REPO };

export function isDemo(owner: string, repo: string, number: number): boolean {
  return owner === DEMO_OWNER && repo === DEMO_REPO && number === DEMO_NUMBER;
}

export async function loadPullRequest(owner: string, repo: string, number: number): Promise<PullRequestBundle> {
  if (isDemo(owner, repo, number)) return demoBundle();
  return fetchPullRequestBundle(owner, repo, number);
}
