# RFC-0.41-002 R1–R6: no service receives on a shared endpoint object

**Governing RFC:** [../accepted/RFC-0.41-002-no-service-receives-on-a-shared-endpoint-object.md](../accepted/RFC-0.41-002-no-service-receives-on-a-shared-endpoint-object.md)
**Handoff:** [../handoffs/RFC-0.41-002-no-service-receives-on-a-shared-endpoint-object/implementation-handoff.md](../handoffs/RFC-0.41-002-no-service-receives-on-a-shared-endpoint-object/implementation-handoff.md)

Written against base `e1be84a` (RFC-0.41-001 landed locally, so `auditd` receives on nothing — step 1).

## Where the RFC is wrong — read first

1. **There are three receivers, not two.** `configd` (`fjell-configd/src/main.rs:62`) is a real serving loop —
   `let ep = 0u32; … sys_ipc_recv(ep)` — and `neg-test` receives on **copies of its slot 0** (its blocked-receive
   tests). The architect's enumeration looked for `sys_ipc_recv(0…)` / `recv_msg(0)` and missed a literal bound to a
   variable. **`netd`'s receive is dead code**: the service exits before its event loop by design (E-084), so it
   has never run. After R1 and R2 as written, `configd` and `neg-test` would still have received on object 0, and
   the property R2 asks to state would have been false.
2. **Twenty is the right count of images holding the grant, but one of them is `init`, and it is granted
   elsewhere.** `init` is spawned by `kmain`, not through `spawn.rs`; `main.rs` installed its slot 0 to object 0
   with `ALL_NON_META`. Removing only `_ => 0` would have left it.
