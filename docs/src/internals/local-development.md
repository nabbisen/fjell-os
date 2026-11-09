# Local Development

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | 1.98.1 (pinned exactly) | Kernel and all service crates |
| `riscv64gc-unknown-none-elf` target | — | Bare-metal kernel cross-build |
| `qemu-system-riscv64` | ≥ 7.0 | Boot, smoke, and negative tests |
| `ld.lld` (LLVM linker) | any | Cross-linker for the kernel |
| `verus` | pinned exactly — see [`verification/verus/TOOLCHAIN.lock`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.lock) | **Required to cut a release** (Gate 10); not needed to build or test |

**On Verus, precisely, because the distinction matters when provisioning a
machine:** it is **not needed to build or test Fjell** — `verus-check` falls
back to conformance-only mode when `verus` is absent, and nothing in the test
suite requires it. It **is required to cut a release**: Gate 10 of
`release-rehearsal` is release-required, **conformance-only is explicitly not
accepted as proof**, and the rehearsal exits non-zero without a real prover
PASS. So "optional" is true for a contributor and false for a release machine,
and this page used to say only the first half. See [Verus Setup](#verus-gate-10)
below.

---

## Install Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.98.1
rustup target add riscv64gc-unknown-none-elf
```

---

## Install QEMU and the linker

The kernel build requires `ld.lld`; the test harness requires
`qemu-system-riscv64`. The disk image is created in pure Rust — no
external `qemu-img` is needed.

### Ubuntu / Debian

```sh
sudo apt-get install qemu-system-misc lld
```

### Arch Linux

```sh
sudo pacman -S qemu-system-riscv lld
```

### macOS (Homebrew)

```sh
brew install qemu llvm
# add LLVM bin to PATH so ld.lld is found:
export PATH="$(brew --prefix llvm)/bin:$PATH"
```

---

## Build

### Host crates (services and tools)

```sh
cargo check            # fast feedback
cargo build            # host-side binaries
cargo test --package fjell-cap
cargo test --package fjell-ipc
cargo test --package fjell-audit-format
```

`fjell-kernel` is not in `default-members`, so `cargo build` without flags
targets host crates only.

### Kernel (RISC-V cross-compile)

```sh
cargo check --package fjell-kernel --target riscv64gc-unknown-none-elf
cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release
```

> **Common mistake:** `--package` and `--target` must always be supplied
> together for the kernel.
>
> ```sh
> # Wrong — builds all default-members for RISC-V, fails on std crates
> cargo build --target riscv64gc-unknown-none-elf
>
> # Wrong — builds for host, fails on bare-metal asm
> cargo build --package fjell-kernel
>
> # Correct
> cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release
> ```

### Full kernel + services (recommended)

```sh
cargo xtask build
```

This compiles all services for RISC-V, runs `objcopy`, and compiles the
kernel with the service binaries embedded.

---

## Run in QEMU

```sh
cargo xtask qemu            # interactive (exit with Ctrl-A then X)
cargo xtask qemu-test m8    # smoke test, non-interactive
```

---

## Test suite

### Host tiers only (fast, no QEMU)

```sh
cargo xtask test-all --no-qemu
```

### Full suite including QEMU

```sh
cargo xtask test-all
```

### Individual negative profiles

```sh
cargo xtask qemu-negative capability
cargo xtask qemu-negative ipc
# ... see tests/qemu/profiles/ for all categories
```

### Release rehearsal (all mechanical gates)

```sh
cargo xtask release-rehearsal
```

---

## Documentation

```sh
cargo install mdbook          # one-time
cd docs && mdbook serve --open
```

---

## Verus (Gate 10)

Verus runs the formal proof checks for the three pilot targets
(capability, lease, boot-control). It is **never** required to build or
test Fjell — `verus-check` falls back to conformance-only mode if `verus`
is not on `PATH`. **Gate 10 of `release-rehearsal` requires a real prover
PASS, so a release cannot be cut without it**, and conformance-only does not
count.

**The pinned version and the install recipe live in
[`verification/verus/TOOLCHAIN.md`](https://github.com/nabbisen/fjell-os/blob/main/verification/verus/TOOLCHAIN.md),
and this page deliberately does not repeat them.** It used to carry a "quick
reference" copy, which drifted: it pinned `release/0.2026.05.24.ecee80a` and
rustup `1.95.0` — both **retired on 2026-07-30** — and described Fjell's own
Rust channel as 1.91 when it is the version in the table above. Following it
installed a prover that Gate 10 then **rejected on a version mismatch**, which
is the gate working correctly against a document that was not. One pinned
recipe, in one place, cited from here.

---

## Workspace layout

```
fjell-os/
├── Cargo.toml                  workspace root
├── .cargo/config.toml          xtask alias + RISC-V linker settings
├── crates/
│   ├── arch/                   architecture trait + platform impls (3 crates)
│   │   ├── fjell-arch/         architecture-neutral trait boundary
│   │   ├── fjell-arch-riscv64/ RISC-V 64 implementation
│   │   └── fjell-arch-arm64/   ARM64 stub (future)
│   ├── drivers/                hardware device drivers (2 crates)
│   ├── formats/                data schema crates — *-format (22 crates)
│   ├── services/               RISC-V runtime programs — *d + helpers (29 crates)
│   ├── fjell-kernel/           the kernel
│   │   ├── .cargo/config.toml  QEMU runner
│   │   ├── link.ld
│   │   └── src/
│   ├── fjell-abi/              stable ABI types
│   ├── fjell-cap/              capability model (host-testable)
│   ├── fjell-ipc/              IPC state machine (host-testable)
│   ├── fjell-syscall/          userspace syscall wrappers
│   └── fjell-tools/            cargo xtask runner
├── docs/
│   ├── src/                    mdBook source (rendered docs)
│   ├── release/                operational release management docs
│   ├── security/               security and threat model docs
│   └── verification/           Verus proof infrastructure
├── rfcs/                       RFC documents
├── tests/
│   ├── qemu/                   QEMU profile definitions and artifacts
│   └── repro/                  reproducibility check baselines
└── verification/
    └── verus/                  Verus targets, TOOLCHAIN.lock, guides
```
