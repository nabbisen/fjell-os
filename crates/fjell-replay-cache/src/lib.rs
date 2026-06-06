//! # `fjell-replay-cache`
//!
//! Implements RFC-v0.11-005: replay-attack defence for attestation records.
//!
//! Two complementary mechanisms:
//!
//! ## 1. Nonce challenge table (`NonceTable`)
//!
//! The verifier issues a 16-byte nonce to each attesting node. The attesting
//! node embeds the nonce in its signed record. The verifier accepts the
//! record only if the nonce is found in the outstanding-challenge table
//! **and** has not yet been consumed. One nonce, one response.
//!
//! ## 2. Sliding-window replay cache (`ReplayCache`)
//!
//! For async measurement reports where pre-issued nonces are impractical,
//! a bounded map from `RecordId → Timestamp` rejects duplicates seen
//! within the configured window. RecordId is the first 16 bytes of the
//! `Digest32` of the canonical record bytes.
//!
//! The cache is **not** persisted. After a verifier reboot the cache is
//! empty. Nonce-based challenges are the primary mechanism; the cache
//! is defence in depth.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use fjell_measure_format::Digest32;

// ── Types ─────────────────────────────────────────────────────────────────────

/// A 16-byte nonce issued to an attesting node.
pub type Nonce = [u8; 16];

/// A 16-byte record identifier (first 16 bytes of SHA-256 of canonical record).
pub type RecordId = [u8; 16];

/// Monotonic "tick" counter — ns since verifier boot.  Not wall-clock.
pub type Tick = u64;

/// Outcome of a nonce-challenge verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceResult {
    /// Nonce is valid and has been consumed.
    Ok,
    /// Nonce is unknown — never issued or already consumed.
    UnknownOrConsumed,
    /// Nonce is valid but past its window — response arrived too late.
    Expired,
}

/// Outcome of a replay-cache check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheResult {
    /// Record not seen in the window — accept.
    Fresh,
    /// Record seen in the window — replay detected.
    Replay,
    /// Record is outside the cache window — treat as fresh but note staleness.
    OutsideWindow,
}

// ── Nonce table ───────────────────────────────────────────────────────────────

/// Maximum number of outstanding nonce challenges.
pub const NONCE_TABLE_CAP: usize = 256;

/// One outstanding nonce challenge.
#[derive(Clone, Copy)]
struct NonceEntry {
    nonce:      Nonce,
    issued_at:  Tick,
    window_ns:  u64,
    consumed:   bool,
}

/// Tracks outstanding nonce challenges (RFC-v0.11-005 §3).
pub struct NonceTable {
    entries: [Option<NonceEntry>; NONCE_TABLE_CAP],
    len:     usize,
    cursor:  usize,   // eviction cursor (FIFO ring)
}

impl NonceTable {
    pub const fn new() -> Self {
        Self {
            entries: [const { None }; NONCE_TABLE_CAP],
            len:     0,
            cursor:  0,
        }
    }

    /// Issue a new nonce challenge. Returns `None` if the table is full
    /// (caller should evict expired entries first).
    pub fn issue(&mut self, nonce: Nonce, now: Tick, window_ns: u64) -> bool {
        // Evict one expired/consumed entry if full
        if self.len >= NONCE_TABLE_CAP {
            self.evict_one(now);
        }
        if self.len >= NONCE_TABLE_CAP { return false; }

        // Find an empty slot
        for slot in self.entries.iter_mut() {
            if slot.is_none() {
                *slot = Some(NonceEntry { nonce, issued_at: now, window_ns, consumed: false });
                self.len += 1;
                return true;
            }
        }
        false
    }

    /// Verify and consume a nonce. The nonce is consumed on `Ok`.
    pub fn consume(&mut self, nonce: &Nonce, now: Tick) -> NonceResult {
        for slot in self.entries.iter_mut() {
            if let Some(entry) = slot.as_mut() {
                if &entry.nonce != nonce { continue; }
                if entry.consumed { return NonceResult::UnknownOrConsumed; }
                if now.saturating_sub(entry.issued_at) > entry.window_ns {
                    entry.consumed = true;
                    return NonceResult::Expired;
                }
                entry.consumed = true;
                self.len = self.len.saturating_sub(1);
                return NonceResult::Ok;
            }
        }
        NonceResult::UnknownOrConsumed
    }

