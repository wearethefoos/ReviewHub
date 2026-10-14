import { describe, expect, it } from "vitest";
import { parseLcov } from "./lcov";

const LCOV = `TN:
SF:/repo/src/a.ts
DA:1,1
DA:2,0
DA:3,4
DA:4,0
BRF:2
BRH:1
end_of_record
SF:src/b.ts
DA:1,1
end_of_record
`;

describe("parseLcov", () => {
  it("computes per-file and overall line coverage", () => {
    const { files, lines } = parseLcov(LCOV, "/repo");
    expect(files).toEqual([
      { path: "src/a.ts", lines: 50, branches: 50, uncoveredLines: [2, 4] },
      { path: "src/b.ts", lines: 100, branches: undefined, uncoveredLines: [] },
    ]);
    expect(lines).toBe(60);
  });
});
