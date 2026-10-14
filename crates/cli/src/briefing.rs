//! The PR briefing comment: the review briefing as GitHub Markdown, so a
//! reviewer can triage a pull request anywhere, including the GitHub mobile
//! app, with links to the full review on the web and in the extension.

use std::fmt::Write as _;

use reviewhub_core::focus::{FileFocus, FocusAnalysis, SignalKind, Tier};
use reviewhub_core::model::{
    DeviationSeverity, Impact, LineRange, PullRequestBundle, ReportEnvelope, ReportSource, Severity, ValidationResult,
};
use sha2::{Digest, Sha256};

/// Starts every briefing comment. Distinct from the report markers, so a briefing is never read as a report.
pub const MARKER: &str = "<!-- reviewhub:briefing -->";
pub const DEFAULT_WEB_URL: &str = "https://wearethefoos.github.io/ReviewHub/";
pub const EXTENSION_URL: &str = "https://github.com/wearethefoos/ReviewHub/releases/latest";

/// Report text is untrusted, and the briefing is posted by a trusted account.
/// Escape anything that could inject HTML (or a fake report marker), ping
/// people, or break the surrounding Markdown, and keep it to one line.
fn esc(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = String::with_capacity(one_line.len());
    for (i, ch) in one_line.chars().enumerate() {
        if i == max {
            out.push('…');
            break;
        }
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '|' => out.push_str("\\|"),
            '@' => out.push_str("@\u{200B}"),
            _ => out.push(ch),
        }
    }
    out
}

