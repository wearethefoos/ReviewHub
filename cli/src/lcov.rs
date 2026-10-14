//! lcov → per-file coverage. Port of `src/lib/metrics/lcov.ts`.

use crate::model::FileCoverage;

fn round1(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

fn pct(hit: u32, found: u32) -> f64 {
    if found == 0 { 100.0 } else { round1(f64::from(hit) / f64::from(found) * 100.0) }
}

/// Parses an lcov report. Paths under `root` are made relative to it so they match the PR's paths.
pub fn parse_lcov(text: &str, root: &str) -> (Vec<FileCoverage>, f64) {
    let prefix = if root.is_empty() || root.ends_with('/') { root.to_string() } else { format!("{root}/") };
    let mut files = Vec::new();
    let (mut total_found, mut total_hit) = (0u32, 0u32);

    struct Current {
        path: String,
        found: u32,
        hit: u32,
        uncovered: Vec<u32>,
        br_found: u32,
        br_hit: u32,
    }
    let mut current: Option<Current> = None;

    for line in text.lines().map(str::trim) {
        if let Some(path) = line.strip_prefix("SF:") {
            let path = if !prefix.is_empty() { path.strip_prefix(&prefix).unwrap_or(path) } else { path };
            let path = path.strip_prefix("./").unwrap_or(path).to_string();
            current = Some(Current { path, found: 0, hit: 0, uncovered: Vec::new(), br_found: 0, br_hit: 0 });
            continue;
        }
        let Some(cur) = current.as_mut() else { continue };
        if let Some(rest) = line.strip_prefix("DA:") {
            let mut parts = rest.split(',');
            let line_no: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let hits: f64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
            cur.found += 1;
            if hits > 0.0 {
                cur.hit += 1;
            } else if line_no > 0 {
                cur.uncovered.push(line_no);
            }
        } else if let Some(n) = line.strip_prefix("BRF:") {
            cur.br_found = n.parse().unwrap_or(0);
        } else if let Some(n) = line.strip_prefix("BRH:") {
            cur.br_hit = n.parse().unwrap_or(0);
        } else if line == "end_of_record" {
            let cur = current.take().unwrap();
            total_found += cur.found;
            total_hit += cur.hit;
            files.push(FileCoverage {
                path: cur.path,
                lines: pct(cur.hit, cur.found),
                branches: (cur.br_found > 0).then(|| pct(cur.br_hit, cur.br_found)),
                uncovered_lines: cur.uncovered,
            });
        }
    }
    (files, pct(total_hit, total_found))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_coverage() {
        let lcov = "TN:\nSF:/repo/src/a.ts\nDA:1,1\nDA:2,0\nDA:3,4\nDA:4,0\nBRF:2\nBRH:1\nend_of_record\nSF:src/b.ts\nDA:1,1\nend_of_record\n";
        let (files, overall) = parse_lcov(lcov, "/repo");
        assert_eq!(files[0].path, "src/a.ts");
        assert_eq!((files[0].lines, files[0].branches), (50.0, Some(50.0)));
        assert_eq!(files[0].uncovered_lines, vec![2, 4]);
        assert_eq!((files[1].lines, files[1].branches), (100.0, None));
        assert_eq!(overall, 60.0);
    }
}
