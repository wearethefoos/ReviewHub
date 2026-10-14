import type { ChangedFile, FileStatus } from "../types";

/** Builds a hunk with a correct `@@` header from lines prefixed with "+", "-" or " ". */
export function hunk(oldStart: number, newStart: number, lines: string[]): string {
  const oldCount = lines.filter((l) => !l.startsWith("+")).length;
  const newCount = lines.filter((l) => !l.startsWith("-")).length;
  return [`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@`, ...lines].join("\n");
}

/** Patch for a brand new file. */
export function addedPatch(content: string): string {
  const lines = content.replace(/\n$/, "").split("\n");
  return hunk(0, 1, lines.map((l) => `+${l}`));
}

export function changedFile(path: string, status: FileStatus, patch: string | undefined, extra: Partial<ChangedFile> = {}): ChangedFile {
  const rows = patch?.split("\n") ?? [];
  return {
    path,
    status,
    additions: rows.filter((l) => l.startsWith("+")).length,
    deletions: rows.filter((l) => l.startsWith("-")).length,
    patch,
    ...extra,
  };
}

/** 1-based line number of the first line containing `needle`. Keeps fixture references in sync with content. */
export function lineOf(content: string, needle: string): number {
  const index = content.split("\n").findIndex((l) => l.includes(needle));
  if (index === -1) throw new Error(`Fixture line not found: ${needle}`);
  return index + 1;
}
