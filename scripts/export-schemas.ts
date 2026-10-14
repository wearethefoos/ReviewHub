/** Writes the JSON Schemas for all report kinds to schemas/, for agents and tools outside TypeScript. */
import { writeFileSync } from "node:fs";
import { z } from "zod";
import { REPORT_KINDS, reportSchemas } from "../src/lib/schema";

for (const kind of REPORT_KINDS) {
  const schema = z.toJSONSchema(reportSchemas[kind], { io: "input" });
  writeFileSync(new URL(`../schemas/${kind}.schema.json`, import.meta.url), `${JSON.stringify(schema, null, 2)}\n`);
}
console.log(`Wrote ${REPORT_KINDS.length} schemas to schemas/`);
