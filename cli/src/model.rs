//! Report and pull request types. These mirror the Zod schemas in
//! `src/lib/schema` (defaults included) and the web app's `PullRequestBundle`.
//! Validation happens against `schemas/*.schema.json` before deserializing.

use serde::{Deserialize, Serialize};

pub type LineRange = (u32, u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
}

// ---------------------------------------------------------------- manifest

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DeviationSeverity {
    #[default]
    Minor,
    Major,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ValidationResult {
    Passed,
    Failed,
    Partial,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Core,
    Supporting,
    Test,
    Config,
    Docs,
    Generated,
    Mechanical,
}

impl FileRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Supporting => "supporting",
            Self::Test => "test",
            Self::Config => "config",
            Self::Docs => "docs",
            Self::Generated => "generated",
            Self::Mechanical => "mechanical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewHint {
    MustReview,
    Skim,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub name: String,
    pub model: Option<String>,
    pub session_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub original: String,
    #[serde(default)]
    pub follow_ups: Vec<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deviation {
    pub id: String,
    pub title: String,
    pub requested: String,
    pub delivered: String,
    pub reason: String,
    #[serde(default)]
    pub severity: DeviationSeverity,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alternative {
    pub title: String,
    pub description: String,
    pub tradeoffs: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub id: String,
    pub title: String,
    pub context: String,
    pub chosen: String,
    pub rationale: String,
    #[serde(default)]
    pub impact: Impact,
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Validation {
    pub id: String,
    pub kind: String,
    pub description: String,
    pub command: Option<String>,
    pub result: ValidationResult,
    pub evidence: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

fn human() -> String {
    "human".into()
}

fn before_merge() -> String {
    "before-merge".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    pub id: String,
    pub title: String,
    pub why: String,
    pub how_to_validate: String,
    #[serde(default = "human")]
    pub owner: String,
    pub effort: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutStep {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "human")]
    pub owner: String,
    #[serde(default = "before_merge")]
    pub when: String,
    #[serde(default)]
    pub blocking: bool,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileNote {
    pub path: String,
    pub role: FileRole,
    pub note: Option<String>,
    pub review_hint: Option<ReviewHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewManifest {
    pub version: String,
    pub agent: Agent,
    pub prompt: Prompt,
    pub summary: String,
    #[serde(default)]
    pub deviations: Vec<Deviation>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub validation: Vec<Validation>,
    #[serde(default)]
    pub gaps: Vec<Gap>,
    #[serde(default)]
    pub rollout: Vec<RolloutStep>,
    #[serde(default)]
    pub files: Vec<FileNote>,
}

// ---------------------------------------------------------------- security

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl Severity {
    pub const ALL: [Severity; 5] = [Self::Critical, Self::High, Self::Medium, Self::Low, Self::Info];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scanner {
    pub name: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub explanation: String,
    pub diff: Option<String>,
    pub prompt: Option<String>,
}

fn medium() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub category: Option<String>,
    pub description: String,
    pub file: String,
    pub lines: Option<LineRange>,
    #[serde(default = "medium")]
    pub confidence: String,
    pub suggestion: Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityReport {
    pub version: String,
    pub scanner: Scanner,
    pub commit: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub findings: Vec<SecurityFinding>,
}

// ---------------------------------------------------------------- metrics

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
    pub path: String,
    pub lines: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branches: Option<f64>,
    #[serde(default)]
    pub uncovered_lines: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_lines: Option<f64>,
    #[serde(default)]
    pub files: Vec<FileCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileQuality {
    pub path: String,
    pub complexity: Option<f64>,
    pub complexity_delta: Option<f64>,
    pub duplication: Option<f64>,
    pub lint_issues: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quality {
    pub tool: Option<String>,
    #[serde(default)]
    pub files: Vec<FileQuality>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsReport {
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Coverage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

// ---------------------------------------------------------------- pull request

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestInfo {
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub state: String,
    #[serde(default)]
    pub draft: bool,
    pub author: Author,
    pub base_ref: String,
    pub head_ref: String,
    pub head_sha: String,
    #[serde(default)]
    pub html_url: String,
    #[serde(default)]
    pub created_at: String,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub commits: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ReportSource {
    #[serde(rename_all = "camelCase")]
    Comment {
        url: String,
        author: String,
        updated_at: String,
    },
    File {
        path: String,
        #[serde(rename = "ref")]
        git_ref: String,
    },
    Fixture,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportEnvelope<T> {
    pub source: ReportSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Reports {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ReportEnvelope<ReviewManifest>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<ReportEnvelope<SecurityReport>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<ReportEnvelope<MetricsReport>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestBundle {
    pub pr: PullRequestInfo,
    pub files: Vec<ChangedFile>,
    #[serde(default)]
    pub reports: Reports,
}

impl PullRequestBundle {
    pub fn manifest(&self) -> Option<&ReviewManifest> {
        self.reports.manifest.as_ref()?.data.as_ref()
    }
    pub fn security(&self) -> Option<&SecurityReport> {
        self.reports.security.as_ref()?.data.as_ref()
    }
    pub fn metrics(&self) -> Option<&MetricsReport> {
        self.reports.metrics.as_ref()?.data.as_ref()
    }
}
