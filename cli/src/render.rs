//! Terminal rendering of a review briefing. Mirrors the web app's Briefing tab,
//! condensed: what was asked, where to look, and what needs a decision.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::LazyLock;

use anstyle::{AnsiColor, Style};
use regex::Regex;

use crate::diff::LineType;
use crate::focus::{Annotation, AnnotationKind, FocusAnalysis, SignalKind, Tier};
use crate::model::{
    DeviationSeverity, Impact, PullRequestBundle, ReportEnvelope, ReportSource, Severity, ValidationResult,
};

const BOLD: Style = Style::new().bold();
const DIM: Style = Style::new().dimmed();
const fn fg(c: AnsiColor) -> Style {
    Style::new().fg_color(Some(anstyle::Color::Ansi(c)))
}
const RED: Style = fg(AnsiColor::Red);
const GREEN: Style = fg(AnsiColor::Green);
const YELLOW: Style = fg(AnsiColor::Yellow);
const MAGENTA: Style = fg(AnsiColor::Magenta);
const CYAN: Style = fg(AnsiColor::Cyan);
const BLUE: Style = fg(AnsiColor::Blue);

fn paint(style: Style, text: impl std::fmt::Display) -> String {
    format!("{style}{text}{style:#}")
}

static INLINE_CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

/// Inline markdown code spans in cyan; everything else stays as written.
fn md(text: &str) -> String {
    INLINE_CODE.replace_all(text, |c: &regex::Captures| paint(CYAN, &c[1])).into_owned()
}

fn width() -> usize {
    textwrap::termwidth().clamp(60, 110)
}

/// Wraps `text` with every line indented by `indent` spaces.
fn wrap(text: &str, indent: usize) -> String {
    labeled("", text, indent)
}

/// Wraps `label text` with a hanging indent, so continuation lines align after the label.
fn labeled(label: &str, text: &str, indent: usize) -> String {
    let first = format!("{}{label}", " ".repeat(indent));
    let rest = " ".repeat(indent + textwrap::core::display_width(label));
    let options = textwrap::Options::new(width())
        .initial_indent(&first)
        .subsequent_indent(&rest)
        .word_splitter(textwrap::WordSplitter::NoHyphenation);
    text.lines().map(|line| textwrap::fill(&md(line), &options)).collect::<Vec<_>>().join("\n")
}

fn heading(out: &mut String, title: &str, count: Option<usize>) {
    let count = count.map(|n| paint(DIM, format!(" ({n})"))).unwrap_or_default();
    let _ = writeln!(out, "\n{}{count}", paint(BOLD, title.to_uppercase()));
}

fn severity_style(s: Severity) -> Style {
    match s {
        Severity::Critical => RED.bold(),
        Severity::High => RED,
        Severity::Medium => YELLOW,
        Severity::Low => DIM,
        Severity::Info => BLUE,
    }
}

fn tier_style(t: Tier) -> Style {
    match t {
        Tier::Focus => RED.bold(),
        Tier::Skim => YELLOW,
        Tier::Skip => DIM,
    }
}

fn tier_label(t: Tier) -> &'static str {
    match t {
        Tier::Focus => "FOCUS",
        Tier::Skim => "SKIM",
        Tier::Skip => "SKIP",
    }
}

fn pct(part: u64, whole: u64) -> u64 {
    if whole == 0 { 0 } else { (part as f64 / whole as f64 * 100.0).round() as u64 }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { format!("1 {word}") } else { format!("{n} {word}s") }
}

pub struct Options {
    /// List skipped files and include alternatives, evidence and prompts.
    pub all: bool,
}

