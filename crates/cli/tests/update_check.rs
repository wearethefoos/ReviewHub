//! `reviewhub update-check` against a stand-in GitHub.

mod support;

use serde_json::Value;
use support::{MockGitHub, run, temp_dir};

const LATEST: &str = "GET /repos/wearethefoos/ReviewHub/releases/latest";

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn tells_plugin_users_how_to_update_without_sending_their_token() {
    let github = MockGitHub::start(vec![(LATEST, 200, r#"{"tag_name":"v99.0.0"}"#.into())]);
    let cache = temp_dir("plugin");
    let output = run(github
        .reviewhub(&["update-check", "--for", "plugin", "--hook"])
        .env("REVIEWHUB_CACHE_DIR", &cache)
        .env("GITHUB_TOKEN", "secret-token"));
    assert!(output.status.success());
    let hook: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let message = hook["systemMessage"].as_str().unwrap();
    assert!(message.starts_with("ReviewHub 99.0.0 is available"), "{message}");
    assert!(message.contains("/plugin marketplace update reviewhub"), "{message}");
    assert_eq!(hook["hookSpecificOutput"]["hookEventName"], "SessionStart");

    let requests = github.requests();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].headers.contains_key("authorization"), "{:?}", requests[0].headers);

    // The answer is cached for a day: a second run doesn't ask again.
    let output = run(github.reviewhub(&["update-check"]).env("REVIEWHUB_CACHE_DIR", &cache));
    assert!(stdout(&output).starts_with("ReviewHub 99.0.0 is available"), "{}", stdout(&output));
    assert!(stdout(&output).contains("install.sh | sh`."));
    assert_eq!(github.requests().len(), 1);
    let _ = std::fs::remove_dir_all(cache);
}

#[test]
fn says_nothing_when_current_offline_or_turned_off() {
    let github = MockGitHub::start(vec![(LATEST, 200, r#"{"tag_name":"v0.1.0"}"#.into())]);
    let cache = temp_dir("quiet");
    // Up to date (the launcher's pinned version is what counts).
    let output = run(github
        .reviewhub(&["update-check", "--for", "repo"])
        .env("REVIEWHUB_CACHE_DIR", &cache)
        .env("REVIEWHUB_PINNED_VERSION", "0.1.0"));
    assert!(output.status.success() && output.stdout.is_empty());

    // Turned off: doesn't even ask.
    let before = github.requests().len();
    let other_cache = temp_dir("quiet-off");
    let output = run(github
        .reviewhub(&["update-check"])
        .env("REVIEWHUB_CACHE_DIR", &other_cache)
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"));
    assert!(output.status.success() && output.stdout.is_empty());
    assert_eq!(github.requests().len(), before);

    // GitHub has no answer (and nothing cached): quiet, and still succeeds.
    let empty = MockGitHub::start(vec![]);
    let output = run(empty.reviewhub(&["update-check"]).env("REVIEWHUB_CACHE_DIR", &other_cache));
    assert!(output.status.success() && output.stdout.is_empty());
    let _ = std::fs::remove_dir_all(cache);
    let _ = std::fs::remove_dir_all(other_cache);
}

#[test]
fn rejects_unknown_installs() {
    let github = MockGitHub::start(vec![]);
    let output = run(&mut github.reviewhub(&["update-check", "--for", "browser"]));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("expected one of: plugin, repo, cli, extension-unpacked"));
}

/// The session-start hook `reviewhub init` writes: puts the CLI on PATH, then passes on the update notice.
#[cfg(unix)]
#[test]
fn repo_session_start_hook_tells_about_updates() {
    use std::os::unix::fs::PermissionsExt;
    let github = MockGitHub::start(vec![(LATEST, 200, r#"{"tag_name":"v99.0.0"}"#.into())]);
    let dir = temp_dir("session-start");
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let script = dir.join("session-start.sh");
    std::fs::write(&script, include_str!("../templates/session-start.sh")).unwrap();
    // The pinned-CLI launcher, standing in as this build.
    let launcher = dir.join("bin/reviewhub");
    std::fs::write(&launcher, format!("#!/bin/sh\nexec '{}' \"$@\"\n", env!("CARGO_BIN_EXE_reviewhub"))).unwrap();
    std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env_file = dir.join("env");

    let output = run(github
        .command("sh")
        .arg(&script)
        .current_dir(&dir)
        .env("REVIEWHUB_CACHE_DIR", dir.join("cache"))
        .env("CLAUDE_ENV_FILE", &env_file)
        .env("PATH", "/usr/bin:/bin"));
    assert!(output.status.success());
    let hook: Value = serde_json::from_str(&stdout(&output)).unwrap();
    let message = hook["systemMessage"].as_str().unwrap();
    assert!(message.starts_with("ReviewHub 99.0.0 is available") && message.contains("-s -- init"), "{message}");
    let path_line = std::fs::read_to_string(&env_file).unwrap();
    assert!(path_line.contains(&format!("{}:", dir.join("bin").display())), "{path_line}");
    let _ = std::fs::remove_dir_all(dir);
}
