import type { FileCoverage } from "../schema";

/**
 * Converts an lcov report into per-file coverage. Absolute paths are made
 * relative to `root` so they match the paths in the pull request.
 */
export function parseLcov(text: string, root = ""): { files: FileCoverage[]; lines: number } {
  const files: FileCoverage[] = [];
  let totalFound = 0;
  let totalHit = 0;
  let current: { path: string; found: number; hit: number; uncovered: number[]; brFound: number; brHit: number } | null = null;
  const prefix = root ? root.replace(/\/?$/, "/") : "";

  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line.startsWith("SF:")) {
      let path = line.slice(3);
      if (prefix && path.startsWith(prefix)) path = path.slice(prefix.length);
      current = { path: path.replace(/^\.\//, ""), found: 0, hit: 0, uncovered: [], brFound: 0, brHit: 0 };
    } else if (!current) {
      continue;
    } else if (line.startsWith("DA:")) {
      const [lineNo, hits] = line.slice(3).split(",").map(Number);
      current.found += 1;
      if (hits && hits > 0) current.hit += 1;
      else if (lineNo) current.uncovered.push(lineNo);
    } else if (line.startsWith("BRF:")) {
      current.brFound = Number(line.slice(4));
    } else if (line.startsWith("BRH:")) {
      current.brHit = Number(line.slice(4));
    } else if (line === "end_of_record") {
      totalFound += current.found;
      totalHit += current.hit;
      files.push({
        path: current.path,
        lines: current.found === 0 ? 100 : round((current.hit / current.found) * 100),
        branches: current.brFound === 0 ? undefined : round((current.brHit / current.brFound) * 100),
        uncoveredLines: current.uncovered,
      });
      current = null;
    }
  }
  return { files, lines: totalFound === 0 ? 100 : round((totalHit / totalFound) * 100) };
}

function round(n: number): number {
  return Math.round(n * 10) / 10;
}
