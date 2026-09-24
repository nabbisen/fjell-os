//! Per-presentation queue and credit engine for the semantic stream
//! (RFC-0.34-001 D8).
//!
//! # Why this exists
//!
//! `semantic-stream` used to forward each envelope to `proxy-text` with a
//! blocking IPC call before it replied to the publisher, so an absent or dead
//! presentation stopped the publisher (E-058, measured). The kernel has no
//! non-blocking send, and a server holds one reply edge, not several, so the
//! only IPC a service can issue that never blocks it is a **reply**. This
//! engine is the state behind that: the stream never *sends* to a presentation
//! (bar one wake, see below), it **answers** presentations that ask.
//!
//! A presentation asks for its next message ([`Fanout::next`]) and is answered
//! at once, either with the next message of the oldest queued envelope or with
//! [`Next::Empty`] — after which it is *parked* and the stream wakes it once,
//! when the next envelope for it arrives ([`Offer::Queued`]'s `wake`).
//!
//! # The policy (RFC-0.34-001 handoff §2)
//!
//! * **Publishers are not told about presentations.** Nothing here returns
//!   anything a publisher's reply could depend on: [`Fanout::offer`] and
//!   [`Fanout::next`] never block and never wait for a presentation. That is
//!   the property the tests below pin, including with presentations that never
//!   ask.
//! * **Undeliverable envelopes are queued with a bound, then dropped.** Each
//!   presentation has a byte ring of `N` bytes holding length-prefixed
//!   envelopes in arrival order. An envelope that does not fit **is dropped**
//!   (drop-newest: it keeps order and never disturbs an envelope already
//!   partly handed out) and counted. A queue with no bound would be a third
//!   failure mode, so there is exactly one number and one place it is
//!   enforced.
//! * **Absence stays observable, before anything is lost.** A queue that holds
//!   a whole boot's traffic would make a presentation that is gone silent until
//!   the bound is hit, so the engine reports two things as [`Report`]s: a
//!   presentation that has not asked for [`BEHIND_AFTER`] envelopes
//!   ([`Report::Behind`], then at each doubling), and drops
//!   ([`Report::NotTaking`], at the first and at each power of two after it).
//!   An unbounded fault costs `log2(n)` lines. A presentation that then empties
//!   its queue reports [`Report::Resumed`].
//!
//! # What this does not do
//!
//! It cannot tell a presentation that is dead from one that is alive and
//! not asking: there is no timer to say how long is too long. Both look like a
//! full queue and a rising drop count, which is observable and correct.
//!
//! No allocation, no `unsafe`, no syscalls.

#![no_std]

/// Bytes of an envelope carried by one `Chunk` message: four 8-byte words.
pub const CHUNK_BYTES: usize = 32;

/// A presentation that has not asked for this many envelopes is reported
/// [`Report::Behind`]. Chosen so that a presentation busy between two questions
/// is not reported (cooperative scheduling means nothing is offered while it
/// runs) and one that has stopped is, well inside the queue's bound: a typical
/// envelope is a few hundred bytes, so eight is far short of overflowing 8 KiB.
pub const BEHIND_AFTER: u64 = 8;

/// Bytes of length prefix in front of each queued envelope.
const LEN_PREFIX: usize = 2;

/// The bound: bytes each presentation may have queued. **The one number** the
/// stream's undeliverable-envelope policy rests on. The widest envelope the
/// wire format can produce is `MAX_WIRE_BYTES` (4,624 bytes today), so this
/// holds the widest envelope with room left over — an envelope can always be
/// delivered into an empty queue — and, typically, dozens of ordinary ones (a
/// test, `the_bound_holds_the_widest_envelope`, checks that against
/// `MAX_WIRE_BYTES` rather than trusting this comment). It does **not** hold
/// two widest envelopes at once; a burst of those is what gets dropped. Two
/// rings cost 16 KiB of the stream's 64 KiB stack, since a service image
/// cannot write to a static.
pub const RING_BYTES: usize = 8192;

/// Which step of an envelope's transfer a [`Msg`] is. These are the same three
/// steps `fjell_service_api::chunked` frames (`BEGIN`/`CHUNK…`/`COMMIT`), so a
/// presentation reassembles with the same `Reassembler` it always used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `words[0]` is the declared length in bytes; the rest are zero.
    Begin,
    /// `words` are the next 32 bytes, zero-padded in the final chunk.
    Chunk,
    /// All zero.
    Commit,
}

