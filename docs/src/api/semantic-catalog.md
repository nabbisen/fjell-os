# Fjell Intent Catalog v1

*Auto-generated from `crates/fjell-semantic-v1/src/catalog.rs`.
Regenerate with `cargo xtask toolkit regenerate`.*

---

## UPDATE domain (tags 0x0100–0x01FF)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0100 | UPDATE.STAGING_STARTED | `emit_update_staging_started` | Bundle enters staging pipeline |
| 0x0101 | UPDATE.STAGING_ADVANCED | `emit_update_staging_advanced` | Bundle advances one stage |
| 0x0102 | UPDATE.STAGING_FAILED | `emit_update_staging_failed` | Bundle fails staging |
| 0x0103 | UPDATE.STAGING_CONFIRMED | `emit_update_staging_confirmed` | Bundle confirmed healthy |
| 0x0110 | UPDATE.ROLLBACK_BLOCKED | `emit_update_rollback_blocked` | Rollback refused by anti-rollback |
| 0x0111 | UPDATE.ROLLBACK_TO_PREVIOUS_SLOT | `emit_update_rollback_to_previous_slot` | Rolled back to prior slot |

---

## ATTEST domain (tags 0x0120–0x012F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0120 | ATTEST.RECORD_SIGNED | `emit_attest_record_signed` | Attestation record signed |
| 0x0121 | ATTEST.RECORD_VERIFY_FAILED | `emit_attest_record_verify_failed` | Attestation verification failure |

---

## SECURITY domain (tags 0x0130–0x013F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0130 | SECURITY.REGISTRY_ENFORCING | `emit_security_registry_enforcing` | Trust registry enters enforcing mode |
| 0x0131 | SECURITY.PROVIDER_FAULTED | `emit_security_provider_faulted` | Trust provider faulted |
| 0x0132 | SECURITY.KEYRING_EPOCH_ADVANCED | `emit_security_keyring_epoch_advanced` | Keyring epoch advanced (v0.11) |

---

## NET domain (tags 0x0140–0x014F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0140 | NET.LINK_UP | `emit_net_link_up` | Network link came up |
| 0x0141 | NET.LINK_DOWN | `emit_net_link_down` | Network link went down |
| 0x0142 | NET.SXT_CHANNEL_OPENED | `emit_net_sxt_channel_opened` | Secure transport channel opened |
| 0x0143 | NET.SXT_CHANNEL_CLOSED | `emit_net_sxt_channel_closed` | Secure transport channel closed |

---

## RECOVERY domain (tags 0x0150–0x015F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0150 | RECOVERY.ENTERED | `emit_recovery_entered` | Node entered recovery mode |
| 0x0151 | RECOVERY.EXITED | `emit_recovery_exited` | Node exited recovery mode |

---

## PLATFORM domain (tags 0x0160–0x016F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0160 | PLATFORM.PROFILES_READY | `emit_platform_profiles_ready` | Boot profiles loaded and validated |

---

## HEALTH domain (tags 0x0170–0x017F)

| Tag | Symbol | Typed emitter | When |
|-----|--------|---------------|------|
| 0x0170 | HEALTH.TARGET_REACHED | `emit_health_target_reached` | Measurement target met |
| 0x0171 | HEALTH.TARGET_FAILED | `emit_health_target_failed` | Measurement target not met |

---

## Usage

All typed emitters live in `fjell_semantic_toolkit::generated`:

```rust
use fjell_semantic_toolkit::generated::{
    UpdateStagingAdvancedArgs,
    emit_update_staging_advanced,
};
```

Or via the SDK re-export:

```rust
use fjell_sdk::sdk_emit; // contains is_known_tag
// typed emitters re-exported in v0.14+
```

---

*See also: [Service Cookbook](../sdk/cookbook.md)*
