//! Report kinds, schema validation and the PR comment format shared with the
//! web app (`src/lib/sources/extract.ts`).

use std::sync::LazyLock;

use regex::Regex;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::model::{MetricsReport, ReviewManifest, SecurityReport};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Manifest,
    Security,
    Metrics,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Manifest, Kind::Security, Kind::Metrics];

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Manifest => "manifest",
            Kind::Security => "security",
            Kind::Metrics => "metrics",
        }
    }

    fn parse(s: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// JSON Schema generated from the Zod schemas by `npm run generate`.
    pub fn schema(self) -> &'static str {
        match self {
            Kind::Manifest => include_str!("../../schemas/manifest.schema.json"),
            Kind::Security => include_str!("../../schemas/security.schema.json"),
            Kind::Metrics => include_str!("../../schemas/metrics.schema.json"),
        }
    }

    pub fn marker(self) -> String {
        format!("<!-- reviewhub:{} -->", self.as_str())
    }

    fn title(self) -> &'static str {
        match self {
            Kind::Manifest => "review manifest",
            Kind::Security => "security review",
            Kind::Metrics => "coverage and quality metrics",
        }
    }
}

/// A validated report of any kind.
#[derive(Debug, Clone)]
pub enum Report {
    Manifest(ReviewManifest),
    Security(SecurityReport),
    Metrics(MetricsReport),
}

impl Report {
    /// The report as JSON, without `null`s: the web app's schemas treat absent and null differently.
    pub fn to_value(&self) -> Value {
        let mut value = match self {
            Report::Manifest(r) => serde_json::to_value(r),
            Report::Security(r) => serde_json::to_value(r),
            Report::Metrics(r) => serde_json::to_value(r),
        }
        .expect("reports serialize");
        strip_nulls(&mut value);
        value
    }

    pub fn summary(&self) -> String {
        match self {
            Report::Security(r) => match r.findings.len() {
                0 => "no findings".into(),
                1 => "1 finding".into(),
                n => format!("{n} findings"),
            },
            Report::Metrics(r) => {
                r.coverage.as_ref().and_then(|c| c.lines).map(|l| format!("{l:.1}% line coverage")).unwrap_or_default()
            }
            Report::Manifest(_) => String::new(),
        }
    }
}

fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !v.is_null());
            map.values_mut().for_each(strip_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

/// Validates `json` against the schema for `kind`, then deserializes it with defaults applied.
pub fn validate(kind: Kind, json: &Value) -> Result<Report, String> {
    let schema: Value = serde_json::from_str(kind.schema()).expect("embedded schema is valid JSON");
    let validator = jsonschema::validator_for(&schema).expect("embedded schema compiles");
    let errors: Vec<String> = validator
        .iter_errors(json)
        .map(|e| {
            let path = e.instance_path().to_string();
            if path.is_empty() { format!("✖ {e}") } else { format!("✖ {e}\n  → at {path}") }
        })
        .collect();
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    fn de<T: DeserializeOwned>(json: &Value) -> Result<T, String> {
        serde_json::from_value(json.clone()).map_err(|e| e.to_string())
    }
    Ok(match kind {
        Kind::Manifest => Report::Manifest(de(json)?),
        Kind::Security => Report::Security(de(json)?),
        Kind::Metrics => Report::Metrics(de(json)?),
    })
}

/// Renders a report as a PR comment body that ReviewHub (web and CLI) reads back.
pub fn render_comment(kind: Kind, report: &Report) -> String {
    let summary = report.summary();
    let heading = if summary.is_empty() {
        format!("**ReviewHub {}**", kind.title())
    } else {
        format!("**ReviewHub {}**: {summary}", kind.title())
    };
    let json = serde_json::to_string_pretty(&report.to_value()).expect("reports serialize");
    [
        kind.marker().as_str(),
        &heading,
        "",
        "<details><summary>Report data</summary>",
        "",
        "```json",
        &json,
        "```",
        "",
        "</details>",
    ]
    .join("\n")
}

static MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!--\s*reviewhub:(manifest|security|metrics)\s*-->").unwrap());
static JSON_FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```json\s*\n(.*?)\n```").unwrap());

pub struct Extracted {
    pub kind: Kind,
    pub result: Result<Report, String>,
}

/// Reads a report out of a comment body, if the comment carries a ReviewHub marker.
pub fn extract(body: &str) -> Option<Extracted> {
    let marker = MARKER.captures(body)?;
    let kind = Kind::parse(&marker[1])?;
    let start = marker.get(0).unwrap().start();
    let Some(fence) = JSON_FENCE.captures(&body[start..]) else {
        return Some(Extracted { kind, result: Err("Marker found but no ```json block follows it.".into()) });
    };
    let result = match serde_json::from_str::<Value>(&fence[1]) {
        Ok(json) => validate(kind, &json),
        Err(e) => Err(format!("Invalid JSON: {e}")),
    };
    Some(Extracted { kind, result })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metrics() -> Value {
        json!({ "version": "1", "coverage": { "lines": 80, "files": [{ "path": "a.ts", "lines": 50, "uncoveredLines": [3] }] } })
    }

    #[test]
    fn round_trips_a_comment() {
        let report = validate(Kind::Metrics, &metrics()).unwrap();
        let body = render_comment(Kind::Metrics, &report);
        assert!(body.contains("80.0% line coverage"));
        assert!(!body.contains("null"), "absent fields must be omitted, not null");
        let extracted = extract(&body).unwrap();
        assert_eq!(extracted.kind, Kind::Metrics);
        let Report::Metrics(m) = extracted.result.unwrap() else { panic!("wrong kind") };
        assert_eq!(m.coverage.unwrap().files[0].uncovered_lines, vec![3]);
    }

    #[test]
    fn reads_only_the_block_after_the_marker() {
        let body = format!("```json\n{{\"not\": 1}}\n```\n<!-- reviewhub:metrics -->\n```json\n{}\n```", metrics());
        assert!(extract(&body).unwrap().result.is_ok());
        assert!(extract("```json\n{}\n```").is_none());
    }

    #[test]
    fn reports_schema_and_json_errors() {
        let err = validate(Kind::Security, &json!({ "version": "2" })).unwrap_err();
        assert!(err.contains("scanner"), "{err}");
        let bad = extract("<!-- reviewhub:manifest -->\n```json\n{oops\n```").unwrap();
        assert!(bad.result.unwrap_err().starts_with("Invalid JSON"));
    }

    #[test]
    fn applies_schema_defaults() {
        let m = json!({ "version": "1", "agent": { "name": "a" }, "prompt": { "original": "p" }, "summary": "",
            "gaps": [{ "id": "g", "title": "t", "why": "w", "howToValidate": "h" }] });
        let Report::Manifest(m) = validate(Kind::Manifest, &m).unwrap() else { panic!() };
        assert_eq!(m.gaps[0].owner, "human");
        assert!(m.decisions.is_empty());
    }
}
