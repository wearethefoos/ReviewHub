//! Path-based file classification. Port of `src/lib/focus/classify.ts`;
//! keep the patterns identical.

use std::sync::LazyLock;

use regex::{Regex, RegexSet};

use crate::model::FileRole;

const LOCKFILES: &[&str] = &[
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
];

fn set(patterns: &[&str]) -> RegexSet {
    RegexSet::new(patterns).expect("valid patterns")
}

static GENERATED: LazyLock<RegexSet> = LazyLock::new(|| {
    set(&[
        r"(^|/)__snapshots__/",
        r"\.snap$",
        r"\.min\.(js|css)$",
        r"(^|/)(dist|build|vendor|node_modules)/",
        r"(^|/)generated/",
        r"\.gen\.[a-z]+$",
        r"\.pb\.go$",
        r"_pb2(_grpc)?\.py$",
        r"\.g\.dart$",
    ])
});

static TESTS: LazyLock<RegexSet> = LazyLock::new(|| {
    set(&[
        r"(^|/)(test|tests|__tests__|spec|e2e|cypress|playwright)/",
        r"\.(test|spec)\.[a-z]+$",
        r"_test\.(go|py|rb|exs?)$",
        r"(^|/)test_[^/]+\.py$",
        r"_spec\.rb$",
    ])
});

static DOCS: LazyLock<RegexSet> = LazyLock::new(|| {
    set(&[r"(?i)\.(md|mdx|rst|adoc|txt)$", r"(^|/)docs?/", r"(^|/)(LICENSE|CHANGELOG|AUTHORS)(\.|$)"])
});

static CONFIG: LazyLock<RegexSet> = LazyLock::new(|| {
    set(&[
        r"\.(json|ya?ml|toml|ini|cfg|conf|properties|env\.example)$",
        r"(^|/)\.[^/]+$",
        r"(^|/)Dockerfile[^/]*$",
        r"(^|/)Makefile$",
        r"(^|/)\.github/",
        r"\.config\.[cm]?[jt]s$",
    ])
});

/// Generated code (snapshots, bundles, vendored and generated sources), as opposed to lockfiles.
pub fn is_generated_code(path: &str) -> bool {
    GENERATED.is_match(path)
}

/// Best-effort role detection from the path alone.
pub fn detect_role(path: &str) -> FileRole {
    let name = path.rsplit('/').next().unwrap_or(path);
    if LOCKFILES.contains(&name) || GENERATED.is_match(path) {
        FileRole::Generated
    } else if TESTS.is_match(path) {
        FileRole::Test
    } else if DOCS.is_match(path) {
        FileRole::Docs
    } else if CONFIG.is_match(path) {
        FileRole::Config
    } else {
        FileRole::Core
    }
}

static SENSITIVE: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    [
        (r"(?i)auth($|[^o]|o($|[^r]))|login|session|oauth|sso|jwt", "authentication"),
        (r"(?i)permission|rbac|acl|policy|role", "authorization"),
        (r"(?i)crypt|cipher|hash|sign(ing|ature)|hmac", "cryptography"),
        (r"(?i)secret|credential|token|password|api[-_]?key", "secrets"),
        (r"(?i)payment|billing|invoice|checkout|refund|charge", "payments"),
        (r"(?i)migrat|schema\.(sql|prisma|rb)|\.sql$", "data migration"),
        (r"(^|/)\.github/workflows/", "CI pipeline"),
    ]
    .into_iter()
    .map(|(re, label)| (Regex::new(re).expect("valid pattern"), label))
    .collect()
});

/// The sensitive area a path belongs to, if any. Independent of what the agent reports.
pub fn sensitive_area(path: &str) -> Option<&'static str> {
    SENSITIVE.iter().find(|(re, _)| re.is_match(path)).map(|(_, label)| *label)
}

static CODE_EXTENSIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\.(tsx?|[cm]?jsx?|py|go|rb|java|kt|kts|rs|cs|php|swift|scala|exs?|c|cc|cpp|h|hpp|m|dart|vue|svelte)$")
        .unwrap()
});

/// Whether a path is source code that coverage tools would instrument.
pub fn is_code_file(path: &str) -> bool {
    CODE_EXTENSIONS.is_match(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_roles() {
        for (path, role) in [
            ("package-lock.json", FileRole::Generated),
            ("src/__snapshots__/a.test.ts.snap", FileRole::Generated),
            ("src/a.test.ts", FileRole::Test),
            ("pkg/server_test.go", FileRole::Test),
            ("README.md", FileRole::Docs),
            (".github/workflows/ci.yml", FileRole::Config),
            ("src/server.ts", FileRole::Core),
        ] {
            assert_eq!(detect_role(path), role, "{path}");
        }
    }

    #[test]
    fn flags_sensitive_paths_but_not_lookalikes() {
        assert_eq!(sensitive_area("src/auth/session.ts"), Some("authentication"));
        assert_eq!(sensitive_area("src/oauth.ts"), Some("authentication"));
        assert_eq!(sensitive_area("src/authors.ts"), None);
    }
}
