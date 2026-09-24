//! RFC-0.33-001 D5/R6 — telling a machine reset from a hang.
//!
//! A reset in a QEMU tier looks, to a harness that matches markers in a
//! serial log, much like a machine that stopped. Measured against a bare-metal
//! probe that prints `BOOT` and then stores to QEMU `virt`'s `sifive_test`
//! device (`0x100000`), QEMU 11.1.1:
//!
//! | probe                 | flags         | exit | `BOOT` lines | QMP `SHUTDOWN`                        |
//! |-----------------------|---------------|-----:|-------------:|---------------------------------------|
//! | hang (no store)       | `-no-reboot`  |  124 |            1 | `guest: false, reason: host-signal`   |
//! | reset (`0x7777`)      | *(none)*      |  124 |       32,086 | *(killed; never reached)*             |
//! | reset (`0x7777`)      | `-no-reboot`  |    0 |            1 | `guest: true,  reason: guest-reset`   |
//! | power-off (`0x5555`)  | `-no-reboot`  |    0 |            1 | `guest: true,  reason: guest-shutdown`|
//!
//! Three consequences shape this module:
//!
//! 1. **Without `-no-reboot` a reset is a boot loop** that ends as a
//!    `timeout` kill — exit 124, exactly what a hang looks like — with the
//!    markers repeated tens of thousands of times.
//! 2. **`-no-reboot` alone is not enough**: reset and power-off both exit 0,
//!    so a kernel that stored `0x5555` instead of `0x7777` would read as
//!    success.
//! 3. **QEMU itself says which** — its QMP `SHUTDOWN` event carries `guest`
//!    (did the guest ask?) and `reason` (`guest-reset`, `guest-shutdown`,
//!    `host-signal`).
//!
//! So the evidence a reset profile accepts is QEMU's own account, not a guest
//! marker asserting one: the run must end with `SHUTDOWN{guest:true,
//! reason:"guest-reset"}` **and** QEMU must have exited by itself rather than
//! being killed by the wrapper's timeout.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

/// `reason` for a guest-initiated reset.
pub const GUEST_RESET: &str = "guest-reset";
/// `reason` for a guest-initiated power-off.
pub const GUEST_SHUTDOWN: &str = "guest-shutdown";

/// Reasons a profile may expect. `host-signal` is deliberately absent: a run
/// that ends because the harness killed it is the failure this exists to
/// detect, never an outcome to expect.
pub const EXPECTABLE: &[&str] = &[GUEST_RESET, GUEST_SHUTDOWN];

/// What QEMU reported when the guest stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shutdown {
    /// `true` when the *guest* asked (a store to the reset device); `false`
    /// when the host stopped it (a signal — the timeout).
    pub guest: bool,
    pub reason: String,
}

/// Everything the QMP watcher learned.
#[derive(Debug, Default)]
pub struct QmpResult {
    pub shutdown: Option<Shutdown>,
    /// Event names seen, in order — recorded in the run's artifact so a
    /// failed check can be read without re-running.
    pub events: Vec<String>,
    /// Set when the watcher could not complete the QMP handshake at all.
    pub error: Option<String>,
}

