//! Report formats and the pull request bundle.
//!
//! These types are the source of truth for ReviewHub's report formats: the
//! JSON Schemas in `schemas/` are generated from them (`reviewhub schema`),
//! and so are the extension's TypeScript types (`--features ts`). Doc comments
//! become schema descriptions, so write them for the agents that read them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Inclusive `[start, end]` line range in the new version of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LineRange(#[schemars(range(min = 1))] pub u32, #[schemars(range(min = 1))] pub u32);

impl LineRange {
    pub fn start(self) -> u32 {
        self.0
    }
    pub fn end(self) -> u32 {
        self.1
    }
}

/// Report format version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReportVersion {
    #[default]
    #[serde(rename = "1")]
    V1,
}

/// A file, optionally narrowed to a line range, that an item is about.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct FileRef {
    /// Repository-relative file path.
    #[schemars(length(min = 1))]
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
}

// ---------------------------------------------------------------- manifest

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum DeviationSeverity {
    #[default]
    Minor,
    Major,
}

/// How costly it is to get a decision wrong or change it later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Impact {
    #[default]
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum ValidationResult {
    Passed,
    Failed,
    Partial,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum ValidationKind {
    UnitTest,
    IntegrationTest,
    E2eTest,
    Typecheck,
    Lint,
    Build,
    Manual,
    Benchmark,
    Other,
}

/// Who should do something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    #[default]
    Human,
    Agent,
    Ci,
    Ops,
}

impl Owner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Ci => "ci",
            Self::Ops => "ops",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum RolloutWhen {
    #[default]
    BeforeMerge,
    AfterMerge,
    AfterDeploy,
}

impl RolloutWhen {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeMerge => "before merge",
            Self::AfterMerge => "after merge",
            Self::AfterDeploy => "after deploy",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

/// The agent's own suggestion. ReviewHub treats "skip" as a claim, not a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "kebab-case")]
pub enum ReviewHint {
    MustReview,
    Skim,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    #[schemars(length(min = 1))]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(url)]
    pub session_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// The user's request, rephrased for reviewers: its goal, scope and constraints, in the
    /// user's terms where possible. Not a verbatim copy: prompts can hold private data, and the
    /// manifest is public on the pull request. Leave out credentials, personal data, customer
    /// details, internal hostnames and anything else the user wouldn't post on the pull request.
    #[schemars(length(min = 1))]
    pub original: String,
    /// Later user messages that changed the scope, summarized the same way. Leave out messages
    /// that only steered the session ("open the PR", "looks good").
    #[serde(default)]
    pub follow_ups: Vec<String>,
    /// Link to the issue or conversation the prompt came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(url)]
    pub source: Option<String>,
}

/// A place where the delivery differs from what was asked.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Deviation {
    /// Stable identifier, unique within its list.
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub title: String,
    /// What the prompt asked for.
    pub requested: String,
    /// What was delivered instead.
    pub delivered: String,
    /// Why the agent deviated.
    pub reason: String,
    #[serde(default)]
    pub severity: DeviationSeverity,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Alternative {
    #[schemars(length(min = 1))]
    pub title: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tradeoffs: Option<String>,
    /// A ready-to-use prompt that asks an agent to switch to this alternative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

/// A question the prompt left open, and how the agent answered it.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Decision {
    /// Stable identifier, unique within its list.
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub title: String,
    /// The question that needed answering and why the prompt did not settle it.
    pub context: String,
    /// What the agent decided.
    pub chosen: String,
    pub rationale: String,
    #[serde(default)]
    pub impact: Impact,
    #[serde(default)]
    pub alternatives: Vec<Alternative>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

/// A check the agent actually ran.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Validation {
    /// Stable identifier, unique within its list.
    #[schemars(length(min = 1))]
    pub id: String,
    pub kind: ValidationKind,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub result: ValidationResult,
    /// Short excerpt of output proving the result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

/// Something the agent could not validate.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Gap {
    /// Stable identifier, unique within its list.
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub title: String,
    /// Why the agent could not validate this.
    pub why: String,
    /// Concrete steps for whoever closes the gap.
    pub how_to_validate: String,
    #[serde(default)]
    pub owner: Owner,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    #[serde(default)]
    pub files: Vec<FileRef>,
}

/// Something that has to happen outside the diff for the change to ship.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct RolloutStep {
    /// Stable identifier, unique within its list.
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub owner: Owner,
    #[serde(default)]
    pub when: RolloutWhen,
    #[serde(default)]
    pub blocking: bool,
    /// Prompt to hand this step to an agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FileNote {
    #[schemars(length(min = 1))]
    pub path: String,
    pub role: FileRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_hint: Option<ReviewHint>,
}

