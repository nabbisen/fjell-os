//! Gate 1's comparison: every committed `.frozen` file is what its encoder
//! generates, and a format cannot be added without one (RFC-0.33-003 D1, D2, D6).
//!
//! Run by `cargo test --workspace`, i.e. by Gate 1, `test-all` and CI alike.

use fjell_schema::registry::{EXCLUSIONS, Exclusion, FORMATS};
use fjell_schema::{compare, generate};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if p.is_dir() {
            if name == "target" || name == ".git" || name == ".git-exclude" || name == "runs" {
                continue;
            }
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

// ── The comparison ───────────────────────────────────────────────────────────

#[test]
fn every_committed_file_is_what_its_encoder_generates() {
    let mut bad = Vec::new();
    for f in FORMATS {
        let committed = std::fs::read_to_string(root().join(f.path)).unwrap_or_else(|_| {
            panic!(
                "{}: the registry says this file exists and it does not",
                f.path
            )
        });
        let generated = generate(f).unwrap_or_else(|p| panic!("{}: {p:?}", f.id));
        let drift = compare(&generated, &committed);
        if !drift.is_empty() {
            bad.push(format!(
                "{} drifts from its encoder:\n  {}",
                f.path,
                drift
                    .iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            ));
        }
    }
    assert!(
        bad.is_empty(),
        "\n{}\n\nA struct's bytes changed without its schema file, or the file was edited by hand. \
         Run `cargo xtask schema dump`, read the diff, and bump the format's version if bytes moved.\n",
        bad.join("\n")
    );
}

#[test]
fn generation_is_deterministic() {
    for f in FORMATS {
        assert_eq!(generate(f), generate(f), "{}: two runs differ", f.id);
    }
}

#[test]
fn a_generated_file_generates_the_same_text_again() {
    // The committed file *is* the generated text, byte for byte — so `dump` on a
    // tree that has not changed writes nothing.
    for f in FORMATS {
        let committed = std::fs::read_to_string(root().join(f.path)).unwrap();
        assert_eq!(generate(f).unwrap(), committed, "{}", f.path);
    }
}

// ── Coverage (D6) ────────────────────────────────────────────────────────────

#[test]
fn registry_entries_are_consistent() {
    let mut ids = std::collections::BTreeSet::new();
    let mut paths = std::collections::BTreeSet::new();
    for f in FORMATS {
        assert!(ids.insert(f.id), "duplicate id {}", f.id);
        assert!(paths.insert(f.path), "duplicate path {}", f.path);
        assert!(
            f.path.ends_with(&format!("schema/{}.frozen", f.id)),
            "{}: the path must be <crate>/schema/<id>.frozen",
            f.path
        );
        assert!(
            f.path.contains(f.krate),
            "{}: the file must live in the crate the registry names ({})",
            f.path,
            f.krate
        );
    }
}

#[test]
fn no_frozen_file_exists_that_the_registry_does_not_generate() {
    let mut files = Vec::new();
    walk(&root().join("crates"), &mut files);
    let known: std::collections::BTreeSet<PathBuf> =
        FORMATS.iter().map(|f| root().join(f.path)).collect();
    let orphans: Vec<_> = files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "frozen"))
        .filter(|p| {
            !known
                .iter()
                .any(|k| k.canonicalize().ok() == p.canonicalize().ok())
        })
        .collect();
    assert!(
        orphans.is_empty(),
        "hand-written `.frozen` file(s) the registry does not generate: {orphans:?}"
    );
}