    /// Evict expired and consumed entries to reclaim space.
    pub fn evict_expired(&mut self, now: Tick) -> usize {
        let mut count = 0;
        for slot in self.entries.iter_mut() {
            let should_evict = slot.as_ref().map_or(false, |e| {
                e.consumed || now.saturating_sub(e.issued_at) > e.window_ns
            });
            if should_evict {
                *slot = None;
                self.len = self.len.saturating_sub(1);
                count += 1;
            }
        }
        count
    }

    fn evict_one(&mut self, now: Tick) {
        // Try to evict the oldest consumed/expired entry first
        for slot in self.entries.iter_mut() {
            if let Some(entry) = slot.as_ref() {
                let expired = now.saturating_sub(entry.issued_at) > entry.window_ns;
                if entry.consumed || expired {
                    *slot = None;
                    self.len = self.len.saturating_sub(1);
                    return;
                }
            }
        }
        // Nothing expired — evict the oldest by cursor
        if self.entries[self.cursor].is_some() {
            self.entries[self.cursor] = None;
            self.len = self.len.saturating_sub(1);
        }
        self.cursor = (self.cursor + 1) % NONCE_TABLE_CAP;
    }
}

impl Default for NonceTable {
    fn default() -> Self { Self::new() }
}

// ── Replay cache ──────────────────────────────────────────────────────────────

/// Default sliding window: 24 hours in nanoseconds.
pub const DEFAULT_WINDOW_NS: u64 = 24 * 3600 * 1_000_000_000;

/// Default cache capacity.
pub const DEFAULT_CACHE_CAP: usize = 4096;

/// Eviction batch size (1/8 of capacity when full).
const EVICT_BATCH: usize = DEFAULT_CACHE_CAP / 8;

/// One cache entry.
#[derive(Clone, Copy)]
struct CacheEntry {
    id:      RecordId,
    seen_at: Tick,
}

/// Bounded sliding-window replay cache (RFC-v0.11-005 §4).
pub struct ReplayCache {
    entries:   [Option<CacheEntry>; DEFAULT_CACHE_CAP],
    len:       usize,
    window_ns: u64,
}

impl ReplayCache {
    pub const fn new(window_ns: u64) -> Self {
        Self {
            entries: [const { None }; DEFAULT_CACHE_CAP],
            len: 0,
            window_ns,
        }
    }

    /// Build a `RecordId` from raw attestation bytes.
    pub fn record_id(bytes: &[u8]) -> RecordId {
        let d = Digest32::of(bytes);
        d.0[..16].try_into().unwrap()
    }

    /// Check and record an attestation. Returns `Fresh` and inserts if
    /// the record has not been seen within the window.
    pub fn check_and_insert(&mut self, id: RecordId, now: Tick) -> CacheResult {
        // Scan for duplicates
        for slot in self.entries.iter() {
            if let Some(e) = slot.as_ref() {
                if e.id != id { continue; }
                let age = now.saturating_sub(e.seen_at);
                if age <= self.window_ns {
                    return CacheResult::Replay;
                } else {
                    return CacheResult::OutsideWindow;
                }
            }
        }
        // Not seen — insert
        if self.len >= DEFAULT_CACHE_CAP {
            self.evict_oldest(now);
        }
        for slot in self.entries.iter_mut() {
            if slot.is_none() {
                *slot = Some(CacheEntry { id, seen_at: now });
                self.len += 1;
                return CacheResult::Fresh;
            }
        }
        CacheResult::Fresh // fallthrough: table full after eviction
    }

