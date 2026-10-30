//! RFC-0.41-001: judging what `auditd` printed against what a test did.
//!
//! The audit tier cannot assert a property of the trail by substring alone. Two of
//! its claims need a value that only the run knows:
//!
//! * **A thing a test did must appear in the trail** (a lease it revoked, a record it
//!   caused), and sometimes **by a given moment** -- before a later line the test
//!   printed. That is [`judge_echoes`]. The "by a given moment" half is what makes a
//!   *lost wakeup* observable: a record `auditd` slept through is not missing from the
//!   log for good (the next append flushes it), it is merely *late*.
//! * **The trail accounts for every record**: each of the `count` records a test
//!   issued is either printed or declared lost in a gap notice, and the notices' counts
//!   are exactly the ones not printed. That is [`judge_accounted`]. A notice that
//!   always said "some records were lost" would fail it.
//!
//! Both read only the serial log. They are pure functions of text, so their own tests
//! run on the host (including a mutation control for each: the same input with one
//! thing wrong must be refused).

/// `spec` is `"<source prefix> => <template containing {}> [=> <bound prefix>]"`.
///
/// For **every** line containing `<source prefix>`, take the decimal number that
/// follows it, substitute it for `{}` in the template, and require a line containing
/// the result. With a bound, that line must come **before the first line containing
/// `<bound prefix>` that follows the source line**.
///
/// Returns a one-line account on success. Fails if no line carries the source prefix
/// (a check that matched nothing would pass anything).
pub fn judge_echoes(text: &str, spec: &str) -> Result<String, String> {
    let parts: Vec<&str> = spec.split(" => ").collect();
    if parts.len() < 2 || parts.len() > 3 || !parts[1].contains("{}") {
        return Err(format!(
            "bad expect_echoed `{spec}`: want `<source prefix> => <template with {{}}> [=> <bound prefix>]`"
        ));
    }
    let (source, template) = (parts[0], parts[1]);
    let bound = parts.get(2).copied();
    let lines: Vec<&str> = text.lines().collect();

    let mut checked = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let Some(at) = line.find(source) else {
            continue;
        };
        let digits: String = line[at + source.len()..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            return Err(format!(
                "line {} has `{source}` but no number after it: {line}",
                i + 1
            ));
        }
        let want = template.replace("{}", &digits);
        let found = lines.iter().position(|l| l.contains(&want));
        let limit = match bound {
            Some(b) => lines
                .iter()
                .enumerate()
                .skip(i + 1)
                .find(|(_, l)| l.contains(b))
                .map(|(j, _)| j),
            None => None,
        };
        match (found, limit, bound) {
            (None, _, _) => {
                return Err(format!(
                    "`{source}{digits}` (line {}) is never echoed: no line contains `{want}`",
                    i + 1
                ));
            }
            (Some(f), Some(b), _) if f >= b => {
                return Err(format!(
                    "`{source}{digits}` (line {}) is echoed too late: `{want}` is on line {}, \
                     after the bound `{}` on line {}",
                    i + 1,
                    f + 1,
                    bound.unwrap_or(""),
                    b + 1
                ));
            }
            (Some(_), None, Some(b)) => {
                return Err(format!(
                    "`{source}{digits}` (line {}) has no bound line `{b}` after it to be early against",
                    i + 1
                ));
            }
            _ => {}
        }
        checked += 1;
    }
    if checked == 0 {
        return Err(format!("no line contains `{source}`; nothing was checked"));
    }
    Ok(format!("{checked} echoed"))
}

