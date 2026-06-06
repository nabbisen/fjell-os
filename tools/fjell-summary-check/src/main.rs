//! `fjell-summary-check` — semantic summary consistency checker.
//!
//! Used by the fleet coordinator to validate each inbound
//! `MeasurementSummary` before accepting it into the authoritative
//! fleet state (RFC-v0.13-005 §3).
//!
//! Exit codes:
//!   0 — all checks passed
//!   1 — one or more consistency violations found
//!   2 — argument or input error

use fjell_fleet_sync::{check_summary_consistency, ConsistencyError};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help") {
        print_usage();
        return ExitCode::SUCCESS;
    }

    // In v0.13.0 the checker is called from the coordinator's IPC handler.
    // The CLI is a test / diagnostic entry point.
    // Usage: fjell-summary-check --seq N --epoch E --boot B --lifecycle L
    //   [--prev-seq N] [--prev-epoch E] [--prev-boot B] [--prev-lifecycle L]
    //   [--bundle-digest <hex32>] [--known-bundle <hex32>]...

    let new_seq    = parse_u64(&args, "--seq")      .unwrap_or(1);
    let new_epoch  = parse_u32(&args, "--epoch")    .unwrap_or(1);
    let new_boot   = parse_u32(&args, "--boot")     .unwrap_or(1);
    let new_lc     = parse_u8(&args,  "--lifecycle").unwrap_or(4);

    let prev_seq   = parse_u64(&args, "--prev-seq")       .unwrap_or(0);
    let prev_epoch = parse_u32(&args, "--prev-epoch")     .unwrap_or(0);
    let prev_boot  = parse_u32(&args, "--prev-boot")      .unwrap_or(0);
    let prev_lc    = parse_u8(&args,  "--prev-lifecycle") .unwrap_or(0);

    let bundle_digest = parse_hex32(&args, "--bundle-digest")
        .unwrap_or([0u8; 32]);
    let known_bundles: Vec<[u8; 32]> = collect_hex32(&args, "--known-bundle");

    let errors = check_summary_consistency(
        new_seq, new_epoch, new_boot, new_lc,
        prev_seq, prev_epoch, prev_boot, prev_lc,
        bundle_digest,
        &known_bundles,
    );

    if errors.is_empty() {
        println!("summary-check: PASS");
        return ExitCode::SUCCESS;
    }

    eprintln!("summary-check: FAIL — {} error(s):", errors.len());
    for e in &errors {
        eprintln!("  {:?}", e);
    }
    ExitCode::FAILURE
}

fn print_usage() {
    println!("Usage: fjell-summary-check [options]");
    println!("  --seq N            new sync_seq");
    println!("  --epoch E          new key_epoch");
    println!("  --boot B           new boot_count");
    println!("  --lifecycle L      new bundle lifecycle (1-6)");
    println!("  --prev-*           previous values (same names)");
    println!("  --bundle-digest X  hex32 bundle_digest");
    println!("  --known-bundle X   hex32 (repeatable) known bundle digests");
}

fn flag_val<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].as_str())
}
fn parse_u64(args: &[String], flag: &str) -> Option<u64> {
    flag_val(args, flag)?.parse().ok()
}
fn parse_u32(args: &[String], flag: &str) -> Option<u32> {
    flag_val(args, flag)?.parse().ok()
}
fn parse_u8(args: &[String], flag: &str) -> Option<u8> {
    flag_val(args, flag)?.parse().ok()
}
fn parse_hex32(args: &[String], flag: &str) -> Option<[u8; 32]> {
    let s = flag_val(args, flag)?;
    if s.len() != 64 { return None; }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i*2..i*2+2], 16).ok()?;
    }
    Some(out)
}
fn collect_hex32(args: &[String], flag: &str) -> Vec<[u8; 32]> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            if let Some(v) = args.get(i+1) {
                if v.len() == 64 {
                    let mut buf = [0u8; 32];
                    let ok = buf.iter_mut().enumerate().all(|(j, b)| {
                        u8::from_str_radix(&v[j*2..j*2+2], 16).ok().map(|x| *b = x).is_some()
                    });
                    if ok { out.push(buf); }
                }
                i += 2; continue;
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex32_works() {
        let hex = "ab".repeat(32);
        let args = vec!["--bundle-digest".to_string(), hex.clone()];
        let result = parse_hex32(&args, "--bundle-digest");
        assert_eq!(result, Some([0xABu8; 32]));
    }

    #[test]
    fn collect_hex32_multiple() {
        let hex1 = "ab".repeat(32);
        let hex2 = "cd".repeat(32);
        let args = vec![
            "--known-bundle".to_string(), hex1,
            "--known-bundle".to_string(), hex2,
        ];
        let result = collect_hex32(&args, "--known-bundle");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], [0xABu8; 32]);
        assert_eq!(result[1], [0xCDu8; 32]);
    }
}
