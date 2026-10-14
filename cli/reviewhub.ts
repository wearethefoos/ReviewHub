#!/usr/bin/env -S npx tsx
/**
 * ReviewHub CLI: helps agents and CI produce and publish reports.
 *
 *   reviewhub validate <kind> <file>                 Check a report against its schema
 *   reviewhub schema <kind>                          Print the JSON Schema for a report
 *   reviewhub metrics --lcov <file> [--root <dir>]   Build a metrics report from lcov
 *            [--base-lines <pct>] [--commit <sha>]
 *   reviewhub comment <kind> <file>                  Print a PR comment body for the report
 *   reviewhub publish <kind> <file> --repo <owner/repo> --pr <number>
 *                                                    Create or update the report comment (needs GITHUB_TOKEN)
 *
 * <kind> is one of: manifest, security, metrics. Use "-" as <file> to read stdin.
 */
import { readFileSync } from "node:fs";
import { z } from "zod";
import { parseLcov } from "../src/lib/metrics/lcov";
import { MetricsReport, parseReport, REPORT_KINDS, reportSchemas, type ReportKind } from "../src/lib/schema";
import { markerFor, renderReportComment } from "../src/lib/sources/extract";

function fail(message: string): never {
  process.stderr.write(`reviewhub: ${message}\n`);
  process.exit(1);
}

function flag(args: string[], name: string): string | undefined {
  const i = args.indexOf(`--${name}`);
  return i === -1 ? undefined : args[i + 1];
}

function kindArg(value: string | undefined): ReportKind {
  if (!value || !REPORT_KINDS.includes(value as ReportKind)) fail(`kind must be one of: ${REPORT_KINDS.join(", ")}`);
  return value as ReportKind;
}

function readReport(kind: ReportKind, file: string | undefined) {
  if (!file) fail("missing <file>");
  let json: unknown;
  try {
    json = JSON.parse(readFileSync(file === "-" ? 0 : file, "utf8"));
  } catch (err) {
    fail(`cannot read ${file}: ${(err as Error).message}`);
  }
  const parsed = parseReport(kind, json);
  if (!parsed.ok) fail(`${file} is not a valid ${kind} report:\n${parsed.error}`);
  return parsed.data;
}

function summarize(kind: ReportKind, report: unknown): string {
  if (kind === "security") {
    const findings = (report as { findings: { severity: string }[] }).findings;
    return findings.length === 0 ? "no findings" : `${findings.length} finding${findings.length === 1 ? "" : "s"}`;
  }
  if (kind === "metrics") {
    const lines = (report as MetricsReport).coverage?.lines;
    return lines === undefined ? "" : `${lines.toFixed(1)}% line coverage`;
  }
  return "";
}

async function publish(kind: ReportKind, report: unknown, repoArg: string | undefined, prArg: string | undefined) {
  const token = process.env.GITHUB_TOKEN;
  if (!token) fail("GITHUB_TOKEN is not set");
  const [owner, repo] = (repoArg ?? "").split("/");
  const number = Number(prArg);
  if (!owner || !repo || !Number.isInteger(number)) fail("use --repo <owner/repo> --pr <number>");

  const api = process.env.GITHUB_API_URL ?? "https://api.github.com";
  const headers = { authorization: `Bearer ${token}`, accept: "application/vnd.github+json", "user-agent": "reviewhub-cli" };
  const call = async (method: string, path: string, body?: unknown) => {
    const res = await fetch(`${api}${path}`, { method, headers, body: body ? JSON.stringify(body) : undefined });
    if (!res.ok) fail(`${method} ${path} failed: ${res.status} ${await res.text()}`);
    return res.json();
  };

  const me = (await call("GET", "/user").catch(() => null)) as { login?: string } | null;
  const body = renderReportComment(kind, report, summarize(kind, report));
  const marker = markerFor(kind);

  // Update our own previous comment of this kind, so the PR keeps one comment per report.
  let existing: { id: number } | undefined;
  for (let page = 1; !existing; page++) {
    const comments = (await call("GET", `/repos/${owner}/${repo}/issues/${number}/comments?per_page=100&page=${page}`)) as {
      id: number;
      body?: string;
      user?: { login: string };
    }[];
    existing = comments.find((c) => c.body?.includes(marker) && (!me?.login || c.user?.login === me.login));
    if (comments.length < 100) break;
  }

  const result = existing
    ? await call("PATCH", `/repos/${owner}/${repo}/issues/comments/${existing.id}`, { body })
    : await call("POST", `/repos/${owner}/${repo}/issues/${number}/comments`, { body });
  process.stdout.write(`${(result as { html_url: string }).html_url}\n`);
}

async function main() {
  const [command, ...args] = process.argv.slice(2);
  switch (command) {
    case "validate": {
      const kind = kindArg(args[0]);
      readReport(kind, args[1]);
      process.stdout.write(`${args[1]} is a valid ${kind} report\n`);
      return;
    }
    case "schema": {
      const kind = kindArg(args[0]);
      process.stdout.write(`${JSON.stringify(z.toJSONSchema(reportSchemas[kind], { io: "input" }), null, 2)}\n`);
      return;
    }
    case "metrics": {
      const lcovPath = flag(args, "lcov");
      if (!lcovPath) fail("missing --lcov <file>");
      const { files, lines } = parseLcov(readFileSync(lcovPath, "utf8"), flag(args, "root") ?? process.cwd());
      const base = flag(args, "base-lines");
      const report = MetricsReport.parse({
        version: "1",
        commit: flag(args, "commit"),
        coverage: { tool: "lcov", lines, baseLines: base === undefined ? undefined : Number(base), files },
      });
      process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
      return;
    }
    case "comment": {
      const kind = kindArg(args[0]);
      const report = readReport(kind, args[1]);
      process.stdout.write(`${renderReportComment(kind, report, summarize(kind, report))}\n`);
      return;
    }
    case "publish": {
      const kind = kindArg(args[0]);
      await publish(kind, readReport(kind, args[1]), flag(args, "repo"), flag(args, "pr"));
      return;
    }
    default: {
      const usage = readFileSync(new URL(import.meta.url), "utf8").split("\n").slice(2, 13).map((l) => l.replace(/^ \*\s?/, ""));
      process.stdout.write(`${usage.join("\n")}\n`);
      if (command && command !== "help") process.exit(1);
    }
  }
}

main().catch((err) => fail((err as Error).message));