/// Every crate under `crates/formats/`, by directory name.
fn format_crates() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join("crates/formats"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn every_format_crate_has_a_file_or_a_stated_reason() {
    let mut missing = Vec::new();
    for c in format_crates() {
        let has_file = FORMATS.iter().any(|f| f.krate == c);
        let excused = EXCLUSIONS.iter().any(|(k, _)| *k == c);
        if has_file == excused {
            missing.push(format!(
                "{c}: {}",
                if has_file {
                    "has a generated file AND an exclusion — one of them is wrong"
                } else {
                    "has neither a generated file nor a stated reason"
                }
            ));
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
    for (k, _) in EXCLUSIONS {
        assert!(
            format_crates().iter().any(|c| c == k),
            "exclusion for `{k}`, which is not a crate under crates/formats/"
        );
    }
}

/// The fixed strings that mean "this crate turns a value into bytes". A tripwire,
/// not a proof: a producer spelt some other way would slip past it, which is why
/// the *positive* control below runs it against crates known to produce bytes.
const PRODUCERS: &[&str] = &[
    "to_le_bytes",
    "to_be_bytes",
    "from_le_bytes",
    "from_be_bytes",
    "Digest32::of",
    "of_parts",
    "repr(C",
    "fn encode",
    "fn decode",
    "as_bytes",
    "fn to_bytes",
    "fn from_bytes",
    // Split so a probe for the raw-view sites (E-046, E-055) does not find this list.
    concat!("from_raw_", "parts"),
];

fn producers_in(krate: &str) -> Vec<String> {
    let mut files = Vec::new();
    walk(
        &root().join("crates/formats").join(krate).join("src"),
        &mut files,
    );
    let mut hits = Vec::new();
    for f in files
        .iter()
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
    {
        let text = std::fs::read_to_string(f).unwrap();
        for p in PRODUCERS {
            if text.contains(p) {
                hits.push(format!(
                    "{}: `{p}`",
                    f.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    hits
}

#[test]
fn an_in_memory_only_reason_is_still_true() {
    for (k, e) in EXCLUSIONS {
        if let Exclusion::InMemoryOnly = e {
            let hits = producers_in(k);
            assert!(
                hits.is_empty(),
                "`{k}` is excused as in-memory only but now contains a byte producer ({hits:?}); \
                 give it a generated file or a truer reason"
            );
        }
    }
}

#[test]
fn the_in_memory_probe_finds_a_producer_where_one_exists() {
    // The control: the probe that says "no producer" must be able to say
    // "producer" about a crate that plainly has one.
    for known in [
        "fjell-fleet-format",
        "fjell-store-format",
        "fjell-audit-format",
    ] {
        assert!(
            !producers_in(known).is_empty(),
            "the probe is blind to {known}"
        );
    }
}

#[test]
fn a_survivor_is_named_in_the_register() {
    let register = std::fs::read_to_string(root().join("rfcs/ERRATA.md")).unwrap();
    for (k, e) in EXCLUSIONS {
        if let Exclusion::Survivor { erratum, what } = e {
            assert!(
                register.contains(&format!("## {erratum} ")),
                "`{k}` names {erratum} as the entry that tracks it ({what}), and the register has no such entry"
            );
            assert!(
                !producers_in(k).is_empty(),
                "`{k}` is a survivor because it produces bytes, and the probe finds none: reclassify it"
            );
        }
    }
}

// ── Controls: the comparison names what changed ──────────────────────────────

fn rollback() -> (String, &'static fjell_schema::registry::Format) {
    let f = FORMATS
        .iter()
        .find(|f| f.id == "rollback-record-v1")
        .unwrap();
    (generate(f).unwrap(), f)
}

#[test]
fn control_an_unchanged_file_reports_nothing() {
    let (g, _) = rollback();
    assert!(compare(&g, &g).is_empty());
}

#[test]
fn control_a_changed_width_is_named() {
    let (g, _) = rollback();
    let drifted = g.replace("field min_counter", "field min_counter").replace(
        "min_counter                u64 LE",
        "min_counter                u32 LE",
    );
    assert_ne!(g, drifted, "the control did not change anything");
    let d = compare(&g, &drifted);
    assert!(
        d.iter().any(|d| d.to_string().contains("field min_counter")
            && d.to_string().contains("u32 LE")
            && d.to_string().contains("u64 LE")),
        "{d:?}"
    );
}

#[test]
fn control_a_removed_and_an_added_field_are_named() {
    let (g, _) = rollback();
    let without: String = g
        .lines()
        .filter(|l| !l.starts_with("field last_advance_source"))
        .map(|l| format!("{l}\n"))
        .collect();
    let d = compare(&g, &without);
    assert!(
        d.iter()
            .any(|d| d.to_string().contains("field last_advance_source")
                && d.to_string().contains("absent from the committed file")),
        "{d:?}"
    );
    let with_extra = format!("{}field invented u8\n", without.trim_end_matches("end\n"));
    let d = compare(&g, &format!("{with_extra}end\n"));
    assert!(
        d.iter().any(|d| d.to_string().contains("field invented")
            && d.to_string().contains("does not write it")),
        "{d:?}"
    );
}

#[test]
fn control_a_reordering_is_reported_even_though_every_field_is_present() {
    let (g, _) = rollback();
    let mut lines: Vec<&str> = g.lines().collect();
    let a = lines
        .iter()
        .position(|l| l.starts_with("field channel_id"))
        .unwrap();
    lines.swap(a, a + 1);
    let swapped: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let d = compare(&g, &swapped);
    assert!(
        d.iter().any(|d| d.to_string().contains("order differs")),
        "a swap of two fields went unreported: {d:?}"
    );
}
