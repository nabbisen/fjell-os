//! Profile-driven QEMU runner.
//!
//! A *profile* is a small declarative description of one QEMU run:
//! kernel, disk image, timeout, expected markers, run id.  Both the
//! smoke runner (`qemu-test`) and the negative runner (`qemu-negative`)
//! are thin wrappers around `run_profile`.
//!
//! Profiles live under `tests/qemu/profiles/<name>.toml`.  v0.1.1 ships
//! a minimal hand-parsed TOML reader to avoid pulling a heavy dep into
//! the xtask crate.  The supported subset is enough for the v0.1.x
//! profiles; v0.2 may switch to `toml` if profiles grow.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::qemu::{KERNEL_ELF, build_all};
use crate::qemu_shutdown;

/// One QEMU run.  Loaded from a profile file or built inline by the
/// smoke runner.
#[derive(Debug)]
pub struct Profile {
    pub name: String,
    /// Path to the kernel ELF, relative to the workspace root.
    pub kernel: PathBuf,
    /// Path to the disk image to attach (created if missing).
    pub disk: PathBuf,
    /// Hard timeout in seconds for the `timeout(1)` wrapper.
    pub timeout_secs: u32,
    /// Markers that must appear in the captured serial log for the run
    /// to count as a pass.  An empty list is allowed for placeholder
    /// profiles per RFC 025 §"chicken-and-egg" exemption.
    pub expected_markers: Vec<String>,
    /// Optional extra QEMU args beyond the defaults.
    pub extra_args: Vec<String>,
    /// QEMU args placed **before** the blk device (`-drive` / `-device virtio-blk-device`), where `extra_args` cannot reach
    /// (RFC-0.50-003 D5, the architect's ruling on the stage-1 review). QEMU `virt` assigns virtio-mmio transports in reverse
    /// creation order, so **a device created before blk takes a higher slot and blk lands lower**; `extra_args` is appended after
    /// blk, so no profile could ever put a device first. Empty for every profile that does not set it -- an additive field, so
    /// every existing command line is byte-identical (a test checks that against the committed `qemu-command.txt` of each one).
    pub pre_blk_args: Vec<String>,
    /// RFC-0.25-001 (Demonstration 6): once this marker appears in the
    /// captured output, write `.1`'s bytes to QEMU's stdin — `-nographic`
    /// wires the guest's UART0 RX to the host process's stdin by default,
    /// so this simulates a character typed at the console. `None` for every
    /// other profile: they keep the plain `Command::output()` path
    /// unchanged (no piped stdin, no reader threads).
    pub inject_after_marker: Option<(String, Vec<u8>)>,
    /// RFC-0.29-001 D1: the structural signal that separates a negative
    /// (or smoke) profile, runnable by this module's single-QEMU
    /// `run_profile`, from a profile like `fleet-demo` that requires
    /// several simultaneous QEMU instances and a dedicated runner
    /// (`cargo xtask fleet-demo`). Anything deriving "the negative-test
    /// categories" from `tests/qemu/profiles/*.toml` must exclude these —
    /// see `discover_negative_categories`.
    pub multi_node: bool,
    /// RFC-0.29-001 D5: `true` unless the profile's own `.toml` says
    /// otherwise. A category can be swept into derived scope (D1) while
    /// still being explicitly excluded from what a release blocks on —
    /// `store` has marker specifications but no emitting scenario yet
    /// (`v1-limitations.md`), and is expected to run and fail honestly
    /// rather than being silently skipped. (`upgrade` was the other half of
    /// this comment until RFC-0.35-001 gave it a real emitting scenario and
    /// set this `true`.) The loader requires `not_gated_reason` whenever
    /// this is `false` — there is no way to opt out without also saying
    /// why, in the one file a reader checking this category will already
    /// have open.
    pub release_gated: bool,
    /// Required, non-empty, whenever `release_gated` is `false`.
    pub not_gated_reason: Option<String>,
    /// RFC-0.33-001 D5/R6: the way this run must *end*, as QEMU reports it —
    /// `"guest-reset"` for a machine that reset itself, `"guest-shutdown"` for
    /// one that powered off. `None` for every other profile, which keep the
    /// runner's existing behaviour exactly (no QMP socket, no `-no-reboot`).
    ///
    /// When set, the run adds `-no-reboot` and a QMP socket and **fails
    /// unless QEMU reports `SHUTDOWN{guest:true, reason:<this>}` and exited by
    /// itself** rather than being killed by the timeout wrapper. Without this
    /// a reset is a boot loop that ends as exit 124 — identical to a hang —
    /// and even with `-no-reboot` a reset and a power-off both exit 0. The
    /// machine's own account is the evidence, not a marker (see
    /// `qemu_shutdown`'s module doc for the measurements).
    pub expect_shutdown: Option<String>,
    /// RFC-0.33-001 D21: how many times the machine must **boot to
    /// `boot_banner`** in one run — `2` for a machine that resets itself once
    /// and must come back. The run is made **without `-no-reboot`** (which
    /// `expect_shutdown` adds, and which is why no tier ever booted a second
    /// time: QEMU exits at the reset, so the boot after it is never observed),
    /// the trigger is injected **once**, and the count of banners in the serial
    /// log is judged **exactly**: fewer is a machine that did not come back,
    /// more is a reset that repeats. Requires `boot_banner`; excludes
    /// `expect_shutdown`.
    pub expect_boots: Option<u32>,
    /// A line printed **late in every boot** — late enough that a boot which
    /// hangs early (the first `satp` failure died six lines in) never prints it.
    pub boot_banner: Option<String>,
    /// RFC-0.33-001 D23: lines that must each appear **exactly once per boot**,
    /// i.e. exactly `expect_boots` times in the run. `expected_markers` match
    /// anywhere in the log, so a claim like "the reset boot reaches readiness
    /// again" is satisfied by the first boot alone; a marker here is counted, so
    /// a second boot that prints the banner and then dies fails. **At least one
    /// should be a service-level line** (e.g. `driver-uart: ready`), not only the
    /// kernel's own: that a service came back means the machine reached the
    /// readiness the first boot did. Requires `expect_boots`.
    pub per_boot_markers: Vec<String>,
    /// RFC-0.34-002 D5 / E-063: lines that must appear **exactly once** in the run.
    /// `expected_markers` match anywhere, so a marker printed by two writers
    /// (`M6: storaged ready` was, by `storaged` and by `init`) passes as happily as
    /// one printed once — the ambiguity is invisible until someone asserts the line
    /// and trips on it. A marker here is counted, so a second writer is a failure.
    pub once_markers: Vec<String>,
    /// RFC-0.34-002 D9: the run's whole serial log must contain no control byte other
    /// than `\n`, `\r`, `\t` — the check R7 asks for, now that E-062's actual source
    /// (three debug probe writes) is gone. Not the default: most profiles' QEMU
    /// banners and the harness's own timeout/shutdown text are not being asserted
    /// here, only opted-in tiers that want the console-cleanliness claim checked.
    pub clean_console: bool,
    /// RFC-0.38-001 R6: line prefixes. For **each**, the run must print that
    /// prefix followed by the 64 hex digits of the service-image set digest **as
    /// the host recomputes it** from `crates/fjell-kernel/prebuilt/*.bin` when
    /// the run is judged (`image_set::image_set_digest`: external `sha2`, the
    /// documented layout assembled by hand, file names mapped to `ImageId`s
    /// without the kernel's own table). The value is *derived at judging time*,
    /// never written in the profile, so it cannot be a stale literal; and it is
    /// compared for **equality** — a digest that is merely present, non-zero or
    /// well-formed passes nothing. Several prefixes make the agreement
    /// three-way (the kernel's line, `measuredd`'s recorded line, and the host),
    /// which also says *where* a disagreement is. Empty for every other profile.
    pub expect_image_set_digest: Vec<String>,
    /// RFC-0.39-002 R5: the same discipline for the **measurement chain head**.
    /// For each prefix the run must print that prefix followed by the 64 hex
    /// digits of the chain head after the four events a boot appends, as the
    /// host derives it (`chain_head::boot_chain_head`: external `sha2`, the
    /// preimage assembled by hand, the events written out in the module, the
    /// image-set digest recomputed from the prebuilt images). Equality, not
    /// "non-zero": a tier asserting only that the flag is `true` and the head
    /// is non-zero would pass against a placeholder chain reported as a
    /// measured one. Empty for every other profile.
    pub expect_chain_head: Vec<String>,
    /// RFC-0.41-001: `"<source prefix> => <template with {}> [=> <bound prefix>]"`.
    /// For **every** line carrying the source prefix, the number after it must be
    /// echoed by a line containing the template with `{}` replaced -- and, with a
    /// bound, **before** the first bound line that follows. This is how a tier asserts
    /// that something a test *did* reached the audit trail, and reached it *in time*
    /// (a record `auditd` slept through is late, not absent). See
    /// `audit_trail::judge_echoes`. Empty for every other profile.
    pub expect_echoed: Vec<String>,
    /// RFC-0.41-001: names the line a test prints after issuing a burst of probe
    /// records (`count=<B> base=<N>`). The trail must **account** for the burst:
    /// records printed plus the records the gap notices declare lost equals `B`, and
    /// at least one was lost. See `audit_trail::judge_accounted`. Empty elsewhere.
    pub expect_accounted: Vec<String>,
    /// RFC-0.41-001 R11: `"<begin prefix> => <end prefix> => <required text>"`. At least
    /// one line containing the required text must fall between the first begin line and
    /// the first end line after it. Used to require that a sweep reached the path it
    /// exists to test -- it asserts that the test still tests, not a kernel property.
    /// See `audit_trail::judge_observed`. Empty elsewhere.
    pub expect_observed: Vec<String>,
    /// RFC-0.41-006 R1: `"<line substring> => <number|null>"`. Every record line
    /// containing the substring must carry a `"task"` that is a number (or `null`) and
    /// a `"tick"`. See `audit_trail::judge_scoped`. Empty elsewhere.
    pub expect_scoped: Vec<String>,
    /// 0.44 small item E: `"<prefix A> => <prefix B>"`. Two services print the same 32-byte
    /// digest, one as the producer and one as the consumer that fetched it (`attestd`'s
    /// `record digest` and `diagnosticsd`'s `last_attestation`). Each prefix is followed by 64
    /// hex digits; **every B line must equal the most recent A line before it**. It fails when
    /// the values differ, when either line is **absent** (a comparison that passes on a
    /// missing line is worse than none), when a line is malformed, when B precedes any A, and
    /// when the value is all zeros (two zeros "agree", and a zero is what a failed query used
    /// to look like -- E-076). Console lines only: nothing is recomputed on the host. See
    /// [`judge_equal_digests`]. Empty for every other profile.
    pub expect_equal: Vec<String>,
    /// RFC-0.45-001 D3: `"<text>"`. **No console line may contain `<text>`.** A scenario-specific
    /// negation: the `net-absent` profile attaches no NIC, and the driver must then print no
    /// `driver-virtio-net: found` line -- a gate that passes only when a device *is* present would
    /// leave E-118's hole where it was. **Not** a profile-level `forbidden_markers`: E-111 refused
    /// that because it would duplicate the global hard-coded list (`FORBIDDEN_MARKERS`) with prose
    /// free to drift from it. This has no hard-coded twin. The loader rejects a profile whose
    /// `expect_absent` text is contained in one of its own required markers (it could never pass)
    /// and a profile that has `expect_absent` but no required marker at all (a run that never
    /// booted would pass it). See [`judge_absent`]. Empty for every other profile.
    pub expect_absent: Vec<String>,
}

impl Profile {
    /// Build the default smoke profile for one milestone (`m1`..`m8`).
    pub fn smoke(milestone: &str, marker: &str) -> Self {
        // Whether a smoke milestone is release-gated is a fact about `test-all`, not a default: `SMOKE_PROFILES` is the list it
        // runs, and `qemu-test` accepts more names than that (`m7`). A synthesised profile that said `true` for every name would
        // make `ArtifactDir` (which now keys its writes on this field, E-135) leave three untracked files behind for every
        // ungated milestone -- E-040 again, one level up.
        let gated = crate::test_all::SMOKE_PROFILES.contains(&milestone);
        Self {
            name: format!("smoke-{milestone}"),
            kernel: PathBuf::from(KERNEL_ELF),
            disk: PathBuf::from("fjell-disk.img"),
            timeout_secs: 60,
            expected_markers: vec![marker.to_string()],
            extra_args: vec![],
            pre_blk_args: vec![],
            inject_after_marker: None,
            multi_node: false,
            release_gated: gated,
            not_gated_reason: if gated {
                None
            } else {
                Some(
                    "not run by `test-all` (`test_all::SMOKE_PROFILES`): an ad-hoc `qemu-test` run, which records nothing"
                        .to_string(),
                )
            },
            expect_shutdown: None,
            expect_boots: None,
            boot_banner: None,
            per_boot_markers: vec![],
            once_markers: vec![],
            clean_console: false,
            expect_image_set_digest: vec![],
            expect_chain_head: vec![],
            expect_echoed: vec![],
            expect_accounted: vec![],
            expect_observed: vec![],
            expect_scoped: vec![],
            expect_equal: vec![],
            expect_absent: vec![],
        }
    }
}

