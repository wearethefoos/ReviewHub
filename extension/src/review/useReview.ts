import { useEffect, useState } from "react";
import demoBundle from "../../../fixtures/demo/bundle.json";
import { reviewFromBundle, reviewFromGitHub, type Review } from "@/lib/core";
import { createClient, parsePrRef } from "@/lib/github";
import { loadSettings } from "@/lib/settings";

export type ReviewState =
  | { status: "idle" }
  | { status: "loading"; key: string }
  | { status: "error"; key: string; message: string }
  | { status: "ready"; key: string; review: Review };

type Outcome = { ok: true; review: Review } | { ok: false; message: string };

/** Results for this page, by pull request. */
const results = new Map<string, Outcome>();

async function load(pr: string | null, demo: boolean): Promise<Review> {
  if (demo) return reviewFromBundle(demoBundle);
  const ref = parsePrRef(pr ?? "");
  if (!ref) throw new Error(`Can't read "${pr}" as a pull request. Use owner/repo#123 or a pull request URL.`);
  const settings = await loadSettings();
  const raw = await createClient(settings.token).fetchPullRequest(ref, settings.trustedAuthors);
  return reviewFromGitHub(raw);
}

/** Loads (and keeps for this page) the review for the routed pull request. `reload` fetches it again. */
export function useReview(pr: string | null, demo: boolean): [ReviewState, () => void] {
  const key = demo ? "demo" : (pr ?? "");
  // Bumped whenever a load finishes or is restarted, to re-read `results`.
  const [version, setVersion] = useState(0);

  useEffect(() => {
    if (!key || results.has(key)) return;
    let cancelled = false;
    load(pr, demo).then(
      (review) => results.set(key, { ok: true, review }),
      (err: unknown) => results.set(key, { ok: false, message: err instanceof Error ? err.message : String(err) }),
    ).finally(() => {
      if (!cancelled) setVersion((v) => v + 1);
    });
    return () => {
      cancelled = true;
    };
  }, [key, pr, demo, version]);

  const reload = () => {
    results.delete(key);
    setVersion((v) => v + 1);
  };

  if (!key) return [{ status: "idle" }, reload];
  const outcome = results.get(key);
  if (!outcome) return [{ status: "loading", key }, reload];
  return [outcome.ok ? { status: "ready", key, review: outcome.review } : { status: "error", key, message: outcome.message }, reload];
}
