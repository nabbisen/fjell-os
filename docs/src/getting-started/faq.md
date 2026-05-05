# FAQ

## Why RISC-V and not x86-64?

RISC-V has a clean privilege architecture with no legacy baggage.  The M/S/U
mode split, `satp`/Sv39, and CLINT are easy to reason about and implement
correctly.  See [ADR-0001](../adr/0001-target-architecture.md).

## Does Fjell OS run on real hardware?

Not yet in v0.1.0.  The target is QEMU `virt`.  SiFive HiFive Unleashed and
similar boards are candidates for a future milestone.

## Will Fjell OS run Linux binaries?

No.  POSIX compatibility is explicitly out of scope.  Fjell OS has its own
minimal ABI designed around capabilities and synchronous IPC.

## Why no kernel heap?

A general `malloc` inside the kernel would make memory ownership harder to
reason about and would complicate future formal verification.  All kernel
data structures use fixed-capacity tables allocated from the `BootAllocator`
during init.

## Where does the name come from?

*Fjell* is Norwegian for "mountain": solid, minimal, enduring.
