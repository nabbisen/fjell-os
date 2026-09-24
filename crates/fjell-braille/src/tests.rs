//! Host tests for the braille renderer (RFC-0.34-001 D4).
//!
//! **The expected cells in this file were derived by hand** from the published
//! dot patterns (a = dot 1, b = dots 1-2, c = dots 1-4 …; capital = dot 6;
//! number sign = dots 3-4-5-6; digits 1–9,0 = a–j), *not* produced by running
//! the renderer and pasting what it printed. A vector the code wrote proves the
//! code agrees with itself.

use super::*;
use fjell_semantic_format::*;
use std::string::String;
use std::vec::Vec;

fn cells_of(text: &str) -> String {
    let mut c = Cells::new();
    c.push_text(text);
    let mut s = String::new();
    for cell in c.cells() {
        s.push(char::from_u32(0x2800 + *cell as u32).unwrap());
    }
    s
}

fn lines(env: &SemanticEnvelope, width: usize) -> Vec<String> {
    let mut v = Vec::new();
    render_lines(env, width, |l| v.push(String::from(l)));
    v
}

fn action(id: u16, label: &str) -> ActionSpec {
    ActionSpec {
        action_id: ActionId(id),
        label: TextToken::new(label),
        kind: ActionKind::Confirm,
        required_capability: None,
        reversibility: Reversibility::Reversible,
        confirmation: ConfirmationPolicy::None,
    }
}

fn intent(title: &str, desc: &str, sev: Severity, actions: &[&str]) -> SemanticEnvelope {
    let mut a: FixedVec<ActionSpec, MAX_ACTIONS> = FixedVec::new();
    for (i, l) in actions.iter().enumerate() {
        assert!(a.push(action(i as u16 + 1, l)));
    }
    SemanticEnvelope::new_intent(
        NodeId {
            producer_index: 6,
            local_sequence: 1,
        },
        1,
        IntentNode {
            kind: IntentKind::ActionRequest,
            title: TextToken::new(title),
            description: TextToken::new(desc),
            severity: sev,
            actions: a,
            consequences: FixedVec::new(),
            expires_at_tick: None,
        },
    )
}

// ── The table ────────────────────────────────────────────────────────────────

#[test]
fn the_alphabet_is_the_standard_pattern_for_each_letter() {
    assert_eq!(
        cells_of("abcdefghijklmnopqrstuvwxyz"),
        "⠁⠃⠉⠙⠑⠋⠛⠓⠊⠚⠅⠇⠍⠝⠕⠏⠟⠗⠎⠞⠥⠧⠺⠭⠽⠵"
    );
}

#[test]
fn a_capital_is_the_dot_6_cell_before_its_letter() {
    assert_eq!(cells_of("Ab"), "⠠⠁⠃");
    assert_eq!(cells_of("ABC"), "⠠⠁⠠⠃⠠⠉");
}

#[test]
fn digits_take_one_number_sign_per_run_and_the_patterns_of_a_to_j() {
    assert_eq!(cells_of("1234567890"), "⠼⠁⠃⠉⠙⠑⠋⠛⠓⠊⠚");
    assert_eq!(cells_of("v0"), "⠧⠼⠚");
    assert_eq!(cells_of("7 8"), "⠼⠛⠀⠼⠓");
}

#[test]
fn a_separator_between_digits_stays_in_the_number() {
    assert_eq!(cells_of("3.14"), "⠼⠉⠲⠁⠙");
    assert_eq!(cells_of("1,5"), "⠼⠁⠂⠑");
    // …but a full stop that ends a sentence, or opens a word, does not.
    assert_eq!(cells_of("end."), "⠑⠝⠙⠲");
    assert_eq!(cells_of("a.b"), "⠁⠲⠃");
    assert_eq!(cells_of("2."), "⠼⠃⠲");
}

#[test]
fn a_letter_a_to_j_right_after_digits_is_marked_so_it_is_not_read_as_a_digit() {
    assert_eq!(cells_of("1a"), "⠼⠁⠰⠁");
    assert_eq!(cells_of("a1b"), "⠁⠼⠁⠰⠃");
    // k–z cannot be mistaken for digits and take no mark.
    assert_eq!(cells_of("12k"), "⠼⠁⠃⠅");
}

#[test]
fn punctuation_has_its_fixed_cells() {
    assert_eq!(cells_of(".,;:!?'-/"), "⠲⠂⠆⠒⠖⠦⠄⠤⠌");
    assert_eq!(cells_of("(x)"), "⠐⠣⠭⠐⠜");
}

