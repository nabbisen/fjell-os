//! The recording sink: runs an encoder and writes down what it wrote.
//!
//! Nothing here knows any format. A [`Recorder`] is a [`Canon`] whose every call
//! appends a line, so the description of a layout is whatever its encoder does.
//!
//! **What it cannot see, it refuses.** A counted group whose sample has no
//! element, or whose elements do not all have the same shape, is recorded as a
//! *problem* and generation fails: a description learned from an element that does
//! not exist, or from the first of several different ones, would be a guess.

use fjell_canon::Canon;

/// One recorded line, before column alignment.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Line {
    Domain(String),
    Magic(String),
    Field { name: String, ty: String },
    Zeros { name: String, n: usize },
    Group { name: String, max: Option<usize> },
    Note { key: String, value: String },
}

#[derive(Default)]
pub struct Recorder {
    lines: Vec<Line>,
    problems: Vec<String>,
    /// `records[].` while inside the group `records`; empty at the top level.
    prefix: String,
}

fn quote(b: &[u8]) -> String {
    let mut s = String::from("\"");
    for &c in b {
        match c {
            b'"' => s.push_str("\\\""),
            b'\\' => s.push_str("\\\\"),
            0x20..=0x7e => s.push(c as char),
            _ => s.push_str(&format!("\\x{c:02x}")),
        }
    }
    s.push('"');
    s
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything the recorder could not honestly describe.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    fn field(&mut self, name: &str, ty: &str) {
        self.lines.push(Line::Field {
            name: format!("{}{name}", self.prefix),
            ty: ty.into(),
        });
    }

    /// The body lines, names padded to one column. A group's fields carry the
    /// group's name (`records[].kind`), so every line stands alone.
    pub fn body(&self) -> String {
        let w = self
            .lines
            .iter()
            .filter_map(|l| match l {
                Line::Field { name, .. } | Line::Zeros { name, .. } => Some(name.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let mut out = String::new();
        for l in &self.lines {
            match l {
                Line::Domain(s) => out.push_str(&format!("domain {s}\n")),
                Line::Magic(s) => out.push_str(&format!("magic {s}\n")),
                Line::Field { name, ty } => out.push_str(&format!("field {name:<w$}  {ty}\n")),
                Line::Zeros { name, n } => {
                    out.push_str(&format!("zeros {name:<w$}  u8 [{n}] = 0\n"))
                }
                Line::Group { name, max: Some(m) } => {
                    out.push_str(&format!("group {name} max {m}\n"))
                }
                Line::Group { name, max: None } => out.push_str(&format!("group {name}\n")),
                Line::Note { key, value } => out.push_str(&format!("note {key} {value}\n")),
            }
        }
        out
    }
}

impl Canon for Recorder {
    fn domain(&mut self, tag: &[u8]) {
        self.lines.push(Line::Domain(quote(tag)));
    }
    fn magic(&mut self, tag: &[u8]) {
        self.lines.push(Line::Magic(quote(tag)));
    }
    fn u8(&mut self, name: &'static str, _: u8) {
        self.field(name, "u8");
    }
    fn constant(&mut self, name: &'static str, v: u8) {
        self.field(name, &format!("u8 = {v:#04x}"));
    }
    fn u16(&mut self, name: &'static str, _: u16) {
        self.field(name, "u16 LE");
    }
    fn u32(&mut self, name: &'static str, _: u32) {
        self.field(name, "u32 LE");
    }
    fn u64(&mut self, name: &'static str, _: u64) {
        self.field(name, "u64 LE");
    }
    fn bytes(&mut self, name: &'static str, v: &[u8]) {
        self.field(name, &format!("u8 [{}]", v.len()));
    }
    fn var_bytes(&mut self, name: &'static str, _: &[u8], len: &'static str) {
        self.field(name, &format!("u8 [{}{len}]", self.prefix));
    }
    fn zeros(&mut self, name: &'static str, n: usize) {
        self.lines.push(Line::Zeros {
            name: format!("{}{name}", self.prefix),
            n,
        });
    }
    fn each(
        &mut self,
        name: &'static str,
        count: usize,
        max: Option<usize>,
        f: &mut dyn FnMut(&mut dyn Canon, usize),
    ) {
        let inner = format!("{}{name}[].", self.prefix);
        self.lines.push(Line::Group {
            name: format!("{}{name}", self.prefix),
            max,
        });
        if count == 0 {
            self.problems.push(format!(
                "group `{name}` has no element in the sample, so its shape was not observed"
            ));
            return;
        }
        let mut first: Option<Vec<Line>> = None;
        for i in 0..count {
            let mut sub = Recorder {
                prefix: inner.clone(),
                ..Recorder::default()
            };
            f(&mut sub, i);
            self.problems.append(&mut sub.problems);
            match &first {
                None => first = Some(sub.lines),
                Some(shape) if *shape != sub.lines => self.problems.push(format!(
                    "group `{name}`: element {i} has a different shape from element 0, so one \
                     description cannot be true of both"
                )),
                Some(_) => {}
            }
        }
        self.lines.extend(first.unwrap());
    }
    fn note(&mut self, key: &'static str, value: &str) {
        self.lines.push(Line::Note {
            key: key.into(),
            value: value.into(),
        });
    }
}
