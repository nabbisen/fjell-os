//! Envelope in, braille cells out (RFC-0.34-001 D4).
//!
//! # What this is, and is not
//!
//! A **pure function** from a [`SemanticEnvelope`] to lines of Unicode braille
//! patterns (U+2800–U+28FF). No syscalls, no allocation, no `unsafe`, no
//! state: the same envelope and width always give the same bytes, so its
//! vectors run in Gate 1 and the service that wraps it is a thin adapter.
//!
//! It is a **presentation for a person who reads braille, written as the
//! stream a braille display driver would consume**. It is **not** a
//! conforming implementation of UEB or any national braille code, it has been
//! read by no braille reader, and QEMU `virt` has no braille display to drive
//! (E-004). Nothing here may be described as Fjell supporting braille or being
//! accessible (RFC-0.34-001 D7).
//!
//! # The rules — deliberately small
//!
//! * **Cells** are six-dot patterns, U+2800 + a mask of dots 1–6 (dot 1 = bit 0
//!   … dot 6 = bit 5). A space is the blank cell U+2800, so a width is a
//!   count of cells.
//! * **Letters** `a`–`z` take the standard patterns. A **capital** is the dot-6
//!   cell (⠠) before its letter, once per capital — a simplification (UEB uses a
//!   capitals-word indicator for runs).
//! * **Digits**: the number sign (⠼, dots 3456) once at the start of a run of
//!   digits, then `1`–`9`,`0` as the patterns of `a`–`j`. A `.` or `,` *between*
//!   digits stays inside the run. A letter `a`–`j` straight after a digit run
//!   is preceded by the grade-1 indicator (⠰) so it is not read as a digit.
//! * **Punctuation**: a short fixed set (`. , ; : ! ? ' - /` and parentheses,
//!   which are two cells).
//! * **Anything else** — every character not listed, including all non-ASCII —
//!   is the **eight-dot full cell U+28FF**, which no six-dot rule produces, so
//!   an unrepresentable character is visible and cannot be mistaken for text.
//! * **Width**: lines wrap at spaces, greedily; a word longer than a line is
//!   broken at the width. Runs of spaces collapse to one.
//!
//! # Structure — where this differs from `proxy-text`
//!
//! **Severity (or status) comes first**, as a word and a colon, before the
//! title. `proxy-text` prints bracketed tags and puts the severity inside them.
//!
//! * *Intent*: `severity: title`; the description; each consequence as
//!   `consequence: text`; then `actions: N` and one numbered line per action.
//! * *State*: `status: title`; the summary; `facts: N` if there are any.
//! * *Event*: `severity result: title`; the description.
//!
//! **Omitted, on purpose:** each action's required capability, reversibility
//! and confirmation policy; an intent's expiry; a state's facts (only their
//! count); an event's subject and audit sequence; and every `TextToken`'s
//! **id** — only the fallback text is used, because no catalogue exists for the
//! id to mean anything (RFC-0.34-001 §E). A person deciding whether to act would
//! want reversibility and confirmation; **there is no input path (D6), so nothing
//! can be acted on through this presentation**, and the omission is a recorded
//! limitation, not something filled in.

#![no_std]

use fjell_semantic_format::{
    EventResult, MAX_TEXT_BYTES, SemanticEnvelope, SemanticPayload, Severity, Status,
};

/// The width the service uses. A parameter of every function here; 40 is a
/// choice, and no claim is made about any device's width.
pub const DEFAULT_WIDTH: usize = 40;

/// The widest line this renderer will produce whatever width is asked for.
pub const MAX_WIDTH: usize = 120;

/// The blank cell.
pub const BLANK: u8 = 0x00;
/// A character with no rule: the eight-dot full cell, never produced by a
/// six-dot rule.
pub const UNREPRESENTABLE: u8 = 0xFF;

// Cell masks: dot 1 = 0x01, dot 2 = 0x02, dot 3 = 0x04, dot 4 = 0x08,
// dot 5 = 0x10, dot 6 = 0x20.
const CAPITAL: u8 = 0x20; // dot 6
const NUMBER: u8 = 0x3C; // dots 3456
const GRADE1: u8 = 0x30; // dots 56

/// The pattern of `a`–`z`, by the standard dot assignments.
const LETTERS: [u8; 26] = [
    0x01, // a  1
    0x03, // b  12
    0x09, // c  14
    0x19, // d  145
    0x11, // e  15
    0x0B, // f  124
    0x1B, // g  1245
    0x13, // h  125
    0x0A, // i  24
    0x1A, // j  245
    0x05, // k  13
    0x07, // l  123
    0x0D, // m  134
    0x1D, // n  1345
    0x15, // o  135
    0x0F, // p  1234
    0x1F, // q  12345
    0x17, // r  1235
    0x0E, // s  234
    0x1E, // t  2345
    0x25, // u  136
    0x27, // v  1236
    0x3A, // w  2456
    0x2D, // x  1346
    0x3D, // y  13456
    0x35, // z  1356
];