#[test]
fn a_character_with_no_rule_is_the_eight_dot_full_cell() {
    assert_eq!(cells_of("é"), "⣿");
    assert_eq!(cells_of("a€b"), "⠁⣿⠃");
    assert_eq!(cells_of("#"), "⣿");
    // A six-dot rule never produces it, so it cannot be mistaken for text.
    assert!(!cells_of("abcdefghijklmnopqrstuvwxyz0123456789.,;:!?'-/()").contains('⣿'));
}

#[test]
fn a_realistic_identifier_from_the_demo_intent() {
    assert_eq!(cells_of("RFC-v0.23-001"), "⠠⠗⠠⠋⠠⠉⠤⠧⠼⠚⠲⠃⠉⠤⠼⠚⠚⠁");
}

// ── Wrapping ─────────────────────────────────────────────────────────────────

fn wrapped(text: &str, width: usize) -> Vec<String> {
    let mut c = Cells::new();
    c.push_text(text);
    let mut v = Vec::new();
    wrap(c.cells(), width, &mut |l| v.push(String::from(l)));
    v
}

#[test]
fn lines_wrap_at_spaces_greedily() {
    // "aa bb cc" at width 5: "aa bb" fits (5 cells), "cc" starts the next.
    assert_eq!(wrapped("aa bb cc", 5), std::vec!["⠁⠁⠀⠃⠃", "⠉⠉"]);
    // Exactly the width is allowed; one more is not.
    assert_eq!(wrapped("aa bb", 5), std::vec!["⠁⠁⠀⠃⠃"]);
    assert_eq!(wrapped("aa bbb", 5), std::vec!["⠁⠁", "⠃⠃⠃"]);
}

#[test]
fn runs_of_spaces_collapse_and_edges_are_trimmed() {
    assert_eq!(wrapped("  a   b  ", 10), std::vec!["⠁⠀⠃"]);
    assert_eq!(wrapped("   ", 10), Vec::<String>::new());
    assert_eq!(wrapped("", 10), Vec::<String>::new());
}

#[test]
fn a_word_longer_than_a_line_is_broken_at_the_width() {
    let long: String = core::iter::repeat_n('a', 12).collect();
    assert_eq!(wrapped(&long, 5), std::vec!["⠁⠁⠁⠁⠁", "⠁⠁⠁⠁⠁", "⠁⠁"]);
    // And what came before it is flushed first.
    assert_eq!(wrapped("b aaaaaaa", 5), std::vec!["⠃", "⠁⠁⠁⠁⠁", "⠁⠁"]);
}

// ── Envelopes ────────────────────────────────────────────────────────────────

/// The one envelope the QEMU tier renders: exactly what `sample-service`
/// publishes (`crates/services/fjell-sample-service`). The lines are derived by
/// hand: "normal" then a colon, the title; the description, wrapped at 40
/// ("RFC-v0.23-001 ABDD live path" is 38 cells, and " demonstration" would make
/// 52); the count of actions; then each numbered.
pub const SAMPLE_INTENT_LINES: [&str; 6] = [
    "⠝⠕⠗⠍⠁⠇⠒⠀⠎⠁⠍⠏⠇⠑⠤⠎⠑⠗⠧⠊⠉⠑⠀⠙⠑⠍⠕⠀⠊⠝⠞⠑⠝⠞",
    "⠠⠗⠠⠋⠠⠉⠤⠧⠼⠚⠲⠃⠉⠤⠼⠚⠚⠁⠀⠠⠁⠠⠃⠠⠙⠠⠙⠀⠇⠊⠧⠑⠀⠏⠁⠞⠓",
    "⠙⠑⠍⠕⠝⠎⠞⠗⠁⠞⠊⠕⠝",
    "⠁⠉⠞⠊⠕⠝⠎⠒⠀⠼⠃",
    "⠼⠁⠀⠁⠉⠅⠝⠕⠺⠇⠑⠙⠛⠑",
    "⠼⠃⠀⠗⠑⠍⠁⠏⠤⠙⠑⠧⠊⠉⠑",
];

pub fn sample_intent() -> SemanticEnvelope {
    intent(
        "sample-service demo intent",
        "RFC-v0.23-001 ABDD live path demonstration",
        Severity::Normal,
        &["acknowledge", "remap-device"],
    )
}

#[test]
fn the_sample_intent_renders_to_the_committed_lines() {
    assert_eq!(lines(&sample_intent(), DEFAULT_WIDTH), SAMPLE_INTENT_LINES);
}

