//! Canonical digest computation for fleet format types.

use fjell_measure_format::Digest32;
use crate::roster::NodeRoster;
use crate::policy::FleetPolicy;

/// Domain-separated magic for fleet roster digest.
const FLEET_ROSTER_DOMAIN: &[u8] = b"FJELL-FLEET-ROSTER-V1";
/// Domain-separated magic for fleet policy digest.
const FLEET_POLICY_DOMAIN: &[u8] = b"FJELL-FLEET-POLICY-V1";

/// Compute the canonical digest of a `NodeRoster`.
pub fn roster_digest(r: &NodeRoster) -> Digest32 {
    let mut data = [0u8; 512];
    let mut pos = 0usize;

    fn write_bytes(buf: &mut [u8], pos: &mut usize, src: &[u8]) {
        let n = src.len().min(buf.len() - *pos);
        buf[*pos..*pos + n].copy_from_slice(&src[..n]);
        *pos += n;
    }
    fn write_u16(buf: &mut [u8], pos: &mut usize, v: u16) {
        write_bytes(buf, pos, &v.to_le_bytes());
    }
    fn write_u32(buf: &mut [u8], pos: &mut usize, v: u32) {
        write_bytes(buf, pos, &v.to_le_bytes());
    }

    write_bytes(&mut data, &mut pos, FLEET_ROSTER_DOMAIN);
    write_u16(&mut data, &mut pos, r.schema_version);
    write_bytes(&mut data, &mut pos, &r.fleet_id);
    write_u32(&mut data, &mut pos, r.generation);
    write_bytes(&mut data, &mut pos, &r.anchor_pubkey);
    write_u16(&mut data, &mut pos, r.entry_count);
    for e in &r.entries[..r.entry_count as usize] {
        write_bytes(&mut data, &mut pos, &e.identity_digest.0);
        write_bytes(&mut data, &mut pos, &e.node_id.0);
        write_bytes(&mut data, &mut pos, &[e.trust_profile_tag.0, e.active as u8]);
        write_u32(&mut data, &mut pos, e.generation);
    }
    Digest32::of(&data[..pos])
}

/// Compute the canonical digest of a `FleetPolicy`.
pub fn policy_digest(p: &FleetPolicy) -> Digest32 {
    let mut data = [0u8; 256];
    let mut pos = 0usize;

    fn wb(buf: &mut [u8], pos: &mut usize, src: &[u8]) {
        let n = src.len().min(buf.len() - *pos);
        buf[*pos..*pos + n].copy_from_slice(&src[..n]);
        *pos += n;
    }
    fn wu16(buf: &mut [u8], pos: &mut usize, v: u16) { wb(buf, pos, &v.to_le_bytes()); }
    fn wu32(buf: &mut [u8], pos: &mut usize, v: u32) { wb(buf, pos, &v.to_le_bytes()); }

    wb(&mut data, &mut pos, FLEET_POLICY_DOMAIN);
    wu16(&mut data, &mut pos, p.schema_version);
    wb(&mut data, &mut pos, &p.fleet_id);
    wu32(&mut data, &mut pos, p.policy_generation);
    wb(&mut data, &mut pos, &p.roster_digest.0);
    wu16(&mut data, &mut pos, p.statement_count);
    for stmt in &p.statements[..p.statement_count as usize] {
        if let Some(s) = stmt {
            wb(&mut data, &mut pos, &[s.action as u8, s.condition as u8, s.allow as u8]);
            wu16(&mut data, &mut pos, s.audit_tag);
        }
    }
    Digest32::of(&data[..pos])
}