/// For each `prefix`, the run output must contain a line with `prefix` followed
/// by exactly the 64 hex digits `want` (equal, and not merely a prefix of a
/// longer run of hex). Prints a `✓` or a MISMATCH naming what was seen, appends
/// the evidence to `record`, and returns whether **every** prefix matched.
/// Shared by the image-set digest (RFC-0.38-001 R6) and the chain head
/// (RFC-0.39-002 R5), so the two cannot drift into judging differently.
fn judge_hex_prefixes(
    text: &str,
    prefixes: &[String],
    want: &str,
    label: &str,
    over: &str,
    detail: &str,
    record: &mut String,
) -> bool {
    let mut all = true;
    for prefix in prefixes {
        let wanted_line = format!("{prefix}{want}");
        let seen: Vec<&str> = text
            .lines()
            .filter(|l| l.contains(prefix.as_str()))
            .collect();
        let found = seen.iter().any(|l| {
            l.find(&wanted_line).is_some_and(|at| {
                !l[at + wanted_line.len()..].starts_with(|c: char| c.is_ascii_hexdigit())
            })
        });
        record.push_str(&format!(
            "prefix = {prefix}\nlines_with_prefix = {}\nequal = {found}\n",
            seen.join(" | ")
        ));
        if found {
            println!("[xtask] `{prefix}…` equals the host recomputation ({detail}) ✓");
        } else {
            eprintln!(
                "[xtask] {label} MISMATCH at `{prefix}…` — the host recomputes sha256:{want} \
                 {over}; the run printed: {}",
                if seen.is_empty() {
                    format!("no line containing `{prefix}`")
                } else {
                    seen.join(" | ")
                }
            );
            all = false;
        }
    }
    all
}

/// RFC-0.45-001 D3: the `expect_absent` judge. **No line of `text` may contain `needle`.**
///
/// `Err` names the **first offending line and its number** -- the line is the evidence. An empty
/// needle is refused (it is contained in every line, so it would "fail" for no reason, and a
/// typo that left one blank would pass the loader). The judge says nothing about whether the run
/// *happened*; that is the loader's rule (a profile with `expect_absent` must also require a
/// marker), and the profile's own required markers.
pub fn judge_absent(text: &str, needle: &str) -> Result<String, String> {
    if needle.trim().is_empty() {
        return Err("bad expect_absent: the text must be non-empty".to_string());
    }
    for (i, line) in text.lines().enumerate() {
        if line.contains(needle) {
            return Err(format!(
                "`{needle}` must not occur, but line {} reads `{}`",
                i + 1,
                line.trim()
            ));
        }
    }
    Ok(format!(
        "no line of {} contains `{needle}`",
        text.lines().count()
    ))
}

/// Why a profile's `expect_absent` is unusable, if it is (RFC-0.45-001 D3's loader checks).
fn expect_absent_problem(absent: &[String], required: &[String]) -> Option<String> {
    if absent.is_empty() {
        return None;
    }
    if required.is_empty() {
        return Some(
            "`expect_absent` with no required marker: a run that never booted would satisfy it. \
             Require at least one marker (for an absence scenario, the line that reports the \
             absence)"
                .to_string(),
        );
    }
    for a in absent {
        if a.trim().is_empty() {
            return Some("`expect_absent` holds an empty text".to_string());
        }
        if let Some(m) = required.iter().find(|m| m.contains(a.as_str())) {
            return Some(format!(
                "`expect_absent` text `{a}` is contained in the required marker `{m}`: the \
                 profile contradicts itself and could never pass"
            ));
        }
    }
    None
}

/// 0.44 small item E: compare two digests that two services print on the console.
///
/// `spec` is `"<prefix A> => <prefix B>"`. A *digest line* for a prefix is a line containing
/// the prefix immediately followed by exactly 64 hex digits (and no further hex digit). Lines are
/// read in order, and **every B line must equal the most recent A line before it** -- the
/// consumer reports the latest thing the producer printed. It is `Err` when:
///   * the spec is not `A => B` with both prefixes non-empty and different;
///   * no line contains A, or none contains B -- **absence is a failure, never a pass**;
///   * a line containing a prefix is not followed by 64 hex digits (malformed);
///   * a B line comes before any A line;
///   * a B value differs from the A value in force;
///   * the value is all zeros (two zero digests "agree", and a zero is what a failed query
///     used to look like -- E-076).
pub fn judge_equal_digests(text: &str, spec: &str) -> Result<String, String> {
    let Some((a, b)) = spec.split_once(" => ") else {
        return Err(format!(
            "bad expect_equal `{spec}`: want `<prefix A> => <prefix B>`"
        ));
    };
    let (a, b) = (a.trim_end(), b.trim_start());
    if a.is_empty() || b.is_empty() || a == b {
        return Err(format!(
            "bad expect_equal `{spec}`: the two prefixes must be non-empty and different"
        ));
    }
    // The 64 hex digits after `prefix` on `line`, or why there are not.
    let digest_after = |line: &str, prefix: &str| -> Result<String, String> {
        let at = line
            .find(prefix)
            .expect("caller checked the prefix is present");
        let rest = &line[at + prefix.len()..];
        let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if hex.len() == 64 {
            Ok(hex.to_ascii_lowercase())
        } else {
            Err(format!(
                "the line `{}` carries {} hex digits after `{prefix}`, not 64",
                line.trim(),
                hex.len()
            ))
        }
    };
    // Absence first, and named as absence: "B before any A" would be the symptom of a missing
    // producer line, not its cause.
    for (p, who) in [(a, "producer's"), (b, "consumer's")] {
        if !text.lines().any(|l| l.contains(p)) {
            return Err(format!(
                "no line contains `{p}`: the {who} digest was never printed, so nothing was compared"
            ));
        }
    }
    let mut last_a: Option<String> = None;
    let (mut a_lines, mut b_lines) = (0usize, 0usize);
    for line in text.lines() {
        if line.contains(a) {
            let d = digest_after(line, a)?;
            if d.bytes().all(|c| c == b'0') {
                return Err(format!("`{a}…` is an all-zero digest: {}", line.trim()));
            }
            a_lines += 1;
            last_a = Some(d);
        } else if line.contains(b) {
            let d = digest_after(line, b)?;
            if d.bytes().all(|c| c == b'0') {
                return Err(format!("`{b}…` is an all-zero digest: {}", line.trim()));
            }
            b_lines += 1;
            match &last_a {
                None => {
                    return Err(format!(
                        "`{b}…` is printed before any `{a}…` line, so there is nothing for it to equal"
                    ));
                }
                Some(want) if *want != d => {
                    return Err(format!(
                        "DIFFERENT digests: `{a}…` printed sha256:{want}, but `{b}…` printed sha256:{d}"
                    ));
                }
                Some(_) => {}
            }
        }
    }
    Ok(format!(
        "{b_lines} `{b}…` line(s) equal the `{a}…` digest before them ({a_lines} produced)"
    ))
}

/// The QEMU command line of a run, before the shutdown flags (`-no-reboot -qmp ...`) that `expect_shutdown` appends: the timeout, the
/// machine, the kernel, then **`pre_blk_args`**, then the blk drive and device, then `extra_args`. Pure, so a test can compare it
/// with every committed `qemu-command.txt`.
pub(crate) fn base_argv(p: &Profile, kernel_str: &str, drive_arg: &str) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        format!("{}", p.timeout_secs),
        "qemu-system-riscv64".into(),
        "-machine".into(),
        "virt".into(),
        "-bios".into(),
        "none".into(),
        "-nographic".into(),
        "-kernel".into(),
        kernel_str.to_string(),
    ];
    argv.extend(p.pre_blk_args.iter().cloned());
    argv.extend([
        "-drive".into(),
        drive_arg.to_string(),
        "-device".into(),
        "virtio-blk-device,drive=hd0".into(),
    ]);
    argv.extend(p.extra_args.iter().cloned());
    argv
}

/// Where artefacts are written for one run.
pub struct ArtifactDir {
    pub path: PathBuf,
    /// Whether this run writes the profile's three summary files (`expected-markers.txt`, `qemu-command.txt`,
    /// `result-summary.txt`) -- see [`ArtifactDir::write_summary`] and [`ArtifactDir::for_run`] for the rule.
    pub writes_summaries: bool,
}

impl ArtifactDir {
    /// `tests/qemu/artifacts/<profile>/`.  Created on demand.
    ///
    /// **The rule (E-135): a profile's summary files are written when the profile is release-gated, or when they are already
    /// committed.** `release_gated` is the profile TOML's own field (`Profile::release_gated`, which the loader requires a
    /// reason for when it is `false`) -- the authority for "does `test-all` gate this".
    ///
    /// It used to be *only* "was `expected-markers.txt` already on disk", a deliberate E-040 fix against ungated milestones
    /// leaving three untracked files behind. That condition has a consequence nobody wanted: **a newly gated profile is absent,
    /// therefore not written, therefore never committed, therefore absent.** Keying on being gated lets a gated profile write
    /// its own first copy, which a human then commits -- the files exist because the mechanism wrote them, never because anyone
    /// seeded them. Keying on *already committed* as well keeps the one case the new rule alone would regress: a profile that
    /// is **not** gated but whose files are tracked (`store`) keeps being refreshed in place rather than silently going stale.
    ///
    /// The file-presence sample is taken *before* creating anything, and on `expected-markers.txt` rather than on the
    /// directory: the directory is the wrong test, because this run creates it, so a second run of the same ungated milestone
    /// would find it present and start writing after all -- the leak would take two runs instead of one. (Found exactly that
    /// way, by running an ungated milestone while demonstrating the first version of the E-040 fix.)
    /// `expected-markers.txt` is stable under repetition: committed for every recorded profile, so present in any checkout; and
    /// never written for an ungated run, so absent however often it runs.
    pub fn for_run(name: &str, release_gated: bool) -> Self {
        let dir = PathBuf::from("tests/qemu/artifacts").join(name);
        let already_committed = dir.join("expected-markers.txt").is_file();
        let _ = fs::create_dir_all(&dir);
        ArtifactDir {
            path: dir,
            writes_summaries: release_gated || already_committed,
        }
    }
    pub fn join(&self, p: &str) -> PathBuf {
        self.path.join(p)
    }

    /// Write one of the three committed per-profile summary files
    /// (`expected-markers.txt`, `qemu-command.txt`, `result-summary.txt`)
    /// -- but **only for a profile that is release-gated or already has them committed**
    /// (the rule is [`ArtifactDir::for_run`]'s).
    ///
    /// Those three files are tracked for every profile `test-all` gates,
    /// and `.gitignore` says in its own words that they "must stay
    /// tracked": a re-run rewrites them in place, so `git status` stays
    /// clean. The first design silently assumed every runnable profile's
    /// directory is already tracked -- true for the gated ones, false for
    /// the ungated smoke milestones (E-040) and for any ad-hoc name.
    /// Running one of those left three untracked files behind **forever**,
    /// and the only remedy was for a human to notice and delete them.
    ///
    /// An ungated run therefore leaves `serial.log` (ignored by `*.log`)
    /// and `runs/<run-id>/` (ignored) and nothing else, so it is clean by
    /// construction rather than by anyone remembering -- and stays clean
    /// however many times that milestone is run. The run's own output is
    /// not lost: `run_dir()` holds a complete per-invocation copy.
    ///
    /// The skip is announced, never silent (see `run_profile`).
    pub fn write_summary(&self, file: &str, bytes: &[u8]) {
        if self.writes_summaries {
            let _ = fs::write(self.join(file), bytes);
        }
    }
    /// `tests/qemu/artifacts/<profile>/runs/<run-id>/` — RFC-0.27-004 R3.
    /// Unlike the flat `serial.log` above (still written, still overwritten
    /// by the next run of this profile — that stays true, unchanged), this
    /// path is unique per invocation, so a promotable copy survives the
    /// next tier running the same profile.
    pub fn run_dir(&self, run_id: &str) -> PathBuf {
        let dir = self.path.join("runs").join(run_id);
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

/// `YYYYMMDD-HHMMSS`, same construction as `test_all::timestamp_str` —
/// duplicated rather than shared (small, self-contained, and the two
/// modules stay independent by this project's own convention; see
/// `standards_mapping::normalise`'s doc-comment for the same call made
/// elsewhere in this codebase).
fn run_id_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::ZERO)
        .as_secs();
    let s = secs % 86400;
    let d = secs / 86400;
    let hh = s / 3600;
    let mm = (s % 3600) / 60;
    let ss = s % 60;
    let days = d + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year, month, day, hh, mm, ss
    )
}

/// The commit `HEAD` resolved to at run time, full 40-hex sha — a short sha
/// would be ambiguous for the ancestry check `evidence promote` and the
/// `evidence` subcheck both perform on it later. `"unknown"` if `git` is
/// unavailable or the tree is not a git checkout; provenance carries this
/// literally rather than silently omitting the field.
fn git_head_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Whether any tracked file differs from `HEAD` at run time — the
/// mechanical proxy for "was this build instrumented" (D3). Not proof by
/// itself (a dirty tree can be unrelated to the binary that produced this
/// log, and a clean tree does not rule out instrumentation from an already
/// -committed-then-amended state), so `evidence promote` still requires a
/// human `--instrumented` answer rather than trusting this alone — but a
/// mismatch between the two is worth a promoter's attention, which is why
/// this is recorded at all. Fails safe: if `git status` cannot be run,
/// reports dirty rather than silently claiming a clean tree.
/// The emulator that produced this run's boot evidence, as it reports itself.
///
/// RFC-0.42-002's milestone, `v1-limitations.md` row 20: 27 of the 33 tiers boot QEMU and
/// their results are the main evidence a release offers about the running system, yet nothing
/// in the tree recorded *which* emulator produced them. The build side is captured to the
/// LLVM version; this was captured nowhere, and at the 0.41.0 cut `qemu-system-riscv64` was
/// absent from the machine entirely and installed mid-cut — so that release's evidence came
/// from a different install than the sessions before it.
///
/// Queried by **bare name**, exactly as `run_profile` invokes it (`qemu-system-riscv64` at
/// the argv construction below), so this is the version of the binary the run actually used
/// and not of some other copy on disk. First line only — that is where the version sits.
/// `unknown` rather than a guess if the query fails, and a failure never fails the run:
/// recording provenance must not be able to turn a passing tier red.
fn emulator_version() -> String {
    Command::new("qemu-system-riscv64")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .map(|l| l.trim().to_string())
        })
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

fn git_tree_dirty() -> bool {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true)
}