#[test]
fn severity_comes_first_for_every_kind() {
    for (sev, word) in [
        (Severity::Low, "⠇⠕⠺⠒⠀"),
        (Severity::Normal, "⠝⠕⠗⠍⠁⠇⠒⠀"),
        (Severity::Important, "⠊⠍⠏⠕⠗⠞⠁⠝⠞⠒⠀"),
        (Severity::Critical, "⠉⠗⠊⠞⠊⠉⠁⠇⠒⠀"),
    ] {
        let l = lines(&intent("t", "", sev, &[]), DEFAULT_WIDTH);
        assert_eq!(l, std::vec![std::format!("{word}⠞")], "{sev:?}");
    }
}

#[test]
fn an_intent_with_no_actions_says_nothing_about_actions() {
    let l = lines(&intent("t", "", Severity::Normal, &[]), DEFAULT_WIDTH);
    assert_eq!(l.len(), 1, "title only: {l:?}");
}

#[test]
fn an_intent_with_the_maximum_actions_lists_each_numbered() {
    let labels = ["a", "b", "c", "d", "e", "f", "g", "h"];
    assert_eq!(labels.len(), MAX_ACTIONS);
    let l = lines(&intent("t", "", Severity::Normal, &labels), DEFAULT_WIDTH);
    assert_eq!(l[0], "⠝⠕⠗⠍⠁⠇⠒⠀⠞");
    assert_eq!(l[1], "⠁⠉⠞⠊⠕⠝⠎⠒⠀⠼⠓"); // "actions: 8"
    assert_eq!(l.len(), 2 + 8);
    assert_eq!(l[2], "⠼⠁⠀⠁"); // "1 a"
    assert_eq!(l[9], "⠼⠓⠀⠓"); // "8 h"
}

#[test]
fn text_at_the_maximum_length_is_wrapped_not_lost() {
    let title: String = core::iter::repeat_n('a', MAX_TEXT_BYTES).collect();
    let l = lines(&intent(&title, "", Severity::Normal, &[]), 40);
    // "normal:" (7 cells), then a 128-cell word that cannot share its line and
    // is broken at 40: 40 + 40 + 40 + 8. The blank between them is the break.
    let lens: Vec<usize> = l.iter().map(|x| x.chars().count()).collect();
    assert_eq!(lens, std::vec![7, 40, 40, 40, 8]);
}

#[test]
fn a_consequence_is_labelled() {
    let mut env = intent("t", "", Severity::Normal, &[]);
    if let SemanticPayload::Intent(n) = &mut env.payload {
        n.consequences.push(Consequence {
            level: Severity::Important,
            text: TextToken::new("it stops"),
        });
    }
    let l = lines(&env, DEFAULT_WIDTH);
    // "consequence: it stops"
    assert_eq!(l[1], "⠉⠕⠝⠎⠑⠟⠥⠑⠝⠉⠑⠒⠀⠊⠞⠀⠎⠞⠕⠏⠎");
}

fn state(status: Status, title: &str, summary: &str, facts: usize) -> SemanticEnvelope {
    let mut f: FixedVec<StateFact, MAX_FACTS> = FixedVec::new();
    for _ in 0..facts {
        assert!(f.push(StateFact {
            key: TextToken::new("k"),
            value: FactValue::U64(1),
            importance: Importance::Normal,
        }));
    }
    SemanticEnvelope::new_state(
        NodeId {
            producer_index: 1,
            local_sequence: 1,
        },
        1,
        StateNode {
            kind: StateKind::ServiceStatus,
            title: TextToken::new(title),
            summary: TextToken::new(summary),
            status,
            facts: f,
        },
    )
}

#[test]
fn a_state_leads_with_its_status_and_counts_its_facts() {
    assert_eq!(
        lines(
            &state(Status::Ok, "Verified boot status", "", 0),
            DEFAULT_WIDTH
        ),
        std::vec!["⠕⠅⠒⠀⠠⠧⠑⠗⠊⠋⠊⠑⠙⠀⠃⠕⠕⠞⠀⠎⠞⠁⠞⠥⠎"],
    );
    let l = lines(&state(Status::Failed, "x", "", 12), DEFAULT_WIDTH);
    assert_eq!(l[0], "⠋⠁⠊⠇⠑⠙⠒⠀⠭");
    assert_eq!(l[1], "⠋⠁⠉⠞⠎⠒⠀⠼⠁⠃"); // "facts: 12"
}