3. **R5's invariant "no two tasks receive on the same endpoint object" is false of this tree and cannot be made a
   structural rule.** The update path's channel (`UPDATE_CHANNEL_EP_OBJECT`, `secure-transportd` ↔ `upgraded`) and
   the `fleetd` ↔ `diagnosticsd` tools channel are deliberate bidirectional rendezvous, each side `SEND|RECV`; and
   several one-way senders are installed with `ALL_NON_META`, which carries `RECV` they never use (changing that is
   this RFC's non-goal). The check enforces the stronger R3 sentence — **no task can receive on object 0** — which is
   the one that was the defect, and says in its module doc why it does not claim the other.

## R4 — the survey (step 2), in full

The 20 images that fell to `_ => 0`. Two means, named per row: **R** = a scratch kernel log (not committed) of
every syscall that presented handle slot 0, with the calling image, over **all 28 profile boots** of one full
`test-all` (run `20261030-052941`, 32 PASS); **S** = reading the image's source. `nr` numbers are
`SyscallNumber` values (21 `IpcRecv`, 10 `CapCopy`, 11 `CapMint`, 90 `MmioMap`, 110 `DmaAlloc`, 80 `PlatformInfoGet`).
`IpcReply`'s `a0` is the reply *tag*, not a handle, so `nr 23` rows are not uses.

| Image | Verdict | R (runtime, 28 boots) | S (source) |
|---|---|---|---|
| `INIT` | **unused** — and granted by `main.rs`, not `spawn.rs` | `nr 80 ×28` (`PlatformInfoGet(0)`: an argument, not a handle) | speaks via slots 1…, named slots; no `CapHandle(0)` |
| `CONFIGD` | **uses it: receives** | `nr 21 ×28` (`IpcRecv`) | `let ep = 0u32; sys_ipc_recv(ep)` |
| `AUDITD` | unused (since RFC-0.41-001) | none | doc says "not used by this one" |
| `DEVMGR` | unused | none | none |
| `DRIVER_VIRTIO_BLK` | unused | none | none |
| `UPGRADED` | unused | none | uses `CAP_SXT_EP` = slot 6 |
| `POWERD` | unused | none | none |
| `ROOTFSD` | unused | none | none |
| `SNAPSHOTD` | unused | none | none |
| `NEG_TEST` | **uses it** | `nr 10 ×140` (copy), `nr 11 ×140` (mint), `nr 90 ×28`, `nr 110 ×28` (wrong-kind negative tests hand it slot 0) | `SLOT_OWN_EP = 0`; copies it to scratch slots and **receives on the copies** (`sys_ipc_recv(SLOT_SCRATCH_A/B)`) |
| `SVC_TIMEOUT` | unused | none | none |
| `SVC_FAULT` | unused | none | none |
| `DRIVER_VIRTIO_NET` | unused | none | its header comment calls slot 0 an `MmioRegion`; what `spawn.rs` installs there is the default endpoint (not examined further) |
| `NETD` | **receive is dead code** | none (exits before the loop) | `recv_msg` → `sys_ipc_recv_msg(0)`, only called from `run_event_loop`, which `service_main` never calls |
| `SECURE_TRANSPORTD` | unused | none | declares `CAP_SESSION = CapHandle(0)` ("Session cap from netd via cap-broker"): never installed anywhere, `dead_code` |
| `DIAGNOSTICSD` | unused | none | its `CAP_SMGR_EP` (slot 0) was deleted at RFC-0.36-001 |
| `IDENTITYD` | **not spawnable** | — | `image_for` returns `None`: no embedded image |
| `SUMMARYD` | **not spawnable** | — | same |
| `SYNCD` | unused | none | none |
| `FLEETD` | unused | none | uses slots 1 and the trigger slot |

**Checked by running vs only by reading.** Runtime: the 28 boots each started every service (I confirmed it by
name in the `audit` boot's log — every spawnable image printed something, `svc-timeout` only when `neg-test`
spawns it — and did not re-check each of the other 27 logs one by one), so the *absence* of a slot-0 use is an
observation, not only a reading. **Only built, not exercised:** `IDENTITYD` and `SUMMARYD` (no
image exists), and `netd`'s event loop (dead code). The profiles do not exercise every code path of every service;
for a service whose slot-0 use depends on a path no profile reaches, the source reading is the only evidence, and I
found no such path.

Result: three images need an endpoint — `CONFIGD`, `NEG_TEST`, `NETD` — and **each earns a dedicated arm**
(`CONFIGD_EP_OBJECT = 22`, `NEG_TEST_OWN_EP_OBJECT = 23`, `NETD_EP_OBJECT = 21`); the other 17 get none.
`NEG_TEST_OWN_EP_OBJECT` is deliberately not `NEG_TEST_EP_OBJECT` (14, `init`'s reboot/DMA trigger): a test
blocked in a receive on its scratch endpoint must not be able to swallow the trigger.

## R2 / R3 — what changed

* `spawn.rs`: `let ep_obj: Option<u32> = match image_id { … _ => None }`; the install is `if let Some(ep_obj)`.
  The comment that said "0 = shared (all non-special services)" now says there is no shared object.
* `main.rs`: three more `et.alloc()` with the same `assert_eq!` against the constants (the "constant that names an
  object nobody allocated" failure the existing comments record); `ENDPOINT_OBJECT_COUNT` 21 → 24; `init`'s slot-0
  grant removed (a comment says why). Object 0 is still allocated so every other object keeps its number.
* `fjell-abi` host tests follow: the table test now reads `Some(N)` arms and **fails if the default arm is not
  `_ => None`**; the new constants are in the "below the count" list.
* `netd`: the CSpace comment no longer calls slot 0 a NetDevice capability or "the netdev endpoint"; the constant
  `CAP_NETDEV` is `CAP_OWN_EP` and `recv_msg` uses it. `configd`: a comment says whose endpoint it is.
* **Prebuilts: none moved** (checked by `git status` after a clean rebuild; the changes in the services are
  comments and dead code). The kernel ELF is not a prebuilt; it is covered by the two-build check.

## R5 — the check, and the failing cases

`capability-topology` gained `object_zero::judge` (`tools/fjell-consistency-check/src/object_zero.rs`). The
kernel will not let a task receive on an endpoint without a `RECV`-carrying capability to it, and capabilities
only attenuate, so "no task can receive on object 0" is equivalent to "no install anywhere gives a `RECV`-carrying
capability to object 0". The check reads the `ep_obj` table (every arm must name a real nonzero object; **the
default arm must be `None`**) and every other `kind: CapKind::Endpoint` install in `spawn.rs` and `main.rs`,
resolving the object and the rights through the constants that define them (`ALL` → `ALL_NON_META` → carries
`RECV`; `PROXY_TEXT_DEMO` = `SEND|REPLY|INSPECT` → does not). An expression it cannot evaluate is a **failure**,
not a pass. The real line: `capability-topology: PASS (6 installed, 1 waived …, 7 checked; object 0: no capability
to receive on it among 18 dedicated arms and 33 other endpoint installs, and the default arm grants none)`. **The
slot figures did not move (6 / 1 / 7)** — slot 0 is still excluded from per-slot discovery; the new invariant is
reported beside them.

**Run through the real subcheck on the real tree, each change reverted and proved byte-identical:**

| Planted | Result |
|---|---|
| (control) the fixed tree | exit 0 |
| the old default `_ => Some(0)` | exit 1: ``the default arm `_ => Some(0)` hands every image without a dedicated arm an endpoint capability: a shared object`` |
| the two-receiver shape: `NETD` and `CONFIGD` both `Some(0)` | exit 1, one line **per image**: ``ep_obj arm `NETD` points slot 0 at object 0 …`` and ``… `CONFIGD` …`` |
| `init`'s old grant restored in `main.rs` | exit 1: ``main.rs:1418: an Endpoint install gives `CapRights::ALL_NON_META` -- which carries RECV -- on object 0`` |
| (after restoring) | exit 0 |

Plus unit tests in the tool for each shape, the loop form (`for (slot, object_id) in [..]`), a commented-out
default, an unknown rights name, and a missing table (230 tests in the crate).

## R6 — the stale comment

`fjell-sample-service/src/main.rs`: the three lines that named "auditd/bootctl's shared-object-0 receive loops" now
say the send is safe because slot 0 is the service's **own dedicated object (6)**. **Line-neutral** (three lines for
three), and the image did not move.

## Behavioural check of the one service that receives

`configd` is the case R4 warns about — a loop that would spin if its receive failed (`Ok(_) | Err(_) =>
reply(0)`). A scratch log (not committed) of its `IpcRecv` outcomes in a full boot: **one** receive, `a0 = 0`
(blocked), no repeat. No task shows `task.quantum_exceeded` records in the `audit` boot either.

## Found and not changed

* `proxy-text` is installed a capability to **object 0** at slot 2 (`PROXY_TEXT_DEMO`, `SEND|REPLY|INSPECT`),
  explicitly, not by the default. It is a policy demonstration; it carries no `RECV`, so the check accepts it by
  name — but after this line nothing can receive on the object it points at, so a send through it can only queue.
* `secure-transportd` declares `CAP_SESSION = CapHandle(0)` ("Session cap from netd") which no install ever
  provided; the default's slot-0 endpoint masked the absence, and now there is no slot 0 at all. The constant is
  `dead_code` and was already so. `driver-virtio-net`'s header likewise calls slot 0 an `MmioRegion`.
* `fjell-kernel/src/task/user_image.rs` still documented a shared slot-0 endpoint for tasks nothing builds; the
  comment now says so.

## Gates

See the review request.
