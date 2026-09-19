//! The typed references a [`Line`](crate::Line) carries.
//!
//! Identifiers are stable strings, part of the fixture and UI contract
//! (`ENGINE-SPEC.md` §3.2: "Line ids are stable strings; the prefix names the
//! worksheet"). They are validated where they are made, so a malformed id is a
//! build or load failure and never a silent mismatch.

use core::fmt;
use std::borrow::Cow;

use pfp_domain::Year;
use serde::{Deserialize, Serialize};

/// Longest identifier accepted, in bytes.
const MAX_LEN: usize = 96;

/// Why a string is not an identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdError {
    /// Empty.
    Empty,
    /// Longer than 96 bytes.
    TooLong,
    /// A leading, trailing or doubled `.`.
    EmptySegment,
    /// A byte outside the identifier's alphabet.
    BadCharacter,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "identifier is empty",
            Self::TooLong => "identifier is longer than 96 bytes",
            Self::EmptySegment => "identifier has an empty dot-separated segment",
            Self::BadCharacter => "identifier has a character outside its alphabet",
        })
    }
}

impl std::error::Error for IdError {}

/// Which bytes a segment may hold besides ASCII digits, lower-case letters and `_`.
#[derive(Clone, Copy)]
enum Alphabet {
    /// `[a-z0-9_]`: line ids and parameter ids (`sched.b0`, `irs.std_deduction`).
    Lower,
    /// `[A-Za-z0-9_]`: rule ids and table elements, which quote statute
    /// paragraphs (`usc26.1f7A.brackets`).
    Mixed,
}

/// Dot-separated, non-empty segments over `alphabet`. `const` so that a static
/// id is checked when the constant is evaluated.
const fn validate(s: &str, alphabet: Alphabet, dots: bool) -> Result<(), IdError> {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return Err(IdError::Empty);
    }
    if bytes.len() > MAX_LEN {
        return Err(IdError::TooLong);
    }
    let mut i = 0;
    let mut segment_len = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' && dots {
            if segment_len == 0 {
                return Err(IdError::EmptySegment);
            }
            segment_len = 0;
        } else {
            let ok = b.is_ascii_digit()
                || b.is_ascii_lowercase()
                || b == b'_'
                || (matches!(alphabet, Alphabet::Mixed) && b.is_ascii_uppercase());
            if !ok {
                return Err(IdError::BadCharacter);
            }
            segment_len += 1;
        }
        i += 1;
    }
    if segment_len == 0 {
        return Err(IdError::EmptySegment);
    }
    Ok(())
}

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident, $alphabet:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Cow<'static, str>);

        impl $name {
            /// Validates and wraps an identifier built at run time.
            ///
            /// # Errors
            ///
            /// [`IdError`] when `id` is not dot-separated, non-empty segments
            /// over this identifier's alphabet, at most 96 bytes.
            pub fn new(id: impl Into<Cow<'static, str>>) -> Result<Self, IdError> {
                let id = id.into();
                match validate(&id, $alphabet, true) {
                    Ok(()) => Ok(Self(id)),
                    Err(e) => Err(e),
                }
            }

            /// Wraps a literal without allocating. In a `const` or `static` a
            /// malformed literal fails the build.
            ///
            /// # Panics
            ///
            /// When `id` is malformed.
            #[must_use]
            pub const fn from_static(id: &'static str) -> Self {
                assert!(
                    validate(id, $alphabet, true).is_ok(),
                    "malformed static identifier"
                );
                Self(Cow::Borrowed(id))
            }

            /// The identifier.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;

            fn try_from(id: String) -> Result<Self, IdError> {
                Self::new(id)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0.into_owned()
            }
        }
    };
}

string_id!(
    /// The stable id of a [`Line`](crate::Line): dot-separated segments of
    /// `[a-z0-9_]`, the first naming the worksheet (`sched.b0`, `pub915.ws1.l9`).
    /// `Ord` is the string's, which is what keys a `BTreeMap<LineId, Line>`.
    LineId,
    Alphabet::Lower
);

string_id!(
    /// The id of a named `RoundingRule` (`irs.whole_dollar`,
    /// `usc26.1f7A.brackets`): dot-separated segments of `[A-Za-z0-9_]`.
    RuleId,
    Alphabet::Mixed
);

