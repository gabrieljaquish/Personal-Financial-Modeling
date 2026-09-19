//! Walking a parsed TOML document with precise errors.
//!
//! The tables are walked by hand rather than through `serde` derives: year keys
//! are data, a value is an integer or an array depending on the table's shape,
//! and every rejection has to name the table and the field.

use pfp_domain::{year_in_domain, Year};
use toml::{Table, Value};

use crate::error::ParamError;

/// The table id used in errors raised before `id` has been read.
pub(crate) const NO_ID: &str = "<no id>";

/// Where in which table the walker currently is.
#[derive(Clone, Copy)]
pub(crate) struct At<'a> {
    /// Table id for error messages.
    pub(crate) table: &'a str,
    /// Dotted path of the map being read; empty at the top level.
    pub(crate) path: &'a str,
}

impl At<'_> {
    pub(crate) fn field(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    pub(crate) fn missing(&self, key: &str) -> ParamError {
        ParamError::Missing {
            table: self.table.to_owned(),
            field: self.field(key),
        }
    }

    pub(crate) fn invalid(&self, key: &str, reason: impl Into<String>) -> ParamError {
        ParamError::Invalid {
            table: self.table.to_owned(),
            field: self.field(key),
            reason: reason.into(),
        }
    }

    /// A type error at `key`; a float gets its own, more useful, error.
    pub(crate) fn wrong(&self, key: &str, found: &Value, expected: &'static str) -> ParamError {
        let (table, field) = (self.table.to_owned(), self.field(key));
        if found.is_float() {
            ParamError::Float { table, field }
        } else {
            ParamError::WrongType {
                table,
                field,
                expected,
            }
        }
    }

    /// Rejects every key of `map` not in `allowed`.
    pub(crate) fn deny_unknown(&self, map: &Table, allowed: &[&str]) -> Result<(), ParamError> {
        match map.keys().find(|k| !allowed.contains(&k.as_str())) {
            Some(key) => Err(ParamError::UnknownField {
                table: self.table.to_owned(),
                field: self.field(key),
            }),
            None => Ok(()),
        }
    }

    pub(crate) fn opt_str<'m>(
        &self,
        map: &'m Table,
        key: &str,
    ) -> Result<Option<&'m str>, ParamError> {
        match map.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s)),
            Some(other) => Err(self.wrong(key, other, "a string")),
        }
    }

    pub(crate) fn req_str<'m>(&self, map: &'m Table, key: &str) -> Result<&'m str, ParamError> {
        self.opt_str(map, key)?.ok_or_else(|| self.missing(key))
    }

    pub(crate) fn opt_int(&self, map: &Table, key: &str) -> Result<Option<i64>, ParamError> {
        match map.get(key) {
            None => Ok(None),
            Some(Value::Integer(i)) => Ok(Some(*i)),
            Some(other) => Err(self.wrong(key, other, "an integer")),
        }
    }

    pub(crate) fn opt_table<'m>(
        &self,
        map: &'m Table,
        key: &str,
    ) -> Result<Option<&'m Table>, ParamError> {
        match map.get(key) {
            None => Ok(None),
            Some(Value::Table(t)) => Ok(Some(t)),
            Some(other) => Err(self.wrong(key, other, "a table")),
        }
    }

    pub(crate) fn opt_array<'m>(
        &self,
        map: &'m Table,
        key: &str,
    ) -> Result<Option<&'m [Value]>, ParamError> {
        match map.get(key) {
            None => Ok(None),
            Some(Value::Array(a)) => Ok(Some(a)),
            Some(other) => Err(self.wrong(key, other, "an array")),
        }
    }

    /// An array of strings.
    pub(crate) fn opt_strings(
        &self,
        map: &Table,
        key: &str,
    ) -> Result<Option<Vec<String>>, ParamError> {
        let Some(items) = self.opt_array(map, key)? else {
            return Ok(None);
        };
        items
            .iter()
            .map(|item| match item {
                Value::String(s) => Ok(s.clone()),
                other => Err(self.wrong(key, other, "an array of strings")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    /// A year written as a TOML integer value.
    pub(crate) fn opt_year(&self, map: &Table, key: &str) -> Result<Option<Year>, ParamError> {
        let Some(raw) = self.opt_int(map, key)? else {
            return Ok(None);
        };
        Year::try_from(raw)
            .ok()
            .filter(|y| year_in_domain(*y))
            .map(Some)
            .ok_or_else(|| self.invalid(key, "is not a year in 1900..=2200"))
    }

    /// A year written as a TOML key (`2026 = ...`).
    pub(crate) fn year_key(&self, key: &str) -> Result<Year, ParamError> {
        let plain = !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit());
        key.parse::<Year>()
            .ok()
            .filter(|y| plain && year_in_domain(*y))
            .ok_or_else(|| self.invalid(key, "is not a year key in 1900..=2200"))
    }
}

/// Parses TOML text into its root table.
pub(crate) fn parse_document(text: &str) -> Result<Table, ParamError> {
    text.parse::<Table>().map_err(|e| ParamError::Syntax {
        message: e.to_string(),
    })
}

/// The document's `id`, which every later error is reported against.
pub(crate) fn document_id(root: &Table) -> Result<&str, ParamError> {
    let at = At {
        table: NO_ID,
        path: "",
    };
    let id = at.req_str(root, "id")?;
    if id.is_empty() {
        return Err(at.invalid("id", "is empty"));
    }
    Ok(id)
}
