//! `Canon` — the trait a byte-producing function writes through
//! (RFC-0.33-003, E-045/E-055).
//!
//! # Why this exists
//!
//! A `.frozen` schema file that a person writes by hand drifts from the code it
//! describes (E-045: eleven files, none of them checked, most of them wrong). A
//! generator that reads *struct fields* describes the wrong thing: the layout a
//! format really has is decided by the **function that produces its bytes** —
//! which fields, in which order, at which width, gated by which version — and a
//! generator that hard-coded the fields would be a second description of it,
//! which is the same defect with more steps.
//!
//! So the function itself is written against this trait, whose every call carries
//! the field's **name and type**, and there are two sinks for the *same*
//! function:
//!
//! * [`BufSink`] collects the bytes (for a digest, or a sector). That is what the
//!   format did before, and a golden test holds it bit for bit.
//! * a *recording* sink (in `fjell-schema`, on the host) runs the function on a
//!   representative value and emits the sequence of `(name, type, width)` — the
//!   body of a `.frozen` file.
//!
//! There is no second list of fields to keep in step, because there is no second
//! list. No allocation, no `unsafe`, no dependencies.
//!
//! # What a call means
//!
//! Integers are **little-endian**, as every format here writes them. `bytes` is a
//! fixed-width array whose width is the slice's length. `zeros` is a constant run
//! that is a field of no struct (a digest placeholder, explicit padding): it is
//! written and recorded, and named so it cannot be mistaken for data. `each` is a
//! counted group whose element shape the recorder learns from the first element.

#![no_std]
#![forbid(unsafe_code)]

