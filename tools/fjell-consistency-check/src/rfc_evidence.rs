//! `rfc-evidence` -- an RFC that says `Implemented` shows that its mechanism runs
//! (RFC-0.45-003).
//!
//! RFC 000 gives `Implemented` a truth condition -- *the work has shipped* -- and nothing
//! checked it. `rfc-status-folder` compares a status string with the **folder** it sits in; it has
//! never compared a status with evidence, because there was no evidence to compare against. Three
//! RFCs in `rfcs/done/` assert `Implemented` where the artefact does not run (056: `sys_cap_install`
//! had no dispatch arm, E-123; `RFC-v0.7.3-001`: the handshake is an unconditional `Err`;
//! `RFC-v0.7.3-002`: neither specified document exists, E-028), each found by a person reading.
//!
//! **The constraint that outranks the feature.** You cannot derive "the mechanism" from an RFC's
//! prose, and a gate that pretended to would be worse than none: scanning for the syscalls and
//! markers an RFC *mentions* reports coverage it does not have -- E-014's class, the defect this
//! closes. So:
//!
//! 1. **Evidence is cited, not inferred.** An RFC that claims `Implemented` carries one header line,
//!    `**Evidence:** <citation>, <citation>, ...`, and this check verifies each citation
//!    **resolves**.
//! 2. **The output states its own limits**, every run, in words ([`render_pass`]): *resolving* is
//!    not *sufficient*, the pre-0.45 RFCs are not covered, and the count is broken down by kind so a
//!    reader sees how much rests on `test:`, the weakest.
//! 3. **Forward-only.** The field is required of RFCs from 0.45 on. Older RFCs are counted and
//!    reported as declaring none -- never failed. Backfilling 229 is not this check's job.
//!
//! # The four kinds, and what each resolves against
//!
//! | Citation | Resolves when | Resolved against |
//! |---|---|---|
//! | `syscall:<Name>` | `SyscallNumber::<Name>` is declared **and not in `undispatched`** | `tests/syscall/expected.toml` (the file `syscall-surface` holds equal to the source) and the ABI enum, **through the same two parsers** (`syscall_table`) -- one rule, not a second parser |
//! | `marker:<profile>:<MARKER>` | the marker is a line of that profile's rendering | `tests/qemu/artifacts/<profile>/expected-markers.txt`, the tracked, derived rendering (a profile with none has no gated markers) |
//! | `file:<path>` | the path is a non-empty file | the working tree |
//! | `test:<path>::<name>` | `<path>` holds `fn <name>(` preceded by `#[test]` | the file's text. **The weakest kind**, named so |
//!
//! Anything else -- an unknown kind, a malformed citation, an empty value, a path that leaves the
//! repository -- is a **failure naming it**, never a skip.
//!
//! # Where the field lives
//!
//! In the **header block**: the lines before the first `---` line or the first `## ` heading,
//! whichever comes first. An RFC that writes the field as prose in its body has no field; the failure
//! says where it looks. One line, comma-separated; each citation may be wrapped in backticks.
//!
//! # Two lists, and why they are two
//!
//! [`KNOWN_FALSE`] holds the statuses known to be false whose disposition is **the owner's pending
//! lifecycle decision** (RFC 000's five states offer nothing true for "shipped except for the
//! load-bearing part"). [`WAIVERS`] holds an **implementer's** judgement inside the gate. Provenance
//! is the point: the output says who owes the resolution, and the three cannot be quietly
//! reclassified as ordinary waivers. Neither list touches a status line.

use crate::status::extract_status_keyword;
use crate::syscall_table::{Expected, parse_declared, parse_expected};
use std::path::{Component, Path, PathBuf};

pub const NAME: &str = "rfc-evidence";

/// The first milestone whose `Implemented` RFCs must carry evidence.
pub const FIRST_REQUIRED: (u32, u32) = (0, 45);

pub const ABI_SYSCALL_PATH: &str = "crates/fjell-abi/src/syscall.rs";
pub const EXPECTED_PATH: &str = "tests/syscall/expected.toml";
pub const MARKERS_DIR: &str = "tests/qemu/artifacts";

// ── The two lists ─────────────────────────────────────────────────────────────

/// A status known to be false, whose disposition is **the owner's**. Reported by name on every
/// run; **its RFC's status line is not touched**.
pub struct KnownFalse {
    pub id: &'static str,
    pub reason: &'static str,
}

/// An implementer's waiver: a `done/` RFC exempt from the requirement, with a reason, printed on
/// every run. **Empty today, on purpose** -- the three known cases are not waivers (see module doc).
pub struct Waiver {
    pub id: &'static str,
    pub reason: &'static str,
}

/// RFC 056 was the third, until RFC-0.45-002 made `sys_cap_install` real; its status is now true
/// and it is on neither list (the order taken: 0.45-002 first).
pub const KNOWN_FALSE: &[KnownFalse] = &[
    KnownFalse {
        id: "RFC-v0.7.3-001",
        reason: "`Implemented (v0.7.2)`, but the handshake is an unconditional `Err` and its \
                 step 1 reads a `sys_net_recv` that exists nowhere",
    },
    KnownFalse {
        id: "RFC-v0.7.3-002",
        reason: "`Implemented (v0.7.1)`, but neither specified document has ever existed (E-028)",
    },
];

pub const WAIVERS: &[Waiver] = &[];

// ── Citations ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Syscall,
    Marker,
    File,
    Test,
}

