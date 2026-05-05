# Architecture Overview

```
┌──────────────────────────────────────────────────┐
│  Presentation Proxy Layer                        │
│  fjell-proxy-text  (M7)                          │
│  future: voice, braille, web-like, machine API   │
├──────────────────────────────────────────────────┤
│  Semantic / Operations Interface Layer           │
│  Intent Stream · State Stream · Event Stream     │
│  fjell-semantic-format  (M7)                     │
├──────────────────────────────────────────────────┤
│  User-Space System Services                      │
│  fjell-init           (M4)                       │
│  fjell-service-manager (M4)                      │
│  fjell-auditd          (M5)                      │
│  fjell-configd         (M6)                      │
│  fjell-sample-service  (M7)                      │
├──────────────────────────────────────────────────┤
│  Fjell ABI / Syscall layer                       │
│  fjell-abi · fjell-syscall · fjell-cap           │
│  fjell-ipc · fjell-audit-format                  │
├──────────────────────────────────────────────────┤
│  Core Microkernel  (fjell-kernel)                │
│  boot · mm · task · scheduler                   │
│  trap · syscall · ipc · capability · audit ring  │
├──────────────────────────────────────────────────┤
│  Architecture layer  (fjell-arch)                │
│  CSR · Sv39 page tables · trap entry             │
│  CLINT timer · PLIC                              │
├──────────────────────────────────────────────────┤
│  QEMU virt  /  RISC-V 64 hardware               │
└──────────────────────────────────────────────────┘
```

## Crate map

| Crate | Kind | Purpose |
|---|---|---|
| `fjell-kernel` | `bin` (no_std) | Kernel binary |
| `fjell-arch` | `lib` (no_std) | RISC-V 64 primitives |
| `fjell-abi` | `lib` (no_std) | Stable kernel/user ABI |
| `fjell-syscall` | `lib` (no_std) | User-space syscall wrappers |
| `fjell-cap` | `lib` (no_std) | Capability model (pure logic) |
| `fjell-ipc` | `lib` (no_std) | IPC state machine (pure logic) |
| `fjell-audit-format` | `lib` (no_std) | Audit event schema |
| `fjell-config-format` | `lib` (no_std) | TOML config schema |
| `fjell-semantic-format` | `lib` (no_std) | Intent/State/Event stream schema |
| `fjell-service-api` | `lib` | User-space service SDK |
| `fjell-init` | `bin` | First user-space process |
| `fjell-service-manager` | `bin` | Service lifecycle manager |
| `fjell-auditd` | `bin` | Audit collection service |
| `fjell-configd` | `bin` | TOML config service |
| `fjell-proxy-text` | `bin` | Text Presentation Proxy |
| `fjell-sample-service` | `bin` | Demo service |
| `fjell-tools` | `bin` | Host `cargo xtask` runner |

## Trust boundary

Only `fjell-kernel` and its immediate boot path are in the Trusted Computing
Base (TCB).  All drivers, filesystems, and presentation logic run in user
space with explicitly-granted capabilities and cannot crash the kernel.
