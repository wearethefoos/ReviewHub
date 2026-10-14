//! `reviewhub`: review agent-authored pull requests from the terminal, and
//! produce and publish the reports ReviewHub reads.

mod github;
mod render;

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use regex::Regex;
use serde_json::{Value, json};

use reviewhub_core::focus::{FocusInput, Tier, analyze};
use reviewhub_core::lcov;
use reviewhub_core::model::{Coverage, MetricsReport, PullRequestBundle, ReportVersion};
use reviewhub_core::reports::{self, Kind, Report};

use crate::github::GitHub;

fn parse_kind(s: &str) -> Result<Kind, String> {
    Kind::parse(s).ok_or_else(|| format!("expected one of: {}", Kind::ALL.map(Kind::as_str).join(", ")))
}

#[derive(Parser)]
#[command(name = "reviewhub", version, about, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show a pull request's review briefing: what to read, what needs a decision
    #[command(after_help = "Uses GITHUB_TOKEN, or the token from `gh auth login`.")]
    Review {
        /// owner/repo#123 or a pull request URL
        #[arg(required_unless_present = "bundle")]
        pr: Option<String>,
        /// Read a saved bundle (from `--json` or fixtures/demo/bundle.json) instead of GitHub
        #[arg(long, value_name = "FILE", conflicts_with = "pr")]
        bundle: Option<PathBuf>,
        /// Print the bundle and focus analysis as JSON (without diffs; see --patches)
        #[arg(long)]
        json: bool,
        /// With --json, include every file's patch and parsed hunks
        #[arg(long, requires = "json")]
        patches: bool,
        /// Also list skipped files, passed checks, prompts and every rollout step
        #[arg(long)]
        all: bool,
        /// Print the diffs of Focus files (with --all, also Skim files), annotated
        #[arg(long)]
        diff: bool,
    },
    /// Check a report against its schema
    Validate {
        #[arg(value_parser = parse_kind, value_name = "manifest|security|metrics")]
        kind: Kind,
        /// Report file, or - for stdin
        file: PathBuf,
    },
    /// Print the JSON Schema of a report
    Schema {
        #[arg(value_parser = parse_kind, value_name = "manifest|security|metrics")]
        kind: Kind,
    },
    /// Build a metrics report from an lcov coverage file
    Metrics {
        #[arg(long, value_name = "FILE")]
        lcov: PathBuf,
        /// Strip this prefix from absolute paths [default: current directory]
        #[arg(long, value_name = "DIR")]
        root: Option<PathBuf>,
        /// Line coverage of the base branch, in percent
        #[arg(long, value_name = "PCT")]
        base_lines: Option<f64>,
        /// Head commit the coverage was measured on
        #[arg(long, value_name = "SHA")]
        commit: Option<String>,
    },
    /// Print the pull request comment body for a report
    Comment {
        #[arg(value_parser = parse_kind, value_name = "manifest|security|metrics")]
        kind: Kind,
        file: PathBuf,
    },
    /// Create or update a report comment on a pull request
    #[command(after_help = "Needs GITHUB_TOKEN, or a login from `gh auth login`.")]
    Publish {
        #[arg(value_parser = parse_kind, value_name = "manifest|security|metrics")]
        kind: Kind,
        file: PathBuf,
        /// owner/repo
        #[arg(long, env = "GITHUB_REPOSITORY")]
        repo: String,
        /// Pull request number
        #[arg(long)]
        pr: u32,
    },
}

static PR_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:https?://[^/]+/)?([\w.-]+)/([\w.-]+)/pulls?/(\d+)").unwrap());
static PR_SHORT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([\w.-]+)/([\w.-]+)#(\d+)$").unwrap());

/// Parses "https://github.com/o/r/pull/1", "o/r#1" or "o/r/pull/1".
fn parse_pr_ref(input: &str) -> Option<(String, String, u32)> {
    let text = input.trim();
    let caps = PR_URL.captures(text).or_else(|| PR_SHORT.captures(text))?;
    Some((caps[1].to_string(), caps[2].to_string(), caps[3].parse().ok()?))
}

fn read_report(kind: Kind, file: &Path) -> Result<Report> {
    let text = if file == Path::new("-") {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?
    };
    let json: Value = serde_json::from_str(&text).with_context(|| format!("{} is not valid JSON", file.display()))?;
    reports::validate(kind, &json)
        .map_err(|e| anyhow!("{} is not a valid {} report:\n{e}", file.display(), kind.as_str()))
}

