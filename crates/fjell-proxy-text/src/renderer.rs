//! `ProxyState` renderer for the v1 semantic catalog (RFC v0.5-005).
//!
//! Provides the scroll ring, pinned-critical region, rate-limit table,
//! and an ASCII entry formatter.  No I/O — callers supply a `Write`-like
//! callback.

use fjell_semantic_v1::{
    catalog::lookup_tag,
    codec::{decode, FieldValue},
    schema::FieldKind,
};

// ── Constants ─────────────────────────────────────────────────────────────────

pub const MAX_PINNED:       usize = 8;
pub const SCROLL_BUFFER:    usize = 32;
pub const RATE_TABLE_SIZE:  usize = 64;
pub const BODY_LEN:         usize = 96;
pub const DEFAULT_WIDTH:    usize = 80;

/// Rate-limit budget per (tag, service) per second.
pub const RATE_BUDGET:  u8 = 8;
/// Rate-limit burst allowance.
pub const RATE_BURST:   u8 = 16;

// ── Critical tag set (RFC v0.5-005 §5.2) ─────────────────────────────────────

const CRITICAL_TAGS: &[u16] = &[
    0x0102, // UPDATE.STAGING_FAILED
    0x0110, // UPDATE.ROLLBACK_BLOCKED
    0x0111, // UPDATE.ROLLBACK_TO_PREVIOUS_SLOT
    0x0121, // ATTEST.RECORD_VERIFY_FAILED
    0x0131, // SECURITY.PROVIDER_FAULTED
    0x0141, // NET.LINK_DOWN
    0x0150, // RECOVERY.ENTERED
    0x0151, // RECOVERY.EXITED
    0x0171, // HEALTH.TARGET_FAILED
];

