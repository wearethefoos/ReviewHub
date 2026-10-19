//! Planning the security review of a pull request. A small pull request is
//! reviewed in one pass; a large one is split into batches of a few files, each
//! reviewed by its own subagent, before one agent reviews the whole across files.
//! Splitting keeps every reviewer's attention on a few files at a time.
//!
//! The plan uses only the paths and the diff, never the author's manifest: the
//! author mustn't be able to move a risky file out of review or onto a cheaper
//! model. Each batch gets a model by risk: the strong model for sensitive areas,
//! the light one for batches of only docs, the standard one otherwise. Every changed file is
//! reviewed: where a file lives is the author's choice, so it never takes a file out of review.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::classify::{detect_role, is_code_file, sensitive_area};
use crate::model::{ChangedFile, Confidence, FileRole, ReportVersion, Scanner, SecurityFinding, SecurityReport};

/// How capable (and costly) a model a part of the review needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Light,
    Standard,
    Strong,
}

/// The model each tier runs on: Claude Code model aliases (`fable`, `opus`, `sonnet`, `haiku`) or model ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Models {
    pub strong: String,
    pub standard: String,
    pub light: String,
}

impl Default for Models {
    fn default() -> Self {
        Models { strong: "opus".into(), standard: "sonnet".into(), light: "haiku".into() }
    }
}

impl Models {
    pub fn get(&self, tier: ModelTier) -> &str {
        match tier {
            ModelTier::Strong => &self.strong,
            ModelTier::Standard => &self.standard,
            ModelTier::Light => &self.light,
        }
    }
}

/// When to split, and how big a batch may get. Changed lines of lockfiles don't count: they're long and read fast.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub single_max_files: usize,
    pub single_max_lines: u32,
    pub batch_max_files: usize,
    pub batch_max_lines: u32,
    /// At most this many batches (each one a subagent): a huge pull request gets bigger batches instead.
    pub max_batches: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { single_max_files: 6, single_max_lines: 400, batch_max_files: 5, batch_max_lines: 350, max_batches: 12 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Small enough for one reviewer to read every file closely.
    Single,
    /// Batches reviewed in parallel, then one pass across files.
    Batched,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub path: String,
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    /// The sensitive area the path belongs to (authentication, payments, CI pipeline, …), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    pub id: String,
    pub tier: ModelTier,
    pub model: String,
    /// Why the batch got its model.
    pub reason: String,
    pub changed_lines: u32,
    pub files: Vec<PlannedFile>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityPlan {
    pub mode: Mode,
    /// The model of the reviewer that runs a single pass, or coordinates the batches and reviews across files.
    pub coordinator_model: String,
    pub models: Models,
    pub changed_files: usize,
    pub changed_lines: u32,
    /// Empty in single mode.
    pub batches: Vec<Batch>,
}

const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "go.mod",
    "requirements.txt",
    "pyproject.toml",
    "setup.py",
    "Gemfile",
    "composer.json",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Pipfile",
    "flake.nix",
];

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn is_lockfile(path: &str) -> bool {
    detect_role(path) == FileRole::Generated && !crate::classify::is_generated_code(path)
}

fn is_dependency_file(path: &str) -> bool {
    let name = file_name(path);
    is_lockfile(path) || MANIFESTS.contains(&name) || (name.starts_with("requirements") && name.ends_with(".txt"))
}

/// Prose nothing runs or obeys, by extension (not by folder: a script in `docs/` still runs). Not `.txt`
/// (`CMakeLists.txt` configures builds), `.mdx` (compiles to JSX) or `.snap` (Jest evaluates snapshots); and not
/// instructions for agents (`CLAUDE.md`, skills, anything in a dot folder such as `.claude/` or `.github/`).
fn is_inert(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = file_name(&lower);
    let instructions = ["claude.md", "agents.md", "gemini.md", "skill.md"].contains(&name)
        || lower
            .split('/')
            .rev()
            .skip(1)
            .any(|dir| dir.starts_with('.') || ["agents", "skills", "commands", "prompts"].contains(&dir));
    !instructions && [".md", ".rst", ".adoc"].iter().any(|ext| name.ends_with(ext))
}

/// A test's subject: `src/a.test.ts`, `src/__tests__/a.ts` and `pkg/a_test.go` all test `a`.
fn stem(path: &str) -> String {
    let name = file_name(path);
    let base = name.split('.').next().unwrap_or(name);
    let base = base.strip_prefix("test_").unwrap_or(base);
    base.strip_suffix("_test").or_else(|| base.strip_suffix("_spec")).unwrap_or(base).to_lowercase()
}

