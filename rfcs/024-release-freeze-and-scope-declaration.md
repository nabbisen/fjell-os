# RFC 024: Release freeze and scope declaration for v0.1.0

**RFC ID:** 024  
**Also known as:** RFC-v0.1.x-001  
**Status:** Accepted  
**Target version:** v0.1.1  
**Affects:** `README.md`, `ROADMAP.md`, `docs/src/`

## Problem

Fjell OS has reached the end of the M0–M8 prototype arc with the v0.1.0
release.  The repository currently does not state, in one durable place,

- what v0.1.0 deliberately is (a local verified prototype),
- what v0.1.0 deliberately is not (production secure boot, remote
  attestation, networked update, POSIX, multi-user OS),
- which of its parts are development-grade (Ed25519-stub crypto via
  SHA-256 keyed under `dev-attest-m8-01`, local attestation, local
  recovery), and
- which threats it does and does not address.

Without an explicit declaration, contributors may mistake v0.1.0 for a
production-capable secure OS, and scope creep into v0.2 becomes likely.

## Proposed fix

Add the following durable artefacts before tagging v0.1.1:

| Path | Purpose |
|---|---|
| `docs/src/releases/v0.1.0-scope.md` | What v0.1.0 includes |
| `docs/src/releases/v0.1.0-limitations.md` | What v0.1.0 does *not* do |
| `docs/src/security/v0.1.0-threat-model.md` | Skeleton, expanded by RFC 027 |
| `docs/src/security/v0.1.0-known-non-goals.md` | Explicit non-goals |
| `docs/src/roadmap/v0.1.x-stabilization.md` | This release line's plan |

`README.md` must link to the limitations document near the overview.
`ROADMAP.md` must declare v0.2 as **Security Boundary Closure**.

### Required content — `v0.1.0-scope.md`

The document must list, verbatim:

- bootable minimal kernel
- memory isolation prototype
- user tasks
- syscall path
- capability-controlled IPC prototype
- service plane prototype
- semantic stream / text proxy
- persistent state store prototype
- immutable upgrade foundation
- signed release / signed policy / immutable rootfs prototype
- local measurement chain
- local attestation record
- recovery target

### Required content — `v0.1.0-limitations.md`

The document must explicitly state, verbatim:

- not production secure boot
- not hardware-rooted trust
- not remote attestation
- not networked update
- not general-purpose networking
- not POSIX
- not a desktop OS
- not a multi-user production OS
- not fully verified
- not yet security-boundary-complete

## Rationale

A single, linked, declarative statement is the cheapest way to prevent
the most common misuse of an early prototype: deployment into a context
that assumes properties it does not yet have.  The exact wording is
fixed by RFC because any softening (“mostly production-ready”) would
defeat the purpose.

## Impact

- Crates affected: none directly.  Documentation-only.
- Backward compatibility: full.
- Affects everyone reading the project.

## Test plan

- `docs/src/SUMMARY.md` references all five new pages.
- `README.md` contains a link to `docs/src/releases/v0.1.0-limitations.md`.
- `ROADMAP.md` contains a heading for v0.2 reading *Security Boundary
  Closure*.
- CI link-check job (added in RFC 025) does not flag any broken link.

## Implementation notes

- Non-goals: no new syscall, no new kernel feature, no networking, no
  production secure boot, no TPM / DICE integration, no remote
  attestation.
- The threat-model skeleton in this RFC is intentionally small; RFC 027
  expands it.  Splitting is required to keep v0.1.1 shippable.
- `LICENSE` and `NOTICE` are unchanged.
