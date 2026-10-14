import type { z } from "zod";
import { MetricsReport, ReviewManifest, SecurityReport } from "../schema";
import type { PullRequestBundle } from "../types";
import { addedPatch, changedFile, hunk, lineOf } from "./fixture-util";

/**
 * A realistic agent-authored pull request used for the demo and in tests.
 * Line references are derived from the file contents so they stay in sync.
 */

export const DEMO_OWNER = "demo";
export const DEMO_REPO = "acme-shop";
export const DEMO_NUMBER = 128;

const refundsTs = `import { db } from "../db";
import { redis } from "../lib/redis";
import { allocate, type Money } from "../lib/money";
import { emitEvent } from "./events";

export interface RefundRequest {
  orderId: string;
  amount: Money;
  reason?: string;
  idempotencyKey: string;
}

export class RefundError extends Error {
  constructor(
    public code: "ORDER_NOT_FOUND" | "ORDER_NOT_CAPTURED" | "AMOUNT_EXCEEDS_REMAINING",
    message: string,
  ) {
    super(message);
  }
}

const IDEMPOTENCY_TTL_SECONDS = 60 * 60 * 24;

export async function createRefund(req: RefundRequest) {
  const cached = await redis.get(\`refund:idem:\${req.idempotencyKey}\`);
  if (cached) return JSON.parse(cached);

  const order = await db.one(
    \`SELECT id, status, captured_amount, refunded_amount, currency, line_items FROM orders WHERE id = '\${req.orderId}'\`,
  );
  if (!order) throw new RefundError("ORDER_NOT_FOUND", \`Order \${req.orderId} not found\`);
  if (order.status !== "captured") {
    throw new RefundError("ORDER_NOT_CAPTURED", "Only captured orders can be refunded");
  }

  const remaining = order.captured_amount - order.refunded_amount;
  if (req.amount.cents > remaining) {
    throw new RefundError("AMOUNT_EXCEEDS_REMAINING", \`Refund exceeds remaining \${remaining}\`);
  }

  const lineItems = allocate(req.amount, order.line_items.map((li: { cents: number }) => li.cents));
  const refund = await db.transaction(async (tx) => {
    const row = await tx.insert("refunds", {
      order_id: order.id,
      amount_cents: req.amount.cents,
      currency: order.currency,
      reason: req.reason ?? null,
    });
    await tx.update("orders", order.id, { refunded_amount: order.refunded_amount + req.amount.cents });
    return row;
  });

  await redis.set(\`refund:idem:\${req.idempotencyKey}\`, JSON.stringify(refund), "EX", IDEMPOTENCY_TTL_SECONDS);
  await emitEvent("refund.created", { refundId: refund.id, orderId: order.id, lineItems });
  return refund;
}
`;

const routeTs = `import { Router } from "express";
import { z } from "zod";
import { flags } from "../../config/featureFlags";
import { requireMerchant } from "../middleware/auth";
import { createRefund, RefundError } from "../../payments/refunds";

const body = z.object({
  amount: z.object({ cents: z.number().int().positive(), currency: z.string().length(3) }),
  reason: z.string().max(500).optional(),
});

export const refundsRouter = Router();

refundsRouter.post("/orders/:orderId/refunds", requireMerchant, async (req, res) => {
  if (!flags.partialRefunds) return res.status(404).end();

  const idempotencyKey = req.header("Idempotency-Key");
  if (!idempotencyKey) return res.status(400).json({ error: "IDEMPOTENCY_KEY_REQUIRED" });

  const parsed = body.safeParse(req.body);
  if (!parsed.success) return res.status(422).json({ error: parsed.error.flatten() });

  try {
    const refund = await createRefund({ orderId: req.params.orderId, ...parsed.data, idempotencyKey });
    return res.status(201).json(refund);
  } catch (err) {
    if (err instanceof RefundError) {
      const status = err.code === "ORDER_NOT_FOUND" ? 404 : 409;
      return res.status(status).json({ error: err.code, message: err.message });
    }
    throw err;
  }
});
`;