/// How many leading path segments two paths share.
fn common_prefix(a: &str, b: &str) -> usize {
    a.split('/').zip(b.split('/')).take_while(|(x, y)| x == y).count()
}

/// A group of files reviewed together: a source file and its tests, or one file.
#[derive(Clone)]
struct Unit {
    tier: ModelTier,
    lines: u32,
    files: Vec<PlannedFile>,
}

fn planned(file: &ChangedFile) -> PlannedFile {
    PlannedFile {
        path: file.path.clone(),
        status: file.status.clone(),
        additions: file.additions,
        deletions: file.deletions,
        sensitive: sensitive_area(&file.path).map(String::from),
    }
}

fn lines(file: &ChangedFile) -> u32 {
    if is_lockfile(&file.path) { 0 } else { file.additions + file.deletions }
}

/// Plans the security review of a pull request's changed files.
pub fn security_plan(files: &[ChangedFile], models: &Models, limits: &Limits) -> SecurityPlan {
    // Every changed file is reviewed: a path that looks generated (`build/`, `vendor/`, `*.gen.ts`) is the author's
    // choice, so it can't be a reason to leave a file out.
    let reviewed: Vec<&ChangedFile> = files.iter().collect();
    let total_lines: u32 = reviewed.iter().map(|f| lines(f)).sum();
    let base = SecurityPlan {
        mode: Mode::Single,
        coordinator_model: models.strong.clone(),
        models: models.clone(),
        changed_files: files.len(),
        changed_lines: files.iter().map(|f| f.additions + f.deletions).sum(),
        batches: Vec::new(),
    };
    if reviewed.len() <= limits.single_max_files && total_lines <= limits.single_max_lines {
        return base;
    }

    // Dependency changes are reviewed together, as the supply chain they are.
    let (dependencies, rest): (Vec<&ChangedFile>, Vec<&ChangedFile>) =
        reviewed.into_iter().partition(|f| is_dependency_file(&f.path));
    let (tests, sources): (Vec<&ChangedFile>, Vec<&ChangedFile>) =
        rest.into_iter().partition(|f| detect_role(&f.path) == FileRole::Test);

    let mut units: Vec<Unit> = sources
        .iter()
        .map(|f| {
            let tier = if sensitive_area(&f.path).is_some() {
                ModelTier::Strong
            } else if is_inert(&f.path) {
                ModelTier::Light
            } else {
                ModelTier::Standard
            };
            Unit { tier, lines: lines(f), files: vec![planned(f)] }
        })
        .collect();
    // Tests go with the code they test, when it changed too (the closest one by path, when several share a name);
    // otherwise with other tests.
    let mut by_stem: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, f) in sources.iter().enumerate() {
        if is_code_file(&f.path) {
            by_stem.entry(stem(&f.path)).or_default().push(i);
        }
    }
    for test in tests {
        let subject = by_stem.get(&stem(&test.path)).and_then(|candidates| {
            candidates
                .iter()
                .copied()
                .max_by_key(|&i| (common_prefix(&sources[i].path, &test.path), std::cmp::Reverse(i)))
        });
        match subject {
            Some(i) => {
                units[i].lines += lines(test);
                units[i].files.push(planned(test));
                if sensitive_area(&test.path).is_some() {
                    units[i].tier = ModelTier::Strong;
                }
            }
            // A test without its subject still runs (in CI, often with secrets), and which folder it sits in is the
            // author's choice: only inert files get the light model.
            None => units.push(Unit {
                tier: if sensitive_area(&test.path).is_some() {
                    ModelTier::Strong
                } else if is_inert(&test.path) {
                    ModelTier::Light
                } else {
                    ModelTier::Standard
                },
                lines: lines(test),
                files: vec![planned(test)],
            }),
        }
    }

    // Riskiest first, then biggest; batches only mix units of the same tier.
    units.sort_by(|a, b| b.tier.cmp(&a.tier).then(b.lines.cmp(&a.lines)));
    // Each batch is a subagent: past the cap, batches grow rather than multiply. At worst there is one per tier.
    let room = limits.max_batches.saturating_sub(usize::from(!dependencies.is_empty())).max(3);
    let mut size = (limits.batch_max_files, limits.batch_max_lines);
    let mut batches = pack(units.clone(), size, models);
    while batches.len() > room {
        size = (size.0 * 2, size.1 * 2);
        batches = pack(units.clone(), size, models);
    }
    if !dependencies.is_empty() {
        batches.push(Batch {
            id: String::new(),
            tier: ModelTier::Standard,
            model: models.standard.clone(),
            reason: String::new(),
            changed_lines: dependencies.iter().map(|f| lines(f)).sum(),
            files: dependencies.iter().map(|f| planned(f)).collect(),
        });
    }
    for (i, batch) in batches.iter_mut().enumerate() {
        batch.id = format!("batch-{}", i + 1);
        batch.reason = reason(batch);
    }
    SecurityPlan { mode: Mode::Batched, batches, ..base }
}

