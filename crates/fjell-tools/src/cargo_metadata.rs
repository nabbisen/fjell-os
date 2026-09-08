//! Minimal JSON reader for `cargo metadata`'s output, plus the one derived
//! fact this project needs from it — RFC-0.29-001 R1/R4.
//!
//! Just enough of JSON to read `cargo metadata --format-version 1`'s
//! `packages` array (objects, arrays, strings with the escapes `cargo`
//! actually emits, numbers, booleans, null): not a general-purpose parser,
//! and not meant to become one — this project avoids a JSON dependency
//! here the same way `qemu_run.rs` avoids a TOML one for profiles.

use std::collections::BTreeMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub enum Json {
    Null,
    // Only String/Array/Object are read by this module's own callers today
    // (`bare_metal_crate_names_from` needs exactly `name`/`manifest_path`
    // strings), but the parser must still round-trip every JSON value type
    // `cargo metadata` can emit — a bool or number field elsewhere in the
    // document must not abort parsing just because nothing reads it yet.
    #[allow(dead_code)]
    Bool(bool),
    #[allow(dead_code)]
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(a) => Some(a),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(m) => m.get(key),
            _ => None,
        }
    }
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn parse_value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(Json::String),
            Some(b't') => {
                self.expect_lit("true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.expect_lit("false")?;
                Ok(Json::Bool(false))
            }
            Some(b'n') => {
                self.expect_lit("null")?;
                Ok(Json::Null)
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            other => Err(format!("unexpected byte {other:?} at offset {}", self.i)),
        }
    }
    fn expect_lit(&mut self, lit: &str) -> Result<(), String> {
        if self.b[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            Ok(())
        } else {
            Err(format!("expected `{lit}` at offset {}", self.i))
        }
    }
    fn parse_object(&mut self) -> Result<Json, String> {
        self.i += 1; // '{'
        let mut m = BTreeMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Object(m));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(format!("expected `:` at offset {}", self.i));
            }
            self.i += 1;
            let val = self.parse_value()?;
            m.insert(key, val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                other => {
                    return Err(format!(
                        "expected `,` or `}}` at offset {} ({other:?})",
                        self.i
                    ));
                }
            }
        }
        Ok(Json::Object(m))
    }
    fn parse_array(&mut self) -> Result<Json, String> {
        self.i += 1; // '['
        let mut v = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Array(v));
        }
        loop {
            let val = self.parse_value()?;
            v.push(val);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                other => {
                    return Err(format!(
                        "expected `,` or `]` at offset {} ({other:?})",
                        self.i
                    ));
                }
            }
        }
        Ok(Json::Array(v))
    }
    fn parse_string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(format!("expected `\"` at offset {}", self.i));
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some(b'"') => {
                    self.i += 1;
                    break;
                }
                Some(b'\\') => {
                    self.i += 1;
                    match self.peek() {
                        Some(b'"') => {
                            out.push('"');
                            self.i += 1;
                        }
                        Some(b'\\') => {
                            out.push('\\');
                            self.i += 1;
                        }
                        Some(b'/') => {
                            out.push('/');
                            self.i += 1;
                        }
                        Some(b'n') => {
                            out.push('\n');
                            self.i += 1;
                        }
                        Some(b't') => {
                            out.push('\t');
                            self.i += 1;
                        }
                        Some(b'r') => {
                            out.push('\r');
                            self.i += 1;
                        }
                        Some(b'b') => {
                            out.push('\u{8}');
                            self.i += 1;
                        }
                        Some(b'f') => {
                            out.push('\u{c}');
                            self.i += 1;
                        }
                        Some(b'u') => {
                            self.i += 1;
                            let hex = self
                                .b
                                .get(self.i..self.i + 4)
                                .and_then(|s| std::str::from_utf8(s).ok())
                                .ok_or("bad \\u escape")?;
                            let cp = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                            if let Some(c) = char::from_u32(cp) {
                                out.push(c);
                            }
                            self.i += 4;
                        }
                        other => return Err(format!("bad escape {other:?}")),
                    }
                }
                Some(c) => {
                    let start = self.i;
                    self.i += utf8_len(c);
                    out.push_str(
                        std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?,
                    );
                }
            }
        }
        Ok(out)
    }
    fn parse_number(&mut self) -> Result<Json, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-'))
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
        s.parse::<f64>()
            .map(Json::Number)
            .map_err(|e| e.to_string())
    }
}

fn utf8_len(b: u8) -> usize {
    if b & 0x80 == 0 {
        1
    } else if b & 0xE0 == 0xC0 {
        2
    } else if b & 0xF0 == 0xE0 {
        3
    } else {
        4
    }
}

pub fn parse(s: &str) -> Result<Json, String> {
    let mut p = Parser {
        b: s.as_bytes(),
        i: 0,
    };
    p.parse_value()
}

/// Workspace member crates whose manifest lives under a bare-metal path
/// (`crates/fjell-kernel`, `crates/services/*`, `crates/drivers/*`).
/// Every one of these is `#![no_std]`/`#![no_main]` with its own
/// `panic_impl` — confirmed live (RFC-0.29-001 R1): `cargo test --bins`
/// against any of them produces `duplicate lang item 'panic_impl'` and
/// unresolved `alloc` types, not a test failure, because the host build
/// links `std` (which already defines that lang item) while these crates
/// are written only for `--target riscv64gc-unknown-none-elf`.
///
/// Derived from `cargo metadata`, not a name list: a new service or driver
/// crate is excluded automatically by where it lives on disk, the same
/// way a new profile file is picked up by
/// `qemu_run::discover_negative_categories` rather than needing to be
/// added to one more hand-maintained list.
pub fn bare_metal_crate_names() -> Result<Vec<String>, String> {
    bare_metal_crate_names_from(Path::new("."))
}

