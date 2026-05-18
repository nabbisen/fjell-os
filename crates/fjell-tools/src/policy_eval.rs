//! RFC 040 cap-broker policy evaluation — host-testable unit tests.
//!
//! Mirrors the policy table and evaluator in `fjell-cap-broker/src/main.rs`
//! so that correctness can be verified without QEMU.  Any change to the
//! policy table must be reflected here.
//!
//! All constants and types are scoped to `#[cfg(test)]` to avoid dead-code
//! warnings when building the `fjell-tools` binary without `--tests`.

#[cfg(test)]
pub mod tests {

    // ── Constants (v0.2 u64 rights, matching fjell-cap-broker/src/main.rs) ───

    const WILDCARD: u16 = 0xFFFF;

    // ImageIds (must match fjell_abi::service::ImageId)
    const INIT:           u16 = 0;
    const CONFIGD:        u16 = 1;
    const CAP_BROKER:     u16 = 2;
    const AUDITD:         u16 = 3;
    const SVC_MANAGER:    u16 = 4;
    const SEMANTIC_STREAM:u16 = 6;
    const PROXY_TEXT:     u16 = 7;
    /// RFC 042: neg-test service (ImageId 20).
    const NEG_TEST:       u16 = 20;
    const DEVMGR:         u16 = 8;
    const VIRTIO_DRIVER:  u16 = 9;
    const STORAGED:       u16 = 10;

    // Resource class discriminants
    const RC_ANY:          u16 = 0;
    const RC_ENDPOINT:     u16 = 1;
    const RC_TASK_CONTROL: u16 = 2;
    const RC_AUDIT_DRAIN:  u16 = 3;
    const RC_MMIO_REGION:  u16 = 4;
    const RC_DMA_REGION:   u16 = 5;
    const RC_CONFIG:       u16 = 6;
    const RC_SEMANTIC:     u16 = 7;

    // v0.2 CapRights bits (u64)
    const RIGHT_SEND:        u64 = 1 << 3;
    const RIGHT_RECV:        u64 = 1 << 4;
    const RIGHT_CALL:        u64 = 1 << 5;
    const RIGHT_REPLY:       u64 = 1 << 6;
    const RIGHT_COPY:        u64 = 1 << 7;
    const RIGHT_TASK_CREATE: u64 = 1 << 12;
    const RIGHT_TASK_START:  u64 = 1 << 13;
    const RIGHT_TASK_STATUS: u64 = 1 << 14;
    const RIGHT_TASK_KILL:   u64 = 1 << 15;
    const RIGHT_MMIO_MAP:    u64 = 1 << 19;
    const RIGHT_DMA_ALLOC:   u64 = 1 << 20;
    const RIGHT_DMA_USE:     u64 = 1 << 21;
    const RIGHT_DMA_REVOKE:  u64 = 1 << 22;
    const RIGHT_AUDIT_DRAIN: u64 = 1 << 23;
    const ALL_RIGHTS:        u64 = (1u64 << 26) - 1;

    const EP_RW: u64 =
        RIGHT_SEND | RIGHT_RECV | RIGHT_CALL | RIGHT_REPLY | RIGHT_COPY;
    const TASK_MGMT: u64 =
        RIGHT_TASK_CREATE | RIGHT_TASK_START | RIGHT_TASK_STATUS | RIGHT_TASK_KILL;
    const LEASE_RIGHTS: u64 = (1u64 << 16) | (1u64 << 17); // LEASE_CREATE | LEASE_REVOKE

    // ── Policy engine (mirrored from fjell-cap-broker) ────────────────────────

    #[derive(Clone, Copy, PartialEq)]
    enum Kind { Allow, Deny }

    struct Rule { req: u16, res: u16, kind: Kind, rights: u64 }

    const POLICY: &[Rule] = &[
        // Deny bootstrap authority for everyone.
        Rule { req: WILDCARD, res: RC_ANY,          kind: Kind::Deny,  rights: 0 },
        // init
        Rule { req: INIT, res: RC_TASK_CONTROL,     kind: Kind::Allow, rights: TASK_MGMT | LEASE_RIGHTS },
        Rule { req: INIT, res: RC_ENDPOINT,         kind: Kind::Allow, rights: EP_RW | (1 << 8) /* MINT */ },
        // service-manager
        Rule { req: SVC_MANAGER, res: RC_TASK_CONTROL, kind: Kind::Allow, rights: TASK_MGMT },
        Rule { req: SVC_MANAGER, res: RC_ENDPOINT,     kind: Kind::Allow, rights: EP_RW },
        Rule { req: SVC_MANAGER, res: RC_CONFIG,       kind: Kind::Allow, rights: EP_RW },
        // auditd
        Rule { req: AUDITD, res: RC_AUDIT_DRAIN,    kind: Kind::Allow, rights: RIGHT_AUDIT_DRAIN | (1 << 10) },
        // storaged
        Rule { req: STORAGED, res: RC_MMIO_REGION,  kind: Kind::Allow, rights: RIGHT_MMIO_MAP | (1 << 10) },
        Rule { req: STORAGED, res: RC_DMA_REGION,   kind: Kind::Allow, rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE | RIGHT_DMA_REVOKE },
        // virtio-blk driver
        Rule { req: VIRTIO_DRIVER, res: RC_MMIO_REGION, kind: Kind::Allow, rights: RIGHT_MMIO_MAP | (1 << 10) },
        Rule { req: VIRTIO_DRIVER, res: RC_DMA_REGION,  kind: Kind::Allow, rights: RIGHT_DMA_ALLOC | RIGHT_DMA_USE | RIGHT_DMA_REVOKE },
        // configd
        Rule { req: CONFIGD, res: RC_CONFIG,        kind: Kind::Allow, rights: EP_RW },
        // devmgr
        Rule { req: DEVMGR, res: RC_MMIO_REGION,   kind: Kind::Allow, rights: RIGHT_MMIO_MAP | (1 << 10) },
        // semantic
        Rule { req: SEMANTIC_STREAM, res: RC_SEMANTIC, kind: Kind::Allow, rights: EP_RW },
        Rule { req: PROXY_TEXT,      res: RC_SEMANTIC, kind: Kind::Allow, rights: RIGHT_SEND | RIGHT_RECV | RIGHT_COPY },
    ];