/// The number following `key=` in `line`.
fn number_after(line: &str, key: &str) -> Option<u64> {
    let at = line.find(key)? + key.len();
    let digits: String = line[at..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// `spec` is `"<begin prefix> => <end prefix> => noise<=N"`. A test prints the begin line just
/// before it issues a burst of probe records, and the end line just after:
/// `... count=<B> base=<N>`, the probes being the unknown-syscall numbers `N .. N+B`.
/// Each produces one audit record whose `arg0` is its number.
///
/// Requires **`printed + dropped == B + noise`** with `0 <= noise <= N`, where `printed`
/// is the number of distinct probe numbers `auditd` printed as records and `dropped` is
/// the sum of the `dropped` of every gap notice **after the begin line** -- and
/// **`dropped > 0`**, so a burst too small to overflow the ring cannot pass for a test
/// of the loss report.
///
/// **Why a tolerance, and why it is bounded.** A gap notice counts every record the
/// ring dropped while it was full, not only the probes, and a system that is still
/// finishing starting does not stop appending for the burst: here `init`'s exit
/// (`task.exit`) is appended inside the window, and it is dropped with the probes. That
/// is a real dropped record, correctly counted, and not the burst's. `N` is the number
/// of such unrelated records the test allows for. It is small on purpose: a count that
/// was wrong by a record or two in the *low* direction is refused outright (`noise`
/// cannot be negative), and one that reported the wrong quantity -- records printed,
/// a cumulative total, a constant -- is off by hundreds.
///
/// Only notices after the begin line count: the ring may also have overflowed during
/// boot, long before the burst, and that loss is real and also reported, but it is not
/// this burst's. (The test settles first, so `auditd` has printed everything queued
/// before the begin line, and boot-time notices precede it.)
pub fn judge_accounted(text: &str, spec: &str) -> Result<String, String> {
    let bad = || {
        format!("bad expect_accounted `{spec}`: want `<begin prefix> => <end prefix> => noise<=N`")
    };
    let parts: Vec<&str> = spec.split(" => ").collect();
    if parts.len() != 3 {
        return Err(bad());
    }
    let (begin, prefix) = (parts[0], parts[1]);
    let allow: u64 = parts[2]
        .strip_prefix("noise<=")
        .and_then(|n| n.parse().ok())
        .ok_or_else(bad)?;
    let begin_at = text
        .lines()
        .position(|l| l.contains(begin))
        .ok_or_else(|| format!("no line contains the begin marker `{begin}`"))?;
    let line = text
        .lines()
        .skip(begin_at)
        .find(|l| l.contains(prefix))
        .ok_or_else(|| format!("no line contains `{prefix}` after `{begin}`"))?;
    let count = number_after(line, "count=").ok_or_else(|| format!("no `count=` in: {line}"))?;
    let base = number_after(line, "base=").ok_or_else(|| format!("no `base=` in: {line}"))?;

    let mut seen = std::collections::BTreeSet::new();
    let mut dropped: u64 = 0;
    // The seq of the last record line printed so far: where each gap notice must sit.
    let mut last_seq: Option<u64> = None;
    // The burst probe the last record line was, if it was one.
    let mut last_probe: Option<u64> = None;
    // After a notice that claims probes were lost: the probe printed just before it.
    let mut before_gap: Option<u64> = None;
    let kind_key = r#""kind":"unknown_syscall","arg0":"#;
    let gap_key = r#"{"gap":{"dropped":"#;
    let after_key = r#""after_seq":"#;
    for (n, l) in text.lines().enumerate() {
        if let Some(rest) = l.strip_prefix(r#"{"seq":"#) {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            last_seq = digits.parse().ok();
            // Is this record a probe of the burst?
            last_probe = l.find(kind_key).and_then(|at| {
                l[at + kind_key.len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<u64>()
                    .ok()
                    .filter(|v| *v >= base && *v < base + count)
            });
            if let Some(v) = last_probe {
                seen.insert(v);
                // Probes dropped inside a notice leave a hole in the numbering: the
                // probe after the notice cannot be the very next number.
                if n > begin_at {
                    if let Some(p) = before_gap.take() {
                        if v < p + 2 {
                            return Err(format!(
                                "a gap notice is misplaced: probes {p} and {v} are consecutive \
                                 on either side of a notice that claims probes were lost"
                            ));
                        }
                    }
                }
            }
        } else if n >= begin_at {
            if let Some(at) = l.find(gap_key) {
                let digits: String = l[at + gap_key.len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect();
                let d = digits
                    .parse::<u64>()
                    .map_err(|_| format!("bad gap line: {l}"))?;
                dropped += d;
                // Placement 1: the notice follows the record it says the loss came after.
                let after = l.find(after_key).and_then(|a| {
                    l[a + after_key.len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<u64>()
                        .ok()
                });
                if let Some(after) = after {
                    if last_seq != Some(after) {
                        return Err(format!(
                            "a gap notice is misplaced: it says the loss came after seq {after} \
                             but the last record printed before it is seq {last_seq:?}: {l}"
                        ));
                    }
                }
                // Placement 2, independent of what the notice says: the ring drops the
                // newest, so what is lost is what the burst appended after the ring was
                // full -- the notice must follow the last probe the ring kept.
                if d > allow {
                    if last_probe.is_none() {
                        return Err(format!(
                            "a gap notice is misplaced: it claims {d} lost but the record \
                             before it is not a probe of the burst: {l}"
                        ));
                    }
                    before_gap = last_probe;
                }
            }
        }
    }
    let printed = seen.len() as u64;
    if dropped == 0 {
        return Err(format!(
            "the burst of {count} probes produced no gap notice (printed {printed}): \
             it did not overflow the ring, so it tests nothing about the loss report"
        ));
    }
    // `dropped` must cover every probe that was not printed, and may exceed it only by
    // unrelated records dropped in the same window.
    let missing = count.saturating_sub(printed);
    if dropped < missing || dropped - missing > allow {
        return Err(format!(
            "the trail does not account for the burst: issued {count}, printed {printed} \
             (so {missing} probes missing), gap notices claim {dropped} lost -- \
             {} unrelated record(s) dropped, allowed 0..={allow}",
            dropped as i64 - missing as i64
        ));
    }
    Ok(format!(
        "issued {count} = printed {printed} + lost {missing}; the notices claim {dropped} \
         ({} other task(s)' records dropped in the window, allowed {allow})",
        dropped - missing
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "\
neg-test: audit probe nr=100\n\
{\"seq\":9,\"kind\":\"unknown_syscall\",\"arg0\":100,\"arg1\":0,\"result\":-1}\n\
neg-test: audit settled\n\
neg-test: audit probe nr=101\n\
{\"seq\":10,\"kind\":\"unknown_syscall\",\"arg0\":101,\"arg1\":0,\"result\":-1}\n\
neg-test: audit settled\n";

    const SPEC: &str = "neg-test: audit probe nr= => \"kind\":\"unknown_syscall\",\"arg0\":{}, => neg-test: audit settled";

    #[test]
    fn every_probe_echoed_before_its_bound_passes() {
        assert_eq!(judge_echoes(GOOD, SPEC), Ok("2 echoed".to_string()));
    }

    /// The mutation control: the second record arrives only AFTER the bound -- a
    /// record the consumer slept through, flushed late. It must be refused.
    #[test]
    fn a_record_that_arrives_after_its_bound_is_refused() {
        let late = "\
neg-test: audit probe nr=100\n\
{\"seq\":9,\"kind\":\"unknown_syscall\",\"arg0\":100,\"arg1\":0,\"result\":-1}\n\
neg-test: audit settled\n\
neg-test: audit probe nr=101\n\
neg-test: audit settled\n\
{\"seq\":10,\"kind\":\"unknown_syscall\",\"arg0\":101,\"arg1\":0,\"result\":-1}\n";
        let e = judge_echoes(late, SPEC).unwrap_err();
        assert!(e.contains("too late"), "{e}");
        assert!(e.contains("101"), "{e}");
    }

    #[test]
    fn a_record_that_never_arrives_is_refused() {
        let missing = "neg-test: audit probe nr=100\nneg-test: audit settled\n";
        assert!(
            judge_echoes(missing, SPEC)
                .unwrap_err()
                .contains("never echoed")
        );
    }

    #[test]
    fn matching_nothing_is_not_a_pass() {
        let e = judge_echoes("unrelated\n", SPEC).unwrap_err();
        assert!(e.contains("nothing was checked"), "{e}");
    }

    #[test]
    fn a_missing_bound_line_is_refused_rather_than_ignored() {
        let no_bound =
            "neg-test: audit probe nr=100\n{\"kind\":\"unknown_syscall\",\"arg0\":100,}\n";
        // The echo exists, but the spec asked for it to be early against a bound.
        let e = judge_echoes(no_bound, SPEC).unwrap_err();
        assert!(e.contains("no bound line"), "{e}");
    }

    #[test]
    fn without_a_bound_presence_is_enough() {
        let spec = "lease arg0= => \"kind\":\"lease.revoked\",\"arg0\":{},";
        let log = "{\"seq\":1,\"kind\":\"lease.revoked\",\"arg0\":7,\"arg1\":2}\nlease arg0=7\n";
        assert!(judge_echoes(log, spec).is_ok());
        // 7 is echoed, 8 is not.
        assert!(judge_echoes(&format!("{log}lease arg0=8\n"), spec).is_err());
    }

    #[test]
    fn a_malformed_spec_is_rejected() {
        assert!(judge_echoes("x", "only one part").is_err());
        assert!(judge_echoes("x", "a => no placeholder").is_err());
    }

    // ── judge_accounted ──────────────────────────────────────────────────────

    const BSPEC: &str = "burst begins => audit burst: => noise<=0";

    fn burst_log(printed: &[u64], gaps: &[u64], count: u64, base: u64) -> String {
        let mut s = String::from("neg-test: audit burst begins\n");
        s.push_str(&format!(
            "neg-test: audit burst: count={count} base={base}\n"
        ));
        for n in printed {
            s.push_str(&format!(
                "{{\"seq\":{n},\"kind\":\"unknown_syscall\",\"arg0\":{n},\"arg1\":0,\"result\":-1}}\n"
            ));
        }
        // A notice sits right after the last record printed, and names it.
        let after = printed.last().copied().unwrap_or(0);
        for d in gaps {
            s.push_str(&format!(
                "{{\"gap\":{{\"dropped\":{d},\"after_seq\":{after}}}}}\n"
            ));
        }
        s
    }

    #[test]
    fn printed_plus_lost_equals_issued_passes() {
        // 10 probes 100..110: 6 printed, notices claim 3 + 1 = 4 lost.
        let log = burst_log(&[100, 101, 102, 103, 104, 105], &[3, 1], 10, 100);
        assert!(judge_accounted(&log, BSPEC).is_ok());
    }

    /// A notice that overstates, and one that understates, are both refused: the
    /// count is checked, not the presence of a notice.
    #[test]
    fn a_wrong_count_is_refused_in_either_direction() {
        let over = burst_log(&[100, 101, 102, 103, 104, 105], &[5], 10, 100);
        let under = burst_log(&[100, 101, 102, 103, 104, 105], &[2], 10, 100);
        assert!(
            judge_accounted(&over, BSPEC)
                .unwrap_err()
                .contains("does not account")
        );
        assert!(
            judge_accounted(&under, BSPEC)
                .unwrap_err()
                .contains("does not account")
        );
    }

    #[test]
    fn a_burst_with_no_gap_notice_tests_nothing() {
        let log = burst_log(&[100, 101, 102], &[], 3, 100);
        let e = judge_accounted(&log, BSPEC).unwrap_err();
        assert!(e.contains("no gap notice"), "{e}");
    }

    #[test]
    fn probes_outside_the_burst_range_and_repeats_are_not_counted() {
        // 99 and 110 are outside 100..110; 100 printed twice counts once.
        let log = burst_log(&[110, 99, 100, 100, 101], &[8], 10, 100);
        assert!(judge_accounted(&log, BSPEC).is_ok());
    }
}

#[cfg(test)]
mod boot_noise_tests {
    use super::*;

    /// A gap notice printed BEFORE the begin line is the boot's own loss, not the
    /// burst's, and must not be counted against it -- the failure this was written
    /// for: the first version summed every notice in the log (1383 against 278).
    #[test]
    fn a_notice_from_before_the_burst_is_not_counted() {
        let log = "\
{\"gap\":{\"dropped\":1105,\"after_seq\":255}}\n\
neg-test: audit burst begins\n\
neg-test: audit burst: count=10 base=100\n\
{\"seq\":1,\"kind\":\"unknown_syscall\",\"arg0\":100,\"arg1\":0,\"result\":-1}\n\
{\"gap\":{\"dropped\":9,\"after_seq\":1}}\n";
        assert!(judge_accounted(log, "burst begins => audit burst: => noise<=0").is_ok());
    }

    #[test]
    fn a_missing_begin_marker_is_refused() {
        let e = judge_accounted(
            "neg-test: audit burst: count=1 base=1\n",
            "burst begins => audit burst: => noise<=0",
        )
        .unwrap_err();
        assert!(e.contains("begin marker"), "{e}");
    }
}

#[cfg(test)]
mod tolerance_tests {
    use super::*;

    fn log(printed: &[u64], gap: u64) -> String {
        let mut s = String::from(
            "neg-test: audit burst begins\nneg-test: audit burst: count=10 base=100\n",
        );
        for n in printed {
            s.push_str(&format!(
                "{{\"seq\":{n},\"kind\":\"unknown_syscall\",\"arg0\":{n},\"arg1\":0,\"result\":-1}}\n"
            ));
        }
        let after = printed.last().copied().unwrap_or(0);
        s.push_str(&format!(
            "{{\"gap\":{{\"dropped\":{gap},\"after_seq\":{after}}}}}\n"
        ));
        s
    }
    const SPEC1: &str = "burst begins => audit burst: => noise<=1";
    const SPEC0: &str = "burst begins => audit burst: => noise<=0";

    #[test]
    fn one_unrelated_dropped_record_is_allowed_when_the_test_says_so() {
        // 6 printed, 4 probes missing, notices claim 5: one other record was dropped.
        let l = log(&[100, 101, 102, 103, 104, 105], 5);
        assert!(judge_accounted(&l, SPEC1).is_ok());
        assert!(judge_accounted(&l, SPEC0).is_err());
    }

    #[test]
    fn a_count_that_is_too_high_by_more_than_the_allowance_is_refused() {
        assert!(judge_accounted(&log(&[100, 101, 102, 103, 104, 105], 6), SPEC1).is_err());
    }

    /// Low is never allowed: a notice that under-reports hides a lost record.
    #[test]
    fn a_count_that_is_too_low_is_refused_whatever_the_allowance() {
        let l = log(&[100, 101, 102, 103, 104, 105], 3);
        assert!(judge_accounted(&l, "burst begins => audit burst: => noise<=9").is_err());
    }

    #[test]
    fn a_malformed_spec_is_rejected() {
        assert!(judge_accounted("x", "a => b").is_err());
        assert!(judge_accounted("x", "a => b => noise<=x").is_err());
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    const SPEC: &str = "burst begins => audit burst: => noise<=0";

    fn log(after_seq: u64) -> String {
        format!(
            "neg-test: audit burst begins\n\
             neg-test: audit burst: count=3 base=100\n\
             {{\"seq\":7,\"kind\":\"unknown_syscall\",\"arg0\":100,\"arg1\":0,\"result\":-1}}\n\
             {{\"seq\":8,\"kind\":\"unknown_syscall\",\"arg0\":101,\"arg1\":0,\"result\":-1}}\n\
             {{\"gap\":{{\"dropped\":1,\"after_seq\":{after_seq}}}}}\n"
        )
    }

    #[test]
    fn a_notice_right_after_the_record_it_names_passes() {
        assert!(judge_accounted(&log(8), SPEC).is_ok());
    }

    /// The mutation control: the same log with the notice naming a different record.
    #[test]
    fn a_notice_naming_the_wrong_record_is_refused() {
        let e = judge_accounted(&log(7), SPEC).unwrap_err();
        assert!(e.contains("misplaced"), "{e}");
    }
}

#[cfg(test)]
mod hole_tests {
    use super::*;

    const SPEC: &str = "burst begins => audit burst: => noise<=0";

    fn rec(seq: u64, kind: &str, arg0: u64) -> String {
        format!("{{\"seq\":{seq},\"kind\":\"{kind}\",\"arg0\":{arg0},\"arg1\":0,\"result\":0}}\n")
    }

    fn head() -> String {
        "neg-test: audit burst begins\nneg-test: audit burst: count=10 base=100\n".to_string()
    }

    /// Probes 100,101,102 kept; 103..=109 lost; the notice sits after 102 and the
    /// numbering jumps across it, as it must.
    #[test]
    fn a_correct_trail_with_a_hole_passes() {
        let mut l = head();
        for (i, n) in [100u64, 101, 102].iter().enumerate() {
            l.push_str(&rec(i as u64 + 1, "unknown_syscall", *n));
        }
        l.push_str("{\"gap\":{\"dropped\":7,\"after_seq\":3}}\n");
        assert!(judge_accounted(&l, SPEC).is_ok());
    }

    /// The control for the misplacement the scratch run could not see from the
    /// notice alone: the notice is printed one record early, so the last kept probe
    /// appears AFTER it and the probe numbers are consecutive across it.
    #[test]
    fn a_notice_one_record_early_is_refused() {
        let mut l = head();
        l.push_str(&rec(1, "unknown_syscall", 100));
        l.push_str(&rec(2, "unknown_syscall", 101));
        l.push_str("{\"gap\":{\"dropped\":7,\"after_seq\":2}}\n");
        l.push_str(&rec(3, "unknown_syscall", 102));
        let e = judge_accounted(&l, SPEC).unwrap_err();
        assert!(e.contains("consecutive"), "{e}");
    }

    /// And one record late: the notice follows an unrelated record, not the last probe.
    #[test]
    fn a_notice_after_an_unrelated_record_is_refused() {
        let mut l = head();
        l.push_str(&rec(1, "unknown_syscall", 100));
        l.push_str(&rec(2, "unknown_syscall", 101));
        l.push_str(&rec(3, "cap.copy", 0));
        l.push_str("{\"gap\":{\"dropped\":8,\"after_seq\":3}}\n");
        let e = judge_accounted(&l, SPEC).unwrap_err();
        assert!(e.contains("not a probe"), "{e}");
    }
}