/// A reference to one cell of a public parameter table: which table, and where
/// in it. The addressing mirrors `ParamOverride` (`DOMAIN-MODEL.md` §15), so an
/// override and a trace name a cell the same way.
///
/// The vintage is not part of the reference: one computation reads one
/// `ParamView`, whose vintage ids are pinned once on the result (seam S8).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    deny_unknown_fields,
    try_from = "RawParamRef"
)]
pub struct ParamRef {
    /// The table's `id`, e.g. `irs.ordinary_brackets`.
    param_id: Cow<'static, str>,
    /// The year whose row was read; `None` for a table that is not year-keyed.
    #[serde(skip_serializing_if = "Option::is_none")]
    year: Option<Year>,
    /// The breakdown key, a wire form of the breakdown's enum (`mfj`); `None`
    /// for a table with no breakdown.
    #[serde(skip_serializing_if = "Option::is_none")]
    breakdown_key: Option<Cow<'static, str>>,
    /// The element within the row for a table whose rows are lists
    /// (`top_of_22`, `rates.2`); `None` for a scalar row.
    #[serde(skip_serializing_if = "Option::is_none")]
    element: Option<Cow<'static, str>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawParamRef {
    param_id: String,
    #[serde(default)]
    year: Option<Year>,
    #[serde(default)]
    breakdown_key: Option<String>,
    #[serde(default)]
    element: Option<String>,
}

impl TryFrom<RawParamRef> for ParamRef {
    type Error = IdError;

    fn try_from(raw: RawParamRef) -> Result<Self, IdError> {
        let mut out = Self::new(raw.param_id)?;
        out.year = raw.year;
        if let Some(key) = raw.breakdown_key {
            out = out.with_breakdown_key(key)?;
        }
        if let Some(element) = raw.element {
            out = out.with_element(element)?;
        }
        Ok(out)
    }
}

impl ParamRef {
    /// A reference to the table `param_id` as a whole.
    ///
    /// # Errors
    ///
    /// [`IdError`] unless `param_id` is dot-separated segments of `[a-z0-9_]`.
    pub fn new(param_id: impl Into<Cow<'static, str>>) -> Result<Self, IdError> {
        let param_id = param_id.into();
        validate(&param_id, Alphabet::Lower, true)?;
        Ok(Self {
            param_id,
            year: None,
            breakdown_key: None,
            element: None,
        })
    }

    /// Narrows the reference to one year's row.
    #[must_use]
    pub fn with_year(mut self, year: Year) -> Self {
        self.year = Some(year);
        self
    }

    /// Narrows the reference to one breakdown key.
    ///
    /// # Errors
    ///
    /// [`IdError`] unless `key` is one segment of `[A-Za-z0-9_]`.
    pub fn with_breakdown_key(
        mut self,
        key: impl Into<Cow<'static, str>>,
    ) -> Result<Self, IdError> {
        let key = key.into();
        validate(&key, Alphabet::Mixed, false)?;
        self.breakdown_key = Some(key);
        Ok(self)
    }

    /// Narrows the reference to one element of a list-valued row.
    ///
    /// # Errors
    ///
    /// [`IdError`] unless `element` is dot-separated segments of `[A-Za-z0-9_]`.
    pub fn with_element(mut self, element: impl Into<Cow<'static, str>>) -> Result<Self, IdError> {
        let element = element.into();
        validate(&element, Alphabet::Mixed, true)?;
        self.element = Some(element);
        Ok(self)
    }

    /// The table's id.
    #[must_use]
    pub fn param_id(&self) -> &str {
        &self.param_id
    }

    /// The year whose row was read.
    #[must_use]
    pub fn year(&self) -> Option<Year> {
        self.year
    }

    /// The breakdown key.
    #[must_use]
    pub fn breakdown_key(&self) -> Option<&str> {
        self.breakdown_key.as_deref()
    }

    /// The element within the row.
    #[must_use]
    pub fn element(&self) -> Option<&str> {
        self.element.as_deref()
    }
}

