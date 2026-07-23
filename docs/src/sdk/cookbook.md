# Fjell Service Cookbook

*RFC-v0.14-003 R1–R8. Each recipe is verified against `fjell-config-sync`
(RFC-v0.14-002) or the existing kernel services.*

---

## R1 — Receive an IPC message and reply

```rust
use fjell_sdk::prelude::*;
use fjell_sdk::syscall::sys_ipc_recv;

// In your service main loop:
let mut buf = [0u64; IPC_WORDS];
// Block until a message arrives on cap handle 1
match sys_ipc_recv(CapHandle(1), &mut buf) {
    Ok(msg_info) => {
        let tag = (msg_info >> 48) as u16;
        // Process and reply
        buf[0] = 0; // reply tag
        let _ = sys_ipc_call(CapHandle(1), msg_info, &mut buf);
    }
    Err(_) => { /* handle error */ }
}
```

---

## R2 — Emit a semantic intent (typed emitter)

```rust
use fjell_sdk::sdk_emit;
use fjell_semantic_toolkit::generated::{
    UpdateStagingAdvancedArgs, emit_update_staging_advanced,
};

let mut out = [0u8; 128];
let args = UpdateStagingAdvancedArgs {
    candidate_id: 1,
    from_state: 1,
    to_state: 2,
};
let n = emit_update_staging_advanced(&args, current_tick(), &mut out)
    .expect("emit ok");
// n bytes written to out — hand to audit drain
```

For `current_tick()` use the monotonic counter from your measurement
profile or `0` in testing.

---

## R3 — Request a capability and check rights

```rust
use fjell_sdk::cap::{CapHandle, CapRights, CapKind};
use fjell_sdk::syscall::sys_cap_derive;

// Attempt to derive a child cap with SEND rights from parent cap at slot 1
let parent = CapHandle(1);
let child = sys_cap_derive(parent, CapRights::SEND)?;

// The CapManifest (cap-manifest.toml) declares what you're allowed to hold.
// Requesting rights you didn't declare will be refused at the cap-broker.
```

Declare the caps you need in `cap-manifest.toml`:

```toml
caps   = ["Endpoint"]
rights = ["SEND", "RECV"]
```

---

## R4 — Open a persistent store and bind a lease

```rust
use fjell_sdk::prelude::*;
use fjell_sdk::store::{StoreHandle, StoreKey, StoreValue};

// Open the store using the PersistentStore cap granted at startup
let store = StoreHandle::open(CapHandle(SLOT_STORE))?;

// Read a value
let key = StoreKey::from_bytes(b"config.digest");
let val: Option<StoreValue> = store.get(&key)?;

// Write a value
let data = b"my config blob";
store.put(&key, StoreValue::from_bytes(data))?;
```

The lease for the store handle is automatically revoked if your service
exits or is killed by the scheduler.

---

## R5 — Emit an audit record

```rust
use fjell_sdk::audit::{AuditDrain, AuditEvent, AuditKind};

// AuditDrain cap is granted if declared in cap-manifest.toml
let drain = AuditDrain::from_cap(CapHandle(SLOT_AUDIT));

drain.emit(AuditEvent {
    kind:       AuditKind::Syscall,
    tick:       current_tick(),
    object_id:  42,
    result_ok:  true,
    extra:      0,
})?;
```

---

## R6 — Declare a CapManifest

Create `cap-manifest.toml` at the crate root:

```toml
# Required fields
service     = "my-service"
sdk_api_rev = 1           # must match SDK_API_REV at compile time

# Capability kinds you will hold
caps   = ["Endpoint", "AuditDrain", "PersistentStore"]

# Rights per cap (union of all caps; broker enforces per-cap limits)
rights = ["SEND", "RECV", "AUDIT_DRAIN", "READ", "WRITE"]

# IPC tags you will send or receive
ipc_tags = ["v0_7::SYNC_ENVELOPE", "tags::READY"]

# Semantic catalog tags you will emit (must exist in catalog v1)
intents = [0x0101, 0x0102]
```

Validate before bundling:

```bash run-verified
cargo xtask dev lint cap-manifest.toml
# dev lint: OK  service=`my-service` sdk_api_rev=1
```

---

## R7 — Package and sign a bundle

```bash run-verified
# Build the service binary
cargo build --release -p my-service

# Build a signed bundle
cargo xtask fleet-demo build-bundle

# Or with explicit signing key
cargo xtask sign-bundle \
    --bundle target/.../my-service.bundle \
    --key    .signing/release.key \
    --out    my-service.bundle.sig
```

---

## R8 — Write a service test with dev-harness

```rust
#[cfg(test)]
mod harness_tests {
    use fjell_sdk::dev_harness::TestHarness;

    #[test]
    fn service_handles_update_message() {
        let mut h = TestHarness::new();
        // Inject a CONFIG_UPDATE IPC message
        h.inject_ipc(0xC001, &[0u64; 8]);
        // Run one event-loop tick
        h.tick();
        // Assert the service emitted CONFIG.DIGEST_REPORTED
        assert!(h.saw_intent(0xC003));
    }
}
```

The dev-harness is available in `fjell_sdk::dev_harness` under the
`test-harness` feature. It does not require QEMU.

---

*See also: [SDK Overview](../sdk/overview.md), [Intent Catalog](../api/semantic-catalog.md)*