#[test]
fn an_event_leads_with_severity_and_result() {
    let env = SemanticEnvelope::new_event(
        NodeId {
            producer_index: 1,
            local_sequence: 1,
        },
        1,
        EventNode {
            kind: EventKind::ServiceReady,
            title: TextToken::new("Action refused"),
            description: TextToken::new(""),
            severity: Severity::Important,
            result: EventResult::Denied,
            subject: None,
            related_audit_seq: Some(7),
        },
    );
    assert_eq!(
        lines(&env, DEFAULT_WIDTH),
        std::vec!["⠊⠍⠏⠕⠗⠞⠁⠝⠞⠀⠙⠑⠝⠊⠑⠙⠒⠀⠠⠁⠉⠞⠊⠕⠝⠀⠗⠑⠋⠥⠎⠑⠙"],
        "the audit sequence is omitted on purpose"
    );
}

// ── Properties that need no vector ───────────────────────────────────────────

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn random_text(r: &mut Lcg) -> String {
    const POOL: &[char] = &[
        'a', 'b', 'z', 'A', 'Q', '0', '1', '9', '.', ',', '-', ':', ' ', ' ', '(', ')', '/', 'é',
        '#', '\'', '?', 'k', 'j',
    ];
    let n = (r.next() % (MAX_TEXT_BYTES as u64 / 2)) as usize;
    (0..n)
        .map(|_| POOL[(r.next() as usize) % POOL.len()])
        .collect()
}

#[test]
fn every_output_is_braille_patterns_within_the_width_and_never_padded() {
    let mut r = Lcg(0x1234_5678_9ABC_DEF0);
    for _ in 0..2_000 {
        let width = 5 + (r.next() % 60) as usize;
        let env = intent(
            &random_text(&mut r),
            &random_text(&mut r),
            Severity::Normal,
            &["x y"],
        );
        for l in lines(&env, width) {
            assert!(!l.is_empty());
            let cells: Vec<char> = l.chars().collect();
            assert!(cells.len() <= width, "{} > {width}: {l}", cells.len());
            assert!(
                cells.iter().all(|c| ('\u{2800}'..='\u{28FF}').contains(c)),
                "{l}"
            );
            assert_ne!(cells[0], '\u{2800}', "leading blank: {l}");
            assert_ne!(*cells.last().unwrap(), '\u{2800}', "trailing blank: {l}");
        }
    }
}

#[test]
fn the_output_is_a_function_of_the_envelope_and_width_alone() {
    let mut r = Lcg(42);
    for _ in 0..200 {
        let env = intent(
            &random_text(&mut r),
            &random_text(&mut r),
            Severity::Critical,
            &["go"],
        );
        assert_eq!(lines(&env, 33), lines(&env, 33));
    }
}

#[test]
fn a_width_of_zero_or_absurd_is_clamped_not_a_panic() {
    let env = sample_intent();
    assert!(!lines(&env, 0).is_empty());
    assert!(
        lines(&env, 100_000)
            .iter()
            .all(|l| l.chars().count() <= MAX_WIDTH)
    );
}

#[test]
fn the_widest_state_and_intent_render_without_panic() {
    let t: String = core::iter::repeat_n('(', MAX_TEXT_BYTES).collect(); // 2 cells each
    let a = [t.as_str(); MAX_ACTIONS];
    let _ = lines(&intent(&t, &t, Severity::Critical, &a), 7);
    let _ = lines(&state(Status::Warning, &t, &t, MAX_FACTS), 7);
}

// ── render_into ──────────────────────────────────────────────────────────────

#[test]
fn render_into_is_the_lines_joined_with_newlines() {
    let mut buf = [0u8; 1024];
    let n = render_into(&sample_intent(), DEFAULT_WIDTH, &mut buf).unwrap();
    let expected: String = SAMPLE_INTENT_LINES
        .iter()
        .map(|l| std::format!("{l}\n"))
        .collect();
    assert_eq!(core::str::from_utf8(&buf[..n]).unwrap(), expected);
}

#[test]
fn render_into_never_writes_a_partial_line() {
    for cap in 0..200 {
        let mut buf = [0u8; 200];
        match render_into(&sample_intent(), DEFAULT_WIDTH, &mut buf[..cap]) {
            Ok(n) => assert!(n <= cap),
            Err(Truncated { written }) => {
                assert!(written <= cap);
                let s = core::str::from_utf8(&buf[..written]).unwrap();
                assert!(s.is_empty() || s.ends_with('\n'), "cap {cap}: partial line");
            }
        }
    }
}
