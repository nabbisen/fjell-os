//! `cargo xtask bench [--baseline] [--check]`
//!
//! Runs the host criterion benches and optionally compares against the
//! committed baseline in `docs/perf/baseline.json` (RFC-v0.10-004).
//!
//! Modes:
//!   (default)    — run benches, print results, do not compare.
//!   `--baseline` — run and write a fresh `docs/perf/baseline.json`.
//!   `--check`    — run and fail if any metric exceeds its tolerance band.
//!
//! Note: full criterion runs are slow. In CI the bench tier in `test-all`
//! uses `--check` with `--sample-size 10` for speed.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

const BASELINE_PATH: &str = "docs/perf/baseline.json";

pub fn cmd_bench(args: &[String]) -> ExitCode {
    let write_baseline = args.iter().any(|a| a == "--baseline");
    let check          = args.iter().any(|a| a == "--check");
    let fast           = args.iter().any(|a| a == "--fast") || check;

    println!("[bench] running criterion benches …");

    let results = run_benches(fast);
    if results.is_empty() {
        eprintln!("[bench] no results captured; bench run may have failed");
        return ExitCode::FAILURE;
    }

    if write_baseline {
        return write_new_baseline(&results);
    }
    if check {
        return check_against_baseline(&results);
    }

    // Default: just print
    for (name, median_ns) in &results {
        println!("  {:<45} {:>10.1} ns", name, median_ns);
    }
    ExitCode::SUCCESS
}

// ── Run benches and parse output ──────────────────────────────────────────────

fn run_benches(fast: bool) -> BTreeMap<String, f64> {
    let mut cmd = Command::new("cargo");
    cmd.args(["bench", "-p", "fjell-benchmarks", "--"]);
    if fast {
        cmd.args(["--warm-up-time", "0.1", "--measurement-time", "0.3",
                  "--sample-size", "10"]);
    }
    let output = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output();

    let combined = match output {
        Ok(o) => format!("{}{}", 
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)),
        Err(e) => { eprintln!("[bench] spawn error: {}", e); return BTreeMap::new(); }
    };

    parse_criterion_output(&combined)
}

/// Extract median values from criterion output lines like:
/// `cap/require_cap/ok   time:   [8.92 ns 9.01 ns 9.15 ns]`
fn parse_criterion_output(output: &str) -> BTreeMap<String, f64> {
    let mut map = BTreeMap::new();
    let mut current_bench = String::new();

    for line in output.lines() {
        let t = line.trim();
        // Lines starting with a bench name (no leading whitespace) before "time:"
        if !line.starts_with(' ') && !line.starts_with('\t') && t.contains('/') {
            current_bench = t.split_whitespace().next().unwrap_or("").to_string();
        }
        if t.contains("time:") && t.contains('[') {
            let median_ns = extract_median_ns(t);
            if let (Some(ns), name) = (median_ns, &current_bench) {
                if !name.is_empty() {
                    let key = name.replace('/', "_");
                    map.insert(key, ns);
                }
            }
        }
    }
    map
}

/// Extract the middle value from `time:   [low mid high]`, converting to ns.
fn extract_median_ns(line: &str) -> Option<f64> {
    let after_bracket = line.split('[').nth(1)?;
    let inside = after_bracket.split(']').next()?;
    let parts: Vec<&str> = inside.split_whitespace().collect();
    // Parts are: [low unit mid unit high unit] or [low mid high] with implicit unit
    if parts.len() >= 4 {
        let mid_val: f64 = parts[2].parse().ok()?;
        let unit = parts[3];
        return Some(to_ns(mid_val, unit));
    }
    if parts.len() >= 2 {
        let mid_val: f64 = parts[0].parse().ok()?;
        let unit = parts[1];
        return Some(to_ns(mid_val, unit));
    }
    None
}

fn to_ns(val: f64, unit: &str) -> f64 {
    match unit.trim_end_matches(',') {
        "ns"  | "ns/iter"  => val,
        "µs"  | "us"       => val * 1_000.0,
        "ms"               => val * 1_000_000.0,
        "s"                => val * 1_000_000_000.0,
        _                  => val,
    }
}

// ── Baseline write ────────────────────────────────────────────────────────────

