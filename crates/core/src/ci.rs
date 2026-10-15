//! Who runs a pull request's security review: the `reviewhub init` workflow, or
//! the Claude Code session that opened the pull request.

use serde::Deserialize;

/// Name of the security review job in `.github/workflows/reviewhub-security.yml`.
/// GitHub reports the job as a check run with this name on the head commit.
pub const SECURITY_JOB: &str = "ReviewHub security review";

/// The fields of a GitHub check run this needs.
#[derive(Debug, Clone, Deserialize)]
pub struct CheckRun {
    pub name: String,
    /// `queued`, `in_progress` or `completed` (and a few rarer pending states).
    pub status: String,
    /// Set once completed: `success`, `skipped`, `failure`, `cancelled`, …
    #[serde(default)]
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityReviewer {
    /// The workflow is reviewing the pull request, or has published its report.
    Ci,
    /// The workflow won't review it (no credentials, failed, or not set up); the session should.
    Session,
    /// The workflow hasn't reported on the head commit yet; ask again shortly.
    Pending,
}

impl SecurityReviewer {
    pub fn explain(self) -> &'static str {
        match self {
            SecurityReviewer::Ci => {
                "ci: the ReviewHub security review workflow reviews this pull request; don't run a review yourself."
            }
            SecurityReviewer::Session => {
                "session: no workflow reviews this pull request (it isn't set up, has no Anthropic credential, or \
                 failed). Run the security review in this session with the security-reviewer subagent."
            }
            SecurityReviewer::Pending => "pending: the security review workflow hasn't started on the head commit yet.",
        }
    }
}

/// Decides from the head commit's check runs. A job that is queued, running or
/// succeeded means CI has it; skipped (no credential), failed or cancelled means
/// the session should review; no such job yet is pending.
pub fn security_reviewer(check_runs: &[CheckRun]) -> SecurityReviewer {
    let ours: Vec<&CheckRun> = check_runs.iter().filter(|r| r.name == SECURITY_JOB).collect();
    if ours.is_empty() {
        return SecurityReviewer::Pending;
    }
    let ci = ours.iter().any(|r| r.status != "completed" || r.conclusion.as_deref() == Some("success"));
    if ci { SecurityReviewer::Ci } else { SecurityReviewer::Session }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(name: &str, status: &str, conclusion: Option<&str>) -> CheckRun {
        CheckRun { name: name.into(), status: status.into(), conclusion: conclusion.map(String::from) }
    }

    #[test]
    fn decides_from_the_security_job_only() {
        use SecurityReviewer::*;
        assert_eq!(security_reviewer(&[]), Pending);
        assert_eq!(security_reviewer(&[run("rust", "completed", Some("success"))]), Pending);
        assert_eq!(security_reviewer(&[run(SECURITY_JOB, "queued", None)]), Ci);
        assert_eq!(security_reviewer(&[run(SECURITY_JOB, "in_progress", None)]), Ci);
        assert_eq!(security_reviewer(&[run(SECURITY_JOB, "completed", Some("success"))]), Ci);
        assert_eq!(security_reviewer(&[run(SECURITY_JOB, "completed", Some("skipped"))]), Session);
        assert_eq!(security_reviewer(&[run(SECURITY_JOB, "completed", Some("failure"))]), Session);
        // A re-run in progress wins over an earlier failed attempt.
        assert_eq!(
            security_reviewer(&[
                run(SECURITY_JOB, "completed", Some("failure")),
                run(SECURITY_JOB, "in_progress", None)
            ]),
            Ci
        );
    }
}