/// The sink a canonical writer writes through. Object-safe on purpose, so a
/// counted group can pass its sink to a closure.
pub trait Canon {
    /// A leading tag that separates one format's stream from another's. Written
    /// as bytes; recorded as `domain "…"`.
    fn domain(&mut self, tag: &[u8]);
    /// A constant magic number at the head of a container. Written as bytes;
    /// recorded as `magic "…"`.
    fn magic(&mut self, tag: &[u8]);
    fn u8(&mut self, name: &'static str, v: u8);
    fn u16(&mut self, name: &'static str, v: u16);
    fn u32(&mut self, name: &'static str, v: u32);
    fn u64(&mut self, name: &'static str, v: u64);
    /// A fixed-width byte array; its width is `v.len()`.
    fn bytes(&mut self, name: &'static str, v: &[u8]);
    /// Bytes whose length is given by another field or rule, named in `len`.
    fn var_bytes(&mut self, name: &'static str, v: &[u8], len: &'static str);
    /// `n` zero bytes that are a field of no struct.
    fn zeros(&mut self, name: &'static str, n: usize);
    /// A counted group: `f` is called for each of `count` elements. `max` is the
    /// group's capacity, if it has one, which the description states.
    fn each(
        &mut self,
        name: &'static str,
        count: usize,
        max: Option<usize>,
        f: &mut dyn FnMut(&mut dyn Canon, usize),
    );
    /// A fact about the format that is **not** part of its bytes (a catalogue size,
    /// a version label). Recorded; never written.
    fn note(&mut self, key: &'static str, value: &str);
}

/// A fixed-capacity byte sink: the "collect the bytes" half of [`Canon`].
///
/// Overrunning `N` **panics**, as the fixed stack buffers this replaces did: a
/// stream longer than its declared capacity is a bug, not a truncation.
pub struct BufSink<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Default for BufSink<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> BufSink<N> {
    pub const fn new() -> Self {
        BufSink {
            buf: [0u8; N],
            len: 0,
        }
    }

    /// A sink whose unwritten bytes are `fill` instead of zero. **For tests**:
    /// run the same writer into `filled(0x00)` and `filled(0xFF)`; if the two
    /// disagree anywhere in `bytes()`, some byte was *not* written by a named
    /// field and its value came from the buffer — which is exactly what
    /// struct padding written to disk is.
    pub const fn filled(fill: u8) -> Self {
        BufSink {
            buf: [fill; N],
            len: 0,
        }
    }

    /// Everything written so far.
    pub fn bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn put(&mut self, b: &[u8]) {
        let end = self.len + b.len();
        assert!(end <= N, "canonical stream overran its {N}-byte buffer");
        self.buf[self.len..end].copy_from_slice(b);
        self.len = end;
    }
}

impl<const N: usize> Canon for BufSink<N> {
    fn domain(&mut self, tag: &[u8]) {
        self.put(tag);
    }
    fn magic(&mut self, tag: &[u8]) {
        self.put(tag);
    }
    fn u8(&mut self, _: &'static str, v: u8) {
        self.put(&[v]);
    }
    fn u16(&mut self, _: &'static str, v: u16) {
        self.put(&v.to_le_bytes());
    }
    fn u32(&mut self, _: &'static str, v: u32) {
        self.put(&v.to_le_bytes());
    }
    fn u64(&mut self, _: &'static str, v: u64) {
        self.put(&v.to_le_bytes());
    }
    fn bytes(&mut self, _: &'static str, v: &[u8]) {
        self.put(v);
    }
    fn var_bytes(&mut self, _: &'static str, v: &[u8], _: &'static str) {
        self.put(v);
    }
    fn zeros(&mut self, _: &'static str, n: usize) {
        let end = self.len + n;
        assert!(end <= N, "canonical stream overran its {N}-byte buffer");
        // Explicit zeros, even in a `filled` sink: a zeros field is a constant.
        self.buf[self.len..end].fill(0);
        self.len = end;
    }
    fn each(
        &mut self,
        _: &'static str,
        count: usize,
        _: Option<usize>,
        f: &mut dyn FnMut(&mut dyn Canon, usize),
    ) {
        for i in 0..count {
            f(self, i);
        }
    }
    fn note(&mut self, _: &'static str, _: &str) {}
}

/// A sink that writes into a caller's slice: the "collect the bytes" half for a
/// codec that fills a buffer it was handed. The caller checks the size first (the
/// codecs here all do); overrunning **panics** rather than truncating.
pub struct SliceSink<'a> {
    buf: &'a mut [u8],
    len: usize,
}

impl<'a> SliceSink<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        SliceSink { buf, len: 0 }
    }

    /// Bytes written so far.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn put(&mut self, b: &[u8]) {
        let end = self.len + b.len();
        assert!(end <= self.buf.len(), "canonical stream overran its buffer");
        self.buf[self.len..end].copy_from_slice(b);
        self.len = end;
    }
}

impl Canon for SliceSink<'_> {
    fn domain(&mut self, tag: &[u8]) {
        self.put(tag);
    }
    fn magic(&mut self, tag: &[u8]) {
        self.put(tag);
    }
    fn u8(&mut self, _: &'static str, v: u8) {
        self.put(&[v]);
    }
    fn u16(&mut self, _: &'static str, v: u16) {
        self.put(&v.to_le_bytes());
    }
    fn u32(&mut self, _: &'static str, v: u32) {
        self.put(&v.to_le_bytes());
    }
    fn u64(&mut self, _: &'static str, v: u64) {
        self.put(&v.to_le_bytes());
    }
    fn bytes(&mut self, _: &'static str, v: &[u8]) {
        self.put(v);
    }
    fn var_bytes(&mut self, _: &'static str, v: &[u8], _: &'static str) {
        self.put(v);
    }
    fn zeros(&mut self, _: &'static str, n: usize) {
        let end = self.len + n;
        assert!(end <= self.buf.len(), "canonical stream overran its buffer");
        self.buf[self.len..end].fill(0);
        self.len = end;
    }
    fn each(
        &mut self,
        _: &'static str,
        count: usize,
        _: Option<usize>,
        f: &mut dyn FnMut(&mut dyn Canon, usize),
    ) {
        for i in 0..count {
            f(self, i);
        }
    }
    fn note(&mut self, _: &'static str, _: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(c: &mut dyn Canon) {
        c.domain(b"D");
        c.u16("a", 0x0102);
        c.u32("b", 0x0304_0506);
        c.u64("c", 0x0708_090A_0B0C_0D0E);
        c.u8("d", 0xFF);
        c.bytes("e", &[1, 2, 3]);
        c.zeros("pad", 2);
        c.u8("n", 2);
        c.each("g", 2, Some(4), &mut |c, i| c.u8("x", i as u8 + 10));
        c.note("k", "not written");
    }

    #[test]
    fn integers_are_little_endian_and_groups_repeat() {
        let mut s = BufSink::<64>::new();
        stream(&mut s);
        assert_eq!(
            s.bytes(),
            &[
                b'D', 0x02, 0x01, 0x06, 0x05, 0x04, 0x03, 0x0E, 0x0D, 0x0C, 0x0B, 0x0A, 0x09, 0x08,
                0x07, 0xFF, 1, 2, 3, 0, 0, 2, 10, 11
            ]
        );
    }

    #[test]
    fn a_fully_named_stream_does_not_depend_on_the_buffer() {
        let (mut a, mut b) = (BufSink::<64>::filled(0x00), BufSink::<64>::filled(0xFF));
        stream(&mut a);
        stream(&mut b);
        assert_eq!(a.bytes(), b.bytes(), "every byte came from a named field");
    }

    #[test]
    fn a_byte_nobody_wrote_is_seen() {
        // The control for the test above: a writer that skips a byte (as struct
        // padding does) shows the fill through, so the two sinks disagree.
        fn gappy(c: &mut BufSink<8>) {
            c.u8("a", 1);
            c.len += 1; // a byte that no named field wrote
            c.u8("b", 2);
        }
        let (mut a, mut b) = (BufSink::<8>::filled(0x00), BufSink::<8>::filled(0xFF));
        gappy(&mut a);
        gappy(&mut b);
        assert_ne!(a.bytes(), b.bytes());
    }

    #[test]
    #[should_panic(expected = "overran")]
    fn overrunning_the_capacity_panics() {
        let mut s = BufSink::<2>::new();
        s.u32("x", 1);
    }

    #[test]
    fn the_slice_sink_writes_what_the_buffer_sink_writes() {
        let mut a = BufSink::<64>::new();
        stream(&mut a);
        let mut raw = [0u8; 64];
        let mut b = SliceSink::new(&mut raw);
        stream(&mut b);
        let n = b.len();
        assert_eq!(&raw[..n], a.bytes());
    }

    #[test]
    #[should_panic(expected = "overran")]
    fn the_slice_sink_panics_rather_than_truncate() {
        let mut raw = [0u8; 2];
        SliceSink::new(&mut raw).u32("x", 1);
    }

    #[test]
    fn notes_are_not_written() {
        let mut s = BufSink::<4>::new();
        s.note("k", "v");
        assert!(s.is_empty());
    }
}
