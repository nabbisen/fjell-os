# fjell-abi

The stable ABI surface of [Fjell OS](https://github.com/nabbisen/fjell-os) — a
capability-based microkernel for high-assurance edge and fleet nodes.

`no_std`, zero dependencies. Contains syscall numbers, capability kinds and
rights, service image identifiers, and boot-control types: the definitions
shared between the kernel and every user-space service.

## Status

**Fjell OS is v0 software under active development. v1.0 is explicitly not in
view.** This crate is published so its API surface is inspectable and its name
is reserved alongside the project; it is not yet intended as a stable
dependency for outside consumers.

The project guards this surface with a snapshot gate — see
`tools/fjell-abi-snapshot` and `tests/abi/snapshot.json` in the repository —
which fails a release if any item is removed or re-signed without the baseline
being updated deliberately.

## Licence

Apache-2.0.