/// Packs units, in order, into batches of at most `max_files` files and `max_lines` changed lines, one tier each.
fn pack(units: Vec<Unit>, (max_files, max_lines): (usize, u32), models: &Models) -> Vec<Batch> {
    let mut batches: Vec<Batch> = Vec::new();
    let mut current: Option<Batch> = None;
    for unit in units {
        let fits = current.as_ref().is_some_and(|b| {
            b.tier == unit.tier
                && b.files.len() + unit.files.len() <= max_files
                && b.changed_lines + unit.lines <= max_lines
        });
        if !fits && let Some(done) = current.take() {
            batches.push(done);
        }
        let batch = current.get_or_insert_with(|| Batch {
            id: String::new(),
            tier: unit.tier,
            model: models.get(unit.tier).to_string(),
            reason: String::new(),
            changed_lines: 0,
            files: Vec::new(),
        });
        batch.changed_lines += unit.lines;
        batch.files.extend(unit.files);
    }
    batches.extend(current);
    batches
}

fn reason(batch: &Batch) -> String {
    let mut areas: Vec<&str> = Vec::new();
    for area in batch.files.iter().filter_map(|f| f.sensitive.as_deref()) {
        if !areas.contains(&area) {
            areas.push(area);
        }
    }
    if batch.files.iter().all(|f| is_dependency_file(&f.path)) {
        return "dependency changes".into();
    }
    match batch.tier {
        ModelTier::Strong => format!("sensitive: {}", areas.join(", ")),
        ModelTier::Light => "only docs".into(),
        ModelTier::Standard => "application code".into(),
    }
}

/// The same problem found twice: same file, overlapping lines, and the same category or title.
fn same_finding(a: &SecurityFinding, b: &SecurityFinding) -> bool {
    let overlap = match (a.lines, b.lines) {
        (Some(x), Some(y)) => x.start() <= y.end() && y.start() <= x.end(),
        (None, None) => true,
        _ => false,
    };
    let same_kind = match (&a.category, &b.category) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => a.title.trim().eq_ignore_ascii_case(b.title.trim()),
    };
    a.file == b.file && overlap && same_kind
}

fn confidence_rank(c: Confidence) -> u8 {
    match c {
        Confidence::Low => 0,
        Confidence::Medium => 1,
        Confidence::High => 2,
    }
}

