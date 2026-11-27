//! E-135: **the profiles the release gates, and the records of them, are compared.**
//!
//! `.gitignore` and `qemu_run.rs::ArtifactDir` both say that three summary files -- `expected-markers.txt`, `qemu-command.txt`,
//! `result-summary.txt` -- are tracked for every profile `test-all` gates. For a long time nothing compared the set of gated profiles
//! with the set of recorded ones, so `fleet-demo` never had them and nothing noticed, and two profiles added at 0.49 stage 2
//! (`task-lifecycle`, `mmio-unmap`) could not record their own first copy because the writer waited for a file that only it could
//! create. What an `rfc-evidence` `marker:<profile>:<text>` citation resolves against is that recorded rendering, so a profile with
//! none cannot be cited by marker in its own RFC.
//!
//! **Three sets, not two** -- running the obvious two-set comparison (the 27 categories `qemu-negative` lists against the 29
//! recorded directories) fails on the two missing records and on four `smoke-*` directories that look like orphans, and **passes
//! over `fleet-demo`**, which is in neither set: it is `multi_node`, so it is excluded from the categories, and run by a different
//! runner. The sets are:
//!
//! 1. **the profile TOMLs** (`tests/qemu/profiles/*.toml`) -- every one is accounted for;
//! 2. **the smoke tier** (`test_all::SMOKE_PROFILES`) -- four names that `qemu_run::Profile::smoke` turns into `smoke-<name>`
//!    directories with no TOML behind them: *owned*, not orphaned;
//! 3. **the recorded directories** (tracked `tests/qemu/artifacts/<name>/expected-markers.txt`).
//!
//! What is required of each profile is a function of what it is:
//!
//! * **release-gated and single-node, or a smoke profile `test-all` runs** -- must have a record. A missing one fails, and says
//!   the remedy is to *run it* (the writer produces the files) and not to write them.
//! * **`release_gated = false`** (`store`) -- a record is optional: the profile runs and fails honestly, and `ArtifactDir` keeps a
//!   committed record refreshed. Its absence is not a failure; its presence is not an orphan.
//! * **`multi_node`** (`fleet-demo`) -- must have a record **or a named waiver with a reason**, so that its absence is *visible in
//!   the gate's own output* and not a gap by construction. A waiver for a profile that has since gained a record, is not
//!   `multi_node`, or does not exist is itself a failure -- the `unsafe-audit` exemption shape.
//! * **a recorded directory owned by nothing** -- an orphan, and a failure.

use std::collections::BTreeSet;

/// What the check needs to know about one `tests/qemu/profiles/*.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    pub name: String,
    pub multi_node: bool,
    pub release_gated: bool,
}

/// Read `name`, `multi_node` and `release_gated` from a profile TOML's top-level `key = value` lines.
///
/// Deliberately narrow, as `qemu_run::load_profile` is the real parser and this reads three scalar keys: a line is
/// `<key> = <value>` with the key at column 0, and `release_gated` defaults to `true` as the loader's does (RFC-0.29-001 D5).
/// A missing `name` is an error, not a guess from the file name.
pub fn parse_profile(toml: &str) -> Result<ProfileInfo, String> {
    let mut name: Option<String> = None;
    let mut multi_node = false;
    let mut release_gated = true;
    for line in toml.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.is_empty() || k.starts_with(char::is_whitespace) || k.starts_with('#') {
            continue; // an indented line is inside an array; only column-0 keys are top-level scalars here
        }
        let k = k.trim_end();
        let v = v.split('#').next().unwrap_or("").trim();
        match k {
            "name" => name = Some(v.trim_matches('"').to_string()),
            "multi_node" => multi_node = parse_bool(v, "multi_node")?,
            "release_gated" => release_gated = parse_bool(v, "release_gated")?,
            _ => {}
        }
    }
    let name = name.ok_or("no `name = \"...\"` line")?;
    if name.is_empty() {
        return Err("`name` is empty".into());
    }
    Ok(ProfileInfo {
        name,
        multi_node,
        release_gated,
    })
}

fn parse_bool(v: &str, key: &str) -> Result<bool, String> {
    match v {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("`{key} = {other}` is not true/false")),
    }
}

