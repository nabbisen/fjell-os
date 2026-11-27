//! E-135: the `qemu-records` subcheck's file-reading and printing half. The judgement is
//! `fjell_consistency_check::qemu_records::evaluate`, in the library, where its tests are.

use fjell_consistency_check::qemu_records::{
    Inputs, ProfileInfo, Waiver, evaluate, parse_profile, parse_smoke_profiles, recorded_dir,
};
use fjell_repo_scope::Scope;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "qemu-records";

/// **Profiles that are `multi_node` and have no record, by name and reason.** The shape of `unsafe-audit`'s exemptions: hand-written,
/// a reason each, and a stale entry fails (the profile gained a record, stopped being `multi_node`, or went away).
///
/// `fleet-demo` is the only one, and the reason is checked against the tree rather than asserted: `crates/fjell-tools/src/fleet_demo.rs`
/// never calls `run_profile` or `ArtifactDir` (the two things that write the three files), its `up`/`verify` read and write
/// `tests/fleet-demo/logs/`, `test-all` does not run it (`SMOKE_PROFILES` and the derived categories both exclude it) and no workflow
/// in `.github/workflows/` names it. Its profile TOML used to say it *"runs as part of the QEMU smoke tier in `cargo xtask test-all`"*,
/// which was false; that sentence is corrected in the same change. **This is a waiver and not a fix on purpose:** recording the fleet
/// demo means running three QEMU instances through a runner that does not record, which is a design change to that runner and not a
/// condition in `write_summary`. It is in this list so that its absence is printed on every run of the gate, not a gap by construction.
const WAIVERS: &[Waiver<'static>] = &[Waiver {
    profile: "fleet-demo",
    reason: "multi_node: run by `cargo xtask fleet-demo` (crates/fjell-tools/src/fleet_demo.rs), which never goes through `run_profile` and so never \
             writes the three summary files; `test-all` does not run it and no workflow names it",
}];

pub fn check() -> ExitCode {
    let root = Path::new(".");
    let tracked = match fjell_repo_scope::files(root, Scope::Tracked) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot list the tracked files: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut profiles: Vec<ProfileInfo> = Vec::new();
    let mut recorded: BTreeSet<String> = BTreeSet::new();
    for f in &tracked {
        let rel = f.strip_prefix(root).unwrap_or(f).to_string_lossy().replace('\\', "/");
        if let Some(d) = recorded_dir(&rel) {
            recorded.insert(d.to_string());
        } else if rel.starts_with("tests/qemu/profiles/") && rel.ends_with(".toml") && rel.matches('/').count() == 3 {
            let Some(text) = crate::read_file(NAME, &rel) else {
                return ExitCode::FAILURE;
            };
            match parse_profile(&text) {
                Ok(p) => profiles.push(p),
                Err(e) => {
                    eprintln!("{NAME}: FAIL — {rel}: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    if profiles.is_empty() {
        eprintln!("{NAME}: FAIL — no tests/qemu/profiles/*.toml among the tracked files: a check over nothing is not a pass");
        return ExitCode::FAILURE;
    }
    let Some(test_all) = crate::read_file(NAME, "crates/fjell-tools/src/test_all.rs") else {
        return ExitCode::FAILURE;
    };
    let smoke = match parse_smoke_profiles(&test_all) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{NAME}: FAIL — {e}");
            return ExitCode::FAILURE;
        }
    };
    match evaluate(&Inputs {
        profiles: &profiles,
        smoke: &smoke,
        recorded: &recorded,
        waivers: WAIVERS,
    }) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} profile TOMLs, {} smoke milestones run by `test-all`, {} tracked records: every release-gated profile has its \
                 record ({} gated profile(s) + {} smoke); {} not-gated profile(s) with a record ({}) and {} without ({}); {} multi_node profile(s) \
                 WAIVED with a reason: {}. NOT covered: whether a record's content is current, which `test-all` rewrites in place and `git diff` shows)",
                r.profiles,
                smoke.len(),
                r.recorded_total,
                r.gated_recorded,
                r.smoke_recorded,
                r.not_gated_recorded.len(),
                r.not_gated_recorded.join(", "),
                r.not_gated_unrecorded.len(),
                r.not_gated_unrecorded.join(", "),
                r.waived.len(),
                r.waived
                    .iter()
                    .map(|(n, why)| format!("`{n}` ({why})"))
                    .collect::<Vec<_>>()
                    .join("; "),
            );
            ExitCode::SUCCESS
        }
        Err(problems) => {
            eprintln!("{NAME}: FAIL");
            for p in &problems {
                eprintln!("  {p}");
            }
            ExitCode::FAILURE
        }
    }
}
