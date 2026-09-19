//! Provenance: where a table's numbers come from, and whether a human has read
//! them back (`DOMAIN-MODEL.md` §15, `TESTING.md` §5.2, ADR-022).

use core::fmt;

use toml::{Table, Value};

use crate::error::ParamError;
use crate::raw::At;

/// A calendar date written `YYYY-MM-DD`. Validated, never read from a clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateYmd {
    /// Year.
    pub year: i32,
    /// Month, `1..=12`.
    pub month: u8,
    /// Day, valid for the month (Gregorian leap years included).
    pub day: u8,
}

impl DateYmd {
    /// Parses exactly `YYYY-MM-DD`; `None` for anything else.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let b = s.as_bytes();
        let digits = |r: core::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        if !(digits(0..4) && digits(5..7) && digits(8..10)) {
            return None;
        }
        let year: i32 = s[0..4].parse().ok()?;
        let month: u8 = s[5..7].parse().ok()?;
        let day: u8 = s[8..10].parse().ok()?;
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let days = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return None,
        };
        (1..=days)
            .contains(&day)
            .then_some(Self { year, month, day })
    }
}

impl fmt::Display for DateYmd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Whether a human has read the table's values back against the primary document.
///
/// The spellings are `TESTING.md` §2.2's fixture values; no design document yet
/// says where a `params/` table records this (`VINTAGE.md`, the seventh deviation),
/// so a table that says nothing is [`VerificationStatus::Unstated`], which every
/// consumer must treat exactly like pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VerificationStatus {
    /// `pending-hand-verification`: transcribed, not yet read back by a human.
    PendingHandVerification,
    /// `primary-source-confirmed`: read back against the archived publication.
    PrimarySourceConfirmed,
    /// `hand-worked-reviewed`: derived by hand and reviewed.
    HandWorkedReviewed,
    /// The table carries no `verification` field.
    Unstated,
}

impl VerificationStatus {
    /// The wire form, or `None` for [`VerificationStatus::Unstated`].
    #[must_use]
    pub const fn wire(self) -> Option<&'static str> {
        match self {
            Self::PendingHandVerification => Some("pending-hand-verification"),
            Self::PrimarySourceConfirmed => Some("primary-source-confirmed"),
            Self::HandWorkedReviewed => Some("hand-worked-reviewed"),
            Self::Unstated => None,
        }
    }

    /// Whether a human has signed the values off. `false` for pending and unstated.
    #[must_use]
    pub const fn is_verified(self) -> bool {
        matches!(
            self,
            Self::PrimarySourceConfirmed | Self::HandWorkedReviewed
        )
    }

    pub(crate) fn read(at: At<'_>, map: &Table) -> Result<Self, ParamError> {
        match at.opt_str(map, "verification")? {
            None => Ok(Self::Unstated),
            Some("pending-hand-verification") => Ok(Self::PendingHandVerification),
            Some("primary-source-confirmed") => Ok(Self::PrimarySourceConfirmed),
            Some("hand-worked-reviewed") => Ok(Self::HandWorkedReviewed),
            Some(_) => Err(at.invalid(
                "verification",
                "is not pending-hand-verification, primary-source-confirmed or hand-worked-reviewed",
            )),
        }
    }
}

/// One `[[source]]` entry: a primary document and its archived, checksummed copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    title: String,
    publisher: Option<String>,
    url: String,
    retrieved: DateYmd,
    archive: Option<String>,
    sha256: String,
    locator: Option<String>,
    as_of: Option<DateYmd>,
}

impl Source {
    /// The document's title, with its section locator where the table wrote one.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The publishing body, if recorded.
    #[must_use]
    pub fn publisher(&self) -> Option<&str> {
        self.publisher.as_deref()
    }

    /// The publisher's own URL (`https://`). Never fetched by this crate.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The date the archived copy was retrieved.
    #[must_use]
    pub const fn retrieved(&self) -> DateYmd {
        self.retrieved
    }

    /// Repository path of the archived copy, under `params/provenance/`.
    #[must_use]
    pub fn archive(&self) -> Option<&str> {
        self.archive.as_deref()
    }

    /// SHA-256 of the archived copy, 64 lowercase hex digits.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Page or section locator inside the document, if recorded.
    #[must_use]
    pub fn locator(&self) -> Option<&str> {
        self.locator.as_deref()
    }

