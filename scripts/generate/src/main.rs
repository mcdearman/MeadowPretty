//! Writes `src/Cases.mw` for pretty.
//!
//! ```text
//! cargo run --release -- <package root>
//! ```
//!
//! Random documents — every builder, column and nesting functions, unions,
//! annotations and blocks — with what the crate renders for them in a few
//! widths, annotations included. The library is ported by hand into `src/`,
//! and the crate's source is fingerprinted.

use pretty::block::{Affixes, BlockDoc};
use pretty::{DocAllocator, DocBuilder, RcAllocator, RcDoc, Render, RenderAnnotated};
use std::fmt::Write as _;
use std::path::PathBuf;

/// The crate version pinned in `Cargo.toml`.
const UPSTREAM_VERSION: &str = "0.12.5";

/// The fingerprint of the sources `src/` ports.
const SOURCES: u64 = 0x532d_a380_292d_a8a7;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "../..".into()));

    let print = fingerprint(include_str!(concat!(env!("OUT_DIR"), "/sources.rs.txt")));
    if print != SOURCES {
        eprintln!(
            "error: pretty is not the version src/ ports.\n\
             Compare its source in {} with the previous version, carry any change\n\
             into src/, then set SOURCES in scripts/generate/src/main.rs to\n\
             {print:#x}",
            env!("UPSTREAM_DIR")
        );
        std::process::exit(1);
    }

    let cases = cases();
    let path = root.join("src/Cases.mw");
    std::fs::write(&path, &cases).unwrap();
    eprintln!("wrote {} ({} bytes)", path.display(), cases.len());
}

/// FNV-1a: stable across builds, which `DefaultHasher` does not promise.
fn fingerprint(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// --- encoding -----------------------------------------------------------------------

/// A number as `digits` base-64 digits, most significant first, each digit the
/// character `'0' + d`: `'0'` to `'o'`, one contiguous run of ASCII.
fn digits(out: &mut String, value: u64, digits: u32) {
    assert!(
        value < 1 << (6 * digits),
        "{value} does not fit in {digits} digits"
    );
    for k in (0..digits).rev() {
        out.push(char::from(b'0' + ((value >> (6 * k)) & 63) as u8));
    }
}

/// A signed number, offset to fit `digits` digits.
fn signed(out: &mut String, value: i64, n: u32) {
    let half = 1i64 << (6 * n - 1);
    assert!((-half..half).contains(&value), "{value} does not fit");
    digits(out, (value + half) as u64, n);
}

/// A string, as its length in bytes (3 digits) and then its bytes.
fn text(out: &mut String, s: &str) {
    digits(out, s.len() as u64, 3);
    out.push_str(s);
}

fn flag(out: &mut String, b: bool) {
    digits(out, u64::from(b), 1);
}

/// `text` as one Meadow string literal, broken with `\`-newline every `width`
/// characters. Printable ASCII and CJK are written raw, and everything else
/// escaped; a space that would start a line is `\x20`, since a continuation
/// drops leading whitespace.
fn long_literal(text: &str, width: usize) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / width * 4 + 2);
    out.push('"');
    for (i, c) in text.chars().enumerate() {
        let line_start = i > 0 && i % width == 0;
        if line_start {
            out.push_str("\\\n    ");
        }
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            ' ' if line_start => out.push_str("\\x20"),
            ' '..='~' => out.push(c),
            '\u{3040}'..='\u{30FF}' | '\u{4E00}'..='\u{9FFF}' => out.push(c),
            _ => {
                let _ = write!(out, "\\u{{{:X}}}", u32::from(c));
            }
        }
    }
    out.push('"');
    out
}

// --- inputs -------------------------------------------------------------------------

/// A small deterministic generator, so that the cases are the same on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[self.below(xs.len() as u64) as usize]
    }
}

static ALLOC: RcAllocator = RcAllocator;

type B = DocBuilder<'static, RcAllocator, u32>;

const TEXTS: &[&str] = &[
    "a",
    "foo",
    "hello world",
    "fn",
    "(",
    ")",
    ",",
    "日本",
    "e\u{301}",
    "😀x",
    " ",
    "",
    "x\ny",
    "longer_identifier",
    "\t",
];

const PROSE: &[&str] = &[
    "the quick brown fox jumps over the lazy dog",
    "a  b\tc\nd",
    "",
    " leading and trailing ",
    "日本語 の テキスト",
    "one",
];

