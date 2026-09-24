//! Host tests for the fan-out engine (RFC-0.34-001 D8).
//!
//! Each test states a property the *stream* depends on: that an offer never
//! waits, that the bound is a bound, that an absent presentation is visible
//! and costs another presentation nothing.

use super::*;
use fjell_service_api::chunked::Reassembler;
use std::collections::VecDeque;
use std::vec::Vec;

const N: usize = 256;
type One = Fanout<1, N>;

fn env(len: usize, seed: u8) -> Vec<u8> {
    (0..len).map(|i| seed.wrapping_add(i as u8)).collect()
}

/// Ask until `Empty`, reassembling with the receivers' own framing. Returns
/// every envelope handed out and every report produced along the way.
fn drain<const P: usize, const M: usize>(
    f: &mut Fanout<P, M>,
    p: usize,
) -> (Vec<Vec<u8>>, Vec<Report>) {
    let mut frames = Reassembler::<8192>::new();
    let mut got = Vec::new();
    let mut reports = Vec::new();
    loop {
        let (n, r) = f.next(p);
        reports.extend(r);
        match n {
            Next::Empty => return (got, reports),
            Next::Message(m) => match m.kind {
                Kind::Begin => frames.begin(m.words[0] as usize).unwrap(),
                Kind::Chunk => frames
                    .chunk(
                        m.words[0] as usize,
                        m.words[1] as usize,
                        m.words[2] as usize,
                        m.words[3] as usize,
                    )
                    .unwrap(),
                Kind::Commit => {
                    got.push(frames.commit().unwrap().to_vec());
                    frames.reset();
                }
            },
        }
    }
}

#[test]
fn an_envelope_comes_out_as_begin_chunks_commit_and_reassembles() {
    for len in [1usize, 31, 32, 33, 64, 100, 250] {
        let mut f = One::new();
        let e = env(len, 7);
        assert!(matches!(f.offer(0, &e), Offer::Queued { wake: false, .. }));
        let (got, _) = drain(&mut f, 0);
        assert_eq!(got, std::vec![e], "len {len}");
    }
}

#[test]
fn the_message_sequence_is_exactly_begin_then_ceil_chunks_then_commit() {
    let mut f = One::new();
    f.offer(0, &env(70, 1)); // 3 chunks
    let mut kinds = Vec::new();
    while let (Next::Message(m), _) = f.next(0) {
        kinds.push(m.kind);
    }
    assert_eq!(
        kinds,
        std::vec![
            Kind::Begin,
            Kind::Chunk,
            Kind::Chunk,
            Kind::Chunk,
            Kind::Commit
        ]
    );
}

#[test]
fn the_final_chunk_is_zero_padded() {
    let mut f = One::new();
    f.offer(0, &[0xAA; 33]);
    let mut last_chunk = None;
    while let (Next::Message(m), _) = f.next(0) {
        if m.kind == Kind::Chunk {
            last_chunk = Some(m.words);
        }
    }
    let w = last_chunk.unwrap();
    assert_eq!(w[0], 0xAA, "one real byte, then padding");
    assert_eq!(&w[1..], &[0, 0, 0]);
}

#[test]
fn envelopes_come_out_in_arrival_order() {
    let mut f = One::new();
    let es: Vec<_> = (0..4).map(|i| env(20 + i * 7, i as u8 * 40)).collect();
    for e in &es {
        assert!(matches!(f.offer(0, e), Offer::Queued { .. }));
    }
    assert_eq!(drain(&mut f, 0).0, es);
}

#[test]
fn asking_with_nothing_queued_parks_and_the_next_offer_wakes_once() {
    let mut f = One::new();
    assert_eq!(f.next(0).0, Next::Empty);
    assert!(f.presentation(0).unwrap().is_parked());
    // The first offer wakes it; the second finds it already woken.
    assert_eq!(
        f.offer(0, &env(10, 0)),
        Offer::Queued {
            wake: true,
            report: None
        }
    );
    assert_eq!(
        f.offer(0, &env(10, 1)),
        Offer::Queued {
            wake: false,
            report: None
        }
    );
    assert!(!f.presentation(0).unwrap().is_parked());
}

#[test]
fn an_offer_to_a_presentation_that_never_asked_needs_no_wake() {
    let mut f = One::new();
    assert_eq!(
        f.offer(0, &env(10, 0)),
        Offer::Queued {
            wake: false,
            report: None
        }
    );
}

// ── The bound ────────────────────────────────────────────────────────────────