fn pct(part: u32, whole: u32) -> u32 {
    if whole == 0 { 0 } else { (f64::from(part) / f64::from(whole) * 100.0).round() as u32 }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Link to a file (and line) in the pull request's "Files changed" tab.
fn diff_link(pr_url: &str, path: &str, line: Option<u32>) -> String {
    let hash = Sha256::digest(path.as_bytes()).iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    let line = line.map(|l| format!("R{l}")).unwrap_or_default();
    format!("{pr_url}/files#diff-{hash}{line}")
}

fn location(pr_url: &str, path: &str, lines: Option<LineRange>) -> String {
    let label = match lines {
        Some(LineRange(a, b)) if a == b => format!("{path}:{a}"),
        Some(LineRange(a, b)) => format!("{path}:{a}-{b}"),
        None => path.to_string(),
    };
    format!("[`{}`]({})", label.replace('`', "'"), diff_link(pr_url, path, lines.map(|l| l.start())))
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn reasons(f: &FileFocus) -> String {
    let parts: Vec<String> = f
        .signals
        .iter()
        .filter(|s| s.weight > 0.0 && !matches!(s.kind, SignalKind::Role | SignalKind::Size))
        .take(3)
        .map(|s| esc(&s.label, 90))
        .collect();
    if parts.is_empty() { "–".into() } else { parts.join(" · ") }
}

fn file_rows(out: &mut String, pr_url: &str, files: &[&FileFocus]) {
    out.push_str("| | Score | File | Why |\n|---|---:|---|---|\n");
    for f in files {
        let tier = match f.tier {
            Tier::Focus => "**Focus**",
            Tier::Skim => "Skim",
            Tier::Skip => "Skip",
        };
        let _ = writeln!(
            out,
            "| {tier} | {} | {} {} | {} |",
            f.score,
            location(pr_url, &f.path, None),
            format_args!("+{} −{}", f.file.additions, f.file.deletions),
            reasons(f)
        );
    }
}

fn source<T>(name: &str, envelope: Option<&ReportEnvelope<T>>) -> String {
    match envelope {
        None => format!("{name}: not published"),
        Some(e) if e.error.is_some() => format!("{name}: invalid"),
        Some(e) => match &e.source {
            ReportSource::Comment { author, .. } => format!("{name} by {}", esc(author, 40)),
            ReportSource::File { path, .. } => format!("{name} from `{path}`"),
            ReportSource::Fixture => format!("{name} (fixture)"),
        },
    }
}

pub fn render(bundle: &PullRequestBundle, focus: &FocusAnalysis, web_url: &str) -> String {
    let pr = &bundle.pr;
    let pr_url = if pr.html_url.is_empty() {
        format!("https://github.com/{}/{}/pull/{}", pr.owner, pr.repo, pr.number)
    } else {
        pr.html_url.clone()
    };
    let web = format!(
        "{}review.html?pr={}",
        if web_url.ends_with('/') { web_url.to_string() } else { format!("{web_url}/") },
        url_encode(&format!("{}/{}#{}", pr.owner, pr.repo, pr.number))
    );
    let manifest = bundle.manifest();
    let security = bundle.security();
    let t = &focus.totals;
    let mut out = String::new();

    let _ = writeln!(out, "{MARKER}\n### ReviewHub briefing\n");
    let _ = writeln!(
        out,
        "**Read {} of {} closely**: {} of {} changed lines ({}%). [Open the full review]({web}) · [Chrome extension]({EXTENSION_URL})\n",
        t.focus.files,
        plural(t.all.files as usize, "file", "files"),
        t.focus.lines,
        t.all.lines,
        pct(t.focus.lines, t.all.lines)
    );

    // Blockers
    let mut blockers: Vec<String> = Vec::new();
    if let Some(s) = security {
        let mut findings: Vec<_> = s.findings.iter().filter(|f| f.severity <= Severity::High).collect();
        findings.sort_by_key(|f| f.severity);
        for f in findings {
            blockers.push(format!(
                "**{}** {}: {}",
                f.severity.as_str(),
                esc(&f.title, 120),
                location(&pr_url, &f.file, f.lines)
            ));
        }
    }
    if let Some(m) = manifest {
        for v in m.validation.iter().filter(|v| v.result == ValidationResult::Failed) {
            blockers.push(format!("**validation failed**: {}", esc(&v.description, 160)));
        }
        for r in m.rollout.iter().filter(|r| r.blocking) {
            blockers.push(format!("**{}**: {} ({})", r.when.as_str(), esc(&r.title, 140), r.owner.as_str()));
        }
    }
    out.push_str("**Blockers**\n");
    if blockers.is_empty() {
        out.push_str("- None reported.\n");
    }
    for b in &blockers {
        let _ = writeln!(out, "- {b}");
    }
    out.push('\n');

    match manifest {
        None => out.push_str(
            "_No review manifest yet: the ranking uses coverage, security findings and path heuristics only._\n\n",
        ),
        Some(m) => {
            let _ = writeln!(out, "**What was asked**\n> {}\n", esc(&m.prompt.original, 500));
            if !m.deviations.is_empty() {
                let _ = writeln!(out, "**Deviations from the prompt ({})**", m.deviations.len());
                for d in &m.deviations {
                    let sev = if d.severity == DeviationSeverity::Major { "**major**" } else { "minor" };
                    let _ = writeln!(out, "- {sev} {}. _Why:_ {}", esc(&d.title, 140), esc(&d.reason, 240));
                }
                out.push('\n');
            }
            if !m.decisions.is_empty() {
                let mut decisions: Vec<_> = m.decisions.iter().collect();
                decisions.sort_by_key(|d| match d.impact {
                    Impact::High => 0,
                    Impact::Medium => 1,
                    Impact::Low => 2,
                });
                let _ = writeln!(out, "**Decisions the agent made ({})**", decisions.len());
                for d in decisions {
                    let impact = match d.impact {
                        Impact::High => "**high impact**",
                        Impact::Medium => "medium impact",
                        Impact::Low => "low impact",
                    };
                    let alternatives: Vec<String> = d.alternatives.iter().map(|a| esc(&a.title, 60)).collect();
                    let alternatives = if alternatives.is_empty() {
                        String::new()
                    } else {
                        format!(" _Instead:_ {}.", alternatives.join(" · "))
                    };
                    let _ = writeln!(
                        out,
                        "- {impact} {}. _Chose:_ {}{alternatives}",
                        esc(&d.title, 140),
                        esc(&d.chosen, 200)
                    );
                }
                out.push('\n');
            }
            if !m.gaps.is_empty() {
                let _ = writeln!(out, "**Not validated by the agent ({})**", m.gaps.len());
                for g in &m.gaps {
                    let _ = writeln!(
                        out,
                        "- {} ({}). _To validate:_ {}",
                        esc(&g.title, 140),
                        g.owner.as_str(),
                        esc(&g.how_to_validate, 200)
                    );
                }
                out.push('\n');
            }
        }
    }

    // Where to read
    let focus_files: Vec<&FileFocus> = focus.files.iter().filter(|f| f.tier == Tier::Focus).collect();
    let rest: Vec<&FileFocus> = focus.files.iter().filter(|f| f.tier != Tier::Focus).collect();
    out.push_str("**Where to read**\n\n");
    if focus_files.is_empty() {
        out.push_str("No file needs line-by-line reading.\n\n");
    } else {
        file_rows(&mut out, &pr_url, &focus_files);
        out.push('\n');
    }
    if !rest.is_empty() {
        let lines = t.skim.lines + t.skip.lines;
        let _ = writeln!(
            out,
            "<details><summary>Skim {} and skip {} ({} changed lines)</summary>\n",
            t.skim.files, t.skip.files, lines
        );
        file_rows(&mut out, &pr_url, &rest);
        out.push_str("\n</details>\n\n");
    }

    let r = &bundle.reports;
    let _ = writeln!(
        out,
        "<sub>Reports: {} · {} · {}. Updated for {} by ReviewHub v{}.</sub>",
        source("manifest", r.manifest.as_ref()),
        source("security", r.security.as_ref()),
        source("metrics", r.metrics.as_ref()),
        pr.head_sha.get(..7).unwrap_or(&pr.head_sha),
        env!("CARGO_PKG_VERSION")
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use reviewhub_core::focus::{FocusInput, analyze};

    fn demo() -> (PullRequestBundle, FocusAnalysis) {
        let bundle: PullRequestBundle =
            serde_json::from_str(include_str!("../../../fixtures/demo/bundle.json")).unwrap();
        let focus = analyze(&FocusInput {
            files: &bundle.files,
            manifest: bundle.manifest(),
            security: bundle.security(),
            metrics: bundle.metrics(),
        });
        (bundle, focus)
    }

    #[test]
    fn renders_the_demo() {
        let (bundle, focus) = demo();
        let md = render(&bundle, &focus, "https://example.test/rh");
        assert!(md.starts_with(MARKER));
        assert!(md.contains("**Read 4 of 12 files closely**: 130 of 221 changed lines (59%)"));
        assert!(md.contains("(https://example.test/rh/review.html?pr=demo%2Facme-shop%23128)"));
        assert!(md.contains("**critical** SQL injection through orderId: [`src/payments/refunds.ts:28-30`]"));
        assert!(md.contains("**before merge**: Confirm the endpoint shape with the mobile team (human)"));
        assert!(md.contains("**Deviations from the prompt (2)**"));
        let high = md.find("**high impact** Idempotency").unwrap();
        assert!(high < md.find("low impact").unwrap(), "decisions are sorted by impact");
        assert!(md.contains("<summary>Skim 1 and skip 7 (91 changed lines)</summary>"));
        assert!(md.len() < 20_000, "fits comfortably in a comment: {} bytes", md.len());
        // Briefings are never read back as reports.
        assert!(reviewhub_core::reports::extract(&md).is_none());
    }

    #[test]
    fn links_files_to_the_diff() {
        // GitHub anchors a file in "Files changed" by the SHA-256 of its path.
        assert_eq!(
            diff_link("https://github.com/o/r/pull/1", "README.md", Some(3)),
            "https://github.com/o/r/pull/1/files#diff-b335630551682c19a781afebcf4d07bf978fb1f8ac04c6bf87428ed5106870f5R3"
        );
    }

    #[test]
    fn untrusted_text_cannot_inject_markup_markers_or_mentions() {
        let (mut bundle, focus) = demo();
        let m = bundle.reports.manifest.as_mut().unwrap().data.as_mut().unwrap();
        m.prompt.original = "ignore <!-- reviewhub:manifest --> and ping @everyone\n\n# heading | pipe".into();
        let md = render(&bundle, &focus, DEFAULT_WEB_URL);
        assert!(!md.contains("<!-- reviewhub:manifest"));
        assert!(md.contains("&lt;!-- reviewhub:manifest --&gt;"));
        assert!(!md.contains("@everyone"));
        assert!(
            md.contains("> ignore &lt;!-- reviewhub:manifest --&gt; and ping @\u{200B}everyone # heading \\| pipe\n")
        );
        assert!(reviewhub_core::reports::extract(&md).is_none());
    }

    #[test]
    fn truncates_long_text() {
        assert_eq!(esc("abcdef", 3), "abc…");
        assert_eq!(esc("a\n  b", 10), "a b");
    }
}