const refundsTestTs = `import { beforeEach, describe, expect, it } from "vitest";
import fc from "fast-check";
import { createRefund, RefundError } from "./refunds";
import { allocate } from "../lib/money";
import { seedOrder, resetDb } from "../../test/db";

describe("createRefund", () => {
  beforeEach(resetDb);

  it("refunds part of a captured order", async () => {
    const order = await seedOrder({ captured: 10_000 });
    const refund = await createRefund({ orderId: order.id, amount: { cents: 2_500, currency: "EUR" }, idempotencyKey: "k1" });
    expect(refund).toMatchSnapshot();
  });

  it("rejects refunds above the remaining amount", async () => {
    const order = await seedOrder({ captured: 10_000, refunded: 9_000 });
    await expect(
      createRefund({ orderId: order.id, amount: { cents: 1_001, currency: "EUR" }, idempotencyKey: "k2" }),
    ).rejects.toThrow(RefundError);
  });

  it("returns the same refund for a repeated idempotency key", async () => {
    const order = await seedOrder({ captured: 10_000 });
    const req = { orderId: order.id, amount: { cents: 100, currency: "EUR" }, idempotencyKey: "k3" };
    expect(await createRefund(req)).toEqual(await createRefund(req));
  });
});

describe("allocate", () => {
  it("always sums to the total", () => {
    fc.assert(
      fc.property(fc.nat(1_000_000), fc.array(fc.nat(10_000), { minLength: 1 }), (cents, weights) => {
        const parts = allocate({ cents, currency: "EUR" }, weights);
        const sum = parts.reduce((a, p) => a + p.cents, 0);
        return weights.every((w) => w === 0) ? sum === 0 : sum === cents;
      }),
    );
  });
});
`;

const migrationSql = `CREATE TABLE refunds (
  id BIGSERIAL PRIMARY KEY,
  order_id BIGINT NOT NULL REFERENCES orders(id),
  amount_cents BIGINT NOT NULL CHECK (amount_cents > 0),
  currency CHAR(3) NOT NULL,
  reason TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX refunds_order_id_idx ON refunds(order_id);

ALTER TABLE orders ADD COLUMN refunded_amount BIGINT NOT NULL DEFAULT 0;
`;

const moneyHunk = hunk(8, 8, [
  "   return { cents: a.cents + b.cents, currency: a.currency };",
  " }",
  " ",
  " export function percentOf(m: Money, pct: number): Money {",
  "-  return { cents: Math.round((m.cents * pct) / 100), currency: m.currency };",
  "+  return { cents: roundHalfEven((m.cents * pct) / 100), currency: m.currency };",
  "+}",
  "+",
  "+/** Banker's rounding: ties go to the nearest even number. */",
  "+export function roundHalfEven(value: number): number {",
  "+  const floor = Math.floor(value);",
  "+  const diff = value - floor;",
  "+  if (diff > 0.5) return floor + 1;",
  "+  if (diff < 0.5) return floor;",
  "+  return floor % 2 === 0 ? floor : floor + 1;",
  "+}",
  "+",
  "+/**",
  "+ * Splits \`total\` across \`weights\` proportionally. Remainders go to the",
  "+ * parts with the largest fractional share so the parts always sum to the total.",
  "+ */",
  "+export function allocate(total: Money, weights: number[]): Money[] {",
  "+  const sum = weights.reduce((a, b) => a + b, 0);",
  "+  if (sum === 0) return weights.map(() => ({ cents: 0, currency: total.currency }));",
  "+  const raw = weights.map((w) => (total.cents * w) / sum);",
  "+  const parts = raw.map(Math.floor);",
  "+  let remainder = total.cents - parts.reduce((a, b) => a + b, 0);",
  "+  const order = raw.map((r, i) => [r - Math.floor(r), i] as const).sort((a, b) => b[0] - a[0]);",
  "+  for (const [, i] of order) {",
  "+    if (remainder-- <= 0) break;",
  "+    parts[i] += 1;",
  "+  }",
  "+  return parts.map((cents) => ({ cents, currency: total.currency }));",
  " }",
  " ",
  " function assertSameCurrency(a: Money, b: Money) {",
]);

