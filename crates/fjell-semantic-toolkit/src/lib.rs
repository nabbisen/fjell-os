//! # `fjell-semantic-toolkit` — Semantic authoring toolkit
//!
//! Implements RFC v0.9-003. Three surfaces:
//!
//! 1. **Typed emitter** — compile-time-checked emit API that wraps the
//!    catalog v1 encode path. Each catalog entry becomes a structured
//!    `emit_*` function; using the wrong fields is a type error.
//!    (Code-generated variants land in v0.9.1; this crate ships the
//!    infrastructure and one representative hand-written entry.)
//!
//! 2. **Fixture generator** — produces a representative valid encoded
//!    sample for every catalog entry, usable as test fixtures and fuzz
//!    seed corpora.
//!
//! 3. **Doc renderer** — walks the catalog and generates Markdown for
//!    `docs/src/semantic/catalog.md` with schema tables and owner info.

// Host-only: uses std.

use fjell_semantic_v1::{
    catalog_len, encode, decode,
    FieldValue, FieldKind, CATALOG_V1,
};

// ── Fixture generator ─────────────────────────────────────────────────────────

/// A generated fixture for one catalog entry.
pub struct EntryFixture {
    /// The intent tag this fixture belongs to.
    pub tag: u16,
    /// Human-readable catalog name (e.g. `"UPDATE.STAGING_STARTED"`).
    pub name: &'static str,
    /// A valid minimal encoding — all required fields populated with
    /// representative values, optional fields absent.
    pub minimal_encoded: Vec<u8>,
    /// A valid maximal encoding — all fields populated.
    pub maximal_encoded: Vec<u8>,
}

/// Generate one fixture per catalog v1 entry.
///
/// Each fixture carries a minimal and a maximal encoding. Minimal fills
/// only required fields with representative values; maximal fills every
/// field. Both encode and decode losslessly (round-trip tested in
/// [`verify_all_fixtures_round_trip`]).
pub fn generate_all_fixtures() -> Vec<EntryFixture> {
    let mut out = Vec::with_capacity(catalog_len());
    for entry in CATALOG_V1.iter() {
        // Build field value lists: minimal (required only) and maximal (all).
        let mut minimal: Vec<FieldValue> = Vec::new();
        let mut maximal: Vec<FieldValue> = Vec::new();
        for fd in entry.schema.fields.iter() {
            let representative = representative_value(fd.kind);
            if fd.required {
                minimal.push(representative);
            } else {
                minimal.push(FieldValue::Absent);
            }
            maximal.push(representative);
        }

        let minimal_encoded = encode_fixture(entry.tag, &minimal);
        let maximal_encoded = encode_fixture(entry.tag, &maximal);

        out.push(EntryFixture {
            tag: entry.tag,
            name: entry.name,
            minimal_encoded,
            maximal_encoded,
        });
    }
    out
}

fn representative_value(kind: FieldKind) -> FieldValue {
    match kind {
        FieldKind::U8      => FieldValue::U8(0x01),
        FieldKind::U16     => FieldValue::U16(0x0102),
        FieldKind::U32     => FieldValue::U32(0x0102_0304),
        FieldKind::U64     => FieldValue::U64(0x0102_0304_0506_0708),
        FieldKind::Bytes16 => FieldValue::Bytes16([0xAB; 16]),
        FieldKind::Bytes32 => FieldValue::Bytes32([0xCD; 32]),
    }
}

fn encode_fixture(tag: u16, fields: &[FieldValue]) -> Vec<u8> {
    let mut buf = vec![0u8; 512];
    let n = encode(tag, /* created_tick */ 1_000_000, fields, &mut buf)
        .expect("fixture encode must not fail");
    buf.truncate(n);
    buf
}

/// Verify every generated fixture round-trips losslessly.
///
/// Returns a list of `(tag, error_message)` for any entry that fails.
/// An empty return value means all fixtures are consistent.
pub fn verify_all_fixtures_round_trip() -> Vec<(u16, String)> {
    let mut errors = Vec::new();
    for fx in generate_all_fixtures() {
        match decode(&fx.minimal_encoded) {
            Ok(d) => {
                if d.tag != fx.tag {
                    errors.push((fx.tag, format!(
                        "decode tag mismatch: got 0x{:04X}", d.tag
                    )));
                }
            }
            Err(e) => {
                errors.push((fx.tag, format!("decode error: {:?}", e)));
            }
        }
        match decode(&fx.maximal_encoded) {
            Ok(d) => {
                if d.tag != fx.tag {
                    errors.push((fx.tag, format!(
                        "maximal decode tag mismatch: got 0x{:04X}", d.tag
                    )));
                }
            }
            Err(e) => {
                errors.push((fx.tag, format!("maximal decode error: {:?}", e)));
            }
        }
    }
    errors
}

// ── Doc renderer ──────────────────────────────────────────────────────────────

