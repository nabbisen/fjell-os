//! RFC-0.50-001 D6 (E-134): **a bare-metal crate's binary contains no test.**
//!
//! `cargo_metadata::bare_metal_crate_names` derives "bare-metal" from where a crate lives (`crates/fjell-kernel`, `crates/services/*`,
//! `crates/drivers/*`) and both host test runs `--exclude` every one, because a `no_std` binary with its own `panic_handler` cannot
//! link the host test harness (`duplicate lang item panic_impl`). **That exclusion is structural and silent**, so a `#[test]` written in
//! one of those binaries is written to a reader, not to a machine: the kernel carried 27 of them, over the user-pointer validator, the
//! lease table, the ready queue and the milestone markers, for as long as it had existed, and nothing could say so. The tests were
//! moved to crates the host *can* build (RFC-0.50-001); this is what stops the class re-forming.
//!
//! The rule, per bare-metal crate: **no `#[test]` and no `#[cfg(test)]` in the crate's binary sources.** A crate with no `[lib]`
//! has only binary sources (every file under `src/`); a crate with a `[lib]` (`fjell-proxy-text`, `fjell-driver-virtio-net`) has its
//! tests in the library, which the host does compile, so only the binary root (`src/main.rs`) and `src/bin/` are examined.
//!
//! **What it does not cover, and says so:** a module that only the *binary* reaches, in a crate that also has a library, is not
//! distinguished from a library module (that needs `mod` resolution, which this does not do); a `#[test]` inside a macro or a string.
//! Comment lines are skipped.

/// One bare-metal crate.
pub struct CrateInfo {
    /// `crates/fjell-kernel`, `crates/services/fjell-init`, ...
    pub dir: String,
    pub manifest: String,
    /// `(path relative to the repository, text)` of every tracked `.rs` file under `<dir>/src/`.
    pub sources: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct Report {
    pub crates: usize,
    pub with_lib: usize,
    pub files_examined: usize,
}

/// Does the manifest declare a `[lib]` target?
pub fn has_lib(manifest: &str) -> bool {
    manifest.lines().any(|l| l.trim() == "[lib]")
}

/// The binary-source files of `c`: every file when there is no `[lib]`, otherwise `src/main.rs` and `src/bin/**`.
fn binary_files<'a>(c: &'a CrateInfo) -> Vec<&'a (String, String)> {
    let lib = has_lib(&c.manifest);
    c.sources
        .iter()
        .filter(|(p, _)| {
            !lib || *p == format!("{}/src/main.rs", c.dir)
                || p.starts_with(&format!("{}/src/bin/", c.dir))
        })
        .collect()
}

/// Lines that carry a test attribute (a comment line is not one).
pub fn test_attributes(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .filter_map(|(i, l)| {
            let t = l.trim_start();
            (!t.starts_with("//") && (t.starts_with("#[test]") || t.starts_with("#[cfg(test)]")))
                .then_some((i + 1, t))
        })
        .collect()
}

