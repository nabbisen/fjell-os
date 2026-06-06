# ABI Stability Policy

*See also [Stability Tiers](./stability.md).*

The stable surface (S1–S9) is frozen at v1.0. See RFC-v0.10-002 for the full policy.

Breaking changes before v1.0 require a CHANGELOG `### Breaking` entry and an ABI snapshot update.

Gate: `cargo xtask abi-snapshot --verify`
