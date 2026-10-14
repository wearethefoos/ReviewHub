// Minimal router for the review page: state lives in the query string
// (?pr=owner/repo#1&tab=changes), and the hash stays free for anchors.
import { useSyncExternalStore } from "react";

export type TabId = "briefing" | "changes" | "decisions" | "verification" | "security" | "rollout";

export const TABS: { id: TabId; label: string }[] = [
  { id: "briefing", label: "Briefing" },
  { id: "changes", label: "Changes" },
  { id: "decisions", label: "Decisions" },
  { id: "verification", label: "Verification" },
  { id: "security", label: "Security" },
  { id: "rollout", label: "Rollout" },
];

export interface Route {
  pr: string | null;
  demo: boolean;
  tab: TabId;
}

const EVENT = "reviewhub:navigate";

export function parseRoute(search: string): Route {
  const params = new URLSearchParams(search);
  const tab = params.get("tab");
  return {
    pr: params.get("pr"),
    demo: params.has("demo"),
    tab: TABS.some((t) => t.id === tab) ? (tab as TabId) : "briefing",
  };
}

export function routeHref(route: Partial<Route>, anchor?: string): string {
  const params = new URLSearchParams();
  if (route.demo) params.set("demo", "");
  if (route.pr) params.set("pr", route.pr);
  if (route.tab && route.tab !== "briefing") params.set("tab", route.tab);
  const query = params.toString().replace("demo=", "demo");
  return `?${query}${anchor ? `#${anchor}` : ""}`;
}

export function navigate(href: string) {
  history.pushState(null, "", href);
  window.dispatchEvent(new Event(EVENT));
}

function subscribe(callback: () => void) {
  window.addEventListener("popstate", callback);
  window.addEventListener(EVENT, callback);
  return () => {
    window.removeEventListener("popstate", callback);
    window.removeEventListener(EVENT, callback);
  };
}

export function useRoute(): Route {
  const search = useSyncExternalStore(subscribe, () => location.search);
  return parseRoute(search);
}

/** Which tab shows the item behind a signal or annotation ref such as "decision-foo". */
export function tabForRef(ref: string): TabId {
  if (ref.startsWith("security-")) return "security";
  if (ref.startsWith("decision-") || ref.startsWith("deviation-")) return "decisions";
  if (ref.startsWith("gap-") || ref.startsWith("validation-")) return "verification";
  return "briefing";
}

export function fileAnchor(path: string): string {
  return `file-${path.replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toLowerCase()}`;
}

/** Link to a tab of the pull request on screen, optionally to an anchor on it. */
export function tabHref(_pr: unknown, tab: TabId, anchor?: string): string {
  return routeHref({ ...parseRoute(location.search), tab }, anchor);
}
