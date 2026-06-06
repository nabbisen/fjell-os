//! # `fjell-cap-manifest` — Capability Request Manifest format and lint
//!
//! Implements RFC v0.9-002. A `CapManifest` is a small TOML file
//! shipped *with* each Fjell service that declares the capabilities,
//! IPC tags, leases, and semantic intents the service intends to use.
//!
//! ## Why a separate format
//!
//! The cap-broker (RFC 040) enforces policy **at runtime**. The
//! manifest brings two new properties:
//!
//! - **Build-time visibility.** Developers see what their service will
//!   be granted before booting Fjell.
//! - **Operator-visible auditability.** A signed bundle (RFC v0.9-004)
//!   carries the manifest digest; what a service *asks for* is part of
//!   what is signed.
//!
//! ## Anti-goals
//!
//! The manifest does NOT enforce policy. The cap-broker remains
//! authoritative. The manifest is a **declaration**; if a service
//! claims it needs MMIO but the deployment's CapBrokerPolicy refuses
//! to grant it, the service is denied at runtime regardless of what
//! the manifest says.
//!
//! ## Scope
//!
//! Host-only. This crate uses `std`. The manifest is build-tooling;
//! Fjell services and kernel never read it at runtime.

use std::collections::BTreeSet;
use std::fmt;

// ── Manifest type ────────────────────────────────────────────────────────────

/// A service's declared capability and emission requirements.
///
/// The wire format is TOML. See [`parse_manifest`] for the parser.
///
/// ```toml
/// service     = "fjell-example"
/// sdk_api_rev = 1
/// caps        = ["Endpoint", "AuditDrain"]
/// rights      = ["SEND", "RECV", "AUDIT_DRAIN"]
/// ipc_tags    = ["v0_7::SYNC_*", "tags::READY"]
/// intents     = [0x0101, 0x0102]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapManifest {
    /// Service crate name, e.g. `"fjell-example"`. Must match the
    /// owning Cargo crate's `name` field.
    pub service: String,
    /// `fjell_sdk::SDK_API_REV` the service was built against.
    /// The bundle installer (RFC v0.9-004) refuses bundles whose
    /// `sdk_api_rev` exceeds the running system's `SDK_API_REV`.
    pub sdk_api_rev: u32,
    /// Capability kinds requested. Each entry is a string matching a
    /// `fjell_cap::CapKind` variant name. Unknown names fail the lint.
    pub caps: Vec<String>,
    /// Capability rights requested. Each entry is a `fjell_cap::CapRights`
    /// bit name (e.g. `"SEND"`, `"AUDIT_DRAIN"`).
    pub rights: Vec<String>,
    /// IPC tag globs or fully-qualified paths the service expects to
    /// send / receive. Glob form is documented in §6.3 of RFC v0.9-002.
    pub ipc_tags: Vec<String>,
    /// Semantic catalog tags (`u16`) the service intends to emit.
    /// Lint cross-checks each tag against the v1 catalog.
    pub intents: Vec<u16>,
}

impl CapManifest {
    /// Convenience constructor for empty manifest (lint will reject as
    /// "empty service field"; useful for tests).
    pub fn empty() -> Self {
        Self {
            service: String::new(), sdk_api_rev: 0,
            caps: vec![], rights: vec![], ipc_tags: vec![], intents: vec![],
        }
    }
}

// ── Errors ───────────────────────────────────────────────────────────────────

/// Errors a manifest can produce during parse or lint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// TOML syntax violation. Carries line + column.
    Syntax { line: usize, msg: String },
    /// A required field is missing.
    MissingField(&'static str),
    /// A field's value has the wrong shape.
    BadValue { field: &'static str, msg: String },
    /// A claimed CapKind is not a known variant of `fjell_cap::CapKind`.
    UnknownCapKind(String),
    /// A claimed CapRights bit is not a known constant.
    UnknownRight(String),
    /// A claimed intent tag is not in the v1 catalog.
    UnknownIntent(u16),
    /// `sdk_api_rev` exceeds the SDK rev this lint was built against.
    SdkApiRevTooHigh { manifest: u32, host: u32 },
    /// Internal consistency violation (e.g. duplicate cap).
    Inconsistent(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Syntax { line, msg } =>
                write!(f, "line {}: {}", line, msg),
            ManifestError::MissingField(name) =>
                write!(f, "missing required field `{}`", name),
            ManifestError::BadValue { field, msg } =>
                write!(f, "field `{}`: {}", field, msg),
            ManifestError::UnknownCapKind(s) =>
                write!(f, "unknown CapKind `{}`", s),
            ManifestError::UnknownRight(s) =>
                write!(f, "unknown CapRights bit `{}`", s),
            ManifestError::UnknownIntent(t) =>
                write!(f, "intent tag 0x{:04X} not in catalog v1", t),
            ManifestError::SdkApiRevTooHigh { manifest, host } =>
                write!(f, "manifest sdk_api_rev {} exceeds host {}", manifest, host),
            ManifestError::Inconsistent(s) =>
                write!(f, "inconsistent manifest: {}", s),
        }
    }
}

