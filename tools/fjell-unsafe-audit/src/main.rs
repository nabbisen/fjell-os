//! `fjell-unsafe-audit` — unsafe boundary inventory scanner (RFC v0.6-004).
//!
//! Usage:
//!   fjell-unsafe-audit [--root <path>] [--json] [--check]
//!   fjell-unsafe-audit --help | -h
//!
//! Exits 0 if every unsafe site has a `// SAFETY: category=raw-pointer-deref` comment within 4 lines.
//! Exits 1 if any unsafe site is missing a SAFETY comment.
//! With `--json`, prints a JSON-lines inventory to stdout.
//!
//! RFC-0.35-003 (E-071's sweep, R4): an unrecognised argument used to be
//! silently discarded (`_ => {}` in the argument loop) and the scan ran
//! anyway — the same "ignores the unrecognised" shape named for
//! `fjell-repro-check`, found by sweeping this directory for it. An
//! unrecognised argument now exits 2 with the usage; `--help`/`-h` prints
//! it and exits 0.

mod audit;
mod exempt;

use audit::{Analysis, Category, Exemption, analyse};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process,
};

// ── Scope discovery (RFC-0.28-005 D1) ──────────────────────────────────────────

/// This tool audits code, committed or not — uncommitted work is exactly
/// when an unsafe-site audit should speak up (RFC-0.28-005 §6), so the
/// walk cannot be "tracked files only." But the boundary still must not be
/// a hand-maintained name list (E-015's family; that was E-025): a scratch
/// checkout under `.git-exclude/tmp/` doubled this tool's own count before
/// this fix, because `.git-exclude` was not one of the three literal names
/// the old walk skipped.
///
/// The derived answer: everything git considers part of this working
/// copy — tracked files, plus untracked files `.gitignore` does not
/// exclude. A new, uncommitted `.rs` file is still audited; a scratch
/// checkout under `.git-exclude/` or `target/` is not — with zero enumerated
/// name anywhere in this tool. The question "which files count as this
/// repository" is answered in one place (`fjell-repo-scope`, 0.46 small item B / E-125).
fn discover_rs_files(root: &Path) -> io::Result<Vec<PathBuf>> {
    fjell_repo_scope::files_with_extension(root, fjell_repo_scope::Scope::Working, "rs")
}

/// `[workspace.package] version` from the root `Cargo.toml`; a seeded exemption is refused once the workspace is past its milestone.
fn workspace_version(root: &Path) -> String {
    let text = fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let mut in_table = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_table = t == "[workspace.package]";
            continue;
        }
        if in_table && let Some(rest) = t.strip_prefix("version") {
            return rest
                .trim_start()
                .trim_start_matches('=')
                .trim()
                .trim_matches('"')
                .to_string();
        }
    }
    String::new()
}

// ── Main ──────────────────────────────────────────────────────────────────────

const USAGE: &str = "Usage: fjell-unsafe-audit [--root <path>] [--json] [--check] [--print-exemptions]\n       fjell-unsafe-audit --help | -h";