pub fn briefing(bundle: &PullRequestBundle, focus: &FocusAnalysis, opts: &Options) -> String {
    let mut out = String::new();
    let pr = &bundle.pr;
    let manifest = bundle.manifest();
    let security = bundle.security();
    let metrics = bundle.metrics();

    // Header
    let state = match (pr.draft, pr.state.as_str()) {
        (true, _) => paint(DIM, "draft"),
        (_, "merged") => paint(MAGENTA, "merged"),
        (_, "closed") => paint(RED, "closed"),
        _ => paint(GREEN, "open"),
    };
    let _ =
        writeln!(out, "{} {}", paint(BOLD, &pr.title), paint(DIM, format!("{}/{}#{}", pr.owner, pr.repo, pr.number)));
    let _ = writeln!(
        out,
        "{state} · {} wants to merge {} into {} from {} · {} {}",
        pr.author.login,
        plural(pr.commits as usize, "commit"),
        paint(CYAN, &pr.base_ref),
        paint(CYAN, &pr.head_ref),
        paint(GREEN, format!("+{}", pr.additions)),
        paint(RED, format!("−{}", pr.deletions)),
    );
    if !pr.html_url.is_empty() {
        let _ = writeln!(out, "{}", paint(DIM, &pr.html_url));
    }

    // The ask
    match manifest {
        Some(m) => {
            heading(&mut out, "What was asked", None);
            let _ = writeln!(out, "{}", wrap(&m.prompt.original, 2));
            for (i, f) in m.prompt.follow_ups.iter().enumerate() {
                let _ = writeln!(out, "{}\n{}", paint(DIM, format!("  Follow-up {}:", i + 1)), wrap(f, 2));
            }
            heading(&mut out, &format!("What {} delivered", m.agent.name), None);
            let _ = writeln!(out, "{}", wrap(&m.summary, 2));
        }
        None => {
            heading(&mut out, "No review manifest", None);
            let _ = writeln!(
                out,
                "{}",
                wrap(
                    "Files are ranked from coverage, security findings and path heuristics only. Ask the authoring agent to publish a manifest (see agents/author.md).",
                    2
                )
            );
        }
    }

    // Where to look
    let t = &focus.totals;
    heading(&mut out, "Where to spend your review", None);
    let _ = writeln!(
        out,
        "  Read {} of {} files closely: {} of {} changed lines ({}%).",
        paint(BOLD, t.focus.files),
        t.all.files,
        paint(BOLD, t.focus.lines),
        t.all.lines,
        pct(t.focus.lines, t.all.lines)
    );
    let path_width = focus.files.iter().map(|f| f.path.len()).max().unwrap_or(0).min(56);
    for tier in Tier::ALL {
        let files: Vec<_> = focus.files.iter().filter(|f| f.tier == tier).collect();
        if files.is_empty() {
            continue;
        }
        if tier == Tier::Skip && !opts.all {
            let lines = t.get(Tier::Skip).lines;
            let _ = writeln!(
                out,
                "  {}  {} files, {} lines: generated, mechanical or low risk {}",
                paint(tier_style(tier), format!("{:<5}", tier_label(tier))),
                files.len(),
                lines,
                paint(DIM, "(--all to list)")
            );
            continue;
        }
        for f in files {
            let _ = writeln!(
                out,
                "  {}  {:>3}  {:<path_width$}  {} {}",
                paint(tier_style(tier), format!("{:<5}", tier_label(tier))),
                f.score,
                f.path,
                paint(GREEN, format!("+{}", f.file.additions)),
                paint(RED, format!("−{}", f.file.deletions)),
            );
            let reasons: Vec<&str> = f
                .signals
                .iter()
                .filter(|s| s.weight > 0.0 && !matches!(s.kind, SignalKind::Role | SignalKind::Size))
                .take(3)
                .map(|s| s.label.as_str())
                .collect();
            if !reasons.is_empty() {
                let _ = writeln!(out, "{}", paint(DIM, wrap(&reasons.join(" · "), 14)));
            }
        }
    }
    if !focus.dangling_refs.is_empty() {
        let _ = writeln!(
            out,
            "  {} reports mention files outside this diff: {}",
            paint(YELLOW, "!"),
            focus.dangling_refs.join(", ")
        );
    }

    // Security
    match security {
        Some(report) => {
            let mut findings: Vec<_> = report.findings.iter().collect();
            findings.sort_by_key(|f| f.severity);
            heading(&mut out, "Security", Some(findings.len()));
            let counts: Vec<String> = Severity::ALL
                .iter()
                .filter_map(|s| {
                    let n = findings.iter().filter(|f| f.severity == *s).count();
                    (n > 0).then(|| paint(severity_style(*s), format!("{n} {}", s.as_str())))
                })
                .collect();
            if counts.is_empty() {
                let _ = writeln!(out, "  {}", paint(GREEN, "No findings."));
            } else {
                let _ = writeln!(out, "  {}", counts.join(" · "));
            }
            if report.commit.as_ref().is_some_and(|c| *c != pr.head_sha) {
                let _ = writeln!(out, "  {}", paint(YELLOW, "Scanned an older commit than the pull request's head."));
            }
            for f in findings {
                let place = match f.lines {
                    Some((a, b)) if a == b => format!("{}:{a}", f.file),
                    Some((a, b)) => format!("{}:{a}-{b}", f.file),
                    None => f.file.clone(),
                };
                let _ = writeln!(
                    out,
                    "  {}  {}  {}",
                    paint(severity_style(f.severity), format!("{:<8}", f.severity.as_str())),
                    md(&f.title),
                    paint(DIM, place)
                );
                let _ = writeln!(out, "{}", paint(DIM, labeled("Fix: ", &f.suggestion.explanation, 12)));
                if opts.all
                    && let Some(prompt) = &f.suggestion.prompt
                {
                    let _ = writeln!(out, "{}", labeled("Prompt: ", prompt, 12));
                }
            }
        }
        None => {
            heading(&mut out, "Security", None);
            let _ = writeln!(out, "  {}", paint(DIM, "No security report yet (see agents/security-reviewer.md)."));
        }
    }

    let Some(m) = manifest else {
        reports_footer(&mut out, bundle);
        return out;
    };

    // Deviations
    heading(&mut out, "Deviations from the prompt", Some(m.deviations.len()));
    for d in &m.deviations {
        let sev = match d.severity {
            DeviationSeverity::Major => paint(YELLOW.bold(), "major"),
            DeviationSeverity::Minor => paint(DIM, "minor"),
        };
        let _ = writeln!(out, "  {sev}  {}", md(&d.title));
        let _ = writeln!(out, "{}", labeled("Asked: ", &d.requested, 9));
        let _ = writeln!(out, "{}", labeled("Got:   ", &d.delivered, 9));
        let _ = writeln!(out, "{}", paint(DIM, labeled("Why:   ", &d.reason, 9)));
    }

    // Decisions
    let mut decisions: Vec<_> = m.decisions.iter().collect();
    decisions.sort_by_key(|d| match d.impact {
        Impact::High => 0,
        Impact::Medium => 1,
        Impact::Low => 2,
    });
    heading(&mut out, "Decisions the agent made", Some(decisions.len()));
    for d in decisions {
        let impact = match d.impact {
            Impact::High => paint(MAGENTA.bold(), "high  "),
            Impact::Medium => paint(MAGENTA, "medium"),
            Impact::Low => paint(DIM, "low   "),
        };
        let _ = writeln!(out, "  {impact}  {}", md(&d.title));
        let _ = writeln!(out, "{}", labeled("Chose: ", &d.chosen, 10));
        let alts: Vec<&str> = d.alternatives.iter().map(|a| a.title.as_str()).collect();
        if !alts.is_empty() {
            let _ = writeln!(out, "{}", paint(DIM, labeled("Instead: ", &alts.join(" · "), 10)));
        }
        if opts.all {
            for alt in d.alternatives.iter().filter(|a| a.prompt.is_some()) {
                let _ = writeln!(
                    out,
                    "{}",
                    labeled(&format!("Prompt for {}: ", alt.title), alt.prompt.as_deref().unwrap_or_default(), 10)
                );
            }
        }
    }

    // Validation
    let passed = m.validation.iter().filter(|v| v.result == ValidationResult::Passed).count();
    heading(&mut out, "Validation", None);
    if m.validation.is_empty() {
        let _ = writeln!(out, "  {}", paint(YELLOW, "The agent did not report any validation."));
    } else {
        let _ = writeln!(out, "  {passed} of {} checks passed.", m.validation.len());
    }
    for v in &m.validation {
        let (mark, show) = match v.result {
            ValidationResult::Passed => (paint(GREEN, "✓"), opts.all),
            ValidationResult::Failed => (paint(RED, "✗"), true),
            ValidationResult::Partial => (paint(YELLOW, "~"), true),
            ValidationResult::Skipped => (paint(DIM, "-"), true),
        };
        if show {
            let _ = writeln!(out, "{}", labeled(&format!("{mark} "), &v.description, 2));
        }
    }
    if let Some(c) = metrics.and_then(|m| m.coverage.as_ref()).and_then(|c| c.lines.map(|l| (l, c.base_lines))) {
        let delta = c.1.map(|base| {
            let d = c.0 - base;
            paint(
                if d < 0.0 { RED } else { GREEN },
                format!(" ({}{d:.1} vs {})", if d >= 0.0 { "+" } else { "" }, pr.base_ref),
            )
        });
        let _ = writeln!(out, "  Line coverage {:.1}%{}", c.0, delta.unwrap_or_default());
    }

    // Gaps
    heading(&mut out, "Not validated by the agent", Some(m.gaps.len()));
    for g in &m.gaps {
        let _ = writeln!(out, "  {}  {}", paint(YELLOW, format!("{:<5}", g.owner)), md(&g.title));
        let _ = writeln!(out, "{}", paint(DIM, labeled("To validate: ", &g.how_to_validate, 9)));
    }

    // Rollout
    let steps: Vec<_> = m.rollout.iter().filter(|r| opts.all || r.blocking).collect();
    heading(&mut out, if opts.all { "Rollout" } else { "Blocking before this ships" }, Some(steps.len()));
    for r in steps {
        let _ = writeln!(
            out,
            "  ☐ {}  {}",
            md(&r.title),
            paint(DIM, format!("{} · {}", r.when.replace('-', " "), r.owner))
        );
    }

    reports_footer(&mut out, bundle);
    out
}