impl Kind {
    pub fn word(self) -> &'static str {
        match self {
            Kind::Syscall => "syscall",
            Kind::Marker => "marker",
            Kind::File => "file",
            Kind::Test => "test",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Citation {
    Syscall(String),
    Marker { profile: String, marker: String },
    File(String),
    Test { path: String, name: String },
}

impl Citation {
    pub fn kind(&self) -> Kind {
        match self {
            Citation::Syscall(_) => Kind::Syscall,
            Citation::Marker { .. } => Kind::Marker,
            Citation::File(_) => Kind::File,
            Citation::Test { .. } => Kind::Test,
        }
    }

    pub fn render(&self) -> String {
        match self {
            Citation::Syscall(n) => format!("syscall:{n}"),
            Citation::Marker { profile, marker } => format!("marker:{profile}:{marker}"),
            Citation::File(p) => format!("file:{p}"),
            Citation::Test { path, name } => format!("test:{path}::{name}"),
        }
    }
}

/// Parse one citation (already trimmed, backticks removed).
pub fn parse_citation(s: &str) -> Result<Citation, String> {
    let Some((kind, rest)) = s.split_once(':') else {
        return Err(format!(
            "`{s}` is not `kind:value` (kinds: syscall, marker, file, test)"
        ));
    };
    if rest.trim().is_empty() {
        return Err(format!("`{s}` has an empty value"));
    }
    match kind {
        "syscall" => {
            let ok = rest.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                && rest.chars().all(|c| c.is_ascii_alphanumeric());
            if ok {
                Ok(Citation::Syscall(rest.to_string()))
            } else {
                Err(format!(
                    "`{s}`: a syscall name is a `SyscallNumber` variant (`CapInstall`), nothing else"
                ))
            }
        }
        "marker" => match rest.split_once(':') {
            Some((profile, marker)) if !profile.is_empty() && !marker.trim().is_empty() => {
                Ok(Citation::Marker {
                    profile: profile.to_string(),
                    marker: marker.to_string(),
                })
            }
            _ => Err(format!(
                "`{s}`: a marker citation is `marker:<profile>:<MARKER>`"
            )),
        },
        "file" => Ok(Citation::File(rest.to_string())),
        "test" => match rest.split_once("::") {
            Some((path, name)) if !path.is_empty() && is_ident(name) => Ok(Citation::Test {
                path: path.to_string(),
                name: name.to_string(),
            }),
            _ => Err(format!(
                "`{s}`: a test citation is `test:<path>::<fn_name>`"
            )),
        },
        other => Err(format!(
            "`{s}`: unknown citation kind `{other}` (kinds: syscall, marker, file, test)"
        )),
    }
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Every citation in an `**Evidence:**` value; **every** malformed one is named.
pub fn parse_citations(field: &str) -> Result<Vec<Citation>, Vec<String>> {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    let items: Vec<&str> = field.split(',').map(str::trim).collect();
    if items.iter().all(|i| i.is_empty()) {
        return Err(vec![
            "the `**Evidence:**` line names no citation (it is empty)".to_string(),
        ]);
    }
    for item in items {
        let item = item
            .strip_prefix('`')
            .and_then(|s| s.strip_suffix('`'))
            .unwrap_or(item);
        if item.is_empty() {
            bad.push("an empty citation (a stray comma)".to_string());
            continue;
        }
        match parse_citation(item) {
            Ok(c) => ok.push(c),
            Err(e) => bad.push(e),
        }
    }
    if bad.is_empty() { Ok(ok) } else { Err(bad) }
}

// ── The header block ──────────────────────────────────────────────────────────

/// The lines before the first `---` line or `## ` heading.
pub fn header_block(src: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for line in src.lines() {
        if line.trim() == "---" || line.starts_with("## ") {
            break;
        }
        out.push(line);
    }
    out
}

/// The value of the `**Evidence:**` field, if there is exactly one such header line.
pub fn evidence_field(src: &str) -> Result<Option<String>, String> {
    let mut found: Vec<String> = header_block(src)
        .into_iter()
        .filter_map(|l| {
            l.trim_start()
                .strip_prefix("**Evidence:**")
                .map(|v| v.trim().to_string())
        })
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        n => Err(format!(
            "the header block carries {n} `**Evidence:**` lines; there must be one"
        )),
    }
}

// ── RFC identity ──────────────────────────────────────────────────────────────

/// `RFC-0.45-003-slug.md` -> `RFC-0.45-003`; `RFC-v0.7.3-001-slug.md` -> `RFC-v0.7.3-001`;
/// `056-slug.md` -> `056`.
pub fn rfc_id(file_name: &str) -> String {
    let stem = file_name.trim_end_matches(".md");
    if stem.starts_with("RFC-") {
        stem.splitn(4, '-').take(3).collect::<Vec<_>>().join("-")
    } else {
        stem.split('-').next().unwrap_or(stem).to_string()
    }
}

/// `RFC-<major>.<minor>-NNN-...` -> `(major, minor)`, compared numerically. The older
/// `RFC-v0.7.3-001` / `RFC-v0.22-001` spellings and the bare `056-` ones carry no such id: they
/// are **pre-0.45** by construction (the `v` form ended before this milestone).
pub fn milestone(file_name: &str) -> Option<(u32, u32)> {
    let rest = file_name.strip_prefix("RFC-")?;
    let ms = rest.split('-').next()?;
    let (a, b) = ms.split_once('.')?;
    if b.contains('.') {
        return None;
    }
    Some((a.parse().ok()?, b.parse().ok()?))
}

pub fn is_implemented(status: Option<&str>) -> bool {
    matches!(
        status,
        Some("Implemented") | Some("Implemented-with-Errata")
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Proposed,
    Accepted,
    Done,
}

/// Whether the RFC must carry evidence: in `done/`, `Implemented*`, and 0.45 or later.
pub fn is_required(folder: Folder, file_name: &str, status: Option<&str>) -> bool {
    folder == Folder::Done
        && is_implemented(status)
        && milestone(file_name).is_some_and(|m| m >= FIRST_REQUIRED)
}

// ── The world a citation resolves against ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyscallState {
    Dispatched,
    Undispatched,
    Undeclared,
    /// The two committed facts could not be read; the reason.
    Unknown(String),
}

pub trait World {
    fn syscall(&self, name: &str) -> SyscallState;
    /// The text of `tests/qemu/artifacts/<profile>/expected-markers.txt`, if it exists.
    fn profile_markers(&self, profile: &str) -> Option<String>;
    /// `None` = no such file; `Some(n)` = a file of `n` bytes.
    fn file_size(&self, rel: &str) -> Option<u64>;
    fn read(&self, rel: &str) -> Option<String>;
}

/// The real tree under `root`.
pub struct FsWorld {
    root: PathBuf,
    surface: Result<(Vec<String>, Expected), String>,
}

impl FsWorld {
    pub fn load(root: &Path) -> FsWorld {
        let surface = (|| {
            let abi = std::fs::read_to_string(root.join(ABI_SYSCALL_PATH))
                .map_err(|e| format!("cannot read {ABI_SYSCALL_PATH}: {e}"))?;
            let exp = std::fs::read_to_string(root.join(EXPECTED_PATH))
                .map_err(|e| format!("cannot read {EXPECTED_PATH}: {e}"))?;
            let expected =
                parse_expected(&exp).map_err(|e| format!("cannot parse {EXPECTED_PATH}: {e}"))?;
            let declared = parse_declared(&abi).into_iter().map(|(n, _)| n).collect();
            Ok((declared, expected))
        })();
        FsWorld {
            root: root.to_path_buf(),
            surface,
        }
    }
}

/// A repository-relative path that cannot leave the repository.
fn safe_rel(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty() && p.is_relative() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

impl World for FsWorld {
    fn syscall(&self, name: &str) -> SyscallState {
        match &self.surface {
            Err(e) => SyscallState::Unknown(e.clone()),
            Ok((declared, expected)) => {
                if !declared.iter().any(|d| d == name) {
                    SyscallState::Undeclared
                } else if expected.undispatched.iter().any(|u| u == name) {
                    SyscallState::Undispatched
                } else {
                    SyscallState::Dispatched
                }
            }
        }
    }

    fn profile_markers(&self, profile: &str) -> Option<String> {
        if !is_ident_dash(profile) {
            return None;
        }
        std::fs::read_to_string(
            self.root
                .join(MARKERS_DIR)
                .join(profile)
                .join("expected-markers.txt"),
        )
        .ok()
    }

    fn file_size(&self, rel: &str) -> Option<u64> {
        if !safe_rel(rel) {
            return None;
        }
        let m = std::fs::metadata(self.root.join(rel)).ok()?;
        m.is_file().then_some(m.len())
    }

    fn read(&self, rel: &str) -> Option<String> {
        if !safe_rel(rel) {
            return None;
        }
        std::fs::read_to_string(self.root.join(rel)).ok()
    }
}

fn is_ident_dash(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Does `src` hold `fn <name>(` as a `#[test]`? The `#[test]` must be in the attribute/doc-comment
/// run directly above the `fn` line (no blank line between).
pub fn has_test_fn(src: &str, name: &str) -> bool {
    let needle = format!("fn {name}(");
    let lines: Vec<&str> = src.lines().collect();
    for (i, l) in lines.iter().enumerate() {
        let mut t = l.trim_start();
        for prefix in ["pub ", "async ", "unsafe "] {
            if let Some(rest) = t.strip_prefix(prefix) {
                t = rest;
            }
        }
        let is_fn = t.starts_with(&needle);
        if !is_fn {
            continue;
        }
        let mut j = i;
        while j > 0 {
            j -= 1;
            let above = lines[j].trim();
            if above == "#[test]" {
                return true;
            }
            if above.starts_with("#[") || above.starts_with("///") {
                continue;
            }
            break;
        }
    }
    false
}

/// Does this one citation resolve? `Err` is the reason it does not.
pub fn resolve(c: &Citation, w: &dyn World) -> Result<(), String> {
    match c {
        Citation::Syscall(name) => match w.syscall(name) {
            SyscallState::Dispatched => Ok(()),
            SyscallState::Undispatched => Err(format!(
                "`{}`: `SyscallNumber::{name}` is declared but **undispatched** \
                 (`{EXPECTED_PATH}` lists it): the kernel answers `UnknownSyscall`, so nothing runs",
                c.render()
            )),
            SyscallState::Undeclared => Err(format!(
                "`{}`: no `SyscallNumber::{name}` is declared in {ABI_SYSCALL_PATH}",
                c.render()
            )),
            SyscallState::Unknown(why) => Err(format!(
                "`{}`: the syscall surface could not be read ({why})",
                c.render()
            )),
        },
        Citation::Marker { profile, marker } => match w.profile_markers(profile) {
            None => Err(format!(
                "`{}`: profile `{profile}` has no committed \
                 `{MARKERS_DIR}/{profile}/expected-markers.txt` -- a profile with no rendering has \
                 no gated markers, and an ungated marker is not evidence",
                c.render()
            )),
            Some(text) => {
                if text.lines().any(|l| l == marker) {
                    Ok(())
                } else if let Some(line) = text.lines().find(|l| l.trim() == marker.trim()) {
                    // The gated line has leading or trailing whitespace; a citation is trimmed
                    // before it is read, so it can never name such a line. Say *that* -- "not a
                    // line of the file" sends the reader to a file in which they can see it.
                    Err(format!(
                        "`{}`: the line is there but cannot be cited -- `{profile}`'s \
                         `expected-markers.txt` holds {line:?}, which has leading or trailing \
                         whitespace, and a citation is trimmed before it is read. Cite a different \
                         marker of that profile",
                        c.render()
                    ))
                } else {
                    Err(format!(
                        "`{}`: `{marker}` is not a line of `{MARKERS_DIR}/{profile}/expected-markers.txt`",
                        c.render()
                    ))
                }
            }
        },
        Citation::File(path) => match w.file_size(path) {
            None => Err(format!(
                "`{}`: `{path}` is not a file in the repository (a missing path, or one that \
                 leaves it)",
                c.render()
            )),
            Some(0) => Err(format!("`{}`: `{path}` is empty", c.render())),
            Some(_) => Ok(()),
        },
        Citation::Test { path, name } => match w.read(path) {
            None => Err(format!(
                "`{}`: `{path}` cannot be read as a file in the repository",
                c.render()
            )),
            Some(src) => {
                if has_test_fn(&src, name) {
                    Ok(())
                } else {
                    Err(format!(
                        "`{}`: `{path}` has no `fn {name}(` directly under a `#[test]`",
                        c.render()
                    ))
                }
            }
        },
    }
}

// ── One RFC ───────────────────────────────────────────────────────────────────

/// What the check found for one RFC. **The one predicate**: the subcheck and `rfc promote` both call
/// [`examine`], so what promotion refuses and what the gate rejects cannot differ.
#[derive(Debug, Default)]
pub struct Examination {
    /// Declared evidence is required of this RFC.
    pub required: bool,
    /// The parsed citations (empty when none were declared or they did not parse).
    pub citations: Vec<Citation>,
    /// Whether any `**Evidence:**` line was declared at all.
    pub declared: bool,
    pub problems: Vec<String>,
}

impl Examination {
    /// Declared, parsed, and every citation resolves.
    pub fn evidenced(&self) -> bool {
        self.declared && self.problems.is_empty() && !self.citations.is_empty()
    }
}

/// Examine one RFC's text. `status` is what the RFC says (or, for `rfc promote`, **what it will
/// say once moved**); `folder` is where it sits (or will).
pub fn examine(
    folder: Folder,
    file_name: &str,
    text: &str,
    status: Option<&str>,
    w: &dyn World,
) -> Examination {
    let mut ex = Examination {
        required: is_required(folder, file_name, status),
        ..Default::default()
    };
    match evidence_field(text) {
        Err(e) => ex.problems.push(e),
        Ok(None) => {
            if ex.required {
                ex.problems.push(
                    "is (or is about to be) `Implemented` and from 0.45 on, so it must cite how its mechanism is shown \
                     to run, and its header block (the lines before the first `---` or `## ` \
                     heading) has no `**Evidence:**` line -- e.g. \
                     `**Evidence:** syscall:CapInstall, marker:policy:NEG:POLICY:…:PASS`. The check \
                     reads only the header block; a field written in the body is not seen"
                        .to_string(),
                );
            }
        }
        Ok(Some(value)) => {
            ex.declared = true;
            match parse_citations(&value) {
                Err(bad) => ex.problems.extend(bad),
                Ok(cs) => {
                    for c in &cs {
                        if let Err(why) = resolve(c, w) {
                            ex.problems.push(why);
                        }
                    }
                    ex.citations = cs;
                }
            }
        }
    }
    ex
}

/// What `rfc promote` asks of an RFC about to become `Implemented` in `done/`: the problems, empty
/// when it may move. Calls [`examine`] -- the gate's own function.
pub fn promotion_problems(file_name: &str, text: &str, w: &dyn World) -> Vec<String> {
    examine(Folder::Done, file_name, text, Some("Implemented"), w).problems
}

// ── The whole tree ────────────────────────────────────────────────────────────

pub struct RfcFile<'a> {
    pub folder: Folder,
    pub name: &'a str,
    pub text: &'a str,
}

#[derive(Debug, Default)]
pub struct Report {
    /// RFCs required to carry evidence (0.45 onward, `Implemented*`).
    pub required: usize,
    /// RFCs with at least one citation, all of which resolve.
    pub evidenced: usize,
    /// Resolved citations, by kind (over every evidenced RFC).
    pub by_kind: [usize; 4],
    /// `Implemented*` RFCs in `done/`, before 0.45, declaring no evidence, and on neither list.
    pub uncovered_pre: usize,
    pub known_false: Vec<String>,
    pub waived: Vec<String>,
}

fn kind_index(k: Kind) -> usize {
    match k {
        Kind::Syscall => 0,
        Kind::Marker => 1,
        Kind::File => 2,
        Kind::Test => 3,
    }
}

/// Judge every RFC. `Err` is every problem, one line each, naming the RFC.
pub fn judge(
    files: &[RfcFile],
    w: &dyn World,
    waivers: &[Waiver],
    known_false: &[KnownFalse],
) -> Result<Report, Vec<String>> {
    let mut problems = Vec::new();
    let mut report = Report::default();

    // Which ids are listed, and in which list (a name in both is a defect).
    let listed =
        |id: &str| waivers.iter().any(|x| x.id == id) || known_false.iter().any(|x| x.id == id);
    for k in known_false {
        if waivers.iter().any(|x| x.id == k.id) {
            problems.push(format!(
                "`{}` is on both KNOWN_FALSE and WAIVERS: one provenance per name",
                k.id
            ));
        }
    }

    for f in files {
        let status = extract_status_keyword(f.text);
        let ex = examine(f.folder, f.name, f.text, status, w);
        let id = rfc_id(f.name);
        let on_list = listed(&id) && f.folder == Folder::Done;

        if ex.required {
            report.required += 1;
        }
        if ex.evidenced() {
            report.evidenced += 1;
            for c in &ex.citations {
                report.by_kind[kind_index(c.kind())] += 1;
            }
        }
        // A listed RFC's *missing* evidence is exempt (its false status is what is being kept
        // visible); a citation that does not resolve is a defect wherever it is written.
        let missing_only = ex.required && !ex.declared;
        if !(on_list && missing_only) {
            for p in &ex.problems {
                problems.push(format!("{}: {p}", f.name));
            }
        }
        if f.folder == Folder::Done
            && is_implemented(status)
            && !ex.required
            && !ex.declared
            && !on_list
        {
            report.uncovered_pre += 1;
        }
    }

    // Each listed name must name a `done/` RFC that is still `Implemented*` and still carries no
    // resolving evidence -- "a waiver is not a place to park a name".
    let mut check_listed = |id: &str, kind: &str, out: &mut Vec<String>| {
        let hit = files
            .iter()
            .find(|f| f.folder == Folder::Done && rfc_id(f.name) == id);
        match hit {
            None => problems.push(format!(
                "{kind} `{id}` names no RFC in rfcs/done/: a {kind} is not a place to park a name"
            )),
            Some(f) => {
                let status = extract_status_keyword(f.text);
                let ex = examine(f.folder, f.name, f.text, status, w);
                if !is_implemented(status) {
                    problems.push(format!(
                        "{kind} `{id}`: {} is no longer `Implemented` (it says {}): retire the entry",
                        f.name,
                        status.unwrap_or("nothing")
                    ));
                } else if ex.evidenced() {
                    problems.push(format!(
                        "{kind} `{id}`: {} now carries resolving evidence: retire the entry",
                        f.name
                    ));
                } else {
                    out.push(id.to_string());
                }
            }
        }
    };
    let mut kf = Vec::new();
    for k in known_false {
        check_listed(k.id, "KNOWN_FALSE entry", &mut kf);
    }
    let mut wv = Vec::new();
    for x in waivers {
        check_listed(x.id, "waiver", &mut wv);
    }
    report.known_false = kf;
    report.waived = wv;

    if problems.is_empty() {
        Ok(report)
    } else {
        Err(problems)
    }
}

/// The one PASS line -- **its wording is a requirement, not polish** (R4). It states what the check
/// does not cover, every run, in words.
pub fn render_pass(r: &Report) -> String {
    let kinds = format!(
        "by kind: syscall {}, marker {}, file {}, test {} -- `test:` is the weakest",
        r.by_kind[0], r.by_kind[1], r.by_kind[2], r.by_kind[3]
    );
    let required = if r.required == 0 {
        "0 RFCs are REQUIRED to carry evidence yet (no RFC from 0.45 on is `Implemented`), so \
         this check enforces nothing yet -- it is the mechanism, not coverage"
            .to_string()
    } else {
        format!(
            "{} RFC(s) are REQUIRED to carry evidence (0.45 onward, `Implemented`)",
            r.required
        )
    };
    let kf = if r.known_false.is_empty() {
        "0 known-false statuses".to_string()
    } else {
        format!(
            "{} known-false status(es) reported by name -- their disposition is the owner's pending \
             lifecycle decision, and their status lines are not touched: {}",
            r.known_false.len(),
            r.known_false.join(", ")
        )
    };
    let wv = if r.waived.is_empty() {
        "0 waivers".to_string()
    } else {
        format!("{} waiver(s): {}", r.waived.len(), r.waived.join(", "))
    };
    format!(
        "{NAME}: PASS ({} RFC(s) evidenced -- their citations RESOLVE ({kinds}), which is not a \
         guarantee the mechanism is sufficient; {required}; {} done RFC(s) from before 0.45 declare \
         no evidence and are NOT covered by this check; {kf}; {wv})",
        r.evidenced, r.uncovered_pre
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Default)]
    struct Fake {
        declared: Vec<&'static str>,
        undispatched: Vec<&'static str>,
        markers: BTreeMap<&'static str, &'static str>,
        files: BTreeMap<&'static str, &'static str>,
    }

    impl World for Fake {
        fn syscall(&self, name: &str) -> SyscallState {
            if !self.declared.contains(&name) {
                SyscallState::Undeclared
            } else if self.undispatched.contains(&name) {
                SyscallState::Undispatched
            } else {
                SyscallState::Dispatched
            }
        }
        fn profile_markers(&self, p: &str) -> Option<String> {
            self.markers.get(p).map(|s| s.to_string())
        }
        fn file_size(&self, rel: &str) -> Option<u64> {
            if !safe_rel(rel) {
                return None;
            }
            self.files.get(rel).map(|s| s.len() as u64)
        }
        fn read(&self, rel: &str) -> Option<String> {
            if !safe_rel(rel) {
                return None;
            }
            self.files.get(rel).map(|s| s.to_string())
        }
    }

    fn world() -> Fake {
        let mut w = Fake {
            declared: vec!["CapInstall", "CapMint", "TaskKill"],
            undispatched: vec!["TaskKill"],
            ..Default::default()
        };
        w.markers
            .insert("policy", "NEG:POLICY:A:PASS\nNEG:POLICY:B:PASS\n");
        w.files.insert("docs/x.md", "text");
        w.files.insert("docs/empty.md", "");
        w.files.insert(
            "crates/a/src/lib.rs",
            "#[test]\nfn is_a_test() {}\n\nfn helper() {}\n\n#[test]\n#[ignore]\nfn also_a_test() {}\n\n#[test]\n\nfn blank_between() {}\n",
        );
        w
    }

    fn rfc(status: &str, extra: &str) -> String {
        format!("# RFC\n\n**Status:** {status}\n**Milestone:** 0.45\n{extra}\n---\n\n## Body\n")
    }

    // ── identity ──────────────────────────────────────────────────────────────

    #[test]
    fn ids_and_milestones_are_read_numerically() {
        assert_eq!(rfc_id("RFC-0.45-003-an-implemented.md"), "RFC-0.45-003");
        assert_eq!(rfc_id("RFC-v0.7.3-001-networking.md"), "RFC-v0.7.3-001");
        assert_eq!(rfc_id("056-cap-broker.md"), "056");
        assert_eq!(milestone("RFC-0.45-003-x.md"), Some((0, 45)));
        assert_eq!(milestone("RFC-0.9-001-x.md"), Some((0, 9)));
        assert_eq!(milestone("RFC-v0.7.3-001-x.md"), None);
        assert_eq!(milestone("056-x.md"), None);
        // Numeric, not lexical: 0.9 is before 0.45, and 0.100 is after it.
        assert!(milestone("RFC-0.9-001-x.md").unwrap() < FIRST_REQUIRED);
        assert!(milestone("RFC-0.100-001-x.md").unwrap() >= FIRST_REQUIRED);
        assert!(milestone("RFC-1.0-001-x.md").unwrap() >= FIRST_REQUIRED);
    }

    #[test]
    fn who_must_carry_evidence() {
        let imp = Some("Implemented");
        assert!(is_required(Folder::Done, "RFC-0.45-001-x.md", imp));
        assert!(is_required(
            Folder::Done,
            "RFC-0.46-001-x.md",
            Some("Implemented-with-Errata")
        ));
        assert!(
            !is_required(Folder::Done, "RFC-0.44-001-x.md", imp),
            "before 0.45"
        );
        assert!(
            !is_required(Folder::Done, "056-x.md", imp),
            "pre-0.45 by construction"
        );
        assert!(!is_required(
            Folder::Accepted,
            "RFC-0.45-001-x.md",
            Some("Accepted")
        ));
        assert!(!is_required(
            Folder::Proposed,
            "RFC-0.45-001-x.md",
            Some("Proposed")
        ));
        assert!(!is_required(
            Folder::Done,
            "RFC-0.45-001-x.md",
            Some("Superseded")
        ));
        assert!(!is_required(
            Folder::Done,
            "RFC-0.45-001-x.md",
            Some("Withdrawn")
        ));
        assert!(!is_required(
            Folder::Done,
            "RFC-0.45-001-x.md",
            Some("Closed")
        ));
    }

    // ── parsing ───────────────────────────────────────────────────────────────

    #[test]
    fn the_four_kinds_parse() {
        let cs = parse_citations(
            "syscall:CapInstall, marker:policy:NEG:POLICY:GRANT_INSTALLS_USABLE_CAP:PASS, \
             file:docs/x.md, test:crates/a/src/lib.rs::is_a_test",
        )
        .unwrap();
        assert_eq!(cs.len(), 4);
        assert_eq!(cs[0], Citation::Syscall("CapInstall".into()));
        assert_eq!(
            cs[1],
            Citation::Marker {
                profile: "policy".into(),
                marker: "NEG:POLICY:GRANT_INSTALLS_USABLE_CAP:PASS".into()
            }
        );
        assert_eq!(cs[2].kind(), Kind::File);
        assert_eq!(cs[3].kind(), Kind::Test);
        // Round-trips.
        assert_eq!(
            cs[1].render(),
            "marker:policy:NEG:POLICY:GRANT_INSTALLS_USABLE_CAP:PASS"
        );
    }

    #[test]
    fn backticks_are_tolerated_and_every_malformed_citation_is_named() {
        assert!(parse_citations("`syscall:CapInstall`, `file:docs/x.md`").is_ok());
        let bad =
            parse_citations("syscall:CapInstall, nonsense, widget:x, file:, marker:policy, test:x")
                .unwrap_err();
        assert_eq!(bad.len(), 5, "{bad:#?}");
        assert!(bad.iter().any(|b| b.contains("nonsense")));
        assert!(
            bad.iter()
                .any(|b| b.contains("unknown citation kind `widget`"))
        );
        assert!(bad.iter().any(|b| b.contains("empty value")));
        assert!(bad.iter().any(|b| b.contains("marker:<profile>:<MARKER>")));
        assert!(bad.iter().any(|b| b.contains("test:<path>::<fn_name>")));
        assert!(parse_citations("").is_err());
        assert!(
            parse_citations("syscall:CapInstall,").is_err(),
            "a stray comma is a defect"
        );
        assert!(
            parse_citations("syscall:capinstall").is_err(),
            "a variant name is capitalised"
        );
    }

    #[test]
    fn only_the_header_block_is_read() {
        let in_header =
            "# R\n\n**Status:** Implemented\n**Evidence:** file:docs/x.md\n\n---\n\nbody\n";
        assert_eq!(
            evidence_field(in_header).unwrap(),
            Some("file:docs/x.md".into())
        );
        let in_body = "# R\n\n**Status:** Implemented\n\n---\n\n**Evidence:** file:docs/x.md\n";
        assert_eq!(
            evidence_field(in_body).unwrap(),
            None,
            "after `---` is the body"
        );
        let after_heading =
            "# R\n\n**Status:** Implemented\n## Evidence\n**Evidence:** file:docs/x.md\n";
        assert_eq!(
            evidence_field(after_heading).unwrap(),
            None,
            "after `## ` is the body"
        );
        let twice = "# R\n**Evidence:** file:a\n**Evidence:** file:b\n---\n";
        assert!(evidence_field(twice).is_err());
    }

    // ── resolution: one test per kind, each in both directions ────────────────

    #[test]
    fn syscall_resolves_only_when_declared_and_dispatched() {
        let w = world();
        assert!(resolve(&Citation::Syscall("CapMint".into()), &w).is_ok());
        // The live control -- RFC 056's own case: declared, undispatched, so red.
        let e = resolve(&Citation::Syscall("TaskKill".into()), &w).unwrap_err();
        assert!(
            e.contains("undispatched") && e.contains("UnknownSyscall"),
            "{e}"
        );
        let e = resolve(&Citation::Syscall("NoSuch".into()), &w).unwrap_err();
        assert!(e.contains("no `SyscallNumber::NoSuch`"), "{e}");
    }

    #[test]
    fn marker_resolves_only_against_a_committed_rendering() {
        let w = world();
        let m = |p: &str, k: &str| Citation::Marker {
            profile: p.into(),
            marker: k.into(),
        };
        assert!(resolve(&m("policy", "NEG:POLICY:A:PASS"), &w).is_ok());
        let e = resolve(&m("policy", "NEG:POLICY:ZZZ:PASS"), &w).unwrap_err();
        assert!(e.contains("is not a line of"), "{e}");
        // A prefix of a line is not the line.
        assert!(resolve(&m("policy", "NEG:POLICY:A"), &w).is_err());
        let e = resolve(&m("no-such-profile", "X"), &w).unwrap_err();
        assert!(
            e.contains("no committed") && e.contains("ungated marker is not evidence"),
            "{e}"
        );
        // A profile name cannot walk out of the artefacts directory.
        assert!(
            resolve(
                &m("../policy", "NEG:POLICY:A:PASS"),
                &FsWorld::load(Path::new("."))
            )
            .is_err()
        );
    }

    #[test]
    fn a_gated_marker_with_trailing_whitespace_says_it_cannot_be_cited_rather_than_that_it_is_absent()
     {
        let mut w = world();
        w.markers.insert("net", "found at slot \nmagic=0x1\n");
        let m = |k: &str| Citation::Marker {
            profile: "net".into(),
            marker: k.into(),
        };
        // The trimmed citation (as `parse_citations` hands it over) matches only after trimming.
        let e = resolve(&m("found at slot"), &w).unwrap_err();
        assert!(
            e.contains("cannot be cited")
                && e.contains("trailing")
                && e.contains("\"found at slot \""),
            "{e}"
        );
        assert!(!e.contains("is not a line of"), "{e}");
        // Controls: a line without whitespace resolves; a marker that is really absent still
        // says so.
        assert!(resolve(&m("magic=0x1"), &w).is_ok());
        assert!(
            resolve(&m("nothing like it"), &w)
                .unwrap_err()
                .contains("is not a line of")
        );
    }

    #[test]
    fn file_resolves_only_when_a_nonempty_file_inside_the_repo() {
        let w = world();
        assert!(resolve(&Citation::File("docs/x.md".into()), &w).is_ok());
        assert!(
            resolve(&Citation::File("docs/none.md".into()), &w)
                .unwrap_err()
                .contains("not a file")
        );
        assert!(
            resolve(&Citation::File("docs/empty.md".into()), &w)
                .unwrap_err()
                .contains("empty")
        );
        assert!(resolve(&Citation::File("../etc/passwd".into()), &w).is_err());
        assert!(resolve(&Citation::File("/etc/passwd".into()), &w).is_err());
    }

    /// The real filesystem world, against a real directory: a citation cannot name a file that
    /// exists but lies **outside** the repository root. (The fake world cannot show this -- its
    /// map simply lacks the path -- so this is the test that holds `safe_rel` to account.)
    #[test]
    fn the_real_world_will_not_resolve_a_file_outside_its_root() {
        let base = std::env::temp_dir().join(format!("fjell-rfc-evidence-{}", std::process::id()));
        let root = base.join("repo");
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/inside.md"), "inside").unwrap();
        std::fs::write(base.join("outside.md"), "outside").unwrap();
        let w = FsWorld::load(&root);
        // Control: a file inside resolves; the outside file exists on disk.
        assert!(resolve(&Citation::File("docs/inside.md".into()), &w).is_ok());
        assert!(
            base.join("outside.md").is_file(),
            "control: the outside file exists"
        );
        for escape in ["../outside.md", "docs/../../outside.md"] {
            assert!(
                resolve(&Citation::File(escape.into()), &w).is_err(),
                "{escape}"
            );
            assert!(
                resolve(
                    &Citation::Test {
                        path: escape.into(),
                        name: "x".into()
                    },
                    &w
                )
                .is_err(),
                "{escape}"
            );
        }
        let abs = base.join("outside.md").to_string_lossy().into_owned();
        assert!(
            resolve(&Citation::File(abs), &w).is_err(),
            "an absolute path is refused"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn test_resolves_only_a_fn_directly_under_a_test_attribute() {
        let w = world();
        let t = |n: &str| Citation::Test {
            path: "crates/a/src/lib.rs".into(),
            name: n.into(),
        };
        assert!(resolve(&t("is_a_test"), &w).is_ok());
        assert!(
            resolve(&t("also_a_test"), &w).is_ok(),
            "other attributes may sit between"
        );
        assert!(
            resolve(&t("helper"), &w)
                .unwrap_err()
                .contains("directly under a `#[test]`"),
            "a fn that is not a test"
        );
        assert!(
            resolve(&t("blank_between"), &w).is_err(),
            "a blank line breaks the run"
        );
        assert!(resolve(&t("not_there"), &w).is_err());
        assert!(
            resolve(
                &Citation::Test {
                    path: "crates/none.rs".into(),
                    name: "x".into()
                },
                &w
            )
            .is_err()
        );
        // The name must be the whole identifier.
        assert!(resolve(&t("is_a"), &w).is_err());
    }

    // ── one RFC ───────────────────────────────────────────────────────────────

    #[test]
    fn an_implemented_0_45_rfc_with_no_field_is_refused_and_told_where_the_check_looks() {
        let w = world();
        let text = rfc("Implemented", "");
        let ex = examine(
            Folder::Done,
            "RFC-0.45-001-x.md",
            &text,
            Some("Implemented"),
            &w,
        );
        assert!(ex.required && !ex.declared && !ex.evidenced());
        assert_eq!(ex.problems.len(), 1);
        assert!(
            ex.problems[0].contains("header block") && ex.problems[0].contains("**Evidence:**")
        );
    }

    #[test]
    fn the_same_rfc_with_resolving_evidence_passes() {
        let w = world();
        let text = rfc(
            "Implemented",
            "**Evidence:** syscall:CapMint, marker:policy:NEG:POLICY:A:PASS",
        );
        let ex = examine(
            Folder::Done,
            "RFC-0.45-001-x.md",
            &text,
            Some("Implemented"),
            &w,
        );
        assert!(ex.evidenced(), "{:#?}", ex.problems);
    }

    #[test]
    fn rfc_056s_exact_case_is_red_while_the_syscall_is_undispatched() {
        // The live control (R5): an `Implemented` RFC citing a declared-but-undispatched syscall.
        let w = world();
        let text = rfc("Implemented", "**Evidence:** syscall:TaskKill");
        let ex = examine(
            Folder::Done,
            "RFC-0.45-009-x.md",
            &text,
            Some("Implemented"),
            &w,
        );
        assert!(!ex.evidenced());
        assert!(
            ex.problems[0].contains("undispatched"),
            "{:#?}",
            ex.problems
        );
    }

    #[test]
    fn a_dangling_citation_is_a_defect_wherever_it_is_written() {
        let w = world();
        // An old RFC, an accepted one: not required, but a citation they do carry must resolve.
        for (folder, name) in [
            (Folder::Done, "056-x.md"),
            (Folder::Accepted, "RFC-0.45-001-x.md"),
            (Folder::Proposed, "RFC-0.46-001-x.md"),
        ] {
            let text = rfc("Accepted", "**Evidence:** file:docs/none.md");
            let ex = examine(folder, name, &text, Some("Accepted"), &w);
            assert!(!ex.required);
            assert_eq!(ex.problems.len(), 1, "{name}: {:#?}", ex.problems);
        }
    }

    #[test]
    fn promotion_asks_the_gates_own_question() {
        let w = world();
        // An accepted RFC about to become Implemented, with no field: refused.
        let none = rfc("Accepted", "");
        assert_eq!(promotion_problems("RFC-0.45-002-x.md", &none, &w).len(), 1);
        // With evidence that resolves: may move.
        let ok = rfc("Accepted", "**Evidence:** syscall:CapMint");
        assert!(promotion_problems("RFC-0.45-002-x.md", &ok, &w).is_empty());
        // With evidence that does not: refused, by the same resolution.
        let bad = rfc("Accepted", "**Evidence:** syscall:TaskKill");
        assert_eq!(promotion_problems("RFC-0.45-002-x.md", &bad, &w).len(), 1);
        // A pre-0.45 milestone is not asked for evidence.
        assert!(promotion_problems("RFC-0.44-002-x.md", &none, &w).is_empty());
    }

    // ── the whole tree: counts, lists, and the line's wording ─────────────────

    fn texts() -> BTreeMap<&'static str, String> {
        let mut t = BTreeMap::new();
        t.insert("RFC-0.46-001-new.md", rfc("Implemented", "**Evidence:** syscall:CapMint, file:docs/x.md, test:crates/a/src/lib.rs::is_a_test"));
        t.insert("RFC-0.44-001-old.md", rfc("Implemented", ""));
        t.insert("RFC-0.44-002-old.md", rfc("Implemented-with-Errata", ""));
        t.insert("RFC-v0.7.3-001-bad.md", rfc("Implemented (v0.7.2)", ""));
        t.insert("RFC-v0.7.3-002-bad.md", rfc("Implemented (v0.7.1)", ""));
        t.insert("RFC-0.46-002-acc.md", rfc("Accepted", ""));
        t
    }

    fn files_of<'a>(t: &'a BTreeMap<&'static str, String>) -> Vec<RfcFile<'a>> {
        t.iter()
            .map(|(n, text)| {
                let folder = if n.contains("-acc") {
                    Folder::Accepted
                } else {
                    Folder::Done
                };
                RfcFile {
                    folder,
                    name: n,
                    text,
                }
            })
            .collect()
    }

    const KF: &[KnownFalse] = &[
        KnownFalse {
            id: "RFC-v0.7.3-001",
            reason: "r1",
        },
        KnownFalse {
            id: "RFC-v0.7.3-002",
            reason: "r2",
        },
    ];

    #[test]
    fn the_tree_is_judged_and_counted_by_kind() {
        let w = world();
        let t = texts();
        let r = judge(&files_of(&t), &w, &[], KF).unwrap();
        assert_eq!(r.required, 1);
        assert_eq!(r.evidenced, 1);
        assert_eq!(r.by_kind, [1, 0, 1, 1], "syscall, marker, file, test");
        assert_eq!(
            r.uncovered_pre, 2,
            "the two 0.44 RFCs; the two known-false are not double-counted"
        );
        assert_eq!(r.known_false, ["RFC-v0.7.3-001", "RFC-v0.7.3-002"]);
        assert!(r.waived.is_empty());
    }

    #[test]
    fn the_pass_line_states_its_limits_in_words() {
        let w = world();
        let t = texts();
        let line = render_pass(&judge(&files_of(&t), &w, &[], KF).unwrap());
        for needle in [
            "rfc-evidence: PASS",
            "1 RFC(s) evidenced",
            "RESOLVE",
            "not a guarantee the mechanism is sufficient",
            "by kind: syscall 1, marker 0, file 1, test 1",
            "`test:` is the weakest",
            "1 RFC(s) are REQUIRED",
            "2 done RFC(s) from before 0.45 declare no evidence and are NOT covered by this check",
            "2 known-false status(es) reported by name",
            "the owner's pending lifecycle decision",
            "status lines are not touched",
            "RFC-v0.7.3-001, RFC-v0.7.3-002",
            "0 waivers",
        ] {
            assert!(line.contains(needle), "missing `{needle}` in: {line}");
        }
    }

    #[test]
    fn zero_required_is_printed_as_zero_and_says_it_enforces_nothing() {
        let w = world();
        let mut t = texts();
        t.remove("RFC-0.46-001-new.md");
        let line = render_pass(&judge(&files_of(&t), &w, &[], KF).unwrap());
        assert!(line.contains("0 RFC(s) evidenced"), "{line}");
        assert!(
            line.contains("0 RFCs are REQUIRED to carry evidence yet"),
            "{line}"
        );
        assert!(line.contains("enforces nothing yet"), "{line}");
        assert!(line.contains("it is the mechanism, not coverage"), "{line}");
    }

    #[test]
    fn a_required_rfc_with_no_field_fails_the_tree() {
        let w = world();
        let mut t = texts();
        t.insert("RFC-0.46-003-newer.md", rfc("Implemented", ""));
        let e = judge(&files_of(&t), &w, &[], KF).unwrap_err();
        assert_eq!(e.len(), 1, "{e:#?}");
        assert!(e[0].starts_with("RFC-0.46-003-newer.md:"), "{e:#?}");
    }

    #[test]
    fn the_known_false_entries_are_exempt_from_the_field_but_not_from_resolution() {
        let w = world();
        let mut t = texts();
        // A listed RFC that now *cites* something dangling: that is still a defect.
        t.insert(
            "RFC-v0.7.3-001-bad.md",
            rfc("Implemented (v0.7.2)", "**Evidence:** file:docs/none.md"),
        );
        let e = judge(&files_of(&t), &w, &[], KF).unwrap_err();
        assert!(e.iter().any(|p| p.contains("docs/none.md")), "{e:#?}");
    }

    #[test]
    fn a_list_entry_for_an_rfc_that_does_not_exist_is_a_failure() {
        let w = world();
        let t = texts();
        let kf = [KnownFalse {
            id: "RFC-v0.9.9-999",
            reason: "x",
        }];
        let e = judge(&files_of(&t), &w, &[], &kf).unwrap_err();
        assert!(
            e.iter().any(|p| p.contains("names no RFC in rfcs/done/")
                && p.contains("not a place to park a name")),
            "{e:#?}"
        );
        let wv = [Waiver {
            id: "RFC-v0.9.9-999",
            reason: "x",
        }];
        assert!(judge(&files_of(&t), &w, &wv, KF).is_err());
    }

    #[test]
    fn a_list_entry_whose_rfc_now_carries_resolving_evidence_must_be_retired() {
        let w = world();
        let mut t = texts();
        t.insert(
            "RFC-v0.7.3-001-bad.md",
            rfc("Implemented (v0.7.2)", "**Evidence:** file:docs/x.md"),
        );
        let e = judge(&files_of(&t), &w, &[], KF).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("now carries resolving evidence: retire the entry")),
            "{e:#?}"
        );
    }

    #[test]
    fn a_list_entry_whose_rfc_is_no_longer_implemented_must_be_retired() {
        let w = world();
        let mut t = texts();
        // The owner resolved the lifecycle question: the status line changed.
        t.insert("RFC-v0.7.3-001-bad.md", rfc("Withdrawn", ""));
        let e = judge(&files_of(&t), &w, &[], KF).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("no longer `Implemented`") && p.contains("retire the entry")),
            "{e:#?}"
        );
    }

    #[test]
    fn a_name_on_both_lists_is_refused() {
        let w = world();
        let t = texts();
        let wv = [Waiver {
            id: "RFC-v0.7.3-001",
            reason: "x",
        }];
        let e = judge(&files_of(&t), &w, &wv, KF).unwrap_err();
        assert!(
            e.iter()
                .any(|p| p.contains("on both KNOWN_FALSE and WAIVERS")),
            "{e:#?}"
        );
    }

    #[test]
    fn a_waiver_exempts_a_required_rfc_and_is_printed() {
        let w = world();
        let mut t = texts();
        t.insert("RFC-0.46-003-newer.md", rfc("Implemented", ""));
        let wv = [Waiver {
            id: "RFC-0.46-003",
            reason: "x",
        }];
        let r = judge(&files_of(&t), &w, &wv, KF).unwrap();
        assert_eq!(r.waived, ["RFC-0.46-003"]);
        assert!(render_pass(&r).contains("1 waiver(s): RFC-0.46-003"));
    }

    #[test]
    fn the_shipped_lists_name_exactly_the_two_known_cases_and_no_waiver() {
        let ids: BTreeSet<&str> = KNOWN_FALSE.iter().map(|k| k.id).collect();
        assert_eq!(
            ids,
            ["RFC-v0.7.3-001", "RFC-v0.7.3-002"]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
        assert!(WAIVERS.is_empty());
        for k in KNOWN_FALSE {
            assert!(!k.reason.is_empty());
        }
    }
}
