//! `lint-dollars`: no dollar constants in engine code (ADR-022; `ARCHITECTURE.md`
//! §2 constraints table; `TESTING.md` §11.2 gate 7).
//!
//! Statutory amounts come from `params/`, sourced and versioned, so that the
//! assistant never authors both a constant and the test that checks it. In the
//! non-test source of every engine crate the lint rejects:
//!
//! 1. an integer literal passed straight to a money constructor
//!    (`Cents(…)`, `cents(…)`, `dollars(…)`, `from_dollars(…)`, `from_cents(…)`),
//!    other than zero; and
//! 2. any decimal integer literal of at least [`THRESHOLD`], the size at which
//!    a bare number in tax or ledger code is a threshold, a cap or a limit.
//!
//! Comments, string literals, `#[cfg(test)]` items and the `tests/`, `benches/`
//! and `examples/` directories are skipped. Hex, octal and binary literals are
//! bit patterns, not amounts, and are skipped. There is deliberately no
//! per-line escape comment: an amount the engine needs is a parameter.

use crate::{engine, repo};

/// Decimal integer literals at or above this value are treated as amounts.
/// Years (`2026`), months, percentages and small counts stay below it.
pub(crate) const THRESHOLD: u128 = 10_000;

const MONEY_CONSTRUCTORS: [&str; 5] = ["Cents", "cents", "dollars", "from_dollars", "from_cents"];

pub(crate) fn run() -> Result<bool, String> {
    let root = repo::root();
    let mut violations = Vec::new();
    for name in engine::existing_engine_crates(&root) {
        let src = root.join("crates").join(name).join("src");
        if !src.is_dir() {
            continue;
        }
        for path in repo::walk(&src)? {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = String::from_utf8_lossy(&repo::read(&path)?).into_owned();
            let rel = repo::relative(&root, &path);
            for v in lint_source(&text) {
                violations.push(format!("{rel}:{}: {}", v.line, v.message));
            }
        }
    }
    for v in &violations {
        eprintln!("lint-dollars: {v}");
    }
    if violations.is_empty() {
        eprintln!("lint-dollars: clean");
        Ok(true)
    } else {
        eprintln!(
            "lint-dollars: {} dollar literal(s) in engine code",
            violations.len()
        );
        Ok(false)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) line: usize,
    pub(crate) message: String,
}

/// Lints one Rust source file and returns every violation in source order.
pub(crate) fn lint_source(text: &str) -> Vec<Violation> {
    let code = blank_comments_and_strings(text);
    let code = blank_cfg_test_items(&code);
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() || is_ident_byte(prev(bytes, i)) || prev(bytes, i) == b'.' {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        let end = i;
        // Skip the type suffix (`_u32`, `i64`) so it is not mistaken for an identifier.
        while i < bytes.len() && is_ident_byte(bytes[i]) {
            i += 1;
        }
        let literal = &code[start..end];
        let next = bytes.get(end).copied().unwrap_or(b' ');
        let digit_follows = bytes.get(end + 1).is_some_and(u8::is_ascii_digit);
        let is_float = digit_follows && matches!(next, b'.' | b'e' | b'E');
        let is_radix = literal == "0" && matches!(next, b'x' | b'o' | b'b');
        if is_radix {
            continue;
        }
        let value: u128 = literal
            .bytes()
            .filter(u8::is_ascii_digit)
            .fold(0, |acc, d| {
                acc.saturating_mul(10).saturating_add(u128::from(d - b'0'))
            });
        let line = code[..start].matches('\n').count() + 1;
        if let Some(ctor) = money_constructor_before(&code, start) {
            // A float handed to a money constructor is an amount whatever its size.
            if value != 0 || is_float {
                out.push(Violation {
                    line,
                    message: format!(
                        "literal amount passed to `{ctor}(`: `{literal}`; read it from params/"
                    ),
                });
                continue;
            }
        }
        if !is_float && value >= THRESHOLD {
            out.push(Violation {
                line,
                message: format!(
                    "integer literal `{literal}` looks like a dollar amount (>= {THRESHOLD}); read it from params/"
                ),
            });
        }
    }
    out
}

