//! `repo-scope` -- no tool answers "what counts as this repository" from a hand-written name list
//! (0.46 small item B, closes E-125).
//!
//! E-025 named the family -- a tool deciding the scope of its scan from a literal list of directory
//! names -- fixed its two enumerated instances, and closed with nothing left behind to find the next
//! member. `fjell-mmio-audit` then carried it for roughly seventeen milestones inside a release gate,
//! reporting a count that was ~91% other people's trees (E-125). The scope question is now answered once,
//! in `tools/fjell-repo-scope` (everything git considers part of the working copy), and this subcheck is
//! the gate that keeps it so.
//!
//! # How it avoids becoming its own E-014
//!
//! A check that greps four known files for `ls-files` is a hand-maintained name list wearing a
//! different hat. This one **derives its own population**: it asks `fjell-repo-scope` for every
//! Rust source in the repository, and examines each. A file is *scope-relevant* if (outside comments)
//! it shells out to `git ls-files`, depends on a directory-walking crate, or pairs `read_dir` with
//! `is_dir` (the signature of a recursive walk). Every scope-relevant file must be one of:
//!
//! 1. **a user of the shared scope** -- it names `fjell_repo_scope::`; or
//! 2. **classified**: listed in [`CLASSIFIED`] with a reason it walks a *named subtree or a fixed
//!    folder* rather than answering "the repository's files".
//!
//! **A scope-relevant file that is neither is a FAILURE**, not a skip: a new tool that walks the tree
//! by itself cannot pass by being unknown. A classified entry whose file no longer exists, or no longer
//! walks, is also a failure -- the list is not a place to park a name.
//!
//! # What this does not do, and the honest cost of the list
//!
//! * The classification table is hand-written. It is *forced to be complete* (an unlisted walker
//!   fails) and *forced to stay true* (a stale entry fails), but each entry's **reason is a judgement
//!   a reviewer reads**; the check cannot verify that "walks `crates/`" is a good reason.
//! * It is textual. A walk spelled in a way the signature does not match (a recursive function over
//!   `std::fs::metadata`, a shell-out to `find`) is invisible. Its detection of the *forbidden
//!   dependency* (`walkdir`, `ignore`, `glob`) is by manifest, which is exact.
//! * It says nothing about whether a classified walk is *correct*. Two of them (`callsite_audit`,
//!   `lint_allow_report`) walk `crates/` with no exclusion at all and would count an untracked,
//!   git-ignored directory placed there; they are recorded as such, not migrated.
//!
//! Needles are assembled at run time so this file does not flag itself.

use std::process::ExitCode;

const NAME: &str = "repo-scope";

/// A scope-relevant file that does not use the shared scope, and why that is acceptable.
pub struct Classified {
    pub path: &'static str,
    pub reason: &'static str,
}

/// Files that list or walk a **named subtree or fixed folder**, not "the repository". Each is
/// verified to still be scope-relevant on every run (a stale entry fails).
pub const CLASSIFIED: &[Classified] = &[
    Classified {
        path: "crates/fjell-schema/tests/frozen.rs",
        reason: "uses the shared scope for its repository walk; the remaining `read_dir` lists the \
                 immediate sub-folders of the fixed folder `crates/formats` (`is_dir` is the per-entry filter)",
    },
    Classified {
        path: "crates/fjell-tools/src/callsite_audit.rs",
        reason: "walks `crates/` for Rust with NO exclusion at all: a named subtree. Would count an \
                 untracked, git-ignored directory placed under `crates/` -- recorded, not migrated \
                 (its tests drive it with synthetic roots that are not git checkouts)",
    },
    Classified {
        path: "crates/fjell-tools/src/lint_allow_report.rs",
        reason: "walks `crates/` for crate roots with NO exclusion: a named subtree; same residual \
                 as `callsite_audit` and the same reason it is not migrated",
    },
    Classified {
        path: "crates/fjell-tools/src/qemu_run.rs",
        reason: "lists the one folder `tests/qemu/profiles` (a flat listing of a fixed folder); the \
                 `is_dir` is in a test assertion",
    },
    Classified {
        path: "crates/fjell-tools/src/rfc_promote.rs",
        reason: "its recursive walk is a TEST helper that dumps a scratch directory the test created; \
                 it never looks at the repository",
    },
    Classified {
        path: "crates/fjell-tools/src/trust_report.rs",
        reason: "uses the shared scope for the repository; the remaining `read_dir`s list `prebuilt/` \
                 and one other fixed folder (flat listings, `is_dir` is a per-entry filter)",
    },
    Classified {
        path: "tools/fjell-consistency-check/src/book_structure.rs",
        reason: "walks the book's source tree, skipping a short named list of GENERATED output \
                 directories: a named subtree, and the list names build output, not scratch",
    },
    Classified {
        path: "tools/fjell-consistency-check/src/evidence.rs",
        reason: "walks `tests/evidence/` (a fixed folder) for `.log` files; the repository-wide \
                 Markdown walk it used to carry is the shared scope's now",
    },
    Classified {
        path: "tools/fjell-consistency-check/src/toolchain_declarations.rs",
        reason: "walks `LIVE_INSTRUCTION_ROOTS` minus `INSTRUCTION_EXCLUSIONS`: a designed list of \
                 named roots, each exclusion carrying its own stated reason (RFC-0.30-003), because \
                 'live instruction file' is a judgement and not a git property",
    },
    Classified {
        path: "tools/fjell-repro-check/src/main.rs",
        reason: "lists `*.bin` in the fixed `DEFAULT_TARGETS` folders; `is_dir` distinguishes a \
                 folder target from a file target",
    },
];