const webhooksPatch = [
  hunk(1, 1, [
    ' import type { Request, Response } from "express";',
    '-import { emitEvent } from "./eventBus";',
    '+import { emitEvent } from "./events";',
    ' import { verifySignature } from "./signature";',
  ]),
].join("\n");

const flagsPatch = hunk(3, 3, [
  " export const flags = {",
  '   newCheckout: env("FLAG_NEW_CHECKOUT", true),',
  '+  partialRefunds: env("FLAG_PARTIAL_REFUNDS", false),',
  " };",
]);

const i18nPatch = hunk(14, 14, [
  '   "orders.notFound": "Order not found",',
  '+  "refunds.amountExceedsRemaining": "The refund is larger than the amount left to refund.",',
  '+  "refunds.orderNotCaptured": "Only paid orders can be refunded.",',
  '+  "refunds.idempotencyKeyRequired": "Send an Idempotency-Key header with every refund request.",',
  '   "orders.shipped": "Order shipped"',
]);

const docsMd = `# Refunds

Create a full or partial refund for a captured order.

## \`POST /orders/:orderId/refunds\`

| Header            | Required | Description                                 |
| ----------------- | -------- | ------------------------------------------- |
| \`Idempotency-Key\` | yes      | Retries with the same key return the same refund. |

\`\`\`json
{ "amount": { "cents": 2500, "currency": "EUR" }, "reason": "Damaged item" }
\`\`\`

### Responses

- \`201\` the refund was created
- \`404\` the order does not exist
- \`409\` the order is not captured, or the amount exceeds what is left to refund
- \`422\` the request body is invalid
`;

const snapshot = `// Vitest Snapshot v1

exports[\`createRefund > refunds part of a captured order 1\`] = \`
{
  "amount_cents": 2500,
  "currency": "EUR",
  "id": 1,
  "order_id": 1,
  "reason": null,
}
\`;
`;

function lockfilePatch(): string {
  const lines: string[] = [];
  const pkgs = ["fast-check", "pure-rand"];
  for (const name of pkgs) {
    lines.push(`+    "node_modules/${name}": {`, `+      "version": "3.23.2",`, `+      "resolved": "https://registry.npmjs.org/${name}/-/${name}-3.23.2.tgz",`);
    lines.push(`+      "integrity": "sha512-${"x".repeat(40)}",`, "+      \"dev\": true,", "+      \"license\": \"MIT\"", "+    },");
  }
  return hunk(4211, 4211, ['     "node_modules/fast-deep-equal": {', ...lines, '     "node_modules/fast-glob": {']);
}

const PATHS = {
  refunds: "src/payments/refunds.ts",
  route: "src/api/routes/refunds.ts",
  money: "src/lib/money.ts",
  test: "src/payments/refunds.test.ts",
  migration: "db/migrations/20261012_add_refunds.sql",
  flags: "src/config/featureFlags.ts",
  webhooks: "src/payments/webhooks.ts",
  events: "src/payments/events.ts",
  i18n: "src/i18n/en.json",
  docs: "docs/api/refunds.md",
  snapshot: "src/payments/__snapshots__/refunds.test.ts.snap",
  lock: "package-lock.json",
} as const;

