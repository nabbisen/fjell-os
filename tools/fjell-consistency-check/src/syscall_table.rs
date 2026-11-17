//! The syscall surface's two committed facts, parsed once (RFC-0.45-003).
//!
//! `parse_declared` reads the `SyscallNumber` enum's variants; `parse_expected` reads
//! `tests/syscall/expected.toml`. They were private to the `syscall-surface` subcheck. The
//! `rfc-evidence` subcheck needs the same two facts (a `syscall:<Name>` citation resolves when
//! the variant is declared and not undispatched), and **a second parser here would be the defect
//! 0.44 item D removed** -- one rule implemented twice, free to drift. So they live in the
//! library half, moved verbatim, and `syscall-surface` calls them from here exactly as before.

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Expected {
    pub declared_count: usize,
    pub dispatched_count: usize,
    pub undispatched: Vec<String>,
}

/// Parse `Name = number,` variant lines inside `pub enum SyscallNumber { ... }`.
pub fn parse_declared(src: &str) -> Vec<(String, u32)> {
    let mut in_enum = false;
    let mut variants = Vec::new();
    for line in src.lines() {
        let trimmed = line.trim();
        if !in_enum {
            if trimmed.starts_with("pub enum SyscallNumber") {
                in_enum = true;
            }
            continue;
        }
        if trimmed == "}" {
            break;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let name_part = trimmed[..eq_pos].trim();
            let looks_like_variant = !name_part.is_empty()
                && name_part
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
                && name_part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            if looks_like_variant {
                let rest = trimmed[eq_pos + 1..].trim().trim_end_matches(',');
                if let Ok(num) = rest.parse::<u32>() {
                    variants.push((name_part.to_string(), num));
                }
            }
        }
    }
    variants
}

/// Minimal parser for the small TOML subset `expected.toml` actually uses
/// (two integer scalars, one string array — inline or multi-line). No
/// general TOML/serde dependency: this project's tools avoid parser
/// dependencies by convention (RFC-v0.22-001 states this explicitly for
/// Gate 11; the same reasoning applies to a format this small).
pub fn parse_expected(src: &str) -> Result<Expected, String> {
    let mut declared_count = None;
    let mut dispatched_count = None;
    let mut undispatched = Vec::new();
    let mut in_array = false;

    for raw_line in src.lines() {
        let line = match raw_line.find('#') {
            Some(i) => raw_line[..i].trim(),
            None => raw_line.trim(),
        };
        if line.is_empty() {
            continue;
        }
        if in_array {
            if line.starts_with(']') {
                in_array = false;
                continue;
            }
            let item = line.trim_end_matches(',').trim().trim_matches('"');
            if !item.is_empty() {
                undispatched.push(item.to_string());
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "declared_count" => {
                declared_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("declared_count: {e}"))?,
                );
            }
            "dispatched_count" => {
                dispatched_count = Some(
                    value
                        .parse::<usize>()
                        .map_err(|e| format!("dispatched_count: {e}"))?,
                );
            }
            "undispatched" => {
                if let Some(after) = value.strip_prefix('[') {
                    if let Some(end) = after.find(']') {
                        for item in after[..end].split(',') {
                            let item = item.trim().trim_matches('"');
                            if !item.is_empty() {
                                undispatched.push(item.to_string());
                            }
                        }
                    } else {
                        in_array = true;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(Expected {
        declared_count: declared_count.ok_or("missing declared_count")?,
        dispatched_count: dispatched_count.ok_or("missing dispatched_count")?,
        undispatched,
    })
}