/// One message of an envelope's transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Msg {
    pub kind: Kind,
    pub words: [u64; 4],
}

/// Something worth saying on the node's own output about one presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Report {
    /// `unasked` envelopes have been offered since the presentation last asked
    /// (the [`BEHIND_AFTER`]th, and each doubling after it). Nothing has been
    /// lost yet. `never_asked` is true if it has not asked for anything at all.
    Behind { unasked: u64, never_asked: bool },
    /// `dropped` envelopes have been dropped in this run (the first, and each
    /// power of two after it). `never_asked` is true if the presentation has
    /// not asked for anything yet — it may not exist.
    NotTaking { dropped: u64, never_asked: bool },
    /// The presentation emptied its queue after dropping `dropped` envelopes
    /// while it was behind or away.
    Resumed { dropped: u64 },
}

/// What [`Fanout::offer`] did with an envelope for one presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// It is queued. `wake` is true exactly when the presentation was parked
    /// and must now be woken (once): the caller sends the wake, and it is the
    /// only send the stream ever makes to a presentation. `report` is set when
    /// the presentation has become [`Report::Behind`].
    Queued { wake: bool, report: Option<Report> },
    /// It did not fit and was dropped. `report` is set when this drop is the
    /// first, or a power of two, of the current run.
    Dropped { report: Option<Report> },
}

/// What [`Fanout::next`] answers a presentation that asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    Message(Msg),
    /// Nothing queued. The presentation is now parked.
    Empty,
}

/// A byte ring of length-prefixed envelopes.
struct Ring<const N: usize> {
    buf: [u8; N],
    start: usize,
    used: usize,
}

impl<const N: usize> Ring<N> {
    const fn new() -> Self {
        Ring {
            buf: [0u8; N],
            start: 0,
            used: 0,
        }
    }

    fn at(&self, off: usize) -> u8 {
        self.buf[(self.start + off) % N]
    }

    fn push(&mut self, env: &[u8]) -> bool {
        let need = LEN_PREFIX + env.len();
        if env.len() > u16::MAX as usize || need > N - self.used {
            return false;
        }
        let len = (env.len() as u16).to_le_bytes();
        let base = self.start + self.used;
        self.buf[base % N] = len[0];
        self.buf[(base + 1) % N] = len[1];
        for (i, b) in env.iter().enumerate() {
            self.buf[(base + LEN_PREFIX + i) % N] = *b;
        }
        self.used += need;
        true
    }

    /// Length of the oldest envelope, if any.
    fn front_len(&self) -> Option<usize> {
        if self.used == 0 {
            return None;
        }
        Some(u16::from_le_bytes([self.at(0), self.at(1)]) as usize)
    }

    fn pop(&mut self) {
        if let Some(l) = self.front_len() {
            let n = LEN_PREFIX + l;
            self.start = (self.start + n) % N;
            self.used -= n;
        }
    }
}

/// One presentation's queue and counters.
pub struct Presentation<const N: usize> {
    ring: Ring<N>,
    /// Messages of the oldest envelope already handed out (0 = none yet).
    sent: usize,
    asked: bool,
    parked: bool,
    dropped_total: u64,
    dropped_run: u64,
    next_report_at: u64,
    /// Envelopes offered since the presentation last asked.
    unasked: u64,
    next_behind_at: u64,
    /// A `Behind` was reported and has not been answered by catching up.
    reported_behind: bool,
    high_water: usize,
}

impl<const N: usize> Presentation<N> {
    pub const fn new() -> Self {
        Presentation {
            ring: Ring::new(),
            sent: 0,
            asked: false,
            parked: false,
            dropped_total: 0,
            dropped_run: 0,
            next_report_at: 1,
            unasked: 0,
            next_behind_at: BEHIND_AFTER,
            reported_behind: false,
            high_water: 0,
        }
    }

    /// Count an offer; the `Behind` report it triggers, if any.
    fn count_unasked(&mut self) -> Option<Report> {
        self.unasked += 1;
        if self.unasked == self.next_behind_at {
            self.next_behind_at = self.next_behind_at.saturating_mul(2);
            self.reported_behind = true;
            return Some(Report::Behind {
                unasked: self.unasked,
                never_asked: !self.asked,
            });
        }
        None
    }

