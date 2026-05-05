# Fjell OS Documentation

Welcome to the Fjell OS documentation.

Fjell OS is a memory-safe, capability-based microkernel written in Rust,
targeting RISC-V 64 on QEMU `virt`.  Its design prioritises a verifiable
minimal core, explicit authority boundaries, and an intent-stream interface
that supports ABDD (Accessible by Default and by Design) without hard-coding
any particular presentation modality.

Use the sidebar to navigate:

- **Getting Started** — if you are new to Fjell OS
- **Reference** — ABI, capability model, IPC, audit, config
- **Internals** — architecture, unsafe policy, local development
- **ADRs** — rationale behind key design decisions