/// The names in `test_all.rs`'s `SMOKE_PROFILES` constant, read from its source.
pub fn parse_smoke_profiles(test_all_source: &str) -> Result<Vec<String>, String> {
    let marker = "const SMOKE_PROFILES: &[&str] = &[";
    let start = test_all_source
        .find(marker)
        .ok_or("`const SMOKE_PROFILES: &[&str] = &[` not found in crates/fjell-tools/src/test_all.rs")?
        + marker.len();
    let end = test_all_source[start..]
        .find("];")
        .ok_or("`SMOKE_PROFILES` has no closing `];`")?;
    let mut out = Vec::new();
    for tok in test_all_source[start..start + end].split(',') {
        let t = tok.trim();
        if t.is_empty() {
            continue;
        }
        let t = t
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .ok_or_else(|| format!("`SMOKE_PROFILES` entry `{t}` is not a plain string"))?;
        out.push(t.to_string());
    }
    if out.is_empty() {
        return Err("`SMOKE_PROFILES` is empty".into());
    }
    Ok(out)
}

/// The directory name under `tests/qemu/artifacts/` a tracked `.../expected-markers.txt` path belongs to.
pub fn recorded_dir(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("tests/qemu/artifacts/")?;
    let dir = rest.strip_suffix("/expected-markers.txt")?;
    (!dir.is_empty() && !dir.contains('/')).then_some(dir)
}

/// A `multi_node` profile with no record, accounted for by name and reason.
pub struct Waiver<'a> {
    pub profile: &'a str,
    pub reason: &'a str,
}

pub struct Inputs<'a> {
    pub profiles: &'a [ProfileInfo],
    pub smoke: &'a [String],
    pub recorded: &'a BTreeSet<String>,
    pub waivers: &'a [Waiver<'a>],
}

#[derive(Debug)]
pub struct Report {
    pub profiles: usize,
    pub gated_recorded: usize,
    pub smoke_recorded: usize,
    pub not_gated_recorded: Vec<String>,
    pub not_gated_unrecorded: Vec<String>,
    pub waived: Vec<(String, String)>,
    pub recorded_total: usize,
}