/// Dependency names that would hide a tree walk behind a crate.
fn walk_crates() -> [String; 3] {
    [
        format!("{}{}", "walk", "dir"),
        "ignore".to_string(),
        "glob".to_string(),
    ]
}

/// The textual signatures of a scope-defining enumeration, in code (comment lines excluded).
fn signatures() -> (String, String, String, String) {
    (
        format!("\"ls-{}\"", "files"),  // a shell-out to `git ls-files`
        format!("read_{}(", "dir"),     // a directory listing...
        format!("is_{}()", "dir"),      // ...that also descends
        format!("{}{}", "Walk", "Dir"), // the walking crate's type
    )
}

/// What a source file does about scope.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub ls_files: bool,
    pub walks: bool,
    pub uses_shared: bool,
}

impl Facts {
    pub fn relevant(&self) -> bool {
        self.ls_files || self.walks
    }
}

/// Read one file's facts, ignoring whole-line comments.
pub fn facts_of(src: &str) -> Facts {
    let (lsf, rd, isd, wd) = signatures();
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    Facts {
        ls_files: code.contains(&lsf),
        walks: (code.contains(&rd) && code.contains(&isd)) || code.contains(&wd),
        uses_shared: code.contains("fjell_repo_scope::"),
    }
}

/// The report of a judged tree.
#[derive(Debug)]
pub struct Report {
    pub examined: usize,
    pub shared_users: usize,
    pub classified: Vec<&'static str>,
}

/// Judge `files` (repository-relative path, text) and manifests (path, text) against `classified`.
pub fn judge(
    files: &[(String, String)],
    manifests: &[(String, String)],
    classified: &[Classified],
) -> Result<Report, Vec<String>> {
    let mut why = Vec::new();
    let mut shared_users = 0usize;
    let mut seen_relevant: Vec<&str> = Vec::new();

    for (path, text) in files {
        // The shared-scope crate IS the one place allowed to enumerate.
        if path.starts_with("tools/fjell-repo-scope/") {
            continue;
        }
        let f = facts_of(text);
        if f.uses_shared {
            shared_users += 1;
        }
        if !f.relevant() {
            continue;
        }
        seen_relevant.push(path.as_str());
        let listed = classified.iter().any(|c| c.path == path);
        // A file that shells out to `git ls-files` itself is re-deriving the scope even if it also
        // uses the shared one: only the shared crate may.
        if f.ls_files {
            why.push(format!(
                "{path}: shells out to `git ls-files` itself. Scope is `fjell-repo-scope`'s to answer; \
                 call `fjell_repo_scope::files`"
            ));
            continue;
        }
        // Using the shared scope does not excuse a walk of one's own: a file that does both still
        // has a walk that must be classified (otherwise a rogue name-list walk could hide behind
        // an unrelated import of the shared crate).
        if !listed {
            why.push(format!(
                "{path}: walks the tree ( `read_dir` + `is_dir`, or a walking crate ) and is not classified in `CLASSIFIED` (tools/fjell-consistency-check/src/repo_scope.rs). \
                 Either ask the shared scope, or add an entry saying which NAMED subtree or fixed folder it walks and why"
            ));
        }
    }

    for c in classified {
        match files.iter().find(|(p, _)| p == c.path) {
            None => why.push(format!(
                "CLASSIFIED entry `{}` names no Rust source in the repository: retire it (a list is not a place to park a name)",
                c.path
            )),
            Some((_, text)) => {
                if !facts_of(text).relevant() {
                    why.push(format!(
                        "CLASSIFIED entry `{}` no longer lists or walks directories: retire it",
                        c.path
                    ));
                }
            }
        }
    }

    // Walking crates are forbidden outright: a manifest says it exactly.
    let banned = walk_crates();
    for (path, text) in manifests {
        let mut in_deps = false;
        for line in text.lines() {
            let l = line.trim();
            if l.starts_with('[') {
                in_deps = l.contains("dependencies");
                continue;
            }
            if in_deps {
                let name = l.split(['=', ' ']).next().unwrap_or("");
                if banned.iter().any(|b| b == name) {
                    why.push(format!(
                        "{path}: depends on `{name}`, a directory-walking crate. Scope is `fjell-repo-scope`'s"
                    ));
                }
            }
        }
    }

    if why.is_empty() {
        Ok(Report {
            examined: files.len(),
            shared_users,
            classified: classified.iter().map(|c| c.path).collect(),
        })
    } else {
        Err(why)
    }
}

