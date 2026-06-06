# ABI Stability Policy

*See also [Stability Tiers](./stability.md).*

The stable surface (S1–S9) is frozen at v1.0. See RFC-v0.10-002 for the full policy.

Breaking changes before v1.0 require a CHANGELOG `### Breaking` entry and an ABI snapshot update.

Gate: `cargo xtask abi-snapshot --verify`

## ABI snapshot gate — scope and limits (RFC-v0.16-005, H-03)

The `fjell-abi-snapshot` gate detects **removals and textual signature
changes** in the stable public surface. It is a **drift guard**, not a
complete Rust semantic-compatibility checker:

- It catches removed or renamed public items and changed signature text.
- It does **not** catch semantic-only breakage with unchanged signature
  text (e.g. a generic-bound change via a blanket impl, or a type-alias
  redefinition in another module).

The scanner is line-based deliberately, to avoid a nightly-toolchain
dependency that would conflict with the reproducible-build gate. A
rustdoc-JSON-based semantic checker is a v1.x improvement, tracked
separately. Release wording must say "ABI drift guard," not "complete
semantic ABI proof."
