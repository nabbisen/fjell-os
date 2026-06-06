# RFC-v0.14-005 — Developer Mode Tooling (`--trace`, `--measure`, `--gdb`)

**Status:** Implemented (v0.14.0)
**Target version:** v0.14.0
**Parent:** v0.14-001.
**Cross-refs:** RFC v0.9-005 (dev-harness), v0.10-006 (docs).

## 1. Problem

`cargo xtask dev run` (RFC v0.9-005) launches QEMU and waits for a
PASS marker. When something goes wrong an author gets a serial log
and not much else. Three small additions make the development loop
visibly tighter:

- **`--trace`** — real-time stream of semantic intents as a service
  emits them.
- **`--measure`** — live measurement chain display so an author can
  see what their service is *measured as*.
- **`--gdb`** — GDB stub attachment to the running kernel for
  interactive debugging.

These are *developer* features. v0.14-005 ensures they cannot leak
into production: each requires the kernel to be built with a
corresponding feature flag, and a production build refuses these
flags.

## 2. `--trace`

```
cargo xtask dev run --svc fjell-config-sync --trace
```

Behaviour:

- QEMU is launched as usual.
- A secondary IPC channel from `auditd` is exposed to the host via a
  QEMU pipe (`-chardev pipe,...,id=trace`).
- The host xtask reads typed catalog records from the pipe, decodes
  them via `fjell-semantic-toolkit`, and prints them in real time.

Format on the host side:

```
[t=1.234s tick=0x00010000] 0x0501 CONFIG.UPDATED { source = secure-transport, digest = ab cd ... }
[t=1.244s tick=0x00010040] 0x0503 CONFIG.DIGEST_REPORTED { digest = ab cd ... }
```

Filtering:

- `--trace=domain:CONFIG` — only the CONFIG domain.
- `--trace=tag:0x0501` — only one tag.
- `--trace=service:fjell-config-sync` — only intents emitted by one
  service (using the kernel-attested sender identity from RFC 055).

The pipe contents are written verbatim to the run-dir log
(`tests/runs/<ts>/<run>.trace.log`) so a later forensics pass can
replay.

## 3. `--measure`

```
cargo xtask dev run --svc fjell-config-sync --measure
```

Renders the live measurement chain (RFC v0.3-004 attestation profile
v2) to the developer's terminal. Each new measurement is shown as:

```
[t=1.234s] measurement #42
    seq: 42
    prev: ab cd ...
    digest: cd ef ...
    bundle: fjell-config-sync@0.1.0
    cap_set: PersistentStore, Endpoint, AuditDrain
    note: "ready"
```

`--measure --watch` keeps the screen updating with the latest n
measurements.

The data path: a small read-only IPC tag in
`fjell-service-api::v0_7` (added in this RFC) lets a host-side helper
service tail the measurement chain. Exposed via the same QEMU pipe
mechanism as `--trace`.

## 4. `--gdb`

```
cargo xtask dev run --svc fjell-config-sync --gdb
```

QEMU is launched with `-s -S` (gdbserver on port 1234, paused). The
xtask prints:

```
[dev] kernel paused at _start. Attach with:
[dev]   gdb-multiarch -ex 'target remote :1234' \
[dev]       target/riscv64gc-unknown-none-elf/release/fjell-kernel
```

Notes:

- The kernel must be built with `feature = "dev-symbols"` to retain
  DWARF info. Production builds omit this feature; the production-
  mode gate (RFC v0.7.3-002) refuses to enter production with the
  feature on.
- A `breakpoints.gdb` script is committed at `docs/dev/breakpoints.gdb`
  with useful breakpoints (`init: ready`, `BOOT.DTB_MISMATCH`, etc.).

## 5. Production refusal

Each mode flag corresponds to a kernel feature:

| Flag | Kernel feature |
|------|-----------------|
| `--trace` | `dev-trace` |
| `--measure` | `dev-measure` |
| `--gdb` | `dev-symbols` |

The production-mode gate refuses to enter production if any of these
features are present in the built kernel. The reproducible-release
build (RFC-v0.10-003) explicitly builds without them.

A developer-friendly compile-time error fires if `cargo xtask dev run
--trace` is invoked against a kernel built without `dev-trace`:

```
error: --trace requires a kernel built with the `dev-trace` feature
       try: cargo xtask build --features dev-trace
```

## 6. Log preservation

Each developer-mode invocation writes to the run-dir:

```text
tests/runs/<ts>/
  <run>.trace.log         (if --trace)
  <run>.measure.log       (if --measure)
  <run>.gdb-session.log   (if --gdb, with breakpoints loaded)
```

Already-existing `serial.log` continues to be captured for all runs.

## 7. Acceptance criteria

1. `cargo xtask dev run --trace` works against the reference
   fleet-demo build and streams typed records.
2. `cargo xtask dev run --measure` displays the live measurement
   chain.
3. `cargo xtask dev run --gdb` launches QEMU paused and prints
   attach instructions.
4. Each mode requires the matching feature flag at build time and
   refuses otherwise with the §5 error.
5. Production-mode gate refuses dev features.
6. Logs land in `tests/runs/<ts>/`.
7. Documentation in `docs/dev/modes.md` covers each mode with a
   worked example.

## 8. Out of scope

- Time-travel debugging.
- Live-patching code under GDB.
- Recording-and-replay of services across runs.
- Remote attached debugging (would require a network-aware GDB
  stub).
- A graphical debugger / IDE integration.
- Mode flags that combine — for v0.14 each flag is exclusive; combos
  may land in v0.14.x if requested.
