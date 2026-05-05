# Local Development

## Prerequisites

| Tool | Version | Purpose |
|---|---|---|
| Rust | 1.91 | Kernel and tools |
| `riscv64gc-unknown-none-elf` target | — | Bare-metal kernel build |
| `qemu-system-riscv64` | ≥ 7.0 | Boot and smoke tests |

### Install Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install 1.91
rustup target add riscv64gc-unknown-none-elf
```

### Install QEMU (Ubuntu/Debian)

```sh
sudo apt-get install qemu-system-misc
```

### Install QEMU (macOS)

```sh
brew install qemu
```

## Build

### ホスト側クレート（サービス群・ツール）

```sh
cargo check          # 高速フィードバック
cargo build          # ホスト向けバイナリ生成
cargo test --package fjell-cap
cargo test --package fjell-ipc
cargo test --package fjell-audit-format
```

`default-members` に `fjell-kernel` は含まれていないため、フラグなしの
`cargo build` / `cargo check` はホスト側クレートのみを対象にします。

### カーネル（RISC-V クロスコンパイル）

```sh
cargo check --package fjell-kernel --target riscv64gc-unknown-none-elf
cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release
```

> **⚠️ よくある間違い**
>
> ```sh
> # NG: --package なし → default-members 全員が RISC-V 向けになり std クレートが失敗
> cargo build --target riscv64gc-unknown-none-elf
>
> # NG: --target なし → ホスト向けになりアセンブリ命令でエラー
> cargo build --package fjell-kernel
>
> # OK: 両方セットで指定する
> cargo build --package fjell-kernel --target riscv64gc-unknown-none-elf --release
> ```
>
> `fjell-kernel` は `no_std` / `no_main` のベアメタルバイナリです。
> `--package` と `--target` は常にセットで指定してください。

### QEMU で起動（推奨）

`cargo xtask qemu` はビルドと起動を一括で行うため、上記コマンドを手打ちするより簡単です。

```sh
cargo xtask qemu          # インタラクティブ起動（Ctrl-A X で終了）
cargo xtask qemu-test     # スモークテスト（10 秒タイムアウト、非インタラクティブ）
```

## Documentation

```sh
# Install mdBook once
cargo install mdbook

# Serve docs locally
cd docs && mdbook serve --open
```

## Workspace layout

```
fjell-os/
├── Cargo.toml              workspace root
├── .cargo/config.toml      xtask alias
├── crates/
│   ├── fjell-kernel/
│   │   ├── .cargo/config.toml   RISC-V target + QEMU runner
│   │   ├── link.ld
│   │   └── src/
│   │       ├── main.rs
│   │       ├── boot.rs
│   │       ├── uart.rs
│   │       └── console.rs
│   ├── fjell-arch/         arch-specific primitives
│   ├── fjell-abi/          stable ABI types
│   ├── fjell-cap/          capability model (host-testable)
│   ├── fjell-ipc/          IPC state machine (host-testable)
│   ├── fjell-audit-format/ audit event schema
│   └── fjell-tools/        cargo xtask runner
├── docs/                   mdBook documentation
└── tests/
    ├── qemu/               QEMU integration test scripts
    └── integration/        host-side integration tests
```