/// The single `cargo test` invocation for "everything `--lib` cannot reach"
/// (RFC-0.29-001 R1) — shared by `test_all`'s own tier and
/// `cargo xtask host-bin-tests` (CI's `ci-host-bins` job), so the flag
/// choice and the bare-metal exclude list are computed in exactly one
/// place rather than kept in step by hand.
pub fn host_bin_test_argv() -> Result<Vec<String>, String> {
    let bare_metal = bare_metal_crate_names()?;
    let mut argv: Vec<String> = vec![
        "cargo".into(),
        "test".into(),
        "--workspace".into(),
        "--bins".into(),
        "--tests".into(),
        "--exclude".into(),
        "fjell-proptest".into(),
        // See `test_all`'s own comment at its tier-1b call site for why
        // this is here: excluding the bare-metal crates above removes
        // fjell-secure-transportd, which was the dependency edge quietly
        // turning this feature on for the whole build via feature
        // unification. Without it, fjell-sxt-crypto's own `compile_error!`
        // guard (RFC-v0.7.3-002) fires for real.
        "--features".into(),
        "fjell-sxt-crypto/crypto-profile-development".into(),
    ];
    for name in bare_metal {
        argv.push("--exclude".into());
        argv.push(name);
    }
    Ok(argv)
}

/// `cargo xtask host-bin-tests` — the standalone entry point CI's
/// `ci-host-bins` job runs, so it and `test_all`'s tier 1b execute the
/// literal same command rather than two invocations kept in step by hand.
/// Unlike `test_all`'s tiers, this inherits stdio directly (no capture, no
/// log-bundle write) — CI's own step output is the log.
pub fn cmd_host_bin_tests() -> std::process::ExitCode {
    let argv = match host_bin_test_argv() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("[xtask] host-bin-tests: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let status = Command::new(&argv[0]).args(&argv[1..]).status();
    match status {
        Ok(s) if s.success() => std::process::ExitCode::SUCCESS,
        _ => std::process::ExitCode::FAILURE,
    }
}

use std::path::Path;

/// `root` is `.` for the real invocation; tests pass an absolute path
/// computed from `CARGO_MANIFEST_DIR` instead, since `cargo test` runs
/// test binaries with the crate's own directory as the working directory,
/// not the workspace's (`cargo metadata` itself does not care — it always
/// resolves relative to `--manifest-path`/cwd, so this is passed through
/// as `--manifest-path <root>/Cargo.toml`, not a cwd change).
pub fn bare_metal_crate_names_from(root: &Path) -> Result<Vec<String>, String> {
    let manifest = root.join("Cargo.toml");
    let out = Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest)
        .output()
        .map_err(|e| format!("cannot run cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let root_json = parse(&text)?;
    let packages = root_json
        .get("packages")
        .and_then(Json::as_array)
        .ok_or("cargo metadata: no `packages` array")?;

    let mut names = Vec::new();
    for pkg in packages {
        let (Some(name), Some(manifest_path)) = (
            pkg.get("name").and_then(Json::as_str),
            pkg.get("manifest_path").and_then(Json::as_str),
        ) else {
            continue;
        };
        let mp = manifest_path.replace('\\', "/");
        if mp.contains("/crates/services/")
            || mp.contains("/crates/drivers/")
            || mp.ends_with("/crates/fjell-kernel/Cargo.toml")
        {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_object() {
        let v = parse(r#"{"a":1,"b":"two","c":true,"d":null}"#).unwrap();
        assert_eq!(v.get("a").unwrap().as_str(), None);
        assert_eq!(v.get("b").unwrap().as_str(), Some("two"));
    }

    #[test]
    fn parses_nested_array_of_objects() {
        let v = parse(r#"{"packages":[{"name":"a"},{"name":"b"}]}"#).unwrap();
        let pkgs = v.get("packages").unwrap().as_array().unwrap();
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].get("name").unwrap().as_str(), Some("a"));
        assert_eq!(pkgs[1].get("name").unwrap().as_str(), Some("b"));
    }

    #[test]
    fn parses_escaped_string() {
        let v = parse(r#"{"path":"C:\\Users\\x"}"#).unwrap();
        assert_eq!(v.get("path").unwrap().as_str(), Some("C:\\Users\\x"));
    }

    fn test_workspace_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn bare_metal_crate_names_excludes_host_tools_includes_kernel_and_services() {
        let names = bare_metal_crate_names_from(&test_workspace_root())
            .expect("cargo metadata should succeed against the real workspace");
        assert!(names.contains(&"fjell-kernel".to_string()));
        assert!(names.contains(&"fjell-init".to_string()));
        assert!(names.contains(&"fjell-driver-uart".to_string()));
        assert!(!names.contains(&"fjell-tools".to_string()));
        assert!(!names.contains(&"fjell-consistency-check".to_string()));
        assert!(!names.contains(&"fjell-unsafe-audit".to_string()));
    }
}
