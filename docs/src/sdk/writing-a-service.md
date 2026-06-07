# Writing a Service

A Fjell service is a standalone `no_std` Rust binary for
`riscv64gc-unknown-none-elf`, embedded into the kernel image at build time
and spawned into its own address space with an explicitly granted CSpace.

## 1. Create the crate from the canonical template

`fjell-storaged` is the canonical service template. Create your crate under
`crates/` and copy its three build files verbatim:

```bash
cargo new --bin crates/fjell-mysvc
cp crates/fjell-storaged/link.ld          crates/fjell-mysvc/
cp crates/fjell-storaged/build.rs         crates/fjell-mysvc/
cp -r crates/fjell-storaged/.cargo        crates/fjell-mysvc/
```

Add the crate to the workspace `members` list in the root `Cargo.toml`.

## 2. The entry point

A service exports `service_main` (see `crates/fjell-sample-service` for the
smallest complete example):

```rust
#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn service_main() -> ! {
    sys_debug_writeln("mysvc: starting");
    // ... initialise ...
    send_ready(CAP_SMGR_EP);     // signal readiness to svc-manager
    loop {
        let (tag, w0, w1) = recv_msg();
        // dispatch on tag; reply with sys_ipc_reply
    }
}
```

Two hard rules learned from real page-fault debugging:

- **No `static mut` state.** Service state must be stack-resident
  (loop-local variables); BSS writes fault in the current memory layout.
- **IPC reply ABI:** the tag goes in `a1`, not `a0` (standing invariant
  across all IPC work).

## 3. Register the service with the kernel

A new private-endpoint service touches four places:

1. `crates/fjell-kernel/src/main.rs` — allocate its endpoint (`et.alloc()`).
2. `crates/fjell-kernel/src/task/spawn.rs` — add the spawn `match` arm for
   the new image.
3. init's CSpace setup — `cs.install_raw` the endpoint capability so init
   (or the intended client) can reach it.
4. The client side — call `wait_service_ready` before the first IPC, so the
   service has signalled readiness.

## 4. Build and test

```bash
cargo xtask build-services      # cross-build + objcopy into prebuilt/
cargo xtask qemu-test m8        # boot and check markers
```

If you rebuilt the prebuilt service binaries, re-record the reproducibility
baseline (`rm tests/repro/baseline-digests.txt && cargo xtask repro-check
--skip-build`) and commit both together.

## 5. Authority comes last, and explicitly

A service has no authority it was not granted. Decide which endpoints and
rights it needs, grant the minimum, and remember the proved invariant works
for you: nothing the service mints can exceed what it was given.
