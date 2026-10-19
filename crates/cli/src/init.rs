//! `reviewhub init`: installs ReviewHub into a repository, for Claude Code
//! sessions that can't install plugins (cloud sessions, including ones started
//! from the mobile app). Writes the plugin's skills and subagent under
//! `.claude/`, a launcher pinned to this CLI's version, hooks in
//! `.claude/settings.json`, an opt-in note in `CLAUDE.md`, and the briefing
//! security review and metrics workflows with a coverage script for the
//! detected test stacks; optionally a git pre-push hook. The plugin's files are compiled in, so there is one source.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::stacks::{self, Stack};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Names change from the plugin's namespaced ones (`reviewhub:review`) to
/// prefixed ones, since repository skills have no namespace.
const RENAMES: &[(&str, &str)] = &[
    ("reviewhub:review-manifest", "reviewhub-manifest"),
    ("reviewhub:security-review", "reviewhub-security-review"),
    ("`review-manifest`", "`reviewhub-manifest`"),
    ("`security-reviewer`", "`reviewhub-security-reviewer`"),
    ("`security-review`", "`reviewhub-security-review`"),
    ("by the security-review skill", "by the reviewhub-security-review skill"),
    ("\nname: review-manifest\n", "\nname: reviewhub-manifest\n"),
    ("\nname: security-reviewer\n", "\nname: reviewhub-security-reviewer\n"),
    ("\nname: security-review\n", "\nname: reviewhub-security-review\n"),
    ("\nname: review\n", "\nname: reviewhub-review\n"),
];

fn rename(text: &str) -> String {
    RENAMES.iter().fold(text.to_string(), |text, (from, to)| text.replace(from, to))
}

const MARKER_START: &str = "<!-- reviewhub:start -->";
const MARKER_END: &str = "<!-- reviewhub:end -->";
const HOOK_DIR: &str = ".claude/reviewhub/";

struct Generated {
    path: &'static str,
    content: String,
    executable: bool,
    /// Written once, then the repository's to edit: never overwritten.
    keep_existing: bool,
}

const COVERAGE_SCRIPT: &str = ".github/reviewhub-coverage.sh";

/// What `reviewhub init` sets up besides the skills, subagent, launcher and Claude Code hooks.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub workflow: bool,
    pub pre_push: bool,
}

const PRE_PUSH: &str = ".claude/reviewhub/pre-push.sh";
const GIT_PRE_PUSH: &str = ".claude/reviewhub/git-pre-push";
/// First line after the shebang of the hook ReviewHub installs in `.git/hooks`.
const GIT_HOOK_MARKER: &str = "# reviewhub:";

fn generated(options: Options, stacks: &[Stack]) -> Vec<Generated> {
    let file = |path, content: &str, executable| Generated {
        path,
        content: content.to_string(),
        executable,
        keep_existing: false,
    };
    let mut files = vec![
        file(
            ".claude/skills/reviewhub-review/SKILL.md",
            &rename(include_str!("../../../plugin/skills/review/SKILL.md")),
            false,
        ),
        file(
            ".claude/skills/reviewhub-manifest/SKILL.md",
            &rename(include_str!("../../../plugin/skills/review-manifest/SKILL.md")),
            false,
        ),
        file(
            ".claude/skills/reviewhub-security-review/SKILL.md",
            &rename(include_str!("../../../plugin/skills/security-review/SKILL.md")),
            false,
        ),
        file(
            ".claude/agents/reviewhub-security-reviewer.md",
            &rename(include_str!("../../../plugin/agents/security-reviewer.md")),
            false,
        ),
        file(".claude/reviewhub/bin/reviewhub", include_str!("../../../plugin/bin/reviewhub"), true),
        file(".claude/reviewhub/bin/VERSION", &format!("{VERSION}\n"), false),
        file(".claude/reviewhub/session-start.sh", include_str!("../templates/session-start.sh"), true),
        file(".claude/reviewhub/pr-created.sh", &rename(include_str!("../../../plugin/scripts/pr-created.sh")), true),
    ];
    if options.pre_push {
        files.push(file(PRE_PUSH, include_str!("../templates/pre-push.sh"), true));
        files.push(file(GIT_PRE_PUSH, include_str!("../templates/git-pre-push"), true));
    }
    if options.workflow {
        files.push(file(
            ".github/workflows/reviewhub.yml",
            &include_str!("../templates/reviewhub.yml").replace("{{VERSION}}", VERSION),
            false,
        ));
        files.push(file(
            ".github/workflows/reviewhub-security.yml",
            &include_str!("../templates/reviewhub-security.yml").replace("{{VERSION}}", VERSION),
            false,
        ));
        files.push(file(
            ".github/workflows/reviewhub-metrics.yml",
            &include_str!("../templates/reviewhub-metrics.yml")
                .replace("{{VERSION}}", VERSION)
                .replace("{{SETUP}}", &stacks::setup_steps(stacks)),
            false,
        ));
        files.push(Generated {
            path: COVERAGE_SCRIPT,
            content: include_str!("../templates/reviewhub-coverage.sh")
                .replace("{{COMMANDS}}", stacks::coverage_commands(stacks).trim_matches('\n')),
            executable: true,
            keep_existing: true,
        });
    }
    files
}

