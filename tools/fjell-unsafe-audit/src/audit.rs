//! The unsafe-site analysis (RFC-0.48-002): find every `unsafe` in the tree, pair it with the `SAFETY` comment that is its
//! own, and refuse a `category=` tag its operation does not support.
//!
//! **What changed, and why.** Until 0.48 this tool found sites by *substring on one line* (`unsafe {`, `unsafe fn `,
//! `unsafe impl `, `unsafe trait `) and judged **only that a comment existed**. It therefore could not see **49** of the tree's
//! 346 `unsafe` tokens -- `#[unsafe(no_mangle)]` (41), `#[unsafe(naked)]` (1), `unsafe extern` (6), and
//! `unsafe impl<T> Sync for KS<T>` (1, a `Sync` soundness claim on the static every `get_kernel_state()` goes through, missed
//! because of the trailing space in `"unsafe impl "`) -- and it passed 27 of 40 `get_*_state()` sites whose justification
//! described something else (*"all user-supplied pointers are checked…"* on a `static mut`).
//!
//! Sites are now found in **code only** (comments, strings and character literals blanked, by `fjell-rust-scan`), in every
//! form: block, fn, `extern` fn, `extern` block, impl (generic or not), trait, and `#[unsafe(...)]` attribute.
//!
//! ## What the check is, and is NOT
//!
//! **The category is gated; the prose is not.** A site's `category=` is *supported* when its body contains that category's
//! **shape** (`asm!` for `csr-asm`, a volatile access for `mmio-access`, a `static` or its accessor for
//! `kernel-global-mutable`, …) **or** it calls an `unsafe fn` (resolved by name in the workspace) that carries the same
//! category -- `unsafe { write_sie(..) }` is `csr-asm` because `write_sie` is. A site may carry two tags
//! (`category=a,b`); each must be supported. **The check can refuse a tag the evidence does not support; it can never decide
//! one**, and a **green means the tag is not contradicted, not that the justification is true.** Checking prose is how E-014
//! happens, and a rule that passed any sentence containing the right words would pass the very boilerplate this line exists to
//! remove. The PASS line says so, and a test pins the sentence.
//!
//! `unsafe impl` / `unsafe trait` / `unsafe extern` blocks / attributes have no body to match: they are judged on **having
//! their own comment and a valid tag**, and counted separately so nothing pretends a shape rule applied.
//!
//! A site whose true justification cannot be determined is **exempted by name with a reason** (`exempt.rs`), never given a
//! plausible sentence: a guessed comment is false in a new way. Each exemption is `Seeded` (awaiting a sweep, and refused once
//! the workspace is past the milestone that seeded it) or `Permanent`; a stale one fails.

use fjell_rust_scan::{Func, code_only, functions};
use std::collections::{BTreeMap, BTreeSet};

/// How far above a site its `// SAFETY:` comment may start (lines). Unchanged from the line-scanner this replaces.
pub const WINDOW: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    RawPointerDeref,
    PageTableMutation,
    CsrAsm,
    MmioAccess,
    PhysIdMapAssumption,
    KernelGlobalMutable,
    UserCopy,
    /// The address of a linker-provided symbol (`&__stack_top as *const u8 as usize`): none of the original seven (RFC-0.48-002 D2).
    LinkerSymbol,
    /// An exported symbol name (`#[unsafe(no_mangle)]`, `#[unsafe(naked)]`): an unchecked claim that nothing else uses the name.
    SymbolExport,
}

impl Category {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.trim() {
            "raw-pointer-deref" => Self::RawPointerDeref,
            "page-table-mutation" => Self::PageTableMutation,
            "csr-asm" => Self::CsrAsm,
            "mmio-access" => Self::MmioAccess,
            "phys-id-map-assumption" => Self::PhysIdMapAssumption,
            "kernel-global-mutable" => Self::KernelGlobalMutable,
            "user-copy" => Self::UserCopy,
            "linker-symbol" => Self::LinkerSymbol,
            "symbol-export" => Self::SymbolExport,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RawPointerDeref => "raw-pointer-deref",
            Self::PageTableMutation => "page-table-mutation",
            Self::CsrAsm => "csr-asm",
            Self::MmioAccess => "mmio-access",
            Self::PhysIdMapAssumption => "phys-id-map-assumption",
            Self::KernelGlobalMutable => "kernel-global-mutable",
            Self::UserCopy => "user-copy",
            Self::LinkerSymbol => "linker-symbol",
            Self::SymbolExport => "symbol-export",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Block,
    Fn,
    Impl,
    Trait,
    /// `unsafe extern "C" { ... }`
    ExternBlock,
    /// `#[unsafe(no_mangle)]`, `#[unsafe(naked)]`, ...
    Attr,
    /// An `unsafe` token in a form this scanner does not recognise: still a site, and it needs a comment.
    Other,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Fn => "fn",
            Self::Impl => "impl",
            Self::Trait => "trait",
            Self::ExternBlock => "extern",
            Self::Attr => "attr",
            Self::Other => "other",
        }
    }
    /// Kinds with no body for a shape to be matched in.
    fn bodyless(self) -> bool {
        matches!(
            self,
            Self::Impl | Self::Trait | Self::ExternBlock | Self::Attr | Self::Other
        )
    }
}