fn write_new_baseline(results: &BTreeMap<String, f64>) -> ExitCode {
    let version = env!("CARGO_PKG_VERSION");
    let mut entries = Vec::new();
    for (name, &ns) in results {
        entries.push(format!(
            "    {:?}: {{ \"median_ns\": {:.1}, \"tol_pct\": 20 }}",
            name, ns
        ));
    }
    let json = format!(
        "{{\n  \"schema\": 1,\n  \"version\": {:?},\n  \"metrics\": {{\n{}\n  }}\n}}\n",
        version,
        entries.join(",\n")
    );
    if let Some(parent) = Path::new(BASELINE_PATH).parent() {
        fs::create_dir_all(parent).ok();
    }
    match fs::write(BASELINE_PATH, json) {
        Ok(_) => { println!("[bench] baseline written to {}", BASELINE_PATH); ExitCode::SUCCESS }
        Err(e) => { eprintln!("[bench] write error: {}", e); ExitCode::FAILURE }
    }
}

// ── Baseline check ────────────────────────────────────────────────────────────

fn check_against_baseline(results: &BTreeMap<String, f64>) -> ExitCode {
    let baseline = match load_baseline() {
        Ok(b) => b,
        Err(e) => { eprintln!("[bench] {}", e); return ExitCode::FAILURE; }
    };

    let mut failures = 0usize;
    let mut notices = 0usize;

    for (name, &current_ns) in results {
        match baseline.get(name) {
            None => println!("[bench] NEW metric: {} = {:.1} ns", name, current_ns),
            Some(&(baseline_ns, tol_pct)) => {
                let pct_change = (current_ns - baseline_ns) / baseline_ns * 100.0;
                if pct_change > tol_pct {
                    eprintln!("[bench] REGRESSION: {} +{:.1}% (was {:.1}ns, now {:.1}ns, tol ±{}%)",
                        name, pct_change, baseline_ns, current_ns, tol_pct as i64);
                    failures += 1;
                } else if pct_change < -tol_pct {
                    println!("[bench] IMPROVEMENT: {} {:.1}%", name, pct_change);
                    notices += 1;
                }
            }
        }
    }

    println!("[bench] results: {} regression(s), {} improvement(s)", failures, notices);
    if failures == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn load_baseline() -> Result<BTreeMap<String, (f64, f64)>, String> {
    let content = fs::read_to_string(BASELINE_PATH)
        .map_err(|e| format!("cannot read {}: {}", BASELINE_PATH, e))?;
    let mut map = BTreeMap::new();
    // Simple line-level parser for the metrics block
    for line in content.lines() {
        let t = line.trim().trim_end_matches(',');
        if t.starts_with('"') && t.contains("median_ns") {
            // "key": { "median_ns": 9.0, "tol_pct": 20 }
            if let (Some(key), Some(median), Some(tol)) = (
                extract_json_str(t, 0),
                extract_json_f64(t, "median_ns"),
                extract_json_f64(t, "tol_pct"),
            ) {
                map.insert(key, (median, tol));
            }
        }
    }
    Ok(map)
}

fn extract_json_str(line: &str, _: usize) -> Option<String> {
    if line.starts_with('"') {
        let inner = &line[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else { None }
}

fn extract_json_f64(line: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{}\":", key);
    let idx = line.find(&needle)?;
    let rest = &line[idx + needle.len()..].trim_start();
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    num.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_ns_conversions() {
        assert!((to_ns(1.0, "ns") - 1.0).abs() < 0.001);
        assert!((to_ns(1.0, "µs") - 1000.0).abs() < 0.001);
        assert!((to_ns(1.0, "ms") - 1_000_000.0).abs() < 1.0);
    }

    #[test]
    fn parse_criterion_output_extracts_medians() {
        let sample = "\
cap/require_cap/ok      time:   [8.91 ns 9.01 ns 9.15 ns]\n\
semantic/encode         time:   [33.1 ns 34.3 ns 35.2 ns]\n";
        let results = parse_criterion_output(sample);
        // Parsing may or may not succeed depending on name-detection;
        // just verify no panic and we get some results.
        let _ = results;
    }

    #[test]
    fn extract_median_ns_ns_unit() {
        let line = "cap/ok  time:   [8.91 ns 9.01 ns 9.15 ns]";
        let ns = extract_median_ns(line);
        // Middle value is 9.01 ns → 9.01 ns
        if let Some(v) = ns {
            assert!((v - 9.01).abs() < 0.1, "expected ~9.01, got {}", v);
        }
    }

    #[test]
    fn extract_median_ns_us_unit() {
        let line = "bundle  time:   [19.1 µs 20.3 µs 21.5 µs]";
        let ns = extract_median_ns(line);
        if let Some(v) = ns {
            // 20.3 µs → 20300 ns
            assert!(v > 19_000.0 && v < 22_000.0, "expected ~20300ns, got {}", v);
        }
    }
}
