//! Focus scoring: ranks every changed file as Focus, Skim or Skip from the
//! reports and independent signals (coverage, complexity, sensitive paths).
//! The weights are documented in docs/reports.md; keep that table in sync.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::classify::{detect_role, is_code_file, sensitive_area};
use crate::diff::{Hunk, added_lines, parse_patch};
use crate::model::{
    ChangedFile, DeviationSeverity, FileNote, FileRef, FileRole, Impact, LineRange, MetricsReport, ReviewHint,
    ReviewManifest, SecurityReport, Severity, ValidationResult,
};

pub const FOCUS_THRESHOLD: f64 = 40.0;
pub const SKIM_THRESHOLD: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Focus,
    Skim,
    Skip,
}

impl Tier {
    pub const ALL: [Tier; 3] = [Tier::Focus, Tier::Skim, Tier::Skip];

    pub fn for_score(score: f64) -> Tier {
        if score >= FOCUS_THRESHOLD {
            Tier::Focus
        } else if score >= SKIM_THRESHOLD {
            Tier::Skim
        } else {
            Tier::Skip
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum SignalKind {
    Security,
    Decision,
    Deviation,
    Gap,
    Validation,
    Coverage,
    Complexity,
    Sensitive,
    Size,
    Role,
    AgentHint,
}

impl SignalKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Security => "security",
            Self::Decision => "decision",
            Self::Deviation => "deviation",
            Self::Gap => "gap",
            Self::Validation => "validation",
            Self::Coverage => "coverage",
            Self::Complexity => "complexity",
            Self::Sensitive => "sensitive",
            Self::Size => "size",
            Self::Role => "role",
            Self::AgentHint => "agent-hint",
        }
    }