const L = {
  idemGet: lineOf(refundsTs, "const cached = await redis.get"),
  idemReturn: lineOf(refundsTs, "if (cached) return"),
  sql: lineOf(refundsTs, "SELECT id, status"),
  notCaptured: lineOf(refundsTs, 'throw new RefundError("ORDER_NOT_CAPTURED"'),
  remaining: lineOf(refundsTs, "const remaining ="),
  remainingCheckEnd: lineOf(refundsTs, "AMOUNT_EXCEEDS_REMAINING\", `Refund exceeds") + 1,
  allocate: lineOf(refundsTs, "const lineItems = allocate"),
  idemSet: lineOf(refundsTs, "await redis.set("),
  routePost: lineOf(routeTs, "refundsRouter.post("),
  routeCreate: lineOf(routeTs, "const refund = await createRefund("),
  routeStatus: lineOf(routeTs, "const status = err.code"),
  alter: lineOf(migrationSql, "ALTER TABLE orders"),
};

const manifest: z.input<typeof ReviewManifest> = {
  version: "1",
  agent: { name: "Claude Code", sessionUrl: "https://claude.ai/code" },
  prompt: {
    original:
      "Add support for partial refunds to the payments API. Merchants should be able to refund part of an order via `PUT /refunds/:orderId` with an amount. Make sure we never refund more than was captured, and put it behind a feature flag. Use our existing money helpers.",
    followUps: ["Also make sure retries from the merchant's side don't create duplicate refunds."],
  },
  summary:
    "Adds `POST /orders/:orderId/refunds` behind the `partialRefunds` flag. A refund is stored in a new `refunds` table, the order's `refunded_amount` is updated in the same transaction, and the refund is spread over the order's line items for the existing `refund.created` worker. Retries are de-duplicated with an `Idempotency-Key` header.",
  deviations: [
    {
      id: "endpoint-shape",
      title: "Endpoint is `POST /orders/:orderId/refunds`, not `PUT /refunds/:orderId`",
      requested: "`PUT /refunds/:orderId` with an amount.",
      delivered: "`POST /orders/:orderId/refunds` that creates a new refund resource on each call.",
      reason:
        "An order can now have several partial refunds, so each call creates a resource rather than replacing one. `PUT` keyed by order id would make a second partial refund overwrite the first. Retry safety comes from the `Idempotency-Key` header instead.",
      severity: "major",
      files: [{ path: PATHS.route, lines: [L.routePost, L.routePost] }],
    },
    {
      id: "money-rounding",
      title: "Changed rounding in the shared `percentOf` helper",
      requested: "Use the existing money helpers.",
      delivered: "`percentOf` now uses banker's rounding (half to even) instead of `Math.round`.",
      reason:
        "Half-up rounding over many line items made allocated parts drift from the refund total by a cent. The new `allocate` helper needs consistent rounding, so I changed `percentOf` to match. This also affects the existing callers of `percentOf` (discounts and tax).",
      severity: "major",
      files: [{ path: PATHS.money, lines: [12, 12] }],
    },
  ],
  decisions: [
    {
      id: "idempotency-store",
      title: "Idempotency keys are stored in Redis with a 24 hour TTL",
      context: "The follow-up asked that merchant retries never create duplicate refunds. The prompt didn't say where to remember keys or for how long.",
      chosen: "Cache the created refund in Redis under the idempotency key for 24 hours.",
      rationale: "Redis is already a dependency, lookups are cheap, and 24 hours matches the retry window of our SDKs.",
      impact: "high",
      alternatives: [
        {
          title: "Unique constraint in Postgres",
          description: "Store `idempotency_key` on the `refunds` row with a unique index on `(merchant_id, idempotency_key)`.",
          tradeoffs: "Durable and transactional: survives a Redis flush and cannot race. Needs one more column and index in the migration.",
          prompt:
            "Replace the Redis-based idempotency in src/payments/refunds.ts with a unique index on refunds(merchant_id, idempotency_key). Add the column to the migration, insert inside the existing transaction, and on a unique violation return the existing refund. Remove the Redis calls and update the tests.",
        },
        {
          title: "Shared idempotency middleware",
          description: "Handle the `Idempotency-Key` header in an Express middleware for all mutating endpoints.",
          tradeoffs: "Reusable across endpoints, but a larger change than this PR asked for.",
        },
      ],
      files: [
        { path: PATHS.refunds, lines: [L.idemGet, L.idemReturn] },
        { path: PATHS.refunds, lines: [L.idemSet, L.idemSet] },
      ],
    },
    {
      id: "allocation",
      title: "Partial refunds are spread over line items proportionally",
      context: "The downstream `refund.created` worker needs per-line-item amounts for tax reporting. The prompt only specified a total amount.",
      chosen: "Split the amount proportionally by line item value, using the largest-remainder method so parts always add up.",
      rationale: "It's predictable and needs no extra input from merchants.",
      impact: "medium",
      alternatives: [
        {
          title: "Let merchants specify line items",
          description: "Accept an optional `lineItems` array in the request body.",
          tradeoffs: "More accurate for item-level returns, but a bigger API surface to document and validate.",
          prompt: "Extend POST /orders/:orderId/refunds to accept an optional lineItems array of { lineItemId, cents }. Validate that it sums to amount.cents and fall back to proportional allocation when it's missing.",
        },
      ],
      files: [{ path: PATHS.refunds, lines: [L.allocate, L.allocate] }],
    },
    {
      id: "conflict-status",
      title: "Business rule violations return 409",
      context: "Refunding more than is left, or refunding an uncaptured order, isn't a malformed request.",
      chosen: "Return 409 Conflict with an error code. 422 is reserved for invalid bodies.",
      rationale: "Matches how the orders API already reports state conflicts.",
      impact: "low",
      alternatives: [{ title: "422 for everything", description: "Use 422 for both validation and business-rule errors." }],
      files: [{ path: PATHS.route, lines: [L.routeStatus, L.routeStatus] }],
    },
  ],
  validation: [
    { id: "unit", kind: "unit-test", description: "Refund service unit tests against the test database", command: "npm test -- refunds", result: "passed", evidence: "✓ src/payments/refunds.test.ts (4 tests) 412ms\nTest Files  1 passed (1)\n     Tests  4 passed (4)", files: [{ path: PATHS.refunds }] },
    { id: "property", kind: "unit-test", description: "Property test: `allocate` always sums to the total (fast-check, 1000 runs)", command: "npm test -- refunds -t allocate", result: "passed", files: [{ path: PATHS.money }] },
    { id: "types", kind: "typecheck", description: "TypeScript compiles without errors", command: "npm run typecheck", result: "passed" },
    { id: "lint", kind: "lint", description: "ESLint passes", command: "npm run lint", result: "passed" },
    { id: "manual", kind: "manual", description: "Called the endpoint with curl against a local server: happy path, over-refund and missing idempotency key", result: "partial", evidence: "201 Created, 409 AMOUNT_EXCEEDS_REMAINING, 400 IDEMPOTENCY_KEY_REQUIRED. Didn't check the flag-off path." },
    { id: "integration", kind: "integration-test", description: "Concurrent refunds against a real Postgres", result: "skipped", evidence: "No Postgres available in the agent sandbox." },
  ],
  gaps: [
    {
      id: "concurrency",
      title: "Two refunds for the same order at the same time",
      why: "The sandbox has no Postgres, so concurrent transactions couldn't be exercised.",
      howToValidate: "Write an integration test that fires two refunds for the same order in parallel and asserts the total never exceeds the captured amount. Run it against the docker-compose Postgres.",
      owner: "agent",
      effort: "small",
      files: [{ path: PATHS.refunds, lines: [L.remaining, L.remainingCheckEnd] }],
    },
    {
      id: "migration-lock",
      title: "Migration lock time on the `orders` table",
      why: "`orders` has around 40M rows in production. I couldn't measure how long the `ALTER TABLE` holds its lock.",
      howToValidate: "Run the migration on a staging snapshot and time it. On Postgres 11+ adding a column with a constant default shouldn't rewrite the table. Confirm the production version.",
      owner: "ops",
      effort: "small",
      files: [{ path: PATHS.migration, lines: [L.alter, L.alter] }],
    },
    {
      id: "route-tests",
      title: "No HTTP-level tests for the refunds route",
      why: "I tested the service directly. The route's flag check, header handling and status mapping are only covered by the manual curl run.",
      howToValidate: "Add supertest cases for: flag off returns 404, missing header returns 400, invalid body returns 422, and each RefundError maps to the right status.",
      owner: "agent",
      effort: "small",
      files: [{ path: PATHS.route }],
    },
    {
      id: "provider",
      title: "End to end with the payment provider sandbox",
      why: "No provider sandbox credentials were available, so the `refund.created` worker never ran against this event shape.",
      howToValidate: "With the flag on in staging, refund a sandbox order partially twice and check both refunds show up at the provider with the right amounts.",
      owner: "human",
      effort: "medium",
    },
  ],
  rollout: [
    { id: "api-shape", title: "Confirm the endpoint shape with the mobile team", description: "The endpoint differs from the one in the original request (see the deviations).", owner: "human", when: "before-merge", blocking: true },
    { id: "callers", title: "Check existing `percentOf` callers still give the expected totals", description: "Discount and tax calculations now use banker's rounding.", owner: "agent", when: "before-merge", blocking: true, prompt: "Find every caller of percentOf in src/. For each one, add a test case on a .5 cent boundary showing the old and new result, and list any that change customer-facing totals." },
    { id: "migrate", title: "Run the migration on staging, then production", owner: "ops", when: "after-merge", blocking: true },
    { id: "flag", title: "Turn on `FLAG_PARTIAL_REFUNDS` for internal merchants, then 10%", owner: "human", when: "after-deploy", blocking: false },
    { id: "alert", title: "Add an alert on `refund.created` worker failures", owner: "ops", when: "after-deploy", blocking: false },
    { id: "changelog", title: "Add the endpoint to the public API changelog", owner: "agent", when: "after-merge", blocking: false, prompt: "Add an entry to docs/changelog.md for the new POST /orders/:orderId/refunds endpoint, linking to docs/api/refunds.md." },
  ],
  files: [
    { path: PATHS.refunds, role: "core", reviewHint: "must-review", note: "The refund service: validation, persistence and idempotency." },
    { path: PATHS.route, role: "core", note: "HTTP layer: flag check, header and body validation, error mapping." },
    { path: PATHS.money, role: "supporting", note: "Shared helper. The `percentOf` change affects existing callers." },
    { path: PATHS.webhooks, role: "mechanical", reviewHint: "skip", note: "Import path update after renaming eventBus.ts to events.ts." },
    { path: PATHS.events, role: "mechanical", reviewHint: "skip", note: "Renamed from eventBus.ts, no content changes." },
    { path: PATHS.i18n, role: "mechanical", reviewHint: "skip", note: "Error messages for the new error codes." },
    { path: PATHS.lock, role: "generated", reviewHint: "skip", note: "Adds fast-check as a dev dependency." },
    { path: PATHS.snapshot, role: "generated", reviewHint: "skip" },
    { path: PATHS.docs, role: "docs", reviewHint: "skim" },
  ],
};

