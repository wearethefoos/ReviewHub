//! The `reviewhub` commands end to end: reports on disk, the demo pull request,
//! and a stand-in GitHub serving it.

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{Value, json};
use support::{MockGitHub, run, temp_dir};

const DEMO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo/bundle.json");

fn demo() -> Value {
    serde_json::from_str(&std::fs::read_to_string(DEMO).unwrap()).unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The demo's reports, written to `dir` as `<kind>.json`.
fn write_reports(dir: &Path) -> Vec<(&'static str, PathBuf)> {
    write_reports_of(&demo(), dir)
}

fn write_reports_of(demo: &Value, dir: &Path) -> Vec<(&'static str, PathBuf)> {
    ["manifest", "security", "metrics"]
        .into_iter()
        .map(|kind| {
            let path = dir.join(format!("{kind}.json"));
            std::fs::write(&path, demo["reports"][kind]["data"].to_string()).unwrap();
            (kind, path)
        })
        .collect()
}

/// A stand-in GitHub serving the demo as `o/r#7`: the GitHub API's view of its pull request, files and report
/// comments (made with `reviewhub comment`), plus `extra` routes.
fn demo_github(extra: Vec<(&str, u16, String)>) -> MockGitHub {
    demo_github_at(demo()["pr"]["headSha"].as_str().unwrap(), extra)
}

/// [`demo_github`] with head commit `sha`. Its security report is for commit `reviewed`.
fn demo_github_at(sha: &str, extra: Vec<(&str, u16, String)>) -> MockGitHub {
    let mut demo = demo();
    demo["reports"]["security"]["data"]["commit"] = json!("reviewed");
    let pr = &demo["pr"];
    let pull = json!({
        "number": 7, "title": pr["title"], "body": pr["body"], "state": "open", "draft": false,
        "user": { "login": "agent-author", "avatar_url": "" },
        "base": { "ref": "main" },
        "head": { "ref": "feature", "sha": sha, "repo": { "name": "r", "owner": { "login": "o" } } },
        "html_url": "https://github.com/o/r/pull/7", "created_at": "2026-10-01T00:00:00Z",
        "additions": 1, "deletions": 0, "changed_files": demo["files"].as_array().unwrap().len(), "commits": 1,
    });
    let files: Vec<Value> = demo["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            json!({
                "filename": f["path"], "status": f["status"], "additions": f["additions"],
                "deletions": f["deletions"], "patch": f["patch"],
            })
        })
        .collect();
    let dir = temp_dir("demo-comments");
    let comments: Vec<Value> = write_reports_of(&demo, &dir)
        .into_iter()
        .enumerate()
        .map(|(i, (kind, path))| {
            let output = run(&mut MockGitHub::start(vec![]).reviewhub(&["comment", kind, path.to_str().unwrap()]));
            json!({
                "id": i + 1, "html_url": format!("https://github.com/o/r/pull/7#issuecomment-{}", i + 1),
                "body": stdout(&output), "user": { "login": "agent-author" },
                "author_association": "OWNER", "updated_at": "2026-10-01T00:00:00Z",
            })
        })
        .collect();
    let _ = std::fs::remove_dir_all(dir);
    let mut routes = vec![
        ("GET /repos/o/r/pulls/7", 200, pull.to_string()),
        ("GET /repos/o/r/pulls/7/files?per_page=100&page=1", 200, Value::Array(files).to_string()),
        ("GET /repos/o/r/issues/7/comments?per_page=100&page=1", 200, Value::Array(comments).to_string()),
    ];
    routes.extend(extra);
    MockGitHub::start(routes)
}