    /// The law date the document itself states, if recorded.
    #[must_use]
    pub const fn as_of(&self) -> Option<DateYmd> {
        self.as_of
    }
}

const SOURCE_FIELDS: [&str; 8] = [
    "title",
    "publisher",
    "url",
    "retrieved",
    "archive",
    "sha256",
    "locator",
    "as_of",
];

/// A required `YYYY-MM-DD` string.
pub(crate) fn req_date(at: At<'_>, map: &Table, key: &str) -> Result<DateYmd, ParamError> {
    opt_date(at, map, key)?.ok_or_else(|| at.missing(key))
}

fn opt_date(at: At<'_>, map: &Table, key: &str) -> Result<Option<DateYmd>, ParamError> {
    match at.opt_str(map, key)? {
        None => Ok(None),
        Some(s) => DateYmd::parse(s)
            .map(Some)
            .ok_or_else(|| at.invalid(key, "is not a YYYY-MM-DD date")),
    }
}

/// Reads the `[[source]]` array; an absent or empty one is an error.
pub(crate) fn read_sources(table: &str, root: &Table) -> Result<Vec<Source>, ParamError> {
    let top = At { table, path: "" };
    let entries = top.opt_array(root, "source")?.unwrap_or_default();
    if entries.is_empty() {
        return Err(ParamError::MissingProvenance {
            table: table.to_owned(),
        });
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let path = format!("source[{i}]");
            let at = At { table, path: &path };
            let Value::Table(map) = entry else {
                return Err(top.wrong(&path, entry, "a table"));
            };
            at.deny_unknown(map, &SOURCE_FIELDS)?;
            let non_empty = |key: &str| -> Result<String, ParamError> {
                let s = at.req_str(map, key)?;
                if s.trim().is_empty() {
                    return Err(at.invalid(key, "is empty"));
                }
                Ok(s.to_owned())
            };
            let url = non_empty("url")?;
            if !url.starts_with("https://") {
                return Err(at.invalid("url", "is not an https:// URL"));
            }
            let sha256 = non_empty("sha256")?;
            let hex = sha256.len() == 64
                && sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if !hex {
                return Err(at.invalid("sha256", "is not 64 lowercase hex digits"));
            }
            let archive = at.opt_str(map, "archive")?.map(str::to_owned);
            if archive
                .as_deref()
                .is_some_and(|a| !a.starts_with("params/provenance/") || a.contains(".."))
            {
                return Err(at.invalid("archive", "is not a path under params/provenance/"));
            }
            Ok(Source {
                title: non_empty("title")?,
                publisher: at.opt_str(map, "publisher")?.map(str::to_owned),
                url,
                retrieved: req_date(at, map, "retrieved")?,
                archive,
                sha256,
                locator: at.opt_str(map, "locator")?.map(str::to_owned),
                as_of: opt_date(at, map, "as_of")?,
            })
        })
        .collect()
}

/// The `[hand_verification] open = [...]` list; other keys under it are notes.
pub(crate) fn read_open_items(table: &str, root: &Table) -> Result<Vec<String>, ParamError> {
    let top = At { table, path: "" };
    let Some(block) = top.opt_table(root, "hand_verification")? else {
        return Ok(Vec::new());
    };
    let at = At {
        table,
        path: "hand_verification",
    };
    Ok(at.opt_strings(block, "open")?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_strict() {
        let d = DateYmd::parse("2024-02-29").unwrap();
        assert_eq!((d.year, d.month, d.day), (2024, 2, 29));
        assert_eq!(d.to_string(), "2024-02-29");
        for bad in [
            "2025-02-29",
            "2100-02-29",
            "2025-13-01",
            "2025-00-10",
            "2025-04-31",
            "2025-4-01",
            "2025-04-1",
            "20250401",
            "+025-04-01",
            "2025-04-01 ",
            "",
        ] {
            assert_eq!(DateYmd::parse(bad), None, "{bad:?}");
        }
        assert!(DateYmd::parse("2000-02-29").is_some());
    }

    #[test]
    fn only_a_human_sign_off_counts_as_verified() {
        assert!(!VerificationStatus::PendingHandVerification.is_verified());
        assert!(!VerificationStatus::Unstated.is_verified());
        assert!(VerificationStatus::PrimarySourceConfirmed.is_verified());
        assert!(VerificationStatus::HandWorkedReviewed.is_verified());
        assert_eq!(VerificationStatus::Unstated.wire(), None);
    }
}
