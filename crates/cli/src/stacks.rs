//! Which test stacks a repository uses, for the coverage part of the metrics
//! workflow `reviewhub init` writes: Rust (cargo-llvm-cov), JavaScript and
//! TypeScript with Vitest or Jest, and Python with pytest. Looks at the
//! repository root and its direct subdirectories.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Runner {
    Vitest,
    Jest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stack {
    Rust { dir: String },
    Node { dir: String, runner: Runner, pm: PackageManager },
    Python { dir: String },
}

const SKIP: &[&str] = &["node_modules", "target", "vendor", "dist", "build", "venv", "fixtures", "examples", "docs"];

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn detect_in(root: &Path, dir: &str) -> Vec<Stack> {
    let path = if dir.is_empty() { root.to_path_buf() } else { root.join(dir) };
    let mut found = Vec::new();
    if path.join("Cargo.toml").is_file() {
        found.push(Stack::Rust { dir: dir.to_string() });
    }
    let package = read(&path.join("package.json"));
    if !package.is_empty() {
        let runner = if package.contains("\"vitest\"") {
            Some(Runner::Vitest)
        } else if package.contains("\"jest\"") {
            Some(Runner::Jest)
        } else {
            None
        };
        if let Some(runner) = runner {
            let pm = if path.join("pnpm-lock.yaml").is_file() {
                PackageManager::Pnpm
            } else if path.join("yarn.lock").is_file() {
                PackageManager::Yarn
            } else {
                PackageManager::Npm
            };
            found.push(Stack::Node { dir: dir.to_string(), runner, pm });
        }
    }
    let python = ["pyproject.toml", "setup.cfg", "setup.py", "requirements.txt", "requirements-dev.txt", "tox.ini"]
        .iter()
        .map(|f| read(&path.join(f)))
        .any(|text| text.contains("pytest"));
    if python {
        found.push(Stack::Python { dir: dir.to_string() });
    }
    found
}

/// The stacks in `root` and its direct subdirectories. A Rust workspace at the root covers its member crates.
pub fn detect(root: &Path) -> Vec<Stack> {
    let mut stacks = detect_in(root, "");
    let root_has_rust = stacks.iter().any(|s| matches!(s, Stack::Rust { .. }));
    let mut dirs: Vec<String> = fs::read_dir(root)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| !name.starts_with('.') && !SKIP.contains(&name.as_str()))
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    for dir in dirs {
        stacks
            .extend(detect_in(root, &dir).into_iter().filter(|s| !(root_has_rust && matches!(s, Stack::Rust { .. }))));
    }
    stacks
}

fn cd(dir: &str) -> String {
    if dir.is_empty() { String::new() } else { format!("cd {dir} && ") }
}

fn name(prefix: &str, dir: &str) -> String {
    if dir.is_empty() { prefix.to_string() } else { format!("{prefix}-{}", dir.replace('/', "-")) }
}

/// GitHub Actions steps that install the toolchains, indented for a job's `steps:`.
pub fn setup_steps(stacks: &[Stack]) -> String {
    let mut steps = String::new();
    let mut seen = Vec::new();
    for stack in stacks {
        let kind = std::mem::discriminant(stack);
        let first = !seen.contains(&kind);
        seen.push(kind);
        match stack {
            Stack::Rust { .. } if first => steps.push_str(
                "      - uses: dtolnay/rust-toolchain@stable\n        with:\n          components: llvm-tools-preview\n      \
                 - uses: taiki-e/install-action@cargo-llvm-cov\n",
            ),
            Stack::Node { dir, pm, .. } => {
                if first {
                    if *pm == PackageManager::Pnpm {
                        steps.push_str("      - uses: pnpm/action-setup@v5\n");
                    }
                    let lock = match pm {
                        PackageManager::Npm => "package-lock.json",
                        PackageManager::Pnpm => "pnpm-lock.yaml",
                        PackageManager::Yarn => "yarn.lock",
                    };
                    let cache = match pm {
                        PackageManager::Npm => "npm",
                        PackageManager::Pnpm => "pnpm",
                        PackageManager::Yarn => "yarn",
                    };
                    let lock_path = if dir.is_empty() { lock.to_string() } else { format!("{dir}/{lock}") };
                    steps.push_str(&format!(
                        "      - uses: actions/setup-node@v7\n        with:\n          node-version: 24\n          \
                         cache: {cache}\n          cache-dependency-path: {lock_path}\n"
                    ));
                }
            }
            Stack::Python { .. } if first => {
                steps.push_str("      - uses: actions/setup-python@v7\n        with:\n          python-version: \"3.x\"\n")
            }
            _ => {}
        }
    }
    steps
}