/// `irs.ordinary_brackets[2026][mfj].top_of_22`
impl fmt::Display for ParamRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.param_id)?;
        if let Some(year) = self.year {
            write!(f, "[{year}]")?;
        }
        if let Some(key) = &self.breakdown_key {
            write!(f, "[{key}]")?;
        }
        if let Some(element) = &self.element {
            write!(f, ".{element}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCHED_TOTAL: LineId = LineId::from_static("sched.total");

    #[test]
    fn line_ids_accept_the_designs_examples_and_nothing_loose() {
        for ok in [
            "sched.b0",
            "pub915.ws1.l9",
            "f8960.niit",
            "total",
            "a_b.c_1",
        ] {
            assert_eq!(LineId::new(ok).unwrap().as_str(), ok);
        }
        let too_long = "a".repeat(97);
        let cases: [(&str, IdError); 9] = [
            ("", IdError::Empty),
            (".", IdError::EmptySegment),
            (".a", IdError::EmptySegment),
            ("a.", IdError::EmptySegment),
            ("a..b", IdError::EmptySegment),
            ("Sched.b0", IdError::BadCharacter),
            ("sched b0", IdError::BadCharacter),
            ("sched-b0", IdError::BadCharacter),
            (&too_long, IdError::TooLong),
        ];
        for (bad, why) in cases {
            assert_eq!(LineId::new(bad.to_owned()), Err(why), "{bad:?}");
        }
        assert!(LineId::new("a".repeat(96)).is_ok());
        assert_eq!(LineId::new("sché.b0"), Err(IdError::BadCharacter));
    }

    #[test]
    fn rule_ids_may_quote_a_statute_paragraph() {
        assert!(RuleId::new("usc26.1f7A.brackets").is_ok());
        assert!(RuleId::new("usc42.1395r_i.irmaa").is_ok());
        assert!(RuleId::new("irs.whole_dollar").is_ok());
        assert_eq!(
            LineId::new("usc26.1f7A.brackets"),
            Err(IdError::BadCharacter)
        );
        assert_eq!(RuleId::new("irs whole"), Err(IdError::BadCharacter));
    }

    #[test]
    fn static_and_owned_ids_are_the_same_id() {
        let owned = LineId::new(String::from("sched.total")).unwrap();
        assert_eq!(owned, SCHED_TOTAL);
        assert_eq!(owned.to_string(), "sched.total");
        assert!(LineId::from_static("sched.b10") < LineId::from_static("sched.b2"));
    }

    #[test]
    #[should_panic(expected = "malformed static identifier")]
    fn a_malformed_static_id_panics() {
        let _ = LineId::from_static("Sched");
    }

    #[test]
    fn ids_are_json_strings_and_are_validated_on_the_way_in() {
        assert_eq!(
            serde_json::to_string(&SCHED_TOTAL).unwrap(),
            "\"sched.total\""
        );
        assert_eq!(
            serde_json::from_str::<LineId>("\"sched.total\"").unwrap(),
            SCHED_TOTAL
        );
        for bad in ["\"\"", "\"a..b\"", "\"A\"", "1", "null", "[\"a\"]"] {
            assert!(serde_json::from_str::<LineId>(bad).is_err(), "{bad}");
        }
        let rule: RuleId = serde_json::from_str("\"usc26.1f7A.brackets\"").unwrap();
        assert_eq!(rule.as_str(), "usc26.1f7A.brackets");
    }

    #[test]
    fn a_param_ref_addresses_a_cell_like_an_override_does() {
        let cell = ParamRef::new("irs.ordinary_brackets")
            .unwrap()
            .with_year(2026)
            .with_breakdown_key("mfj")
            .unwrap()
            .with_element("top_of_22")
            .unwrap();
        assert_eq!(cell.param_id(), "irs.ordinary_brackets");
        assert_eq!(cell.year(), Some(2026));
        assert_eq!(cell.breakdown_key(), Some("mfj"));
        assert_eq!(cell.element(), Some("top_of_22"));
        assert_eq!(
            cell.to_string(),
            "irs.ordinary_brackets[2026][mfj].top_of_22"
        );
        let json = serde_json::to_string(&cell).unwrap();
        assert_eq!(
            json,
            r#"{"paramId":"irs.ordinary_brackets","year":2026,"breakdownKey":"mfj","element":"top_of_22"}"#
        );
        assert_eq!(serde_json::from_str::<ParamRef>(&json).unwrap(), cell);

        let table = ParamRef::new("irs.whole_dollar").unwrap();
        assert_eq!(
            serde_json::to_string(&table).unwrap(),
            r#"{"paramId":"irs.whole_dollar"}"#
        );
        assert_eq!(table.to_string(), "irs.whole_dollar");
    }

    #[test]
    fn a_param_ref_is_validated_field_by_field() {
        assert_eq!(ParamRef::new("IRS.brackets"), Err(IdError::BadCharacter));
        let table = ParamRef::new("irs.ordinary_brackets").unwrap();
        assert_eq!(
            table.clone().with_breakdown_key("m.fj"),
            Err(IdError::BadCharacter)
        );
        assert_eq!(table.clone().with_breakdown_key(""), Err(IdError::Empty));
        assert_eq!(
            table.clone().with_element("top of 22"),
            Err(IdError::BadCharacter)
        );
        assert!(table.with_breakdown_key("usLarge").is_ok());
        for bad in [
            "{}",
            r#"{"paramId":""}"#,
            r#"{"paramId":"a","breakdownKey":"a b"}"#,
            r#"{"paramId":"a","element":".x"}"#,
            r#"{"paramId":"a","vintage":"x"}"#,
            r#"{"paramId":"a","year":"2026"}"#,
        ] {
            assert!(serde_json::from_str::<ParamRef>(bad).is_err(), "{bad}");
        }
    }
}