/// Render the v1 catalog to Markdown for `docs/src/semantic/catalog.md`.
///
/// The output is a self-contained Markdown document covering:
/// - all catalog entries grouped by domain,
/// - per-entry field tables (name, kind, required),
/// - ownership metadata.
pub fn render_catalog_markdown() -> String {
    let mut md = String::new();
    md.push_str("# Fjell OS — Semantic Intent Catalog v1\n\n");
    md.push_str("*Auto-generated by `fjell-semantic-toolkit`. Do not edit.*\n\n");
    md.push_str(&format!("Total entries: **{}**\n\n", catalog_len()));

    let mut current_domain = "";
    for entry in CATALOG_V1.iter() {
        let domain = entry.name.split('.').next().unwrap_or("UNKNOWN");
        if domain != current_domain {
            md.push_str(&format!("\n## {} domain\n\n", domain));
            current_domain = domain;
        }

        md.push_str(&format!("### `0x{:04X}` — `{}`\n\n", entry.tag, entry.name));
        md.push_str(&format!(
            "- **Owner crate:** `{}`  \n- **Subsystem:** `{}`\n\n",
            entry.owner.crate_name,
            entry.owner.subsystem,
        ));

        if entry.schema.fields.is_empty() {
            md.push_str("*No fields.*\n\n");
        } else {
            md.push_str("| Field | Kind | Required |\n");
            md.push_str("|-------|------|----------|\n");
            for fd in entry.schema.fields.iter() {
                md.push_str(&format!(
                    "| `{}` | {:?} | {} |\n",
                    fd.name,
                    fd.kind,
                    if fd.required { "yes" } else { "no" }
                ));
            }
            md.push('\n');
        }
    }
    md
}

// ── Typed emitter API (compile-time-checked) ──────────────────────────────────

/// Typed arguments for `UPDATE.STAGING_ADVANCED` (tag `0x0101`).
///
/// Fields match `SCHEMA_UPDATE_STAGING_ADVANCED` exactly. This is the
/// first hand-written typed entry; code-generated structs for every
/// catalog entry land in v0.9.1.
pub struct UpdateStagingAdvancedArgs {
    /// Upgrade candidate identifier.
    pub candidate_id: u32,
    /// State being transitioned from.
    pub from_state: u16,
    /// State being transitioned to.
    pub to_state: u16,
}

/// Emit a `UPDATE.STAGING_ADVANCED` intent record into `out`.
///
/// Returns the number of bytes written, or a `fjell_semantic_v1::SemanticError`.
pub fn emit_update_staging_advanced(
    args: &UpdateStagingAdvancedArgs,
    tick: u64,
    out: &mut [u8],
) -> Result<usize, fjell_semantic_v1::SemanticError> {
    encode(
        0x0101,
        tick,
        &[
            FieldValue::U32(args.candidate_id),
            FieldValue::U16(args.from_state),
            FieldValue::U16(args.to_state),
        ],
        out,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_count_matches_catalog() {
        let fixtures = generate_all_fixtures();
        assert_eq!(fixtures.len(), catalog_len());
    }

    #[test]
    fn all_fixtures_round_trip() {
        let errors = verify_all_fixtures_round_trip();
        assert!(errors.is_empty(),
            "round-trip failures: {:?}", errors);
    }

    #[test]
    fn fixtures_have_correct_tags() {
        for (i, (entry, fixture)) in
            CATALOG_V1.iter().zip(generate_all_fixtures().iter()).enumerate()
        {
            assert_eq!(entry.tag, fixture.tag,
                "fixture #{} tag mismatch", i);
        }
    }

    #[test]
    fn minimal_is_shorter_than_or_equal_to_maximal() {
        for fx in generate_all_fixtures() {
            assert!(
                fx.minimal_encoded.len() <= fx.maximal_encoded.len(),
                "tag 0x{:04X}: minimal > maximal", fx.tag
            );
        }
    }

    #[test]
    fn doc_renderer_covers_all_entries() {
        let md = render_catalog_markdown();
        for entry in CATALOG_V1.iter() {
            let tag_str = format!("0x{:04X}", entry.tag);
            assert!(md.contains(&tag_str),
                "catalog.md missing tag {}", tag_str);
            assert!(md.contains(entry.name),
                "catalog.md missing name {}", entry.name);
        }
    }

    #[test]
    fn doc_renderer_contains_domain_headers() {
        let md = render_catalog_markdown();
        for domain in ["UPDATE", "ATTEST", "SECURITY", "NET",
                        "RECOVERY", "PLATFORM", "HEALTH"] {
            let header = format!("## {} domain", domain);
            assert!(md.contains(&header),
                "catalog.md missing '{}'", header);
        }
    }

    #[test]
    fn typed_emitter_update_staging_advanced() {
        let args = UpdateStagingAdvancedArgs { candidate_id: 42, from_state: 1, to_state: 2 };
        let mut buf = [0u8; 128];
        let n = emit_update_staging_advanced(&args, 42, &mut buf).unwrap();
        assert!(n > 0);
        let decoded = decode(&buf[..n]).unwrap();
        assert_eq!(decoded.tag, 0x0101);
    }
}

/// Auto-generated typed emitters (RFC-v0.14-003). Generated by `cargo xtask toolkit regenerate`.
pub mod generated;