/// Cells for one character, or `None` if it has no rule. Parentheses are two
/// cells, so this returns a small array and a length.
fn punctuation(c: char) -> Option<([u8; 2], usize)> {
    Some(match c {
        '.' => ([0x32, 0], 1),    // 256
        ',' => ([0x02, 0], 1),    // 2
        ';' => ([0x06, 0], 1),    // 23
        ':' => ([0x12, 0], 1),    // 25
        '!' => ([0x16, 0], 1),    // 235
        '?' => ([0x26, 0], 1),    // 236
        '\'' => ([0x04, 0], 1),   // 3
        '-' => ([0x24, 0], 1),    // 36
        '/' => ([0x0C, 0], 1),    // 34
        '(' => ([0x10, 0x23], 2), // 5, 126
        ')' => ([0x10, 0x1C], 2), // 5, 345
        _ => return None,
    })
}

/// The most cells any text can become: every character at worst three cells
/// (a capital never combines with a paren, so two is the real bound; three is
/// slack), plus room for the labels this crate prepends.
const CELLS_CAP: usize = 3 * MAX_TEXT_BYTES + 64;

/// Cells accumulated for one paragraph, before wrapping.
struct Cells {
    buf: [u8; CELLS_CAP],
    len: usize,
}

impl Cells {
    const fn new() -> Self {
        Cells {
            buf: [0; CELLS_CAP],
            len: 0,
        }
    }

    fn push(&mut self, cell: u8) {
        if self.len < CELLS_CAP {
            self.buf[self.len] = cell;
            self.len += 1;
        }
    }

    fn cells(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Translate `text` by the rules above.
    fn push_text(&mut self, text: &str) {
        let mut in_number = false;
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                'a'..='z' | 'A'..='Z' => {
                    let lower = c.to_ascii_lowercase();
                    let idx = lower as usize - 'a' as usize;
                    if in_number && idx <= 9 {
                        // a–j straight after digits would read as a digit.
                        self.push(GRADE1);
                    }
                    if c.is_ascii_uppercase() {
                        self.push(CAPITAL);
                    }
                    self.push(LETTERS[idx]);
                    in_number = false;
                }
                '0'..='9' => {
                    if !in_number {
                        self.push(NUMBER);
                        in_number = true;
                    }
                    // 1..9 are a..i, 0 is j.
                    let idx = if c == '0' {
                        9
                    } else {
                        c as usize - '1' as usize
                    };
                    self.push(LETTERS[idx]);
                }
                '.' | ',' if in_number && chars.peek().is_some_and(|n| n.is_ascii_digit()) => {
                    // A separator between digits stays inside the number.
                    if let Some((p, n)) = punctuation(c) {
                        p[..n].iter().for_each(|x| self.push(*x));
                    }
                }
                ' ' | '\t' | '\n' | '\r' => {
                    self.push(BLANK);
                    in_number = false;
                }
                other => {
                    in_number = false;
                    match punctuation(other) {
                        Some((p, n)) => p[..n].iter().for_each(|x| self.push(*x)),
                        None => self.push(UNREPRESENTABLE),
                    }
                }
            }
        }
    }
}

/// Write one cell as its Unicode braille pattern (three UTF-8 bytes).
fn utf8(cell: u8, out: &mut [u8]) {
    let cp = 0x2800u32 + cell as u32;
    out[0] = 0xE0 | ((cp >> 12) & 0x0F) as u8;
    out[1] = 0x80 | ((cp >> 6) & 0x3F) as u8;
    out[2] = 0x80 | (cp & 0x3F) as u8;
}