const security: z.input<typeof SecurityReport> = {
  version: "1",
  scanner: { name: "ReviewHub security agent" },
  summary:
    "One critical and two high findings in the new refund path. The SQL injection and missing ownership check together let any merchant refund any order. Fix both before merging.",
  findings: [
    {
      id: "sqli",
      title: "SQL injection through orderId",
      severity: "critical",
      category: "CWE-89",
      confidence: "high",
      description:
        "`orderId` comes straight from the URL and is interpolated into the SQL string. A request to `/orders/1' OR '1'='1/refunds` changes the query.",
      file: PATHS.refunds,
      lines: [L.sql - 1, L.sql + 1],
      suggestion: {
        explanation: "Use a parameterized query so the driver escapes the value.",
        diff: [
          `--- a/${PATHS.refunds}`,
          `+++ b/${PATHS.refunds}`,
          hunk(L.sql - 1, L.sql - 1, [
            "   const order = await db.one(",
            "-    `SELECT id, status, captured_amount, refunded_amount, currency, line_items FROM orders WHERE id = '${req.orderId}'`,",
            "+    \"SELECT id, status, captured_amount, refunded_amount, currency, line_items FROM orders WHERE id = $1\",",
            "+    [req.orderId],",
            "   );",
          ]),
        ].join("\n"),
        prompt: "In src/payments/refunds.ts, replace the interpolated SQL in createRefund with a parameterized query. Then search src/ for other template-literal SQL passed to db.one, db.many or db.query and fix those the same way.",
      },
    },
    {
      id: "idor",
      title: "Merchants can refund orders that belong to other merchants",
      severity: "high",
      category: "CWE-639",
      confidence: "high",
      description:
        "`requireMerchant` authenticates the caller, but nothing checks that the order belongs to that merchant. Any merchant who can guess an order id can refund it.",
      file: PATHS.route,
      lines: [L.routeCreate, L.routeCreate],
      suggestion: {
        explanation: "Pass the authenticated merchant through and filter the order lookup by it, so foreign orders look the same as missing ones (404).",
        diff: [
          `--- a/${PATHS.route}`,
          `+++ b/${PATHS.route}`,
          hunk(L.routeCreate, L.routeCreate, [
            "-    const refund = await createRefund({ orderId: req.params.orderId, ...parsed.data, idempotencyKey });",
            "+    const refund = await createRefund({",
            "+      merchantId: req.merchant.id,",
            "+      orderId: req.params.orderId,",
            "+      ...parsed.data,",
            "+      idempotencyKey,",
            "+    });",
          ]),
        ].join("\n"),
        prompt: "Add a required merchantId to RefundRequest in src/payments/refunds.ts and filter the order lookup with `AND merchant_id = $2`. Pass req.merchant.id from the route. Add a test showing a merchant gets 404 for another merchant's order.",
      },
    },
    {
      id: "toctou",
      title: "Concurrent refunds can exceed the captured amount",
      severity: "high",
      category: "CWE-362",
      confidence: "medium",
      description:
        "The remaining amount is checked before the transaction starts, and the update writes an absolute value. Two parallel requests can both pass the check, and the second write overwrites the first.",
      file: PATHS.refunds,
      lines: [L.remaining, L.remainingCheckEnd],
      suggestion: {
        explanation: "Lock the order row inside the transaction (`SELECT … FOR UPDATE`), check the remaining amount there, and update with `refunded_amount = refunded_amount + $1`.",
        prompt: "In src/payments/refunds.ts, move the order lookup and the remaining-amount check inside db.transaction and lock the row with SELECT ... FOR UPDATE. Update refunded_amount relative to its current value. Add an integration test that fires two refunds in parallel and asserts only one succeeds when together they exceed the captured amount.",
      },
    },
    {
      id: "idem-scope",
      title: "Idempotency keys aren't scoped to the merchant",
      severity: "medium",
      category: "CWE-639",
      confidence: "medium",
      description:
        "The Redis key only contains the client-supplied idempotency key. If two merchants send the same key, the second gets the first merchant's refund back, including its order id and amount.",
      file: PATHS.refunds,
      lines: [L.idemGet, L.idemGet],
      suggestion: {
        explanation: "Prefix the key with the merchant id (once it's passed in, see the ownership finding).",
        diff: [
          `--- a/${PATHS.refunds}`,
          `+++ b/${PATHS.refunds}`,
          hunk(L.idemGet, L.idemGet, [
            "-  const cached = await redis.get(`refund:idem:${req.idempotencyKey}`);",
            "+  const cached = await redis.get(`refund:idem:${req.merchantId}:${req.idempotencyKey}`);",
          ]),
        ].join("\n"),
      },
    },
    {
      id: "flag-404",
      title: "Flag-off responses reveal the endpoint exists",
      severity: "info",
      category: "CWE-200",
      confidence: "low",
      description: "The 404 is returned after `requireMerchant`, so unauthenticated callers get 401 and can tell the route exists. Low impact while the flag is temporary.",
      file: PATHS.route,
      lines: [L.routePost, L.routePost + 1],
      suggestion: { explanation: "Check the flag when mounting the router instead of inside the handler." },
    },
  ],
};

