# RFC 046: v0.1.x release checklist

**RFC ID:** 046  
**Also known as:** RFC-v0.1.x-010  
**Status:** Proposed  
**Target version:** v0.1.4  
**Affects:** `docs/src/releases/`, release process

## Problem

The project needs a repeatable release process before v0.2
introduces major security boundary changes.  Without a checklist:

- a contributor cannot tell when a v0.1.x release is "done",
- a release may ship missing the artefacts other tools (CI,
  reproducibility check) expect,
- the CHANGELOG style drifts release-to-release.

## Proposed fix

Add `docs/src/releases/v0.1.x-release-checklist.md` describing
the release gates that must be satisfied before tagging any
v0.1.x version.

### Release gates

#### Build

- workspace builds (`cargo check --workspace --exclude fjell-kernel`)
- kernel builds (`riscv64gc-unknown-none-elf` release)
- service crates build for the target
- tools build for the host

#### Tests

- host unit tests pass (`cargo test --lib --bins` on every
  host-buildable crate)
- QEMU smoke tests pass (`TEST:M1:PASS` … `TEST:M8:PASS`)
- QEMU negative tests pass (every category enabled for this version)
- trap-register regression test passes (RFC 001)

#### Docs

- scope document updated
- limitation document updated
- threat model updated
- ABI inventory updated (if new syscalls or ABI changes)
- ADRs updated to reflect any new decisions
- CHANGELOG updated

#### Artefacts

- kernel image (ELF + flat binary)
- rootfs image, if applicable
- boot-control image, if applicable
- QEMU disk image, if applicable
- serial log from the release smoke test
- release manifest (digests of every artefact)

### CHANGELOG style (per release)

Each v0.1.x release entry follows the same rubric:

```
## [0.1.x] - YYYY-MM-DD

### Added
### Changed
### Fixed
### Security
### Known Limitations
### Deferred to v0.2
```

`Deferred to v0.2` is the bridge to the v0.2 backlog (RFC 047).

### Archive naming

Per the project instructions, the release archive name appends the
version number with underscores instead of dots:

```
fjell-os-0_1_1_tar.gz
fjell-os-0_1_2_tar.gz
…
```

## Rationale

The same checklist serves five releases (v0.1.1 through v0.1.5).
Treating release as a routine instead of a per-version negotiation
removes a common source of release-day mistakes (forgetting the
serial log, forgetting to bump version, forgetting to update
CHANGELOG).

The fixed CHANGELOG rubric makes release notes machine-readable
later if needed.

## Impact

- Documentation + procedural.  No code changes.
- Establishes the bar that v0.2 will inherit and extend (RFC 043).

## Test plan

- Document exists.
- Each subsequent v0.1.x release tag references the checklist in
  its PR description.
- Each tagged release archive contains the listed artefacts.
- A dry-run of v0.1.1 against this checklist completes
  successfully (this is the v0.1.1 implementation).

## Implementation notes

- Non-goals: production-grade security advisory process, stable v1
  compatibility policy, signed-release infrastructure (deferred to
  v0.3+).
- This RFC is the *meta* RFC of the v0.1.x line; each individual
  v0.1.x release uses it.