/// Wrap `cells` at `width` and hand each line to `emit` as UTF-8 text.
fn wrap(cells: &[u8], width: usize, emit: &mut dyn FnMut(&str)) {
    let width = width.clamp(1, MAX_WIDTH);
    let mut line = [0u8; MAX_WIDTH];
    let mut len = 0usize;
    let flush = |line: &[u8], len: usize, emit: &mut dyn FnMut(&str)| {
        let mut bytes = [0u8; MAX_WIDTH * 3];
        for (i, cell) in line[..len].iter().enumerate() {
            utf8(*cell, &mut bytes[i * 3..i * 3 + 3]);
        }
        // Only braille patterns were written, so this is valid UTF-8.
        emit(core::str::from_utf8(&bytes[..len * 3]).unwrap_or(""));
    };
    let mut i = 0usize;
    while i < cells.len() {
        while i < cells.len() && cells[i] == BLANK {
            i += 1;
        }
        if i >= cells.len() {
            break;
        }
        let start = i;
        while i < cells.len() && cells[i] != BLANK {
            i += 1;
        }
        let mut word = &cells[start..i];
        // A word that cannot fit a whole line is broken at the width.
        while word.len() > width {
            if len > 0 {
                flush(&line, len, emit);
                len = 0;
            }
            flush(&word[..width], width, emit);
            word = &word[width..];
        }
        if len > 0 && len + 1 + word.len() > width {
            flush(&line, len, emit);
            len = 0;
        }
        if len > 0 {
            line[len] = BLANK;
            len += 1;
        }
        line[len..len + word.len()].copy_from_slice(word);
        len += word.len();
    }
    if len > 0 {
        flush(&line, len, emit);
    }
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Low => "low",
        Severity::Normal => "normal",
        Severity::Important => "important",
        Severity::Critical => "critical",
    }
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Unknown => "unknown",
        Status::Ok => "ok",
        Status::Degraded => "degraded",
        Status::Warning => "warning",
        Status::Failed => "failed",
    }
}

fn result_word(r: EventResult) -> &'static str {
    match r {
        EventResult::Ok => "ok",
        EventResult::Denied => "denied",
        EventResult::Failed => "failed",
        EventResult::TimedOut => "timed out",
        EventResult::NotApplicable => "n/a",
    }
}

/// Render `envelope` as lines of braille at `width` cells, calling `line` once
/// per line. Every line is UTF-8 made only of U+2800–U+28FF patterns.
pub fn render_lines(envelope: &SemanticEnvelope, width: usize, mut line: impl FnMut(&str)) {
    let emit: &mut dyn FnMut(&str) = &mut line;
    let mut paragraph = |build: &dyn Fn(&mut Cells)| {
        let mut c = Cells::new();
        build(&mut c);
        wrap(c.cells(), width, emit);
    };
    match &envelope.payload {
        SemanticPayload::Intent(n) => {
            paragraph(&|c| {
                c.push_text(severity_word(n.severity));
                c.push_text(": ");
                c.push_text(n.title.as_str());
            });
            if !n.description.fallback.is_empty() {
                paragraph(&|c| c.push_text(n.description.as_str()));
            }
            for cons in n.consequences.iter() {
                paragraph(&|c| {
                    c.push_text("consequence: ");
                    c.push_text(cons.text.as_str());
                });
            }
            if !n.actions.is_empty() {
                paragraph(&|c| {
                    c.push_text("actions: ");
                    push_number(c, n.actions.len());
                });
                for (i, a) in n.actions.iter().enumerate() {
                    paragraph(&|c| {
                        push_number(c, i + 1);
                        c.push_text(" ");
                        c.push_text(a.label.as_str());
                    });
                }
            }
        }
        SemanticPayload::State(n) => {
            paragraph(&|c| {
                c.push_text(status_word(n.status));
                c.push_text(": ");
                c.push_text(n.title.as_str());
            });
            if !n.summary.fallback.is_empty() {
                paragraph(&|c| c.push_text(n.summary.as_str()));
            }
            if !n.facts.is_empty() {
                paragraph(&|c| {
                    c.push_text("facts: ");
                    push_number(c, n.facts.len());
                });
            }
        }
        SemanticPayload::Event(n) => {
            paragraph(&|c| {
                c.push_text(severity_word(n.severity));
                c.push_text(" ");
                c.push_text(result_word(n.result));
                c.push_text(": ");
                c.push_text(n.title.as_str());
            });
            if !n.description.fallback.is_empty() {
                paragraph(&|c| c.push_text(n.description.as_str()));
            }
        }
    }
}

/// Push a small count as decimal digits, through the same digit rule.
fn push_number(c: &mut Cells, mut n: usize) {
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    if n == 0 {
        i -= 1;
        digits[i] = b'0';
    }
    while n > 0 {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    c.push_text(core::str::from_utf8(&digits[i..]).unwrap_or("0"));
}

/// The output did not fit the buffer. `written` counts the bytes of whole
/// lines that did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Truncated {
    pub written: usize,
}

/// Render into `out`, each line followed by `\n`. Returns the bytes written,
/// or `Truncated` if a whole line did not fit — never a partial line.
pub fn render_into(
    envelope: &SemanticEnvelope,
    width: usize,
    out: &mut [u8],
) -> Result<usize, Truncated> {
    let mut at = 0usize;
    let mut full = false;
    render_lines(envelope, width, |l| {
        let need = l.len() + 1;
        if full || at + need > out.len() {
            full = true;
            return;
        }
        out[at..at + l.len()].copy_from_slice(l.as_bytes());
        out[at + l.len()] = b'\n';
        at += need;
    });
    if full {
        Err(Truncated { written: at })
    } else {
        Ok(at)
    }
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