/// What a green means, and does not. **Pinned by a test**: the overstatement this line exists to remove must not be editable back in.
pub fn pass_line(a: &Analysis) -> String {
    let permanent = a.exemptions_total - a.seeded;
    format!(
        "unsafe-audit: PASS ({} sites; every one has its own SAFETY comment and a category its body or its callees support -- {} judged against a shape, {} with no body to judge (impl, trait, extern block, attribute) judged only on having a comment and a valid tag; \
         {} exempted, of which {} are seeded awaiting a sweep and {} permanent -- an exempted site is UNJUDGED. This does NOT check that any comment's prose is true: a green means a tag is not contradicted, not that a justification is correct)",
        a.sites.len(),
        a.judged,
        a.bodyless,
        a.exemptions_total,
        a.seeded,
        permanent
    )
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }

    let mut root = PathBuf::from(".");
    let mut json = false;
    let mut check = false;
    let mut print_ex = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                if i < args.len() {
                    root = PathBuf::from(&args[i]);
                }
            }
            "--json" => json = true,
            "--check" => check = true,
            "--print-exemptions" => print_ex = true,
            other => {
                eprintln!("fjell-unsafe-audit: unrecognised argument: {other}");
                eprintln!("{USAGE}");
                process::exit(2);
            }
        }
        i += 1;
    }

    let paths = match discover_rs_files(&root) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("fjell-unsafe-audit: {e}");
            process::exit(2);
        }
    };
    let mut files: Vec<(String, String)> = Vec::new();
    for f in &paths {
        match fs::read_to_string(f) {
            Ok(t) => files.push((f.display().to_string().replace('\\', "/"), t)),
            Err(e) => {
                eprintln!("fjell-unsafe-audit: scan error on {}: {e}", f.display());
                process::exit(2);
            }
        }
    }
    let exemptions: Vec<Exemption> = exempt::exemptions();
    let a = analyse(&files, &exemptions, &workspace_version(&root));

    let total = a.sites.len();
    let missing = a.sites.iter().filter(|s| s.comment.is_none()).count();
    let covered = total - missing;
    let valid_cats = a
        .sites
        .iter()
        .filter(|s| s.comment.is_some() && s.has_category && s.unknown_tags.is_empty())
        .count();
    let missing_cats = a
        .sites
        .iter()
        .filter(|s| s.comment.is_some() && (!s.has_category || !s.unknown_tags.is_empty()))
        .count();

    if print_ex {
        // The unsupported, un-exempted sites as `Exemption` literals, to seed the list. A seeded entry is a debt, not a finding settled.
        for s in a
            .sites
            .iter()
            .filter(|s| !s.unsupported.is_empty() && s.exempt.is_none())
        {
            println!(
                "    Exemption {{ file: \"{}\", scope: \"{}\", n: {}, kind: SEEDED, reason: \"awaiting RFC-0.48-001's sweep: tagged {} on a body showing {}\" }},",
                s.file,
                s.scope,
                s.n,
                s.unsupported
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                s.kind.as_str()
            );
        }
        return;
    }

    if json {
        for s in &a.sites {
            let text = s.comment.clone().unwrap_or_default();
            println!(
                r#"{{"file":"{f}","line":{l},"kind":"{k}","has_safety":{h},"category":"{cat}","category_valid":{cv},"supported":{sup},"exempt":{ex},"safety_text":"{t}"}}"#,
                f = s.file.replace('\\', "/"),
                l = s.line,
                k = s.kind.as_str(),
                h = s.comment.is_some(),
                cat = s
                    .tags
                    .iter()
                    .map(|c| c.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                cv = s.comment.is_some() && s.has_category && s.unknown_tags.is_empty(),
                sup = s.unsupported.is_empty(),
                ex = s.exempt.is_some(),
                t = text.replace('"', "\\\""),
            );
        }
    } else {
        println!("fjell-unsafe-audit  root={}", root.display());
        println!("  total unsafe sites : {total}");
        println!("  with SAFETY comment: {covered}");
        println!("  with valid category tag: {valid_cats}");
        if missing_cats > 0 {
            println!("  MISSING/UNKNOWN category: {missing_cats}");
        }
        println!("  missing comment    : {missing}");
        println!(
            "  exempted           : {} (seeded {}, permanent {})",
            a.exemptions_total,
            a.seeded,
            a.exemptions_total - a.seeded
        );
        if !a.problems.is_empty() {
            println!();
            println!("PROBLEMS ({}):", a.problems.len());
            for p in &a.problems {
                println!("  {p}");
            }
        } else {
            println!();
            println!("{}", pass_line(&a));
        }
    }

    if check && !a.problems.is_empty() {
        process::exit(1);
    }
    let _ = Category::CsrAsm; // keep the public enum referenced from main for the JSON writer's sake
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sentence that says what a green is not. **Pinned**: the overstatement RFC-0.48-002 exists to remove -- "N sites, all
    /// annotated" read as "every justification is true" -- must not be editable back in.
    #[test]
    fn the_pass_line_says_it_does_not_check_the_prose_and_counts_exemptions_apart() {
        let a = Analysis {
            exemptions_total: 5,
            seeded: 2,
            judged: 10,
            bodyless: 3,
            ..Default::default()
        };
        let l = pass_line(&a);
        assert!(
            l.contains("does NOT check that any comment's prose is true"),
            "{l}"
        );
        assert!(l.contains("an exempted site is UNJUDGED"), "{l}");
        assert!(
            l.contains("of which 2 are seeded awaiting a sweep and 3 permanent"),
            "{l}"
        );
        assert!(
            l.contains("not a justification is correct")
                || l.contains("not that a justification is correct"),
            "{l}"
        );
    }
}