fn prev(bytes: &[u8], i: usize) -> u8 {
    if i == 0 {
        b' '
    } else {
        bytes[i - 1]
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// If the literal at `pos` is the first argument of a money constructor, returns
/// the constructor's name: `Cents(`, `Cents( -`, `Cents::from_dollars(`, ….
fn money_constructor_before(code: &str, pos: usize) -> Option<&'static str> {
    let bytes = code.as_bytes();
    let mut i = pos;
    while i > 0 && matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'-' | b'+') {
        i -= 1;
    }
    if i == 0 || bytes[i - 1] != b'(' {
        return None;
    }
    let close = i - 1;
    let mut j = close;
    while j > 0 && is_ident_byte(bytes[j - 1]) {
        j -= 1;
    }
    let ident = &code[j..close];
    MONEY_CONSTRUCTORS.iter().copied().find(|c| *c == ident)
}

/// Replaces every comment, string literal and character literal with spaces
/// (newlines are kept) so that offsets and line numbers survive.
fn blank_comments_and_strings(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    while i < b.len() {
        let end = match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => Some(
                b[i..]
                    .iter()
                    .position(|c| *c == b'\n')
                    .map_or(b.len(), |p| i + p),
            ),
            b'/' if b.get(i + 1) == Some(&b'*') => Some(block_comment_end(b, i)),
            b'r' | b'b' if !is_ident_byte(prev(b, i)) => raw_string_end(b, i),
            b'"' => Some(quoted_end(b, i, b'"')),
            b'\'' if !is_ident_byte(prev(b, i)) && is_char_literal(b, i) => {
                Some(quoted_end(b, i, b'\''))
            }
            _ => None,
        };
        match end {
            Some(end) => {
                for byte in &mut out[i..end] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
                i = end;
            }
            None => i += 1,
        }
    }
    String::from_utf8(out).expect("blanking whole bytes to spaces keeps the text valid UTF-8")
}

/// The end of the (possibly nested) block comment that opens at `start`.
fn block_comment_end(b: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut k = start;
    while k + 1 < b.len() {
        if b[k] == b'/' && b[k + 1] == b'*' {
            depth += 1;
            k += 2;
        } else if b[k] == b'*' && b[k + 1] == b'/' {
            depth -= 1;
            k += 2;
            if depth == 0 {
                return k;
            }
        } else {
            k += 1;
        }
    }
    b.len()
}

/// The end of the raw string (`r"…"`, `r#"…"#`, `br"…"`, `br#"…"#`) that opens at
/// `start`, or `None` if this `r`/`b` does not open one. A plain `b"…"` byte string
/// is handled by the `"` arm once the scan reaches its quote.
fn raw_string_end(b: &[u8], start: usize) -> Option<usize> {
    let mut k = start + 1;
    if b[start] == b'b' {
        if b.get(k) != Some(&b'r') {
            return None;
        }
        k += 1;
    }
    let mut hashes = 0;
    while b.get(k) == Some(&b'#') {
        hashes += 1;
        k += 1;
    }
    if b.get(k) != Some(&b'"') {
        return None;
    }
    let mut close = b"\"".to_vec();
    close.extend(std::iter::repeat_n(b'#', hashes));
    let end = b[k + 1..]
        .windows(close.len())
        .position(|w| w == close.as_slice())
        .map_or(b.len(), |p| k + 1 + p + close.len());
    Some(end)
}

/// The end of the string or character literal whose opening `quote` is at `start`,
/// honouring backslash escapes.
fn quoted_end(b: &[u8], start: usize, quote: u8) -> usize {
    let mut k = start + 1;
    while k < b.len() && b[k] != quote {
        if b[k] == b'\\' {
            k += 1;
        }
        k += 1;
    }
    (k + 1).min(b.len())
}