    fn offer(&mut self, env: &[u8]) -> Offer {
        let behind = self.count_unasked();
        if self.ring.push(env) {
            if self.ring.used > self.high_water {
                self.high_water = self.ring.used;
            }
            let wake = self.parked;
            self.parked = false;
            return Offer::Queued {
                wake,
                report: behind,
            };
        }
        self.dropped_total += 1;
        self.dropped_run += 1;
        let report = if self.dropped_run == self.next_report_at {
            self.next_report_at = self.next_report_at.saturating_mul(2);
            Some(Report::NotTaking {
                dropped: self.dropped_run,
                never_asked: !self.asked,
            })
        } else {
            // Once envelopes are being lost, the drop reports say more than a
            // `Behind` at every doubling would; do not say both.
            None
        };
        let _ = behind;
        Offer::Dropped { report }
    }

    fn next(&mut self) -> (Next, Option<Report>) {
        self.asked = true;
        self.unasked = 0;
        self.next_behind_at = BEHIND_AFTER;
        let Some(len) = self.ring.front_len() else {
            self.parked = true;
            self.sent = 0;
            // Empty after falling behind or dropping: the presentation has
            // caught up.
            let report = if self.dropped_run > 0 || self.reported_behind {
                let r = Report::Resumed {
                    dropped: self.dropped_run,
                };
                self.dropped_run = 0;
                self.next_report_at = 1;
                self.reported_behind = false;
                Some(r)
            } else {
                None
            };
            return (Next::Empty, report);
        };
        let chunks = len.div_ceil(CHUNK_BYTES);
        let idx = self.sent;
        let msg = if idx == 0 {
            self.sent = 1;
            Msg {
                kind: Kind::Begin,
                words: [len as u64, 0, 0, 0],
            }
        } else if idx <= chunks {
            let base = (idx - 1) * CHUNK_BYTES;
            let mut bytes = [0u8; CHUNK_BYTES];
            for (i, slot) in bytes.iter_mut().enumerate() {
                if base + i < len {
                    *slot = self.ring.at(LEN_PREFIX + base + i);
                }
            }
            let mut words = [0u64; 4];
            for (w, chunk) in words.iter_mut().zip(bytes.chunks_exact(8)) {
                *w = u64::from_le_bytes(chunk.try_into().unwrap());
            }
            self.sent += 1;
            Msg {
                kind: Kind::Chunk,
                words,
            }
        } else {
            self.ring.pop();
            self.sent = 0;
            Msg {
                kind: Kind::Commit,
                words: [0; 4],
            }
        };
        (Next::Message(msg), None)
    }

    /// Envelopes dropped since the start.
    pub fn dropped_total(&self) -> u64 {
        self.dropped_total
    }

    /// Bytes currently queued, including length prefixes.
    pub fn queued_bytes(&self) -> usize {
        self.ring.used
    }

    /// The most bytes ever queued at once. Never exceeds `N`.
    pub fn high_water(&self) -> usize {
        self.high_water
    }

    /// Has this presentation ever asked?
    pub fn has_asked(&self) -> bool {
        self.asked
    }

    /// Is this presentation waiting to be woken?
    pub fn is_parked(&self) -> bool {
        self.parked
    }
}

impl<const N: usize> Default for Presentation<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// `P` presentations, each with an `N`-byte queue.
///
/// The set is fixed by the caller; nothing here decodes, validates or changes
/// an envelope (RFC-0.34-001 D2 — the stream's decoding does not depend on how
/// many presentations exist).
pub struct Fanout<const P: usize, const N: usize> {
    presentations: [Presentation<N>; P],
}

impl<const P: usize, const N: usize> Fanout<P, N> {
    pub const fn new() -> Self {
        Fanout {
            presentations: [const { Presentation::new() }; P],
        }
    }

    /// Offer an encoded envelope to presentation `p`. Never blocks.
    ///
    /// An index outside `0..P` is dropped and reported as such by returning
    /// `Dropped { report: None }`: it names no presentation, so it has no
    /// counter to keep.
    pub fn offer(&mut self, p: usize, envelope: &[u8]) -> Offer {
        match self.presentations.get_mut(p) {
            Some(pr) => pr.offer(envelope),
            None => Offer::Dropped { report: None },
        }
    }

    /// Presentation `p` asked for its next message. Never blocks; answers at
    /// once. `Empty` with `p` out of range.
    pub fn next(&mut self, p: usize) -> (Next, Option<Report>) {
        match self.presentations.get_mut(p) {
            Some(pr) => pr.next(),
            None => (Next::Empty, None),
        }
    }

    pub fn presentation(&self, p: usize) -> Option<&Presentation<N>> {
        self.presentations.get(p)
    }
}

impl<const P: usize, const N: usize> Default for Fanout<P, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