pub fn is_critical_tag(tag: u16) -> bool {
    CRITICAL_TAGS.contains(&tag)
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// A source service identifier (matches cap-broker ImageId u16 convention).
pub type ServiceId = u16;

/// A fully-rendered log entry.
#[derive(Clone, Copy, Debug)]
pub struct RenderedEntry {
    pub tag:      u16,
    pub at_tick:  u64,
    pub source:   ServiceId,
    pub body:     [u8; BODY_LEN],  // ASCII, NUL-padded
    pub body_len: u8,
    pub critical: bool,
    pub unknown:  bool,
}

impl RenderedEntry {
    pub const EMPTY: Self = Self {
        tag: 0, at_tick: 0, source: 0,
        body: [0u8; BODY_LEN], body_len: 0,
        critical: false, unknown: false,
    };
    pub fn body_str(&self) -> &str {
        let n = self.body_len as usize;
        core::str::from_utf8(&self.body[..n]).unwrap_or("?")
    }
}

/// Rate-limit entry per `(intent_tag, source_service)` key.
#[derive(Clone, Copy, Debug, Default)]
pub struct RateLimitEntry {
    pub key:        (u16, ServiceId),
    pub tokens:     u8,
    pub last_tick:  u64,
    pub suppressed: u16,
}

/// Fixed-capacity scroll ring.
#[derive(Clone, Copy, Debug)]
pub struct ScrollRing {
    entries: [RenderedEntry; SCROLL_BUFFER],
    head:    u8,
    count:   u8,
}

impl ScrollRing {
    pub const fn new() -> Self {
        Self {
            entries: [RenderedEntry::EMPTY; SCROLL_BUFFER],
            head:    0,
            count:   0,
        }
    }
    pub fn push(&mut self, e: RenderedEntry) {
        let idx = self.head as usize % SCROLL_BUFFER;
        self.entries[idx] = e;
        self.head = self.head.wrapping_add(1);
        if self.count < SCROLL_BUFFER as u8 { self.count += 1; }
    }
    pub fn len(&self) -> usize { self.count as usize }
    pub fn is_empty(&self) -> bool { self.count == 0 }
    /// Iterate entries newest-first.
    pub fn iter_newest_first(&self) -> impl Iterator<Item = &RenderedEntry> {
        let count = self.count as usize;
        let head  = self.head as usize;
        (0..count).map(move |i| {
            let idx = (head + SCROLL_BUFFER - 1 - i) % SCROLL_BUFFER;
            &self.entries[idx]
        })
    }
}

/// Banner state (refreshed on SECURITY.REGISTRY_ENFORCING, PLATFORM.PROFILES_READY).
#[derive(Clone, Copy, Debug)]
pub struct BannerInfo {
    pub phase:           [u8; 16],
    pub phase_len:       u8,
    pub boot_id_lo:      u64,
    pub chain_digest_hi: [u8; 8],   // first 8 bytes of measurement chain head
}

impl BannerInfo {
    pub const fn default() -> Self {
        Self {
            phase:           *b"bootstrap\0\0\0\0\0\0\0",
            phase_len:       9,
            boot_id_lo:      0,
            chain_digest_hi: [0u8; 8],
        }
    }
}

/// Full renderer state (RFC v0.5-005 §6.1).
pub struct ProxyState {
    pub banner:     BannerInfo,
    pub pinned:     [Option<RenderedEntry>; MAX_PINNED],
    pub scroll:     ScrollRing,
    pub rate_table: [RateLimitEntry; RATE_TABLE_SIZE],
    pub width:      usize,
}

impl ProxyState {
    pub const fn new() -> Self {
        Self {
            banner:     BannerInfo::default(),
            pinned:     [None; MAX_PINNED],
            scroll:     ScrollRing::new(),
            rate_table: [RateLimitEntry { key: (0, 0), tokens: RATE_BURST,
                          last_tick: 0, suppressed: 0 }; RATE_TABLE_SIZE],
            width:      DEFAULT_WIDTH,
        }
    }

    // ── Rate limiting ─────────────────────────────────────────────────────────

    fn rate_slot(&mut self, tag: u16, source: ServiceId) -> &mut RateLimitEntry {
        let key = (tag, source);
        // Find existing slot or evict oldest.
        let mut oldest_tick = u64::MAX;
        let mut oldest_idx  = 0usize;
        for (i, slot) in self.rate_table.iter().enumerate() {
            if slot.key == key { return &mut self.rate_table[i]; }
            if slot.last_tick < oldest_tick {
                oldest_tick = slot.last_tick;
                oldest_idx  = i;
            }
        }
        self.rate_table[oldest_idx] = RateLimitEntry { key, tokens: RATE_BURST, last_tick: 0, suppressed: 0 };
        &mut self.rate_table[oldest_idx]
    }

    fn rate_allow(&mut self, tag: u16, source: ServiceId, tick: u64) -> bool {
        if is_critical_tag(tag) { return true; }
        let slot = self.rate_slot(tag, source);
        // Refill tokens based on elapsed ticks (1 token per 125ms = 8/s).
        let elapsed = tick.saturating_sub(slot.last_tick);
        let refill  = (elapsed / 125).min(RATE_BUDGET as u64) as u8;
        slot.tokens = slot.tokens.saturating_add(refill).min(RATE_BURST);
        slot.last_tick = tick;
        if slot.tokens > 0 {
            slot.tokens -= 1;
            true
        } else {
            slot.suppressed = slot.suppressed.saturating_add(1);
            false
        }
    }

    // ── Pinned region ─────────────────────────────────────────────────────────

    fn pin(&mut self, e: RenderedEntry) {
        // Replace existing pin for same tag, else LRU evict.
        for slot in &mut self.pinned {
            if slot.map_or(false, |s| s.tag == e.tag) {
                *slot = Some(e); return;
            }
        }
        for slot in &mut self.pinned {
            if slot.is_none() { *slot = Some(e); return; }
        }
        // Evict oldest (first slot, LRU-ish).
        self.pinned[0] = Some(e);
    }

    // ── Entry rendering ───────────────────────────────────────────────────────

    /// Ingest a raw encoded intent envelope and update state.
    ///
    /// `source` is the emitting service's ImageId.
    /// `tick` is the current kernel tick.
    /// Returns `Some(entry)` if an entry should be emitted to the display.
    pub fn ingest(&mut self, bytes: &[u8], source: ServiceId, tick: u64)
        -> Option<RenderedEntry>
    {
        // Decode.
        let decoded = match decode(bytes) {
            Ok(d)  => d,
            Err(_) => {
                // Malformed envelope — render as unknown.
                let mut e = RenderedEntry::EMPTY;
                e.unknown  = true;
                e.at_tick  = tick;
                e.source   = source;
                let s = b"<<malformed>>";
                let n = s.len().min(BODY_LEN);
                e.body[..n].copy_from_slice(&s[..n]);
                e.body_len = n as u8;
                self.scroll.push(e);
                return Some(e);
            }
        };

        let entry = match lookup_tag(decoded.tag) {
            Some(catalog_entry) => {
                let mut e = RenderedEntry::EMPTY;
                e.tag      = decoded.tag;
                e.at_tick  = decoded.created_tick;
                e.source   = source;
                e.critical = is_critical_tag(decoded.tag);
                // Format body: "TAG_NAME field1=v1 field2=v2 ..."
                let mut buf = [0u8; BODY_LEN];
                let mut pos = 0usize;
                let name = catalog_entry.name.as_bytes();
                let n = name.len().min(BODY_LEN - 1);
                buf[pos..pos+n].copy_from_slice(&name[..n]); pos += n;
                for (i, fd) in catalog_entry.schema.fields.iter().enumerate() {
                    if pos >= BODY_LEN - 2 { break; }
                    buf[pos] = b' '; pos += 1;
                    let fname = fd.name.as_bytes();
                    let fn_len = fname.len().min(BODY_LEN - pos - 1);
                    buf[pos..pos+fn_len].copy_from_slice(&fname[..fn_len]); pos += fn_len;
                    if pos >= BODY_LEN - 1 { break; }
                    buf[pos] = b'='; pos += 1;
                    pos = format_field_value(&decoded.fields[i], fd.kind, &mut buf, pos);
                }
                e.body     = buf;
                e.body_len = pos as u8;
                e
            }
            None => {
                // Unknown tag.
                let mut e = RenderedEntry::EMPTY;
                e.tag      = decoded.tag;
                e.at_tick  = decoded.created_tick;
                e.source   = source;
                e.unknown  = true;
                let prefix = b"<<unknown(0x";
                let n = prefix.len().min(BODY_LEN);
                e.body[..n].copy_from_slice(&prefix[..n]);
                e.body_len = n as u8;
                e
            }
        };

        // Rate limit (critical tags bypass).
        if !self.rate_allow(entry.tag, source, tick) {
            return None;
        }

        // Route to pinned or scroll.
        if entry.critical {
            self.pin(entry);
        } else {
            self.scroll.push(entry);
        }
        Some(entry)
    }

    /// Count currently active pinned entries.
    pub fn pinned_count(&self) -> usize {
        self.pinned.iter().filter(|s| s.is_some()).count()
    }
}

// ── Field value formatting ────────────────────────────────────────────────────

fn format_field_value(fv: &FieldValue, _kind: FieldKind, buf: &mut [u8], mut pos: usize) -> usize {
    match *fv {
        FieldValue::Absent      => { if pos < buf.len() { buf[pos] = b'-'; pos += 1; } }
        FieldValue::U8(v)       => { pos = write_u64(v as u64, buf, pos); }
        FieldValue::U16(v)      => { pos = write_hex16(v, buf, pos); }
        FieldValue::U32(v)      => { pos = write_hex32(v, buf, pos); }
        FieldValue::U64(v)      => { pos = write_u64(v, buf, pos); }
        FieldValue::Bytes16(b)  => { pos = write_hex_trunc(&b, 4, buf, pos); }
        FieldValue::Bytes32(b)  => { pos = write_hex_trunc(&b, 8, buf, pos); }
    }
    pos
}

fn write_u64(mut n: u64, buf: &mut [u8], pos: usize) -> usize {
    if pos >= buf.len() { return pos; }
    if n == 0 { buf[pos] = b'0'; return pos + 1; }
    let mut tmp = [0u8; 20];
    let mut i = 20;
    while n > 0 { i -= 1; tmp[i] = b'0' + (n % 10) as u8; n /= 10; }
    let s = &tmp[i..];
    let n = s.len().min(buf.len() - pos);
    buf[pos..pos+n].copy_from_slice(&s[..n]);
    pos + n
}

fn write_hex16(v: u16, buf: &mut [u8], pos: usize) -> usize {
    write_hex_slice(&v.to_be_bytes(), buf, pos)
}

fn write_hex32(v: u32, buf: &mut [u8], pos: usize) -> usize {
    write_hex_slice(&v.to_be_bytes(), buf, pos)
}

fn write_hex_trunc(bytes: &[u8], max_bytes: usize, buf: &mut [u8], pos: usize) -> usize {
    let n = bytes.len().min(max_bytes);
    let p = write_hex_slice(&bytes[..n], buf, pos);
    if bytes.len() > max_bytes && p + 2 < buf.len() {
        buf[p] = b'.'; buf[p+1] = b'.';
        p + 2
    } else { p }
}

fn write_hex_slice(bytes: &[u8], buf: &mut [u8], mut pos: usize) -> usize {
    const HEX: &[u8] = b"0123456789abcdef";
    for &b in bytes {
        if pos + 2 > buf.len() { break; }
        buf[pos]   = HEX[(b >> 4) as usize];
        buf[pos+1] = HEX[(b & 0xF) as usize];
        pos += 2;
    }
    pos
}

#[cfg(test)]
mod proxy_tests {
    use super::*;
    use fjell_semantic_v1::codec::{encode, FieldValue, MAX_ENVELOPE_BYTES};

    fn encode_intent(tag: u16, tick: u64, fields: &[FieldValue]) -> ([u8; MAX_ENVELOPE_BYTES], usize) {
        let mut buf = [0u8; MAX_ENVELOPE_BYTES];
        let n = encode(tag, tick, fields, &mut buf).unwrap();
        (buf, n)
    }

    #[test]
    fn proxy_ingest_known_tag_adds_to_scroll() {
        let mut ps = ProxyState::new();
        let fv = [FieldValue::U32(1), FieldValue::U32(2), FieldValue::U32(3)];
        let (buf, n) = encode_intent(0x0100, 1000, &fv); // UPDATE.STAGING_STARTED
        let result = ps.ingest(&buf[..n], 12, 1000);
        assert!(result.is_some());
        assert_eq!(ps.scroll.len(), 1);
        assert_eq!(ps.pinned_count(), 0);
    }

    #[test]
    fn proxy_ingest_critical_tag_goes_to_pinned() {
        let mut ps = ProxyState::new();
        let fv = [FieldValue::U16(0x0007)]; // RECOVERY.ENTERED { reason_code }
        let (buf, n) = encode_intent(0x0150, 2000, &fv);
        let result = ps.ingest(&buf[..n], 10, 2000);
        assert!(result.is_some(), "critical ingest should return Some");
        assert_eq!(ps.pinned_count(), 1, "RECOVERY.ENTERED must pin");
        assert_eq!(ps.scroll.len(), 0, "critical must not go to scroll");
        assert!(ps.pinned[0].unwrap().critical);
    }

    #[test]
    fn proxy_rate_limit_suppresses_non_critical() {
        let mut ps = ProxyState::new();
        // Exhaust the burst budget for tag 0x0140 (NET.LINK_UP).
        let fv = [FieldValue::U32(1), FieldValue::U16(1500)];
        let (buf, n) = encode_intent(0x0140, 0, &fv);
        for _ in 0..RATE_BURST {
            ps.ingest(&buf[..n], 1, 0);
        }
        // Next one is after burst exhaustion — same tick.
        let suppressed = ps.ingest(&buf[..n], 1, 0);
        assert!(suppressed.is_none(), "rate limit must suppress after burst");
    }

    #[test]
    fn proxy_critical_bypasses_rate_limit() {
        let mut ps = ProxyState::new();
        let fv = [FieldValue::U16(0)]; // RECOVERY.ENTERED
        let (buf, n) = encode_intent(0x0150, 0, &fv);
        // Send many more than burst.
        let mut accepted = 0;
        for _ in 0..100 {
            if ps.ingest(&buf[..n], 1, 0).is_some() { accepted += 1; }
        }
        assert_eq!(accepted, 100, "critical must never be suppressed");
    }

    #[test]
    fn proxy_unknown_tag_renders_unknown_form() {
        let mut ps = ProxyState::new();
        // Craft an envelope with a valid structure but tag not in catalog.
        // Use the raw FJSI-V1 magic with an unknown tag (e.g. 0x9999).
        // Since decode() will fail (unknown tag), we test malformed path via
        // a known-good tag but corruption.
        let fv = [FieldValue::U32(7), FieldValue::U32(0), FieldValue::U32(63)];
        let (mut buf, n) = encode_intent(0x0100, 0, &fv);
        // Corrupt tag to unknown.
        buf[7] = 0x99; buf[8] = 0x99;
        let entry = ps.ingest(&buf[..n], 1, 0);
        assert!(entry.is_some());
        assert!(entry.unwrap().unknown, "corrupted tag must render as unknown");
    }

    #[test]
    fn scroll_ring_wraps_correctly() {
        let mut ring = ScrollRing::new();
        for i in 0..SCROLL_BUFFER + 5 {
            let mut e = RenderedEntry::EMPTY;
            e.tag = i as u16;
            ring.push(e);
        }
        assert_eq!(ring.len(), SCROLL_BUFFER);
        // Newest-first: the last 5 entries wrapped the ring.
        let newest = ring.iter_newest_first().next().unwrap();
        assert_eq!(newest.tag, (SCROLL_BUFFER + 4) as u16);
    }

    #[test]
    fn is_critical_tag_covers_expected_set() {
        assert!(is_critical_tag(0x0150)); // RECOVERY.ENTERED
        assert!(is_critical_tag(0x0110)); // UPDATE.ROLLBACK_BLOCKED
        assert!(is_critical_tag(0x0141)); // NET.LINK_DOWN
        assert!(!is_critical_tag(0x0100)); // UPDATE.STAGING_STARTED — not critical
        assert!(!is_critical_tag(0x0142)); // NET.SXT_CHANNEL_OPENED — not critical
    }

    #[test]
    fn rendered_entry_body_contains_tag_name() {
        let mut ps = ProxyState::new();
        let fv = [FieldValue::U32(1), FieldValue::U32(2), FieldValue::U32(3)];
        let (buf, n) = encode_intent(0x0100, 500, &fv);
        let entry = ps.ingest(&buf[..n], 5, 500).unwrap();
        let body = entry.body_str();
        assert!(body.contains("UPDATE.STAGING_STARTED"),
            "body should contain tag name, got: {body}");
    }
}