fn claude_md_block() -> String {
    format!(
        "{MARKER_START}\n## ReviewHub\n\n\
         Pull requests in this repository are reviewed with ReviewHub. Publish a review manifest with every pull \
         request you open or update (reviewhub-manifest skill). The `reviewhub` CLI (v{VERSION}) is on PATH in \
         Claude Code sessions; elsewhere run `.claude/reviewhub/bin/reviewhub`. To update these files, run \
         `reviewhub init` with a newer CLI.\n{MARKER_END}\n"
    )
}

/// Adds or replaces the ReviewHub section of CLAUDE.md, leaving the rest alone.
fn update_claude_md(existing: &str) -> String {
    let block = claude_md_block();
    if let (Some(start), Some(end)) = (existing.find(MARKER_START), existing.find(MARKER_END))
        && start < end
    {
        let after = &existing[end + MARKER_END.len()..];
        return format!("{}{}{}", &existing[..start], block.trim_end_matches('\n'), after);
    }
    if existing.trim().is_empty() {
        return block;
    }
    format!("{}\n\n{block}", existing.trim_end())
}

fn hook(command: &str, timeout: u32) -> Value {
    json!({ "type": "command", "command": command, "timeout": timeout })
}

/// Adds ReviewHub's hooks to `.claude/settings.json`, replacing earlier ones and keeping everything else.
fn update_settings(existing: &str) -> Result<String> {
    let mut settings: Value = if existing.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(existing).context(".claude/settings.json is not valid JSON")?
    };
    let Some(root) = settings.as_object_mut() else { bail!(".claude/settings.json must contain a JSON object") };
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let Some(hooks) = hooks.as_object_mut() else { bail!("`hooks` in .claude/settings.json must be an object") };

    let ours = [
        ("SessionStart", json!({ "hooks": [hook("\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/session-start.sh", 120)] })),
        (
            "PostToolUse",
            json!({
                "matcher": "Bash|mcp__github__create_pull_request",
                "hooks": [hook("\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/pr-created.sh", 10)]
            }),
        ),
    ];
    for (event, entry) in ours {
        let list = hooks.entry(event).or_insert_with(|| json!([]));
        let Some(list) = list.as_array_mut() else {
            bail!("`hooks.{event}` in .claude/settings.json must be an array")
        };
        list.retain(|group| {
            !group["hooks"]
                .as_array()
                .is_some_and(|hs| hs.iter().any(|h| h["command"].as_str().is_some_and(|c| c.contains(HOOK_DIR))))
        });
        list.push(entry);
    }
    Ok(format!("{}\n", serde_json::to_string_pretty(&settings)?))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    Created,
    Updated,
    Unchanged,
}

pub struct Outcome {
    pub changes: Vec<(String, Change)>,
}

impl Outcome {
    pub fn changed(&self) -> bool {
        self.changes.iter().any(|(_, c)| *c != Change::Unchanged)
    }
}

/// Whether the repository at `root` uses ReviewHub's pre-push hook.
pub fn pre_push_installed(root: &Path) -> bool {
    root.join(PRE_PUSH).is_file()
}

/// Whether the repository in the current directory uses ReviewHub's pre-push hook.
pub fn pre_push_installed_here() -> bool {
    let out = std::process::Command::new("git").args(["rev-parse", "--show-toplevel"]).output();
    out.ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| pre_push_installed(Path::new(String::from_utf8_lossy(&o.stdout).trim())))
}