const metrics: z.input<typeof MetricsReport> = {
  version: "1",
  coverage: {
    tool: "vitest (v8)",
    lines: 84.2,
    baseLines: 85.0,
    files: [
      { path: PATHS.refunds, lines: 88.6, uncoveredLines: [L.idemReturn, L.notCaptured] },
      { path: PATHS.money, lines: 93.1, uncoveredLines: [30] },
      { path: PATHS.webhooks, lines: 100, uncoveredLines: [] },
      { path: PATHS.flags, lines: 100, uncoveredLines: [] },
    ],
  },
  quality: {
    tool: "eslint complexity",
    files: [
      { path: PATHS.refunds, complexity: 9, complexityDelta: 9 },
      { path: PATHS.route, complexity: 7, complexityDelta: 7 },
      { path: PATHS.money, complexity: 14, complexityDelta: 6 },
    ],
  },
};

export function demoBundle(): PullRequestBundle {
  const files = [
    changedFile(PATHS.refunds, "added", addedPatch(refundsTs)),
    changedFile(PATHS.route, "added", addedPatch(routeTs)),
    changedFile(PATHS.money, "modified", moneyHunk),
    changedFile(PATHS.test, "added", addedPatch(refundsTestTs)),
    changedFile(PATHS.migration, "added", addedPatch(migrationSql)),
    changedFile(PATHS.flags, "modified", flagsPatch),
    changedFile(PATHS.webhooks, "modified", webhooksPatch),
    changedFile(PATHS.events, "renamed", undefined, { previousPath: "src/payments/eventBus.ts" }),
    changedFile(PATHS.i18n, "modified", i18nPatch),
    changedFile(PATHS.docs, "added", addedPatch(docsMd)),
    changedFile(PATHS.snapshot, "added", addedPatch(snapshot)),
    changedFile(PATHS.lock, "modified", lockfilePatch()),
  ];
  const additions = files.reduce((n, f) => n + f.additions, 0);
  const deletions = files.reduce((n, f) => n + f.deletions, 0);

  return {
    pr: {
      owner: DEMO_OWNER,
      repo: DEMO_REPO,
      number: DEMO_NUMBER,
      title: "Add partial refunds to the payments API",
      body: "Adds partial refunds behind the `partialRefunds` flag. See the ReviewHub briefing for decisions, validation and rollout.",
      state: "open",
      draft: false,
      author: { login: "claude[bot]", avatarUrl: "" },
      baseRef: "main",
      headRef: "claude/partial-refunds",
      headSha: "4f1c2e9a7b3d5c8e0f6a1b2c3d4e5f6a7b8c9d0e",
      htmlUrl: "",
      createdAt: "2026-10-12T09:41:00Z",
      additions,
      deletions,
      changedFiles: files.length,
      commits: 6,
    },
    files,
    reports: {
      // Parsing applies schema defaults and fails loudly if the fixture drifts from the schema.
      manifest: { source: { type: "fixture" }, data: ReviewManifest.parse(manifest) },
      security: { source: { type: "fixture" }, data: SecurityReport.parse(security) },
      metrics: { source: { type: "fixture" }, data: MetricsReport.parse(metrics) },
    },
  };
}
