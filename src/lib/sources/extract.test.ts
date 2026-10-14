import { describe, expect, it } from "vitest";
import { extractReport, renderReportComment } from "./extract";

const metrics = { version: "1", coverage: { lines: 80, files: [{ path: "a.ts", lines: 50, uncoveredLines: [3] }] } };

describe("extractReport", () => {
  it("round-trips a rendered comment", () => {
    const result = extractReport(renderReportComment("metrics", metrics, "80% coverage"));
    expect(result?.kind).toBe("metrics");
    expect(result?.error).toBeUndefined();
    expect(result?.data).toMatchObject(metrics);
  });

  it("ignores comments without a marker", () => {
    expect(extractReport("```json\n{}\n```")).toBeNull();
  });

  it("only reads the JSON block after the marker", () => {
    const body = ["```json", '{"not": "this"}', "```", "<!-- reviewhub:metrics -->", "```json", JSON.stringify(metrics), "```"].join("\n");
    expect(extractReport(body)?.data).toMatchObject(metrics);
  });

  it("reports invalid JSON and schema errors instead of throwing", () => {
    expect(extractReport("<!-- reviewhub:manifest -->\n```json\n{oops\n```")?.error).toMatch(/Invalid JSON/);
    expect(extractReport('<!-- reviewhub:security -->\n```json\n{"version":"2"}\n```')?.error).toBeTruthy();
    expect(extractReport("<!-- reviewhub:security -->\nno block")?.error).toMatch(/no ```json block/);
  });
});
