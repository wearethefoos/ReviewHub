import type { FocusAnalysis, Tier } from "@/lib/types";
import { Box, TIER_META } from "./ui";

const BAR: Record<Tier, string> = { focus: "bg-danger", skim: "bg-attention", skip: "bg-neutral-emphasis/40" };

function pct(part: number, whole: number): number {
  return whole === 0 ? 0 : Math.round((part / whole) * 100);
}

/** How much of the diff actually needs a human's attention. */
export function ReviewBudget({ focus }: { focus: FocusAnalysis }) {
  const { totals } = focus;
  const tiers: Tier[] = ["focus", "skim", "skip"];
  return (
    <Box className="px-4 py-3">
      <p className="text-base">
        Read <span className="font-semibold">{totals.focus.files}</span> of {totals.all.files} files closely:{" "}
        <span className="font-semibold">{totals.focus.lines.toLocaleString("en-US")}</span> of {totals.all.lines.toLocaleString("en-US")} changed lines (
        {pct(totals.focus.lines, totals.all.lines)}%).
      </p>
      <div className="mt-3 flex h-2 overflow-hidden rounded-full bg-neutral-subtle" role="img" aria-label="Changed lines by review tier">
        {tiers.map((tier) => (
          <div key={tier} className={BAR[tier]} style={{ width: `${pct(totals[tier].lines, totals.all.lines)}%` }} />
        ))}
      </div>
      <ul className="mt-3 grid gap-2 text-xs sm:grid-cols-3">
        {tiers.map((tier) => (
          <li key={tier} className="flex items-start gap-2">
            <span className={`mt-1 size-2.5 shrink-0 rounded-sm ${BAR[tier]}`} />
            <span>
              <span className="font-semibold">
                {TIER_META[tier].label}: {totals[tier].files} {totals[tier].files === 1 ? "file" : "files"}, {totals[tier].lines.toLocaleString("en-US")} lines
              </span>
              <span className="block text-fg-muted">{TIER_META[tier].description}</span>
            </span>
          </li>
        ))}
      </ul>
    </Box>
  );
}