    /// Return the number of entries currently in the cache.
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }

    fn evict_oldest(&mut self, now: Tick) {
        // First pass: evict entries outside the window
        let window = self.window_ns;
        let mut evicted = 0;
        for slot in self.entries.iter_mut() {
            if let Some(e) = slot.as_ref() {
                if now.saturating_sub(e.seen_at) > window {
                    *slot = None;
                    self.len = self.len.saturating_sub(1);
                    evicted += 1;
                    if evicted >= EVICT_BATCH { return; }
                }
            }
        }
        if evicted > 0 { return; }
        // Second pass: evict the EVICT_BATCH oldest by seen_at
        let mut oldest: [Option<(Tick, usize)>; EVICT_BATCH] = [None; EVICT_BATCH];
        for (i, slot) in self.entries.iter().enumerate() {
            if let Some(e) = slot.as_ref() {
                let worst = oldest.iter_mut().max_by_key(|o: &&mut Option<(Tick, usize)>| o.map_or(Tick::MAX, |v| v.0));
                if let Some(w) = worst {
                    let should_replace = w.map_or(true, |v| e.seen_at < v.0);
                    if should_replace {
                        *w = Some((e.seen_at, i));
                    }
                }
            }
        }
        for slot in oldest.iter() {
            if let Some((_, idx)) = slot {
                self.entries[*idx] = None;
                self.len = self.len.saturating_sub(1);
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── NonceTable ─────────────────────────────────────────────────────────────

    #[test]
    fn nonce_issue_and_consume_ok() {
        let mut t = NonceTable::new();
        let nonce = [1u8; 16];
        assert!(t.issue(nonce, 0, 10_000));
        assert_eq!(t.consume(&nonce, 1_000), NonceResult::Ok);
    }

    #[test]
    fn nonce_consumed_twice_rejected() {
        let mut t = NonceTable::new();
        let nonce = [2u8; 16];
        t.issue(nonce, 0, 10_000);
        assert_eq!(t.consume(&nonce, 100), NonceResult::Ok);
        assert_eq!(t.consume(&nonce, 200), NonceResult::UnknownOrConsumed);
    }

    #[test]
    fn nonce_unknown_rejected() {
        let mut t = NonceTable::new();
        let nonce = [3u8; 16];
        // Not issued
        assert_eq!(t.consume(&nonce, 0), NonceResult::UnknownOrConsumed);
    }

    #[test]
    fn nonce_expired_rejected() {
        let mut t = NonceTable::new();
        let nonce = [4u8; 16];
        t.issue(nonce, 0, 1_000);   // window = 1000 ns
        // Arrive at t=5000 — well past window
        assert_eq!(t.consume(&nonce, 5_000), NonceResult::Expired);
    }

    #[test]
    fn nonce_evict_expired_reclaims_space() {
        let mut t = NonceTable::new();
        for i in 0u8..10 {
            t.issue([i; 16], 0, 100);
        }
        // All expire at now=500
        let evicted = t.evict_expired(500);
        assert!(evicted > 0);
    }

    // ── ReplayCache ────────────────────────────────────────────────────────────

    #[test]
    fn fresh_record_accepted() {
        let mut c = ReplayCache::new(DEFAULT_WINDOW_NS);
        let id = [0xAAu8; 16];
        assert_eq!(c.check_and_insert(id, 0), CacheResult::Fresh);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn duplicate_in_window_rejected() {
        let mut c = ReplayCache::new(1_000_000_000);
        let id = [0xBBu8; 16];
        c.check_and_insert(id, 0);
        assert_eq!(c.check_and_insert(id, 100), CacheResult::Replay);
    }

    #[test]
    fn same_id_outside_window_accepted() {
        let mut c = ReplayCache::new(1_000);  // 1000 ns window
        let id = [0xCCu8; 16];
        c.check_and_insert(id, 0);
        // Revisit at t = 5000 — outside window
        assert_eq!(c.check_and_insert(id, 5_000), CacheResult::OutsideWindow);
    }

    #[test]
    fn many_different_records_accepted() {
        let mut c = ReplayCache::new(DEFAULT_WINDOW_NS);
        for i in 0u8..100 {
            let id = [i; 16];
            assert_eq!(c.check_and_insert(id, i as u64 * 1000), CacheResult::Fresh);
        }
        assert_eq!(c.len(), 100);
    }

    #[test]
    fn record_id_from_bytes_deterministic() {
        let bytes = b"test attestation record";
        let id1 = ReplayCache::record_id(bytes);
        let id2 = ReplayCache::record_id(bytes);
        assert_eq!(id1, id2);
        let id3 = ReplayCache::record_id(b"different");
        assert_ne!(id1, id3);
    }

    #[test]
    fn cache_evicts_when_full() {
        // Use a small window so entries expire quickly
        let mut c = ReplayCache::new(100);
        // Fill past capacity using expired timestamps
        for i in 0..(DEFAULT_CACHE_CAP + 100) {
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&(i as u64).to_le_bytes());
            // Use large now so entries look expired
            c.check_and_insert(id, (i as u64 + 1) * 1_000_000);
        }
        // Should not panic or overflow
        assert!(c.len() <= DEFAULT_CACHE_CAP);
    }
}