#[test]
fn the_bound_holds_the_widest_envelope() {
    // The number the policy rests on, checked against what it must carry: the
    // widest envelope the format can produce is deliverable into an empty
    // queue, with room to spare. (It is *not* room for two of them.)
    let widest = fjell_semantic_format::wire::MAX_WIRE_BYTES;
    assert!(
        RING_BYTES >= widest + LEN_PREFIX,
        "{RING_BYTES} vs {widest}"
    );
    let mut f: Fanout<1, RING_BYTES> = Fanout::new();
    assert!(matches!(
        f.offer(0, &std::vec![0u8; widest]),
        Offer::Queued { .. }
    ));
    // And a second widest one does not fit, which is the drop policy at work.
    assert!(matches!(
        f.offer(0, &std::vec![0u8; widest]),
        Offer::Dropped { .. }
    ));
}

#[test]
fn a_full_queue_drops_the_newest_and_keeps_the_rest_intact() {
    let mut f = One::new();
    // 256 / (2 + 60) = 4 fit.
    let es: Vec<_> = (0..6).map(|i| env(60, i as u8)).collect();
    let results: Vec<_> = es.iter().map(|e| f.offer(0, e)).collect();
    assert!(
        results[..4]
            .iter()
            .all(|r| matches!(r, Offer::Queued { .. }))
    );
    assert!(
        results[4..]
            .iter()
            .all(|r| matches!(r, Offer::Dropped { .. }))
    );
    assert_eq!(f.presentation(0).unwrap().dropped_total(), 2);
    assert_eq!(
        drain(&mut f, 0).0,
        es[..4],
        "the first four, in order, whole"
    );
}

#[test]
fn queued_bytes_never_exceed_the_bound_whatever_is_offered() {
    let mut f = One::new();
    for i in 0..10_000usize {
        f.offer(0, &env(1 + i % 200, i as u8));
        let p = f.presentation(0).unwrap();
        assert!(p.queued_bytes() <= N);
        assert!(p.high_water() <= N);
    }
}

#[test]
fn an_envelope_larger_than_the_ring_is_dropped_and_counted() {
    let mut f = One::new();
    assert!(matches!(f.offer(0, &env(N, 0)), Offer::Dropped { .. }));
    assert!(matches!(f.offer(0, &env(N - 1, 0)), Offer::Dropped { .. }));
    assert!(matches!(f.offer(0, &env(N - 2, 0)), Offer::Queued { .. }));
    assert_eq!(f.presentation(0).unwrap().dropped_total(), 2);
}

#[test]
fn an_envelope_longer_than_a_length_prefix_can_say_is_dropped() {
    let mut f: Fanout<1, 100_000> = Fanout::new();
    assert!(matches!(
        f.offer(0, &std::vec![0u8; 70_000]),
        Offer::Dropped { .. }
    ));
}

// ── Absence is visible, and costs nobody else anything ──────────────────────

#[test]
fn a_presentation_that_never_asks_never_stops_an_offer() {
    // The D8 property in its plainest form: 100,000 publishes against a
    // presentation that has never asked. Every offer returns; memory stays
    // bounded; and the fault says so `log2(n)` times, not `n` times.
    let mut f = One::new();
    let mut reports = 0usize;
    for i in 0..100_000usize {
        if let Offer::Dropped { report: Some(_) } = f.offer(0, &env(40, i as u8)) {
            reports += 1;
        }
    }
    let p = f.presentation(0).unwrap();
    assert!(p.queued_bytes() <= N);
    let expected_drops = 100_000 - (N / 42) as u64;
    assert_eq!(p.dropped_total(), expected_drops);
    assert!(
        reports <= 20,
        "{reports} reports for {expected_drops} drops"
    );
    assert!(reports >= 10);
}

#[test]
fn reports_come_at_the_first_drop_and_at_each_power_of_two() {
    let mut f = One::new();
    while matches!(f.offer(0, &env(60, 0)), Offer::Queued { .. }) {}
    // The offer that just failed was drop #1.
    let mut seen = Vec::new();
    // Drop #1's report was consumed by the loop above; count the rest.
    for _ in 0..40 {
        if let Offer::Dropped {
            report: Some(Report::NotTaking { dropped, .. }),
        } = f.offer(0, &env(60, 0))
        {
            seen.push(dropped);
        }
    }
    assert_eq!(seen, std::vec![2, 4, 8, 16, 32]);
}

