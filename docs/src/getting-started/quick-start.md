# Quick Start

## Prerequisites

Install the following tools before proceeding.

### Rust 1.91 + RISC-V target

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.91
rustup target add riscv64gc-unknown-none-elf
```

### QEMU (Ubuntu / Debian)

```sh
sudo apt-get install qemu-system-misc
```

### QEMU (macOS)

```sh
brew install qemu
```

## Clone and run

```sh
git clone https://github.com/nabbisen/fjell-os
cd fjell-os

# ── ホスト側クレートの確認（フラグなしで OK） ──────────────────────────
cargo check
cargo build

# ── カーネルをビルドして QEMU で起動 ─────────────────────────────────
# --package と --target は必ずセットで指定する
cargo xtask qemu          # Ctrl-A X で終了
cargo xtask qemu-test     # スモークテスト（非インタラクティブ）
```

> **⚠️ `--target riscv64gc-unknown-none-elf` を単独で使わないこと**
>
> `cargo build --target riscv64gc-unknown-none-elf`（`--package` なし）は
> すべての `default-members` を RISC-V 向けにビルドしようとします。
> `std` を使うサービス系クレートが失敗します。
> カーネルは `cargo xtask qemu` か、
> `cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release`
> で明示的にビルドしてください。

## Expected M1 output

```
=============================
  Fjell OS kernel started.
=============================

arch  : riscv64
mach  : qemu-virt
stage : M1 bootable kernel
```

## Next steps

- Read [Architecture Overview](../internals/architecture-overview.md)
- Read [Design Philosophy](../internals/design-philosophy.md)
- Check the [ROADMAP](../../ROADMAP.md) for upcoming milestones