/// What a column, nesting or width function does with the number it is
/// given, `x`: `k` is its parameter and `d` its document.
fn apply(kind: u64, k: i64, d: B, x: i64) -> RcDoc<'static, u32> {
    match kind {
        0 => d.append(ALLOC.as_string(x)).into_doc(),
        1 => {
            if x > k {
                ALLOC.hardline().into_doc()
            } else {
                d.into_doc()
            }
        }
        2 => d.nest((k - x) as isize).into_doc(),
        _ => {
            if x % 2 == 0 {
                d.into_doc()
            } else {
                d.group().into_doc()
            }
        }
    }
}

/// A random document of at most `depth` levels, written as the tree of calls
/// that builds it.
fn random_doc(rng: &mut Rng, out: &mut String, depth: u32) -> B {
    // Leaves are tags 0 to 8, of which 8 (`fail`) is kept rare: a document
    // with one outside a union cannot be rendered at all.
    let leaf = |rng: &mut Rng| {
        if rng.chance(4) { 8 } else { rng.below(8) }
    };
    let tag = if depth == 0 || rng.chance(25) {
        leaf(rng)
    } else {
        9 + rng.below(21)
    };
    digits(out, tag, 1);
    let sub = |rng: &mut Rng, out: &mut String| random_doc(rng, out, depth.saturating_sub(1));
    match tag {
        0 => ALLOC.nil(),
        1 => {
            let t = rng.pick(TEXTS);
            text(out, t);
            ALLOC.text(t)
        }
        2 => ALLOC.hardline(),
        3 => ALLOC.space(),
        4 => ALLOC.line(),
        5 => ALLOC.line_(),
        6 => ALLOC.softline(),
        7 => ALLOC.softline_(),
        8 => ALLOC.fail(),
        9 => {
            let a = sub(rng, out);
            let b = sub(rng, out);
            a.append(b)
        }
        10 => sub(rng, out).group(),
        11 => {
            let k = rng.below(13) as i64 - 4;
            signed(out, k, 2);
            sub(rng, out).nest(k as isize)
        }
        12 => {
            let a = sub(rng, out);
            let b = sub(rng, out);
            a.flat_alt(b)
        }
        13 => {
            let n = rng.below(10);
            digits(out, n, 1);
            sub(rng, out).annotate(n as u32)
        }
        14 => {
            let a = sub(rng, out);
            let b = sub(rng, out);
            a.union(b)
        }
        15 => sub(rng, out).align(),
        16 => {
            let k = rng.below(13) as i64 - 4;
            signed(out, k, 2);
            sub(rng, out).hang(k as isize)
        }
        17 => {
            let k = if rng.chance(10) {
                95 + rng.below(20)
            } else {
                rng.below(6)
            };
            digits(out, k, 2);
            sub(rng, out).indent(k as usize)
        }
        18 => {
            let kind = rng.below(4);
            let k = rng.below(21) as i64 - 4;
            digits(out, kind, 1);
            signed(out, k, 2);
            let a = sub(rng, out);
            let b = sub(rng, out);
            a.width(move |w| apply(kind, k, b.clone(), w as i64))
        }
        19 => {
            let which = rng.below(6);
            digits(out, which, 1);
            let d = sub(rng, out);
            match which {
                0 => d.single_quotes(),
                1 => d.double_quotes(),
                2 => d.parens(),
                3 => d.angles(),
                4 => d.braces(),
                _ => d.brackets(),
            }
        }
        20 => {
            let d = sub(rng, out);
            let before = sub(rng, out);
            let after = sub(rng, out);
            d.enclose(before, after)
        }
        21 | 22 => {
            let n = rng.below(5);
            digits(out, n, 1);
            let docs: Vec<B> = (0..n).map(|_| sub(rng, out)).collect();
            if tag == 21 {
                ALLOC.concat(docs)
            } else {
                let sep = sub(rng, out);
                ALLOC.intersperse(docs, sep)
            }
        }
        23 => {
            let t = rng.pick(PROSE);
            text(out, t);
            ALLOC.reflow(t)
        }
        24 | 25 => {
            let kind = rng.below(4);
            let k = rng.below(21) as i64 - 4;
            digits(out, kind, 1);
            signed(out, k, 2);
            let d = sub(rng, out);
            if tag == 24 {
                ALLOC.column(move |c| apply(kind, k, d.clone(), c as i64))
            } else {
                ALLOC.nesting(move |n| apply(kind, k, d.clone(), n as i64))
            }
        }
        26 => {
            let n = rng.below(4000) as i64 - 2000;
            signed(out, n, 2);
            ALLOC.as_string(n)
        }
        27 => {
            // Text made by hand, which the crate measures in bytes.
            let t = rng.pick(TEXTS);
            text(out, t);
            DocBuilder(&ALLOC, pretty::Doc::OwnedText(t.into()).into())
        }
        29 => {
            // A nesting function inside a group inside a nest: what the group
            // measures depends on the indentation it gives the function.
            let n = rng.below(21);
            let kind = rng.below(4);
            let k = rng.below(21) as i64 - 4;
            digits(out, n, 2);
            digits(out, kind, 1);
            signed(out, k, 2);
            let d = sub(rng, out);
            let e = sub(rng, out);
            ALLOC
                .nesting(move |x| apply(kind, k, d.clone(), x as i64))
                .append(e)
                .group()
                .nest(n as isize)
        }
        _ => {
            let indent = rng.below(9) as i64 - 2;
            let count = rng.below(4);
            signed(out, indent, 1);
            digits(out, count, 1);
            let affixes = (0..count)
                .map(|_| {
                    let prefix = sub(rng, out);
                    let suffix = sub(rng, out);
                    let nest = rng.chance(50);
                    flag(out, nest);
                    let a = Affixes::new(prefix, suffix);
                    if nest { a.nest() } else { a }
                })
                .collect();
            let body = sub(rng, out);
            BlockDoc { affixes, body }.format(indent as isize)
        }
    }
}