/// Entry point: `cargo xtask qemu-run --profile <name>`.
pub fn cmd_qemu_run(profile_name: Option<&str>) -> ExitCode {
    let name = match profile_name {
        Some(n) => n,
        None => {
            eprintln!("Usage: cargo xtask qemu-run --profile <name>");
            return ExitCode::FAILURE;
        }
    };
    let profile = match load_profile(Path::new("."), name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[xtask] qemu-run: {e}");
            return ExitCode::FAILURE;
        }
    };
    run_profile(&profile)
}

/// The core run loop, shared by smoke / negative / explicit run.
///
/// 1. Build the kernel if missing.
/// 2. (Re)create the disk image.
/// 3. Run QEMU with `timeout(N) qemu-system-riscv64 ...`.
/// 4. Capture combined stdout + stderr to `serial.log`.
/// 5. Write `qemu-command.txt` and `expected-markers.txt`.
/// 6. Assert every expected marker via `qemu-log-check::log_check`.
/// 7. Write `result-summary.txt` and return the verdict.
pub fn run_profile(p: &Profile) -> ExitCode {
    let art = ArtifactDir::for_run(&p.name, p.release_gated);
    println!(
        "[xtask] running profile `{}` (timeout {}s)",
        p.name, p.timeout_secs
    );
    if !art.writes_summaries {
        // Announced, not silent -- see `ArtifactDir::write_summary`.
        println!(
            "[xtask] profile `{}` is not release-gated and tests/qemu/artifacts/{}/expected-markers.txt is not \
             committed, so this profile's summary files are not written and this run leaves nothing untracked \
             behind. Full output is under runs/<run-id>/. A profile `test-all` gates writes them on its own; \
             to gate `{}`, set `release_gated = true` in its profile.",
            p.name, p.name, p.name
        );
    }

    // Build kernel if needed (smoke profiles always build_all; the
    // arg `--profile` path assumes the kernel is already built).
    if !Path::new(&p.kernel).exists() {
        eprintln!("[xtask] kernel ELF missing — running build_all()");
        let _ = build_all();
    }

    // (Re)create disk image — required by virtio-blk smoke path.
    // Pure-Rust fallback: a raw QEMU disk image is just a zero-filled
    // sparse file. This avoids requiring qemu-img (a separate package on
    // Arch Linux: `qemu-img`; on Debian/Ubuntu: `qemu-utils`).
    if p.disk.exists() {
        let _ = fs::remove_file(&p.disk);
    }
    if let Err(e) = fs::File::create(&p.disk).and_then(|f| f.set_len(16 * 1024 * 1024)) {
        eprintln!(
            "[xtask] WARNING: could not create disk image {}: {e}",
            p.disk.display()
        );
    }

    let kernel_str = p.kernel.to_string_lossy().to_string();
    let disk_str = p.disk.to_string_lossy().to_string();
    let drive_arg = format!("file={disk_str},format=raw,if=none,id=hd0");

    // Build the command vector once so we can both run it and persist
    // it to qemu-command.txt.
    let mut argv = base_argv(p, &kernel_str, &drive_arg);

    // RFC-0.33-001: a profile that expects the machine to stop is run with
    // `-no-reboot` (so a reset ends the run instead of looping) and a QMP
    // socket (so QEMU can say *why* it stopped). Both are part of the
    // command written to `qemu-command.txt`, so the artefact shows what ran.
    //
    // The socket is a *relative* path: AF_UNIX paths are limited to ~107
    // bytes, and an absolute path under a deep checkout blows that limit
    // (found the hard way, measuring this).
    let qmp_sock: Option<PathBuf> = p
        .expect_shutdown
        .as_ref()
        .map(|_| art.path.join("qmp.sock"));
    if let Some(sock) = &qmp_sock {
        let s = sock.to_string_lossy().to_string();
        if s.len() > 100 {
            eprintln!(
                "[xtask] QMP socket path `{s}` is {} bytes; AF_UNIX allows ~107. Use a shorter \
                 profile name.",
                s.len()
            );
            return ExitCode::FAILURE;
        }
        let _ = fs::remove_file(sock);
        argv.push("-no-reboot".into());
        argv.push("-qmp".into());
        argv.push(format!("unix:{s},server=on,wait=on"));
    }

    art.write_summary("qemu-command.txt", argv.join(" ").as_bytes());
    let mut expected_text = p.expected_markers.join("\n");
    for m in &p.once_markers {
        expected_text.push_str(&format!("\n{m}    [exactly 1x]"));
    }
    for m in &p.per_boot_markers {
        // The count is part of the assertion, so it is part of the record.
        expected_text.push_str(&format!(
            "\n{m}    [exactly {}x: once per boot]",
            p.expect_boots.unwrap_or(0)
        ));
    }
    for prefix in &p.expect_image_set_digest {
        // The value is derived when the run is judged, so the committed record
        // states the rule, not a digest that would go stale with the next build.
        expected_text.push_str(&format!(
            "\n{prefix}<64 hex: the service-image set digest, recomputed on the host from \
             crates/fjell-kernel/prebuilt/*.bin>    [equality]"
        ));
    }
    for prefix in &p.expect_chain_head {
        expected_text.push_str(&format!(
            "\n{prefix}<64 hex: the measurement chain head after the four events a boot \
             appends, recomputed on the host (chain_head.rs)>    [equality]"
        ));
    }
    for spec in &p.expect_echoed {
        expected_text.push_str(&format!(
            "\n{spec}    [echoed: every such line must be echoed by the trail, in time]"
        ));
    }
    for prefix in &p.expect_accounted {
        expected_text.push_str(&format!(
            "\n{prefix}count=<B> base=<N>    [accounted: printed + lost after the begin line == B (+ a stated allowance for unrelated records), lost > 0]"
        ));
    }
    for spec in &p.expect_observed {
        expected_text.push_str(&format!(
            "\n{spec}    [observed: the required line occurs between the begin and end lines]"
        ));
    }
    for spec in &p.expect_equal {
        expected_text.push_str(&format!(
            "\n{spec}    [equal: every B digest equals the most recent A digest before it; both present, 64 hex, non-zero]"
        ));
    }
    for text in &p.expect_absent {
        expected_text.push_str(&format!(
            "\n{text}    [absent: no console line may contain this text]"
        ));
    }
    for spec in &p.expect_scoped {
        expected_text.push_str(&format!(
            "\n{spec}    [scoped: every such record names its task (or null) and carries a tick]"
        ));
    }
    art.write_summary("expected-markers.txt", expected_text.as_bytes());

    // A run that expects a particular ending needs the exit status and QMP's
    // account, which the two older paths discard. Only those profiles take
    // this path; every other profile keeps the behaviour it had.
    let mut outcome: Option<RunOutcome> = None;
    let combined = if qmp_sock.is_some() {
        let o = run_qemu(
            &argv,
            p.inject_after_marker
                .as_ref()
                .map(|(m, b)| (m.as_bytes(), b.as_slice())),
            qmp_sock.as_deref(),
            std::time::Duration::from_secs(p.timeout_secs as u64),
        );
        let out = o.output.clone();
        outcome = Some(o);
        out
    } else {
        match &p.inject_after_marker {
            None => {
                let output = Command::new("timeout")
                    .args(&argv[..])
                    .output()
                    .expect("failed to run qemu-system-riscv64");
                let mut combined = output.stdout.clone();
                combined.extend_from_slice(&output.stderr);
                combined
            }
            Some((marker, inject_bytes)) => {
                run_with_stdin_injection(&argv, marker.as_bytes(), inject_bytes)
            }
        }
    };

    let log_path = art.join("serial.log");
    let _ = fs::write(&log_path, &combined);

    // RFC-0.27-004 R3: also retain this run under a run-id-keyed directory,
    // so the flat path above being overwritten by the *next* run of this
    // profile no longer destroys the only copy — `evidence promote` reads
    // from here, not from the flat path. Provenance captured now, at run
    // time, because the commit sha and dirty-tree state are true facts only
    // at this moment; asking for them later at promotion time would be
    // asking the wrong point in history (D2/D3).
    let run_id = run_id_now();
    let run_dir = art.run_dir(&run_id);
    let _ = fs::write(run_dir.join("serial.log"), &combined);
    let _ = fs::write(run_dir.join("qemu-command.txt"), argv.join(" ").as_bytes());
    let run_info = format!(
        "run_id = {run_id}\nprofile = {}\ncommit_sha = {}\ntree_dirty_at_run_time = {}\nemulator = {}\ncommand = {}\n",
        p.name,
        git_head_sha(),
        git_tree_dirty(),
        emulator_version(),
        argv.join(" "),
    );
    let _ = fs::write(run_dir.join("run-info.txt"), run_info.as_bytes());

    // Empty marker list = placeholder profile (no cases registered) — unless
    // it expects a particular ending or a number of boots, which is itself a case.
    if p.expected_markers.is_empty()
        && p.once_markers.is_empty()
        && p.expect_shutdown.is_none()
        && p.expect_boots.is_none()
        && p.expect_image_set_digest.is_empty()
        && p.expect_chain_head.is_empty()
        && p.expect_echoed.is_empty()
        && p.expect_accounted.is_empty()
        && p.expect_observed.is_empty()
        && p.expect_scoped.is_empty()
        && p.expect_equal.is_empty()
        && p.expect_absent.is_empty()
    {
        art.write_summary(
            "result-summary.txt",
            b"PASS (placeholder; no expected markers)\n",
        );
        println!(
            "[xtask] profile `{}` is a placeholder — no markers \
                  to check (RFC 025 §chicken-and-egg). PASS.",
            p.name
        );
        return ExitCode::SUCCESS;
    }

    // Check every expected marker.
    let mut all_ok = true;

    // RFC-0.33-001 D21: did the machine come back from its own reset, and did
    // it do so exactly as often as expected? Counted from the serial log, which
    // is what a reader can open; recorded per run before it is judged.
    if let (Some(expected), Some(banner)) = (p.expect_boots, &p.boot_banner) {
        let counted = qemu_shutdown::count_boots(&combined, banner.as_bytes());
        let _ = fs::write(
            run_dir.join("boots.txt"),
            format!("expected = {expected}\nobserved = {counted}\nboot_banner = {banner}\n"),
        );
        match qemu_shutdown::judge_boots(expected, counted, banner) {
            Ok(()) => println!(
                "[xtask] the machine booted {counted} time(s), as expected (`{banner}` seen \
                 {counted}x, no -no-reboot) ✓"
            ),
            Err(why) => {
                eprintln!("[xtask] boot count check FAILED — {why}");
                all_ok = false;
            }
        }
        // D23: each per-boot marker, once per boot. Recorded beside the boot
        // count before it is judged, so a failure can be read without re-running.
        let mut record = String::new();
        for m in &p.per_boot_markers {
            let n = qemu_shutdown::count_boots(&combined, m.as_bytes());
            record.push_str(&format!("per_boot_marker = {m}\nobserved = {n}\n"));
            match qemu_shutdown::judge_per_boot(expected, n, m) {
                Ok(()) => println!("[xtask] `{m}` seen {n}x — once per boot ✓"),
                Err(why) => {
                    eprintln!("[xtask] per-boot marker check FAILED — {why}");
                    all_ok = false;
                }
            }
        }
        if !record.is_empty() {
            use std::io::Write;
            if let Ok(mut f) = fs::OpenOptions::new()
                .append(true)
                .open(run_dir.join("boots.txt"))
            {
                let _ = f.write_all(record.as_bytes());
            }
        }
    }

    // RFC-0.33-001 D5: how the run ended, by QEMU's own account. Recorded
    // before it is judged, so a failure can be read without re-running.
    if let (Some(expected), Some(o)) = (&p.expect_shutdown, &outcome) {
        let shutdown = o.qmp.as_ref().and_then(|q| q.shutdown.as_ref());
        let qmp_error = o.qmp.as_ref().and_then(|q| q.error.as_deref());
        let record = format!(
            "expected = {expected}\nobserved = {}\nguest_initiated = {}\nqemu_exit_code = {}\n\
             qmp_events = {}\nqmp_error = {}\n",
            shutdown.map_or("(none)".to_string(), |s| s.reason.clone()),
            shutdown.map_or("(none)".to_string(), |s| s.guest.to_string()),
            o.exit_code.map_or("(none)".to_string(), |c| c.to_string()),
            o.qmp
                .as_ref()
                .map_or("(none)".to_string(), |q| q.events.join(",")),
            qmp_error.unwrap_or("(none)"),
        );
        // Per-run only: `runs/<run-id>/` is ignored, so this leaves nothing to
        // commit or churn, unlike the three flat summary files.
        let _ = fs::write(run_dir.join("qemu-shutdown.txt"), &record);
        match qemu_shutdown::judge(expected, shutdown, o.exit_code, qmp_error) {
            Ok(()) => println!(
                "[xtask] QEMU reported SHUTDOWN `{expected}`, guest-initiated, and exited by \
                 itself (exit {}) ✓",
                o.exit_code.unwrap_or(-1)
            ),
            Err(why) => {
                eprintln!("[xtask] shutdown check FAILED — {why}");
                all_ok = false;
            }
        }
    }
    // RFC-0.34-002 D9/R7: the console is a surface a person reads, and this is where
    // "clean" becomes a checked claim rather than an assertion that a fix compiled.
    if p.clean_console {
        // A byte-range check (`b < 0x20`) is not enough: the bytes E-062 was filed
        // over, `0x90`/`0x92`, are C1 controls (`0x80..=0x9F`), which that range
        // misses entirely — confirmed by running this exact check against the
        // unmodified `storaged` in a scratch worktree, where it wrongly reported
        // clean. Deciding by valid UTF-8 plus Unicode's own control-character
        // classification catches both C0 and C1 controls correctly, and — unlike a
        // blanket "no byte ≥ 0x80" — still allows a tier that prints braille (valid
        // multi-byte UTF-8) to opt in later without this check flagging it.
        match core::str::from_utf8(&combined) {
            Err(e) => {
                eprintln!(
                    "[xtask] clean-console check FAILED — invalid UTF-8 at byte offset {} in {}",
                    e.valid_up_to(),
                    log_path.display()
                );
                all_ok = false;
            }
            Ok(text) => match text
                .char_indices()
                .find(|&(_, c)| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            {
                Some((i, c)) => {
                    eprintln!(
                        "[xtask] clean-console check FAILED — control character {c:?} at byte offset {i} in {}",
                        log_path.display()
                    );
                    all_ok = false;
                }
                None => println!(
                    "[xtask] console is clean (valid UTF-8, no control character other than \\n \\r \\t) ✓"
                ),
            },
        }
    }
    // D5: each once-marker appears exactly once (a second writer is a failure).
    for m in &p.once_markers {
        let n = qemu_shutdown::count_boots(&combined, m.as_bytes());
        if let Err(why) = qemu_shutdown::judge_once(n, m) {
            eprintln!("[xtask] once-marker check FAILED — {why}");
            all_ok = false;
        }
    }
    // RFC-0.38-001 R6: the recorded measurement must EQUAL the host's independent
    // recomputation. Recorded before it is judged, so a failure can be read
    // without re-running.
    if !p.expect_image_set_digest.is_empty() {
        match crate::image_set::image_set_digest(Path::new(".")) {
            Err(why) => {
                eprintln!("[xtask] image-set digest cannot be recomputed on the host — {why}");
                all_ok = false;
            }
            Ok((count, want)) => {
                let text = String::from_utf8_lossy(&combined);
                let mut record = format!("images = {count}\nhost_recomputed = {want}\n");
                let detail = format!("{count} images, sha256:{want}");
                if !judge_hex_prefixes(
                    &text,
                    &p.expect_image_set_digest,
                    &want,
                    "image-set digest",
                    &format!("over {count} prebuilt image(s)"),
                    &detail,
                    &mut record,
                ) {
                    all_ok = false;
                }
                let _ = fs::write(run_dir.join("image-set-digest.txt"), record);
            }
        }
    }
    // RFC-0.39-002 R5: the chain head a signed bundle carries must EQUAL the
    // host's independent derivation, not merely be present.
    if !p.expect_chain_head.is_empty() {
        match crate::chain_head::boot_chain_head(Path::new(".")) {
            Err(why) => {
                eprintln!("[xtask] chain head cannot be recomputed on the host — {why}");
                all_ok = false;
            }
            Ok((events, want)) => {
                let text = String::from_utf8_lossy(&combined);
                let mut record = format!("events = {events}\nhost_recomputed = {want}\n");
                let detail = format!("{events} events, sha256:{want}");
                if !judge_hex_prefixes(
                    &text,
                    &p.expect_chain_head,
                    &want,
                    "chain head",
                    &format!("over the {events} events a boot appends"),
                    &detail,
                    &mut record,
                ) {
                    all_ok = false;
                }
                let _ = fs::write(run_dir.join("chain-head.txt"), record);
            }
        }
    }
    // 0.44 small item E: a digest one service produced and another reports it fetched must
    // be the same digest, compared as two console lines.
    if !p.expect_equal.is_empty() {
        let text = String::from_utf8_lossy(&combined);
        let mut record = String::new();
        for spec in &p.expect_equal {
            match judge_equal_digests(&text, spec) {
                Ok(note) => {
                    println!("[xtask] `{spec}` — {note} ✓");
                    record.push_str(&format!("equal: {spec}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] equality check FAILED — {why}");
                    record.push_str(&format!("equal: {spec}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        let _ = fs::write(run_dir.join("equal-digests.txt"), record);
    }
    // RFC-0.45-001 D3: lines that must NOT occur (an absence scenario: no NIC attached, so no
    // `found` line may be printed).
    if !p.expect_absent.is_empty() {
        let text = String::from_utf8_lossy(&combined);
        let mut record = String::new();
        for needle in &p.expect_absent {
            match judge_absent(&text, needle) {
                Ok(note) => {
                    println!("[xtask] absent: {note} ✓");
                    record.push_str(&format!("absent: {needle}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] absence check FAILED — {why}");
                    record.push_str(&format!("absent: {needle}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        let _ = fs::write(run_dir.join("absent.txt"), record);
    }
    // RFC-0.41-001: what a test did must reach the trail (in time), and the trail
    // must account for a burst it overflowed.
    if !p.expect_echoed.is_empty()
        || !p.expect_accounted.is_empty()
        || !p.expect_observed.is_empty()
        || !p.expect_scoped.is_empty()
    {
        let text = String::from_utf8_lossy(&combined);
        let mut record = String::new();
        for spec in &p.expect_echoed {
            match crate::audit_trail::judge_echoes(&text, spec) {
                Ok(note) => {
                    println!("[xtask] `{spec}` — {note} ✓");
                    record.push_str(&format!("echoed: {spec}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] echo check FAILED — {why}");
                    record.push_str(&format!("echoed: {spec}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        for prefix in &p.expect_accounted {
            match crate::audit_trail::judge_accounted(&text, prefix) {
                Ok(note) => {
                    println!("[xtask] `{prefix}…` — {note} ✓");
                    record.push_str(&format!("accounted: {prefix}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] accounting check FAILED — {why}");
                    record.push_str(&format!("accounted: {prefix}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        for spec in &p.expect_observed {
            match crate::audit_trail::judge_observed(&text, spec) {
                Ok(note) => {
                    println!("[xtask] `{spec}` — {note} ✓");
                    record.push_str(&format!("observed: {spec}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] observation check FAILED — {why}");
                    record.push_str(&format!("observed: {spec}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        for spec in &p.expect_scoped {
            match crate::audit_trail::judge_scoped(&text, spec) {
                Ok(note) => {
                    println!("[xtask] `{spec}` — {note} ✓");
                    record.push_str(&format!("scoped: {spec}: {note}\n"));
                }
                Err(why) => {
                    eprintln!("[xtask] scope check FAILED — {why}");
                    record.push_str(&format!("scoped: {spec}: FAILED {why}\n"));
                    all_ok = false;
                }
            }
        }
        let _ = fs::write(run_dir.join("audit-trail.txt"), record);
    }
    for marker in &p.expected_markers {
        let ok = combined
            .windows(marker.len())
            .any(|w| w == marker.as_bytes());
        if !ok {
            eprintln!(
                "[xtask] missing marker `{marker}` in {}",
                log_path.display()
            );
            all_ok = false;
        }
    }

    // Fail-closed (architect review v0.19 RB-01): the run FAILS if the serial
    // log contains any harness-failure or panic marker, even when every
    // expected marker matched. A wrong-error or unexpected-success result must
    // never produce a green profile.
    const FORBIDDEN: &[&str] = &[
        "NEG:HARNESS:WRONG_ERROR",
        "NEG:HARNESS:UNEXPECTED_OK",
        // RFC-0.28-003: test_ipc_blocked_recv's own D3-required exhaustion
        // markers. This list is a closed, hand-maintained set (E-014's own
        // "instruments deciding by fixed-string match" family) — a new
        // NEG:HARNESS:* marker has no effect on this gate until it is added
        // here explicitly, which is why these two are, rather than a
        // rename or a generic prefix match (the latter would also catch
        // NEG:HARNESS:CSpace_LAYOUT_VALID:PASS, a real passing marker).
        "NEG:HARNESS:BLOCKED_RECV_IDENTITY_EXCHANGE_FAILED",
        "NEG:HARNESS:BLOCKED_RECV_POLL_EXHAUSTED",
        // RFC-0.42-001's review, 2026-11-10: the seven below were emitted by
        // `fjell-neg-test` and absent from this list, so the gate read a harness
        // failure and passed. `fjell-neg-test` emits eleven `NEG:HARNESS:*`
        // markers; four were listed. Found because an `audit`-profile boot printed
        // `SETUP_FAILED` and the profile still reported PASS. `REBOOT_RETURNED` is
        // the sharpest: `tests/qemu/profiles/reboot.toml:36` states it is "a
        // forbidden harness marker" and that "the run fails closed", which was not
        // true of this list. Each is a failure or a failed-setup signal, checked at
        // its emitting site; none appears in any retained serial log, so adding
        // them turns no tier red today.
        "NEG:HARNESS:SETUP_FAILED",
        "NEG:HARNESS:REBOOT_NARROW_MINT_FAILED",
        "NEG:HARNESS:REBOOT_RETURNED",
        "NEG:HARNESS:TRY_RECV_REPLY_REFUSED",
        "NEG:HARNESS:UPGRADE_ACTIVE_SLOT_CALL_FAILED",
        "NEG:HARNESS:UPGRADE_VERIFY_INVALID_CALL_FAILED",
        "NEG:HARNESS:UPGRADE_VERIFY_UNSIGNED_CALL_FAILED",
        // RFC-0.39-004: the kernel's own refusals when it will not zero, free or
        // unmap a DMA frame it cannot verify. They are printed and the frame is
        // left alone, which is safe -- and invisible to a gate unless named here.
        // In a normal run none ever appears; if one does, the v0.8.x question
        // ("was the unmap called with a wrong VA?") has an answer.
        "kernel: DMA frame",
        "kernel: DMA region",
        "kernel: free of DMA frame",
        "kernel: sys_dma_alloc for task",
        "kernel panic",
        "panicked at",
    ];
    for bad in FORBIDDEN {
        let found = combined.windows(bad.len()).any(|w| w == bad.as_bytes());
        if found {
            eprintln!(
                "[xtask] FORBIDDEN marker `{bad}` present in {}",
                log_path.display()
            );
            all_ok = false;
        }
    }
    // RFC-0.29-002 R3/D1: the literal "TEST:FAIL" used to live in
    // FORBIDDEN above, but it is not a substring of the real message
    // this project actually emits — `fjell-kernel`'s
    // `trap/dispatch.rs:486` prints `TEST:M7:FAIL (init did not exit
    // cleanly)`, and "TEST:" immediately followed by "FAIL" never
    // occurs; the milestone token always sits between them. Structural
    // check instead: `TEST:<token>:FAIL` for any token, not one literal
    // shape.
    if contains_test_fail_marker(&combined) {
        eprintln!(
            "[xtask] FORBIDDEN marker `TEST:<milestone>:FAIL` present in {}",
            log_path.display()
        );
        all_ok = false;
    }
    let summary = if all_ok { "PASS\n" } else { "FAIL\n" };
    art.write_summary("result-summary.txt", summary.as_bytes());

    if all_ok {
        println!(
            "[xtask] profile `{}` PASS ({} marker(s) matched) ✓",
            p.name,
            p.expected_markers.len()
        );
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "[xtask] profile `{}` FAIL — see {}",
            p.name,
            log_path.display()
        );
        // Print the last 60 lines of serial.log directly so failures are
        // visible without opening a separate file (RFC-v0.7.1-003 §smoke).
        if let Ok(log_bytes) = fs::read(&log_path) {
            let log_text = String::from_utf8_lossy(&log_bytes);
            let lines: Vec<&str> = log_text.lines().collect();
            let tail = if lines.len() > 60 {
                &lines[lines.len() - 60..]
            } else {
                &lines[..]
            };
            eprintln!("[xtask] --- serial.log tail ({} lines) ---", tail.len());
            for line in tail {
                eprintln!("[serial] {line}");
            }
            eprintln!("[xtask] --- end serial.log ---");
        }
        ExitCode::FAILURE
    }
}

/// Everything one QEMU run produced.
struct RunOutcome {
    /// stdout and stderr merged as they arrived (the serial console).
    output: Vec<u8>,
    /// The wrapper's exit status. **124 means `timeout` killed QEMU** — the
    /// signature of a hang, and of a reset loop.
    exit_code: Option<i32>,
    /// QEMU's own account of why it stopped, when a QMP socket was requested.
    qmp: Option<qemu_shutdown::QmpResult>,
}

/// Run `timeout <argv...>` with piped stdio.
///
/// * `inject` — write `.1` to the child's stdin the first time `.0` appears
///   in its output (RFC-0.25-001 Demonstration 6: simulates a character typed
///   at the QEMU console, since `-nographic` wires UART0's RX to host stdin).
/// * `qmp_sock` — watch QEMU's QMP socket for its `SHUTDOWN` event
///   (RFC-0.33-001): the guest's own store to a reset device is invisible to
///   the serial log, but QEMU reports it. The socket must already be in
///   `argv` (`-qmp unix:<sock>,server=on,wait=on`).
///
/// Output is merged as it arrives rather than stdout-then-stderr, which is
/// more accurate about ordering, not less.
fn run_qemu(
    argv: &[String],
    inject: Option<(&[u8], &[u8])>,
    qmp_sock: Option<&Path>,
    qmp_deadline: std::time::Duration,
) -> RunOutcome {
    let mut child = Command::new("timeout")
        .args(argv)
        .stdin(if inject.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn qemu-system-riscv64");

    let child_stdin = child.stdin.take();
    let mut child_stdout = child.stdout.take().expect("piped stdout");
    let mut child_stderr = child.stderr.take().expect("piped stderr");

    let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let injected = Arc::new(AtomicBool::new(false));

    let stdout_thread = {
        let buf = Arc::clone(&buf);
        let injected = Arc::clone(&injected);
        let inject = inject.map(|(m, b)| (m.to_vec(), b.to_vec()));
        let mut stdin = child_stdin;
        thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match child_stdout.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut b = buf.lock().unwrap();
                        b.extend_from_slice(&chunk[..n]);
                        if let (Some((marker, bytes)), Some(stdin)) = (&inject, stdin.as_mut()) {
                            if !injected.load(Ordering::SeqCst)
                                && !marker.is_empty()
                                && b.windows(marker.len()).any(|w| w == marker.as_slice())
                            {
                                let _ = stdin.write_all(bytes);
                                let _ = stdin.flush();
                                injected.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }
            }
        })
    };

    let stderr_thread = {
        let buf = Arc::clone(&buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 256];
            loop {
                match child_stderr.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.lock().unwrap().extend_from_slice(&chunk[..n]),
                }
            }
        })
    };

    let qmp_thread = qmp_sock.map(|sock| {
        let sock = sock.to_path_buf();
        thread::spawn(move || qemu_shutdown::watch(&sock, qmp_deadline))
    });

    let status = child.wait();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let qmp = qmp_thread.and_then(|t| t.join().ok());

    RunOutcome {
        output: Arc::try_unwrap(buf)
            .map(|m| m.into_inner().unwrap())
            .unwrap_or_default(),
        exit_code: status.ok().and_then(|s| s.code()),
        qmp,
    }
}

/// The stdin-injection path (`uart-rx` profiles), as a wrapper over
/// [`run_qemu`] — one runner, so the two cannot drift.
fn run_with_stdin_injection(argv: &[String], marker: &[u8], inject_bytes: &[u8]) -> Vec<u8> {
    run_qemu(
        argv,
        Some((marker, inject_bytes)),
        None,
        std::time::Duration::ZERO,
    )
    .output
}

/// Minimal TOML reader for the v0.1.x profile schema.
///
/// Supports:
///   name             = "string"
///   kernel           = "path"
///   disk             = "path"
///   timeout_secs     = integer
///   expected_markers = ["a", "b", "c"]
///   extra_args       = ["-d", "trace:..."]
///   pre_blk_args     = ["-netdev", ...]   (RFC-0.50-003 D5: QEMU args placed BEFORE the blk device)
///   multi_node       = true|false   (RFC-0.29-001)
///   release_gated    = true|false   (RFC-0.29-001; requires not_gated_reason when false)
///   not_gated_reason = "string"     (RFC-0.29-001)
///   expect_shutdown  = "guest-reset" | "guest-shutdown"   (RFC-0.33-001)
/// Does this profile end by the machine's own reset (`expect_shutdown`)? Such a
/// tier finishes in well under a second while its neighbours wait out a
/// 60-second timeout, which reads like a skipped run in a summary table; the
/// summary says what it is instead (RFC-0.33-001's D15 review).
pub(crate) fn profile_ends_by_machine_shutdown(root: &Path, name: &str) -> bool {
    load_profile(root, name).is_ok_and(|p| p.expect_shutdown.is_some())
}

pub(crate) fn load_profile(root: &Path, name: &str) -> Result<Profile, String> {
    let path = root
        .join("tests/qemu/profiles")
        .join(format!("{name}.toml"));
    let src =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let mut name_v = name.to_string();
    let mut kernel_v = PathBuf::from(KERNEL_ELF);
    let mut disk_v = PathBuf::from("fjell-disk.img");
    let mut timeout_v: u32 = 60;
    let mut markers: Vec<String> = Vec::new();
    let mut pre_blk: Vec<String> = Vec::new();
    let mut extra: Vec<String> = Vec::new();
    let mut inject_after_marker_v: Option<String> = None;
    let mut inject_bytes_v: Option<String> = None;
    let mut multi_node_v = false;
    let mut release_gated_v = true;
    let mut not_gated_reason_v: Option<String> = None;
    let mut expect_shutdown_v: Option<String> = None;
    let mut expect_boots_v: Option<u32> = None;
    let mut boot_banner_v: Option<String> = None;
    let mut per_boot_v: Vec<String> = Vec::new();
    let mut once_v: Vec<String> = Vec::new();
    let mut clean_console_v = false;
    let mut expect_image_set_digest_v: Vec<String> = Vec::new();
    let mut expect_chain_head_v: Vec<String> = Vec::new();
    let mut expect_echoed_v: Vec<String> = Vec::new();
    let mut expect_accounted_v: Vec<String> = Vec::new();
    let mut expect_observed_v: Vec<String> = Vec::new();
    let mut expect_scoped_v: Vec<String> = Vec::new();
    let mut expect_equal_v: Vec<String> = Vec::new();
    let mut expect_absent_v: Vec<String> = Vec::new();

    let mut lines = src.lines().enumerate().peekable();
    while let Some((idx, raw)) = lines.next() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        // An array (`key = [ ... ]`, on one line or several) is read by a
        // quote-aware reader (RFC-0.33-004 D6, E-057): the array ends at the
        // first `]` **outside a string**, a comma or bracket inside a marker is
        // part of the marker, and a `#` comment outside a string is ignored.
        // What it cannot carry is refused here, with the line, instead of being
        // reshaped — the old reader split on every comma and closed at the first
        // `]` anywhere, so a marker could be shortened without a word.
        let v_owned: String = if v.starts_with('[') {
            let mut acc = String::from(v);
            loop {
                match scan_array(&acc) {
                    Scan::Done(_) => break,
                    Scan::Bad(msg, at) => {
                        return Err(format!(
                            "{}: `{k}` (line {}): {msg}",
                            path.display(),
                            idx + 1 + acc[..at].matches('\n').count()
                        ));
                    }
                    Scan::Incomplete => {
                        let Some((_, cont)) = lines.next() else {
                            return Err(format!(
                                "{}: `{k}` (line {}): the array is never closed",
                                path.display(),
                                idx + 1
                            ));
                        };
                        acc.push('\n');
                        acc.push_str(cont);
                    }
                }
            }
            acc
        } else {
            v.to_string()
        };
        let v = v_owned.as_str();
        match k {
            "name" => name_v = unquote(v),
            "kernel" => kernel_v = PathBuf::from(unquote(v)),
            "disk" => disk_v = PathBuf::from(unquote(v)),
            "timeout_secs" => {
                timeout_v = v
                    .parse::<u32>()
                    .map_err(|e| format!("bad timeout_secs: {e}"))?
            }
            "expected_markers" => markers = array_items(&path, k, idx, v)?,
            "extra_args" => extra = array_items(&path, k, idx, v)?,
            "pre_blk_args" => pre_blk = array_items(&path, k, idx, v)?,
            // RFC-0.25-001 (Demonstration 6): a byte is written to QEMU's
            // stdin once `inject_after_marker` appears in the output.
            // `inject_bytes` is the literal string whose bytes are sent —
            // both keys must be present or injection is disabled.
            "inject_after_marker" => inject_after_marker_v = Some(unquote(v)),
            "inject_bytes" => inject_bytes_v = Some(unquote(v)),
            "multi_node" => {
                multi_node_v = parse_bool(v).map_err(|e| format!("bad multi_node: {e}"))?
            }
            "release_gated" => {
                release_gated_v = parse_bool(v).map_err(|e| format!("bad release_gated: {e}"))?
            }
            "not_gated_reason" => not_gated_reason_v = Some(unquote(v)),
            "expect_shutdown" => expect_shutdown_v = Some(unquote(v)),
            "expect_boots" => {
                expect_boots_v = Some(
                    v.parse::<u32>()
                        .map_err(|e| format!("bad expect_boots: {e}"))?,
                )
            }
            "boot_banner" => boot_banner_v = Some(unquote(v)),
            "per_boot_markers" => per_boot_v = array_items(&path, k, idx, v)?,
            "once_markers" => once_v = array_items(&path, k, idx, v)?,
            "clean_console" => {
                clean_console_v = parse_bool(v).map_err(|e| format!("bad clean_console: {e}"))?
            }
            "expect_image_set_digest" => expect_image_set_digest_v = array_items(&path, k, idx, v)?,
            "expect_chain_head" => expect_chain_head_v = array_items(&path, k, idx, v)?,
            "expect_echoed" => expect_echoed_v = array_items(&path, k, idx, v)?,
            "expect_accounted" => expect_accounted_v = array_items(&path, k, idx, v)?,
            "expect_observed" => expect_observed_v = array_items(&path, k, idx, v)?,
            "expect_scoped" => expect_scoped_v = array_items(&path, k, idx, v)?,
            "expect_equal" => expect_equal_v = array_items(&path, k, idx, v)?,
            "expect_absent" => expect_absent_v = array_items(&path, k, idx, v)?,
            _ => {} // forward-compatibility: ignore unknown keys
        }
    }

    if !release_gated_v && not_gated_reason_v.as_deref().unwrap_or("").is_empty() {
        return Err(format!(
            "{}: release_gated = false requires a non-empty not_gated_reason (RFC-0.29-001 D5) \
             — say why this category isn't release-gated, in the file a reader checking it will \
             already have open",
            path.display()
        ));
    }

    // RFC-0.45-001 D3: an `expect_absent` that cannot work is refused at load time.
    {
        let mut required: Vec<String> = markers.clone();
        required.extend(once_v.iter().cloned());
        required.extend(per_boot_v.iter().cloned());
        if let Some(why) = expect_absent_problem(&expect_absent_v, &required) {
            return Err(format!("{}: {why}", path.display()));
        }
    }

    // RFC-0.33-001: a typo here must not silently become "no expectation" —
    // the unknown-key arm above ignores misspelt *keys*, so a misspelt
    // *value* is refused at load time instead.
    if let Some(v) = &expect_shutdown_v {
        if !qemu_shutdown::EXPECTABLE.contains(&v.as_str()) {
            return Err(format!(
                "{}: expect_shutdown = {v:?} is not one of {:?} (`host-signal` is what a hang \
                 looks like, and is never an outcome to expect)",
                path.display(),
                qemu_shutdown::EXPECTABLE
            ));
        }
    }

    // RFC-0.33-001 D21: the count of boots is only meaningful against a banner,
    // and only when QEMU is allowed to reboot — `expect_shutdown` adds
    // `-no-reboot`, so the two cannot describe one run.
    if let Some(n) = expect_boots_v {
        if n == 0 || boot_banner_v.as_deref().unwrap_or("").is_empty() {
            return Err(format!(
                "{}: expect_boots = {n} needs a count of at least 1 and a non-empty boot_banner \
                 (a line printed late in every boot)",
                path.display()
            ));
        }
        if expect_shutdown_v.is_some() {
            return Err(format!(
                "{}: expect_boots and expect_shutdown cannot both be set: expect_shutdown runs \
                 with -no-reboot, so a second boot could never happen",
                path.display()
            ));
        }
    }

    if !per_boot_v.is_empty() && expect_boots_v.is_none() {
        return Err(format!(
            "{}: per_boot_markers is judged against expect_boots (a marker must appear once per \
             boot), and expect_boots is not set",
            path.display()
        ));
    }

    let inject_after_marker = match (inject_after_marker_v, inject_bytes_v) {
        (Some(m), Some(b)) => Some((m, b.into_bytes())),
        _ => None,
    };

    Ok(Profile {
        name: name_v,
        kernel: kernel_v,
        disk: disk_v,
        timeout_secs: timeout_v,
        expected_markers: markers,
        extra_args: extra,
        pre_blk_args: pre_blk,
        inject_after_marker,
        multi_node: multi_node_v,
        release_gated: release_gated_v,
        not_gated_reason: not_gated_reason_v,
        expect_shutdown: expect_shutdown_v,
        expect_boots: expect_boots_v,
        boot_banner: boot_banner_v,
        per_boot_markers: per_boot_v,
        once_markers: once_v,
        clean_console: clean_console_v,
        expect_image_set_digest: expect_image_set_digest_v,
        expect_chain_head: expect_chain_head_v,
        expect_echoed: expect_echoed_v,
        expect_accounted: expect_accounted_v,
        expect_observed: expect_observed_v,
        expect_scoped: expect_scoped_v,
        expect_equal: expect_equal_v,
        expect_absent: expect_absent_v,
    })
}

fn parse_bool(v: &str) -> Result<bool, String> {
    match v.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("expected true or false, got {other:?}")),
    }
}

/// One entry of `discover_negative_categories`'s derived list.
pub struct CategoryInfo {
    pub name: String,
    pub release_gated: bool,
    pub not_gated_reason: Option<String>,
}

/// RFC-0.29-001 D1/§6: the authority for "the negative-test categories" is
/// `tests/qemu/profiles/*.toml` itself, not any of the five lists the RFC
/// found disagreeing (this project's own `NEG_CATEGORIES`, `ci.yml`'s
/// matrix, two `KNOWN_*` constants, and a stale doc-comment). A profile is
/// a negative-test category unless its own `multi_node = true` says it
/// needs a different runner entirely (`fleet-demo`, which requires three
/// simultaneous QEMU instances and is invoked through `cargo xtask
/// fleet-demo`, never through this module's single-QEMU `run_profile`) —
/// a structural exclusion that does not grow as new single-QEMU profiles
/// are added, unlike a name a person must remember to list.
///
/// Every remaining profile is included, gated or not (D5): `store` and
/// `upgrade` are swept in like any other category, distinguished only by
/// their own `release_gated = false` — the derivation does not special-
/// case them by name.
pub fn discover_negative_categories() -> Result<Vec<CategoryInfo>, String> {
    discover_negative_categories_at(Path::new("."))
}

/// `root` is `.` for the real invocation (`cargo xtask` always runs from
/// the workspace root); tests pass an absolute path computed from
/// `CARGO_MANIFEST_DIR` instead, since `cargo test` runs test binaries
/// with the crate's own directory as the working directory, not the
/// workspace's (same reasoning as `callsite_audit`'s `test_workspace_root`).
pub fn discover_negative_categories_at(root: &Path) -> Result<Vec<CategoryInfo>, String> {
    let dir = root.join("tests/qemu/profiles");
    let entries = fs::read_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;

    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }
    names.sort();

    let mut out = Vec::new();
    for name in names {
        let p = load_profile(root, &name)?;
        if p.multi_node {
            continue;
        }
        out.push(CategoryInfo {
            name,
            release_gated: p.release_gated,
            not_gated_reason: p.not_gated_reason,
        });
    }
    Ok(out)
}

/// Does `combined` contain a `TEST:<token>:FAIL` marker, for any milestone
/// token (`M7`, `V0.4-NET`, ...) — not just the literal `"TEST:FAIL"`,
/// which is not a substring of the real message this project emits
/// (`TEST:M7:FAIL (init did not exit cleanly)`, `trap/dispatch.rs:486`).
/// A milestone token never itself contains `:`, so scanning for the next
/// `:` after each `TEST:` occurrence and checking whether it starts
/// `:FAIL` is exact, not a heuristic.
fn contains_test_fail_marker(combined: &[u8]) -> bool {
    let s = String::from_utf8_lossy(combined);
    let mut rest: &str = &s;
    while let Some(pos) = rest.find("TEST:") {
        let after = &rest[pos + "TEST:".len()..];
        if let Some(colon) = after.find(':') {
            if after[colon..].starts_with(":FAIL") {
                return true;
            }
        }
        rest = after;
    }
    false
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if (t.starts_with('"') && t.ends_with('"') && t.len() >= 2)
        || (t.starts_with('\'') && t.ends_with('\'') && t.len() >= 2)
    {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

/// The result of reading a (possibly partial) array value.
enum Scan {
    /// The array closed; the items, in order.
    Done(Vec<String>),
    /// The text ended outside a string with no closing `]`: more lines may follow.
    Incomplete,
    /// The text cannot be read as an array of strings: what is wrong, and the
    /// byte offset it is at.
    Bad(&'static str, usize),
}

/// Read a TOML-style array of strings: `["a", 'b', ...]`, across any number of
/// lines (`s` holds them joined with `\n`).
///
/// Supported, deliberately little: basic strings with the escapes `\"` and `\\`,
/// literal `'...'` strings, `#` comments outside strings, a trailing comma.
/// Everything else — a bare word, an unknown escape, a string that runs to the
/// end of its line, an empty marker (which matches every output), a missing
/// comma, text after the `]` — is [`Scan::Bad`]. Not a TOML parser; the profile
/// schema needs no more than this.
fn scan_array(s: &str) -> Scan {
    let b = s.as_bytes();
    debug_assert_eq!(b.first(), Some(&b'['));
    let mut i = 1;
    let mut items: Vec<String> = Vec::new();
    let mut want_item = true; // false after an item: a comma or `]` must follow
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'#' => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b',' => {
                if want_item {
                    return Scan::Bad("a comma with no marker before it", i);
                }
                want_item = true;
                i += 1;
            }
            b']' => {
                // Only whitespace or a comment may follow the closing bracket.
                let mut j = i + 1;
                while j < b.len() {
                    match b[j] {
                        b' ' | b'\t' | b'\r' | b'\n' => j += 1,
                        b'#' => {
                            while j < b.len() && b[j] != b'\n' {
                                j += 1;
                            }
                        }
                        _ => return Scan::Bad("text after the closing `]`", j),
                    }
                }
                return Scan::Done(items);
            }
            q @ (b'"' | b'\'') => {
                if !want_item {
                    return Scan::Bad("expected a comma between two markers", i);
                }
                let start = i;
                i += 1;
                let mut item: Vec<u8> = Vec::new();
                loop {
                    match b.get(i) {
                        None | Some(b'\n') => return Scan::Bad("unterminated string", start),
                        Some(&c) if c == q => {
                            i += 1;
                            break;
                        }
                        Some(b'\\') if q == b'"' => match b.get(i + 1) {
                            Some(b'"') => {
                                item.push(b'"');
                                i += 2;
                            }
                            Some(b'\\') => {
                                item.push(b'\\');
                                i += 2;
                            }
                            _ => {
                                return Scan::Bad(
                                    "unsupported escape (only \\\" and \\\\ are carried)",
                                    i,
                                );
                            }
                        },
                        Some(&c) => {
                            item.push(c);
                            i += 1;
                        }
                    }
                }
                if item.is_empty() {
                    return Scan::Bad("an empty marker matches every output", start);
                }
                // The bytes came from a `&str` and only whole ASCII escapes were
                // substituted, so this is valid UTF-8.
                items.push(String::from_utf8(item).expect("utf-8 preserved"));
                want_item = false;
            }
            _ => return Scan::Bad("expected a quoted string", i),
        }
    }
    Scan::Incomplete
}

/// The items of the array a key holds, or the reason the profile is refused.
fn array_items(path: &Path, key: &str, idx: usize, v: &str) -> Result<Vec<String>, String> {
    if !v.starts_with('[') {
        return Err(format!(
            "{}: `{key}` (line {}): expected an array `[...]`",
            path.display(),
            idx + 1
        ));
    }
    match scan_array(v) {
        Scan::Done(items) => Ok(items),
        Scan::Bad(msg, at) => Err(format!(
            "{}: `{key}` (line {}): {msg}",
            path.display(),
            idx + 1 + v[..at].matches('\n').count()
        )),
        Scan::Incomplete => Err(format!(
            "{}: `{key}` (line {}): the array is never closed",
            path.display(),
            idx + 1
        )),
    }
}

#[cfg(test)]
mod forbidden_tests {
    use super::*;

    /// RFC-0.29-002 R3 required demonstration: the real message this
    /// project emits (`trap/dispatch.rs:486`) — the old literal
    /// `"TEST:FAIL"` is not a substring of it.
    #[test]
    fn catches_the_real_test_m7_fail_message() {
        assert!(!"TEST:M7:FAIL (init did not exit cleanly)".contains("TEST:FAIL"));
        assert!(contains_test_fail_marker(
            b"TEST:M7:FAIL (init did not exit cleanly)"
        ));
    }

    #[test]
    fn does_not_match_a_passing_marker() {
        assert!(!contains_test_fail_marker(b"TEST:M7:PASS"));
        assert!(!contains_test_fail_marker(b"TEST:V0.4-NET:PASS"));
    }

    #[test]
    fn matches_any_milestone_token() {
        assert!(contains_test_fail_marker(b"TEST:V0.5-PLATFORM:FAIL"));
    }
}

/// RFC-0.33-004 D6 / E-057: the profile reader respects quoting. Each test loads a
/// real file through `load_profile`, so it exercises the reader as `qemu-negative`
/// does, not a helper beside it.
#[cfg(test)]
mod array_reader_tests {
    use super::*;

    fn load(body: &str) -> Result<Profile, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("qemu_run_array_{}_{n}", std::process::id()));
        let profiles = dir.join("tests/qemu/profiles");
        fs::create_dir_all(&profiles).unwrap();
        fs::write(profiles.join("p.toml"), format!("name = \"p\"\n{body}")).unwrap();
        let r = load_profile(&dir, "p");
        fs::remove_dir_all(&dir).ok();
        r
    }

    fn markers(body: &str) -> Vec<String> {
        load(body).expect("loads").expected_markers
    }

    /// E-014's originally-filed instance and E-057's bracket half: a `]` inside a
    /// marker string is part of the marker. (This test used to assert the *bug*,
    /// "still live"; it now asserts the fix.)
    #[test]
    fn a_bracket_inside_a_marker_is_part_of_the_marker() {
        let m = markers(
            "expected_markers = [\n    \"[INTENT] marker one\",\n    \"marker two\",\n    \"marker three\",\n]\n",
        );
        assert_eq!(m, ["[INTENT] marker one", "marker two", "marker three"]);
    }

    /// E-057's comma half: `health-fail.toml` had to split one printed line in two.
    #[test]
    fn a_comma_inside_a_marker_is_part_of_the_marker() {
        let m = markers("expected_markers = [\"one, two\", \"three\"]\n");
        assert_eq!(m, ["one, two", "three"]);
    }

    #[test]
    fn both_at_once_on_one_line_and_across_lines() {
        assert_eq!(
            markers("expected_markers = [\"[A] x, y ]\", \"z\"]\n"),
            ["[A] x, y ]", "z"]
        );
        assert_eq!(
            markers("expected_markers = [\n  \"[A] x, y ]\",\n  \"z\",\n]\n"),
            ["[A] x, y ]", "z"]
        );
    }

    #[test]
    fn a_comment_inside_an_array_is_not_a_marker() {
        let m = markers(
            "expected_markers = [\n  # a note, with a comma and a ] bracket\n  \"a\", # trailing, note ]\n  \"b\",\n]\n",
        );
        assert_eq!(m, ["a", "b"]);
    }

    #[test]
    fn a_hash_inside_a_string_is_not_a_comment() {
        assert_eq!(markers("expected_markers = [\"a # b\"]\n"), ["a # b"]);
    }

    #[test]
    fn escaped_quote_and_backslash_are_carried() {
        assert_eq!(
            markers("expected_markers = [\"say \\\"hi\\\" and \\\\ done\"]\n"),
            [r#"say "hi" and \ done"#]
        );
    }

    #[test]
    fn single_quoted_literal_strings_are_carried_verbatim() {
        assert_eq!(
            markers("expected_markers = ['a, \"b\" ]']\n"),
            ["a, \"b\" ]"]
        );
    }

    #[test]
    fn a_trailing_comma_and_an_empty_array_are_fine() {
        assert_eq!(markers("expected_markers = [\"a\",]\n"), ["a"]);
        assert!(
            load("expected_markers = []\nrelease_gated = false\nnot_gated_reason = \"x\"\n")
                .unwrap()
                .expected_markers
                .is_empty()
        );
    }

    /// **RFC-0.50-003 D5, verified and not assumed:** adding `pre_blk_args` changed no existing command line. For every committed
    /// profile with a recorded `qemu-command.txt`, the pure argv builder reproduces that file **byte for byte** (a profile that ends
    /// by machine reset appends `-no-reboot -qmp ...` after the builder's output, so for those the record must *start with* it).
    /// Moving the blk `-device` after `extra_args` instead -- the tidier-looking fix -- would fail this for every profile.
    #[test]
    fn every_existing_command_line_is_byte_identical_after_pre_blk_args() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut checked = 0;
        for entry in fs::read_dir(root.join("tests/qemu/profiles")).unwrap().flatten() {
            let path = entry.path();
            let Some(name) = path.file_stem().map(|s| s.to_string_lossy().to_string()) else { continue };
            if path.extension().is_none_or(|e| e != "toml") || name == "README" {
                continue;
            }
            let p = load_profile(&root, &name).unwrap();
            let rec = root.join("tests/qemu/artifacts").join(&name).join("qemu-command.txt");
            let Ok(recorded) = fs::read_to_string(&rec) else { continue };
            if !p.pre_blk_args.is_empty() {
                continue; // a profile that sets the new field is, by definition, not an old command line
            }
            let kernel = p.kernel.to_string_lossy().to_string();
            let disk = p.disk.to_string_lossy().to_string();
            let built = base_argv(&p, &kernel, &format!("file={disk},format=raw,if=none,id=hd0")).join(" ");
            if p.expect_shutdown.is_none() {
                assert_eq!(recorded, built, "{name}: the command line changed");
            } else {
                assert!(recorded.starts_with(&built), "{name}: the command line changed");
            }
            checked += 1;
        }
        assert!(checked >= 20, "only {checked} recorded profiles were compared: the test is not looking at the tree");
    }

    #[test]
    fn pre_blk_args_land_between_the_kernel_and_the_blk_device_and_extra_args_stay_after_it() {
        let mut p = Profile::smoke("m8", "x");
        p.pre_blk_args = vec!["-netdev".into(), "user,id=n0".into(), "-device".into(), "virtio-net-device,netdev=n0".into()];
        p.extra_args = vec!["-d".into(), "trace:a".into()];
        let argv = base_argv(&p, "K", "D").join(" ");
        let at = |needle: &str| argv.find(needle).unwrap_or_else(|| panic!("{needle} missing in {argv}"));
        assert!(at("-kernel K") < at("virtio-net-device") && at("virtio-net-device") < at("-drive D"));
        assert!(at("-drive D") < at("virtio-blk-device") && at("virtio-blk-device") < at("-d trace:a"));
    }

    #[test]
    fn extra_args_use_the_same_reader() {
        let p = load("expected_markers = [\"a\"]\nextra_args = [\"-d\", \"trace:a,b\"]\n").unwrap();
        assert_eq!(p.extra_args, ["-d", "trace:a,b"]);
    }

    // ── What the reader cannot carry is refused, not silently reshaped ────────

    fn refused(body: &str, needle: &str) {
        let e = load(body).expect_err("should be refused");
        assert!(
            e.contains(needle),
            "the refusal should mention {needle:?}: {e}"
        );
        assert!(
            e.contains("expected_markers") || e.contains("extra_args"),
            "{e}"
        );
    }

    #[test]
    fn an_unterminated_string_is_refused_naming_the_line() {
        refused(
            "expected_markers = [\n  \"a\",\n  \"b,\n]\n",
            "unterminated string",
        );
    }

    #[test]
    fn an_unterminated_array_is_refused() {
        refused("expected_markers = [\n  \"a\",\n", "never closed");
    }

    #[test]
    fn a_bare_word_is_refused() {
        refused("expected_markers = [a, b]\n", "quoted string");
    }

    #[test]
    fn an_unsupported_escape_is_refused() {
        refused("expected_markers = [\"a\\qb\"]\n", "escape");
    }

    #[test]
    fn an_empty_marker_is_refused_because_it_matches_everything() {
        refused("expected_markers = [\"\"]\n", "empty");
    }

    #[test]
    fn text_after_the_closing_bracket_is_refused() {
        refused("expected_markers = [\"a\"] junk\n", "after the closing");
    }

    #[test]
    fn a_missing_comma_between_two_markers_is_refused() {
        refused("expected_markers = [\"a\" \"b\"]\n", "comma");
    }

    /// The committed profiles all still load, and `health-fail`'s marker is the
    /// printed line whole, comma and all — the workaround is gone.
    #[test]
    fn every_committed_profile_loads_and_health_fail_has_its_line_whole() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut n = 0;
        for e in fs::read_dir(root.join("tests/qemu/profiles"))
            .unwrap()
            .flatten()
        {
            let file = e.file_name().to_string_lossy().to_string();
            let Some(name) = file.strip_suffix(".toml") else {
                continue; // the directory's README
            };
            load_profile(&root, name).unwrap_or_else(|err| panic!("{name}: {err}"));
            n += 1;
        }
        assert!(n >= 14, "expected the committed profiles, found {n}");
        let hf = load_profile(&root, "health-fail").unwrap();
        assert!(
            hf.expected_markers.iter().any(|m| m
                == "bootctl: health FAILED on the last confirmed slot; no fallback, not resetting"),
            "{:?}",
            hf.expected_markers
        );
    }
}

#[cfg(test)]
mod discover_tests {
    use super::*;

    /// `cargo test` runs test binaries with the crate's own directory as
    /// the working directory, not the workspace root the real `cargo
    /// xtask` invocation always uses.
    fn test_workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn scratch_root(tag: &str, profile: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("fjell-qemu-run-{tag}-{}", std::process::id()));
        let dir = root.join("tests/qemu/profiles");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("p.toml"), profile).unwrap();
        root
    }

    #[test]
    fn expect_boots_loads_from_the_real_profile_and_is_absent_elsewhere() {
        let root = test_workspace_root();
        let p = load_profile(&root, "reboot-again").expect("reboot-again loads");
        assert_eq!(p.expect_boots, Some(2));
        assert_eq!(p.boot_banner.as_deref(), Some("sched: started"));
        assert!(p.expect_shutdown.is_none(), "the two are exclusive");
        let r = load_profile(&root, "reboot").expect("reboot loads");
        assert_eq!(r.expect_boots, None);
        assert!(r.expect_shutdown.is_some());
    }

    #[test]
    fn expect_boots_is_refused_without_a_banner_or_alongside_expect_shutdown() {
        let no_banner = scratch_root("nb", "name = \"p\"\nexpect_boots = 2\n");
        let e = load_profile(&no_banner, "p").err().expect("refused");
        assert!(e.contains("boot_banner"), "{e}");
        let zero = scratch_root("z", "name = \"p\"\nexpect_boots = 0\nboot_banner = \"b\"\n");
        assert!(load_profile(&zero, "p").is_err());
        let both = scratch_root(
            "both",
            "name = \"p\"\nexpect_boots = 2\nboot_banner = \"b\"\nexpect_shutdown = \"guest-reset\"\n",
        );
        let e = load_profile(&both, "p").err().expect("refused");
        assert!(e.contains("-no-reboot"), "{e}");
        // Control: the same profile without the conflict loads.
        let ok = scratch_root(
            "ok",
            "name = \"p\"\nexpect_boots = 2\nboot_banner = \"b\"\n",
        );
        assert!(load_profile(&ok, "p").is_ok());
    }

    #[test]
    fn a_profile_that_ends_by_machine_reset_is_recognised_and_others_are_not() {
        let root = test_workspace_root();
        // `reboot` sets `expect_shutdown`, so it ends by the machine's own reset.
        assert!(profile_ends_by_machine_shutdown(&root, "reboot"));
        // Controls: profiles that wait out their timeout, and one that does not exist.
        assert!(!profile_ends_by_machine_shutdown(&root, "semantic"));
        assert!(!profile_ends_by_machine_shutdown(&root, "health-fail"));
        assert!(!profile_ends_by_machine_shutdown(&root, "no-such-profile"));
    }

    #[test]
    fn discover_negative_categories_excludes_fleet_demo_includes_store_upgrade() {
        let cats = discover_negative_categories_at(&test_workspace_root())
            .expect("discover should succeed against the real tree");
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert!(
            !names.contains(&"fleet-demo"),
            "fleet-demo is multi_node, must be excluded"
        );
        assert!(names.contains(&"store"));
        assert!(names.contains(&"upgrade"));
        let store = cats.iter().find(|c| c.name == "store").unwrap();
        assert!(!store.release_gated);
        assert!(store.not_gated_reason.is_some());
    }

    #[test]
    fn multi_node_profile_without_release_gated_still_loads() {
        // fleet-demo has no release_gated key at all — confirms the
        // default (true) applies and the loader does not require the key
        // on every profile, only on ones that opt out.
        let p = load_profile(&test_workspace_root(), "fleet-demo")
            .expect("fleet-demo.toml should still parse on its own");
        assert!(p.multi_node);
        assert!(p.release_gated);
        assert!(p.not_gated_reason.is_none());
    }

    /// E-040's neighbour, found while accounting for three untracked
    /// directories left by ungated smoke milestones: the summary files must
    /// not appear for a profile that is neither gated nor already recorded,
    /// or every ungated run leaks three untracked files that only a human
    /// deleting them can clear.
    #[test]
    fn summary_files_are_written_only_for_a_profile_that_writes_them() {
        let base = std::env::temp_dir().join(format!("qemu_art_test_{}", std::process::id()));
        fs::remove_dir_all(&base).ok();

        let fresh = base.join("ungated");
        fs::create_dir_all(&fresh).unwrap();
        let ungated = ArtifactDir {
            path: fresh.clone(),
            writes_summaries: false,
        };
        ungated.write_summary("result-summary.txt", b"FAIL\n");
        assert!(
            !fresh.join("result-summary.txt").exists(),
            "an ungated run must leave no summary file behind"
        );

        let gated = base.join("gated");
        fs::create_dir_all(&gated).unwrap();
        let tracked = ArtifactDir {
            path: gated.clone(),
            writes_summaries: true,
        };
        tracked.write_summary("result-summary.txt", b"PASS\n");
        assert_eq!(
            fs::read_to_string(gated.join("result-summary.txt")).unwrap(),
            "PASS\n"
        );

        fs::remove_dir_all(&base).ok();
    }

    /// **E-135, the defect itself.** A newly gated profile has no `expected-markers.txt` yet, and under the old rule that made it
    /// unable to write its first copy. A release-gated profile writes in a directory with nothing in it.
    #[test]
    fn a_newly_gated_profile_writes_its_first_summaries() {
        let name = format!("_qemu_art_newly_gated_{}", std::process::id());
        let dir = PathBuf::from("tests/qemu/artifacts").join(&name);
        fs::remove_dir_all(&dir).ok();
        assert!(!dir.join("expected-markers.txt").exists());

        let art = ArtifactDir::for_run(&name, true);
        assert!(art.writes_summaries, "a gated profile writes on its own");
        art.write_summary("expected-markers.txt", b"TEST:X:PASS");
        art.write_summary("result-summary.txt", b"PASS\n");
        assert!(dir.join("expected-markers.txt").is_file());
        assert!(dir.join("result-summary.txt").is_file());

        fs::remove_dir_all(&dir).ok();
    }

    /// The property that makes the E-040 guard hold under repetition -- and the one a directory-existence check does *not*
    /// have. The first version of that fix keyed on `dir.is_dir()`; the run that demonstrated it created the directory, so the
    /// *second* run of the same ungated milestone would have found it present and started writing. For a profile that is not
    /// gated, nothing changes with E-135: still nothing written, however often, until `expected-markers.txt` is committed.
    #[test]
    fn repeated_ungated_runs_never_start_writing_summaries() {
        let name = format!("_qemu_art_probe_{}", std::process::id());
        let dir = PathBuf::from("tests/qemu/artifacts").join(&name);
        fs::remove_dir_all(&dir).ok();

        let first = ArtifactDir::for_run(&name, false);
        assert!(!first.writes_summaries);
        first.write_summary("result-summary.txt", b"FAIL\n");

        // The directory now exists -- created by the first run -- which is
        // exactly the state a directory-existence guard would misread.
        assert!(dir.is_dir());
        let second = ArtifactDir::for_run(&name, false);
        assert!(
            !second.writes_summaries,
            "a second ungated run must still write nothing"
        );
        second.write_summary("result-summary.txt", b"FAIL\n");
        assert!(!dir.join("result-summary.txt").exists());

        // A profile that is not gated but whose summaries are already committed (`store`) keeps being refreshed in place.
        fs::write(dir.join("expected-markers.txt"), b"TEST:X:PASS").unwrap();
        let third = ArtifactDir::for_run(&name, false);
        assert!(third.writes_summaries);
        third.write_summary("result-summary.txt", b"PASS\n");
        assert!(dir.join("result-summary.txt").exists());

        fs::remove_dir_all(&dir).ok();
    }

    /// `Profile::smoke` says whether `test-all` gates the milestone, so an ad-hoc `qemu-test m7` does not leave files behind.
    #[test]
    fn a_smoke_milestone_is_gated_exactly_when_test_all_runs_it() {
        for m in crate::test_all::SMOKE_PROFILES {
            let p = Profile::smoke(m, "x");
            assert!(p.release_gated, "{m} is run by test-all");
            assert!(p.not_gated_reason.is_none());
        }
        let p = Profile::smoke("m7", "x");
        assert!(!crate::test_all::SMOKE_PROFILES.contains(&"m7"));
        assert!(!p.release_gated, "m7 is accepted by qemu-test and not run by test-all");
        assert!(p.not_gated_reason.is_some());
    }

    #[test]
    fn release_gated_false_without_reason_is_rejected() {
        let dir = std::env::temp_dir().join(format!("qemu_run_test_{}", std::process::id()));
        let profiles_dir = dir.join("tests/qemu/profiles");
        fs::create_dir_all(&profiles_dir).unwrap();
        fs::write(
            profiles_dir.join("bad.toml"),
            "name = \"bad\"\nexpected_markers = []\nrelease_gated = false\n",
        )
        .unwrap();
        match load_profile(&dir, "bad") {
            Err(e) => assert!(e.contains("not_gated_reason")),
            Ok(_) => panic!("release_gated=false with no reason must fail closed"),
        }
        fs::remove_dir_all(&dir).ok();
    }

    // ── RFC-0.33-001 D23: markers counted once per boot ─────────────────────

    #[test]
    fn per_boot_markers_are_refused_without_expect_boots() {
        let r = scratch_root(
            "pb1",
            "name = \"p\"\nper_boot_markers = [\"a\"]\nexpected_markers = [\"x\"]\n",
        );
        let e = load_profile(&r, "p").unwrap_err();
        assert!(
            e.contains("per_boot_markers") && e.contains("expect_boots"),
            "{e}"
        );
    }

    #[test]
    fn per_boot_markers_load_with_expect_boots_and_the_real_profile_has_a_service_level_one() {
        let r = scratch_root(
            "pb2",
            "name = \"p\"\nexpect_boots = 2\nboot_banner = \"b\"\nper_boot_markers = [\"a, b\", \"c\"]\n",
        );
        let p = load_profile(&r, "p").unwrap();
        assert_eq!(p.per_boot_markers, ["a, b", "c"]);
        // the committed tier: the device tree AND a service, per boot
        let real = load_profile(&test_workspace_root(), "reboot-again").unwrap();
        assert_eq!(real.expect_boots, Some(2));
        assert!(
            real.per_boot_markers
                .iter()
                .any(|m| m == "mm: device tree reserved")
        );
        assert!(
            real.per_boot_markers
                .iter()
                .any(|m| m == "driver-uart: ready"),
            "at least one per-boot marker must be a service's line"
        );
    }

    // ── RFC-0.39-002 R5: expect_chain_head and the shared judge ─────────────
    #[test]
    fn rfc_0_41_001_directives_are_opt_in_and_load_lists() {
        let m = Profile::smoke("m1", "x");
        assert!(m.expect_echoed.is_empty() && m.expect_accounted.is_empty());
        let r = scratch_root("ea1", "name = \"p\"\nexpected_markers = [\"a\"]\n");
        let p = load_profile(&r, "p").unwrap();
        assert!(p.expect_echoed.is_empty() && p.expect_accounted.is_empty());
        let r = scratch_root(
            "ea2",
            "name = \"p\"\nexpect_echoed = [\"src => t{} => bound\"]\nexpect_accounted = [\"burst: \"]\n",
        );
        let p = load_profile(&r, "p").unwrap();
        assert_eq!(p.expect_echoed, ["src => t{} => bound"]);
        assert_eq!(p.expect_accounted, ["burst: "]);
    }

    #[test]
    fn expect_observed_is_opt_in_and_loads_a_list() {
        assert!(Profile::smoke("m1", "x").expect_observed.is_empty());
        let r = scratch_root("eo1", "name = \"p\"\nexpected_markers = [\"a\"]\n");
        assert!(load_profile(&r, "p").unwrap().expect_observed.is_empty());
        let r = scratch_root("eo2", "name = \"p\"\nexpect_observed = [\"b => e => w\"]\n");
        assert_eq!(
            load_profile(&r, "p").unwrap().expect_observed,
            ["b => e => w"]
        );
    }

    // ── 0.44 small item E: judge_equal_digests ───────────────────────────────────

    const EQ_SPEC: &str =
        "attestd: record digest sha256: => diagnosticsd: last_attestation sha256:";

    fn hex(c: char) -> String {
        std::iter::repeat(c).take(64).collect()
    }

    fn eq_log(a: Option<&str>, b: Option<&str>) -> String {
        let mut t = String::from("noise\n");
        if let Some(a) = a {
            t.push_str(&format!("attestd: record digest sha256:{a}\n"));
        }
        t.push_str("middle line\n");
        if let Some(b) = b {
            t.push_str(&format!("diagnosticsd: last_attestation sha256:{b}\n"));
        }
        t
    }

    #[test]
    fn equal_digests_pass() {
        let d = hex('a');
        let note = judge_equal_digests(&eq_log(Some(&d), Some(&d)), EQ_SPEC).unwrap();
        assert!(
            note.contains("1 `diagnosticsd: last_attestation sha256:…` line(s) equal"),
            "{note}"
        );
    }

    /// The three reds the handoff asks for.
    #[test]
    fn digests_that_differ_fail() {
        let e =
            judge_equal_digests(&eq_log(Some(&hex('a')), Some(&hex('b'))), EQ_SPEC).unwrap_err();
        assert!(e.contains("DIFFERENT digests"), "{e}");
        assert!(e.contains(&hex('a')) && e.contains(&hex('b')), "{e}");
    }

    #[test]
    fn a_missing_producer_line_fails_rather_than_passing_vacuously() {
        let e = judge_equal_digests(&eq_log(None, Some(&hex('b'))), EQ_SPEC).unwrap_err();
        // named as ABSENCE (the cause), not as "B before any A" (the symptom)
        assert!(
            e.contains("no line contains `attestd: record digest sha256:`"),
            "{e}"
        );
        // and with NEITHER side present at all
        let e = judge_equal_digests("just noise\n", EQ_SPEC).unwrap_err();
        assert!(
            e.contains("no line contains `attestd: record digest sha256:`"),
            "{e}"
        );
    }

    /// Both present, but the consumer printed first: nothing for it to equal at that point.
    #[test]
    fn a_consumer_line_before_the_producers_fails() {
        let log = format!(
            "diagnosticsd: last_attestation sha256:{}\nattestd: record digest sha256:{}\n",
            hex('a'),
            hex('a')
        );
        let e = judge_equal_digests(&log, EQ_SPEC).unwrap_err();
        assert!(e.contains("before any"), "{e}");
    }

    #[test]
    fn a_missing_consumer_line_fails_rather_than_passing_vacuously() {
        let e = judge_equal_digests(&eq_log(Some(&hex('a')), None), EQ_SPEC).unwrap_err();
        assert!(
            e.contains("no line contains `diagnosticsd: last_attestation sha256:`"),
            "{e}"
        );
    }

    #[test]
    fn malformed_zero_and_overlong_values_fail() {
        // short
        let e = judge_equal_digests(&eq_log(Some(&hex('a')), Some("abcd")), EQ_SPEC).unwrap_err();
        assert!(e.contains("carries 4 hex digits"), "{e}");
        // 65 hex digits: the value is not 64
        let long = format!("{}f", hex('a'));
        assert!(judge_equal_digests(&eq_log(Some(&hex('a')), Some(&long)), EQ_SPEC).is_err());
        // all zeros on both sides: "equal", and exactly E-076's failure
        let z = "0".repeat(64);
        let e = judge_equal_digests(&eq_log(Some(&z), Some(&z)), EQ_SPEC).unwrap_err();
        assert!(e.contains("all-zero"), "{e}");
        // a prefix with nothing after it
        assert!(judge_equal_digests(&eq_log(Some(&hex('a')), Some("")), EQ_SPEC).is_err());
    }

    /// The consumer reports the latest thing the producer printed: a second record
    /// supersedes the first, and a consumer line that matches only the OLD one is stale.
    #[test]
    fn the_consumer_must_equal_the_most_recent_producer_line_before_it() {
        let (a1, a2) = (hex('1'), hex('2'));
        let log = format!(
            "attestd: record digest sha256:{a1}\nattestd: record digest sha256:{a2}\n\
             diagnosticsd: last_attestation sha256:{a2}\n"
        );
        assert!(judge_equal_digests(&log, EQ_SPEC).is_ok());
        let stale = log.replace(
            &format!("last_attestation sha256:{a2}"),
            &format!("last_attestation sha256:{a1}"),
        );
        assert!(
            judge_equal_digests(&stale, EQ_SPEC)
                .unwrap_err()
                .contains("DIFFERENT")
        );
    }

    #[test]
    fn a_bad_spec_is_an_error_not_a_pass() {
        for bad in ["only one prefix", " => b", "a => ", "same => same"] {
            assert!(
                judge_equal_digests("a\nb\n", bad)
                    .unwrap_err()
                    .contains("bad expect_equal"),
                "{bad}"
            );
        }
    }

    // ── RFC-0.45-001 D3: expect_absent ─────────────────────────────────────────

    const RUN: &str =
        "boot\ndriver-virtio-net: no virtio-net device found (scanned 8 transports)\ndone\n";

    #[test]
    fn absent_passes_when_no_line_contains_the_text_and_names_what_it_checked() {
        let note = judge_absent(RUN, "driver-virtio-net: found").unwrap();
        assert!(
            note.contains("no line of 3 contains `driver-virtio-net: found`"),
            "{note}"
        );
    }

    #[test]
    fn absent_fails_on_the_first_offending_line_and_names_it() {
        let run =
            "boot\ndriver-virtio-net: found virtio-mmio device id=1 at slot 6 magic=0x74726976\n";
        let e = judge_absent(run, "driver-virtio-net: found").unwrap_err();
        assert!(
            e.contains("line 2") && e.contains("found virtio-mmio device id=1"),
            "{e}"
        );
    }

    #[test]
    fn absent_is_a_containment_test_not_a_prefix_test() {
        // A device fact printed after another prefix on the line still counts.
        let run = "[1.0] driver-virtio-net: found x\n";
        assert!(judge_absent(run, "driver-virtio-net: found").is_err());
        // And a near miss does not.
        assert!(
            judge_absent(
                "driver-virtio-net: no device found\n",
                "driver-virtio-net: found"
            )
            .is_ok()
        );
    }

    #[test]
    fn absent_refuses_an_empty_text_which_every_line_contains() {
        assert!(judge_absent(RUN, "").unwrap_err().contains("non-empty"));
        assert!(judge_absent(RUN, "   ").is_err());
    }

    #[test]
    fn absent_on_an_empty_run_passes_which_is_why_the_loader_demands_a_required_marker() {
        // The judge cannot tell "nothing happened" from "nothing wrong": that is the loader's
        // rule, below. This pins that the judge alone is vacuous on an empty run.
        assert!(judge_absent("", "driver-virtio-net: found").is_ok());
    }

    #[test]
    fn the_loader_rejects_an_absent_text_contained_in_its_own_required_marker() {
        let r = scratch_root(
            "ea-self",
            "name = \"p\"\nexpected_markers = [\"driver-virtio-net: found one\"]\nexpect_absent = [\"driver-virtio-net: found\"]\n",
        );
        let e = load_profile(&r, "p").unwrap_err();
        assert!(
            e.contains("contradicts itself") && e.contains("driver-virtio-net: found one"),
            "{e}"
        );
    }

    #[test]
    fn the_loader_rejects_expect_absent_with_no_required_marker() {
        let r = scratch_root("ea-vac", "name = \"p\"\nexpect_absent = [\"x: found\"]\n");
        let e = load_profile(&r, "p").unwrap_err();
        assert!(e.contains("never booted"), "{e}");
    }

    #[test]
    fn the_loader_accepts_a_sound_profile_and_it_is_opt_in() {
        assert!(Profile::smoke("m1", "x").expect_absent.is_empty());
        let r = scratch_root(
            "ea-ok",
            "name = \"p\"\nexpected_markers = [\"driver-virtio-net: no virtio-net device found\"]\nexpect_absent = [\"driver-virtio-net: found\"]\n",
        );
        assert_eq!(
            load_profile(&r, "p").unwrap().expect_absent,
            ["driver-virtio-net: found"]
        );
    }

    #[test]
    fn expect_equal_is_opt_in_and_loads_a_list() {
        assert!(Profile::smoke("m1", "x").expect_equal.is_empty());
        let r = scratch_root("ee1", "name = \"p\"\nexpect_equal = [\"a: => b:\"]\n");
        assert_eq!(load_profile(&r, "p").unwrap().expect_equal, ["a: => b:"]);
    }

    #[test]
    fn expect_scoped_is_opt_in_and_loads_a_list() {
        assert!(Profile::smoke("m1", "x").expect_scoped.is_empty());
        let r = scratch_root("es1", "name = \"p\"\nexpect_scoped = [\"k => number\"]\n");
        assert_eq!(
            load_profile(&r, "p").unwrap().expect_scoped,
            ["k => number"]
        );
    }

    #[test]
    fn expect_chain_head_is_opt_in_and_loads_a_list() {
        assert!(Profile::smoke("m1", "x").expect_chain_head.is_empty());
        let r = scratch_root("ech1", "name = \"p\"\nexpected_markers = [\"a\"]\n");
        assert!(load_profile(&r, "p").unwrap().expect_chain_head.is_empty());
        let r = scratch_root(
            "ech2",
            "name = \"p\"\nexpect_chain_head = [\"head: \", \"also: \"]\n",
        );
        let p = load_profile(&r, "p").unwrap();
        assert_eq!(p.expect_chain_head, ["head: ", "also: "]);
    }

    #[test]
    fn judge_hex_prefixes_demands_equality_not_presence() {
        let want = "ab".repeat(32);
        let mut rec = String::new();
        // Equal: passes.
        assert!(judge_hex_prefixes(
            &format!("noise\nhead: {want}\n"),
            &["head: ".to_string()],
            &want,
            "chain head",
            "over x",
            "d",
            &mut rec
        ));
        // Present and well-formed but different: fails.
        let other = "cd".repeat(32);
        assert!(!judge_hex_prefixes(
            &format!("head: {other}\n"),
            &["head: ".to_string()],
            &want,
            "chain head",
            "over x",
            "d",
            &mut rec
        ));
        // Equal followed by more hex (a longer value): fails.
        assert!(!judge_hex_prefixes(
            &format!("head: {want}ff\n"),
            &["head: ".to_string()],
            &want,
            "chain head",
            "over x",
            "d",
            &mut rec
        ));
        // Absent: fails.
        assert!(!judge_hex_prefixes(
            "nothing here\n",
            &["head: ".to_string()],
            &want,
            "chain head",
            "over x",
            "d",
            &mut rec
        ));
    }

    // ── RFC-0.38-001 R6: expect_image_set_digest ────────────────────────────

    #[test]
    fn expect_image_set_digest_is_opt_in_and_loads_a_list() {
        let r = scratch_root("eisd1", "name = \"p\"\nexpected_markers = [\"a\"]\n");
        assert!(
            load_profile(&r, "p")
                .unwrap()
                .expect_image_set_digest
                .is_empty()
        );
        let r = scratch_root(
            "eisd2",
            "name = \"p\"\nexpect_image_set_digest = [\"one: \", \"two, sha256:\"]\n",
        );
        let p = load_profile(&r, "p").unwrap();
        assert_eq!(p.expect_image_set_digest, ["one: ", "two, sha256:"]);
    }

    #[test]
    fn an_empty_digest_prefix_is_refused() {
        // The array reader refuses an empty entry for every key: it would match the
        // digest anywhere in the log.
        let r = scratch_root(
            "eisd3",
            "name = \"p\"\nexpect_image_set_digest = [\"ok: \", \"\"]\n",
        );
        let e = load_profile(&r, "p").unwrap_err();
        assert!(
            e.contains("expect_image_set_digest") && e.contains("empty marker"),
            "{e}"
        );
    }

    #[test]
    fn a_digest_expectation_alone_is_not_a_placeholder_profile() {
        // `expected_markers` empty must not make the profile pass vacuously when
        // it states a digest to be checked: it loads with that expectation intact.
        let r = scratch_root(
            "eisd4",
            "name = \"p\"\nexpect_image_set_digest = [\"d: \"]\n",
        );
        let p = load_profile(&r, "p").unwrap();
        assert!(p.expected_markers.is_empty());
        assert_eq!(p.expect_image_set_digest.len(), 1);
    }

    #[test]
    fn clean_console_defaults_false_and_semantic_opts_in() {
        let r = scratch_root("cc", "name = \"p\"\nexpected_markers = [\"a\"]\n");
        assert!(
            !load_profile(&r, "p").unwrap().clean_console,
            "clean_console is opt-in"
        );
        let semantic = load_profile(&test_workspace_root(), "semantic").unwrap();
        assert!(
            semantic.clean_console,
            "semantic.toml should opt into the clean-console check"
        );
        let health_fail = load_profile(&test_workspace_root(), "health-fail").unwrap();
        assert!(!health_fail.clean_console, "health-fail did not opt in");
    }
}

/// RFC-0.33-001: the `expect_shutdown` key.
#[cfg(test)]
mod expect_shutdown_tests {
    use super::*;

    /// A throwaway workspace root holding one profile, so the loader is
    /// exercised on real files without touching `tests/qemu/profiles/`.
    fn root_with(profile: &str, body: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "qemu_expect_shutdown_{}_{profile}",
            std::process::id()
        ));
        let dir = root.join("tests/qemu/profiles");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{profile}.toml")), body).unwrap();
        root
    }

    #[test]
    fn a_profile_may_expect_a_reset() {
        let root = root_with(
            "reset-ok",
            "name = \"reset-ok\"\nexpect_shutdown = \"guest-reset\"\nexpected_markers = [\n  \"a\",\n]\n",
        );
        let p = load_profile(&root, "reset-ok").expect("loads");
        assert_eq!(p.expect_shutdown.as_deref(), Some("guest-reset"));
    }

    /// The default: every existing profile keeps today's behaviour exactly.
    #[test]
    fn a_profile_that_says_nothing_expects_nothing() {
        let root = root_with(
            "plain",
            "name = \"plain\"\nexpected_markers = [\n  \"a\",\n]\n",
        );
        assert_eq!(load_profile(&root, "plain").unwrap().expect_shutdown, None);
        assert_eq!(Profile::smoke("m8", "TEST:M8:PASS").expect_shutdown, None);
    }

    /// A misspelt *value* must fail at load time. The loader ignores unknown
    /// *keys* for forward compatibility, so without this a typo would quietly
    /// become "no expectation" and the profile would pass without checking
    /// the ending at all.
    #[test]
    fn a_misspelt_expectation_is_refused() {
        let root = root_with(
            "typo",
            "name = \"typo\"\nexpect_shutdown = \"guest-rest\"\nexpected_markers = [\n  \"a\",\n]\n",
        );
        let err = load_profile(&root, "typo").err().expect("must refuse");
        assert!(err.contains("guest-rest"), "{err}");
    }

    #[test]
    fn host_signal_cannot_be_expected() {
        let root = root_with(
            "hostsig",
            "name = \"hostsig\"\nexpect_shutdown = \"host-signal\"\nexpected_markers = [\n  \"a\",\n]\n",
        );
        assert!(load_profile(&root, "hostsig").is_err());
    }
}