/// `"key": "value"` in a JSON line, without a JSON dependency — QMP lines are
/// flat enough for this, and the xtask crate keeps its dependencies minimal
/// (see `qemu_run`'s module note on the hand-rolled TOML reader).
fn json_str_field(line: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let after = &line[line.find(&pat)? + pat.len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let after = after.strip_prefix('"')?;
    Some(after[..after.find('"')?].to_string())
}

fn json_bool_field(line: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\"");
    let after = &line[line.find(&pat)? + pat.len()..];
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    if after.starts_with("true") {
        Some(true)
    } else if after.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

/// The event name of a QMP line, if it is an event.
pub fn event_name(line: &str) -> Option<String> {
    json_str_field(line, "event")
}

/// A `SHUTDOWN` event's payload. `None` for any other line — and for a
/// `SHUTDOWN` whose payload cannot be read, which is reported as no event
/// rather than guessed at.
pub fn parse_shutdown_event(line: &str) -> Option<Shutdown> {
    if event_name(line).as_deref() != Some("SHUTDOWN") {
        return None;
    }
    let data = &line[line.find("\"data\"")?..];
    Some(Shutdown {
        guest: json_bool_field(data, "guest")?,
        reason: json_str_field(data, "reason")?,
    })
}

/// Decide whether a run ended the way the profile expects, and say precisely
/// why not if it did not. Pure, so each refusal is a unit test.
///
/// The order matters: a wrapper kill (`exit 124`) is checked first, because it
/// is the case this whole module exists to catch and it must never be
/// explained away by an event that happens to be present.
pub fn judge(
    expected: &str,
    shutdown: Option<&Shutdown>,
    exit_code: Option<i32>,
    qmp_error: Option<&str>,
) -> Result<(), String> {
    if exit_code == Some(124) {
        return Err(format!(
            "QEMU was killed by the wrapper's timeout (exit 124): the machine hung or kept \
             running. A `{expected}` was not observed — without `-no-reboot` this is also what \
             a reset loop looks like"
        ));
    }
    let Some(s) = shutdown else {
        return Err(match qmp_error {
            Some(e) => format!(
                "QEMU reported no SHUTDOWN event because the QMP handshake failed ({e}); a \
                 reset cannot be told from a stop without it"
            ),
            None => "QEMU exited without reporting a SHUTDOWN event".to_string(),
        });
    };
    if !s.guest {
        return Err(format!(
            "QEMU stopped for a host reason (`{}`), not because the guest asked to `{expected}`",
            s.reason
        ));
    }
    if s.reason != expected {
        return Err(format!(
            "the guest asked for `{}`, but the profile expects `{expected}` — a store of the \
             wrong value to the reset device is exactly this",
            s.reason
        ));
    }
    Ok(())
}

/// Connect to QEMU's QMP socket, complete the handshake, and collect events
/// until QEMU closes the connection.
///
/// QEMU is started with `wait=on`, so it holds the guest until a client
/// connects — without that, a guest that resets in milliseconds (as the probe
/// does) would exit before this could attach. The connect is retried until
/// `deadline`; if it never succeeds the failure is *returned*, not swallowed,
/// and `judge` turns it into a refusal.
pub fn watch(sock: &Path, deadline: Duration) -> QmpResult {
    let mut out = QmpResult::default();
    let start = Instant::now();
    let stream = loop {
        match UnixStream::connect(sock) {
            Ok(s) => break s,
            Err(e) => {
                if start.elapsed() > deadline {
                    out.error = Some(format!("cannot connect to {}: {e}", sock.display()));
                    return out;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    };
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(e) => {
            out.error = Some(format!("cannot clone the QMP socket: {e}"));
            return out;
        }
    };
    let mut reader = BufReader::new(stream);
    let mut greeting = String::new();
    if reader.read_line(&mut greeting).is_err() || !greeting.contains("QMP") {
        out.error = Some(format!("no QMP greeting (got {greeting:?})"));
        return out;
    }
    if writer
        .write_all(b"{\"execute\":\"qmp_capabilities\"}\n")
        .and_then(|_| writer.flush())
        .is_err()
    {
        out.error = Some("could not send qmp_capabilities".to_string());
        return out;
    }
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if let Some(name) = event_name(&line) {
                    out.events.push(name);
                }
                if let Some(s) = parse_shutdown_event(&line) {
                    out.shutdown = Some(s);
                }
            }
        }
    }
    out
}

// ── Counting boots (RFC-0.33-001 D21) ─────────────────────────────────────────

/// How many times `banner` appears in `log`, non-overlapping.
///
/// The banner must be a line printed **late in every boot** — late enough that a
/// boot which hangs early never prints it. The first version of this check's
/// subject failed six lines into its second boot; counting the *first* line of
/// the kernel's output would have counted that dead boot as a boot.
pub fn count_boots(log: &[u8], banner: &[u8]) -> usize {
    if banner.is_empty() {
        return 0;
    }
    let mut n = 0;
    let mut i = 0;
    while i + banner.len() <= log.len() {
        if &log[i..i + banner.len()] == banner {
            n += 1;
            i += banner.len();
        } else {
            i += 1;
        }
    }
    n
}