/// Installs (`enable`) or removes ReviewHub's hook in the clone's git hooks directory. Leaves other pre-push hooks
/// alone. Returns a note for the user.
pub fn install_git_hook(root: &Path, enable: bool) -> Result<Option<String>> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--path-format=absolute", "--git-path", "hooks"])
        .output()
        .context("running git")?;
    if !out.status.success() {
        return Ok(enable.then(|| "Not a git repository, so the pre-push hook wasn't installed.".to_string()));
    }
    let hooks = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let hook = hooks.join("pre-push");
    let current = fs::read_to_string(&hook).ok();
    let ours = current.as_deref().is_some_and(|c| c.lines().nth(1).is_some_and(|l| l.starts_with(GIT_HOOK_MARKER)));
    if !enable {
        if ours {
            fs::remove_file(&hook)?;
            return Ok(Some(format!("removed  {}", hook.display())));
        }
        return Ok(None);
    }
    let content = include_str!("../templates/git-pre-push");
    if current.as_deref() == Some(content) {
        return Ok(None);
    }
    if current.is_some() && !ours {
        return Ok(Some(format!(
            "{} already exists, so ReviewHub's pre-push hook wasn't installed. Add this line to it:\n  \
             \"$(git rev-parse --show-toplevel)/{PRE_PUSH}\" \"$@\"",
            hook.display()
        )));
    }
    fs::create_dir_all(&hooks)?;
    fs::write(&hook, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
    }
    Ok(Some(format!(
        "{:>10}  {} (git hook, not committed; Claude Code sessions install it too)",
        "installed",
        hook.display()
    )))
}

