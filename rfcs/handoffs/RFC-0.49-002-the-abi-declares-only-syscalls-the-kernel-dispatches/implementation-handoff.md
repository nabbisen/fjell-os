# Developer Handoff — RFC-0.49-002

**Governing RFC:** [RFC-0.49-002](../../done/RFC-0.49-002-the-abi-declares-only-syscalls-the-kernel-dispatches.md)
**Milestone:** 0.49 · second line
**Status:** inherited from the governing RFC (Implemented, 0.49.0)
**Audience:** implementation model

---

## 1. The line was widened before it was built, because its first form was a partial fix of its own subject

The scope said "MMIO has no unmap". `MmioUnmap = 91` was declared in the published ABI and answered
`UnknownSyscall` — **a promise the kernel did not keep** — and closing one of the three undispatched
syscalls would have left the ABI advertising two more. So the line became the rule: **every declared
syscall is dispatched, or it is retired.**

## 2. A capacity figure the gates caught, not the review

The first design sized the mapping table from **call sites** — "four tasks call `sys_mmio_map`" — and
wrote 16 entries. `driver-virtio-net` and `storaged` each loop over **all eight virtio transports and
map every one**: **up to 17 live mappings in an ordinary boot.** `net-absent` and `store` went red.
`MAX_MMIO_MAPPINGS` is **64**. Counting call sites where the question is live instances is the same
class of error as the architect's blanket-grant miss, and it was caught by a tier rather than by a
reviewer.

## 3. What was built

`sys_mmio_map` reserves its record **before** any PTE (table full → `NoMemory`, failure → released).
`sys_mmio_unmap` is capability-checked through the funnel, owner-only, whole-mapping, two-phase
verified, and **never frees the device page** — observed, not asserted: the kernel logs the allocator's
free count before and after each unmap and the profile requires the absence. Death releases the
records. `bootstrap-leases` premise 5 retired and replaced by a classification of all **15** call
sites of a device-unmap primitive, with lease revocation deliberately not among them.

## 4. `DmaShare` retired — the owner's ruling, and the wording that was rejected

The implementer recommended retiring it with the documents saying bulk transfer is chunked "and that
is a stated v1 limit, **not a pending feature**". **The architect rejected that phrase and the owner
ruled: retire the syscall, do not retire the requirement.** `DmaShare` was mis-specified — it routed
*inter-service* transfer through *device DMA* memory — so `111` joins `120` in the retired-numbers
note. What replaces it is **v1.0 limitations row 28**, carrying the measured cost and the project's own
sentence that the chunked path is *"a demonstration-scale workaround, not a designed transport"*.

**The figures were published wrong twice, in both directions**: `ipc.md` carried 157/314 (computed
from the 4936-byte struct, not the 4624-byte wire size) and `chunked`'s own doc-comment said "dozens
of round trips" where the real figure is 145 chunks. The architect nearly made a third error, reading
a `n < 400` assertion that covers one payload variant as the envelope's size. They are now **147** and
**294**, stated as upper bounds for the widest envelope, and pinned by
`crates/fjell-service-api/tests/bulk_transfer_cost.rs` so they cannot drift again unseen.

## 5. The gate became a rule

`syscall-surface` reports **35 declared / 35 dispatched / 0 undispatched**; the `undispatched` key is
retired and the parser refuses it, so a new declared-but-undispatched variant **fails** the gate
instead of being recorded by it. The hold mechanism survives for a future dated exception; this use of
it ended with the retirement.