pub fn render_pass(r: &Report) -> String {
    format!(
        "{NAME}: PASS ({} tracked Rust source(s) examined, derived from fjell-repo-scope itself; {} use the shared scope; \
         {} walk or list a NAMED subtree or fixed folder, each with a recorded reason (hand-written, and a stale or missing entry fails). \
         NOT covered: a walk the textual signature does not match, whether a classified reason is a good one, \
         and `callsite_audit`/`lint_allow_report`, which walk `crates/` with no exclusion)",
        r.examined,
        r.shared_users,
        r.classified.len()
    )
}

pub fn check() -> ExitCode {
    use fjell_repo_scope::{Scope, files_with_extension};
    let root = std::path::Path::new(".");
    let rs = match files_with_extension(root, Scope::Working, "rs") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot derive the repository's file set: {e}");
            return ExitCode::FAILURE;
        }
    };
    let tomls = match files_with_extension(root, Scope::Working, "toml") {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{NAME}: FAIL — cannot derive the repository's file set: {e}");
            return ExitCode::FAILURE;
        }
    };
    let rel = |p: &std::path::Path| {
        p.strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let load = |paths: Vec<std::path::PathBuf>, only_manifests: bool| -> Vec<(String, String)> {
        paths
            .iter()
            .filter(|p| !only_manifests || p.file_name().is_some_and(|n| n == "Cargo.toml"))
            .filter_map(|p| std::fs::read_to_string(p).ok().map(|t| (rel(p), t)))
            .collect()
    };
    let files = load(rs, false);
    let manifests = load(tomls, true);
    match judge(&files, &manifests, CLASSIFIED) {
        Ok(r) => {
            for c in CLASSIFIED {
                println!("{NAME}: note — classified `{}`: {}", c.path, c.reason);
            }
            println!("{}", render_pass(&r));
            ExitCode::SUCCESS
        }
        Err(why) => {
            for w in &why {
                eprintln!("{NAME}: FAIL — {w}");
            }
            eprintln!("{NAME}: FAIL");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walker() -> String {
        // A recursive walk: lists a directory and descends into the entries that are directories.
        format!(
            "fn f(d: &Path) {{ for e in fs::{}(d)? {{ if e.path().{}() {{ f(&e.path()); }} }} }}",
            "read_dir", "is_dir"
        )
    }

    fn lister() -> String {
        // The signature of a flat listing: a listing with no descent.
        format!(
            "fn f(d: &Path) {{ for e in fs::{}(d)? {{ use_it(e); }} }}",
            "read_dir"
        )
    }

    fn git_ls() -> String {
        format!("Command::new(\"git\").args([\"ls-{}\", \"-z\"])", "files")
    }

    fn f(path: &str, text: &str) -> (String, String) {
        (path.to_string(), text.to_string())
    }

    const NONE: &[Classified] = &[];

    #[test]
    fn a_walker_that_is_neither_shared_nor_classified_fails() {
        let e = judge(&[f("tools/new/src/main.rs", &walker())], &[], NONE).unwrap_err();
        assert!(
            e.len() == 1 && e[0].contains("tools/new/src/main.rs") && e[0].contains("CLASSIFIED"),
            "{e:#?}"
        );
    }

    #[test]
    fn importing_the_shared_scope_does_not_excuse_a_walk_of_ones_own() {
        let src = format!("use fjell_repo_scope::files;\n{}", walker());
        let e = judge(&[f("tools/new/src/main.rs", &src)], &[], NONE).unwrap_err();
        assert!(e.len() == 1 && e[0].contains("not classified"), "{e:#?}");
    }

    #[test]
    fn the_same_walker_passes_once_it_asks_the_shared_scope_instead() {
        let src = format!("{}\nuse fjell_repo_scope::files;", "fn main() {}");
        assert!(judge(&[f("tools/new/src/main.rs", &src)], &[], NONE).is_ok());
    }

    #[test]
    fn a_classified_walker_passes_and_is_counted() {
        static C: &[Classified] = &[Classified {
            path: "tools/old/src/main.rs",
            reason: "walks docs/",
        }];
        let r = judge(&[f("tools/old/src/main.rs", &walker())], &[], C).unwrap();
        assert_eq!(r.classified, ["tools/old/src/main.rs"]);
    }

    #[test]
    fn a_flat_listing_is_not_scope_relevant() {
        assert!(!facts_of(&lister()).relevant());
        assert!(judge(&[f("tools/x/src/main.rs", &lister())], &[], NONE).is_ok());
    }

    #[test]
    fn a_shell_out_to_git_ls_files_is_refused_even_alongside_the_shared_scope() {
        let src = format!("{}\nuse fjell_repo_scope::files;", git_ls());
        let e = judge(&[f("tools/x/src/main.rs", &src)], &[], NONE).unwrap_err();
        assert!(
            e[0].contains("shells out to `git ls-files` itself"),
            "{e:#?}"
        );
    }

    #[test]
    fn the_shared_crate_itself_may_enumerate() {
        assert!(
            judge(
                &[f("tools/fjell-repo-scope/src/lib.rs", &git_ls())],
                &[],
                NONE
            )
            .is_ok()
        );
    }

    #[test]
    fn comment_lines_do_not_count() {
        let src = format!("// {}\n{}", walker(), "fn main() {}");
        assert!(!facts_of(&src).relevant());
        // ...but the same text in code does.
        assert!(facts_of(&walker()).relevant());
    }

    #[test]
    fn a_stale_classified_entry_fails_both_ways() {
        static C: &[Classified] = &[Classified {
            path: "tools/gone/src/main.rs",
            reason: "x",
        }];
        // The file is gone.
        let e = judge(&[], &[], C).unwrap_err();
        assert!(
            e[0].contains("names no Rust source") && e[0].contains("retire"),
            "{e:#?}"
        );
        // The file exists and no longer walks.
        let e = judge(&[f("tools/gone/src/main.rs", "fn main() {}")], &[], C).unwrap_err();
        assert!(e[0].contains("no longer lists or walks"), "{e:#?}");
    }

    #[test]
    fn a_walking_crate_in_a_manifest_is_refused() {
        let name = format!("{}{}", "walk", "dir");
        let m = format!("[package]\nname = \"x\"\n\n[dependencies]\n{name} = \"2\"\n");
        let e = judge(&[], &[f("tools/x/Cargo.toml", &m)], NONE).unwrap_err();
        assert!(
            e[0].contains(&name) && e[0].contains("tools/x/Cargo.toml"),
            "{e:#?}"
        );
        // Control: the same name under [package] is not a dependency.
        let ok = format!("[package]\nname = \"{name}\"\n");
        assert!(judge(&[], &[f("tools/x/Cargo.toml", &ok)], NONE).is_ok());
    }

    #[test]
    fn the_pass_line_states_its_limits() {
        let r = Report {
            examined: 9,
            shared_users: 4,
            classified: vec!["a", "b"],
        };
        let line = render_pass(&r);
        for needle in [
            "repo-scope: PASS",
            "9 tracked Rust source(s)",
            "4 use the shared scope",
            "2 walk or list a NAMED subtree",
            "NOT covered",
            "no exclusion",
        ] {
            assert!(line.contains(needle), "missing `{needle}`: {line}");
        }
    }

    #[test]
    fn the_real_tree_passes_and_the_population_is_derived_not_listed() {
        use fjell_repo_scope::{Scope, files_with_extension};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .unwrap();
        let rs = files_with_extension(root, Scope::Working, "rs").unwrap();
        let files: Vec<(String, String)> = rs
            .iter()
            .filter_map(|p| {
                std::fs::read_to_string(p).ok().map(|t| {
                    (
                        p.strip_prefix(root).unwrap().to_string_lossy().into_owned(),
                        t,
                    )
                })
            })
            .collect();
        assert!(
            files.len() > 300,
            "control: the derived population is the whole repository ({})",
            files.len()
        );
        let r = judge(&files, &[], CLASSIFIED).unwrap_or_else(|e| panic!("{e:#?}"));
        assert!(
            r.shared_users >= 5,
            "the migrated tools use the shared scope: {r:?}"
        );
    }
}
