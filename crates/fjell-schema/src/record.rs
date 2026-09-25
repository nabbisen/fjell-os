//! The recording sink: runs an encoder and writes down what it wrote.
//!
//! Nothing here knows any format. A [`Recorder`] is a [`Canon`] whose every call
//! extends a **tree** of what was written, and [`record`] merges the trees of a *set*
//! of samples into the one description a `.frozen` file holds.
//!
//! **Why a set.** One encoder run takes one arm of every tagged union and one side of
//! every optional. A description of a union is the union of what its arms write, so a
//! format with such shapes is recorded from samples that reach every arm, and their
//! recordings are merged: nodes that must be the same in every run (fields, constants,
//! padding, notes) are checked to be, and the optional and variant arms are unioned. A
//! flat format is a set of one, and its file is what it always was.
//!
//! **What it cannot see, it refuses.** A counted group with no element in any sample,
//! an optional whose body no sample reaches, or two samples that disagree about a fixed
//! node, is a *problem* and generation fails: a description learned from an arm nobody
//! exercised would be a guess.

use fjell_canon::{Canon, Structure};

/// One node of a recording.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Node {
    Domain(String),
    Magic(String),
    Field {
        name: String,
        ty: String,
    },
    Zeros {
        name: String,
        n: usize,
    },
    Note {
        key: String,
        value: String,
    },
    /// A counted group. `body` is `None` until some element has been seen.
    Group {
        name: String,
        max: Option<usize>,
        body: Option<Vec<Node>>,
    },
    /// An optional value: its presence byte, and the body once some sample has it.
    Optional {
        name: String,
        body: Option<Vec<Node>>,
    },
    /// A tagged union: its tag byte, and each arm some sample took.
    Choice {
        name: String,
        arms: Vec<Arm>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Arm {
    tag: u8,
    label: String,
    body: Vec<Node>,
}

/// What is being recorded into, innermost last.
enum Frame {
    /// A scope: names get a `name.` prefix, nothing else changes.
    Scope,
    Optional {
        name: String,
        present: bool,
        body: Vec<Node>,
    },
    Variant {
        name: String,
        tag: u8,
        label: String,
        body: Vec<Node>,
    },
}

#[derive(Default)]
pub struct Recorder {
    /// The top level, and (while a group element is being recorded) nothing else:
    /// group elements use a sub-recorder.
    nodes: Vec<Node>,
    stack: Vec<Frame>,
    problems: Vec<String>,
    /// `records[].` while inside the group `records`, then scopes: `records[].title.`.
    prefix: String,
    /// The prefix each open frame started from, to restore on `Leave`.
    saved: Vec<String>,
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

    /// Where a node goes: into the innermost open optional/variant body (a scope has
    /// no body of its own, so the search passes over it), or the top level.
    fn out(&mut self) -> &mut Vec<Node> {
        let at = self.stack.iter().rposition(|f| !matches!(f, Frame::Scope));
        match at {
            Some(i) => match &mut self.stack[i] {
                Frame::Optional { body, .. } | Frame::Variant { body, .. } => body,
                Frame::Scope => unreachable!("filtered above"),
            },
            None => &mut self.nodes,
        }
    }

    fn field(&mut self, name: &str, ty: &str) {
        let node = Node::Field {
            name: format!("{}{name}", self.prefix),
            ty: ty.into(),
        };
        self.out().push(node);
    }

    /// Fold `other`'s finished recording into this one (the merge across samples).
    fn merge_top(&mut self, other: Recorder) {
        let mut problems = std::mem::take(&mut self.problems);
        problems.extend(other.problems);
        let mut mine = std::mem::take(&mut self.nodes);
        if mine.is_empty() {
            mine = other.nodes;
        } else {
            merge(&mut mine, other.nodes, &mut problems, "the top level");
        }
        self.nodes = mine;
        self.problems = problems;
    }

    /// The description's body lines, once every sample has been merged. Also reports
    /// what was never observed.
    pub fn body(&self) -> String {
        let w = width(&self.nodes);
        let mut out = String::new();
        render(&self.nodes, 0, w, &mut out);
        out
    }

    /// Unobserved arms, as problems.
    pub fn unobserved(&self) -> Vec<String> {
        let mut p = Vec::new();
        check_observed(&self.nodes, &mut p, "");
        p
    }
}

/// Merge `b` into `a`. Fixed nodes must agree; optional and variant arms are unioned;
/// a group's element shape is merged like everything else.
fn merge(a: &mut Vec<Node>, b: Vec<Node>, problems: &mut Vec<String>, at: &str) {
    if a.len() != b.len() {
        problems.push(format!(
            "in {at}: two samples write a different number of parts ({} and {}), so one \
             description cannot be true of both",
            a.len(),
            b.len()
        ));
        return;
    }
    for (x, y) in a.iter_mut().zip(b) {
        merge_node(x, y, problems, at);
    }
}

fn merge_bodies(
    a: &mut Option<Vec<Node>>,
    b: Option<Vec<Node>>,
    problems: &mut Vec<String>,
    at: &str,
) {
    match (a.as_mut(), b) {
        (_, None) => {}
        (None, Some(b)) => *a = Some(b),
        (Some(a), Some(b)) => merge(a, b, problems, at),
    }
}

fn merge_node(x: &mut Node, y: Node, problems: &mut Vec<String>, at: &str) {
    match (&mut *x, y) {
        (
            Node::Group { name, max, body },
            Node::Group {
                name: n2,
                max: m2,
                body: b2,
            },
        ) if *name == n2 && *max == m2 => merge_bodies(body, b2, problems, name),
        (
            Node::Optional { name, body },
            Node::Optional {
                name: n2, body: b2, ..
            },
        ) if *name == n2 => merge_bodies(body, b2, problems, name),
        (Node::Choice { name, arms }, Node::Choice { name: n2, arms: a2 }) if *name == n2 => {
            for arm in a2 {
                match arms.iter_mut().find(|a| a.label == arm.label) {
                    Some(existing) => {
                        if existing.tag != arm.tag {
                            problems.push(format!(
                                "in `{name}`: the arm `{}` has tag {} in one sample and {} in another",
                                arm.label, existing.tag, arm.tag
                            ));
                        } else {
                            merge(&mut existing.body, arm.body, problems, &arm.label);
                        }
                    }
                    None => arms.push(arm),
                }
            }
            arms.sort_by_key(|a| a.tag);
        }
        (a, b) if *a == b => {}
        (a, b) => problems.push(format!(
            "in {at}: two samples disagree about a fixed part: {} versus {}",
            describe(a),
            describe(&b)
        )),
    }
}

fn describe(n: &Node) -> String {
    match n {
        Node::Domain(s) => format!("domain {s}"),
        Node::Magic(s) => format!("magic {s}"),
        Node::Field { name, ty } => format!("field {name} {ty}"),
        Node::Zeros { name, n } => format!("zeros {name} [{n}]"),
        Node::Note { key, value } => format!("note {key} {value}"),
        Node::Group { name, .. } => format!("group {name}"),
        Node::Optional { name, .. } => format!("optional {name}"),
        Node::Choice { name, .. } => format!("choice {name}"),
    }
}

/// Every group and optional must have been observed with a body by some sample.
fn check_observed(nodes: &[Node], problems: &mut Vec<String>, at: &str) {
    for n in nodes {
        match n {
            Node::Group { name, body, .. } => match body {
                None => problems.push(format!(
                    "group `{name}` has no element in any sample, so its shape was not observed"
                )),
                Some(b) => check_observed(b, problems, name),
            },
            Node::Optional { name, body } => match body {
                None => problems.push(format!(
                    "optional `{name}` was absent in every sample, so its body was not observed"
                )),
                Some(b) => check_observed(b, problems, name),
            },
            Node::Choice { arms, .. } => {
                for a in arms {
                    check_observed(&a.body, problems, &a.label);
                }
            }
            _ => {
                let _ = at;
            }
        }
    }
}

fn width(nodes: &[Node]) -> usize {
    nodes
        .iter()
        .map(|n| match n {
            Node::Field { name, .. } | Node::Zeros { name, .. } => name.len(),
            Node::Group { body: Some(b), .. } | Node::Optional { body: Some(b), .. } => width(b),
            Node::Choice { arms, .. } => arms.iter().map(|a| width(&a.body)).max().unwrap_or(0),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Group bodies render flat (their fields carry the group's name); optional and variant
/// bodies indent, so the structure a reader must see — *this only when that* — shows.
fn render(nodes: &[Node], depth: usize, w: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    for n in nodes {
        match n {
            Node::Domain(s) => out.push_str(&format!("{pad}domain {s}\n")),
            Node::Magic(s) => out.push_str(&format!("{pad}magic {s}\n")),
            Node::Field { name, ty } => out.push_str(&format!("{pad}field {name:<w$}  {ty}\n")),
            Node::Zeros { name, n } => {
                out.push_str(&format!("{pad}zeros {name:<w$}  u8 [{n}] = 0\n"))
            }
            Node::Note { key, value } => out.push_str(&format!("{pad}note {key} {value}\n")),
            Node::Group { name, max, body } => {
                match max {
                    Some(m) => out.push_str(&format!("{pad}group {name} max {m}\n")),
                    None => out.push_str(&format!("{pad}group {name}\n")),
                }
                if let Some(b) = body {
                    render(b, depth, w, out);
                }
            }
            Node::Optional { name, body } => {
                out.push_str(&format!(
                    "{pad}optional {name}  u8 presence (0 = absent, 1 = present)\n"
                ));
                if let Some(b) = body {
                    render(b, depth + 1, w, out);
                }
            }
            Node::Choice { name, arms } => {
                out.push_str(&format!("{pad}choice {name}  u8 tag\n"));
                for a in arms {
                    out.push_str(&format!("{pad}  variant {} tag={}\n", a.label, a.tag));
                    render(&a.body, depth + 2, w, out);
                }
            }
        }
    }
}

/// Record a format from a set of samples and merge them. `Err` lists every problem.
pub fn record(samples: &[fn(&mut dyn Canon)]) -> Result<String, Vec<String>> {
    let mut all = Recorder::new();
    for (i, s) in samples.iter().enumerate() {
        let mut one = Recorder::new();
        s(&mut one);
        if i == 0 {
            all = one;
        } else {
            all.merge_top(one);
        }
    }
    let mut problems = all.problems().to_vec();
    problems.extend(all.unobserved());
    if problems.is_empty() {
        Ok(all.body())
    } else {
        Err(problems)
    }
}

impl Canon for Recorder {
    fn domain(&mut self, tag: &[u8]) {
        self.out().push(Node::Domain(quote(tag)));
    }
    fn magic(&mut self, tag: &[u8]) {
        self.out().push(Node::Magic(quote(tag)));
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
    fn i64(&mut self, name: &'static str, _: i64) {
        self.field(name, "i64 LE");
    }
    fn bytes(&mut self, name: &'static str, v: &[u8]) {
        self.field(name, &format!("u8 [{}]", v.len()));
    }
    fn var_bytes(&mut self, name: &'static str, _: &[u8], len: &'static str) {
        let ty = format!("u8 [{}{len}]", self.prefix);
        self.field(name, &ty);
    }
    fn zeros(&mut self, name: &'static str, n: usize) {
        let node = Node::Zeros {
            name: format!("{}{name}", self.prefix),
            n,
        };
        self.out().push(node);
    }
    fn each(
        &mut self,
        name: &'static str,
        count: usize,
        max: Option<usize>,
        f: &mut dyn FnMut(&mut dyn Canon, usize),
    ) {
        let inner = format!("{}{name}[].", self.prefix);
        let mut body: Option<Vec<Node>> = None;
        for i in 0..count {
            let mut sub = Recorder {
                prefix: inner.clone(),
                ..Recorder::default()
            };
            f(&mut sub, i);
            self.problems.append(&mut sub.problems);
            match &mut body {
                None => body = Some(sub.nodes),
                Some(b) => merge(b, sub.nodes, &mut self.problems, name),
            }
        }
        let node = Node::Group {
            name: format!("{}{name}", self.prefix),
            max,
            body,
        };
        self.out().push(node);
    }
    fn note(&mut self, key: &'static str, value: &str) {
        self.out().push(Node::Note {
            key: key.into(),
            value: value.into(),
        });
    }
    fn structure(&mut self, ev: Structure) {
        match ev {
            Structure::EnterScope(name) => {
                self.saved.push(self.prefix.clone());
                self.prefix = format!("{}{name}.", self.prefix);
                self.stack.push(Frame::Scope);
            }
            Structure::EnterOptional { name, present } => {
                // The presence byte was just recorded as a plain field: it becomes the node.
                self.out().pop();
                self.saved.push(self.prefix.clone());
                self.prefix = format!("{}{name}.", self.prefix);
                self.stack.push(Frame::Optional {
                    name: format!("{}{name}", self.saved.last().unwrap()),
                    present,
                    body: Vec::new(),
                });
            }
            Structure::EnterVariant { name, tag, label } => {
                self.out().pop(); // the tag byte
                self.saved.push(self.prefix.clone());
                self.prefix = format!("{}{label}.", self.prefix);
                self.stack.push(Frame::Variant {
                    name: format!("{}{name}", self.saved.last().unwrap()),
                    tag,
                    label: label.into(),
                    body: Vec::new(),
                });
            }
            Structure::Leave => {
                let frame = self.stack.pop();
                self.prefix = self.saved.pop().unwrap_or_default();
                match frame {
                    Some(Frame::Optional {
                        name,
                        present,
                        body,
                    }) => {
                        let node = Node::Optional {
                            name,
                            body: present.then_some(body),
                        };
                        self.out().push(node);
                    }
                    Some(Frame::Variant {
                        name,
                        tag,
                        label,
                        body,
                    }) => {
                        let node = Node::Choice {
                            name,
                            arms: vec![Arm { tag, label, body }],
                        };
                        self.out().push(node);
                    }
                    Some(Frame::Scope) | None => {}
                }
            }
        }
    }
}
