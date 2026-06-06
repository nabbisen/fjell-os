# RFC 060 — `fjell-unsafe-audit` string-literal false positives

**Status:** Implemented (v0.8.1)
**Target version:** v0.8.1 (hotfix on top of v0.8.0)
**Affects:** `tools/fjell-unsafe-audit/src/main.rs`

## Problem

`cargo run -p fjell-unsafe-audit -- --workspace . --check` exits with
status 1 against the current tree:

```
total unsafe sites : 278
with SAFETY comment: 267
with valid category tag: 267
missing comment    : 11

MISSING SAFETY comments:
  ./tools/fjell-unsafe-audit/src/main.rs:105 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:107 [fn]
  ./tools/fjell-unsafe-audit/src/main.rs:109 [impl]
  ./tools/fjell-unsafe-audit/src/main.rs:111 [trait]
  ./tools/fjell-unsafe-audit/src/main.rs:301 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:310 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:320 [fn]
  ./tools/fjell-unsafe-audit/src/main.rs:331 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:351 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:361 [block]
  ./tools/fjell-unsafe-audit/src/main.rs:371 [block]
```

None of these 11 sites is a real `unsafe` block. The first four
(lines 105–111) are the scanner's own pattern strings:

```rust
let kind = if trimmed.contains("unsafe {") || trimmed.starts_with("unsafe {") {
    Some(UnsafeKind::Block)
} else if trimmed.contains("unsafe fn ") {
    Some(UnsafeKind::Fn)
} else if trimmed.contains("unsafe impl ") {
    Some(UnsafeKind::Impl)
} else if trimmed.contains("unsafe trait ") {
    Some(UnsafeKind::Trait)
}
```

The remaining seven (lines 301–371) are unit-test fixture strings such
as `"unsafe { *ptr }\n"` passed to `TmpFile::write` inside `#[test]`
functions.

The CHANGELOG entry for v0.7.3 claims `270/270 unsafe sites carry
category= tags`. The CI gate (RFC-v0.7.1-002, shipped in v0.7.4)
runs `--check` and would fail on these false positives.

## Proposed fix

The scanner's line-level `contains` test must distinguish between

1. `unsafe {` as Rust syntax, and
2. `"unsafe {"` as a substring inside a string literal.

The simplest rule that closes every observed false positive:

> Before classifying a line as containing an `unsafe` keyword, strip
> string-literal regions from the line. A line whose `unsafe` token
> only appears inside `"…"` or `r#"…"#` is not an unsafe site.

A second, narrower rule:

> Inside a `#[cfg(test)] mod tests { … }` or under a `#[test]`
> attribute, the scanner still scans, but does not require SAFETY
> comments on `unsafe { }` constructed inside source-string literals
> intended as test fixtures.

This RFC adopts the first rule only; the second is unnecessary once
string-literal stripping is in place.

### Stripping rule

Before checking for `unsafe`, transform each line by removing every
character span that begins with `"` and ends with the next unescaped
`"`. Raw strings `r"…"` and `r#"…"#` are removed by matching
`r` followed by zero or more `#`, then `"`, up to the closing
matching delimiter.

The transformation is line-local: multi-line strings that span
several source lines remain unhandled in this RFC. The codebase
does not currently embed `unsafe {` inside a multi-line string;
if that changes, RFC 061 can extend the rule.

### Pseudocode

```rust
fn strip_string_literals(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            // Raw string: r"…" or r#"…"# … or r##"…"## etc.
            'r' if chars.peek() == Some(&'"') || chars.peek() == Some(&'#') => {
                let mut hashes = 0;
                while chars.peek() == Some(&'#') { chars.next(); hashes += 1; }
                if chars.peek() == Some(&'"') {
                    chars.next();
                    // skip until closing " followed by `hashes` #s
                    let mut buf = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '"' {
                            let mut peeked = 0;
                            while peeked < hashes && chars.peek() == Some(&'#') {
                                chars.next(); peeked += 1;
                            }
                            if peeked == hashes { break; }
                            buf.push('"');
                            for _ in 0..peeked { buf.push('#'); }
                        }
                    }
                } else {
                    out.push('r');
                    for _ in 0..hashes { out.push('#'); }
                }
            }
            // Normal string: "…"
            '"' => {
                while let Some(ch) = chars.next() {
                    if ch == '\\' { chars.next(); continue; }
                    if ch == '"' { break; }
                }
            }
            _ => out.push(c),
        }
    }
    out
}
```

The scanner then runs its existing `contains("unsafe {")` checks on
the stripped line.

## Acceptance criteria

1. After this RFC ships, `cargo run -p fjell-unsafe-audit -- --workspace . --check`
   exits 0 against the current tree.
2. The total count of real unsafe sites does not change (the tool
   reports `0 missing`, not `0 total`).
3. The scanner still detects a real `unsafe { … }` block; a regression
   test adds a fixture containing both real unsafe and `"unsafe {"`
   inside a string, and verifies the real one is counted and the string
   one is not.
4. CI's `ci-unsafe-audit` job (`.github/workflows/ci.yml`, RFC-v0.7.1-002)
   passes without modification.
5. No other workspace member needs a SAFETY-comment change; the 11
   currently-flagged sites disappear and no new sites are introduced.

## Out of scope

- Multi-line string literals containing `unsafe {`.
- Comments containing `unsafe {` (already handled by the existing
  `SAFETY` lookback window, since the comment line precedes the
  scanned line; in practice such comments are explanatory and the
  current `has_safety` logic accepts them).
- A full Rust lexer. The line-local stripping rule is sufficient
  for every real-world false positive observed today.