fn reports_footer(out: &mut String, bundle: &PullRequestBundle) {
    fn line<T>(name: &str, env: Option<&ReportEnvelope<T>>) -> String {
        match env {
            None => format!("{} {name}: not published", paint(DIM, "○")),
            Some(e) => {
                let source = match &e.source {
                    ReportSource::Comment { author, .. } => format!("comment by {author}"),
                    ReportSource::File { path, .. } => path.clone(),
                    ReportSource::Fixture => "fixture".into(),
                };
                match &e.error {
                    Some(err) => {
                        format!("{} {name}: invalid ({source})\n{}", paint(RED, "✗"), paint(DIM, wrap(err, 4)))
                    }
                    None => format!("{} {name}: {source}", paint(GREEN, "✓")),
                }
            }
        }
    }
    let r = &bundle.reports;
    let _ = writeln!(out, "\n{}", paint(BOLD, "REPORTS"));
    let _ = writeln!(out, "  {}", line("manifest", r.manifest.as_ref()));
    let _ = writeln!(out, "  {}", line("security", r.security.as_ref()));
    let _ = writeln!(out, "  {}", line("metrics", r.metrics.as_ref()));
}

fn annotation_style(kind: AnnotationKind) -> (Style, &'static str) {
    match kind {
        AnnotationKind::Security => (RED, "security"),
        AnnotationKind::Deviation => (YELLOW, "deviation"),
        AnnotationKind::Decision => (MAGENTA, "decision"),
        AnnotationKind::Gap => (YELLOW, "not validated"),
        AnnotationKind::Uncovered => (DIM, "no test coverage"),
    }
}