#[test]
fn schema_validate_and_comment_for_each_kind() {
    let dir = temp_dir("reports");
    let github = MockGitHub::start(vec![]);
    for (kind, path) in write_reports(&dir) {
        let path = path.to_str().unwrap();
        let schema: Value = serde_json::from_str(&stdout(&run(&mut github.reviewhub(&["schema", kind])))).unwrap();
        assert!(schema["properties"].is_object(), "{kind}");

        let output = run(&mut github.reviewhub(&["validate", kind, path]));
        assert!(output.status.success() && stdout(&output).contains(&format!("is a valid {kind} report")));

        let output = run(&mut github.reviewhub(&["comment", kind, path]));
        assert!(stdout(&output).starts_with(&format!("<!-- reviewhub:{kind}")), "{}", stdout(&output));
    }

    // From stdin.
    let mut cmd = github.reviewhub(&["validate", "metrics", "-"]);
    cmd.stdin(std::fs::File::open(dir.join("metrics.json")).unwrap());
    assert!(run(&mut cmd).status.success());

    // Not a report, not JSON, not there, not a kind.
    std::fs::write(dir.join("bad.json"), r#"{"version":"1"}"#).unwrap();
    std::fs::write(dir.join("text.json"), "hello").unwrap();
    for (args, error) in [
        (vec!["validate", "manifest", "bad.json"], "is not a valid manifest report"),
        (vec!["validate", "manifest", "text.json"], "is not valid JSON"),
        (vec!["validate", "manifest", "missing.json"], "cannot read missing.json"),
        (vec!["schema", "coverage"], "expected one of: manifest, security, metrics"),
    ] {
        let output = run(github.reviewhub(&args).current_dir(&dir));
        assert!(!output.status.success() && stderr(&output).contains(error), "{args:?}: {}", stderr(&output));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reviews_a_saved_bundle() {
    let github = MockGitHub::start(vec![]);
    let text = stdout(&run(&mut github.reviewhub(&["review", "--bundle", DEMO])));
    assert!(text.contains(demo()["pr"]["title"].as_str().unwrap()), "{text}");
    assert!(text.contains("WHERE TO SPEND YOUR REVIEW") && text.contains("FOCUS"), "{text}");

    let all = stdout(&run(&mut github.reviewhub(&["review", "--bundle", DEMO, "--all", "--diff"])));
    assert!(all.len() > text.len());
    assert!(all.contains("@@"), "the diff of Focus and Skim files: {all}");

    let json: Value =
        serde_json::from_str(&stdout(&run(&mut github.reviewhub(&["review", "--bundle", DEMO, "--json"])))).unwrap();
    assert!(json["focus"]["files"].is_array());
    assert!(json["bundle"]["files"][0].get("patch").is_none(), "patches are left out without --patches");
    let json: Value = serde_json::from_str(&stdout(&run(&mut github.reviewhub(&[
        "review",
        "--bundle",
        DEMO,
        "--json",
        "--patches",
    ]))))
    .unwrap();
    assert!(json["bundle"]["files"][0]["patch"].is_string());

    let briefing = stdout(&run(&mut github.reviewhub(&["briefing", "--bundle", DEMO])));
    assert!(briefing.starts_with("<!-- reviewhub:briefing"), "{briefing}");

    let output = run(&mut github.reviewhub(&["review", "--bundle", "/nonexistent.json"]));
    assert!(!output.status.success());
}

#[test]
fn reviews_a_pull_request_on_github() {
    let github = demo_github(vec![]);
    let output = run(&mut github.reviewhub(&["review", "o/r#7", "--json"]));
    assert!(output.status.success());
    let json: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let reports = &json["bundle"]["reports"];
    assert_eq!(reports["manifest"]["data"], demo()["reports"]["manifest"]["data"]);
    assert_eq!(reports["security"]["data"]["findings"], demo()["reports"]["security"]["data"]["findings"]);
    assert_eq!(reports["security"]["data"]["commit"], "reviewed");
    // Metrics come back with their numbers as floats (85.0 for 85).
    let paths =
        |m: &Value| m["coverage"]["files"].as_array().unwrap().iter().map(|f| f["path"].clone()).collect::<Vec<_>>();
    assert_eq!(paths(&reports["metrics"]["data"]), paths(&demo()["reports"]["metrics"]["data"]));
    assert_eq!(json["bundle"]["files"].as_array().unwrap().len(), demo()["files"].as_array().unwrap().len());
    // It also looked for report files on the head commit.
    let sha = demo()["pr"]["headSha"].as_str().unwrap().to_string();
    assert!(
        github.requests().iter().any(|r| r.path == format!("/repos/o/r/contents/.reviewhub/manifest.json?ref={sha}"))
    );

    // The briefing, too, and via a URL.
    let output = run(&mut github.reviewhub(&["briefing", "https://github.com/o/r/pull/7"]));
    assert!(stdout(&output).starts_with("<!-- reviewhub:briefing"));
}

#[test]
fn explains_github_errors() {
    let github = MockGitHub::start(vec![
        ("GET /repos/o/r/pulls/1", 401, r#"{"message":"Bad credentials"}"#.into()),
        ("GET /repos/o/r/pulls/2", 403, r#"{"message":"Resource not accessible"}"#.into()),
        ("GET /repos/o/r/pulls/3", 500, r#"{"message":"oops"}"#.into()),
    ]);
    for (pr, error) in [
        ("o/r#1", "GitHub rejected the token (401)"),
        ("o/r#2", "403"),
        ("o/r#3", "500"),
        ("o/r#4", "o/r#4 not found"),
        ("nonsense", "use owner/repo#123 or a pull request URL"),
    ] {
        let output = run(&mut github.reviewhub(&["review", pr]));
        assert!(!output.status.success() && stderr(&output).contains(error), "{pr}: {}", stderr(&output));
    }
}

#[test]
fn publishes_reports_as_comments() {
    let dir = temp_dir("publish");
    let reports = write_reports(&dir);
    let manifest = reports[0].1.to_str().unwrap();
    let posted = r#"{"html_url":"https://github.com/o/r/pull/7#issuecomment-1"}"#.to_string();
    let github = MockGitHub::start(vec![
        ("GET /user", 200, r#"{"login":"me"}"#.into()),
        ("GET /repos/o/r/issues/7/comments?per_page=100&page=1", 200, "[]".into()),
        ("POST /repos/o/r/issues/7/comments", 201, posted.clone()),
        (
            "GET /repos/o/r/issues/8/comments?per_page=100&page=1",
            200,
            json!([
                { "id": 5, "body": "<!-- reviewhub:manifest -->\nsomeone else's", "user": { "login": "other" } },
                { "id": 6, "body": "<!-- reviewhub:manifest -->\nmine", "user": { "login": "me" } },
            ])
            .to_string(),
        ),
        ("PATCH /repos/o/r/issues/comments/6", 200, posted.clone()),
    ]);

    // Without a token: refused before asking GitHub anything.
    let output = run(&mut github.reviewhub(&["publish", "manifest", manifest, "--repo", "o/r", "--pr", "7"]));
    assert!(!output.status.success() && stderr(&output).contains("publishing needs a token"));
    assert!(github.requests().is_empty());

    // A new comment, with the token. Outside a git repository nothing is saved for the pre-push hook.
    let output = run(github
        .reviewhub(&["publish", "manifest", manifest, "--repo", "o/r", "--pr", "7"])
        .env("GITHUB_TOKEN", "t0ken")
        .current_dir(&dir));
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "https://github.com/o/r/pull/7#issuecomment-1");
    let post = github.requests().into_iter().find(|r| r.method == "POST").unwrap();
    assert_eq!(post.headers["authorization"], "Bearer t0ken");
    let body: Value = serde_json::from_str(&post.body).unwrap();
    assert!(body["body"].as_str().unwrap().starts_with("<!-- reviewhub:manifest"));

    // Updates this user's earlier comment, not someone else's.
    let output = run(github
        .reviewhub(&["publish", "manifest", manifest, "--repo", "o/r", "--pr", "8"])
        .env("GITHUB_TOKEN", "t0ken"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(github.requests().iter().any(|r| r.method == "PATCH" && r.path == "/repos/o/r/issues/comments/6"));

    let output = run(github
        .reviewhub(&["publish", "manifest", manifest, "--repo", "o-r", "--pr", "7"])
        .env("GITHUB_TOKEN", "t"));
    assert!(stderr(&output).contains("--repo must be owner/repo"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn tells_who_reviews_security() {
    let sha = demo()["pr"]["headSha"].as_str().unwrap().to_string();
    let runs = |status: &str, conclusion: Value| {
        json!({ "check_runs": [{ "name": "ReviewHub security review", "status": status, "conclusion": conclusion }] })
            .to_string()
    };
    let check_runs = |pr: u32| {
        format!("GET /repos/o/r/commits/{sha}-{pr}/check-runs?check_name=ReviewHub%20security%20review&per_page=100")
    };
    let pull = |pr: u32| json!({ "head": { "sha": format!("{sha}-{pr}") } }).to_string();
    let (c1, c2, c3) = (check_runs(1), check_runs(2), check_runs(3));
    let github = MockGitHub::start(vec![
        ("GET /repos/o/r/pulls/1", 200, pull(1)),
        (&c1, 200, runs("in_progress", Value::Null)),
        ("GET /repos/o/r/pulls/2", 200, pull(2)),
        (&c2, 200, runs("completed", json!("skipped"))),
        ("GET /repos/o/r/pulls/3", 200, pull(3)),
        (&c3, 200, json!({ "check_runs": [] }).to_string()),
    ]);
    for (pr, answer) in [("o/r#1", "ci:"), ("o/r#2", "session:"), ("o/r#3", "session:")] {
        let output = run(&mut github.reviewhub(&["security-status", pr, "--wait", "0"]));
        assert!(stdout(&output).starts_with(answer), "{pr}: {}", stdout(&output));
    }
}

#[test]
fn builds_metrics_from_lcov_and_git() {
    let repo = temp_dir("metrics");
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
    };
    git(&["init", "-q"]);
    std::fs::create_dir_all(repo.join("web/src")).unwrap();
    std::fs::write(repo.join("web/src/a.ts"), "export const a = 1;\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "base"]);
    git(&["checkout", "-qb", "feature"]);
    std::fs::write(
        repo.join("web/src/a.ts"),
        "export function a(x: number) {\n  if (x > 1) return 2;\n  return 1;\n}\n",
    )
    .unwrap();
    git(&["commit", "-qam", "change"]);
    // Written by a tool run in web/, with paths relative to it.
    std::fs::create_dir_all(repo.join("web/coverage")).unwrap();
    std::fs::write(repo.join("web/coverage/lcov.info"), "SF:src/a.ts\nDA:1,1\nDA:2,1\nDA:3,0\nend_of_record\n")
        .unwrap();

    let github = MockGitHub::start(vec![]);
    let output = run(github
        .reviewhub(&[
            "metrics",
            "--lcov",
            "web/coverage/lcov.info",
            "--base",
            "main",
            "--base-lines",
            "80",
            "--commit",
            "abc",
        ])
        .current_dir(&repo)
        .env("PATH", "/usr/bin:/bin"));
    assert!(output.status.success(), "{}", stderr(&output));
    let report: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["commit"], "abc");
    let file = &report["coverage"]["files"][0];
    assert_eq!(file["path"], "web/src/a.ts");
    assert_eq!(file["uncoveredLines"], json!([3]));
    assert_eq!(report["coverage"]["baseLines"], 80.0);
    assert_eq!(report["quality"]["files"][0]["path"], "web/src/a.ts", "{report}");

    let output = run(github.reviewhub(&["metrics", "--base", "nope"]).current_dir(&repo).env("PATH", "/usr/bin:/bin"));
    assert!(stderr(&output).contains("can't find a merge base of nope"), "{}", stderr(&output));
    let output = run(github.reviewhub(&["metrics", "--lcov", "missing.info"]).current_dir(&repo));
    assert!(stderr(&output).contains("cannot read missing.info"));
    let _ = std::fs::remove_dir_all(repo);
}

/// A git repository on branch `feature` with one commit, whose `origin` is github.com/o/r.
fn git_repo(name: &str) -> PathBuf {
    let repo = temp_dir(name);
    for args in [
        &["init", "-q", "-b", "feature"][..],
        &["remote", "add", "origin", "https://github.com/o/r.git"],
        &["commit", "-q", "--allow-empty", "-m", "start"],
    ] {
        git(&repo, args);
    }
    repo
}

fn git(repo: &Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
}

/// A stand-in `claude` in `dir` that "reviews" by copying `report` to the reviewer's working directory.
fn fake_claude(dir: &Path, report: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = format!(
        "#!/bin/sh\n[ \"$1\" = --version ] && exit 0\necho \"$@\" > '{log}'\npwd > '{log}.cwd'\nfind . | sort > '{log}.files'\ncat src/app.ts > '{log}.app' 2>/dev/null\ngit rev-parse --git-dir > '{log}.git' 2>&1\necho \"$REVIEWHUB_PLAN_DIR\" > '{log}.plan-dir'\nwhile [ $# -gt 0 ]; do [ \"$1\" = --add-dir ] && dir=\"$2\"; shift; done\ncp '{report}' \"$dir/reviewhub-security.json\"\n",
        log = dir.join("claude-args").display(),
        report = report.display()
    );
    std::fs::write(bin.join("claude"), script).unwrap();
    std::fs::set_permissions(bin.join("claude"), std::fs::Permissions::from_mode(0o755)).unwrap();
    format!("{}:/usr/bin:/bin", bin.display())
}

const SAFE_PATH: &str = "/usr/bin:/bin";

#[cfg(unix)]
#[test]
fn agent_pushes_get_security_reviewed() {
    let repo = git_repo("pushes");
    let reports = temp_dir("pushes-reports");
    let manifest = write_reports(&reports)[0].1.clone();
    let posted = r#"{"html_url":"https://github.com/o/r/pull/7#issuecomment-9"}"#.to_string();
    let github = MockGitHub::start(vec![
        ("GET /repos/o/r/issues/7/comments?per_page=100&page=1", 200, "[]".into()),
        ("POST /repos/o/r/issues/7/comments", 201, posted),
    ]);
    std::fs::create_dir_all(repo.join(".claude/reviewhub")).unwrap();
    std::fs::write(repo.join(".claude/reviewhub/pre-push.sh"), "").unwrap();

    // Publishing a manifest from an agent session saves it for the branch, and asks for a security review.
    let output = run(github
        .reviewhub(&["publish", "manifest", manifest.to_str().unwrap(), "--repo", "o/r", "--pr", "7"])
        .current_dir(&repo)
        .env("PATH", SAFE_PATH)
        .env("CLAUDECODE", "1")
        .env("GITHUB_TOKEN", "t"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("ReviewHub: o/r#7 needs a security review"), "{}", stdout(&output));
    let saved: Value =
        serde_json::from_str(&std::fs::read_to_string(repo.join(".git/reviewhub/manifests/feature.json")).unwrap())
            .unwrap();
    assert_eq!((saved["repo"].as_str(), saved["pr"].as_u64()), (Some("o/r"), Some(7)));

    // The pre-push hook recognises the branch from the saved manifest.
    let push = format!("refs/heads/feature {} refs/heads/feature {}\n", "a".repeat(40), "0".repeat(40));
    let pre_push = |env: &[(&str, &str)], path: &str| {
        let mut cmd = github.reviewhub(&["pre-push", "origin", "https://github.com/o/r.git"]);
        cmd.current_dir(&repo)
            .env("PATH", path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        cmd.envs(env.iter().copied());
        let mut child = cmd.spawn().unwrap();
        use std::io::Write as _;
        child.stdin.take().unwrap().write_all(push.as_bytes()).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "a push is never blocked");
        stderr(&output)
    };
    // In a Claude Code session: the session runs the headless review, which publishes, and doesn't review itself.
    let told = pre_push(&[("CLAUDECODE", "1")], SAFE_PATH);
    assert!(told.contains("run `reviewhub security-review o/r#7` in the background"), "{told}");
    assert!(told.contains("Don't review the change or publish a security report yourself."), "{told}");
    assert_eq!(pre_push(&[("CI", "true")], SAFE_PATH), "");
    // From a terminal without Claude Code installed: says how to get the review instead.
    assert!(pre_push(&[], SAFE_PATH).contains("couldn't be started: Claude Code (`claude`) isn't installed"));
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(reports);
}

#[cfg(unix)]
#[test]
fn pre_push_finds_agent_pull_requests_on_github() {
    let repo = git_repo("pre-push-github");
    let github = demo_github(vec![
        ("GET /repos/o/r/pulls?state=open&head=o:feature&per_page=1", 200, r#"[{"number":7}]"#.into()),
        ("GET /repos/o/r/pulls?state=open&head=o:other&per_page=1", 200, "[]".into()),
    ]);
    let mut child = github
        .reviewhub(&["pre-push", "origin", "git@github.com:o/r.git"])
        .current_dir(&repo)
        .env("PATH", SAFE_PATH)
        .env("CLAUDECODE", "1")
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write as _;
    let sha = "b".repeat(40);
    let zero = "0".repeat(40);
    let input = format!(
        "refs/heads/feature {sha} refs/heads/feature {zero}\nrefs/heads/other {sha} refs/heads/other {zero}\n(delete) {zero} refs/heads/gone {sha}\n"
    );
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let output = child.wait_with_output().unwrap();
    let err = stderr(&output);
    assert!(err.contains("o/r#7 needs a security review"), "{err}");
    assert_eq!(err.matches("needs a security review").count(), 1, "only the agent's branch: {err}");
    let _ = std::fs::remove_dir_all(repo);
}

#[cfg(unix)]
#[test]
fn security_review_runs_claude_unless_done_or_ci_has_it() {
    let check_runs = |sha: &str| {
        format!("GET /repos/o/r/commits/{sha}/check-runs?check_name=ReviewHub%20security%20review&per_page=100")
    };
    let runs = |status: &str, conclusion: Value| {
        json!({ "check_runs": [{ "name": "ReviewHub security review", "status": status, "conclusion": conclusion }] })
            .to_string()
    };
    let dir = temp_dir("security-review");
    let report = dir.join("report.json");
    let mut security = demo()["reports"]["security"]["data"].clone();
    security["commit"] = json!("c0ffee");
    std::fs::write(&report, security.to_string()).unwrap();
    let path = fake_claude(&dir, &report);
    let repo = git_repo("security-review-repo");

    // Already reviewed at the head commit.
    let github = demo_github_at("reviewed", vec![]);
    let output = run(github.reviewhub(&["security-review", "o/r#7"]).current_dir(&repo).env("PATH", &path));
    assert!(stdout(&output).contains("already has a security report for reviewed"), "{}", stdout(&output));

    // CI reviews it.
    let ci = check_runs("ci-head");
    let github = demo_github_at("ci-head", vec![(&ci, 200, runs("queued", Value::Null))]);
    let output = run(github
        .reviewhub(&["security-review", "https://github.com/o/r/pull/7"])
        .current_dir(&repo)
        .env("PATH", &path));
    assert!(stdout(&output).contains("security review workflow reviews o/r#7"), "{}", stdout(&output));
    assert!(!dir.join("claude-args").exists());

    // Nobody has: runs Claude Code, then validates and publishes what it wrote.
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/app.ts"), "export const app = 1;\n\n// end\n").unwrap();
    std::os::unix::fs::symlink("/etc/passwd", repo.join("src/passwords")).unwrap();
    // The pull request's .gitattributes can't hide a file from the reviewer.
    std::fs::write(repo.join("src/hidden.ts"), "export {};\n").unwrap();
    std::fs::write(repo.join(".gitattributes"), "src/hidden.ts export-ignore\n").unwrap();
    // Laid out as a bare repository, whose config could run commands when git runs in the export.
    for dir in ["objects/info", "refs/heads"] {
        std::fs::create_dir_all(repo.join(dir)).unwrap();
        std::fs::write(repo.join(dir).join("keep"), "").unwrap();
    }
    std::fs::write(repo.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    std::fs::write(repo.join("config"), "[core]\n\tbare = true\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "head"]);
    let head = String::from_utf8(
        std::process::Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo).output().unwrap().stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let skipped = check_runs(&head);
    let github = demo_github_at(
        &head,
        vec![
            (&skipped, 200, runs("completed", json!("skipped"))),
            ("GET /user", 200, r#"{"login":"me"}"#.into()),
            (
                "POST /repos/o/r/issues/7/comments",
                201,
                r#"{"html_url":"https://github.com/o/r/pull/7#issuecomment-2"}"#.into(),
            ),
        ],
    );
    let output = run(github
        .reviewhub(&["security-review", "o/r#7", "--sha", &head])
        .current_dir(&repo)
        .env("PATH", &path)
        .env("GITHUB_TOKEN", "t"));
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Published the security report: https://github.com/o/r/pull/7#issuecomment-2"));
    let args = std::fs::read_to_string(dir.join("claude-args")).unwrap();
    assert!(args.contains(&format!("Review o/r#7, head commit {head}")) && args.contains("--allowedTools"), "{args}");
    // The batch reviewer comes from the CLI, not from an agent file the pull request could change.
    assert!(args.contains("--agents {\"reviewhub-batch-reviewer\""), "{args}");
    // Nor do the checkout's project settings, hooks or agents.
    assert!(args.contains("--setting-sources user") && args.contains("--strict-mcp-config"), "{args}");
    // The reviewer runs no git: it reads the head commit's files, exported without symlinks, then removed.
    assert!(!args.contains("Bash(git"), "{args}");
    let cwd = std::fs::read_to_string(dir.join("claude-args.cwd")).unwrap();
    assert!(cwd.trim().ends_with("/head"), "{cwd}");
    assert!(!Path::new(cwd.trim()).exists(), "the export is removed afterwards");
    let files = std::fs::read_to_string(dir.join("claude-args.files")).unwrap();
    assert!(files.contains("./src/app.ts") && files.contains("./src/hidden.ts"), "{files}");
    assert!(!files.contains("passwords"), "{files}");
    let app = std::fs::read_to_string(dir.join("claude-args.app")).unwrap();
    assert_eq!(app, "export const app = 1;\n\n// end\n", "files are exported byte for byte");
    let plan_dir = std::fs::read_to_string(dir.join("claude-args.plan-dir")).unwrap();
    assert!(plan_dir.trim().ends_with("/batches") && args.contains(plan_dir.trim()), "{plan_dir}");
    let git_dir = std::fs::read_to_string(dir.join("claude-args.git")).unwrap();
    assert!(git_dir.contains("not a git repository"), "git must not use the export as a repository: {git_dir}");
    let post = github.requests().into_iter().find(|r| r.method == "POST").unwrap();
    assert!(post.body.contains("reviewhub:security"));

    // Once per commit: a second run finds the lock.
    let output = run(github.reviewhub(&["security-review", "o/r#7"]).current_dir(&repo).env("PATH", &path));
    assert!(stdout(&output).contains("is already running or recently ran"), "{}", stdout(&output));

    // A reviewer that writes nothing usable is an error, not a published report.
    std::fs::write(&report, r#"{"version":"1"}"#).unwrap();
    let output = run(github
        .reviewhub(&["security-review", "o/r#7", "--force"])
        .current_dir(&repo)
        .env("PATH", &path)
        .env("GITHUB_TOKEN", "t"));
    assert!(!output.status.success() && stderr(&output).contains("the report is not valid"), "{}", stderr(&output));

    // A head commit the repository doesn't have and can't fetch.
    git(&repo, &["remote", "set-url", "origin", "/nonexistent"]);
    let missing = "0123456789abcdef0123456789abcdef01234567";
    let github = demo_github_at(missing, vec![(&check_runs(missing), 200, runs("completed", json!("skipped")))]);
    let output = run(github.reviewhub(&["security-review", "o/r#7"]).current_dir(&repo).env("PATH", &path));
    assert!(stderr(&output).contains(&format!("doesn't have commit {missing}")), "{}", stderr(&output));

    let output = run(&mut github.reviewhub(&["security-review", "nonsense"]));
    assert!(stderr(&output).contains("expected owner/repo#123"));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(repo);
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("reviewhub-security-{head}")));
}

#[test]
fn plans_and_merges_a_batched_security_review() {
    let github = MockGitHub::start(vec![]);
    let dir = temp_dir("plan");
    let output = run(github
        .reviewhub(&["security-plan", "--bundle", DEMO, "--out-dir", dir.join("batches").to_str().unwrap()])
        .env("REVIEWHUB_MODEL_STRONG", "fable"));
    assert!(output.status.success(), "{}", stderr(&output));
    let plan: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(plan["mode"], "batched");
    assert_eq!(plan["coordinatorModel"], "fable");
    assert_eq!(plan["pr"], "demo/acme-shop#128");
    let batches = plan["batches"].as_array().unwrap();
    let models: Vec<&str> = batches.iter().map(|b| b["model"].as_str().unwrap()).collect();
    // The demo's docs are about refunds and its snapshots run: no batch is light enough for the light model.
    assert_eq!(models, ["fable", "fable", "sonnet", "sonnet"]);
    for batch in batches {
        let prompt = batch["prompt"].as_str().unwrap();
        let output = batch["output"].as_str().unwrap();
        let notes = batch["notes"].as_str().unwrap();
        assert!(prompt.contains(output) && prompt.contains(notes) && prompt.contains("injection"));
        // Batch reviewers write no files (one could overwrite another's report): they reply, and the launcher saves.
        assert!(prompt.contains("Don't create or modify any files.") && prompt.contains("then a line `NOTES:`"));
        // Replies are saved as they are, so they must be the validated report (a trial run lost `version`).
        assert!(prompt.contains("`version` \"1\"") && prompt.contains("exactly as it passed validation"));
        for file in batch["files"].as_array().unwrap() {
            assert!(prompt.contains(file["path"].as_str().unwrap()));
        }
    }
    // With REVIEWHUB_PLAN_DIR set, as for reviewers, no other --out-dir is accepted.
    let pinned = run(github
        .reviewhub(&["security-plan", "--bundle", DEMO, "--out-dir", dir.join("elsewhere").to_str().unwrap()])
        .env("REVIEWHUB_PLAN_DIR", dir.join("pinned")));
    assert!(!pinned.status.success() && stderr(&pinned).contains("--out-dir must be"), "{}", stderr(&pinned));
    assert!(!dir.join("elsewhere").exists());

    // The batch reviewer's definition, for the workflow's --agents.
    let agents: Value = serde_json::from_str(&stdout(&run(&mut github.reviewhub(&["security-agents"])))).unwrap();
    assert_eq!(agents["reviewhub-batch-reviewer"]["tools"], json!(["Read", "Grep", "Glob", "Bash"]));
    for batch in batches {
        assert!(batch["prompt"].as_str().unwrap().contains("Don't create or modify any files."));
    }

    // A directory that already exists could hold planted reports.
    let again = run(&mut github.reviewhub(&[
        "security-plan",
        "--bundle",
        DEMO,
        "--out-dir",
        dir.join("batches").to_str().unwrap(),
    ]));
    assert!(!again.status.success() && stderr(&again).contains("it must not exist yet"), "{}", stderr(&again));
    // Paths are quoted in the instructions.
    assert!(batches[0]["prompt"].as_str().unwrap().contains("- \"src/"), "{}", batches[0]["prompt"]);

    // The plan is saved for the reviewer of the whole pull request.
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("batches/plan.json")).unwrap()).unwrap();
    assert_eq!(saved, plan);
    assert_eq!(plan["planFile"], dir.join("batches/plan.json").display().to_string());

    // Each batch writes a report about its files; merging keeps one of a duplicate, the more severe.
    let security = demo()["reports"]["security"]["data"].clone();
    let findings = security["findings"].as_array().unwrap();
    let head = plan["headSha"].as_str().unwrap();
    let report_for = |batch: &Value| {
        let files: Vec<&str> = batch["files"].as_array().unwrap().iter().map(|f| f["path"].as_str().unwrap()).collect();
        let mut report = security.clone();
        report["scanner"] =
            json!({ "name": format!("reviewhub security-reviewer ({})", batch["id"].as_str().unwrap()) });
        report["commit"] = json!(head);
        report["findings"] =
            json!(findings.iter().filter(|f| files.contains(&f["file"].as_str().unwrap())).collect::<Vec<_>>());
        report
    };
    let with_findings = batches.iter().position(|b| !report_for(b)["findings"].as_array().unwrap().is_empty()).unwrap();
    let expected = findings.len();
    for (i, batch) in batches.iter().enumerate() {
        let mut report = report_for(batch);
        if i == with_findings {
            let mut duplicate = report["findings"][0].clone();
            duplicate["severity"] = json!("info");
            report["findings"].as_array_mut().unwrap().push(duplicate);
        }
        std::fs::write(batch["output"].as_str().unwrap(), report.to_string()).unwrap();
    }
    let plan_file = dir.join("batches/plan.json");
    let merge = |plan: &Path, extra: &[&str]| {
        let mut args = vec!["security-merge", "--bundle", DEMO, "--plan", plan.to_str().unwrap()];
        args.extend_from_slice(extra);
        run(&mut github.reviewhub(&args))
    };
    let output = merge(&plan_file, &["--model", "fable"]);
    assert!(output.status.success() && stderr(&output).is_empty(), "{}", stderr(&output));
    let report: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["findings"].as_array().unwrap().len(), expected);
    assert_eq!(report["scanner"], json!({ "name": "reviewhub security-reviewer", "model": "fable" }));
    assert_eq!(report["commit"], head);
    assert!(!report["summary"].as_str().unwrap().contains("No usable report"));
    let merged = dir.join("merged.json");
    std::fs::write(&merged, report.to_string()).unwrap();
    assert!(run(&mut github.reviewhub(&["validate", "security", merged.to_str().unwrap()])).status.success());
    // It only prints: batch reviewers can run it too, and mustn't plant a file where the reviewer's report goes.
    let out = merge(&plan_file, &["--out", merged.to_str().unwrap()]);
    assert!(!out.status.success() && stderr(&out).contains("unexpected argument '--out'"), "{}", stderr(&out));
    let printed: Value = serde_json::from_str(&stdout(&merge(&plan_file, &[]))).unwrap();
    assert_eq!(printed["scanner"], json!({ "name": "reviewhub security-reviewer" }));

    // A batch reviewer reads the author's files and could be talked into writing another batch's report: a report
    // that isn't by its batch's reviewer, on the head commit and about its batch's files isn't used.
    let output_of = |i: usize| PathBuf::from(batches[i]["output"].as_str().unwrap());
    let mut forged = report_for(&batches[0]);
    forged["scanner"]["name"] = json!("reviewhub security-reviewer (batch-9)");
    std::fs::write(output_of(0), forged.to_string()).unwrap();
    let mut stale = report_for(&batches[1]);
    stale["commit"] = json!("0000000");
    std::fs::write(output_of(1), stale.to_string()).unwrap();
    let mut elsewhere = report_for(&batches[2]);
    let mut outside = findings[0].clone();
    (outside["id"], outside["file"]) = (json!("x"), json!("README.md"));
    elsewhere["findings"] = json!([outside]);
    std::fs::write(output_of(2), elsewhere.to_string()).unwrap();
    std::fs::write(output_of(3), "{}").unwrap();
    let output = merge(&plan_file, &[]);
    assert!(output.status.success(), "{}", stderr(&output));
    let err = stderr(&output);
    for (i, reason) in [
        "its scanner is \"reviewhub security-reviewer (batch-9)\"",
        "it reviews commit 0000000, not the head commit",
        "finding x is in README.md, which isn't one of the batch's files",
        "is not a valid security report",
    ]
    .iter()
    .enumerate()
    {
        let id = batches[i]["id"].as_str().unwrap();
        assert!(err.contains(&format!("No usable report from {id} (")) && err.contains(reason), "{reason}: {err}");
    }
    assert!(err.contains("Review its files yourself."), "{err}");
    let partial: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let summary = partial["summary"].as_str().unwrap();
    assert!(summary.contains(&format!("No usable report from {} (", batches[0]["id"].as_str().unwrap())), "{summary}");
    std::fs::write(output_of(3), "not json").unwrap();
    std::fs::remove_file(output_of(2)).unwrap();
    let err = stderr(&merge(&plan_file, &[]));
    assert!(err.contains("is not valid JSON") && err.contains("cannot read"), "{err}");

    // The plan must still be the one for the pull request's head: not another pull request's, not a stale one, not
    // edited to drop a batch.
    let tampered = |name: &str, change: &dyn Fn(&mut Value)| {
        let mut changed = plan.clone();
        change(&mut changed);
        let file = dir.join(name);
        std::fs::write(&file, changed.to_string()).unwrap();
        stderr(&merge(&file, &[]))
    };
    assert!(
        tampered("other.json", &|p| p["pr"] = json!("demo/other#1")).contains("is not a plan for demo/acme-shop#128")
    );
    assert!(tampered("stale.json", &|p| p["headSha"] = json!("abc")).contains("plan the review again"));
    let dropped = tampered("dropped.json", &|p| {
        p["batches"].as_array_mut().unwrap().pop();
    });
    assert!(dropped.contains("doesn't match demo/acme-shop#128's changed files"), "{dropped}");
    let renamed = tampered("renamed.json", &|p| p["batches"][0]["files"][0]["path"] = json!("README.md"));
    assert!(renamed.contains("doesn't match"), "{renamed}");
    assert!(stderr(&merge(&dir.join("missing.json"), &[])).contains("cannot read"));
    std::fs::write(dir.join("broken.json"), "plan").unwrap();
    assert!(stderr(&merge(&dir.join("broken.json"), &[])).contains("is not valid JSON"));

    // A small pull request is reviewed in one pass: no batches, no directory.
    let mut small = demo();
    small["files"] = json!([demo()["files"][0], demo()["files"][1]]);
    let small_bundle = dir.join("small.json");
    std::fs::write(&small_bundle, small.to_string()).unwrap();
    let output = run(&mut github.reviewhub(&["security-plan", "--bundle", small_bundle.to_str().unwrap()]));
    let single: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!((single["mode"].as_str(), single["batches"].as_array().map(Vec::len)), (Some("single"), Some(0)));
    assert!(single.get("planFile").is_none());

    // Without --out-dir, the batches get a new private directory.
    let output = run(&mut github.reviewhub(&["security-plan", "--bundle", DEMO]));
    let fresh: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let plan_file = PathBuf::from(fresh["planFile"].as_str().unwrap());
    assert!(plan_file.is_file() && plan_file.parent().unwrap() != dir.join("batches"));
    std::fs::remove_dir_all(plan_file.parent().unwrap()).unwrap();

    let _ = std::fs::remove_dir_all(dir);
}