/// A char literal (`'x'`, `'\n'`, `'\u{1F600}'`) as opposed to a lifetime (`'a`).
fn is_char_literal(b: &[u8], i: usize) -> bool {
    b.get(i + 1) == Some(&b'\\') || (b.get(i + 2) == Some(&b'\'') && b.get(i + 1) != Some(&b'\''))
}

/// Blanks every item that follows a `#[cfg(test)]` attribute: a `mod tests { … }`
/// block, a single `#[cfg(test)] fn`, or a `use` line. The item ends at the
/// matching close brace of its first `{`, or at the first `;` before any `{`.
fn blank_cfg_test_items(code: &str) -> String {
    let mut out = code.as_bytes().to_vec();
    let b = code.as_bytes();
    let needle = b"#[cfg(test)]";
    let mut from = 0;
    while let Some(p) = b[from..].windows(needle.len()).position(|w| w == needle) {
        let attr = from + p;
        let mut k = attr + needle.len();
        let mut depth = 0usize;
        let mut end = b.len();
        while k < b.len() {
            match b[k] {
                b'{' => depth += 1,
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        end = k + 1;
                        break;
                    }
                }
                b';' if depth == 0 => {
                    end = k + 1;
                    break;
                }
                _ => {}
            }
            k += 1;
        }
        for byte in &mut out[attr..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        from = end;
    }
    String::from_utf8(out).expect("blanking ASCII bytes keeps the text valid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(text: &str) -> Vec<usize> {
        lint_source(text).into_iter().map(|v| v.line).collect()
    }

    #[test]
    fn money_constructor_with_literal_is_rejected() {
        let src = "fn f() -> Cents { Cents(600_000) }\n";
        let v = lint_source(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, 1);
        assert!(v[0].message.contains("`Cents(`"), "{}", v[0].message);
        assert_eq!(lines("let x = Cents::from_dollars( -7500 );"), vec![1]);
        assert_eq!(lines("let x = dollars(1);"), vec![1]);
        assert_eq!(lines("let x = from_dollars(7500.0);"), vec![1]);
    }

    #[test]
    fn zero_and_small_literals_are_allowed() {
        assert!(lines("let z = Cents(0); let y = Year(2026); let m = 12; let p = 100;").is_empty());
        assert!(lines("const MONTHS: u32 = 12;\nlet n = 9_999;").is_empty());
    }

    #[test]
    fn large_literal_is_rejected_with_its_line() {
        let src = "let a = 1;\nlet cap = 40_000_00;\nlet b = 2;\nconst X: i64 = 10000u32 as i64;\n";
        assert_eq!(lines(src), vec![2, 4]);
    }

    #[test]
    fn comments_strings_and_floats_are_ignored() {
        let src = concat!(
            "// the SALT cap was 40_000 dollars\n",
            "/* block 750_000\n   comment */\n",
            "let id = \"line 8960000 in a string\";\n",
            "let raw = r#\"raw 123456\"#;\n",
            "let f = 12345.0; let g = 1e10; let h = 0xFFFF_FFFF;\n",
            "let c = '9'; let idx = t.1234567;\n",
        );
        assert!(lines(src).is_empty(), "{:?}", lint_source(src));
    }

    #[test]
    fn cfg_test_items_are_skipped() {
        let src = concat!(
            "pub fn live() -> i64 { 1 }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    fn t() { assert_eq!(Cents(600_000), schedule(1_000_000)); }\n",
            "}\n",
            "fn after() -> i64 { 50_000 }\n",
        );
        assert_eq!(lines(src), vec![6]);
    }

    #[test]
    fn nested_braces_inside_cfg_test_are_handled() {
        let src = "#[cfg(test)]\nmod t { fn a() { if x { 99_999 } } }\nfn b() { 99_999 }\n";
        assert_eq!(lines(src), vec![3]);
    }

    #[test]
    fn engine_crates_in_the_tree_are_clean() {
        // The design's list, filtered to what exists, is what `run` lints.
        assert!(engine::existing_engine_crates(&repo::root()).contains(&"pfp-money"));
        assert!(run().unwrap());
    }
}