/// Combines the batches' security reports into one, as the starting point of the pass across files: duplicates are
/// dropped (keeping the more severe, then more confident one), ids made unique, findings sorted by severity, and the
/// batches' summaries kept, prefixed with their batch, for the coordinator to rewrite.
pub fn merge_security(
    batches: &[(String, SecurityReport)],
    scanner: Scanner,
    commit: Option<String>,
) -> SecurityReport {
    let mut findings: Vec<SecurityFinding> = Vec::new();
    for (_, report) in batches {
        for finding in &report.findings {
            match findings.iter_mut().find(|f| same_finding(f, finding)) {
                Some(kept) => {
                    let better = (finding.severity, std::cmp::Reverse(confidence_rank(finding.confidence)))
                        < (kept.severity, std::cmp::Reverse(confidence_rank(kept.confidence)));
                    if better {
                        *kept = finding.clone();
                    }
                }
                None => findings.push(finding.clone()),
            }
        }
    }
    findings.sort_by_key(|f| f.severity);
    let mut seen = std::collections::BTreeSet::new();
    for finding in &mut findings {
        let base = finding.id.clone();
        let mut n = 1;
        while !seen.insert(finding.id.clone()) {
            n += 1;
            finding.id = format!("{base}-{n}");
        }
    }
    let summary = batches
        .iter()
        .filter(|(_, r)| !r.summary.trim().is_empty())
        .map(|(id, r)| format!("{id}: {}", r.summary.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    let commit = commit.or_else(|| batches.iter().find_map(|(_, r)| r.commit.clone()));
    SecurityReport { version: ReportVersion::V1, scanner, commit, summary, findings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LineRange, Severity, Suggestion};

    fn file(path: &str, lines: u32) -> ChangedFile {
        ChangedFile {
            path: path.into(),
            previous_path: None,
            status: "modified".into(),
            additions: lines,
            deletions: 0,
            patch: Some("@@ -1 +1 @@\n+x".into()),
        }
    }

    fn paths(batch: &Batch) -> Vec<&str> {
        batch.files.iter().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn reviews_small_pull_requests_in_one_pass() {
        let plan = security_plan(
            &[file("src/a.ts", 100), file("src/a.test.ts", 50), file("src/__snapshots__/a.snap", 30)],
            &Models::default(),
            &Limits::default(),
        );
        assert_eq!(plan.mode, Mode::Single);
        assert_eq!(plan.coordinator_model, "opus");
        assert!(plan.batches.is_empty());
        assert_eq!(plan.changed_lines, 180);
    }

    #[test]
    fn batches_large_pull_requests_by_risk() {
        let files = [
            file("src/payments/refunds.ts", 120),
            file("src/payments/refunds.test.ts", 90),
            file("src/api/routes/orders.ts", 200),
            file("src/api/routes/users.ts", 200),
            file("src/ui/button.tsx", 40),
            file("src/ui/form.tsx", 60),
            file("test/e2e/profile.spec.ts", 80),
            file("docs/api.md", 30),
            file(".github/workflows/ci.yml", 10),
            file("package.json", 3),
            file("package-lock.json", 4000),
            file("src/__snapshots__/a.snap", 500),
        ];
        let models = Models { strong: "fable".into(), ..Models::default() };
        let plan = security_plan(&files, &models, &Limits::default());
        assert_eq!(plan.mode, Mode::Batched);
        assert_eq!(plan.coordinator_model, "fable");
        let summary: Vec<(&str, &str, Vec<&str>)> =
            plan.batches.iter().map(|b| (b.id.as_str(), b.model.as_str(), paths(b))).collect();
        assert_eq!(
            summary,
            vec![
                // Sensitive files on the strong model, each test with its source.
                (
                    "batch-1",
                    "fable",
                    vec!["src/payments/refunds.ts", "src/payments/refunds.test.ts", ".github/workflows/ci.yml"]
                ),
                // Application code on the standard model, within the line budget; a test without its source
                // still runs, so it goes there too.
                // Jest evaluates snapshots: they aren't prose.
                ("batch-2", "sonnet", vec!["src/__snapshots__/a.snap"]),
                ("batch-3", "sonnet", vec!["src/api/routes/orders.ts"]),
                ("batch-4", "sonnet", vec!["src/api/routes/users.ts", "test/e2e/profile.spec.ts", "src/ui/form.tsx"]),
                ("batch-5", "sonnet", vec!["src/ui/button.tsx"]),
                // Docs on the light model; nothing is left out.
                ("batch-6", "haiku", vec!["docs/api.md"]),
                // The supply chain together; lockfile lines don't count against the budget.
                ("batch-7", "sonnet", vec!["package.json", "package-lock.json"]),
            ]
        );
        assert_eq!(plan.batches[0].reason, "sensitive: payments, CI pipeline");
        assert_eq!(plan.batches[5].reason, "only docs");
        assert_eq!(plan.batches[6].reason, "dependency changes");
        assert_eq!(plan.batches[6].changed_lines, 3);
    }

    #[test]
    fn splits_when_there_are_many_small_files() {
        let files: Vec<ChangedFile> = (0..12).map(|i| file(&format!("src/f{i}.ts"), 5)).collect();
        let plan = security_plan(&files, &Models::default(), &Limits::default());
        assert_eq!(plan.mode, Mode::Batched);
        assert_eq!(plan.batches.iter().map(|b| b.files.len()).collect::<Vec<_>>(), vec![5, 5, 2]);
    }

    #[test]
    fn caps_the_number_of_batches() {
        let mut files: Vec<ChangedFile> = (0..1000).map(|i| file(&format!("src/f{i}.ts"), 20)).collect();
        files.push(file("src/auth/login.ts", 20));
        files.push(file("docs/guide.md", 20));
        files.push(file("package.json", 1));
        let plan = security_plan(&files, &Models::default(), &Limits::default());
        assert!(plan.batches.len() <= Limits::default().max_batches, "{}", plan.batches.len());
        assert_eq!(plan.batches.iter().map(|b| b.files.len()).sum::<usize>(), files.len(), "every file is reviewed");
        // Bigger batches, but still one tier each and the strong model for the sensitive file.
        let tiers: Vec<&str> = plan.batches.iter().map(|b| b.model.as_str()).collect();
        assert_eq!(tiers.first(), Some(&"opus"));
        assert!(tiers.contains(&"haiku") && tiers.last() == Some(&"sonnet"), "{tiers:?}");
        // Even a cap below the number of tiers leaves one batch per tier.
        let tight = Limits { max_batches: 1, ..Limits::default() };
        assert_eq!(security_plan(&files, &Models::default(), &tight).batches.len(), 4);
    }

    #[test]
    fn finds_what_a_test_tests() {
        assert_eq!(stem("src/a.test.ts"), "a");
        assert_eq!(stem("pkg/server_test.go"), "server");
        assert_eq!(stem("tests/test_views.py"), "views");
        assert_eq!(stem("spec/user_spec.rb"), "user");
    }

    fn finding(id: &str, file: &str, lines: (u32, u32), category: &str, severity: Severity) -> SecurityFinding {
        SecurityFinding {
            id: id.into(),
            title: format!("{category} in {file}"),
            severity,
            category: Some(category.into()),
            description: String::new(),
            file: file.into(),
            lines: Some(LineRange(lines.0, lines.1)),
            confidence: Confidence::Medium,
            suggestion: Suggestion { explanation: "Fix it.".into(), diff: None, prompt: None },
        }
    }

    fn report(summary: &str, findings: Vec<SecurityFinding>) -> SecurityReport {
        SecurityReport {
            version: ReportVersion::V1,
            scanner: Scanner { name: "batch".into(), model: Some("sonnet".into()) },
            commit: Some("abc".into()),
            summary: summary.into(),
            findings,
        }
    }

    #[test]
    fn merges_batch_reports() {
        let a = report(
            "One injection.",
            vec![
                finding("sqli", "src/a.ts", (10, 12), "CWE-89", Severity::Medium),
                finding("x", "src/a.ts", (40, 40), "CWE-79", Severity::Low),
            ],
        );
        let b = report(
            "",
            vec![
                // The same injection, seen from its caller's batch as more severe.
                finding("sqli", "src/a.ts", (12, 14), "cwe-89", Severity::High),
                finding("authz", "src/b.ts", (3, 3), "CWE-639", Severity::Critical),
            ],
        );
        let merged = merge_security(
            &[("batch-1".into(), a), ("batch-2".into(), b)],
            Scanner { name: "reviewhub security-reviewer".into(), model: Some("opus".into()) },
            None,
        );
        let got: Vec<(&str, Severity)> = merged.findings.iter().map(|f| (f.id.as_str(), f.severity)).collect();
        assert_eq!(got, vec![("authz", Severity::Critical), ("sqli", Severity::High), ("x", Severity::Low)]);
        assert_eq!(merged.summary, "batch-1: One injection.");
        assert_eq!(merged.commit.as_deref(), Some("abc"));
        assert_eq!(merged.scanner.model.as_deref(), Some("opus"));

        // Different problems that happen to share an id keep both, with unique ids.
        let c = report("", vec![finding("f1", "src/c.ts", (1, 1), "CWE-22", Severity::Low)]);
        let d = report("", vec![finding("f1", "src/d.ts", (1, 1), "CWE-22", Severity::Low)]);
        let merged = merge_security(
            &[("1".into(), c), ("2".into(), d)],
            Scanner { name: "s".into(), model: None },
            Some("h".into()),
        );
        assert_eq!(merged.findings.iter().map(|f| f.id.as_str()).collect::<Vec<_>>(), vec!["f1", "f1-2"]);

        // Without lines or a category, the file and the title decide; a finding with lines differs from one without.
        let mut whole_file = finding("leak", "src/e.ts", (1, 1), "x", Severity::Low);
        (whole_file.lines, whole_file.category) = (None, None);
        let mut same = whole_file.clone();
        same.title = format!("  {}  ", whole_file.title.to_uppercase());
        same.confidence = Confidence::High;
        let mut other_title = whole_file.clone();
        other_title.title = "Something else".into();
        let mut unsure = same.clone();
        unsure.confidence = Confidence::Low;
        let with_lines = finding("leak", "src/e.ts", (5, 6), "x", Severity::Low);
        let titled = merge_security(
            &[("1".into(), report("", vec![whole_file, same, unsure, other_title, with_lines]))],
            Scanner { name: "s".into(), model: None },
            None,
        );
        let got: Vec<(&str, Option<&str>, Confidence)> =
            titled.findings.iter().map(|f| (f.id.as_str(), f.category.as_deref(), f.confidence)).collect();
        assert_eq!(
            got,
            vec![
                ("leak", None, Confidence::High),
                ("leak-2", None, Confidence::Medium),
                ("leak-3", Some("x"), Confidence::Medium)
            ],
            "the more confident duplicate wins"
        );
        assert_eq!(merged.commit.as_deref(), Some("h"));
    }

    #[test]
    fn pairs_tests_with_the_closest_code() {
        let files = [
            file("docs/api/refunds.md", 40),
            file("src/api/routes/refunds.ts", 200),
            file("src/payments/refunds.ts", 200),
            file("src/payments/refunds.test.ts", 100),
        ];
        let plan = security_plan(&files, &Models::default(), &Limits::default());
        let summary: Vec<(&str, Vec<&str>)> = plan.batches.iter().map(|b| (b.model.as_str(), paths(b))).collect();
        assert_eq!(
            summary,
            vec![
                ("opus", vec!["src/payments/refunds.ts", "src/payments/refunds.test.ts"]),
                // Docs about a sensitive area get the strong model too.
                ("opus", vec!["src/api/routes/refunds.ts", "docs/api/refunds.md"]),
            ]
        );
    }

    #[test]
    fn where_a_file_lives_never_takes_it_out_of_review() {
        let files = [
            file("src/generated/auth.ts", 100),
            file("scripts/build/deploy.sh", 100),
            file("docs/auth_middleware.ts", 100),
            file("vendor/lib/parse.js", 100),
            file("docs/guide.md", 100),
            file("CMakeLists.txt", 100),
            file("test/fixtures/notes.md", 100),
            file("src/__snapshots__/a.test.ts.snap", 100),
            file("docs/intro.mdx", 100),
            file("CLAUDE.md", 100),
            file(".claude/agents/reviewer.md", 100),
            file("plugin/skills/review/notes.md", 100),
            file("test/.github/notes.md", 100),
            file("docs/payments.md", 100),
            file("test/auth/login.test.ts", 100),
            file("src/ui/banner.ts", 100),
            file("src/ui/auth/banner.test.ts", 100),
            file("src/a.ts", 100),
            file("src/b.ts", 100),
        ];
        let plan = security_plan(&files, &Models::default(), &Limits { batch_max_files: 1, ..Limits::default() });
        let model = |path: &str| {
            plan.batches.iter().find(|b| b.files.iter().any(|f| f.path == path)).map(|b| b.model.as_str()).unwrap()
        };
        assert_eq!(model("src/generated/auth.ts"), "opus");
        assert_eq!(model("docs/auth_middleware.ts"), "opus", "code in docs/ is still code");
        assert_eq!(model("scripts/build/deploy.sh"), "sonnet");
        assert_eq!(model("vendor/lib/parse.js"), "sonnet");
        assert_eq!(model("docs/guide.md"), "haiku");
        assert_eq!(model("CMakeLists.txt"), "sonnet", ".txt files can configure builds");
        assert_eq!(model("test/fixtures/notes.md"), "haiku", "prose in a test folder doesn't run");
        assert_eq!(model("src/__snapshots__/a.test.ts.snap"), "sonnet", "Jest evaluates snapshots");
        assert_eq!(model("docs/intro.mdx"), "sonnet", "MDX compiles to JSX");
        for steers_agents in ["CLAUDE.md", ".claude/agents/reviewer.md", "plugin/skills/review/notes.md"] {
            assert_eq!(model(steers_agents), "sonnet", "{steers_agents}");
        }
        assert_eq!(model("test/.github/notes.md"), "sonnet", "a test without its subject in a dot folder");
        assert_eq!(model("docs/payments.md"), "opus", "docs in a sensitive area");
        assert_eq!(model("test/auth/login.test.ts"), "opus", "a test in a sensitive area without its subject");
        assert_eq!(model("src/ui/auth/banner.test.ts"), "opus", "a test in a sensitive area lifts its subject's batch");
        assert_eq!(model("src/ui/banner.ts"), "opus");
        assert_eq!(plan.batches.iter().map(|b| b.files.len()).sum::<usize>(), files.len());
    }
}
