//! `Redacted<T>`: a value that cannot be printed by accident (`SECURITY.md` §5, §8).
//!
//! `Debug` and `Display` both print the fixed string `<redacted>`, whatever the
//! formatter flags, so a `Redacted` field inside a larger `#[derive(Debug)]`
//! struct stays hidden in `{:?}`, `{:#?}`, a panic message or an assertion
//! failure. There is deliberately **no** `Serialize` implementation: a redacted
//! value cannot reach a JSON body or a snapshot by being part of a DTO.
//!
//! [`Redacted::expose`] is the single way out, and it is greppable.
//!
//! `Redacted` is about *output*, not memory: it does not wipe the value on drop.
//! Key material that must be wiped is the business of the `Secret<T>` types that
//! arrive with the vault (M1).

use std::fmt;

/// The text printed in place of the value.
pub const REDACTED: &str = "<redacted>";

/// A wrapper whose `Debug` and `Display` never show the wrapped value.
///
/// ```
/// use pfp_server::Redacted;
///
/// let token = Redacted::new(String::from("do-not-print"));
/// assert_eq!(format!("{token}"), "<redacted>");
/// assert_eq!(format!("{token:?}"), "<redacted>");
/// assert_eq!(token.expose(), "do-not-print");
/// ```
///
/// It cannot be serialized. The same bound that accepts a `String` …
///
/// ```
/// fn needs_serialize<T: serde::Serialize>(_: &T) {}
/// needs_serialize(&String::from("fine"));
/// ```
///
/// … rejects a `Redacted<String>` at compile time:
///
/// ```compile_fail,E0277
/// fn needs_serialize<T: serde::Serialize>(_: &T) {}
/// needs_serialize(&pfp_server::Redacted::new(String::from("never")));
/// ```
///
/// There is no `PartialEq` either: secrets are compared in constant time by the
/// code that owns them, never with `==` on the wrapper.
#[derive(Clone)]
pub struct Redacted<T>(T);

impl<T> Redacted<T> {
    /// Wraps a value.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// The wrapped value. This is the only accessor; every use is a deliberate,
    /// reviewable act.
    pub const fn expose(&self) -> &T {
        &self.0
    }
}

impl<T> From<T> for Redacted<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use super::{Redacted, REDACTED};

    const CANARY: &str = "canary-7f3a";

    #[derive(Debug)]
    struct Outer {
        // Read only through `Debug`, which is the point of the test.
        #[allow(dead_code)]
        label: &'static str,
        #[allow(dead_code)]
        inner: Redacted<String>,
        #[allow(dead_code)]
        list: Vec<Redacted<u64>>,
    }

    #[test]
    fn debug_and_display_print_the_fixed_text() {
        let value = Redacted::new(CANARY.to_owned());
        assert_eq!(format!("{value}"), REDACTED);
        assert_eq!(format!("{value:?}"), REDACTED);
        assert_eq!(format!("{value:#?}"), REDACTED);
        // Width, precision and alternate flags must not re-enable the inner formatter.
        assert_eq!(format!("{value:>40.3}"), REDACTED);
        assert_eq!(value.to_string(), REDACTED);
    }

    #[test]
    fn a_type_without_debug_can_still_be_wrapped_and_printed() {
        struct Opaque;
        let value = Redacted::new(Opaque);
        assert_eq!(format!("{value:?}"), REDACTED);
        assert_eq!(format!("{value}"), REDACTED);
    }

    #[test]
    fn nested_in_a_derived_debug_the_value_stays_hidden() {
        let outer = Outer {
            label: "visible",
            inner: CANARY.to_owned().into(),
            list: vec![Redacted::new(4_242_424_242)],
        };
        for text in [format!("{outer:?}"), format!("{outer:#?}")] {
            assert!(text.contains("visible"));
            assert!(text.contains(REDACTED));
            assert!(!text.contains(CANARY));
            assert!(!text.contains("4242424242"));
        }
    }

    #[test]
    fn a_failed_result_does_not_print_the_value() {
        let result: Result<(), Redacted<String>> = Err(Redacted::new(CANARY.to_owned()));
        let text = format!("{result:?}");
        assert_eq!(text, format!("Err({REDACTED})"));
    }

    #[test]
    fn expose_is_the_way_out_and_a_clone_keeps_the_value() {
        let a = Redacted::new(CANARY.to_owned());
        assert_eq!(a.expose(), CANARY);
        assert_eq!(a.clone().expose(), CANARY);
    }
}
