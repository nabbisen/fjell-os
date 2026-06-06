# Capability Manifests

A `cap-manifest.toml` declares what your service requests at deploy time.

```toml
service     = "my-service"
sdk_api_rev = 1
caps        = ["Endpoint", "AuditDrain"]
rights      = ["SEND", "RECV", "AUDIT_DRAIN"]
ipc_tags    = ["tags::READY"]
intents     = [0x0101]
```

Validate with:

```bash
cargo xtask dev lint cap-manifest.toml
```

*References RFC v0.9-002.*