#[test]
fn a_report_says_whether_the_presentation_ever_asked() {
    let mut f = One::new();
    while matches!(f.offer(0, &env(60, 0)), Offer::Queued { .. }) {}
    // Drop the report for drop #1 was swallowed above; force the next one.
    let r = loop {
        if let Offer::Dropped { report: Some(r) } = f.offer(0, &env(60, 0)) {
            break r;
        }
    };
    assert!(matches!(
        r,
        Report::NotTaking {
            never_asked: true,
            ..
        }
    ));

    let mut g = One::new();
    g.next(0); // it asked (and is parked)
    g.offer(0, &env(60, 0));
    while matches!(g.offer(0, &env(60, 0)), Offer::Queued { .. }) {}
    let r = loop {
        if let Offer::Dropped { report: Some(r) } = g.offer(0, &env(60, 0)) {
            break r;
        }
    };
    assert!(matches!(
        r,
        Report::NotTaking {
            never_asked: false,
            ..
        }
    ));
}

#[test]
fn a_presentation_that_drains_after_dropping_reports_resumed() {
    let mut f = One::new();
    for _ in 0..10 {
        f.offer(0, &env(60, 0));
    }
    let dropped = f.presentation(0).unwrap().dropped_total();
    assert!(dropped > 0);
    let (_, reports) = drain(&mut f, 0);
    assert_eq!(reports, std::vec![Report::Resumed { dropped }]);
    // And only once: the run is over.
    assert_eq!(drain(&mut f, 0).1, Vec::<Report>::new());
}

#[test]
fn a_presentation_still_behind_does_not_report_resumed() {
    let mut f = One::new();
    for _ in 0..10 {
        f.offer(0, &env(60, 0));
    }
    // One message is not "caught up".
    let (_, r) = f.next(0);
    assert_eq!(r, None);
}

#[test]
fn one_absent_presentation_costs_another_nothing() {
    // Presentation 0 never starts. Presentation 1 gets every envelope, in
    // order, whole — the E-058 shape, with a second presentation in it.
    let mut f: Fanout<2, 4096> = Fanout::new();
    let es: Vec<_> = (0..30).map(|i| env(50 + i, i as u8)).collect();
    let mut got = Vec::new();
    for e in &es {
        f.offer(0, e);
        f.offer(1, e);
        got.extend(drain(&mut f, 1).0);
    }
    assert_eq!(got, es);
    assert_eq!(f.presentation(1).unwrap().dropped_total(), 0);
    assert!(!f.presentation(0).unwrap().has_asked());
}

#[test]
fn a_presentation_that_stops_mid_envelope_leaves_the_rest_alone() {
    // Presentation 0 takes three messages of an envelope and never asks again
    // (the relay blocked on a dead peer, say). Nothing the stream does from
    // here on can wait for it.
    let mut f: Fanout<2, 512> = Fanout::new();
    f.offer(0, &env(100, 1));
    f.offer(1, &env(100, 1));
    for _ in 0..3 {
        assert!(matches!(f.next(0).0, Next::Message(_)));
    }
    let mut good = Vec::new();
    for i in 0..500usize {
        let e = env(30, i as u8);
        f.offer(0, &e);
        f.offer(1, &e);
        good.extend(drain(&mut f, 1).0);
    }
    assert_eq!(good.len(), 500 + 1);
    assert!(f.presentation(0).unwrap().dropped_total() > 0);
    assert!(f.presentation(0).unwrap().queued_bytes() <= 512);
}

#[test]
fn an_index_that_names_no_presentation_is_harmless() {
    let mut f = One::new();
    assert_eq!(f.offer(7, &env(4, 0)), Offer::Dropped { report: None });
    assert_eq!(f.next(7), (Next::Empty, None));
}

// ── Model check ──────────────────────────────────────────────────────────────