/// Judge the boot count **exactly**. Fewer is a machine that did not come back;
/// more is a reset that repeats — the loop D17 refused the entropy-device
/// trigger for. Both are failures, and each says which it is.
pub fn judge_boots(expected: u32, counted: usize, banner: &str) -> Result<(), String> {
    let expected = expected as usize;
    match counted.cmp(&expected) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Less => Err(format!(
            "expected the machine to boot {expected} time(s) but `{banner}` appeared {counted}: \
             it did not come back (a boot that dies early never prints it)"
        )),
        std::cmp::Ordering::Greater => Err(format!(
            "expected the machine to boot {expected} time(s) but `{banner}` appeared {counted}: \
             the reset repeats"
        )),
    }
}

/// Judge a per-boot marker (RFC-0.33-001 D23): it must appear **exactly once per
/// boot**, i.e. `boots` times in the run. Fewer means some boot never reached the
/// line — a second boot that printed the banner and then died; more means it
/// printed twice in one boot.
pub fn judge_per_boot(boots: u32, counted: usize, marker: &str) -> Result<(), String> {
    let boots = boots as usize;
    match counted.cmp(&boots) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Less => Err(format!(
            "`{marker}` must appear once per boot ({boots} boots) but appeared {counted}x: a boot \
             did not reach it"
        )),
        std::cmp::Ordering::Greater => Err(format!(
            "`{marker}` must appear once per boot ({boots} boots) but appeared {counted}x: it \
             printed more than once in a boot"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The three lines below are QEMU 11.1.1's own, captured from the probe.
    const RESET: &str = r#"{"timestamp": {"seconds": 1, "microseconds": 2}, "event": "SHUTDOWN", "data": {"guest": true, "reason": "guest-reset"}}"#;
    const POWEROFF: &str = r#"{"timestamp": {"seconds": 1, "microseconds": 2}, "event": "SHUTDOWN", "data": {"guest": true, "reason": "guest-shutdown"}}"#;
    const KILLED: &str = r#"{"timestamp": {"seconds": 1, "microseconds": 2}, "event": "SHUTDOWN", "data": {"guest": false, "reason": "host-signal"}}"#;

    #[test]
    fn parses_the_three_real_shutdown_events() {
        assert_eq!(
            parse_shutdown_event(RESET),
            Some(Shutdown {
                guest: true,
                reason: "guest-reset".into()
            })
        );
        assert_eq!(
            parse_shutdown_event(POWEROFF),
            Some(Shutdown {
                guest: true,
                reason: "guest-shutdown".into()
            })
        );
        assert_eq!(
            parse_shutdown_event(KILLED),
            Some(Shutdown {
                guest: false,
                reason: "host-signal".into()
            })
        );
    }

    #[test]
    fn other_lines_are_not_shutdown_events() {
        assert_eq!(parse_shutdown_event(r#"{"event": "STOP"}"#), None);
        assert_eq!(parse_shutdown_event(r#"{"return": {}}"#), None);
        assert_eq!(parse_shutdown_event(""), None);
        // A SHUTDOWN whose payload cannot be read is no event, not a guess.
        assert_eq!(
            parse_shutdown_event(r#"{"event": "SHUTDOWN", "data": {}}"#),
            None
        );
    }

    #[test]
    fn a_guest_reset_passes() {
        let s = parse_shutdown_event(RESET).unwrap();
        assert_eq!(judge(GUEST_RESET, Some(&s), Some(0), None), Ok(()));
    }

    /// The case the probe showed: a power-off exits 0 exactly as a reset does.
    #[test]
    fn a_poweroff_is_not_a_reset() {
        let s = parse_shutdown_event(POWEROFF).unwrap();
        let e = judge(GUEST_RESET, Some(&s), Some(0), None).unwrap_err();
        assert!(
            e.contains("guest-shutdown") && e.contains("guest-reset"),
            "{e}"
        );
    }

    /// A hang: QEMU is killed by the wrapper. The `host-signal` event is
    /// present, and must not rescue it.
    #[test]
    fn a_hang_is_refused_even_when_an_event_is_present() {
        let s = parse_shutdown_event(KILLED).unwrap();
        let e = judge(GUEST_RESET, Some(&s), Some(124), None).unwrap_err();
        assert!(e.contains("killed by the wrapper's timeout"), "{e}");
    }

    #[test]
    fn exit_124_beats_a_guest_reset_event() {
        // Defensive: if both were ever reported, the kill still wins — the
        // run did not end by itself.
        let s = parse_shutdown_event(RESET).unwrap();
        assert!(judge(GUEST_RESET, Some(&s), Some(124), None).is_err());
    }

    #[test]
    fn a_host_stop_is_refused_by_reason() {
        let s = parse_shutdown_event(KILLED).unwrap();
        let e = judge(GUEST_RESET, Some(&s), Some(0), None).unwrap_err();
        assert!(
            e.contains("host reason") && e.contains("host-signal"),
            "{e}"
        );
    }

    #[test]
    fn no_event_is_refused_and_says_whether_qmp_failed() {
        let e = judge(GUEST_RESET, None, Some(0), None).unwrap_err();
        assert!(e.contains("without reporting a SHUTDOWN"), "{e}");
        let e = judge(GUEST_RESET, None, Some(0), Some("cannot connect")).unwrap_err();
        assert!(e.contains("QMP handshake failed"), "{e}");
    }

    #[test]
    fn host_signal_can_never_be_an_expected_reason() {
        assert!(!EXPECTABLE.contains(&"host-signal"));
        assert!(EXPECTABLE.contains(&GUEST_RESET) && EXPECTABLE.contains(&GUEST_SHUTDOWN));
    }
}

#[cfg(test)]
mod boot_count_tests {
    use super::*;

    const B: &[u8] = b"sched: started";

    #[test]
    fn counts_every_banner_and_nothing_else() {
        assert_eq!(count_boots(b"", B), 0);
        assert_eq!(count_boots(b"boot\nsched: started\nrun\n", B), 1);
        assert_eq!(count_boots(b"sched: started\n...\nsched: started\n", B), 2);
        assert_eq!(
            count_boots(b"sched: start", B),
            0,
            "a partial banner is not a boot"
        );
        assert_eq!(
            count_boots(b"anything", b""),
            0,
            "an empty banner counts nothing"
        );
    }

    #[test]
    fn a_boot_that_dies_early_is_not_counted() {
        // The shape of the satp failure: the second boot printed its first six
        // lines and never reached the banner. Counting the FIRST line would have
        // called this two boots.
        let log = b"Fjell OS kernel started.\n...\nsched: started\n[reset]\n\
                    Fjell OS kernel started.\nmode: S\nmm: frame allocator ready\n";
        assert_eq!(count_boots(log, b"Fjell OS kernel started."), 2);
        assert_eq!(count_boots(log, B), 1);
    }

    #[test]
    fn the_count_is_judged_exactly() {
        assert!(judge_boots(2, 2, "b").is_ok());
        let fewer = judge_boots(2, 1, "b").unwrap_err();
        assert!(fewer.contains("did not come back"), "{fewer}");
        let more = judge_boots(2, 3, "b").unwrap_err();
        assert!(more.contains("repeats"), "{more}");
        assert!(judge_boots(1, 1, "b").is_ok());
        assert!(judge_boots(1, 0, "b").is_err());
    }

    #[test]
    fn a_per_boot_marker_must_appear_exactly_once_per_boot() {
        assert!(judge_per_boot(2, 2, "m").is_ok());
        let fewer = judge_per_boot(2, 1, "m").unwrap_err();
        assert!(fewer.contains("did not reach"), "{fewer}");
        let more = judge_per_boot(2, 3, "m").unwrap_err();
        assert!(more.contains("more than once"), "{more}");
    }
}
