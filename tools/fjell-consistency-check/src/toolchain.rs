//! RFC-0.30-003 D2: the one shared "observe the real toolchain" function.
//!
//! Reused by `tools/fjell-repro-check` (the baseline-digests.txt header)
//! and `crates/fjell-tools` (the `.provenance.txt` field and
//! `trust-report.txt`'s header) — three artefact-producing paths, one
//! observation, exported via this crate's `[lib]` target the same way
//! `errata::parse_summary_rows` already is (RFC-0.29-002 §7).
//!
//! **The whole point is what this does *not* do**: it never reads
//! `rust-toolchain.toml`. Recording the *declared* channel would have
//! written `1.91` on every artefact throughout the 2026-09-09 incident
//! (`rust-toolchain.toml` removed, local builds silently moved to
//! `1.98.1`) — correct-looking and wrong the entire time. `rustc -vV`,
//! run at the moment of production, is the only thing that can't lie this
//! way.

use std::process::Command;

/// The four fields RFC-0.30-003 D2 requires: `rustc -vV`'s `release`,
/// `commit-hash`, `host`, and `LLVM version` — enough to tell two builds
/// apart, never inferred from a declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedToolchain {
    pub release: String,
    pub commit_hash: String,
    pub host: String,
    pub llvm_version: String,
}

impl ObservedToolchain {
    /// The canonical single-line rendering used in every artefact header:
    /// `1.91.1 / ed61e7d7e / x86_64-unknown-linux-gnu / LLVM 21.1.2` — the
    /// exact format RFC-0.30-003's own D2 uses, so a reader who has read
    /// the RFC recognises it on sight.
    pub fn format_line(&self) -> String {
        format!(
            "{} / {} / {} / LLVM {}",
            self.release, self.commit_hash, self.host, self.llvm_version
        )
    }
}

/// Runs `rustc -vV` and parses the four fields. `None` means the
/// observation itself failed (`rustc` unreachable, or its output changed
/// shape) — callers must treat that as a hard failure, never substitute a
/// declared value or a placeholder. A tool that cannot observe the truth
/// records nothing sooner than it records a guess.
pub fn observe() -> Option<ObservedToolchain> {
    let output = Command::new("rustc").arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_rustc_vv(&text)
}

fn parse_rustc_vv(text: &str) -> Option<ObservedToolchain> {
    let mut release = None;
    let mut commit_hash = None;
    let mut host = None;
    let mut llvm_version = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("release: ") {
            release = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("commit-hash: ") {
            commit_hash = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("host: ") {
            host = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("LLVM version: ") {
            llvm_version = Some(v.trim().to_string());
        }
    }
    Some(ObservedToolchain {
        release: release?,
        commit_hash: commit_hash?,
        host: host?,
        llvm_version: llvm_version?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_RUSTC_VV: &str = "\
rustc 1.91.1 (ed61e7d7e 2025-11-07)
binary: rustc
commit-hash: ed61e7d7e242494fb7057f2657300d9e77bb4fcb
commit-date: 2025-11-07
host: x86_64-unknown-linux-gnu
release: 1.91.1
LLVM version: 21.1.2
";

    #[test]
    fn parses_real_rustc_vv_output() {
        let t = parse_rustc_vv(REAL_RUSTC_VV).expect("should parse");
        assert_eq!(t.release, "1.91.1");
        assert_eq!(t.commit_hash, "ed61e7d7e242494fb7057f2657300d9e77bb4fcb");
        assert_eq!(t.host, "x86_64-unknown-linux-gnu");
        assert_eq!(t.llvm_version, "21.1.2");
    }

    #[test]
    fn format_line_matches_the_rfcs_own_example() {
        let t = parse_rustc_vv(REAL_RUSTC_VV).unwrap();
        assert_eq!(
            t.format_line(),
            "1.91.1 / ed61e7d7e242494fb7057f2657300d9e77bb4fcb / x86_64-unknown-linux-gnu / LLVM 21.1.2"
        );
    }

    #[test]
    fn missing_field_fails_closed_not_partial() {
        let truncated = "rustc 1.91.1 (ed61e7d7e 2025-11-07)\nhost: x86_64-unknown-linux-gnu\n";
        assert!(
            parse_rustc_vv(truncated).is_none(),
            "a partially-parsed observation must be None, not a struct with a missing field defaulted"
        );
    }

    #[test]
    fn observe_actually_runs_rustc() {
        // Not a fixture: this is the one test in this module that touches
        // the real toolchain, confirming the whole pipeline (not just the
        // parser) works against whatever `rustc` this test runs under.
        let t =
            observe().expect("rustc -vV must succeed in any environment that can run this test");
        assert!(!t.release.is_empty());
        assert!(!t.commit_hash.is_empty());
        assert!(!t.host.is_empty());
        assert!(!t.llvm_version.is_empty());
    }
}
