# QEMU Negative-Test Profiles

This directory holds one TOML profile per negative-test category
recognised by `cargo xtask qemu-negative <category>`.

## Status (v0.1.1)

v0.1.1 only registers the *infrastructure*: every profile here has an
empty `expected_markers` list, which `qemu_run::run_profile` treats as
a placeholder PASS (RFC 025 §"chicken-and-egg" exemption). Real cases
land per-RFC:

- v0.1.2 (RFC 026) adds the first wave of test bodies and turns each
  placeholder profile into a real one.
- Each v0.2 RFC (031–041) adds its own `NEG:*` markers to the
  appropriate category file.

## Schema

```toml
name             = "capability"             # required
kernel           = "target/.../fjell-kernel" # optional, default = built kernel
disk             = "fjell-disk.img"         # optional
timeout_secs     = 60                       # optional, default 60
expected_markers = [                        # optional, default []
    "NEG:CAP:MISSING_RIGHT:PASS",
    "NEG:CAP:WRONG_KIND:PASS",
]
extra_args       = []                       # optional, passed to QEMU
```

An empty `expected_markers` list means the profile is a placeholder
and the runner asserts only that QEMU starts.

## Artefacts

Each run writes to `tests/qemu/artifacts/<profile-name>/`:

- `serial.log` — combined stdout/stderr from QEMU
- `qemu-command.txt` — the exact argv that was run
- `expected-markers.txt` — the markers asserted (one per line)
- `result-summary.txt` — `PASS` or `FAIL`

CI uploads this directory verbatim as a job artifact.
