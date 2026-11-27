//! RFC-0.50-001 D6: the `dead-tests` subcheck's file-reading half. The judgement is `fjell_consistency_check::dead_tests::evaluate`.

use fjell_consistency_check::dead_tests::{CrateInfo, evaluate};
use fjell_repo_scope::Scope;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

const NAME: &str = "dead-tests";

/// The crate directory a tracked path belongs to, if it is a bare-metal one -- by where it lives, as
/// `cargo_metadata::bare_metal_crate_names` derives it: `crates/fjell-kernel`, `crates/services/*`, `crates/drivers/*`.
fn bare_metal_dir(rel: &str) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.as_slice() {
        ["crates", "fjell-kernel", ..] => Some("crates/fjell-kernel".into()),
        ["crates", kind @ ("services" | "drivers"), name, ..] => Some(format!("crates/{kind}/{name}")),
        _ => None,
    }
}

pub fn check() -> ExitCode {
    let tracked = match fjell_repo_scope::files(Path::new("."), Scope::Tracked) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot list the tracked files: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &tracked {
        let rel = f.strip_prefix(".").unwrap_or(f).to_string_lossy().replace('\\', "/");
        let rel = rel.trim_start_matches("./").to_string();
        if let Some(dir) = bare_metal_dir(&rel) {
            let e = by_dir.entry(dir.clone()).or_default();
            if rel.starts_with(&format!("{dir}/src/")) && rel.ends_with(".rs") {
                e.push(rel);
            }
        }
    }
    let mut crates = Vec::new();
    for (dir, files) in by_dir {
        let manifest_path = format!("{dir}/Cargo.toml");
        let Some(manifest) = crate::read_file(NAME, &manifest_path) else {
            return ExitCode::FAILURE;
        };
        let mut sources = Vec::new();
        for f in files {
            let Some(text) = crate::read_file(NAME, &f) else {
                return ExitCode::FAILURE;
            };
            sources.push((f, text));
        }
        crates.push(CrateInfo { dir, manifest, sources });
    }
    match evaluate(&crates) {
        Ok(r) => {
            println!(
                "{NAME}: PASS ({} bare-metal crates (the kernel, the services, the drivers -- the set `cargo_metadata::bare_metal_crate_names` excludes from both host test runs), \
                 {} of them with a `[lib]` whose tests the host compiles; {} binary source files examined, none carries `#[test]` or `#[cfg(test)]`. NOT covered: a module only \
                 the binary reaches in a crate that also has a library, and comments)",
                r.crates, r.with_lib, r.files_examined
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
