# Fjell OS — Decision Log

*Consolidated register of decisions that future work must preserve or
consciously revisit. Version: v0.21.1.*

## Project / scope decisions

| ID | Decision | Why | Consequence | Source |
|---|---|---|---|---|
| DEC-001 | v1.0 is a narrow, supported QEMU profile, not a production OS | Honest scoping; the project would over-claim otherwise | Release notes must explicitly state every non-claim | `docs/release/v1.0-release-notes.md` |
| DEC-002 | v1.0.0 cannot be tagged, published, or announced without explicit owner confirmation | Single human authority over the release event | No CI job or agent may apply the v1.0.0 tag | Architect review v0.20.0 §7 |
| DEC-005 | Store/upgrade negative profiles deferred from the v1 gate | Late-stage scope control | Must be documented as non-gated; mandatory for v1.1 | Architect review v0.20.0 §4.2 |
| DEC-006 | svc READY negative pair accepted as partial (2/4) for v1.0 | Timing-sensitive; not yet deterministic | Do not claim full service-lifecycle coverage | Architect review v0.20.0 §4.3 |

## Design / architecture decisions

| ID | Decision | Alternatives | Rationale | Source |
|---|---|---|---|---|
| DEC-003 | Selective Verus boundary: capability + lease predicates are release-required; boot-control is pilot-only | Verify whole kernel; verify nothing | Prove the security-critical pure logic; test the runtime plumbing | RFC-v0.18-001 |
| DEC-004 | Crate subdirectory grouping (arch/drivers/formats/services) | Keep 80 flat crates | Navigability; crate names stay path-independent | CHANGELOG v0.21.0 |
| TR-02 | User-space service plane over a micro-kernel | Monolithic kernel | Minimal TCB; policy out of the kernel | ADR-0001 |
| TR-05 | No kernel heap; fixed-capacity tables from `BootAllocator` | General allocator | Reasoning + future verification | ADR / design philosophy |

## Implementation decisions

| ID | Decision | Reason | Where enforced |
|---|---|---|---|
| IMP-01 | `forbid(unsafe_code)` outside kernel/arch; every unsafe site categorised | Small auditable TCB | Gate 2 (unsafe-audit) |
| IMP-02 | Every `write_volatile` carries `MMIO-ORDER:` | Ordering correctness | Gate 3 (mmio-audit) |
| IMP-03 | No `static mut` in services | BSS-write page faults in no_std RISC-V | Service bootstrap pattern |
| IMP-04 | Disk image via `File::create + set_len` | No `qemu-img` dependency (Arch ships it separately) | `qemu_run.rs` |
| IMP-05 | ABI surface frozen; snapshot forbids removals | Downstream stability | Gate 4 (abi-snapshot) |
| IMP-06 | Release archive unpacks to `fjell-os-v{version}/`, no nesting | Clean extraction | `package-release` xtask |
| E-010 | IPC word count packed in tag bits 16–23; identity at a6; badge dropped | Correct payload delivery | `docs/rfcs/ERRATA.md` §E-010; `docs/src/abi/ipc-register-layout.md` |
| H-02 | `CapError::WrongKind → SysError::WrongType` (canonical) | Removed divergence from `to_sys_error()` | `trap/syscall.rs` (v0.20.1) |

## Security decisions

| ID | Decision | Why | Source |
|---|---|---|---|
| SEC-02 | No silent TOFU; `--allow-tofu-provision` required | Prevents accidental trust-on-first-use | RFC-v0.17-001 |
| SEC-03 | Ed25519/RFC 8032 signature verification before execution | Authenticity of deployed binaries | `fjell-sig-ed25519` |
| SEC-04 | Audited upstream crypto (dalek, Argon2id, AES-256-GCM); no custom crypto | Reduce cryptographic risk | dependency manifest |
| SEC-06 | No plaintext secrets in repo or release archive; `provision/` excluded | Supply-chain hygiene | `package-release` exclusions |

## Errata (closed)

| ID | Issue | Resolution |
|---|---|---|
| E-004 | VisionFive 2 profile never booted on silicon | ACCEPTED as v1.0 limitation; hardware bring-up is post-v1.0 |
| E-010 | IPC words ABI broken (word count not packed; badge collision) | FIXED v0.20.0; register layout now normative |

For the live errata register read `docs/rfcs/ERRATA.md` (Gate 7 requires 0 OPEN
entries).