/// Diffs dominate the JSON's size; agents read the summary and ask for `--diff` when they need code.
fn strip_patches(out: &mut Value) {
    for file in out["bundle"]["files"].as_array_mut().into_iter().flatten() {
        file.as_object_mut().map(|f| f.remove("patch"));
    }
    for file in out["focus"]["files"].as_array_mut().into_iter().flatten() {
        if let Some(f) = file.as_object_mut() {
            f.remove("hunks");
            f.get_mut("file").and_then(Value::as_object_mut).map(|f| f.remove("patch"));
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::Review { pr, bundle, json, patches, all, diff } => {
            let bundle: PullRequestBundle = match (bundle, pr) {
                (Some(path), _) => {
                    let text =
                        std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
                    let value: Value = serde_json::from_str(&text)?;
                    // Accept both a bare bundle and the output of `review --json`.
                    serde_json::from_value(value.get("bundle").cloned().unwrap_or(value))
                        .with_context(|| format!("{} is not a pull request bundle", path.display()))?
                }
                (None, Some(pr)) => {
                    let (owner, repo, number) = parse_pr_ref(&pr)
                        .ok_or_else(|| anyhow!("can't read {pr:?}: use owner/repo#123 or a pull request URL"))?;
                    GitHub::new(github::resolve_token()).fetch_bundle(&owner, &repo, number)?
                }
                (None, None) => unreachable!("clap requires one of them"),
            };
            let analysis = analyze(&FocusInput {
                files: &bundle.files,
                manifest: bundle.manifest(),
                security: bundle.security(),
                metrics: bundle.metrics(),
            });
            if json {
                let mut out = json!({ "bundle": bundle, "focus": analysis });
                if !patches {
                    strip_patches(&mut out);
                }
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                anstream::print!("{}", render::briefing(&bundle, &analysis, &render::Options { all }));
                if diff {
                    let tiers: &[Tier] = if all { &[Tier::Focus, Tier::Skim] } else { &[Tier::Focus] };
                    anstream::print!("{}", render::diffs(&analysis, tiers));
                }
            }
        }
        Cmd::Validate { kind, file } => {
            read_report(kind, &file)?;
            println!("{} is a valid {} report", file.display(), kind.as_str());
        }
        Cmd::Schema { kind } => println!("{}", serde_json::to_string_pretty(&kind.schema())?),
        Cmd::Metrics { lcov, root, base_lines, commit } => {
            let text = std::fs::read_to_string(&lcov).with_context(|| format!("cannot read {}", lcov.display()))?;
            let root = match root {
                Some(r) => r,
                None => std::env::current_dir()?,
            };
            let (files, lines) = lcov::parse_lcov(&text, &root.to_string_lossy());
            let report = MetricsReport {
                version: ReportVersion::V1,
                commit,
                coverage: Some(Coverage { tool: Some("lcov".into()), lines: Some(lines), base_lines, files }),
                quality: None,
            };
            // Round-trip through validation so the output is guaranteed to be accepted.
            let report = reports::validate(Kind::Metrics, &serde_json::to_value(&report)?).map_err(|e| anyhow!(e))?;
            println!("{}", serde_json::to_string_pretty(&report.to_value())?);
        }
        Cmd::Comment { kind, file } => println!("{}", reports::render_comment(kind, &read_report(kind, &file)?)),
        Cmd::Publish { kind, file, repo, pr } => {
            let report = read_report(kind, &file)?;
            let Some((owner, name)) = repo.split_once('/') else { bail!("--repo must be owner/repo") };
            let url = GitHub::new(github::resolve_token()).publish(kind, &report, owner, name, pr)?;
            println!("{url}");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            anstream::eprintln!("{}error:{} {err:#}", anstyle::AnsiColor::Red.on_default().bold(), anstyle::Reset);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pr_refs() {
        let expected = Some(("o".to_string(), "r.js".to_string(), 12));
        assert_eq!(parse_pr_ref("https://github.com/o/r.js/pull/12/files"), expected);
        assert_eq!(parse_pr_ref("o/r.js#12"), expected);
        assert_eq!(parse_pr_ref("o/r.js/pulls/12"), expected);
        assert_eq!(parse_pr_ref("nonsense"), None);
    }

    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
