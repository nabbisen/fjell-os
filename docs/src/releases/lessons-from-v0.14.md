# SDK Trial — Lessons Learned from v0.14

*Required by RFC-v0.14-002 §8. Captures every rough edge encountered
while authoring `fjell-config-sync` using only `fjell-sdk`.*

---

## L1: Catalog tag allocation is a pre-requisite

**Date:** v0.14.0 development  
**Phase:** manifest authoring  
**File:** `crates/fjell-config-sync/cap-manifest.toml`

**What happened:**
The `intents` field of `cap-manifest.toml` requires catalog tags that
already exist in the v1 catalog snapshot. The CONFIG domain tags
(`0x0501`–`0x0503`) were not in the v1 catalog, so `cargo xtask dev lint`
rejected the manifest with `UnknownIntent(0x0501)`.

**Why it was hard:**
The connection between "intent I want to emit" and "tag must pre-exist
in the frozen catalog" is not obvious from the SDK docs. A new service
author has to discover this by running lint.

**Resolution:**
Added CONFIG domain tags to a reserved range and updated the catalog
(forward reference: RFC-v0.14-003 handles typed struct generation).
For now the SDK docs are annotated with a callout.

---

## L2: `is_known_tag` returns false for newly-allocated tags without regeneration

**Date:** v0.14.0 development  
**Phase:** handler code  
**File:** `crates/fjell-config-sync/src/lib.rs`, `should_emit_report()`

**What happened:**
`fjell_sdk::sdk_emit::is_known_tag(0x0503)` returned `false` even after
adding the tag to the catalog source, because the snapshot used by the
SDK at compile time had not been regenerated (`cargo xtask toolkit regenerate`).

**Why it was hard:**
The SDK compiles the catalog into a static array. Without explicit regeneration,
the static snapshot is stale. There is no compile-time error — only a silent
`false` return.

**Resolution:**
Added a `cargo xtask toolkit regenerate` step to the service authoring
workflow. The typed emitter API (RFC-v0.14-003) turns this into a
compile-time error because the generated `emit_*` functions would not
exist for unregistered tags.

---

## L3: IPC tag space is untracked; collision risk

**Date:** v0.14.0 development  
**Phase:** handler design  
**File:** `crates/fjell-config-sync/src/lib.rs`, `ConfigIpcTag`

**What happened:**
`ConfigIpcTag::ConfigUpdate = 0xC001` was chosen arbitrarily. There is no
mechanism in `fjell-service-api::v0_7` to register or check the service's
private tag range.

**Why it was hard:**
The `v0_7` module provides only the kernel-known tag range. Private
service tags are not registered anywhere, so two independently-developed
services could silently pick the same tag value.

**Resolution:**
Accepted as a known limitation for v1.0. A service registry (analogous to
the semantic catalog but for IPC tags) is a post-v1.0 candidate.
Added to the v1.0 non-goals doc.

---

## L4: `cap-manifest.toml` requires `sdk_api_rev` but SDK version drift not caught at build

**Date:** v0.14.0 development  
**Phase:** bundle build  
**File:** `crates/fjell-config-sync/cap-manifest.toml`

**What happened:**
`sdk_api_rev = 1` is a static declaration. If the SDK bumps to revision 2,
the manifest would still say 1 but the binary would link against revision 2.
The bundle installer (RFC-v0.14-004) correctly detects this at deploy time,
but there is no build-time warning.

**Why it was hard:**
The gap between compile-time SDK version and manifest-declared version is
invisible until the installer rejects the bundle.

**Resolution:**
Filed as a v0.14.1 improvement: `cargo xtask dev lint` should compare
manifest `sdk_api_rev` against `SDK_API_REV` from the local `fjell-sdk`
crate and warn on mismatch.

---

## L5 — Runtime dispatch held at the SDK boundary (RFC-v0.16-007)

**What we learned:**
Driving `fjell-config-sync` through a full update lifecycle via its real
`handle_ipc` entry point — cold start, update, idempotent re-apply, query,
unknown-tag rejection — exercised cleanly without reaching past the SDK
surface. The v0.14 library-only trial could not have shown this; a service
can compile against the SDK yet still need a non-public hook at runtime.
It did not.

**Why it matters:**
This is the first evidence that the SDK surface is *runtime-sufficient*,
not merely *link-sufficient*, for a stateful reference service.

## L6 — Digest determinism is the convergence precondition (RFC-v0.16-007)

**What we learned:**
Two independent service instances applying the same config blob converged
to the same `ConfigDigest`. Fleet-wide config sync is only viable if this
holds, and it could only be confirmed by running two instances — exactly
what the library-only trial omitted.

**Caveat:**
The trial drives handler logic directly, not kernel-mediated IPC delivery
in QEMU. Runtime *transport* viability remains unproven and is listed as a
v1.0 limitation.

---

*Total lessons recorded: 6 (RFC-v0.14-002 §8 acceptance criterion: ≥ 1).*