/// A plain `VecDeque` model of the policy: accepted iff it fits in the bytes
/// left (counting length prefixes and the envelope still being handed out),
/// handed out in order, whole.
#[test]
fn the_engine_agrees_with_a_plain_queue_over_many_operations() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut rnd = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut f: Fanout<1, 300> = Fanout::new();
    let mut model: VecDeque<Vec<u8>> = VecDeque::new();
    let mut out_frames = Reassembler::<8192>::new();
    let mut delivered: Vec<Vec<u8>> = Vec::new();
    let mut expected: Vec<Vec<u8>> = Vec::new();
    for step in 0..20_000usize {
        if rnd() % 2 == 0 {
            let e = env(1 + (rnd() % 120) as usize, step as u8);
            let used: usize = model.iter().map(|m| LEN_PREFIX + m.len()).sum();
            let fits = LEN_PREFIX + e.len() <= 300 - used;
            match f.offer(0, &e) {
                Offer::Queued { .. } => {
                    assert!(fits, "engine accepted what the model says cannot fit");
                    model.push_back(e);
                }
                Offer::Dropped { .. } => assert!(!fits, "engine dropped what fits"),
            }
        } else {
            match f.next(0).0 {
                Next::Empty => assert!(model.is_empty()),
                Next::Message(m) => match m.kind {
                    Kind::Begin => out_frames.begin(m.words[0] as usize).unwrap(),
                    Kind::Chunk => out_frames
                        .chunk(
                            m.words[0] as usize,
                            m.words[1] as usize,
                            m.words[2] as usize,
                            m.words[3] as usize,
                        )
                        .unwrap(),
                    Kind::Commit => {
                        delivered.push(out_frames.commit().unwrap().to_vec());
                        out_frames.reset();
                        expected.push(model.pop_front().unwrap());
                    }
                },
            }
        }
        assert!(f.presentation(0).unwrap().queued_bytes() <= 300);
    }
    assert!(
        delivered.len() > 500,
        "the run must actually deliver things"
    );
    assert_eq!(delivered, expected);
}

// ── Behind: absence is visible before anything is lost ──────────────────────

/// Offer `n` small envelopes to a roomy presentation and collect the reports.
fn offer_many(f: &mut Fanout<1, 4096>, n: usize) -> Vec<Report> {
    let mut reports = Vec::new();
    for i in 0..n {
        match f.offer(0, &env(20, i as u8)) {
            Offer::Queued { report, .. } | Offer::Dropped { report } => reports.extend(report),
        }
    }
    reports
}

#[test]
fn a_presentation_that_never_asks_is_reported_behind_long_before_anything_drops() {
    let mut f: Fanout<1, 4096> = Fanout::new();
    let reports = offer_many(&mut f, 40);
    assert_eq!(
        f.presentation(0).unwrap().dropped_total(),
        0,
        "nothing lost"
    );
    assert_eq!(
        reports,
        std::vec![
            Report::Behind {
                unasked: 8,
                never_asked: true
            },
            Report::Behind {
                unasked: 16,
                never_asked: true
            },
            Report::Behind {
                unasked: 32,
                never_asked: true
            },
        ]
    );
}

#[test]
fn one_that_asked_once_and_stopped_is_reported_behind_as_having_asked() {
    let mut f: Fanout<1, 4096> = Fanout::new();
    f.offer(0, &env(20, 0));
    f.next(0); // one message taken, then it stops (the relay blocked on a dead peer)
    let reports = offer_many(&mut f, 8);
    assert_eq!(
        reports,
        std::vec![Report::Behind {
            unasked: 8,
            never_asked: false
        }]
    );
}

#[test]
fn a_presentation_that_keeps_asking_is_never_reported_behind() {
    let mut f: Fanout<1, 4096> = Fanout::new();
    for i in 0..1000usize {
        match f.offer(0, &env(20, i as u8)) {
            Offer::Queued { report, .. } | Offer::Dropped { report } => assert_eq!(report, None),
        }
        // It asks between offers, as a busy-but-alive one does, and takes
        // everything queued each time.
        let (_, reports) = drain(&mut f, 0);
        assert_eq!(reports, Vec::<Report>::new());
    }
}

#[test]
fn a_presentation_that_falls_behind_then_drains_reports_resumed_once() {
    let mut f: Fanout<1, 4096> = Fanout::new();
    let behind = offer_many(&mut f, 10);
    assert_eq!(behind.len(), 1);
    let (got, reports) = drain(&mut f, 0);
    assert_eq!(got.len(), 10, "nothing was lost");
    assert_eq!(reports, std::vec![Report::Resumed { dropped: 0 }]);
    assert_eq!(drain(&mut f, 0).1, Vec::<Report>::new(), "and only once");
}

#[test]
fn a_late_starting_presentation_is_reported_behind_then_resumed_and_gets_everything() {
    // The boot case: the stream runs and publishes before a presentation has
    // started. Everything queued is delivered once it does.
    let mut f: Fanout<1, 4096> = Fanout::new();
    let es: Vec<_> = (0..14).map(|i| env(60 + i, i as u8)).collect();
    for e in &es {
        f.offer(0, e);
    }
    let (got, reports) = drain(&mut f, 0);
    assert_eq!(got, es);
    assert!(reports.contains(&Report::Resumed { dropped: 0 }));
}
