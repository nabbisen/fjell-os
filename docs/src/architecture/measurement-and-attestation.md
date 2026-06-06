# Measurement and Attestation

*References RFC v0.3-001..004.*

At boot, the kernel measures every service binary into a running hash chain. `AttestationRecordV2` captures the chain at any point. The record is signed by the `HardwareTrustProvider` (Ed25519 in v0.11).
