#!/usr/bin/env bash
# RFC-0.32-004 D4/D5 — check every tracked Cargo.lock against the RustSec
# advisory database, and say exactly what that covers.
#
# Run by CI (ci-dependency-advisories) and runnable locally, identically:
#
#   cargo install cargo-audit --locked --version 0.22.2
#   .github/scripts/dependency-advisories.sh
#
# Exit status is the result, and three outcomes are kept distinct because a
# release cut treats them differently (docs/src/releasing/v0-release-cycle.md):
#   0  every lockfile read, no vulnerability and no unsoundness advisory
#   1  at least one finding — each is an erratum like any other
#   2  the check could not run: tool missing, database unreachable, or the
#      surface statement below could not be verified. Never reported as a pass.
set -uo pipefail

PINNED_AUDIT="0.22.2"

# The newest a run's advisory database may be older than, in days. The same
# bound the release cycle applies to a cut.
#
# This is not belt-and-braces. Pointed at an unreachable database,
# cargo-audit 0.22.2 exits 0, prints nothing to stderr, and reports from its
# local cache in ~/.cargo/advisory-db — with a real commit id, so the report
# looks exactly like a fresh one. On a maintainer's machine that cache can be
# months old. The database's own last-updated date is the only thing in the
# report that tells the two apart, so it is checked here, not trusted.
MAX_DB_AGE_DAYS=7
PUBLISHED_CRATES=(fjell-os fjell-abi)

cd "$(git rev-parse --show-toplevel)" || exit 2

say() { printf '%s\n' "$*"; }
cannot_run() { say "dependency-advisories: CANNOT RUN — $*"; exit 2; }

command -v cargo-audit >/dev/null || cannot_run "cargo-audit is not installed; see the header of this script"
command -v jq >/dev/null || cannot_run "jq is not installed"
have=$(cargo audit --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
[ "$have" = "$PINNED_AUDIT" ] || cannot_run "cargo-audit is $have, but this check is pinned to $PINNED_AUDIT"

say "dependency-advisories: cargo-audit $have"

# ── D5 (2): the surface statement, derived rather than asserted ──────────────
# A green run is not a claim about what Fjell OS ships, and the report says so
# only after checking it is true today. If a published crate ever gains a
# third-party dependency, this stops being printable, and the run says so.
say ""
say "What this covers"
for crate in "${PUBLISHED_CRATES[@]}"; do
  tree=$(cargo tree -p "$crate" -e normal --prefix none 2>/dev/null) || cannot_run "cargo tree -p $crate failed"
  [ -n "$tree" ] || cannot_run "cargo tree -p $crate printed nothing"
  third=$(printf '%s\n' "$tree" | grep -v '^fjell-' || true)
  if [ -n "$third" ]; then
    say "  $crate HAS third-party dependencies — the statement below would be false:"
    printf '%s\n' "$third" | sed 's/^/      /'
    exit 2
  fi
done
say "  The published crates (${PUBLISHED_CRATES[*]}) have no third-party dependencies:"
say "    cargo tree -p fjell-os -e normal   ->  fjell-os, fjell-abi"
say "    cargo tree -p fjell-abi -e normal  ->  fjell-abi"
say "  So this is NOT a statement about what Fjell OS ships. It covers the build and"
say "  host surface: tools, tests, benchmarks and the development-grade crypto crate."

# ── every tracked lockfile, derived rather than listed ──────────────────────
mapfile -t lockfiles < <(git ls-files | grep -E '(^|/)Cargo\.lock$')
[ "${#lockfiles[@]}" -gt 0 ] || cannot_run "git ls-files lists no Cargo.lock"

findings=0
db_seen=""
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

for lock in "${lockfiles[@]}"; do
  say ""
  # --deny unsound: this project removed an unsound decode path in RFC-0.32-002,
  # and an unsoundness advisory is a finding here, not a warning.
  cargo audit --file "$lock" --json --deny unsound >"$tmp" 2>/dev/null
  if ! jq -e '.database["last-commit"]' "$tmp" >/dev/null 2>&1; then
    cannot_run "no usable report for $lock — most often the advisory database is unreachable"
  fi

  # ── the database is fresh, or this run says nothing ──
  updated=$(jq -r '.database["last-updated"]' "$tmp")
  updated_s=$(date -d "$updated" +%s 2>/dev/null) || cannot_run "unreadable database date '$updated' for $lock"
  age_days=$(( ( $(date +%s) - updated_s ) / 86400 ))
  if [ "$age_days" -gt "$MAX_DB_AGE_DAYS" ]; then
    cannot_run "the advisory database read for $lock was last updated $updated, $age_days days ago (limit $MAX_DB_AGE_DAYS). cargo-audit falls back to its local cache without saying so when the database cannot be fetched — so this run cannot tell a clean result from a stale one"
  fi

  # ── D5 (1) and (3): packages covered, database identity ──
  pkgs=$(jq -r '.lockfile["dependency-count"]' "$tmp")
  db=$(jq -r '"\(.database["last-commit"]) (updated \(.database["last-updated"]), \(.database["advisory-count"]) advisories)"' "$tmp")
  db="$db, $age_days day(s) old"
  say "$lock: $pkgs packages checked"
  if [ "$db" != "$db_seen" ]; then say "  advisory database: $db"; db_seen="$db"; fi

  n=$(jq '[.vulnerabilities.list[]] + [(.warnings.unsound // [])[]] | length' "$tmp")
  if [ "$n" -eq 0 ]; then
    say "  no vulnerability, no unsoundness advisory"
  else
    findings=$((findings + n))
    jq -r '
      ( .vulnerabilities.list[]      | "  VULNERABLE  \(.advisory.id)  \(.package.name) \(.package.version)  patched: \(.versions.patched | join(" "))  — \(.advisory.title)" ),
      ( (.warnings.unsound // [])[]  | "  UNSOUND     \(.advisory.id)  \(.package.name) \(.package.version)  patched: \((.versions.patched // []) | join(" "))  — \(.advisory.title)" )
    ' "$tmp"
  fi
  # informational only: reported, never the reason a run fails
  jq -r '(.warnings.unmaintained // [])[] | "  (unmaintained, informational)  \(.advisory.id)  \(.package.name) \(.package.version)"' "$tmp"
done

say ""
if [ "$findings" -gt 0 ]; then
  say "dependency-advisories: FAIL — $findings finding(s). Each is an erratum; see docs/src/security/advisory-process.md §7."
  exit 1
fi
say "dependency-advisories: PASS — ${#lockfiles[@]} lockfile(s), no vulnerability and no unsoundness advisory."
exit 0
