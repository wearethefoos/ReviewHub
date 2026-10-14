export type DiffLineType = "add" | "del" | "ctx";

export interface DiffLine {
  type: DiffLineType;
  text: string;
  oldLine: number | null;
  newLine: number | null;
}

export interface DiffHunk {
  header: string;
  oldStart: number;
  newStart: number;
  lines: DiffLine[];
}

const HUNK_HEADER = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/;

/**
 * Parses the hunks of a single-file unified diff, as returned in the `patch`
 * field of GitHub's pull request files API. File headers (`---`/`+++`) are
 * skipped if present.
 */
export function parsePatch(patch: string | undefined | null): DiffHunk[] {
  if (!patch) return [];
  const hunks: DiffHunk[] = [];
  let current: DiffHunk | null = null;
  let oldNo = 0;
  let newNo = 0;

  const rows = patch.split("\n");
  if (rows.at(-1) === "") rows.pop();

  for (const raw of rows) {
    const header = HUNK_HEADER.exec(raw);
    if (header) {
      oldNo = Number(header[1]);
      newNo = Number(header[2]);
      current = { header: raw, oldStart: oldNo, newStart: newNo, lines: [] };
      hunks.push(current);
      continue;
    }
    if (!current) continue;
    if (raw.startsWith("\\")) continue; // "\ No newline at end of file"

    const marker = raw[0];
    const text = raw.slice(1);
    if (marker === "+") {
      current.lines.push({ type: "add", text, oldLine: null, newLine: newNo++ });
    } else if (marker === "-") {
      current.lines.push({ type: "del", text, oldLine: oldNo++, newLine: null });
    } else if (marker === " " || raw === "") {
      current.lines.push({ type: "ctx", text, oldLine: oldNo++, newLine: newNo++ });
    }
  }
  return hunks;
}

/** Line numbers (new side) that this patch adds. */
export function addedLines(hunks: DiffHunk[]): number[] {
  const lines: number[] = [];
  for (const hunk of hunks) {
    for (const line of hunk.lines) if (line.type === "add" && line.newLine !== null) lines.push(line.newLine);
  }
  return lines;
}

/** Splits a multi-file unified diff (e.g. a suggested fix) into per-file patches. */
export function splitUnifiedDiff(diff: string): { path: string; hunks: DiffHunk[] }[] {
  const files: { path: string; body: string[] }[] = [];
  let current: { path: string; body: string[] } | null = null;
  for (const line of diff.split("\n")) {
    if (line.startsWith("+++ ")) {
      const path = line.slice(4).replace(/^b\//, "").trim();
      current = { path, body: [] };
      files.push(current);
      continue;
    }
    if (line.startsWith("--- ") || line.startsWith("diff --git") || line.startsWith("index ")) continue;
    current?.body.push(line);
  }
  return files.map((f) => ({ path: f.path, hunks: parsePatch(f.body.join("\n")) }));
}