/// Written by the agent that authored a pull request: the context a reviewer
/// needs but cannot get from the diff.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct ReviewManifest {
    pub version: ReportVersion,
    pub agent: Agent,
    pub prompt: Prompt,
    /// What was delivered, in a few sentences.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Scanner {
    #[schemars(length(min = 1))]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Suggestion {
    pub explanation: String,
    /// Unified diff that fixes the finding, applicable with `git apply`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    /// Prompt that asks an agent to implement the fix or refactor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct SecurityFinding {
    #[schemars(length(min = 1))]
    pub id: String,
    #[schemars(length(min = 1))]
    pub title: String,
    pub severity: Severity,
    /// For example a CWE id such as "CWE-89".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub description: String,
    #[schemars(length(min = 1))]
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<LineRange>,
    #[serde(default)]
    pub confidence: Confidence,
    pub suggestion: Suggestion,
}

/// Written by a separate security agent that did not author the change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct SecurityReport {
    pub version: ReportVersion,
    pub scanner: Scanner,
    /// Head commit SHA the scan ran against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub findings: Vec<SecurityFinding>,
}

// ---------------------------------------------------------------- metrics

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FileCoverage {
    #[schemars(length(min = 1))]
    pub path: String,
    /// Line coverage of the whole file, in percent.
    #[schemars(range(min = 0, max = 100))]
    pub lines: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 100))]
    pub branches: Option<f64>,
    /// Executable lines with zero hits in the new version of the file.
    #[serde(default)]
    pub uncovered_lines: Vec<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Overall line coverage, in percent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 100))]
    pub lines: Option<f64>,
    /// Overall line coverage on the base branch, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 100))]
    pub base_lines: Option<f64>,
    #[serde(default)]
    pub files: Vec<FileCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct FileQuality {
    #[schemars(length(min = 1))]
    pub path: String,
    /// Cyclomatic complexity of the file after the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0))]
    pub complexity: Option<f64>,
    /// Change in complexity introduced by this pull request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity_delta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0, max = 100))]
    pub duplication: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lint_issues: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Quality {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default)]
    pub files: Vec<FileQuality>,
}

/// Coverage and code quality numbers, usually produced in CI.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct MetricsReport {
    pub version: ReportVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<Coverage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<Quality>,
}

// ---------------------------------------------------------------- pull request

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub login: String,
    #[serde(default)]
    pub avatar_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "lowercase")]
pub enum PrState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestInfo {
    pub owner: String,
    pub repo: String,
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub body: String,
    pub state: PrState,
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
    pub additions: u32,
    pub deletions: u32,
    pub changed_files: u32,
    pub commits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_path: Option<String>,
    /// GitHub's file status: added, modified, removed, renamed, copied, changed or unchanged.
    pub status: String,
    pub additions: u32,
    pub deletions: u32,
    /// Unified diff hunks for this file. Missing for binary or very large files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

/// Where a report was found.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct ReportEnvelope<T> {
    pub source: ReportSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Set when a report was found but did not validate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Reports {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<ReportEnvelope<ReviewManifest>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security: Option<ReportEnvelope<SecurityReport>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<ReportEnvelope<MetricsReport>>,
}

/// A pull request with its changed files and the ReviewHub reports found for it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
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
