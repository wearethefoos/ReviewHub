import type { FileRole } from "../schema";

const LOCKFILES = new Set([
  "package-lock.json",
  "yarn.lock",
  "pnpm-lock.yaml",
  "bun.lockb",
  "Cargo.lock",
  "go.sum",
  "poetry.lock",
  "uv.lock",
  "Gemfile.lock",
  "composer.lock",
  "Pipfile.lock",
  "flake.lock",
]);

const GENERATED = [
  /(^|\/)__snapshots__\//,
  /\.snap$/,
  /\.min\.(js|css)$/,
  /(^|\/)(dist|build|vendor|node_modules)\//,
  /(^|\/)generated\//,
  /\.gen\.[a-z]+$/,
  /\.pb\.go$/,
  /_pb2(_grpc)?\.py$/,
  /\.g\.dart$/,
];

const TESTS = [
  /(^|\/)(test|tests|__tests__|spec|e2e|cypress|playwright)\//,
  /\.(test|spec)\.[a-z]+$/,
  /_test\.(go|py|rb|exs?)$/,
  /(^|\/)test_[^/]+\.py$/,
  /_spec\.rb$/,
];

const DOCS = [/\.(md|mdx|rst|adoc|txt)$/i, /(^|\/)docs?\//, /(^|\/)(LICENSE|CHANGELOG|AUTHORS)(\.|$)/];

const CONFIG = [
  /\.(json|ya?ml|toml|ini|cfg|conf|properties|env\.example)$/,
  /(^|\/)\.[^/]+$/,
  /(^|\/)Dockerfile[^/]*$/,
  /(^|\/)Makefile$/,
  /(^|\/)\.github\//,
  /\.config\.[cm]?[jt]s$/,
];

/** Best-effort role detection from the path alone. */
export function detectRole(path: string): FileRole {
  const name = path.split("/").at(-1) ?? path;
  if (LOCKFILES.has(name) || GENERATED.some((re) => re.test(path))) return "generated";
  if (TESTS.some((re) => re.test(path))) return "test";
  if (DOCS.some((re) => re.test(path))) return "docs";
  if (CONFIG.some((re) => re.test(path))) return "config";
  return "core";
}

const SENSITIVE: [RegExp, string][] = [
  [/auth(?!or)|login|session|oauth|sso|jwt/i, "authentication"],
  [/permission|rbac|acl|policy|role/i, "authorization"],
  [/crypt|cipher|hash|sign(ing|ature)|hmac/i, "cryptography"],
  [/secret|credential|token|password|api[-_]?key/i, "secrets"],
  [/payment|billing|invoice|checkout|refund|charge/i, "payments"],
  [/migrat|schema\.(sql|prisma|rb)|\.sql$/i, "data migration"],
  [/(^|\/)\.github\/workflows\//, "CI pipeline"],
];

/** Returns the sensitive area a path belongs to, if any. Independent of what the agent reports. */
export function sensitiveArea(path: string): string | null {
  for (const [re, label] of SENSITIVE) if (re.test(path)) return label;
  return null;
}

const CODE_EXTENSIONS = /\.(tsx?|[cm]?jsx?|py|go|rb|java|kt|kts|rs|cs|php|swift|scala|exs?|c|cc|cpp|h|hpp|m|dart|vue|svelte)$/;

/** Whether a path is source code that coverage tools would instrument. */
export function isCodeFile(path: string): boolean {
  return CODE_EXTENSIONS.test(path);
}
