//! A stand-in GitHub API for running the `reviewhub` binary in tests: answers
//! canned JSON per method and path, and records every request it gets.
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct MockGitHub {
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl MockGitHub {
    /// Serves `routes`: `("GET /repos/o/r/pulls/1", status, body)`. Unknown routes get a 404.
    pub fn start(routes: Vec<(&str, u16, String)>) -> MockGitHub {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let routes: HashMap<String, (u16, String)> =
            routes.into_iter().map(|(route, status, body)| (route.to_string(), (status, body))).collect();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&requests);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let Some(request) = read_request(&mut stream) else { continue };
                let (status, body) = routes
                    .get(&format!("{} {}", request.method, request.path))
                    .cloned()
                    .unwrap_or((404, r#"{"message":"Not Found"}"#.into()));
                log.lock().unwrap().push(request);
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        MockGitHub { url, requests }
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    /// The `reviewhub` binary, talking to this server, with no token, proxy or cache from the environment.
    pub fn reviewhub(&self, args: &[&str]) -> Command {
        let mut cmd = self.command(env!("CARGO_BIN_EXE_reviewhub"));
        cmd.args(args);
        cmd
    }

    /// `program` with an environment that sends `reviewhub` to this server, like [`MockGitHub::reviewhub`].
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut cmd = Command::new(program);
        cmd.env("GITHUB_API_URL", &self.url).env("PATH", "/nonexistent").stdin(Stdio::null());
        for var in [
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "HTTPS_PROXY",
            "https_proxy",
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
            "REVIEWHUB_NO_UPDATE_CHECK",
            "DISABLE_UPDATES",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "REVIEWHUB_PINNED_VERSION",
            "GITHUB_ACTIONS",
        ] {
            cmd.env_remove(var);
        }
        cmd
    }
}

fn read_request(stream: &mut std::net::TcpStream) -> Option<Request> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next()?.to_string(), parts.next()?.to_string());
    let mut headers = HashMap::new();
    loop {
        line.clear();
        reader.read_line(&mut line).ok()?;
        let Some((name, value)) = line.trim_end().split_once(':') else { break };
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    let length = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Request { method, path, headers, body: String::from_utf8_lossy(&body).into() })
}

pub fn run(cmd: &mut Command) -> Output {
    let output = cmd.output().unwrap();
    eprintln!(
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// A fresh, empty directory for one test.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("reviewhub-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