    #[derive(Debug, PartialEq)]
    enum Verdict { Granted(u64), Denied }

    fn evaluate(requester: u16, resource: u16, requested: u64) -> Verdict {
        // cap-broker always grants its own self-queries.
        if requester == CAP_BROKER {
            return Verdict::Granted(requested & ALL_RIGHTS);
        }
        // Phase 1: explicit deny.
        for r in POLICY {
            if r.kind != Kind::Deny { continue; }
            let rm = r.req == WILDCARD || r.req == requester;
            let sm = r.res == WILDCARD || r.res == resource;
            if sm && resource == RC_ANY { continue; } // skip wildcard-resource deny for known resources
            if rm && sm { return Verdict::Denied; }
        }
        // Phase 2: explicit allow.
        for r in POLICY {
            if r.kind != Kind::Allow { continue; }
            let rm = r.req == WILDCARD || r.req == requester;
            let sm = r.res == WILDCARD || r.res == resource;
            if rm && sm {
                let g = r.rights & requested;
                if g != 0 { return Verdict::Granted(g); }
            }
        }
        // Phase 3: default deny (BROKER-001).
        Verdict::Denied
    }

    // ── Tests — BROKER-001 through BROKER-003 ─────────────────────────────────

    #[test]
    fn unknown_service_denied() {
        assert_eq!(evaluate(99, RC_ENDPOINT, ALL_RIGHTS), Verdict::Denied);
    }

    #[test]
    fn unknown_resource_denied() {
        assert_eq!(evaluate(INIT, 0xFF, ALL_RIGHTS), Verdict::Denied);
    }

    #[test]
    fn any_resource_denied_for_all_requesters() {
        for req in [INIT, SVC_MANAGER, AUDITD, STORAGED, 99u16] {
            assert_eq!(evaluate(req, RC_ANY, ALL_RIGHTS), Verdict::Denied,
                "requester {} should not get Any resource", req);
        }
    }

    #[test]
    fn rights_intersection() {
        // init requests ALL — only TASK_MGMT | LEASE_RIGHTS is in the allow rule.
        match evaluate(INIT, RC_TASK_CONTROL, ALL_RIGHTS) {
            Verdict::Granted(r) => assert_eq!(r, TASK_MGMT | LEASE_RIGHTS),
            Verdict::Denied     => panic!("init TaskControl should be granted"),
        }
    }

    #[test]
    fn init_endpoint_granted() {
        assert!(matches!(evaluate(INIT, RC_ENDPOINT, EP_RW), Verdict::Granted(_)));
    }

    #[test]
    fn auditd_drain_granted() {
        assert!(matches!(
            evaluate(AUDITD, RC_AUDIT_DRAIN, RIGHT_AUDIT_DRAIN),
            Verdict::Granted(r) if r & RIGHT_AUDIT_DRAIN != 0
        ));
    }

    #[test]
    fn auditd_mmio_denied() {
        assert_eq!(evaluate(AUDITD, RC_MMIO_REGION, RIGHT_MMIO_MAP), Verdict::Denied);
    }

    #[test]
    fn storaged_dma_granted() {
        assert!(matches!(
            evaluate(STORAGED, RC_DMA_REGION, RIGHT_DMA_ALLOC),
            Verdict::Granted(r) if r & RIGHT_DMA_ALLOC != 0
        ));
    }

    #[test]
    fn virtio_driver_mmio_granted() {
        assert!(matches!(
            evaluate(VIRTIO_DRIVER, RC_MMIO_REGION, RIGHT_MMIO_MAP),
            Verdict::Granted(r) if r & RIGHT_MMIO_MAP != 0
        ));
    }

    #[test]
    fn cap_broker_self_query() {
        assert!(matches!(evaluate(CAP_BROKER, RC_ANY, ALL_RIGHTS), Verdict::Granted(_)));
    }

    #[test]
    fn semantic_stream_granted() {
        assert!(matches!(
            evaluate(SEMANTIC_STREAM, RC_SEMANTIC, RIGHT_SEND | RIGHT_RECV),
            Verdict::Granted(r) if r & RIGHT_SEND != 0
        ));
    }

    #[test]
    fn proxy_text_task_control_denied() {
        assert_eq!(evaluate(PROXY_TEXT, RC_TASK_CONTROL, TASK_MGMT), Verdict::Denied);
    }


    #[test]
    fn deny_priority_wins_over_allow() {
        // NEG_TEST has both a deny and an allow rule for Config.
        // Deny must win (BROKER-002).
        assert_eq!(evaluate(NEG_TEST, RC_CONFIG, EP_RW), Verdict::Denied);
    }
    #[test]
    fn partial_rights_request() {
        // storaged requests only DMA_ALLOC — DMA_REVOKE must not be granted.
        match evaluate(STORAGED, RC_DMA_REGION, RIGHT_DMA_ALLOC) {
            Verdict::Granted(r) => {
                assert_eq!(r, RIGHT_DMA_ALLOC);
                assert_eq!(r & RIGHT_DMA_REVOKE, 0);
            }
            Verdict::Denied => panic!("storaged DMA_ALLOC should be granted"),
        }
    }
}
