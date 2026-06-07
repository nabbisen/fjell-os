# v1.0 Non-Goals

The authoritative, individually justified list (N1–N23) is maintained in
[`docs/release/v1-non-goals.md`](https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-non-goals.md),
governed by RFC-v0.15-005; changes require an identity-level RFC.

Highlights, with their identifiers:

- **N1 POSIX** — no POSIX surface; descriptors are ambient authority.
- **N7 Hard real-time** — no hard scheduling guarantees.
- **N18 ARM64** — RISC-V only as the supported platform.
- **N21 Kernel-IPC for the SDK reference service** — deferred.
- **N23 Byte-level key erasure** — no verified `ZeroizeOnDrop` guarantee.

For the release-gate view of v1.0 limitations (Gate 9), see
[`docs/release/v1-limitations.md`](https://github.com/nabbisen/fjell-os/blob/main/docs/release/v1-limitations.md).