/// A site as discovered in one file.
#[derive(Debug, Clone)]
pub struct RawSite {
    pub line: usize,
    pub kind: Kind,
    /// Code-only text of the body (or of the attribute).
    pub body: String,
    /// The innermost enclosing function, or -- for an attribute -- the item it decorates; `<module>` otherwise.
    pub scope: String,
    /// The function's own name for `Kind::Fn`.
    pub own_name: Option<String>,
}

fn is_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] as char).is_whitespace() {
        i += 1;
    }
    i
}

fn word_at(b: &[u8], i: usize, w: &str) -> bool {
    let w = w.as_bytes();
    b.len() >= i + w.len()
        && &b[i..i + w.len()] == w
        && (i + w.len() == b.len() || !is_ident(b[i + w.len()]))
        && (i == 0 || !is_ident(b[i - 1]))
}

/// Matching `}` for the `{` at `open` (code-only text, so no braces hide in strings or comments).
fn match_brace(b: &[u8], open: usize) -> Option<usize> {
    let mut d = 0i32;
    for (k, c) in b.iter().enumerate().skip(open) {
        match c {
            b'{' => d += 1,
            b'}' => {
                d -= 1;
                if d == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// Every `unsafe` in `src`, in source order. `Err` names a construct whose body cannot be delimited.
pub fn discover(src: &str) -> Result<Vec<RawSite>, String> {
    let code = code_only(src);
    let b = code.as_bytes();
    let funcs: Vec<Func> = functions(&code)?;
    let line_of = |pos: usize| code[..pos].matches('\n').count() + 1;
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if !word_at(b, i, "unsafe") {
            i += 1;
            continue;
        }
        let start = i;
        i += "unsafe".len();
        let j = skip_ws(b, i);
        let line = line_of(start);
        let enclosing = || {
            funcs
                .iter()
                .filter(|f| f.start_line <= line && line <= f.end_line)
                .min_by_key(|f| f.end_line - f.start_line)
                .map(|f| f.name.clone())
        };
        let (kind, body, scope, own) = if j < b.len() && b[j] == b'{' {
            let end = match_brace(b, j).ok_or_else(|| {
                format!("line {line}: an `unsafe {{` block whose braces do not balance")
            })?;
            (
                Kind::Block,
                code[j..=end].to_string(),
                enclosing().unwrap_or_else(|| "<module>".into()),
                None,
            )
        } else if word_at(b, j, "fn")
            || (word_at(b, j, "extern") && word_at(b, skip_ws(b, j + "extern".len()), "fn"))
        {
            // the function's own body: find its `fn` keyword, then the next `{` outside parentheses before any `;`
            let fn_at = if word_at(b, j, "fn") {
                j
            } else {
                (j..b.len()).find(|&p| word_at(b, p, "fn")).unwrap_or(j)
            };
            let name_start = skip_ws(b, fn_at + 2);
            let name: String = code[name_start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let mut paren = 0i32;
            let mut open = None;
            let mut k = fn_at;
            while k < b.len() {
                match b[k] {
                    b'(' | b'[' => paren += 1,
                    b')' | b']' => paren -= 1,
                    b'{' if paren <= 0 => {
                        open = Some(k);
                        break;
                    }
                    b';' if paren <= 0 => break,
                    _ => {}
                }
                k += 1;
            }
            let body = match open {
                Some(o) => {
                    let e = match_brace(b, o).ok_or_else(|| {
                        format!("line {line}: `unsafe fn {name}` whose braces do not balance")
                    })?;
                    code[o..=e].to_string()
                }
                None => String::new(),
            };
            (Kind::Fn, body, name.clone(), Some(name))
        } else if word_at(b, j, "impl") {
            (
                Kind::Impl,
                String::new(),
                enclosing().unwrap_or_else(|| "<module>".into()),
                None,
            )
        } else if word_at(b, j, "trait") {
            (
                Kind::Trait,
                String::new(),
                enclosing().unwrap_or_else(|| "<module>".into()),
                None,
            )
        } else if word_at(b, j, "extern") {
            // `unsafe extern "C" {` (a block of declarations; the ABI string is blanked)
            let end = skip_ws(b, j + "extern".len());
            let close = if end < b.len() && b[end] == b'{' {
                match_brace(b, end)
            } else {
                None
            };
            (
                Kind::ExternBlock,
                code[end..close.map_or((end + 200).min(b.len()), |c| c + 1)].to_string(),
                "<module>".into(),
                None,
            )
        } else if j < b.len() && b[j] == b'(' && start >= 2 && &code[start - 2..start] == "#[" {
            // `#[unsafe(no_mangle)]`
            let end = code[j..].find(')').map_or(b.len(), |p| j + p + 1);
            let attr = code[j..end].to_string();
            // the item it decorates: the next `fn name` within the following lines
            let after = &code[end..(end + 400).min(b.len())];
            let item = after
                .find("fn ")
                .map(|p| {
                    after[p + 3..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect::<String>()
                })
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "<module>".into());
            (Kind::Attr, attr, item, None)
        } else {
            (
                Kind::Other,
                String::new(),
                enclosing().unwrap_or_else(|| "<module>".into()),
                None,
            )
        };
        out.push(RawSite {
            line,
            kind,
            body,
            scope,
            own_name: own,
        });
    }
    Ok(out)
}

/// The nearest `// SAFETY:` line above `idx` (0-based line index of the site) in the contiguous block of comment and attribute
/// lines directly above it, within [`WINDOW`] lines. Returns that line's index and its text.
pub fn find_comment(lines: &[&str], idx: usize) -> Option<(usize, String)> {
    let from = idx.saturating_sub(WINDOW);
    for k in (from..idx).rev() {
        let t = lines[k].trim();
        if t.starts_with("// SAFETY:") {
            return Some((k, t.to_string()));
        }
        if t.is_empty() || (!t.starts_with("//") && !t.starts_with('#')) {
            return None;
        }
    }
    None
}

/// The tags in `category=a,b` -- up to whitespace or `;` -- and any that are not categories.
pub fn parse_tags(comment: &str) -> Option<(Vec<Category>, Vec<String>)> {
    let pos = comment.find("category=")?;
    let rest = &comment[pos + "category=".len()..];
    let list = rest
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or("");
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    for t in list.split(',').filter(|t| !t.is_empty()) {
        match Category::parse(t) {
            Some(c) => ok.push(c),
            None => bad.push(t.to_string()),
        }
    }
    Some((ok, bad))
}

// ── the rule: which shapes a category may annotate ─────────────────────────────────────────────────────────────────────

fn has_word(body: &str, w: &str) -> bool {
    let b = body.as_bytes();
    let mut from = 0;
    while let Some(i) = body[from..].find(w) {
        let at = from + i;
        let before = at == 0 || !is_ident(b[at - 1]);
        let after = at + w.len() >= b.len() || !is_ident(b[at + w.len()]);
        if before && after {
            return true;
        }
        from = at + 1;
    }
    false
}

/// A `*` used as a dereference: preceded by an opener or operator, followed by an identifier or `(`.
fn has_deref(body: &str) -> bool {
    let b = body.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c != b'*' || i + 1 >= b.len() {
            continue;
        }
        let next = b[i + 1];
        if next == b'*' || next == b'=' || next == b'/' {
            continue;
        }
        let before = body[..i].trim_end();
        // `&mut *p` and `&*p`: the `*` follows a reference sigil, possibly through `mut`
        let after_ref = before.ends_with("&mut") || before.ends_with('&');
        let prev = before.bytes().last().unwrap_or(b'{');
        if (next.is_ascii_alphabetic() || next == b'_' || next == b'(')
            && (after_ref || matches!(prev, b'{' | b'(' | b',' | b'=' | b'!' | b';' | b'|' | b'>'))
        {
            return true;
        }
    }
    false
}

/// What the workspace declares, for the rules that need names resolved.
#[derive(Default, Debug)]
pub struct Workspace {
    /// ALL_CAPS names declared `static` / `static mut`.
    pub statics: BTreeSet<String>,
    /// `unsafe fn` name -> categories it carries.
    pub unsafe_fns: BTreeMap<String, BTreeSet<Category>>,
}

/// Does `body` (code-only) show the operation `cat` annotates, directly or through an `unsafe fn` callee of that category?
pub fn supported(cat: Category, kind: Kind, body: &str, ws: &Workspace) -> bool {
    if kind == Kind::Attr {
        return cat == Category::SymbolExport;
    }
    if kind == Kind::ExternBlock {
        return cat == Category::LinkerSymbol;
    }
    let direct = match cat {
        Category::CsrAsm => ["asm!", "global_asm!", "naked_asm!"]
            .iter()
            .any(|t| body.contains(t)),
        Category::MmioAccess => {
            body.contains("volatile") || has_word(body, "mmio") || has_word(body, "MMIO")
        }
        Category::KernelGlobalMutable => {
            [
                "get_kernel_state",
                "get_lease_table",
                "fa_static_ptr",
                ".0.get()",
                "addr_of",
            ]
            .iter()
            .any(|t| body.contains(t))
                || ws.statics.iter().any(|s| has_word(body, s))
        }
        Category::RawPointerDeref => {
            has_deref(body)
                || [
                    "from_raw_parts",
                    ".read()",
                    ".write(",
                    " as *",
                    "*const",
                    "*mut",
                    ".offset(",
                    ".add(",
                    ".cast",
                    "transmute",
                    "ptr::",
                    "(*",
                ]
                .iter()
                .any(|t| body.contains(t))
        }
        Category::PageTableMutation => [
            "page_table",
            "map_page",
            "unmap_page",
            "sfence",
            "satp",
            "Pte",
            "PageTable",
        ]
        .iter()
        .any(|t| body.contains(t)),
        Category::UserCopy => [
            "UserPtr",
            "copy_from_user",
            "copy_to_user",
            "user",
            "translate",
            "from_raw_parts",
        ]
        .iter()
        .any(|t| body.contains(t)),
        Category::PhysIdMapAssumption => {
            ["phys", "pfn", "<< 12", "PHYS", ".pa()", "frame", "identity"]
                .iter()
                .any(|t| body.contains(t))
        }
        Category::LinkerSymbol => {
            body.contains("&__") || body.contains("addr_of!(__") || has_linker_ident(body)
        }
        Category::SymbolExport => false,
    };
    if direct {
        return true;
    }
    callees(body)
        .iter()
        .any(|n| ws.unsafe_fns.get(n).is_some_and(|cs| cs.contains(&cat)))
}

fn has_linker_ident(body: &str) -> bool {
    let b = body.as_bytes();
    body.match_indices("__").any(|(i, _)| {
        (i == 0 || !is_ident(b[i - 1])) && b.get(i + 2).is_some_and(|c| c.is_ascii_lowercase())
    })
}

/// Identifiers directly followed by `(` in `body`.
pub fn callees(body: &str) -> BTreeSet<String> {
    let b = body.as_bytes();
    let mut out = BTreeSet::new();
    let mut i = 0;
    while i < b.len() {
        if is_ident(b[i]) && (i == 0 || !is_ident(b[i - 1])) {
            let s = i;
            while i < b.len() && is_ident(b[i]) {
                i += 1;
            }
            if b.get(i) == Some(&b'(') {
                out.insert(body[s..i].to_string());
            }
        } else {
            i += 1;
        }
    }
    out
}

// ── exemptions ─────────────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExemptKind {
    /// Awaiting a sweep. **Refused once the workspace version is past `until`**, so a seeded entry cannot become permanent by being
    /// forgotten.
    Seeded {
        until: &'static str,
    },
    Permanent,
}

#[derive(Debug, Clone)]
pub struct Exemption {
    /// A path suffix, e.g. `fjell-kernel/src/main.rs`.
    pub file: &'static str,
    /// The enclosing function (or decorated item), or `<module>`.
    pub scope: &'static str,
    /// The n-th (1-based) unsafe site in that scope within the file.
    pub n: usize,
    pub kind: ExemptKind,
    pub reason: &'static str,
}

fn version_key(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.split('.').map(|p| p.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

// ── the analysis ───────────────────────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Site {
    pub file: String,
    pub line: usize,
    pub kind: Kind,
    pub scope: String,
    pub n: usize,
    pub body: String,
    pub comment: Option<String>,
    pub tags: Vec<Category>,
    pub unknown_tags: Vec<String>,
    pub has_category: bool,
    /// Categories this site's body does not support (empty if all are, or the site has no body to judge).
    pub unsupported: Vec<Category>,
    pub exempt: Option<ExemptKind>,
    /// The comment is also the nearest comment of an earlier site: this one has none of its own.
    pub shares_comment_with: Option<usize>,
}

#[derive(Debug, Default)]
pub struct Analysis {
    pub sites: Vec<Site>,
    pub problems: Vec<String>,
    pub exemptions_total: usize,
    pub seeded: usize,
    pub judged: usize,
    pub bodyless: usize,
}

/// The whole analysis, pure over in-memory sources. `files` is `(path, text)`; `workspace_version` gates seeded exemptions.
pub fn analyse(
    files: &[(String, String)],
    exemptions: &[Exemption],
    workspace_version: &str,
) -> Analysis {
    let mut a = Analysis::default();
    // pass 1: sites per file, and the workspace's names
    let mut ws = Workspace::default();
    let mut per_file: Vec<(&str, &str, Vec<RawSite>)> = Vec::new();
    for (path, text) in files {
        let code = code_only(text);
        for l in code.lines() {
            let t = l.trim_start();
            let t = t.strip_prefix("pub ").unwrap_or(t);
            if let Some(rest) = t.strip_prefix("static ") {
                let rest = rest.strip_prefix("mut ").unwrap_or(rest);
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if name.len() > 1
                    && name
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                {
                    ws.statics.insert(name);
                }
            }
        }
        match discover(text) {
            Ok(s) => per_file.push((path, text, s)),
            Err(e) => a.problems.push(format!("{path}: {e}")),
        }
    }
    // callee categories: every `unsafe fn` site that carries a comment with valid tags
    for (_, text, sites) in &per_file {
        let lines: Vec<&str> = text.lines().collect();
        for s in sites.iter().filter(|s| s.kind == Kind::Fn) {
            if let Some((_, c)) = find_comment(&lines, s.line - 1)
                && let Some((tags, _)) = parse_tags(&c)
                && let Some(name) = &s.own_name
            {
                ws.unsafe_fns.entry(name.clone()).or_default().extend(tags);
            }
        }
    }
    // pass 2: judge
    let mut used_exemptions: BTreeSet<usize> = BTreeSet::new();
    for (path, text, sites) in &per_file {
        let lines: Vec<&str> = text.lines().collect();
        let mut ordinal: BTreeMap<String, usize> = BTreeMap::new();
        let mut owner_of_comment: BTreeMap<usize, usize> = BTreeMap::new();
        for raw in sites {
            let n = {
                let e = ordinal.entry(raw.scope.clone()).or_insert(0);
                *e += 1;
                *e
            };
            let found = find_comment(&lines, raw.line - 1);
            let (comment, tags, unknown, has_category, shares) = match &found {
                Some((idx, text)) => {
                    let shares = owner_of_comment.get(idx).copied();
                    owner_of_comment.entry(*idx).or_insert(raw.line);
                    match parse_tags(text) {
                        Some((t, u)) => (Some(text.clone()), t, u, true, shares),
                        None => (Some(text.clone()), vec![], vec![], false, shares),
                    }
                }
                None => (None, vec![], vec![], false, None),
            };
            // `impl` / `trait` / unrecognised forms have no operation to match: judged only on having a comment and a valid tag.
            // An attribute and an `extern` block are bodyless too, but each *names* its category (`symbol-export`, `linker-symbol`).
            let judged_shape = !matches!(raw.kind, Kind::Impl | Kind::Trait | Kind::Other);
            let unsupported: Vec<Category> = if judged_shape {
                tags.iter()
                    .copied()
                    .filter(|c| !supported(*c, raw.kind, &raw.body, &ws))
                    .collect()
            } else {
                Vec::new()
            };
            let ex = exemptions
                .iter()
                .position(|e| path.ends_with(e.file) && e.scope == raw.scope && e.n == n);
            if let Some(ix) = ex {
                used_exemptions.insert(ix);
            }
            a.sites.push(Site {
                file: (*path).to_string(),
                line: raw.line,
                kind: raw.kind,
                scope: raw.scope.clone(),
                n,
                body: raw.body.clone(),
                comment,
                tags,
                unknown_tags: unknown,
                has_category,
                unsupported,
                exempt: ex.map(|ix| exemptions[ix].kind.clone()),
                shares_comment_with: shares,
            });
        }
    }
    // verdicts
    for s in &a.sites {
        let at = format!(
            "{}:{} ({} `{}`#{})",
            s.file,
            s.line,
            s.kind.as_str(),
            s.scope,
            s.n
        );
        if s.comment.is_none() {
            a.problems
                .push(format!("{at}: no SAFETY comment of its own"));
            continue;
        }
        if let Some(first) = s.shares_comment_with {
            a.problems.push(format!("{at}: its nearest SAFETY comment is the one for the site at line {first} -- a site borrowing a neighbour's justification has none of its own"));
        }
        if !s.has_category {
            a.problems
                .push(format!("{at}: the SAFETY comment names no `category=`"));
        }
        for u in &s.unknown_tags {
            a.problems
                .push(format!("{at}: `category={u}` is not a known category"));
        }
        if s.kind.bodyless() {
            a.bodyless += 1;
        } else {
            a.judged += 1;
        }
        if !s.unsupported.is_empty() {
            match &s.exempt {
                Some(_) => {}
                None => a.problems.push(format!(
                    "{at}: tagged {:?} but its body shows no operation of that category and calls no `unsafe fn` carrying it \
                     (the body does contain: {}) -- re-tag it, correct the comment to describe what the site does, or exempt it by name with a reason",
                    s.unsupported.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                    describe_body(&s.body, &ws)
                )),
            }
        }
    }
    for (ix, e) in exemptions.iter().enumerate() {
        if !used_exemptions.contains(&ix) {
            a.problems.push(format!(
                "stale exemption `{}::{}`#{}: no such site exists any more",
                e.file, e.scope, e.n
            ));
            continue;
        }
        if e.reason.trim().is_empty() {
            a.problems.push(format!(
                "exemption `{}::{}`#{} has no reason",
                e.file, e.scope, e.n
            ));
        }
        // an exemption for a site that is now supported is stale: the list can only be as long as it is true
        let site = a
            .sites
            .iter()
            .find(|s| s.file.ends_with(e.file) && s.scope == e.scope && s.n == e.n);
        if let Some(s) = site
            && s.unsupported.is_empty()
            && s.comment.is_some()
        {
            a.problems.push(format!(
                "stale exemption `{}::{}`#{}: the site is supported now ({}:{}), so it is retired",
                e.file, e.scope, e.n, s.file, s.line
            ));
        }
        if let ExemptKind::Seeded { until } = &e.kind {
            a.seeded += 1;
            if let (Some(w), Some(u)) = (version_key(workspace_version), version_key(until))
                && w > u
            {
                a.problems.push(format!(
                    "seeded exemption `{}::{}`#{} outlived its milestone ({until}): the workspace is at {workspace_version}. \
                     Sweep it or make it Permanent with a reason",
                    e.file, e.scope, e.n
                ));
            }
        }
    }
    a.exemptions_total = exemptions.len();
    a
}

/// What a site's body does contain, so a failure can be read off: shapes found and unsafe callees with their categories.
pub fn describe_body(body: &str, ws: &Workspace) -> String {
    let mut parts = Vec::new();
    for c in [
        Category::CsrAsm,
        Category::MmioAccess,
        Category::KernelGlobalMutable,
        Category::RawPointerDeref,
        Category::PageTableMutation,
        Category::UserCopy,
        Category::PhysIdMapAssumption,
        Category::LinkerSymbol,
    ] {
        // direct shape only (no callee delegation), by asking `supported` with no callees known
        let empty = Workspace {
            statics: ws.statics.clone(),
            unsafe_fns: BTreeMap::new(),
        };
        if supported(c, Kind::Block, body, &empty) {
            parts.push(format!("a {} shape", c.as_str()));
        }
    }
    let cs: Vec<String> = callees(body)
        .into_iter()
        .filter_map(|n| {
            ws.unsafe_fns.get(&n).map(|cs| {
                format!(
                    "{n}() [{}]",
                    cs.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(",")
                )
            })
        })
        .collect();
    if !cs.is_empty() {
        parts.push(format!("calls unsafe fn {}", cs.join(", ")));
    }
    if parts.is_empty() {
        "no recognised shape and no unsafe callee".into()
    } else {
        parts.join("; ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Analysis {
        analyse(&[("a/x.rs".to_string(), src.to_string())], &[], "0.48.0")
    }
    fn run_with(src: &str, ex: &[Exemption], ver: &str) -> Analysis {
        analyse(&[("a/x.rs".to_string(), src.to_string())], ex, ver)
    }

    // ── the scanner: every form, and nothing in a string or comment ─────────────────────────────────────────────────────

    #[test]
    fn every_form_of_unsafe_is_a_site_including_the_ones_the_old_scanner_could_not_see() {
        let src = "#[unsafe(no_mangle)]\npub extern \"C\" fn a() {}\n\
                   unsafe impl<T> Sync for KS<T> {}\nunsafe impl Send for Y {}\n\
                   pub unsafe extern \"C\" fn b() { }\nunsafe extern \"C\" { static __x: u8; }\n\
                   unsafe trait Z {}\nfn f() { unsafe { g() } }\npub unsafe fn h() {}\n";
        let kinds: Vec<Kind> = discover(src).unwrap().iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Attr,
                Kind::Impl,
                Kind::Impl,
                Kind::Fn,
                Kind::ExternBlock,
                Kind::Trait,
                Kind::Block,
                Kind::Fn
            ]
        );
    }

    #[test]
    fn unsafe_in_a_string_a_comment_a_raw_string_or_an_identifier_is_not_a_site() {
        let src = "let s = \"unsafe { foo }\";\n// unsafe { bar }\n/* unsafe fn x() {} */\nlet r = r#\"unsafe fn y() {}\"#;\nlet unsafe_code = 1; let is_unsafe = 2;\n#![allow(unsafe_code)]\n";
        assert!(discover(src).unwrap().is_empty());
    }

    #[test]
    fn an_unbalanced_unsafe_block_is_an_error_not_a_skip() {
        assert!(discover("fn f() { unsafe { oops ").is_err());
    }

    // ── comment pairing ───────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn a_site_with_no_comment_is_a_problem_and_a_commented_one_is_not() {
        assert!(
            run("unsafe { *p }\n")
                .problems
                .iter()
                .any(|p| p.contains("no SAFETY comment of its own"))
        );
        let ok = run("// SAFETY: category=raw-pointer-deref ok\nunsafe { *p }\n");
        assert!(ok.problems.is_empty(), "{:?}", ok.problems);
    }

    #[test]
    fn the_comment_may_be_several_lines_above_through_comments_and_attributes_and_not_through_code()
    {
        let ok = run(
            "// SAFETY: category=raw-pointer-deref ok\n// more\n#[inline]\n// still\nunsafe { *p }\n",
        );
        assert!(ok.problems.is_empty(), "{:?}", ok.problems);
        let through_code =
            run("// SAFETY: category=raw-pointer-deref ok\nlet x = 1;\nunsafe { *p }\n");
        assert!(
            through_code
                .problems
                .iter()
                .any(|p| p.contains("no SAFETY comment"))
        );
    }

    #[test]
    fn two_sites_cannot_share_one_comment() {
        // stacked sites with one comment above them all: the first has it, the others have none of their own
        let a = run(
            "// SAFETY: category=symbol-export the entry\n#[unsafe(no_mangle)]\n#[unsafe(naked)]\npub extern \"C\" fn e() {}\n",
        );
        assert_eq!(
            a.problems
                .iter()
                .filter(|p| p.contains("borrowing a neighbour"))
                .count(),
            1,
            "{:?}",
            a.problems
        );
        // code between them breaks the block: the second site simply has no comment
        let b = run(
            "// SAFETY: category=raw-pointer-deref ok\nunsafe { *p }\n#[inline]\nunsafe { *q }\n",
        );
        assert!(
            b.problems.iter().any(|p| p.contains("no SAFETY comment")),
            "{:?}",
            b.problems
        );
    }

    #[test]
    fn category_parsing_handles_semicolons_commas_two_tags_and_unknowns() {
        assert_eq!(
            parse_tags("// SAFETY: category=csr-asm; the AES table")
                .unwrap()
                .0,
            vec![Category::CsrAsm]
        );
        assert_eq!(
            parse_tags("category=mmio-access, ordered").unwrap().0,
            vec![Category::MmioAccess]
        );
        let (t, u) = parse_tags("category=page-table-mutation,raw-pointer-deref text").unwrap();
        assert_eq!(
            t,
            vec![Category::PageTableMutation, Category::RawPointerDeref]
        );
        assert!(u.is_empty());
        assert_eq!(
            parse_tags("category=csr-asm,nonsense x").unwrap().1,
            vec!["nonsense".to_string()]
        );
        assert!(parse_tags("no tag here").is_none());
        assert!(
            run("// SAFETY: category=made-up why\nunsafe { x }\n")
                .problems
                .iter()
                .any(|p| p.contains("not a known category"))
        );
        assert!(
            run("// SAFETY: just prose\nunsafe { x }\n")
                .problems
                .iter()
                .any(|p| p.contains("names no `category=`"))
        );
    }

    // ── the rule: each category, planted ──────────────────────────────────────────────────────────────────────────────

    /// For every category, a body with none of its evidence is refused, naming the tag and what the body *does* contain.
    #[test]
    fn each_category_is_refused_on_a_body_that_shows_none_of_it() {
        for cat in [
            "csr-asm",
            "mmio-access",
            "kernel-global-mutable",
            "raw-pointer-deref",
            "page-table-mutation",
            "user-copy",
            "phys-id-map-assumption",
            "linker-symbol",
        ] {
            let a = run(&format!(
                "// SAFETY: category={cat} x\nunsafe {{ compute(1 + 2) }}\n"
            ));
            let p = a
                .problems
                .iter()
                .find(|p| p.contains(&format!("[\"{cat}\"]")));
            assert!(p.is_some(), "{cat}: {:?}", a.problems);
            assert!(p.unwrap().contains("the body does contain:"), "{cat}");
        }
    }

    #[test]
    fn each_category_is_accepted_on_a_body_that_shows_it() {
        let cases = [
            ("csr-asm", "unsafe { core::arch::asm!(\"ecall\") }"),
            ("mmio-access", "unsafe { p.write_volatile(1) }"),
            (
                "kernel-global-mutable",
                "unsafe { crate::get_kernel_state() }",
            ),
            ("raw-pointer-deref", "unsafe { &mut *tf }"),
            ("page-table-mutation", "unsafe { map_page(a, b) }"),
            ("user-copy", "unsafe { UserPtr::new(a) }"),
            ("phys-id-map-assumption", "unsafe { frame.pa() }"),
            (
                "linker-symbol",
                "unsafe { &__stack_top as *const u8 as usize }",
            ),
        ];
        for (cat, body) in cases {
            let a = run(&format!("// SAFETY: category={cat} x\n{body}\n"));
            assert!(a.problems.is_empty(), "{cat}: {:?}", a.problems);
        }
    }

    #[test]
    fn a_static_declared_in_the_workspace_makes_an_access_to_it_kernel_global_mutable() {
        let a = run(
            "static mut COUNTER: u32 = 0;\n// SAFETY: category=kernel-global-mutable single hart\nunsafe { COUNTER += 1 }\n",
        );
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        let b =
            run("// SAFETY: category=kernel-global-mutable single hart\nunsafe { OTHER += 1 }\n");
        assert!(!b.problems.is_empty(), "an undeclared name is not a static");
    }

    /// The finding: *"all user-supplied pointers are checked…"* on a `static mut` accessor. The tag is refused whatever the prose says.
    #[test]
    fn the_wrong_category_on_a_static_accessor_is_refused_which_is_the_defect_this_line_exists_for()
    {
        let src = "// SAFETY: category=user-copy all user-supplied pointers are checked against the task address space before dereferencing.\nlet lt = unsafe { crate::get_lease_table() };\n";
        let a = run(src);
        assert!(
            a.problems
                .iter()
                .any(|p| p.contains("[\"user-copy\"]") && p.contains("kernel-global-mutable")),
            "{:?}",
            a.problems
        );
    }

    #[test]
    fn a_call_to_an_unsafe_fn_of_the_same_category_supports_the_tag_and_a_different_one_does_not() {
        let defs = "// SAFETY: category=csr-asm reads sstatus\npub unsafe fn read_sstatus() -> usize { core::arch::asm!(\"csrr\") }\n";
        let ok = run(&format!(
            "{defs}// SAFETY: category=csr-asm masks SIE\nunsafe {{ read_sstatus() }}\n"
        ));
        assert!(ok.problems.is_empty(), "{:?}", ok.problems);
        let bad = run(&format!(
            "{defs}// SAFETY: category=kernel-global-mutable x\nunsafe {{ read_sstatus() }}\n"
        ));
        assert!(
            bad.problems
                .iter()
                .any(|p| p.contains("read_sstatus() [csr-asm]")),
            "{:?}",
            bad.problems
        );
    }

    #[test]
    fn two_tags_each_must_be_supported() {
        let a = run("// SAFETY: category=raw-pointer-deref,csr-asm x\nunsafe { *p }\n");
        assert!(
            a.problems
                .iter()
                .any(|p| p.contains("[\"csr-asm\"]") && !p.contains("raw-pointer-deref\"]")),
            "{:?}",
            a.problems
        );
    }

    #[test]
    fn impl_and_trait_have_no_body_and_are_judged_on_a_comment_and_a_tag_only() {
        let a =
            run("// SAFETY: category=kernel-global-mutable x\nunsafe impl<T> Sync for KS<T> {}\n");
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        assert_eq!((a.judged, a.bodyless), (0, 1));
        assert!(
            run("unsafe impl<T> Sync for KS<T> {}\n")
                .problems
                .iter()
                .any(|p| p.contains("no SAFETY comment")),
            "the site the old scanner could not see"
        );
    }

    #[test]
    fn an_attribute_must_say_symbol_export_and_an_extern_block_linker_symbol() {
        assert!(run("// SAFETY: category=symbol-export the entry\n#[unsafe(no_mangle)]\npub extern \"C\" fn e() {}\n").problems.is_empty());
        assert!(
            !run(
                "// SAFETY: category=csr-asm x\n#[unsafe(no_mangle)]\npub extern \"C\" fn e() {}\n"
            )
            .problems
            .is_empty()
        );
        assert!(run("// SAFETY: category=linker-symbol the script defines them\nunsafe extern \"C\" { static __x: u8; }\n").problems.is_empty());
        assert!(
            !run(
                "// SAFETY: category=raw-pointer-deref x\nunsafe extern \"C\" { static __x: u8; }\n"
            )
            .problems
            .is_empty()
        );
    }

    // ── exemptions ────────────────────────────────────────────────────────────────────────────────────────────────────

    const FLAGGED: &str = "// SAFETY: category=csr-asm x\nfn f() { unsafe { compute() } }\n";
    fn ex(kind: ExemptKind, reason: &'static str) -> Exemption {
        Exemption {
            file: "x.rs",
            scope: "f",
            n: 1,
            kind,
            reason,
        }
    }

    #[test]
    fn an_exempted_site_passes_and_is_counted_unjudged_and_a_seeded_one_is_counted_apart() {
        let a = run_with(
            FLAGGED,
            &[ex(
                ExemptKind::Seeded { until: "0.48.0" },
                "awaiting a sweep",
            )],
            "0.48.0",
        );
        assert!(a.problems.is_empty(), "{:?}", a.problems);
        assert_eq!((a.exemptions_total, a.seeded), (1, 1));
        let b = run_with(
            FLAGGED,
            &[ex(ExemptKind::Permanent, "a host stub")],
            "0.48.0",
        );
        assert_eq!((b.exemptions_total, b.seeded), (1, 0));
    }

    #[test]
    fn a_seeded_exemption_is_refused_once_the_workspace_is_past_its_milestone() {
        let e = [ex(
            ExemptKind::Seeded { until: "0.48.0" },
            "awaiting a sweep",
        )];
        assert!(run_with(FLAGGED, &e, "0.48.0").problems.is_empty());
        assert!(
            run_with(FLAGGED, &e, "0.49.0")
                .problems
                .iter()
                .any(|p| p.contains("outlived its milestone"))
        );
        assert!(
            run_with(FLAGGED, &e, "1.0.0")
                .problems
                .iter()
                .any(|p| p.contains("outlived its milestone"))
        );
    }

    #[test]
    fn an_exemption_for_a_supported_or_vanished_site_is_stale_and_one_with_no_reason_fails() {
        let supported =
            "// SAFETY: category=csr-asm x\nfn f() { unsafe { core::arch::asm!(\"nop\") } }\n";
        assert!(
            run_with(supported, &[ex(ExemptKind::Permanent, "r")], "0.48.0")
                .problems
                .iter()
                .any(|p| p.contains("stale") && p.contains("supported now"))
        );
        assert!(
            run_with("fn g() {}\n", &[ex(ExemptKind::Permanent, "r")], "0.48.0")
                .problems
                .iter()
                .any(|p| p.contains("no such site"))
        );
        assert!(
            run_with(FLAGGED, &[ex(ExemptKind::Permanent, "  ")], "0.48.0")
                .problems
                .iter()
                .any(|p| p.contains("has no reason"))
        );
    }

    #[test]
    fn a_site_with_no_comment_cannot_be_exempted_into_a_pass() {
        let a = run_with(
            "fn f() { unsafe { compute() } }\n",
            &[ex(ExemptKind::Permanent, "r")],
            "0.48.0",
        );
        assert!(
            a.problems.iter().any(|p| p.contains("no SAFETY comment")),
            "{:?}",
            a.problems
        );
    }

    // ── the real tree ─────────────────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn the_real_tree_passes_with_the_exemptions_it_carries_and_sees_the_sites_the_old_scanner_could_not()
     {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let paths =
            fjell_repo_scope::files_with_extension(&root, fjell_repo_scope::Scope::Working, "rs")
                .unwrap();
        let files: Vec<(String, String)> = paths
            .iter()
            .map(|p| {
                (
                    p.display().to_string().replace('\\', "/"),
                    std::fs::read_to_string(p).unwrap(),
                )
            })
            .collect();
        let a = analyse(&files, &crate::exempt::exemptions(), "0.48.0");
        assert!(a.problems.is_empty(), "{:#?}", a.problems);
        // the forms the old scanner missed are in the population (49 at the time of the finding; a floor, so a removal is not a failure)
        let attrs = a.sites.iter().filter(|s| s.kind == Kind::Attr).count();
        let externs = a
            .sites
            .iter()
            .filter(|s| {
                matches!(s.kind, Kind::ExternBlock)
                    || (s.kind == Kind::Fn
                        && s.comment.as_deref().is_some_and(|c| c.contains("extern")))
            })
            .count();
        assert!(
            attrs >= 30,
            "attribute sites are in the population: {attrs}"
        );
        assert!(
            a.sites
                .iter()
                .any(|s| s.kind == Kind::Impl && s.file.ends_with("fjell-kernel/src/main.rs")),
            "the `unsafe impl<T> Sync for KS<T>` is a site"
        );
        let _ = externs;
    }
}