// --- rendering ----------------------------------------------------------------------

enum Event {
    Write(String),
    Push(u32),
    Pop,
}

/// Records what the crate renders, joining adjacent text.
#[derive(Default)]
struct Recorder(Vec<Event>);

impl Render for Recorder {
    type Error = ();

    fn write_str(&mut self, s: &str) -> Result<usize, ()> {
        if !s.is_empty() {
            if let Some(Event::Write(t)) = self.0.last_mut() {
                t.push_str(s);
            } else {
                self.0.push(Event::Write(s.to_string()));
            }
        }
        Ok(s.len())
    }

    fn fail_doc(&self) {}
}

impl<'a> RenderAnnotated<'a, u32> for Recorder {
    fn push_annotation(&mut self, a: &'a u32) -> Result<(), ()> {
        self.0.push(Event::Push(*a));
        Ok(())
    }

    fn pop_annotation(&mut self) -> Result<(), ()> {
        self.0.push(Event::Pop);
        Ok(())
    }
}

// --- cases --------------------------------------------------------------------------

fn cases() -> String {
    let mut rng = Rng(0x9e77_1e5d_0c5e_ed01);
    let mut body = String::new();
    let count = 2000;
    let mut rendered = 0;
    for _ in 0..count {
        let depth = 1 + rng.below(6) as u32;
        let doc = random_doc(&mut rng, &mut body, depth).into_doc();
        let widths = [rng.below(12), 12 + rng.below(30), rng.pick(&[40, 80, 200])];
        for w in widths {
            digits(&mut body, w, 2);
            let mut recorder = Recorder::default();
            let ok = doc.render_raw(w as usize, &mut recorder).is_ok();
            rendered += usize::from(ok);
            flag(&mut body, ok);
            if ok {
                digits(&mut body, recorder.0.len() as u64, 2);
                for event in &recorder.0 {
                    match event {
                        Event::Write(t) => {
                            digits(&mut body, 0, 1);
                            text(&mut body, t);
                        }
                        Event::Push(a) => {
                            digits(&mut body, 1, 1);
                            digits(&mut body, (*a).into(), 1);
                        }
                        Event::Pop => digits(&mut body, 2, 1),
                    }
                }
            }
        }
    }

    eprintln!("{rendered} of {} renderings succeeded", count * 3);
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- GENERATED by scripts/generate.sh from pretty {UPSTREAM_VERSION}.
-- Do not edit: run the script again instead.
--
-- Inputs, with what the crate makes of them, for `Tests.mw`: {count} random
-- documents, each rendered in three widths.
--
-- Copyright Jonathan Sterling, Darin Morrison, Markus Westerlind and the
-- pretty.rs contributors, and the Meadow port's authors. Licensed under MIT:
-- see LICENSE and COPYRIGHT.

-- Each document is written as the tree of calls that builds it, in the order
-- `Tests.mw` reads them, and then, for each width, whether the crate rendered
-- it and the text and annotations it rendered. A number is base-64 digits; a
-- string is its length in bytes (3 digits) and then its bytes.
@cfg(test)
@pub(pkg) def cases =
  {}",
        long_literal(&body, 96)
    );
    out
}
