/**
 * Writes the files shared with tools outside the web app:
 * - schemas/*.schema.json: JSON Schemas for the reports (the Rust CLI validates against these)
 * - fixtures/demo/bundle.json: the demo pull request
 * - fixtures/demo/focus.json: the web app's focus analysis of it (the Rust CLI must match it)
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { z } from "zod";
import { analyzeFocus } from "../src/lib/focus/score";
import { REPORT_KINDS, reportSchemas } from "../src/lib/schema";
import { demoBundle } from "../src/lib/sources/demo";

const write = (path: string, data: unknown) => writeFileSync(new URL(`../${path}`, import.meta.url), `${JSON.stringify(data, null, 2)}\n`);

for (const kind of REPORT_KINDS) {
  write(`schemas/${kind}.schema.json`, z.toJSONSchema(reportSchemas[kind], { io: "input" }));
}

const bundle = demoBundle();
const { manifest, security, metrics } = bundle.reports;
const focus = analyzeFocus({ files: bundle.files, manifest: manifest?.data, security: security?.data, metrics: metrics?.data });

mkdirSync(new URL("../fixtures/demo", import.meta.url), { recursive: true });
write("fixtures/demo/bundle.json", bundle);
write("fixtures/demo/focus.json", {
  totals: focus.totals,
  danglingRefs: focus.danglingRefs,
  files: focus.files.map((f) => ({
    path: f.file.path,
    role: f.role,
    score: f.score,
    tier: f.tier,
    uncoveredChangedLines: f.uncoveredChangedLines,
    signals: f.signals,
    annotations: f.annotations,
  })),
});

console.log("Wrote schemas/ and fixtures/demo/");
