//! Canonical digest computation for fleet format types.
//!
//! Each digest is taken over a stream written through `Canon`
//! (RFC-0.33-003): the same function is what `fjell-schema` records for the frozen
//! description. The buffers are sized from the capacity constants, so **no member
//! or statement can fall outside the digest** — the streams used to be built in
//! fixed 512- and 256-byte buffers by a writer that silently dropped what did not
//! fit, and a roster of more than eight members was digested over its first eight
//! (E-066).

use crate::policy::{FleetPolicy, MAX_POLICY_STATEMENTS};
use crate::roster::{MAX_ROSTER_ENTRIES, NodeRoster};
use fjell_canon::{BufSink, Canon};
use fjell_measure_format::Digest32;

/// Domain-separated magic for fleet roster digest.
const FLEET_ROSTER_DOMAIN: &[u8] = b"FJELL-FLEET-ROSTER-V1";
/// Domain-separated magic for fleet policy digest.
const FLEET_POLICY_DOMAIN: &[u8] = b"FJELL-FLEET-POLICY-V1";

/// Bytes of one roster entry: identity digest, node id, tag, active, generation.
const ROSTER_ENTRY_BYTES: usize = 32 + 16 + 1 + 1 + 4;
/// The roster stream at capacity: domain, version, fleet id, generation, anchor,
/// count, then every entry.
const ROSTER_STREAM_MAX: usize = 21 + 2 + 16 + 4 + 32 + 2 + MAX_ROSTER_ENTRIES * ROSTER_ENTRY_BYTES;
/// Bytes of one policy statement: action, condition, allow, audit tag.
const STATEMENT_BYTES: usize = 1 + 1 + 1 + 2;
/// The policy stream at capacity.
const POLICY_STREAM_MAX: usize = 21 + 2 + 16 + 4 + 32 + 2 + MAX_POLICY_STATEMENTS * STATEMENT_BYTES;

/// The canonical byte stream a roster's digest is taken over — **the function the
/// digest is computed from and the frozen schema is generated from**.
pub fn write_roster_canonical(r: &NodeRoster, c: &mut dyn Canon) {
    c.domain(FLEET_ROSTER_DOMAIN);
    c.u16("schema_version", r.schema_version);
    c.bytes("fleet_id", &r.fleet_id);
    c.u32("generation", r.generation);
    c.bytes("anchor_pubkey", &r.anchor_pubkey);
    c.u16("entry_count", r.entry_count);
    c.each(
        "entries",
        r.entry_count as usize,
        Some(MAX_ROSTER_ENTRIES),
        &mut |c, i| {
            let e = &r.entries[i];
            c.bytes("identity_digest", &e.identity_digest.0);
            c.bytes("node_id", &e.node_id.0);
            c.u8("trust_profile_tag", e.trust_profile_tag.0);
            c.u8("active", e.active as u8);
            c.u32("generation", e.generation);
        },
    );
}

/// The canonical byte stream a policy's digest is taken over. A statement slot
/// that is `None` writes nothing, as it always did.
pub fn write_policy_canonical(p: &FleetPolicy, c: &mut dyn Canon) {
    c.domain(FLEET_POLICY_DOMAIN);
    c.u16("schema_version", p.schema_version);
    c.bytes("fleet_id", &p.fleet_id);
    c.u32("policy_generation", p.policy_generation);
    c.bytes("roster_digest", &p.roster_digest.0);
    c.u16("statement_count", p.statement_count);
    c.each(
        "statements",
        p.statement_count as usize,
        Some(MAX_POLICY_STATEMENTS),
        &mut |c, i| {
            if let Some(s) = &p.statements[i] {
                c.u8("action", s.action as u8);
                c.u8("condition", s.condition as u8);
                c.u8("allow", s.allow as u8);
                c.u16("audit_tag", s.audit_tag);
            }
        },
    );
}

/// Compute the canonical digest of a `NodeRoster`.
pub fn roster_digest(r: &NodeRoster) -> Digest32 {
    let mut sink = BufSink::<{ ROSTER_STREAM_MAX }>::new();
    write_roster_canonical(r, &mut sink);
    Digest32::of(sink.bytes())
}

/// Compute the canonical digest of a `FleetPolicy`.
pub fn policy_digest(p: &FleetPolicy) -> Digest32 {
    let mut sink = BufSink::<{ POLICY_STREAM_MAX }>::new();
    write_policy_canonical(p, &mut sink);
    Digest32::of(sink.bytes())
}