/// Diffs of the files in `tiers`, with the briefing's annotations placed after
/// the last visible line of their range (the same placement as the web app).
pub fn diffs(focus: &FocusAnalysis, tiers: &[Tier]) -> String {
    let mut out = String::new();
    for f in focus.files.iter().filter(|f| tiers.contains(&f.tier)) {
        let _ = writeln!(
            out,
            "\n{} {}  {}",
            paint(tier_style(f.tier), format!("{} {}", tier_label(f.tier), f.score)),
            paint(BOLD, &f.path),
            paint(DIM, f.role.as_str())
        );
        if f.hunks.is_empty() {
            let _ = writeln!(out, "  {}", paint(DIM, "No diff available."));
            continue;
        }

        let visible: HashSet<u32> = f.hunks.iter().flat_map(|h| &h.lines).filter_map(|l| l.new_line).collect();
        let mut placed: HashMap<u32, Vec<&Annotation>> = HashMap::new();
        for a in &f.annotations {
            match (a.start..=a.end).rev().find(|n| visible.contains(n)) {
                Some(at) => placed.entry(at).or_default().push(a),
                None => {
                    let (style, label) = annotation_style(a.kind);
                    let _ = writeln!(out, "  {} {} {}", paint(style, "▌"), paint(style, label), md(&a.title));
                }
            }
        }

        for hunk in &f.hunks {
            let _ = writeln!(out, "{}", paint(CYAN, &hunk.header));
            for line in &hunk.lines {
                let marked = line.new_line.and_then(|n| {
                    let covering: Vec<_> = f.annotations.iter().filter(|a| n >= a.start && n <= a.end).collect();
                    covering.iter().min_by_key(|a| a.kind as u8).map(|a| annotation_style(a.kind).0)
                });
                let gutter = marked.map(|s| paint(s, "▌")).unwrap_or_else(|| " ".into());
                let num = |n: Option<u32>| n.map(|n| format!("{n:>4}")).unwrap_or_else(|| "    ".into());
                let (sign, style) = match line.kind {
                    LineType::Add => ("+", GREEN),
                    LineType::Del => ("-", RED),
                    LineType::Ctx => (" ", Style::new()),
                };
                let _ = writeln!(
                    out,
                    "{} {}{}",
                    paint(DIM, format!("{} {}", num(line.old_line), num(line.new_line))),
                    gutter,
                    paint(style, format!("{sign}{}", line.text))
                );
                if line.kind != LineType::Del
                    && let Some(here) = line.new_line.and_then(|n| placed.get(&n))
                {
                    for a in here {
                        let (style, label) = annotation_style(a.kind);
                        let severity = a.severity.map(|s| format!(" {}", s.as_str())).unwrap_or_default();
                        let _ =
                            writeln!(out, "{}", paint(style, format!("            ┌ {label}{severity}: {}", a.title)));
                        if let Some(detail) = &a.detail {
                            let options = textwrap::Options::new(width())
                                .initial_indent("            │ ")
                                .subsequent_indent("            │ ")
                                .word_splitter(textwrap::WordSplitter::NoHyphenation);
                            let _ = writeln!(out, "{}", paint(DIM, textwrap::fill(&md(detail), &options)));
                        }
                    }
                }
            }
        }
    }
    out
}
