//! `FilingStatus` and its wire forms (`DOMAIN-MODEL.md` §4, §20 deviation 5).

use core::fmt;
use core::str::FromStr;

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};

/// Federal filing status.
///
/// The wire forms are the short ones the tax kernel, its fixtures and every
/// parameter table's `filingStatus` breakdown use: `single`, `mfj`, `mfs`, `hoh`,
/// `qss`. The long names (Married Filing Jointly, Qualifying Surviving Spouse) are
/// UI labels only and never parse (`DOMAIN-MODEL.md` §4, §15). Variant order is
/// the declaration order of the design and is the type's `Ord`.
///
/// Serde is written by hand rather than derived so that the wire form is a
/// string and nothing else: the derived enum representation would also accept
/// the map form `{"mfj": null}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FilingStatus {
    /// Single. Wire form `single`.
    Single,
    /// Married filing jointly. Wire form `mfj`.
    Mfj,
    /// Married filing separately. Wire form `mfs`.
    Mfs,
    /// Head of household. Wire form `hoh`.
    Hoh,
    /// Qualifying surviving spouse. Wire form `qss`.
    Qss,
}

impl FilingStatus {
    /// Every filing status, in declaration order. Iterate this rather than
    /// listing variants by hand, so a table or a test cannot omit one.
    pub const ALL: [Self; 5] = [Self::Single, Self::Mfj, Self::Mfs, Self::Hoh, Self::Qss];

    /// The wire form: the only spelling that serializes, deserializes or keys a
    /// parameter table's breakdown.
    #[must_use]
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Mfj => "mfj",
            Self::Mfs => "mfs",
            Self::Hoh => "hoh",
            Self::Qss => "qss",
        }
    }

    /// Resolves a wire form. Exact match only: no long name, no upper-case alias,
    /// no surrounding white space (`DOMAIN-MODEL.md` §15, "breakdown keys are the
    /// wire forms of the breakdown's enum, and nothing else").
    #[must_use]
    pub fn from_wire(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|status| status.wire() == s)
    }
}

impl Serialize for FilingStatus {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire())
    }
}

impl<'de> Deserialize<'de> for FilingStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct WireForm;

        impl Visitor<'_> for WireForm {
            type Value = FilingStatus;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("one of \"single\", \"mfj\", \"mfs\", \"hoh\", \"qss\"")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<FilingStatus, E> {
                FilingStatus::from_wire(v)
                    .ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &self))
            }
        }

        deserializer.deserialize_str(WireForm)
    }
}

impl fmt::Display for FilingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// A string that is not one of the five `FilingStatus` wire forms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownFilingStatus(pub String);

impl fmt::Display for UnknownFilingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown filing status {:?}: expected one of single, mfj, mfs, hoh, qss",
            self.0
        )
    }
}

impl std::error::Error for UnknownFilingStatus {}

impl FromStr for FilingStatus {
    type Err = UnknownFilingStatus;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_wire(s).ok_or_else(|| UnknownFilingStatus(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The design's wire forms, written out independently of `wire()` so that a
    /// typo in either place fails (`DOMAIN-MODEL.md` §4).
    const WIRE: [(FilingStatus, &str); 5] = [
        (FilingStatus::Single, "single"),
        (FilingStatus::Mfj, "mfj"),
        (FilingStatus::Mfs, "mfs"),
        (FilingStatus::Hoh, "hoh"),
        (FilingStatus::Qss, "qss"),
    ];

    /// An exhaustive match: adding a variant without extending `ALL` and the
    /// table above stops this file compiling or fails the count below.
    fn index(status: FilingStatus) -> usize {
        match status {
            FilingStatus::Single => 0,
            FilingStatus::Mfj => 1,
            FilingStatus::Mfs => 2,
            FilingStatus::Hoh => 3,
            FilingStatus::Qss => 4,
        }
    }

    #[test]
    fn all_lists_every_variant_once_in_declaration_order() {
        assert_eq!(FilingStatus::ALL.len(), WIRE.len());
        for (i, status) in FilingStatus::ALL.into_iter().enumerate() {
            assert_eq!(index(status), i);
            assert_eq!(WIRE[i].0, status);
        }
        let mut sorted = FilingStatus::ALL;
        sorted.sort();
        assert_eq!(sorted, FilingStatus::ALL);
    }

    #[test]
    fn every_wire_form_round_trips_through_every_surface() {
        for (status, wire) in WIRE {
            assert_eq!(status.wire(), wire);
            assert_eq!(status.to_string(), wire);
            assert_eq!(FilingStatus::from_wire(wire), Some(status));
            assert_eq!(wire.parse::<FilingStatus>(), Ok(status));
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, format!("\"{wire}\""));
            assert_eq!(serde_json::from_str::<FilingStatus>(&json).unwrap(), status);
        }
    }

    #[test]
    fn nothing_but_the_five_wire_forms_parses() {
        let rejected = [
            "",
            " mfj",
            "mfj ",
            "MFJ",
            "Mfj",
            "Single",
            "SINGLE",
            "joint",
            "JOINT",
            "marriedFilingJointly",
            "married_filing_jointly",
            "headOfHousehold",
            "qw",
            "widow",
            "0",
        ];
        for s in rejected {
            assert_eq!(FilingStatus::from_wire(s), None, "{s:?}");
            assert_eq!(
                s.parse::<FilingStatus>(),
                Err(UnknownFilingStatus(s.to_owned())),
                "{s:?}"
            );
            let json = serde_json::to_string(s).unwrap();
            assert!(
                serde_json::from_str::<FilingStatus>(&json).is_err(),
                "{s:?}"
            );
        }
        // Not a string at all.
        assert!(serde_json::from_str::<FilingStatus>("1").is_err());
        assert!(serde_json::from_str::<FilingStatus>("null").is_err());
        assert!(serde_json::from_str::<FilingStatus>("{\"mfj\":null}").is_err());
    }

    #[test]
    fn it_keys_an_ordered_map_by_wire_form() {
        use std::collections::BTreeMap;
        let map: BTreeMap<FilingStatus, u8> = FilingStatus::ALL.into_iter().zip(0u8..).collect();
        let json = serde_json::to_string(&map).unwrap();
        assert_eq!(json, r#"{"single":0,"mfj":1,"mfs":2,"hoh":3,"qss":4}"#);
        let back: BTreeMap<FilingStatus, u8> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, map);
    }

    #[test]
    fn the_error_names_the_offending_input() {
        let err = "JOINT".parse::<FilingStatus>().unwrap_err();
        assert!(err.to_string().contains("\"JOINT\""));
    }
}