/// Installs (or with `check`, only compares) ReviewHub's files in `root`.
pub fn run(root: &Path, options: Options, check: bool) -> Result<Outcome> {
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let stacks = stacks::detect(root);
    let mut planned: Vec<(PathBuf, String, bool, bool)> = generated(options, &stacks)
        .into_iter()
        .map(|g| (root.join(g.path), g.content, g.executable, g.keep_existing))
        .collect();
    // Turning the pre-push hook off removes its files.
    if !options.pre_push && !check {
        for path in [PRE_PUSH, GIT_PRE_PUSH] {
            let _ = fs::remove_file(root.join(path));
        }
    }

    let read = |p: &Path| fs::read_to_string(p).unwrap_or_default();
    let settings_path = root.join(".claude/settings.json");
    planned.push((settings_path.clone(), update_settings(&read(&settings_path))?, false, false));
    let claude_md = root.join("CLAUDE.md");
    planned.push((claude_md.clone(), update_claude_md(&read(&claude_md)), false, false));

    let mut changes = Vec::new();
    for (path, content, executable, keep_existing) in planned {
        let relative = path.strip_prefix(root).unwrap_or(&path).display().to_string();
        let change = match fs::read_to_string(&path) {
            Ok(current) if current == content || keep_existing => Change::Unchanged,
            Ok(_) => Change::Updated,
            Err(_) => Change::Created,
        };
        if !check && change != Change::Unchanged {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
            }
            fs::write(&path, &content).with_context(|| format!("writing {}", path.display()))?;
            #[cfg(unix)]
            if executable {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
        #[cfg(not(unix))]
        let _ = executable;
        changes.push((relative, change));
    }
    Ok(Outcome { changes })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: Options = Options { workflow: true, pre_push: true };

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("reviewhub-init-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn installs_and_is_idempotent() {
        let dir = tempdir("idempotent");
        let first = run(&dir, ALL, false).unwrap();
        assert!(first.changes.iter().all(|(_, c)| *c == Change::Created));
        assert!(!run(&dir, ALL, true).unwrap().changed(), "a second run changes nothing");
        fs::write(dir.join(COVERAGE_SCRIPT), "#!/bin/sh\nmake coverage\n").unwrap();
        assert!(!run(&dir, ALL, false).unwrap().changed(), "the coverage script is the repository's once written");
        assert_eq!(fs::read_to_string(dir.join(COVERAGE_SCRIPT)).unwrap(), "#!/bin/sh\nmake coverage\n");
        assert_eq!(fs::read_to_string(dir.join(".claude/reviewhub/bin/VERSION")).unwrap().trim(), VERSION);
        for name in ["reviewhub.yml", "reviewhub-security.yml"] {
            let workflow = fs::read_to_string(dir.join(".github/workflows").join(name)).unwrap();
            assert!(
                workflow.contains(&format!("REVIEWHUB_VERSION: \"{VERSION}\"")) && !workflow.contains("{{VERSION}}")
            );
        }
        // The security review runs Claude Code in the pull request's checkout: none of the pull request's CLAUDE.md
        // files, project settings, hooks or agents may load, and the reviewer runs no git.
        let security = fs::read_to_string(dir.join(".github/workflows/reviewhub-security.yml")).unwrap();
        for guard in [
            "--setting-sources user",
            "--strict-mcp-config",
            "REVIEWHUB_PLAN_DIR: ${{ runner.temp }}/reviewhub-review/batches",
            "--agents '${{ steps.prompt.outputs.agents }}'",
            "rm -rf .claude/agents",
        ] {
            assert!(security.contains(guard), "{guard}");
        }
        assert!(!security.contains("Bash(git"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join(".claude/reviewhub/bin/reviewhub")).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn installs_the_git_hook_without_clobbering_others() {
        let dir = tempdir("git-hook");
        assert!(std::process::Command::new("git").args(["init", "-q"]).current_dir(&dir).status().unwrap().success());
        let hook = dir.join(".git/hooks/pre-push");

        assert!(install_git_hook(&dir, true).unwrap().unwrap().contains("installed"));
        assert_eq!(fs::read_to_string(&hook).unwrap(), include_str!("../templates/git-pre-push"));
        assert_eq!(install_git_hook(&dir, true).unwrap(), None, "already installed");
        assert!(install_git_hook(&dir, false).unwrap().unwrap().contains("removed"));
        assert!(!hook.exists());

        fs::write(&hook, "#!/bin/sh\nnpm test\n").unwrap();
        assert!(install_git_hook(&dir, true).unwrap().unwrap().contains("already exists"));
        assert_eq!(install_git_hook(&dir, false).unwrap(), None);
        assert_eq!(fs::read_to_string(&hook).unwrap(), "#!/bin/sh\nnpm test\n", "someone else's hook stays");

        let off = run(&dir, Options { workflow: false, pre_push: false }, false).unwrap();
        assert!(!pre_push_installed(&dir) && off.changes.iter().all(|(p, _)| !p.contains("pre-push")));
        run(&dir, ALL, false).unwrap();
        assert!(pre_push_installed(&dir) && dir.join(GIT_PRE_PUSH).is_file());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn renames_every_reference_to_plugin_names() {
        for g in generated(ALL, &[]) {
            for stale in [
                "reviewhub:review",
                "reviewhub:security",
                "`review-manifest`",
                "`security-review`",
                "`security-reviewer`",
                "\nname: review\n",
            ] {
                assert!(!g.content.contains(stale), "{} still mentions {stale:?}", g.path);
            }
        }
        let agent = generated(Options { workflow: false, pre_push: false }, &[])
            .into_iter()
            .find(|g| g.path.ends_with("reviewhub-security-reviewer.md"))
            .unwrap();
        assert!(agent.content.contains("\nname: reviewhub-security-reviewer\n"));
    }

    #[test]
    fn merges_settings_and_replaces_its_own_hooks() {
        let existing = r#"{ "model": "opus", "hooks": { "SessionStart": [
            { "hooks": [{ "type": "command", "command": "npm ci" }] },
            { "hooks": [{ "type": "command", "command": "\"$CLAUDE_PROJECT_DIR\"/.claude/reviewhub/old.sh" }] }
        ] } }"#;
        let merged: Value = serde_json::from_str(&update_settings(existing).unwrap()).unwrap();
        assert_eq!(merged["model"], "opus");
        let start = merged["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(start.len(), 2, "keeps the project's hook, replaces the old ReviewHub one");
        assert_eq!(start[0]["hooks"][0]["command"], "npm ci");
        assert!(start[1]["hooks"][0]["command"].as_str().unwrap().ends_with("session-start.sh"));
        assert_eq!(merged["hooks"]["PostToolUse"].as_array().unwrap().len(), 1);
        assert!(update_settings("[1]").is_err());
    }

    #[test]
    fn keeps_the_rest_of_claude_md() {
        let first = update_claude_md("# Project\n\nOur rules.\n");
        assert!(first.starts_with("# Project\n\nOur rules.\n\n<!-- reviewhub:start -->"));
        let again = update_claude_md(&format!("{first}\n## Later section\n"));
        assert_eq!(again.matches(MARKER_START).count(), 1);
        assert!(again.ends_with("## Later section\n"));
        assert_eq!(update_claude_md(""), claude_md_block());
    }
}
