//! `reviewhub`: review agent-authored pull requests from the terminal, and
//! produce and publish the reports ReviewHub reads.

mod briefing;
mod github;
mod init;
mod metrics;
mod plan;
mod push;
mod render;
mod stacks;
mod updates;

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use regex::Regex;
use serde_json::{Value, json};

use reviewhub_core::ci;
use reviewhub_core::focus::{FocusAnalysis, FocusInput, Tier, analyze};
use reviewhub_core::lcov;
use reviewhub_core::model::{Coverage, MetricsReport, PullRequestBundle, ReportVersion};
use reviewhub_core::reports::{self, Kind, Report};

use crate::github::GitHub;

fn parse_install(s: &str) -> Result<reviewhub_core::update::Install, String> {
    reviewhub_core::update::Install::parse(s)
        .ok_or_else(|| "expected one of: plugin, repo, cli, extension-unpacked".into())
}

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
    /// Write the review briefing as a GitHub comment, or publish it on the pull request
    #[command(
        after_help = "With --publish, uses GITHUB_TOKEN or your `gh auth login`; in GitHub Actions, the workflow token."
    )]
    Briefing {
        /// owner/repo#123 or a pull request URL
        #[arg(required_unless_present = "bundle")]
        pr: Option<String>,
        /// Read a saved bundle instead of GitHub
        #[arg(long, value_name = "FILE", conflicts_with_all = ["pr", "publish"])]
        bundle: Option<PathBuf>,
        /// Create or update the briefing comment on the pull request instead of printing it
        #[arg(long)]
        publish: bool,
        /// Where "Open the full review" points: the ReviewHub web app
        #[arg(long, env = "REVIEWHUB_WEB_URL", default_value = briefing::DEFAULT_WEB_URL)]
        web_url: String,
    },
    /// Set up a repository for ReviewHub in Claude Code, including cloud sessions that can't install plugins
    #[command(
        after_help = "Writes the ReviewHub skills and security subagent under .claude/, a launcher pinned to this CLI's \
version, hooks in .claude/settings.json, a note in CLAUDE.md, and the PR briefing, security review and metrics workflows (with a coverage script for the test stacks it detects). \
Commit the result. Run it again with a newer CLI to update."
    )]
    Init {
        /// Repository root [default: the git repository containing the current directory]
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
        /// Don't add the workflows (briefing, security review, metrics) or the coverage script
        #[arg(long)]
        no_workflow: bool,
        /// Install the git pre-push hook that gets pushes from agent sessions security-reviewed
        /// [default: ask on a terminal; kept once installed]
        #[arg(long, conflicts_with = "no_pre_push")]
        pre_push: bool,
        /// Don't install the git pre-push hook, and remove ReviewHub's
        #[arg(long)]
        no_pre_push: bool,
        /// Only report whether the files are up to date; exit with 1 if not
        #[arg(long)]
        check: bool,
    },
    /// Tell whether CI or this session should run a pull request's security review
    #[command(
        after_help = "Prints `ci` or `session` with an explanation. Looks for the \"ReviewHub security review\" job \
of the workflow `reviewhub init` adds, on the pull request's head commit, waiting up to --wait seconds for it to start. \
Uses GITHUB_TOKEN, or the token from `gh auth login`."
    )]
    SecurityStatus {
        /// owner/repo#123 or a pull request URL
        pr: String,
        /// Seconds to wait for the workflow to start before deciding `session`
        #[arg(long, default_value_t = 60)]
        wait: u64,
    },
    /// Run a headless Claude Code security review of a pull request and publish it
    #[command(
        after_help = "Skips the review when the pull request already has a security report for its head commit, or \
when the ReviewHub security review workflow reviews it (unless --force). Needs Claude Code (`claude`) and GITHUB_TOKEN or \
`gh auth login`. Run it from a checkout of the repository."
    )]
    SecurityReview {
        /// owner/repo#123 or a pull request URL
        pr: String,
        /// Wait until the pull request's head is this commit (after a push) before reviewing
        #[arg(long, value_name = "SHA")]
        sha: Option<String>,
        /// Review even if CI does or a report for the head commit exists
        #[arg(long)]
        force: bool,
    },
    /// Say whether a newer ReviewHub release exists and how to update this install
    #[command(
        after_help = "Checks GitHub at most once a day (the answer is cached). Prints nothing when up to date or \
offline. Set REVIEWHUB_NO_UPDATE_CHECK=1 to turn it off; DISABLE_UPDATES and CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC \
turn it off too."
    )]
    UpdateCheck {
        /// How ReviewHub was installed: plugin, repo (reviewhub init), cli or extension-unpacked
        #[arg(long = "for", value_name = "INSTALL", default_value = "cli", value_parser = parse_install)]
        install: reviewhub_core::update::Install,
        /// Print Claude Code SessionStart hook output (JSON) instead of text
        #[arg(long)]
        hook: bool,
    },
    /// The git pre-push hook installed by `reviewhub init` (reads git's pre-push input on stdin)
    #[command(hide = true)]
    PrePush { remote: Option<String>, url: Option<String> },
    /// Plan a pull request's security review: one pass, or batches of files for subagents, with a model for each
    #[command(
        after_help = "Prints JSON. A small pull request is reviewed in one pass; a large one is split into batches, \
each with complete instructions (`prompt`), a report path (`output`) and a model, for one subagent each. The plan uses \
only the paths and the diff, never the author's manifest. Models: REVIEWHUB_MODEL_STRONG (default opus), \
REVIEWHUB_MODEL_STANDARD (sonnet) and REVIEWHUB_MODEL_LIGHT (haiku), as Claude Code model aliases or ids. \
Uses GITHUB_TOKEN, or the token from `gh auth login`."
    )]
    /// Print the batch reviewer's definition for Claude Code's --agents
    SecurityAgents,
    SecurityPlan {
        /// owner/repo#123 or a pull request URL
        #[arg(required_unless_present = "bundle")]
        pr: Option<String>,
        /// Read a saved bundle instead of GitHub
        #[arg(long, value_name = "FILE", conflicts_with = "pr")]
        bundle: Option<PathBuf>,
        /// New directory for the batches' reports; it must not exist yet [default: a new private temporary directory]
        #[arg(long, value_name = "DIR")]
        out_dir: Option<PathBuf>,
    },
    /// Combine the batches' security reports into one, for the review across files
    #[command(
        after_help = "Checks the plan is still the one for the pull request's head and that each batch's report \
                      is by its batch's reviewer, on the head commit and about the batch's files; reads them from \
                      the plan's directory, drops duplicate findings (keeping the more severe), makes ids unique and \
                      prints the report. Batches without a usable report are listed on stderr and in the summary."
    )]
    SecurityMerge {
        /// owner/repo#123 or a pull request URL
        #[arg(required_unless_present = "bundle")]
        pr: Option<String>,
        /// Read a saved bundle instead of GitHub
        #[arg(long, value_name = "FILE", conflicts_with = "pr")]
        bundle: Option<PathBuf>,
        /// The plan.json `reviewhub security-plan` wrote
        #[arg(long, value_name = "FILE")]
        plan: PathBuf,
        /// The model of the reviewer combining them, for the report's scanner
        #[arg(long)]
        model: Option<String>,
        /// Write the report to this new file instead of printing it (an existing file is never replaced)
        #[arg(long, value_name = "FILE")]
        out: Option<PathBuf>,
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
    /// Build a metrics report: coverage from lcov files, complexity of the changed files
    #[command(
        after_help = "Run it from the repository root. Relative paths in an lcov file are resolved against the root, \
or else against the directories above the lcov file (for a project in a subdirectory)."
    )]
    Metrics {
        /// lcov coverage files (e.g. one per language)
        #[arg(long, value_name = "FILE", num_args = 1.., required_unless_present = "base")]
        lcov: Vec<PathBuf>,
        /// Measure the complexity of the files changed since this git ref (its merge base with HEAD)
        #[arg(long, value_name = "REF")]
        base: Option<String>,
        /// Repository root; stripped from absolute paths [default: current directory]
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

fn load_bundle(path: Option<PathBuf>, pr: Option<&str>) -> Result<PullRequestBundle> {
    match (path, pr) {
        (Some(path), _) => {
            let text = std::fs::read_to_string(&path).with_context(|| format!("cannot read {}", path.display()))?;
            let value: Value = serde_json::from_str(&text)?;
            // Accept both a bare bundle and the output of `review --json`.
            serde_json::from_value(value.get("bundle").cloned().unwrap_or(value))
                .with_context(|| format!("{} is not a pull request bundle", path.display()))
        }
        (None, Some(pr)) => {
            let (owner, repo, number) = parse_pr_ref(pr)
                .ok_or_else(|| anyhow!("can't read {pr:?}: use owner/repo#123 or a pull request URL"))?;
            GitHub::new(github::resolve_token()).fetch_bundle(&owner, &repo, number)
        }
        (None, None) => unreachable!("clap requires one of them"),
    }
}

fn analyze_bundle(bundle: &PullRequestBundle) -> FocusAnalysis {
    analyze(&FocusInput {
        files: &bundle.files,
        manifest: bundle.manifest(),
        security: bundle.security(),
        metrics: bundle.metrics(),
    })
}

fn git_root() -> Option<PathBuf> {
    let out = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).output().ok()?;
    out.status.success().then(|| PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()))
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::Briefing { pr, bundle, publish, web_url } => {
            let bundle = load_bundle(bundle, pr.as_deref())?;
            let markdown = briefing::render(&bundle, &analyze_bundle(&bundle), &web_url);
            if publish {
                let p = &bundle.pr;
                let github = GitHub::new(github::resolve_token());
                println!("{}", github.upsert_comment(&p.owner, &p.repo, p.number, briefing::MARKER, &markdown)?);
            } else {
                print!("{markdown}");
            }
        }
        Cmd::Review { pr, bundle, json, patches, all, diff } => {
            let bundle = load_bundle(bundle, pr.as_deref())?;
            let analysis = analyze_bundle(&bundle);
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
        Cmd::Init { dir, no_workflow, pre_push, no_pre_push, check } => {
            let root = match dir {
                Some(dir) => dir,
                None => git_root().unwrap_or(std::env::current_dir()?),
            };
            let installed = init::pre_push_installed(&root);
            let pre_push = !no_pre_push && (pre_push || installed || (!check && push::ask_install()));
            let outcome = init::run(&root, init::Options { workflow: !no_workflow, pre_push }, check)?;
            for (path, change) in &outcome.changes {
                let label = match change {
                    init::Change::Created if check => "missing",
                    init::Change::Updated if check => "outdated",
                    init::Change::Created => "created",
                    init::Change::Updated => "updated",
                    init::Change::Unchanged => "up to date",
                };
                println!("{label:>10}  {path}");
            }
            if check && outcome.changed() {
                bail!("ReviewHub files are not up to date with this CLI (v{}); run `reviewhub init`", init::VERSION);
            }
            if !check {
                match init::install_git_hook(&root, pre_push) {
                    Ok(Some(note)) => println!("{note}"),
                    Ok(None) => {}
                    Err(e) => eprintln!("warning: {e:#}"),
                }
            }
            if !check && outcome.changed() {
                println!("\nReviewHub v{} is set up in {}. Commit these files.", init::VERSION, root.display());
                if !pre_push {
                    println!(
                        "Tip: `reviewhub init --pre-push` adds a git hook that gets pushes from agent sessions security-reviewed."
                    );
                }
            }
        }
        Cmd::SecurityAgents => println!("{}", plan::batch_agents()),
        Cmd::SecurityPlan { pr, bundle, out_dir } => {
            let bundle = load_bundle(bundle, pr.as_deref())?;
            let models = plan::models(|name| std::env::var(name).ok());
            println!("{}", serde_json::to_string_pretty(&plan::plan_json(&bundle, &models, out_dir)?)?);
        }
        Cmd::SecurityMerge { pr, bundle, plan: plan_file, model, out } => {
            let bundle = load_bundle(bundle, pr.as_deref())?;
            let (report, problems) = plan::merge(&plan_file, &bundle, model)?;
            for problem in problems {
                eprintln!("No usable report from {problem}. Review its files yourself.");
            }
            let report = serde_json::to_string_pretty(&report.to_value())?;
            match out {
                Some(out) => plan::write(&out, &report)?,
                None => println!("{report}"),
            }
        }
        Cmd::Validate { kind, file } => {
            read_report(kind, &file)?;
            println!("{} is a valid {} report", file.display(), kind.as_str());
        }
        Cmd::Schema { kind } => println!("{}", serde_json::to_string_pretty(&kind.schema())?),
        Cmd::Metrics { lcov, base, root, base_lines, commit } => {
            let root = match root {
                Some(r) => r,
                None => std::env::current_dir()?,
            };
            let coverage = if lcov.is_empty() {
                None
            } else {
                let mut text = String::new();
                for file in &lcov {
                    let raw =
                        std::fs::read_to_string(file).with_context(|| format!("cannot read {}", file.display()))?;
                    text.push_str(&metrics::resolve_lcov_paths(&raw, &root, file));
                    text.push('\n');
                }
                let (files, lines) = lcov::parse_lcov(&text, &root.to_string_lossy());
                Some(Coverage { tool: Some("lcov".into()), lines: Some(lines), base_lines, files })
            };
            let quality = base.map(|base| metrics::changed_quality(&root, &base)).transpose()?;
            let report = MetricsReport { version: ReportVersion::V1, commit, coverage, quality };
            // Round-trip through validation so the output is guaranteed to be accepted.
            let report = reports::validate(Kind::Metrics, &serde_json::to_value(&report)?).map_err(|e| anyhow!(e))?;
            println!("{}", serde_json::to_string_pretty(&report.to_value())?);
        }
        Cmd::Comment { kind, file } => println!("{}", reports::render_comment(kind, &read_report(kind, &file)?)),
        Cmd::SecurityStatus { pr, wait } => {
            let (owner, name, number) = parse_pr_ref(&pr)
                .ok_or_else(|| anyhow!("expected owner/repo#123 or a pull request URL, got {pr:?}"))?;
            let gh = GitHub::new(github::resolve_token());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(wait);
            let decision = loop {
                let runs = gh.head_check_runs(&owner, &name, number, ci::SECURITY_JOB)?;
                match ci::security_reviewer(&runs) {
                    ci::SecurityReviewer::Pending if std::time::Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_secs(10))
                    }
                    ci::SecurityReviewer::Pending => break ci::SecurityReviewer::Session,
                    decided => break decided,
                }
            };
            println!("{}", decision.explain());
        }
        Cmd::Publish { kind, file, repo, pr } => {
            let report = read_report(kind, &file)?;
            let Some((owner, name)) = repo.split_once('/') else { bail!("--repo must be owner/repo") };
            let url = GitHub::new(github::resolve_token()).publish(kind, &report, owner, name, pr)?;
            println!("{url}");
            // Remember the agent's branch for the pre-push hook, and get the pull request security-reviewed.
            if kind == Kind::Manifest
                && push::save_manifest(&repo, pr, &report.to_value()).is_some()
                && init::pre_push_installed_here()
                && let Some(message) = push::request_review(&repo, pr, None)
            {
                println!("{message}");
            }
        }
        Cmd::UpdateCheck { install, hook } => updates::check(install, hook),
        Cmd::SecurityReview { pr, sha, force } => push::security_review(&pr, sha.as_deref(), force)?,
        Cmd::PrePush { url, .. } => {
            let mut input = String::new();
            let _ = std::io::stdin().read_to_string(&mut input);
            push::pre_push(url.as_deref().unwrap_or(""), &input);
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
