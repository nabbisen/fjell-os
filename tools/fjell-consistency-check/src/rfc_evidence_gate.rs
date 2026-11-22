//! The printing half of `rfc-evidence` (RFC-0.45-003): read the three RFC folders, judge them with
//! the library's own function, print the one line. All of the logic is in
//! `fjell_consistency_check::rfc_evidence`, so `rfc promote` asks the same question.

use fjell_consistency_check::rfc_evidence::{
    Folder, FsWorld, KNOWN_FALSE, NAME, RfcFile, WAIVERS, judge, render_pass,
};
use std::path::Path;
use std::process::ExitCode;

const DIRS: &[(&str, Folder)] = &[
    ("rfcs/proposed", Folder::Proposed),
    ("rfcs/accepted", Folder::Accepted),
    ("rfcs/done", Folder::Done),
];

pub fn check() -> ExitCode {
    if let Err(e) = crate::status::vocabulary() {
        eprintln!("{NAME}: FAIL — {e}");
        return ExitCode::FAILURE;
    }
    let mut loaded: Vec<(Folder, String, String)> = Vec::new();
    for (dir, folder) in DIRS {
        let Some(entries) = crate::read_dir_named(NAME, dir) else {
            return ExitCode::FAILURE;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "md"))
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match std::fs::read_to_string(&path) {
                Ok(text) => loaded.push((*folder, name, text)),
                Err(e) => {
                    eprintln!("{NAME}: FAIL — cannot read {}: {e}", path.display());
                    return ExitCode::FAILURE;
                }
            }
        }
    }
    let files: Vec<RfcFile> = loaded
        .iter()
        .map(|(folder, name, text)| RfcFile {
            folder: *folder,
            name,
            text,
        })
        .collect();
    let world = FsWorld::load(Path::new("."));

    match judge(&files, &world, WAIVERS, KNOWN_FALSE) {
        Ok(r) => {
            // The reasons are printed on every run, not only the ids: the list is the point.
            for k in KNOWN_FALSE {
                println!("{NAME}: note — known-false `{}`: {}", k.id, k.reason);
            }
            for w in WAIVERS {
                println!("{NAME}: note — waiver `{}`: {}", w.id, w.reason);
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