    /// Kinds that point at real risk, as opposed to the file's shape.
    fn is_risk(self) -> bool {
        matches!(
            self,
            Self::Security
                | Self::Decision
                | Self::Deviation
                | Self::Gap
                | Self::Validation
                | Self::Coverage
                | Self::Complexity
                | Self::Sensitive
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Signal {
    pub kind: SignalKind,
    pub label: String,
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
/// Declared in display priority: when annotations overlap, the first kind wins.
pub enum AnnotationKind {
    Security,
    Deviation,
    Decision,
    Gap,
    Uncovered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub start: u32,
    pub end: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FileFocus {
    pub path: String,
    pub role: FileRole,
    pub score: f64,
    pub tier: Tier,
    pub uncovered_changed_lines: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Whether `role` came from the agent's manifest or from the path.
    pub role_from_agent: bool,
    pub signals: Vec<Signal>,
    pub annotations: Vec<Annotation>,
    pub file: ChangedFile,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct TierTotals {
    pub files: u32,
    pub lines: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Totals {
    pub all: TierTotals,
    pub focus: TierTotals,
    pub skim: TierTotals,
    pub skip: TierTotals,
}

impl Totals {
    pub fn get(&self, tier: Tier) -> TierTotals {
        match tier {
            Tier::Focus => self.focus,
            Tier::Skim => self.skim,
            Tier::Skip => self.skip,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FocusAnalysis {
    pub totals: Totals,
    pub dangling_refs: Vec<String>,
    pub files: Vec<FileFocus>,
}

pub struct FocusInput<'a> {
    pub files: &'a [ChangedFile],
    pub manifest: Option<&'a ReviewManifest>,
    pub security: Option<&'a SecurityReport>,
    pub metrics: Option<&'a MetricsReport>,
}

fn role_base(role: FileRole) -> f64 {
    match role {
        FileRole::Core | FileRole::Test => 10.0,
        FileRole::Supporting | FileRole::Config => 6.0,
        FileRole::Mechanical | FileRole::Docs => 1.0,
        FileRole::Generated => 0.0,
    }
}

fn security_weight(severity: Severity) -> f64 {
    match severity {
        Severity::Critical => 60.0,
        Severity::High => 45.0,
        Severity::Medium => 25.0,
        Severity::Low => 10.0,
        Severity::Info => 2.0,
    }
}

fn decision_weight(impact: Impact) -> f64 {
    match impact {
        Impact::High => 30.0,
        Impact::Medium => 15.0,
        Impact::Low => 5.0,
    }
}

fn impact_str(impact: Impact) -> &'static str {
    match impact {
        Impact::High => "high",
        Impact::Medium => "medium",
        Impact::Low => "low",
    }
}

const GAP_WEIGHT: f64 = 18.0;
const FAILED_VALIDATION_WEIGHT: f64 = 25.0;

fn refs_for<'a>(refs: &'a [FileRef], path: &str) -> Vec<&'a FileRef> {
    refs.iter().filter(|r| r.path == path).collect()
}

fn collapse_ranges(lines: &[u32]) -> Vec<(u32, u32)> {
    let sorted: BTreeSet<u32> = lines.iter().copied().collect();
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for n in sorted {
        match ranges.last_mut() {
            Some(last) if n == last.1 + 1 => last.1 = n,
            _ => ranges.push((n, n)),
        }
    }
    ranges
}

fn signal(kind: SignalKind, label: String, weight: f64, reference: Option<String>) -> Signal {
    Signal { kind, label, weight, reference }
}

fn analyze_file(file: &ChangedFile, input: &FocusInput, notes: &HashMap<&str, &FileNote>) -> FileFocus {
    let hunks = parse_patch(file.patch.as_deref());
    let note = notes.get(file.path.as_str()).copied();
    let role = note.map(|n| n.role).unwrap_or_else(|| detect_role(&file.path));
    let mut signals: Vec<Signal> = Vec::new();
    let mut annotations: Vec<Annotation> = Vec::new();
    let churn = file.additions + file.deletions;
    let path = file.path.as_str();

    let per_agent = if note.is_some() { " (per agent)" } else { "" };
    signals.push(signal(SignalKind::Role, format!("{} file{per_agent}", role.as_str()), role_base(role), None));

    if role != FileRole::Generated && role != FileRole::Mechanical {
        let factor = if matches!(role, FileRole::Test | FileRole::Docs) { 0.5 } else { 1.0 };
        let size = ((churn as f64).sqrt() * factor).round().min(15.0);
        if size > 0.0 {
            signals.push(signal(SignalKind::Size, format!("{churn} changed lines"), size, None));
        }
    }

    if let Some(area) = sensitive_area(path).filter(|_| role != FileRole::Test && role != FileRole::Docs) {
        signals.push(signal(SignalKind::Sensitive, format!("Touches {area}"), 10.0, None));
    }

    if let Some(security) = input.security {
        for f in security.findings.iter().filter(|f| f.file == path) {
            let r = format!("security-{}", f.id);
            let label = format!("{} security finding: {}", f.severity.as_str(), f.title);
            signals.push(signal(SignalKind::Security, label, security_weight(f.severity), Some(r.clone())));
            if let Some(LineRange(start, end)) = f.lines {
                annotations.push(Annotation {
                    kind: AnnotationKind::Security,
                    start,
                    end,
                    title: f.title.clone(),
                    detail: Some(f.description.clone()),
                    severity: Some(f.severity),
                    reference: Some(r),
                });
            }
        }
    }

    if let Some(manifest) = input.manifest {
        let mut annotate = |kind, refs: &[&FileRef], title: &str, detail: &str, r: &str| {
            for fr in refs {
                if let Some(LineRange(start, end)) = fr.lines {
                    annotations.push(Annotation {
                        kind,
                        start,
                        end,
                        title: title.to_string(),
                        detail: Some(detail.to_string()),
                        severity: None,
                        reference: Some(r.to_string()),
                    });
                }
            }
        };

        for d in &manifest.decisions {
            let refs = refs_for(&d.files, path);
            if refs.is_empty() {
                continue;
            }
            let r = format!("decision-{}", d.id);
            let label = format!("{}-impact decision: {}", impact_str(d.impact), d.title);
            signals.push(signal(SignalKind::Decision, label, decision_weight(d.impact), Some(r.clone())));
            annotate(AnnotationKind::Decision, &refs, &d.title, &d.chosen, &r);
        }

        for d in &manifest.deviations {
            let refs = refs_for(&d.files, path);
            if refs.is_empty() {
                continue;
            }
            let r = format!("deviation-{}", d.id);
            let (sev, weight) = match d.severity {
                DeviationSeverity::Major => ("major", 30.0),
                DeviationSeverity::Minor => ("minor", 12.0),
            };
            signals.push(signal(
                SignalKind::Deviation,
                format!("{sev} deviation: {}", d.title),
                weight,
                Some(r.clone()),
            ));
            annotate(AnnotationKind::Deviation, &refs, &d.title, &d.reason, &r);
        }

        for g in &manifest.gaps {
            let refs = refs_for(&g.files, path);
            if refs.is_empty() {
                continue;
            }
            let r = format!("gap-{}", g.id);
            signals.push(signal(SignalKind::Gap, format!("Not validated: {}", g.title), GAP_WEIGHT, Some(r.clone())));
            annotate(AnnotationKind::Gap, &refs, &g.title, &g.why, &r);
        }

        for v in &manifest.validation {
            let result = match v.result {
                ValidationResult::Passed | ValidationResult::Skipped => continue,
                ValidationResult::Failed => "failed",
                ValidationResult::Partial => "partial",
            };
            if refs_for(&v.files, path).is_empty() {
                continue;
            }
            let label = format!("Validation {result}: {}", v.description);
            let r = Some(format!("validation-{}", v.id));
            signals.push(signal(SignalKind::Validation, label, FAILED_VALIDATION_WEIGHT, r));
        }
    }

    let mut uncovered_changed_lines = 0;
    let coverage = input.metrics.and_then(|m| m.coverage.as_ref());
    if let Some(coverage) =
        coverage.filter(|_| matches!(role, FileRole::Core | FileRole::Supporting) && is_code_file(path))
    {
        match coverage.files.iter().find(|c| c.path == path) {
            None => {
                if file.status != "removed" && file.additions > 0 {
                    signals.push(signal(SignalKind::Coverage, "Not exercised by any test".into(), 15.0, None));
                }
            }
            Some(entry) => {
                let added: HashSet<u32> = added_lines(&hunks).into_iter().collect();
                let uncovered: Vec<u32> = entry.uncovered_lines.iter().copied().filter(|n| added.contains(n)).collect();
                uncovered_changed_lines = uncovered.len() as u32;
                if uncovered.is_empty() {
                    let label = format!("All changed lines covered ({}% of file)", entry.lines.round());
                    signals.push(signal(SignalKind::Coverage, label, 0.0, None));
                } else {
                    let label = format!("{} changed lines not covered by tests", uncovered.len());
                    signals.push(signal(SignalKind::Coverage, label, (uncovered.len() as f64 * 2.0).min(25.0), None));
                    for (start, end) in collapse_ranges(&uncovered) {
                        annotations.push(Annotation {
                            kind: AnnotationKind::Uncovered,
                            start,
                            end,
                            title: "Not covered by tests".into(),
                            detail: None,
                            severity: None,
                            reference: None,
                        });
                    }
                }
            }
        }
    }

    let quality = input.metrics.and_then(|m| m.quality.as_ref()).and_then(|q| q.files.iter().find(|f| f.path == path));
    if let Some(delta) = quality.and_then(|q| q.complexity_delta).filter(|d| *d > 0.0) {
        signals.push(signal(SignalKind::Complexity, format!("Complexity +{delta}"), (delta * 2.0).min(20.0), None));
    }
    if let Some(n) = quality.and_then(|q| q.lint_issues).filter(|n| *n > 0) {
        signals.push(signal(SignalKind::Complexity, format!("{n} lint issues"), (f64::from(n) * 2.0).min(10.0), None));
    }

    let mut score: f64 = signals.iter().map(|s| s.weight).sum();
    let has_security = signals.iter().any(|s| s.kind == SignalKind::Security);

    // Generated files only surface when the security agent found something in them.
    if role == FileRole::Generated && !has_security {
        score = score.min(5.0);
    }

    // The agent's own hints: "must-review" always surfaces, "skip" only holds if
    // nothing independent of the agent's judgement says otherwise.
    match note.and_then(|n| n.review_hint) {
        Some(ReviewHint::MustReview) if score < FOCUS_THRESHOLD => {
            let label = "Agent asked for a careful review".to_string();
            signals.push(signal(SignalKind::AgentHint, label, FOCUS_THRESHOLD - score, None));
            score = FOCUS_THRESHOLD;
        }
        Some(ReviewHint::Skip) => {
            let independent: Vec<&str> = signals
                .iter()
                .filter(|s| s.kind.is_risk() && s.weight > 0.0)
                .filter(|s| !matches!(s.kind, SignalKind::Decision | SignalKind::Deviation | SignalKind::Gap))
                .map(|s| s.kind.as_str())
                .collect();
            if independent.is_empty() {
                let capped = score.min(SKIM_THRESHOLD - 1.0);
                let label = "Agent marked this as safe to skip".to_string();
                signals.push(signal(SignalKind::AgentHint, label, capped - score, None));
                score = capped;
            } else {
                let label = format!("Agent suggested skipping, overridden by: {}", independent.join(", "));
                signals.push(signal(SignalKind::AgentHint, label, 0.0, None));
            }
        }
        _ => {}
    }

    let score = score.round().clamp(0.0, 100.0);
    annotations.sort_by_key(|a| a.start);
    signals.sort_by(|a, b| b.weight.total_cmp(&a.weight));

    FileFocus {
        path: file.path.clone(),
        role,
        score,
        tier: Tier::for_score(score),
        uncovered_changed_lines,
        note: note.and_then(|n| n.note.clone()),
        role_from_agent: note.is_some(),
        signals,
        annotations,
        file: file.clone(),
        hunks,
    }
}

pub fn analyze(input: &FocusInput) -> FocusAnalysis {
    let notes: HashMap<&str, &FileNote> =
        input.manifest.map(|m| m.files.iter().map(|n| (n.path.as_str(), n)).collect()).unwrap_or_default();

    let mut files: Vec<FileFocus> = input.files.iter().map(|f| analyze_file(f, input, &notes)).collect();
    files.sort_by(|a, b| {
        a.tier
            .cmp(&b.tier)
            .then(b.score.total_cmp(&a.score))
            .then_with(|| a.path.encode_utf16().cmp(b.path.encode_utf16()))
    });

    let mut totals = Totals::default();
    for f in &files {
        let lines = f.file.additions + f.file.deletions;
        let tier = match f.tier {
            Tier::Focus => &mut totals.focus,
            Tier::Skim => &mut totals.skim,
            Tier::Skip => &mut totals.skip,
        };
        for t in [tier, &mut totals.all] {
            t.files += 1;
            t.lines += lines;
        }
    }

    let changed: HashSet<&str> = input.files.iter().map(|f| f.path.as_str()).collect();
    let mut referenced: Vec<&str> = Vec::new();
    if let Some(m) = input.manifest {
        referenced.extend(m.decisions.iter().flat_map(|d| d.files.iter().map(|r| r.path.as_str())));
        referenced.extend(m.deviations.iter().flat_map(|d| d.files.iter().map(|r| r.path.as_str())));
        referenced.extend(m.gaps.iter().flat_map(|g| g.files.iter().map(|r| r.path.as_str())));
    }
    if let Some(s) = input.security {
        referenced.extend(s.findings.iter().map(|f| f.file.as_str()));
    }
    let dangling: BTreeSet<String> =
        referenced.into_iter().filter(|p| !changed.contains(p)).map(String::from).collect();

    FocusAnalysis { totals, dangling_refs: dangling.into_iter().collect(), files }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PullRequestBundle;

    fn demo() -> FocusAnalysis {
        let bundle: PullRequestBundle =
            serde_json::from_str(include_str!("../../../fixtures/demo/bundle.json")).unwrap();
        analyze(&FocusInput {
            files: &bundle.files,
            manifest: bundle.manifest(),
            security: bundle.security(),
            metrics: bundle.metrics(),
        })
    }

    fn file<'a>(a: &'a FocusAnalysis, path: &str) -> &'a FileFocus {
        a.files.iter().find(|f| f.path == path).unwrap()
    }

    #[test]
    fn demo_puts_risky_files_first_and_noise_last() {
        let a = demo();
        let tiers: Vec<(&str, Tier, f64)> = a.files.iter().map(|f| (f.path.as_str(), f.tier, f.score)).collect();
        assert_eq!(
            tiers[..5],
            [
                ("src/api/routes/refunds.ts", Tier::Focus, 100.0),
                ("src/payments/refunds.ts", Tier::Focus, 100.0),
                ("src/lib/money.ts", Tier::Focus, 55.0),
                ("db/migrations/20261012_add_refunds.sql", Tier::Focus, 41.0),
                ("src/payments/refunds.test.ts", Tier::Skim, 13.0),
            ]
        );
        assert!(a.files[5..].iter().all(|f| f.tier == Tier::Skip));
        assert_eq!((a.totals.all.files, a.totals.all.lines, a.totals.focus.lines), (12, 221, 130));
        assert!(a.dangling_refs.is_empty());
    }

    #[test]
    fn annotates_findings_decisions_and_uncovered_lines() {
        let a = demo();
        let refunds = file(&a, "src/payments/refunds.ts");
        let kinds: Vec<(AnnotationKind, u32, u32)> =
            refunds.annotations.iter().map(|x| (x.kind, x.start, x.end)).collect();
        assert!(kinds.contains(&(AnnotationKind::Security, 28, 30)));
        assert!(kinds.contains(&(AnnotationKind::Uncovered, 26, 26)));
        assert_eq!(refunds.uncovered_changed_lines, 2);
        assert!(refunds.annotations.windows(2).all(|w| w[0].start <= w[1].start));
    }

    #[test]
    fn agent_skip_hint_holds_only_without_independent_signals() {
        let a = demo();
        let webhooks = file(&a, "src/payments/webhooks.ts");
        assert_eq!(webhooks.tier, Tier::Skip);
        assert!(webhooks.signals.iter().any(|s| s.kind == SignalKind::AgentHint));
        let lock = file(&a, "package-lock.json");
        assert!(lock.score <= 5.0);
    }

    #[test]
    fn collapses_ranges() {
        assert_eq!(collapse_ranges(&[9, 4, 5, 6, 4]), vec![(4, 6), (9, 9)]);
    }

    #[test]
    fn tiers() {
        assert_eq!(
            [Tier::for_score(0.0), Tier::for_score(12.0), Tier::for_score(40.0)],
            [Tier::Skip, Tier::Skim, Tier::Focus]
        );
    }
}
