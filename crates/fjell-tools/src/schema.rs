//! `schema dump | check | show <id>` — the `.frozen` files are generated
//! (RFC-0.33-003, E-045). The logic is in `fjell-schema`; this is only the file I/O.
//!
//!   schema dump          write every registered file from its encoder
//!   schema check         compare every registered file with its encoder; exit 1 on drift
//!   schema show <id>     print what the encoder generates for one format

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fjell_schema::registry::FORMATS;
use fjell_schema::{compare, generate};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn cmd_schema(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("dump") => dump(),
        Some("check") => check(),
        Some("show") => match args
            .get(1)
            .and_then(|id| FORMATS.iter().find(|f| f.id == id))
        {
            Some(f) => match generate(f) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(problems) => {
                    for p in problems {
                        eprintln!("schema: {}: {p}", f.id);
                    }
                    ExitCode::FAILURE
                }
            },
            None => {
                eprintln!("usage: schema show <id>; ids: {}", ids());
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!("usage: schema <dump|check|show <id>>");
            ExitCode::FAILURE
        }
    }
}

fn ids() -> String {
    FORMATS.iter().map(|f| f.id).collect::<Vec<_>>().join(", ")
}

fn dump() -> ExitCode {
    let root = root();
    let mut failed = false;
    for f in FORMATS {
        match generate(f) {
            Ok(text) => {
                let path = root.join(f.path);
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let was = std::fs::read_to_string(&path).unwrap_or_default();
                if let Err(e) = std::fs::write(&path, &text) {
                    eprintln!("schema: cannot write {}: {e}", f.path);
                    failed = true;
                } else if was == text {
                    println!("schema: {} unchanged", f.path);
                } else {
                    println!("schema: {} written", f.path);
                }
            }
            Err(problems) => {
                failed = true;
                for p in problems {
                    eprintln!("schema: {}: {p}", f.id);
                }
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn check() -> ExitCode {
    let root = root();
    let mut bad = 0;
    for f in FORMATS {
        let Ok(committed) = std::fs::read_to_string(root.join(f.path)) else {
            bad += 1;
            println!(
                "schema: {} does not exist; the registry says it must",
                f.path
            );
            continue;
        };
        match generate(f) {
            Ok(text) => {
                let drift = compare(&text, &committed);
                if !drift.is_empty() {
                    bad += 1;
                    println!("schema: {} drifts from its encoder:", f.path);
                    for d in drift {
                        println!("  {d}");
                    }
                }
            }
            Err(problems) => {
                bad += 1;
                for p in problems {
                    println!("schema: {}: {p}", f.id);
                }
            }
        }
    }
    if bad == 0 {
        println!("schema: {} file(s) match their encoders", FORMATS.len());
        ExitCode::SUCCESS
    } else {
        println!(
            "schema: {bad} of {} file(s) drift — `cargo xtask schema dump` regenerates them",
            FORMATS.len()
        );
        ExitCode::FAILURE
    }
}
