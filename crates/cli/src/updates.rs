//! `reviewhub update-check`: whether a newer ReviewHub release exists, checked
//! at most once a day, and what to do about it for this kind of install. Used
//! by the Claude Code plugin's and `reviewhub init`'s session-start hooks.

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use reviewhub_core::update::{self, Install};

use crate::github::GitHub;

const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// Off when the user turned off ReviewHub's check, or Claude Code's updates or non-essential traffic.
fn disabled() -> bool {
    ["REVIEWHUB_NO_UPDATE_CHECK", "DISABLE_UPDATES", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"]
        .iter()
        .any(|v| std::env::var(v).is_ok_and(|v| !v.is_empty() && v != "0"))
}

fn cache_file() -> Option<PathBuf> {
    let dir = std::env::var_os("REVIEWHUB_CACHE_DIR")
        .or_else(|| std::env::var_os("CLAUDE_PLUGIN_DATA"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CACHE_HOME").map(|d| PathBuf::from(d).join("reviewhub")))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/reviewhub")))?;
    Some(dir.join("latest-release.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The latest release tag: from the cache when it is under a day old, else from GitHub (3 s at most).
fn latest_release() -> Option<String> {
    let cache = cache_file();
    let cached: Option<Value> =
        cache.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok());
    if let Some(c) = &cached
        && now().saturating_sub(c["checked"].as_u64().unwrap_or(0)) < CHECK_EVERY.as_secs()
    {
        return c["tag"].as_str().map(String::from);
    }
    let tag = GitHub::with_timeout(crate::github::resolve_token(), Some(Duration::from_secs(3)))
        .latest_reviewhub_release()
        .ok();
    match (&tag, &cache) {
        (Some(tag), Some(file)) => {
            if let Some(dir) = file.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(file, json!({ "tag": tag, "checked": now() }).to_string());
            Some(tag.clone())
        }
        // Offline: fall back to an older answer rather than none.
        (None, _) => cached.and_then(|c| c["tag"].as_str().map(String::from)),
        (Some(tag), None) => Some(tag.clone()),
    }
}

/// Prints the update notice, if any. With `hook`, as Claude Code SessionStart hook output: shown to the user,
/// and given to Claude so it can mention it. Never fails: an update check mustn't get in the way.
pub fn check(install: Install, hook: bool) {
    if disabled() {
        return;
    }
    // Launchers run the CLI of a pinned version; while a new release's binaries are still building they fall back
    // to an older CLI, so the pin (not this binary's version) is what's installed.
    let current = std::env::var("REVIEWHUB_PINNED_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let Some(latest) = latest_release() else { return };
    let Some(message) = update::notice(install, &current, &latest) else { return };
    if hook {
        let context = format!("{message} Tell the user this in one short line at the start of your first reply.");
        println!(
            "{}",
            json!({
                "systemMessage": message,
                "hookSpecificOutput": { "hookEventName": "SessionStart", "additionalContext": context }
            })
        );
    } else {
        println!("{message}");
    }
}