impl std::error::Error for ManifestError {}

// ── Parser: minimal hand-rolled TOML subset ──────────────────────────────────
//
// The manifest grammar is intentionally narrow: top-level scalars and
// arrays of scalars. A full TOML library is unnecessary and would
// drag in a non-trivial dependency for a few dozen lines of input.

/// Parse a CapManifest from TOML text.
pub fn parse_manifest(input: &str) -> Result<CapManifest, ManifestError> {
    let mut m = CapManifest::empty();
    let mut have_service = false;
    let mut have_rev = false;

    for (line_no_zero, raw) in input.lines().enumerate() {
        let line_no = line_no_zero + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }

        let eq = line.find('=').ok_or(ManifestError::Syntax {
            line: line_no, msg: "expected `key = value`".into(),
        })?;
        let key = line[..eq].trim();
        let value = line[eq + 1..].trim();

        match key {
            "service" => {
                m.service = parse_string(value, line_no, "service")?;
                have_service = true;
            }
            "sdk_api_rev" => {
                m.sdk_api_rev = parse_u32(value, line_no, "sdk_api_rev")?;
                have_rev = true;
            }
            "caps"     => m.caps     = parse_string_array(value, line_no, "caps")?,
            "rights"   => m.rights   = parse_string_array(value, line_no, "rights")?,
            "ipc_tags" => m.ipc_tags = parse_string_array(value, line_no, "ipc_tags")?,
            "intents"  => m.intents  = parse_u16_array(value, line_no, "intents")?,
            other => return Err(ManifestError::BadValue {
                field: "(unknown)",
                msg: format!("line {}: unknown key `{}`", line_no, other),
            }),
        }
    }

    if !have_service { return Err(ManifestError::MissingField("service")); }
    if !have_rev     { return Err(ManifestError::MissingField("sdk_api_rev")); }
    Ok(m)
}

fn parse_string(s: &str, line: usize, field: &'static str) -> Result<String, ManifestError> {
    let s = s.trim();
    if !(s.starts_with('"') && s.ends_with('"') && s.len() >= 2) {
        return Err(ManifestError::BadValue {
            field, msg: format!("line {}: expected quoted string", line),
        });
    }
    Ok(s[1..s.len() - 1].to_string())
}

fn parse_u32(s: &str, line: usize, field: &'static str) -> Result<u32, ManifestError> {
    s.trim().parse::<u32>().map_err(|_| ManifestError::BadValue {
        field, msg: format!("line {}: expected integer", line),
    })
}