pub fn evaluate(i: &Inputs) -> Result<Report, Vec<String>> {
    let mut problems = Vec::new();
    let mut owned: BTreeSet<String> = BTreeSet::new();
    let mut gated_recorded = 0usize;
    let mut not_gated_recorded = Vec::new();
    let mut not_gated_unrecorded = Vec::new();
    let mut waived = Vec::new();
    let mut seen = BTreeSet::new();

    for p in i.profiles {
        if !seen.insert(p.name.as_str()) {
            problems.push(format!("two profiles are named `{}`", p.name));
        }
        owned.insert(p.name.clone());
        let has = i.recorded.contains(&p.name);
        let waiver = i.waivers.iter().find(|w| w.profile == p.name);
        if p.multi_node {
            match (has, waiver) {
                (true, Some(w)) => problems.push(format!(
                    "stale waiver for `{}`: it has a record now, so the waiver (\"{}\") is retired",
                    p.name, w.reason
                )),
                (true, None) => gated_recorded += 1,
                (false, Some(w)) => waived.push((p.name.clone(), w.reason.to_string())),
                (false, None) => problems.push(format!(
                    "`{}` is `multi_node` and has no recorded `expected-markers.txt` and no waiver. A multi-node profile is run by a different \
                     runner and was never in the categories `qemu-negative` lists, which is how one stayed unrecorded for many releases; \
                     record it, or waive it by name WITH a reason in the gate",
                    p.name
                )),
            }
            continue;
        }
        if let Some(w) = waiver {
            problems.push(format!(
                "stale waiver for `{}`: it is not `multi_node`, so there is nothing to waive (\"{}\")",
                p.name, w.reason
            ));
        }
        if p.release_gated {
            if has {
                gated_recorded += 1;
            } else {
                problems.push(format!(
                    "`{}` is release-gated and has no tracked tests/qemu/artifacts/{}/expected-markers.txt. Run it (`cargo run -p fjell-tools -- \
                     qemu-run --profile {}`): the writer produces the three files for a gated profile, and they are committed as written. Do not \
                     write them by hand -- that would make this check pass over the defect it exists to find (E-135)",
                    p.name, p.name, p.name
                ));
            }
        } else if has {
            not_gated_recorded.push(p.name.clone());
        } else {
            not_gated_unrecorded.push(p.name.clone());
        }
    }

    for w in i.waivers {
        if !i.profiles.iter().any(|p| p.name == w.profile) {
            problems.push(format!(
                "stale waiver for `{}`: no profile of that name exists any more",
                w.profile
            ));
        }
        if w.reason.trim().is_empty() {
            problems.push(format!(
                "waiver for `{}` has no reason: a waiver without one is a record, not an exception",
                w.profile
            ));
        }
    }

    let mut smoke_recorded = 0usize;
    for m in i.smoke {
        let dir = format!("smoke-{m}");
        if i.recorded.contains(&dir) {
            smoke_recorded += 1;
        } else {
            problems.push(format!(
                "`test-all` runs smoke milestone `{m}` and tests/qemu/artifacts/{dir}/expected-markers.txt is not tracked. Run `cargo run -p \
                 fjell-tools -- qemu-test {m}` and commit what it writes"
            ));
        }
        if owned.contains(&dir) {
            problems.push(format!(
                "`{dir}` is both a smoke directory and a profile name: two owners for one directory"
            ));
        }
        owned.insert(dir);
    }

    for d in i.recorded {
        if !owned.contains(d) {
            problems.push(format!(
                "tests/qemu/artifacts/{d}/expected-markers.txt is tracked and no profile and no smoke milestone `test-all` runs owns the name `{d}` -- an orphaned record"
            ));
        }
    }

    if problems.is_empty() {
        Ok(Report {
            profiles: i.profiles.len(),
            gated_recorded,
            smoke_recorded,
            not_gated_recorded,
            not_gated_unrecorded,
            waived,
            recorded_total: i.recorded.len(),
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, multi_node: bool, release_gated: bool) -> ProfileInfo {
        ProfileInfo {
            name: name.into(),
            multi_node,
            release_gated,
        }
    }
    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }
    fn smoke() -> Vec<String> {
        vec!["m8".into()]
    }
    const FLEET_WAIVER: Waiver<'static> = Waiver {
        profile: "fleet-demo",
        reason: "run by `cargo xtask fleet-demo`, which records nothing",
    };

    /// A consistent world: one gated profile with a record, one not-gated profile with one, the smoke tier, one waived fleet.
    fn world() -> (Vec<ProfileInfo>, Vec<String>, BTreeSet<String>) {
        (
            vec![
                p("audit", false, true),
                p("store", false, false),
                p("fleet-demo", true, true),
            ],
            smoke(),
            set(&["audit", "store", "smoke-m8"]),
        )
    }

    fn run(
        profiles: &[ProfileInfo],
        smoke: &[String],
        recorded: &BTreeSet<String>,
        waivers: &[Waiver],
    ) -> Result<Report, Vec<String>> {
        evaluate(&Inputs {
            profiles,
            smoke,
            recorded,
            waivers,
        })
    }

    #[test]
    fn a_consistent_world_passes_and_reports_the_waiver_by_name() {
        let (pr, sm, rec) = world();
        let r = run(&pr, &sm, &rec, &[FLEET_WAIVER]).unwrap();
        assert_eq!((r.profiles, r.gated_recorded, r.smoke_recorded), (3, 1, 1));
        assert_eq!(r.not_gated_recorded, vec!["store"]);
        assert_eq!(r.waived.len(), 1);
        assert_eq!(r.waived[0].0, "fleet-demo");
    }

    /// Direction 1: a gated profile with no record. The remedy named is to RUN it, not to write the files.
    #[test]
    fn a_gated_profile_without_a_record_fails_and_says_to_run_it() {
        let (mut pr, sm, rec) = world();
        pr.push(p("task-lifecycle", false, true));
        let err = run(&pr, &sm, &rec, &[FLEET_WAIVER]).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("`task-lifecycle` is release-gated")
                && e.contains("Run it")
                && e.contains("Do not write them by hand")),
            "{err:?}"
        );
    }

    /// Direction 2: a record owned by nothing is an orphan -- and the four smoke directories are NOT orphans.
    #[test]
    fn an_orphaned_record_fails_and_a_smoke_directory_is_not_one() {
        let (pr, sm, mut rec) = world();
        assert!(run(&pr, &sm, &rec, &[FLEET_WAIVER]).is_ok(), "smoke-m8 is owned by SMOKE_PROFILES");
        rec.insert("removed-profile".into());
        let err = run(&pr, &sm, &rec, &[FLEET_WAIVER]).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("`removed-profile`") && e.contains("orphaned")),
            "{err:?}"
        );
        assert!(!err.iter().any(|e| e.contains("smoke-m8")), "{err:?}");
    }

    /// Direction 3 -- the one the two-set comparison misses: a `multi_node` profile outside the categories, with no record.
    #[test]
    fn a_multi_node_profile_with_no_record_and_no_waiver_fails() {
        let (pr, sm, rec) = world();
        let err = run(&pr, &sm, &rec, &[]).unwrap_err();
        assert!(
            err.iter().any(|e| e.contains("`fleet-demo` is `multi_node`") && e.contains("no waiver")),
            "{err:?}"
        );
    }

    #[test]
    fn a_waiver_with_no_reason_or_for_nothing_or_gone_stale_fails() {
        let (pr, sm, rec) = world();
        // no reason
        let w = [Waiver { profile: "fleet-demo", reason: " " }];
        assert!(run(&pr, &sm, &rec, &w).unwrap_err().iter().any(|e| e.contains("has no reason")));
        // no such profile
        let w = [FLEET_WAIVER, Waiver { profile: "ghost", reason: "x" }];
        assert!(run(&pr, &sm, &rec, &w).unwrap_err().iter().any(|e| e.contains("no profile of that name")));
        // the profile has a record now: the waiver is retired
        let mut rec2 = rec.clone();
        rec2.insert("fleet-demo".into());
        assert!(run(&pr, &sm, &rec2, &[FLEET_WAIVER]).unwrap_err().iter().any(|e| e.contains("stale waiver") && e.contains("record now")));
        // waiving a profile that is not multi_node
        let w = [FLEET_WAIVER, Waiver { profile: "audit", reason: "x" }];
        assert!(run(&pr, &sm, &rec, &w).unwrap_err().iter().any(|e| e.contains("not `multi_node`")));
    }

    #[test]
    fn a_not_gated_profile_may_have_a_record_or_none() {
        let (pr, sm, mut rec) = world();
        rec.remove("store");
        let r = run(&pr, &sm, &rec, &[FLEET_WAIVER]).unwrap();
        assert_eq!(r.not_gated_unrecorded, vec!["store"]);
    }

    #[test]
    fn a_smoke_milestone_test_all_runs_must_be_recorded() {
        let (pr, _, rec) = world();
        let sm = vec!["m8".to_string(), "v9".to_string()];
        let err = run(&pr, &sm, &rec, &[FLEET_WAIVER]).unwrap_err();
        assert!(err.iter().any(|e| e.contains("smoke milestone `v9`")), "{err:?}");
    }

    #[test]
    fn parse_profile_reads_the_three_keys_and_defaults_release_gated_true() {
        let t = "# c\nname = \"fleet-demo\"\ntimeout_secs = 120\nmulti_node = true\nexpected_markers = [\n    \"a\",\n]\n";
        let i = parse_profile(t).unwrap();
        assert_eq!((i.name.as_str(), i.multi_node, i.release_gated), ("fleet-demo", true, true));
        let t = "name = \"store\"\nrelease_gated = false # why\nnot_gated_reason = \"x\"\n";
        let i = parse_profile(t).unwrap();
        assert_eq!((i.multi_node, i.release_gated), (false, false));
        assert!(parse_profile("timeout_secs = 1\n").is_err(), "no name is an error, not a guess");
        assert!(parse_profile("name = \"x\"\nrelease_gated = maybe\n").is_err());
    }

    #[test]
    fn parse_smoke_profiles_reads_the_constant() {
        let src = "/// c\npub(crate) const SMOKE_PROFILES: &[&str] = &[\"m8\", \"v0.4-net\"];\nconst X: u8 = 1;\n";
        assert_eq!(parse_smoke_profiles(src).unwrap(), vec!["m8", "v0.4-net"]);
        assert!(parse_smoke_profiles("nothing here").is_err());
        assert!(parse_smoke_profiles("const SMOKE_PROFILES: &[&str] = &[];").is_err(), "an empty list is a failure");
    }

    #[test]
    fn recorded_dir_reads_only_the_flat_marker_files() {
        assert_eq!(recorded_dir("tests/qemu/artifacts/audit/expected-markers.txt"), Some("audit"));
        assert_eq!(recorded_dir("tests/qemu/artifacts/smoke-m8/expected-markers.txt"), Some("smoke-m8"));
        assert_eq!(recorded_dir("tests/qemu/artifacts/audit/runs/x/expected-markers.txt"), None);
        assert_eq!(recorded_dir("tests/qemu/artifacts/audit/serial.log"), None);
    }
}
