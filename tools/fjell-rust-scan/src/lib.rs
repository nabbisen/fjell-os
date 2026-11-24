//! The scanner two tools had each grown a copy of (RFC-0.48-002 D5): `authority-funnel` (RFC-0.47-001) and, until now, a
//! line-substring scan in `fjell-unsafe-audit` that could not see 49 of the tree's `unsafe` tokens.
//!
//! **Tokenising and extraction only -- no verdicts.** This crate has no function that says a thing is supported, missing or
//! allowed, and it must not grow one: *sharing a scanner must not become sharing a judgement*. A tool decides what a site
//! means; this crate says only where the code is, with comments, strings and character literals blanked (byte offsets and
//! newlines preserved, so a line number in the blanked text is the line number in the source).
//!
//! A function it cannot delimit is an `Err`, never a skip: a scanner that quietly skipped what it could not read would let
//! every caller vouch for nothing.

/// Blank comments, string literals and character literals (keeping every newline and byte offset), so what
/// remains is code. Raw strings and nested block comments are handled; a lifetime is not a character literal.
pub fn code_only(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let n = b.len();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for k in from..to.min(out.len()) {
            if out[k] != b'\n' {
                out[k] = b' ';
            }
        }
    };
    while i < n {
        match b[i] {
            b'/' if i + 1 < n && b[i + 1] == b'/' => {
                let s = i;
                while i < n && b[i] != b'\n' {
                    i += 1;
                }
                blank(&mut out, s, i);
            }
            b'/' if i + 1 < n && b[i + 1] == b'*' => {
                let s = i;
                let mut depth = 0;
                while i < n {
                    if i + 1 < n && b[i] == b'/' && b[i + 1] == b'*' {
                        depth += 1;
                        i += 2;
                    } else if i + 1 < n && b[i] == b'*' && b[i + 1] == b'/' {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                blank(&mut out, s, i);
            }
            b'r' if i + 1 < n
                && (b[i + 1] == b'"' || b[i + 1] == b'#')
                && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) =>
            {
                // raw string r"..." or r#"..."#
                let mut j = i + 1;
                let mut hashes = 0;
                while j < n && b[j] == b'#' {
                    hashes += 1;
                    j += 1;
                }
                if j < n && b[j] == b'"' {
                    let s = i;
                    j += 1;
                    loop {
                        if j >= n {
                            break;
                        }
                        if b[j] == b'"'
                            && (0..hashes).all(|h| j + 1 + h < n && b[j + 1 + h] == b'#')
                        {
                            j += 1 + hashes;
                            break;
                        }
                        j += 1;
                    }
                    blank(&mut out, s, j);
                    i = j;
                } else {
                    i += 1;
                }
            }
            b'"' => {
                let s = i;
                i += 1;
                while i < n && b[i] != b'"' {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i = (i + 1).min(n);
                blank(&mut out, s, i);
            }
            b'\'' => {
                // a character literal ('x' or '\n') -- or a lifetime ('a), which has no closing quote.
                if i + 2 < n && b[i + 1] == b'\\' {
                    let s = i;
                    let mut j = i + 2;
                    while j < n && b[j] != b'\'' {
                        j += 1;
                    }
                    i = (j + 1).min(n);
                    blank(&mut out, s, i);
                } else if i + 2 < n && b[i + 2] == b'\'' && b[i + 1] != b'\'' {
                    blank(&mut out, i, i + 3);
                    i += 3;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

/// A function with the (1-based) line span of its body.
#[derive(Debug, Clone)]
pub struct Func {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    /// byte span of the body in the code-only text
    pub body: (usize, usize),
}

fn line_of(offsets: &[usize], pos: usize) -> usize {
    match offsets.binary_search(&pos) {
        Ok(i) => i + 1,
        Err(i) => i,
    }
}

/// Every `fn name(...) { ... }` in `code` (code-only text), with its body span. Trait method declarations
/// (`fn f();`) have no body and are skipped. `Err` names a function whose braces do not balance.
pub fn functions(code: &str) -> Result<Vec<Func>, String> {
    let b = code.as_bytes();
    let n = b.len();
    let mut line_starts = vec![0usize];
    for (i, c) in b.iter().enumerate() {
        if *c == b'\n' {
            line_starts.push(i + 1);
        }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < n {
        let boundary_before = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if boundary_before && &b[i..i + 3] == b"fn " {
            let mut j = i + 3;
            while j < n && b[j] == b' ' {
                j += 1;
            }
            let ns = j;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            if j == ns {
                i += 1;
                continue;
            }
            let name = String::from_utf8_lossy(&b[ns..j]).to_string();
            // the signature ends at the first `{` (a body) or `;` (a declaration) outside () [] <>
            let mut paren = 0i32;
            let mut k = j;
            let mut open = None;
            while k < n {
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
            if let Some(o) = open {
                let mut depth = 0i32;
                let mut m = o;
                let mut close = None;
                while m < n {
                    match b[m] {
                        b'{' => depth += 1,
                        b'}' => {
                            depth -= 1;
                            if depth == 0 {
                                close = Some(m);
                                break;
                            }
                        }
                        _ => {}
                    }
                    m += 1;
                }
                let Some(c) = close else {
                    return Err(format!(
                        "function `{name}` at line {}: braces do not balance, so its body cannot be delimited",
                        line_of(&line_starts, i)
                    ));
                };
                out.push(Func {
                    name,
                    start_line: line_of(&line_starts, i),
                    end_line: line_of(&line_starts, c),
                    body: (o, c),
                });
                i = j; // nested functions are found by continuing inside the body
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    Ok(out)
}

/// 1-based line ranges of `#[cfg(test)] mod name { ... }` blocks in code-only text: host unit tests, which
/// call the lease table and test rights directly and are not production authority decisions.
pub fn test_module_ranges(code: &str) -> Vec<(usize, usize)> {
    let b = code.as_bytes();
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(i) = code[from..].find("#[cfg(test)]") {
        let at = from + i;
        from = at + 1;
        let rest = &code[at + "#[cfg(test)]".len()..];
        let t = rest.trim_start();
        if !t.starts_with("mod ") && !t.starts_with("pub mod ") {
            continue;
        }
        let Some(open_rel) = rest.find('{') else {
            continue;
        };
        let open = at + "#[cfg(test)]".len() + open_rel;
        let mut depth = 0i32;
        let mut close = None;
        for (k, c) in b.iter().enumerate().skip(open) {
            match c {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(k);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(c) = close {
            let line = |pos: usize| code[..pos].matches('\n').count() + 1;
            ranges.push((line(at), line(c)));
        }
    }
    ranges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_strings_and_chars_are_blanked_and_offsets_and_newlines_survive() {
        let src = "let a = 1; // unsafe { x }\nlet b = \"unsafe { y }\"; /* unsafe fn z */ let c = '{';\nunsafe { real }\n";
        let code = code_only(src);
        assert_eq!(code.len(), src.len());
        assert_eq!(code.matches('\n').count(), src.matches('\n').count());
        assert_eq!(code.matches("unsafe").count(), 1, "{code:?}");
        assert!(code.contains("unsafe { real }"));
    }

    #[test]
    fn raw_strings_and_nested_block_comments_are_blanked() {
        let src = "let r = r#\"unsafe { }\"#; /* a /* nested unsafe */ still */ unsafe { k }";
        assert_eq!(code_only(src).matches("unsafe").count(), 1);
    }

    #[test]
    fn a_lifetime_is_not_a_character_literal() {
        let code = code_only("fn f<'a>(x: &'a str) -> &'a str { unsafe { x } }");
        assert!(code.contains("unsafe { x }"), "{code:?}");
    }

    #[test]
    fn functions_are_delimited_with_their_line_spans_and_unbalanced_braces_are_an_error() {
        let code = code_only(
            "fn a() {\n    let x = \"}}\";\n}\nfn b();\npub unsafe extern \"C\" fn c() { }\n",
        );
        let f = functions(&code).unwrap();
        let names: Vec<&str> = f.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            ["a", "c"],
            "a bodiless declaration is not a function"
        );
        assert_eq!((f[0].start_line, f[0].end_line), (1, 3));
        assert!(functions(&code_only("fn broken() {\n if x {\n}\n")).is_err());
    }

    #[test]
    fn test_modules_are_found() {
        let code = code_only("fn p() {}\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\nfn q() {}\n");
        assert_eq!(test_module_ranges(&code), vec![(2, 5)]);
    }
}