fn parse_string_array(s: &str, line: usize, field: &'static str)
    -> Result<Vec<String>, ManifestError>
{
    let s = s.trim();
    if !(s.starts_with('[') && s.ends_with(']')) {
        return Err(ManifestError::BadValue {
            field, msg: format!("line {}: expected array literal", line),
        });
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() { return Ok(vec![]); }
    let mut out = Vec::new();
    for elem in inner.split(',') {
        let elem = elem.trim();
        if elem.is_empty() { continue; }
        out.push(parse_string(elem, line, field)?);
    }
    Ok(out)
}

fn parse_u16_array(s: &str, line: usize, field: &'static str)
    -> Result<Vec<u16>, ManifestError>
{
    let s = s.trim();
    if !(s.starts_with('[') && s.ends_with(']')) {
        return Err(ManifestError::BadValue {
            field, msg: format!("line {}: expected array literal", line),
        });
    }
    let inner = &s[1..s.len() - 1];
    if inner.trim().is_empty() { return Ok(vec![]); }
    let mut out = Vec::new();
    for elem in inner.split(',') {
        let elem = elem.trim();
        if elem.is_empty() { continue; }
        let n = if let Some(hex) = elem.strip_prefix("0x").or_else(|| elem.strip_prefix("0X")) {
            u16::from_str_radix(hex, 16)
        } else {
            elem.parse::<u16>()
        };
        out.push(n.map_err(|_| ManifestError::BadValue {
            field, msg: format!("line {}: bad integer `{}`", line, elem),
        })?);
    }
    Ok(out)
}

// ── Lint ─────────────────────────────────────────────────────────────────────

/// Run the full manifest lint pass.
///
/// Checks performed:
///
/// 1. `service` is non-empty.
/// 2. `sdk_api_rev <= host_sdk_api_rev`.
/// 3. Every name in `caps` resolves to a `CapKind` variant.
/// 4. Every name in `rights` resolves to a `CapRights` bit constant.
/// 5. Every tag in `intents` is present in the v1 semantic catalog.
/// 6. No duplicates in `caps`, `rights`, or `intents`.
///
/// Future v0.9.x patches will add fleet-policy cross-check (RFC v0.9-002 §3
/// goal-line "fleet's CapBrokerPolicy").
pub fn lint_manifest(m: &CapManifest, host_sdk_api_rev: u32) -> Result<(), ManifestError> {
    if m.service.is_empty() {
        return Err(ManifestError::BadValue {
            field: "service", msg: "must be non-empty".into(),
        });
    }
    if m.sdk_api_rev > host_sdk_api_rev {
        return Err(ManifestError::SdkApiRevTooHigh {
            manifest: m.sdk_api_rev, host: host_sdk_api_rev,
        });
    }

    // dedup checks
    let mut caps_set: BTreeSet<&str> = BTreeSet::new();
    for c in &m.caps {
        if !caps_set.insert(c.as_str()) {
            return Err(ManifestError::Inconsistent(format!("duplicate cap `{}`", c)));
        }
    }
    let mut rights_set: BTreeSet<&str> = BTreeSet::new();
    for r in &m.rights {
        if !rights_set.insert(r.as_str()) {
            return Err(ManifestError::Inconsistent(format!("duplicate right `{}`", r)));
        }
    }
    let mut intents_set: BTreeSet<u16> = BTreeSet::new();
    for t in &m.intents {
        if !intents_set.insert(*t) {
            return Err(ManifestError::Inconsistent(format!(
                "duplicate intent 0x{:04X}", t
            )));
        }
    }

    // CapKind name resolution
    for c in &m.caps {
        if !known_cap_kind(c) {
            return Err(ManifestError::UnknownCapKind(c.clone()));
        }
    }

    // CapRights name resolution
    for r in &m.rights {
        if !known_right(r) {
            return Err(ManifestError::UnknownRight(r.clone()));
        }
    }

    // Catalog cross-check
    for t in &m.intents {
        if fjell_semantic_v1::lookup_tag(*t).is_none() {
            return Err(ManifestError::UnknownIntent(*t));
        }
    }
    Ok(())
}

/// Return `true` if `name` is the textual name of a known `CapKind`.
fn known_cap_kind(name: &str) -> bool {
    matches!(name,
        "Endpoint" | "Reply" | "TaskControl" | "TaskCreate" | "TaskInspect" |
        "LeaseAdmin" | "MmioRegion" | "DmaRegion" | "AuditDrain" |
        "BootEvidence" | "Reboot" | "PersistentStore" | "BootControl" |
        "UpgradeTransaction" | "Verification" | "RootfsRead" |
        "SnapshotCreate" | "SnapshotRead" | "CapInstall" |
        "Interrupt" | "NetDevice"
    )
}

/// Return `true` if `name` is a known `CapRights` bit constant.
fn known_right(name: &str) -> bool {
    matches!(name,
        "READ" | "WRITE" | "EXECUTE" | "SEND" | "RECV" | "CALL" | "REPLY" |
        "COPY" | "MINT" | "REVOKE" | "INSPECT" | "DROP" |
        "TASK_CREATE" | "TASK_START" | "TASK_STATUS" | "TASK_KILL" |
        "LEASE_CREATE" | "LEASE_REVOKE" | "LEASE_INSPECT" |
        "MMIO_MAP" | "DMA_ALLOC" | "DMA_USE" | "DMA_REVOKE" |
        "AUDIT_DRAIN" | "BOOT_READ" | "REBOOT" |
        "CAP_INSTALL" | "IRQ_BIND" | "IRQ_UNBIND" | "IRQ_ACK" |
        "NET_SEND" | "NET_RECV"
    )
}

// ── Canonical digest ─────────────────────────────────────────────────────────

/// Compute a stable 16-byte canonical digest of the manifest.
///
/// The digest is the first 16 bytes of SHA-256 over a canonical
/// rendering of the manifest fields, sorted lexicographically. This is
/// what `fjell-bundle-format` (RFC v0.9-004) embeds in the bundle's
/// metadata so a swapped manifest invalidates the bundle signature.
///
/// The hash function lives in `fjell-measure-format`; we re-import via
/// a simple FNV-style fallback here to keep this crate self-contained
/// for the alpha. v0.9.1 swaps in the real SHA-256.
pub fn manifest_digest(m: &CapManifest) -> [u8; 16] {
    let canonical = canonicalize(m);
    let mut h: u128 = 0xCBF29CE484222325;
    for b in canonical.bytes() {
        h ^= b as u128;
        h = h.wrapping_mul(0x100000001B3);
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.to_le_bytes());
    out
}

fn canonicalize(m: &CapManifest) -> String {
    let mut caps = m.caps.clone();    caps.sort();
    let mut rights = m.rights.clone(); rights.sort();
    let mut tags = m.ipc_tags.clone(); tags.sort();
    let mut intents = m.intents.clone(); intents.sort();
    let mut s = String::new();
    s.push_str("svc:"); s.push_str(&m.service); s.push('\n');
    s.push_str("rev:"); s.push_str(&m.sdk_api_rev.to_string()); s.push('\n');
    s.push_str("caps:"); s.push_str(&caps.join(",")); s.push('\n');
    s.push_str("rights:"); s.push_str(&rights.join(",")); s.push('\n');
    s.push_str("ipc:"); s.push_str(&tags.join(",")); s.push('\n');
    s.push_str("int:");
    for (i, t) in intents.iter().enumerate() {
        if i > 0 { s.push(','); }
        s.push_str(&format!("{:04X}", t));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_input() -> &'static str {
"service     = \"fjell-example\"
sdk_api_rev = 1
caps        = [\"Endpoint\", \"AuditDrain\"]
rights      = [\"SEND\", \"RECV\", \"AUDIT_DRAIN\"]
ipc_tags    = [\"v0_7::SYNC_ENVELOPE\"]
intents     = [0x0101, 0x0102]
"
    }

    #[test]
    fn parses_minimal_manifest() {
        let m = parse_manifest(ok_input()).expect("parse");
        assert_eq!(m.service, "fjell-example");
        assert_eq!(m.sdk_api_rev, 1);
        assert_eq!(m.caps, vec!["Endpoint", "AuditDrain"]);
        assert_eq!(m.intents, vec![0x0101, 0x0102]);
    }

    #[test]
    fn lint_passes_for_valid_manifest() {
        let m = parse_manifest(ok_input()).unwrap();
        lint_manifest(&m, 1).expect("lint");
    }

    #[test]
    fn lint_rejects_unknown_cap_kind() {
        let mut m = parse_manifest(ok_input()).unwrap();
        m.caps.push("NotACap".to_string());
        let e = lint_manifest(&m, 1).unwrap_err();
        assert!(matches!(e, ManifestError::UnknownCapKind(_)));
    }

    #[test]
    fn lint_rejects_unknown_right() {
        let mut m = parse_manifest(ok_input()).unwrap();
        m.rights.push("GROOT".to_string());
        let e = lint_manifest(&m, 1).unwrap_err();
        assert!(matches!(e, ManifestError::UnknownRight(_)));
    }

    #[test]
    fn lint_rejects_unknown_intent() {
        let mut m = parse_manifest(ok_input()).unwrap();
        m.intents.push(0xFFFE);
        let e = lint_manifest(&m, 1).unwrap_err();
        assert!(matches!(e, ManifestError::UnknownIntent(0xFFFE)));
    }

    #[test]
    fn lint_rejects_future_sdk_rev() {
        let mut m = parse_manifest(ok_input()).unwrap();
        m.sdk_api_rev = 9999;
        let e = lint_manifest(&m, 1).unwrap_err();
        assert!(matches!(e, ManifestError::SdkApiRevTooHigh { manifest: 9999, host: 1 }));
    }

    #[test]
    fn lint_rejects_duplicate_cap() {
        let mut m = parse_manifest(ok_input()).unwrap();
        m.caps.push("Endpoint".to_string());
        let e = lint_manifest(&m, 1).unwrap_err();
        assert!(matches!(e, ManifestError::Inconsistent(_)));
    }

    #[test]
    fn parse_rejects_unquoted_string() {
        let bad = "service = fjell-example\nsdk_api_rev = 1\n";
        assert!(parse_manifest(bad).is_err());
    }

    #[test]
    fn parse_rejects_missing_service() {
        let bad = "sdk_api_rev = 1\n";
        let e = parse_manifest(bad).unwrap_err();
        assert!(matches!(e, ManifestError::MissingField("service")));
    }

    #[test]
    fn parse_accepts_comments_and_blank_lines() {
        let s = "
# leading comment
service = \"x\"
# mid comment

sdk_api_rev = 0
";
        let m = parse_manifest(s).unwrap();
        assert_eq!(m.service, "x");
    }

    #[test]
    fn digest_is_canonical_under_reordering() {
        let mut a = parse_manifest(ok_input()).unwrap();
        let mut b = a.clone();
        b.caps.reverse();
        b.intents.reverse();
        assert_eq!(manifest_digest(&a), manifest_digest(&b),
            "digest must be order-independent within fields");

        a.service = "different".to_string();
        assert_ne!(manifest_digest(&a), manifest_digest(&b));
    }

    #[test]
    fn digest_is_deterministic() {
        let m = parse_manifest(ok_input()).unwrap();
        let d1 = manifest_digest(&m);
        let d2 = manifest_digest(&m);
        assert_eq!(d1, d2);
    }
}
