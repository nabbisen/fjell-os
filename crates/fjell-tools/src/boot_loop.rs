//! RFC-0.41-006 R5: a boot-level repro harness.
//!
//! `cargo xtask boot-loop [--boots N] [--jobs J] [--drive-extra STR] [--kernel PATH] [--out DIR]`
//!
//! Boots the kernel image `N` times, `J` at a time, each on its own fresh 16 MiB disk, and
//! stops each boot at the point `init`'s M6 section has **passed** or **failed** -- not
//! after a fixed profile duration. A profile boot costs about a minute because it runs for
//! its whole timeout; this costs about a twentieth of a second, so thousands of boots fit in
//! the time one `test-all` takes.
//!
//! **It is an instrument, and an instrument must be shown to see the thing it measures.**
//! E-104 was seen at roughly one suite in ten. This harness has *not* reproduced that rate
//! on the unfixed tree (see the RFC-0.41-006 review request for the runs and what they
//! rule out), so a clean count from it is **not** evidence that a fix works. **Since
//! 2026-11-03 that limitation has a reason**: E-104's cause is a host write-latency outlier,
//! and the latency degrades with accumulated host state, so failures cluster. Boots of a
//! twentieth of a second never accumulate any — which is exactly why ~25,000 of them caught
//! nothing while a long sequence of full suites caught three in one day. It does
//! reproduce the failure's signature on demand when the disk is made slow enough
//! (`--drive-extra ,throttling.bps-total=50`), which is what makes it a harness for the
//! *mechanism* and not yet one for the *rate*.
//!
//! The classifier ([`classify`]) is a pure function of the console text, with its own tests.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// `init` prints this when its M6 section (device, storaged, the six block writes, the
/// upgrade foundation) has finished.
const M6_DONE: &str = "M6: persistent store and upgrade foundation ready";
/// `init`'s own message at each of the guarded block writes; `sys_exit(1)` follows.
const M6_ERROR: &str = "M6: block I/O error";
/// What the kernel prints when `init` exits non-zero.
const M7_FAIL: &str = "TEST:M7:FAIL";

/// How one boot ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `init` finished M6.
    Pass,
    /// `init` reported a block I/O error or the kernel reported the unclean exit.
    Fail,
    /// QEMU's output ended before either (it exited, or rejected an option).
    Eof,
    /// Neither happened within the deadline.
    Timeout,
}

/// Judge a boot from the console text so far. `None` means undecided.
pub fn classify(text: &str) -> Option<Verdict> {
    if text.contains(M6_DONE) {
        return Some(Verdict::Pass);
    }
    if text.contains(M6_ERROR) || text.contains(M7_FAIL) {
        return Some(Verdict::Fail);
    }
    None
}

struct Opts {
    boots: usize,
    jobs: usize,
    drive_extra: String,
    kernel: PathBuf,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        boots: 200,
        jobs: 8,
        drive_extra: String::new(),
        kernel: PathBuf::from("target/riscv64gc-unknown-none-elf/release/fjell-kernel"),
        out: PathBuf::from("target/boot-loop"),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} needs a value"));
        match a.as_str() {
            "--boots" => {
                o.boots = val("--boots")?
                    .parse()
                    .map_err(|e| format!("--boots: {e}"))?
            }
            "--jobs" => o.jobs = val("--jobs")?.parse().map_err(|e| format!("--jobs: {e}"))?,
            "--drive-extra" => o.drive_extra = val("--drive-extra")?,
            "--kernel" => o.kernel = PathBuf::from(val("--kernel")?),
            "--out" => o.out = PathBuf::from(val("--out")?),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    if o.jobs == 0 || o.boots == 0 {
        return Err("--boots and --jobs must be at least 1".into());
    }
    Ok(o)
}