/// The coverage script's commands, one block per stack, each writing `$out/<name>.info`.
pub fn coverage_commands(stacks: &[Stack]) -> String {
    let mut out = String::new();
    for stack in stacks {
        match stack {
            Stack::Rust { dir } => out.push_str(&format!(
                "\n# Rust{}\n({}cargo llvm-cov --workspace --lcov --output-path \"$out/{}.info\") || status=1\n",
                if dir.is_empty() { String::new() } else { format!(": {dir}") },
                cd(dir),
                name("rust", dir)
            )),
            Stack::Node { dir, runner, pm } => {
                let install = match pm {
                    PackageManager::Npm => "npm ci",
                    PackageManager::Pnpm => "pnpm install --frozen-lockfile",
                    PackageManager::Yarn => "yarn install --frozen-lockfile",
                };
                let report = name("node", dir);
                let run = match runner {
                    Runner::Vitest => format!(
                        "npm install --no-save \"@vitest/coverage-v8@$(node -p \"require('vitest/package.json').version\")\" && \
                         npx vitest run --coverage --coverage.reporter=lcov --coverage.reportsDirectory=\"$out/{report}\""
                    ),
                    Runner::Jest => {
                        format!("npx jest --coverage --coverageReporters=lcov --coverageDirectory=\"$out/{report}\"")
                    }
                };
                out.push_str(&format!(
                    "\n# JavaScript/TypeScript ({}){}\n({}{install} && {run}) || status=1\n\
                     [ -f \"$out/{report}/lcov.info\" ] && mv \"$out/{report}/lcov.info\" \"$out/{report}.info\"\n",
                    if *runner == Runner::Vitest { "Vitest" } else { "Jest" },
                    if dir.is_empty() { String::new() } else { format!(": {dir}") },
                    cd(dir)
                ));
            }
            Stack::Python { dir } => out.push_str(&format!(
                "\n# Python (pytest){}\n({}python -m pip install --quiet pytest pytest-cov \
                 && {{ [ ! -f requirements.txt ] || python -m pip install --quiet -r requirements.txt; }} \
                 && {{ [ ! -f pyproject.toml ] || python -m pip install --quiet -e . 2>/dev/null || true; }} \
                 && python -m pytest --cov --cov-report=\"lcov:$out/{}.info\") || status=1\n",
                if dir.is_empty() { String::new() } else { format!(": {dir}") },
                cd(dir),
                name("python", dir)
            )),
        }
    }
    if out.is_empty() {
        out.push_str(
            "\n# No test stack was detected. Add commands that write lcov files to \"$out\", for example:\n\
             # npx vitest run --coverage --coverage.reporter=lcov --coverage.reportsDirectory=\"$out/node\"\n",
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_stacks_at_the_root_and_one_level_down() {
        let dir = std::env::temp_dir().join(format!("reviewhub-stacks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for sub in ["crates/core", "web", "api", "node_modules/x", "docs"] {
            fs::create_dir_all(dir.join(sub)).unwrap();
        }
        fs::write(dir.join("Cargo.toml"), "[workspace]").unwrap();
        fs::write(dir.join("crates/Cargo.toml"), "[package]").unwrap();
        fs::write(dir.join("web/package.json"), r#"{"devDependencies":{"vitest":"^3"}}"#).unwrap();
        fs::write(dir.join("web/pnpm-lock.yaml"), "").unwrap();
        fs::write(dir.join("api/pyproject.toml"), "[project.optional-dependencies]\ntest = [\"pytest\"]").unwrap();
        fs::write(dir.join("docs/package.json"), r#"{"devDependencies":{"jest":"^29"}}"#).unwrap();

        assert_eq!(
            detect(&dir),
            vec![
                Stack::Rust { dir: String::new() },
                Stack::Python { dir: "api".into() },
                Stack::Node { dir: "web".into(), runner: Runner::Vitest, pm: PackageManager::Pnpm },
            ]
        );
        let commands = coverage_commands(&detect(&dir));
        assert!(commands.contains("cargo llvm-cov --workspace --lcov --output-path \"$out/rust.info\""), "{commands}");
        assert!(commands.contains("(cd web && pnpm install --frozen-lockfile && npm install --no-save"), "{commands}");
        assert!(commands.contains("lcov:$out/python-api.info"), "{commands}");
        let setup = setup_steps(&detect(&dir));
        assert!(
            setup.contains("pnpm/action-setup") && setup.contains("cache-dependency-path: web/pnpm-lock.yaml"),
            "{setup}"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn leaves_a_note_when_nothing_is_detected() {
        assert!(coverage_commands(&[]).contains("No test stack was detected"));
        assert!(setup_steps(&[]).is_empty());
    }
}