pub fn evaluate(crates: &[CrateInfo]) -> Result<Report, Vec<String>> {
    if crates.is_empty() {
        return Err(vec![
            "no bare-metal crate found: a check over nothing is not a pass".into(),
        ]);
    }
    let mut problems = Vec::new();
    let mut files = 0usize;
    for c in crates {
        if c.sources.is_empty() {
            problems.push(format!("{}: no tracked `.rs` source under src/ (an unreadable crate is a failure, not a pass)", c.dir));
        }
        for (path, text) in binary_files(c) {
            files += 1;
            for (line, attr) in test_attributes(text) {
                problems.push(format!(
                    "{path}:{line}: `{attr}` in a bare-metal binary. This crate cannot compile a test on the host (a `no_std` binary with its own `panic_handler`; \
                     `cargo test` dies on `duplicate lang item`), so the test has never run and reports coverage it has not got (E-134). Move the logic it tests into a \
                     host-testable crate, as `fjell-user-ptr`, `fjell-lease` and `fjell-task` were"
                ));
            }
        }
    }
    if problems.is_empty() {
        Ok(Report {
            crates: crates.len(),
            with_lib: crates.iter().filter(|c| has_lib(&c.manifest)).count(),
            files_examined: files,
        })
    } else {
        Err(problems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(dir: &str, manifest: &str, files: &[(&str, &str)]) -> CrateInfo {
        CrateInfo {
            dir: dir.into(),
            manifest: manifest.into(),
            sources: files
                .iter()
                .map(|(p, t)| (format!("{dir}/src/{p}"), t.to_string()))
                .collect(),
        }
    }
    const BIN: &str = "[package]\nname = \"x\"\n[[bin]]\nname = \"x\"\npath = \"src/main.rs\"\n";
    const WITH_LIB: &str = "[package]\nname = \"y\"\n[lib]\npath = \"src/core/lib.rs\"\n[[bin]]\nname = \"y\"\npath = \"src/main.rs\"\n";

    #[test]
    fn a_bin_only_crate_with_no_test_passes_and_is_counted() {
        let c = [krate(
            "crates/fjell-kernel",
            BIN,
            &[
                ("main.rs", "fn main() {}\n"),
                ("lease/mod.rs", "fn a() {}\n"),
            ],
        )];
        let r = evaluate(&c).unwrap();
        assert_eq!((r.crates, r.with_lib, r.files_examined), (1, 0, 2));
    }

    /// The planted `#[test]`: the demonstration the ruling asked for.
    #[test]
    fn a_planted_test_in_a_bin_only_crate_is_red_wherever_it_sits() {
        let c = [krate(
            "crates/fjell-kernel",
            BIN,
            &[
                ("main.rs", "fn main() {}\n"),
                (
                    "mm/user_ptr.rs",
                    "fn v() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {}\n}\n",
                ),
            ],
        )];
        let err = evaluate(&c).unwrap_err();
        assert_eq!(err.len(), 2, "both attributes: {err:?}");
        assert!(
            err[0].contains("crates/fjell-kernel/src/mm/user_ptr.rs:3") && err[0].contains("E-134"),
            "{err:?}"
        );
    }

    #[test]
    fn in_a_crate_with_a_lib_only_the_binary_root_is_examined() {
        let tests_in_lib = "#[cfg(test)]\nmod t { #[test] fn a() {} }\n";
        let ok = [krate(
            "crates/drivers/fjell-driver-virtio-net",
            WITH_LIB,
            &[
                ("main.rs", "fn main() {}\n"),
                ("core/scan.rs", tests_in_lib),
            ],
        )];
        let r = evaluate(&ok).unwrap();
        assert_eq!(
            (r.with_lib, r.files_examined),
            (1, 1),
            "the lib's tests compile on the host and are not this check's"
        );
        let bad = [krate(
            "crates/drivers/fjell-driver-virtio-net",
            WITH_LIB,
            &[("main.rs", tests_in_lib), ("core/scan.rs", tests_in_lib)],
        )];
        let err = evaluate(&bad).unwrap_err();
        assert!(err.iter().all(|e| e.contains("src/main.rs")), "{err:?}");
    }

    #[test]
    fn a_comment_that_mentions_the_attribute_is_not_one() {
        assert!(
            test_attributes("// #[test] used to be here\n/// `#[cfg(test)]` too\nfn f() {}\n")
                .is_empty()
        );
        assert_eq!(test_attributes("    #[test]\n").len(), 1);
    }

    #[test]
    fn nothing_to_examine_is_a_failure_not_a_pass() {
        assert!(evaluate(&[]).is_err());
        let c = [krate("crates/services/fjell-x", BIN, &[])];
        assert!(evaluate(&c).unwrap_err()[0].contains("no tracked `.rs` source"));
    }
}