/// Boot once. Returns the verdict and the console text.
fn boot(opts: &Opts, id: usize) -> (Verdict, String) {
    let dir = opts.out.join(format!("b{}-{id}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let disk = dir.join("disk.img");
    let made = std::fs::File::create(&disk).and_then(|f| f.set_len(16 * 1024 * 1024));
    if let Err(e) = made {
        return (
            Verdict::Eof,
            format!("could not create {}: {e}", disk.display()),
        );
    }
    let drive = format!(
        "file={},format=raw,if=none,id=hd0{}",
        disk.display(),
        opts.drive_extra
    );
    let (mut reader, writer) = match std::io::pipe() {
        Ok(p) => p,
        Err(e) => return (Verdict::Eof, format!("pipe: {e}")),
    };
    let writer2 = match writer.try_clone() {
        Ok(w) => w,
        Err(e) => return (Verdict::Eof, format!("pipe: {e}")),
    };
    let child = Command::new("qemu-system-riscv64")
        .args(["-machine", "virt", "-bios", "none", "-nographic", "-kernel"])
        .arg(&opts.kernel)
        .args(["-drive", &drive, "-device", "virtio-blk-device,drive=hd0"])
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(writer2)
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (Verdict::Eof, format!("spawn qemu-system-riscv64: {e}")),
    };
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let t = thread::spawn(move || {
        let mut buf = [0u8; 65536];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let start = Instant::now();
    let deadline = Duration::from_secs(30);
    let mut bytes: Vec<u8> = Vec::new();
    let mut verdict = Verdict::Timeout;
    while start.elapsed() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => {
                bytes.extend_from_slice(&chunk);
                if let Some(v) = classify(&String::from_utf8_lossy(&bytes)) {
                    verdict = v;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                verdict = Verdict::Eof;
                break;
            }
        }
    }
    if verdict == Verdict::Fail {
        // Keep reading a moment, so the records around the failure are in the log.
        let until = Instant::now() + Duration::from_secs(1);
        while Instant::now() < until {
            if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(200)) {
                bytes.extend_from_slice(&chunk);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = t.join();
    let _ = std::fs::remove_dir_all(&dir);
    (verdict, String::from_utf8_lossy(&bytes).into_owned())
}

pub fn cmd_boot_loop(args: &[String]) -> ExitCode {
    let opts = match parse(args) {
        Ok(o) => Arc::new(o),
        Err(e) => {
            eprintln!(
                "boot-loop: {e}\nUsage: cargo xtask boot-loop [--boots N] [--jobs J] \
                 [--drive-extra STR] [--kernel PATH] [--out DIR]"
            );
            return ExitCode::FAILURE;
        }
    };
    if !opts.kernel.exists() {
        eprintln!(
            "boot-loop: no kernel at {} (run `cargo xtask build` first)",
            opts.kernel.display()
        );
        return ExitCode::FAILURE;
    }
    let _ = std::fs::create_dir_all(&opts.out);
    let next = Arc::new(AtomicUsize::new(0));
    let tally = Arc::new(Mutex::new([0usize; 4])); // pass, fail, eof, timeout
    let started = Instant::now();
    let workers: Vec<_> = (0..opts.jobs.min(opts.boots))
        .map(|_| {
            let (opts, next, tally) = (Arc::clone(&opts), Arc::clone(&next), Arc::clone(&tally));
            thread::spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::SeqCst);
                    if i >= opts.boots {
                        break;
                    }
                    let (v, text) = boot(&opts, i);
                    if v != Verdict::Pass {
                        let name = format!("fail-{i}-{v:?}.log");
                        if let Ok(mut f) = std::fs::File::create(opts.out.join(name)) {
                            let _ = f.write_all(text.as_bytes());
                        }
                    }
                    let k = match v {
                        Verdict::Pass => 0,
                        Verdict::Fail => 1,
                        Verdict::Eof => 2,
                        Verdict::Timeout => 3,
                    };
                    tally.lock().unwrap()[k] += 1;
                }
            })
        })
        .collect();
    for w in workers {
        let _ = w.join();
    }
    let t = *tally.lock().unwrap();
    println!(
        "boot-loop: {} boots, {} at a time, {:.1}s: pass {} / fail {} / eof {} / timeout {}",
        opts.boots,
        opts.jobs,
        started.elapsed().as_secs_f64(),
        t[0],
        t[1],
        t[2],
        t[3]
    );
    if t[1] + t[2] + t[3] > 0 {
        println!(
            "boot-loop: console of each non-passing boot is in {}/fail-<n>-<verdict>.log",
            opts.out.display()
        );
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn m6_done_is_a_pass() {
        assert_eq!(
            classify("a\nM6: persistent store and upgrade foundation ready\n"),
            Some(Verdict::Pass)
        );
    }

    #[test]
    fn a_block_error_or_the_unclean_exit_is_a_fail() {
        assert_eq!(
            classify("M6: block write ok\nM6: block I/O error\n"),
            Some(Verdict::Fail)
        );
        assert_eq!(
            classify("TEST:M7:FAIL (init did not exit cleanly)"),
            Some(Verdict::Fail)
        );
    }

    #[test]
    fn undecided_until_one_of_them_prints() {
        assert_eq!(classify("M6: storaged ready\nM6: block write ok\n"), None);
        assert_eq!(classify(""), None);
    }

    #[test]
    fn the_real_failure_signature_is_a_fail_and_a_real_pass_is_a_pass() {
        let failing = "[task#15 svc-fault]: fault(LoadPageFault) sepc=0x400aa stval=0x0\n\
                       M6: block I/O error\nTEST:V0.5-PLATFORM:PASS\nTEST:M7:FAIL (init did not exit cleanly)\n";
        assert_eq!(classify(failing), Some(Verdict::Fail));
        let passing = "[task#21 svc-fault]: fault(LoadPageFault) sepc=0x400aa stval=0x0\n\
                       M6: persistent store and upgrade foundation ready\n";
        assert_eq!(classify(passing), Some(Verdict::Pass));
    }

    #[test]
    fn arguments_are_parsed_and_bad_ones_refused() {
        let a: Vec<String> = ["--boots", "5", "--jobs", "2", "--drive-extra", ",x=1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let o = parse(&a).unwrap();
        assert_eq!((o.boots, o.jobs, o.drive_extra.as_str()), (5, 2, ",x=1"));
        assert!(parse(&["--boots".into()]).is_err());
        assert!(parse(&["--jobs".into(), "0".into()]).is_err());
        assert!(parse(&["--nope".into()]).is_err());
    }
}
