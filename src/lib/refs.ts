export type TabId = "briefing" | "changes" | "decisions" | "verification" | "security" | "rollout";

export const TABS: { id: TabId; label: string }[] = [
  { id: "briefing", label: "Briefing" },
  { id: "changes", label: "Changes" },
  { id: "decisions", label: "Decisions" },
  { id: "verification", label: "Verification" },
  { id: "security", label: "Security" },
  { id: "rollout", label: "Rollout" },
];

export function isTab(value: unknown): value is TabId {
  return TABS.some((t) => t.id === value);
}

export function prPath(pr: { owner: string; repo: string; number: number }): string {
  return `/${pr.owner}/${pr.repo}/pull/${pr.number}`;
}

export function tabHref(pr: { owner: string; repo: string; number: number }, tab: TabId, anchor?: string): string {
  const base = tab === "briefing" ? prPath(pr) : `${prPath(pr)}?tab=${tab}`;
  return anchor ? `${base}#${anchor}` : base;
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

/** Parses "https://github.com/o/r/pull/1", "o/r#1" or "o/r/pull/1". */
export function parsePullRequestRef(input: string): { owner: string; repo: string; number: number } | null {
  const text = input.trim();
  const url = /^(?:https?:\/\/[^/]+\/)?([\w.-]+)\/([\w.-]+)\/pulls?\/(\d+)/.exec(text);
  const short = /^([\w.-]+)\/([\w.-]+)#(\d+)$/.exec(text);
  const match = url ?? short;
  if (!match) return null;
  return { owner: match[1]!, repo: match[2]!, number: Number(match[3]) };
}
