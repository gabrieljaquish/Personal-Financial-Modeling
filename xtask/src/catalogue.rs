//! `assumption-catalogue`: `docs/assumption-catalogue.md`, generated from
//! `params/` (`PLAN.md` §4.1: "authoritative for parameter ids").
//!
//! ```text
//! cargo xtask assumption-catalogue           write docs/assumption-catalogue.md
//! cargo xtask assumption-catalogue --check   fail if the committed file is stale
//! ```
//!
//! Every parameter id, with its vintage, verification status, as-of date,
//! projection and rounding rule, published values, sources and open questions, as
//! the loader reads them. The parse is the data-hygiene gate's
//! (`hygiene::parse_vintages`, which is `pfp-params`), so the catalogue cannot
//! name an id the engine would not load. It is a pure function of `params/`: no
//! clock, no path, no person.
//!
//! Amounts are printed in dollars from the integer cents the loader holds; a
//! rate is printed as the exact ratio the table declares. Nothing here is money
//! the engine computed: it is the published table, rendered.

use std::fmt::Write as _;
use std::fs;

use pfp_money::{Cents, Ratio};
use pfp_params::{BaseYear, Increment, ParamTable, RoundingSpec, Source, Vintage};

use crate::{hygiene, repo};

/// The committed catalogue.
pub(crate) const CATALOGUE_PATH: &str = "docs/assumption-catalogue.md";

fn dollars(cents: Cents) -> String {
    let sign = if cents.0 < 0 { "-" } else { "" };
    let abs = cents.0.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    // Thousands separators, from the right.
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i != 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    if frac == 0 {
        format!("{sign}${grouped}")
    } else {
        format!("{sign}${grouped}.{frac:02}")
    }
}

fn ratio(r: Ratio) -> String {
    format!("{}/{}", r.num, r.den)
}

fn rounding_words(spec: &RoundingSpec) -> String {
    let increment = match &spec.increment {
        Increment::Scalar(c) => dollars(*c),
        Increment::ByKey(map) => map
            .iter()
            .map(|(k, c)| format!("{k} {}", dollars(*c)))
            .collect::<Vec<_>>()
            .join(", "),
    };
    format!(
        "increment {increment}; direction {:?}; basis {:?}",
        spec.direction, spec.basis
    )
}

fn source_lines(out: &mut String, sources: &[Source]) {
    for source in sources {
        let _ = writeln!(
            out,
            "- {}{}; retrieved {}; `{}`; archived at `{}`; SHA-256 `{}`{}",
            source.title(),
            source
                .publisher()
                .map_or(String::new(), |p| format!(" ({p})")),
            source.retrieved(),
            source.url(),
            source.archive().unwrap_or("(no archived copy recorded)"),
            source.sha256(),
            source
                .locator()
                .map_or(String::new(), |l| format!("; locator: {l}"))
        );
    }
}

// A rendering: one section, top to bottom, like the table it renders.
#[allow(clippy::too_many_lines)]
fn table_section(out: &mut String, table: &ParamTable, vintage: &Vintage) {
    let _ = writeln!(out, "### `{}`\n", table.id());
    let _ = writeln!(
        out,
        "- Vintage: {} (content id `{}`, {})",
        vintage.name(),
        vintage.content_id(),
        if vintage.id().is_some() {
            "locked"
        } else {
            "unlocked"
        }
    );
    let _ = writeln!(
        out,
        "- Verification: {}",
        table.verification().wire().unwrap_or("unstated")
    );
    let _ = writeln!(out, "- As of: {}", table.as_of());
    let _ = writeln!(
        out,
        "- Period: {}",
        table.period().unwrap_or("(not stated)")
    );
    if !table.components().is_empty() {
        let _ = writeln!(out, "- Components: {}", table.components().join(", "));
    }
    let p = table.projection();
    let base_year = match &p.base_year {
        None => "none".to_owned(),
        Some(BaseYear::Scalar(y)) => y.to_string(),
        Some(BaseYear::ByComponent(map)) => map
            .iter()
            .map(|(from, years)| {
                format!(
                    "from tax year {from}: {}",
                    years
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect::<Vec<_>>()
            .join("; "),
    };
    let _ = writeln!(
        out,
        "- Projection: rule {}; index {}; index series {}; lag {} year(s); base year {}; first adjusted year {}",
        p.rule.wire(),
        p.index.as_deref().unwrap_or("none"),
        p.index_series.as_deref().unwrap_or("none"),
        p.lag_years.map_or("none".to_owned(), |l| l.to_string()),
        base_year,
        p.first_adjusted_year
            .map_or("none".to_owned(), |y| y.to_string())
    );
    if !p.base_values.is_empty() {
        let _ = writeln!(
            out,
            "- Base values: {}",
            p.base_values
                .iter()
                .map(|(k, v)| format!(
                    "{k} {}",
                    v.iter()
                        .map(|c| dollars(*c))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    if !p.derived.is_empty() {
        let _ = writeln!(
            out,
            "- Derived keys: {}",
            p.derived
                .iter()
                .map(|(k, d)| format!("{k} = {} × {}", ratio(d.multiplier), d.from))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    let _ = writeln!(
        out,
        "- Rounding: {}",
        p.rounding
            .as_ref()
            .map_or("none".to_owned(), rounding_words)
    );
    let _ = writeln!(out);

    // Published values: one row per key, one column per published year.
    let mut years: Vec<i32> = Vec::new();
    for key in table.keys() {
        for year in table.published_years(key) {
            if !years.contains(&year) {
                years.push(year);
            }
        }
    }
    years.sort_unstable();
    if !years.is_empty() {
        let _ = writeln!(
            out,
            "Published values ({}):\n",
            if table.components().is_empty() {
                "one amount"
            } else {
                "one amount per component, low to high"
            }
        );
        let _ = writeln!(
            out,
            "| Key | {} |",
            years
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" | ")
        );
        let _ = writeln!(out, "|---|{}", "---|".repeat(years.len()));
        for key in table.keys() {
            let cells: Vec<String> = years
                .iter()
                .map(|year| {
                    table
                        .published(*year, key)
                        .map_or("-".to_owned(), |amounts| {
                            amounts
                                .iter()
                                .map(|c| dollars(*c))
                                .collect::<Vec<_>>()
                                .join(" / ")
                        })
                })
                .collect();
            let _ = writeln!(out, "| {key} | {} |", cells.join(" | "));
        }
        let _ = writeln!(out);
        let mut rate_rows = Vec::new();
        for year in &years {
            if let Some(ladder) = table.rates(*year) {
                let words = ladder
                    .iter()
                    .map(|r| ratio(*r))
                    .collect::<Vec<_>>()
                    .join(", ");
                if rate_rows
                    .last()
                    .is_none_or(|(_, w): &(i32, String)| *w != words)
                {
                    rate_rows.push((*year, words));
                }
            }
        }
        if !rate_rows.is_empty() {
            let _ = writeln!(
                out,
                "Rates (exact ratios, low to high, from the first year each ladder applies):\n"
            );
            for (year, words) in rate_rows {
                let _ = writeln!(out, "- {year}: {words}");
            }
            let _ = writeln!(out);
        }
    }
    let _ = writeln!(out, "Sources:\n");
    source_lines(out, table.sources());
    let _ = writeln!(out);
    if !table.open_items().is_empty() {
        let _ = writeln!(out, "Open questions a person still owes this table:\n");
        for item in table.open_items() {
            let _ = writeln!(out, "- {item}");
        }
        let _ = writeln!(out);
    }
}

/// The catalogue for the vintages of `params/`.
pub(crate) fn render(vintages: &std::collections::BTreeMap<String, Vintage>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Assumption catalogue\n");
    let _ = writeln!(
        out,
        "<!-- GENERATED by `cargo xtask assumption-catalogue` from params/. Do not edit; regenerate. \
`cargo xtask assumption-catalogue --check` fails when this file is stale. -->\n"
    );
    let _ = writeln!(
        out,
        "Every parameter table and archived index series the engine loads, read through \
`pfp-params` exactly as the engine and the data-hygiene gate read them (`ARCHITECTURE.md` §6, \
`DOMAIN-MODEL.md` §15, ADR-010). This file is authoritative for parameter **ids**; the values are \
the published tables rendered, not anything computed. The Assumptions Registry in the application \
serves the same documents.\n"
    );
    if vintages.is_empty() {
        let _ = writeln!(out, "No vintage exists under `params/vintages/`.\n");
        return out;
    }
    let _ = writeln!(
        out,
        "| Vintage | Content id | Locked | Verified | Tables | Index series |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|");
    for (name, vintage) in vintages {
        let series = series_of(vintage);
        let _ = writeln!(
            out,
            "| {name} | `{}` | {} | {} | {} | {} |",
            vintage.content_id(),
            if vintage.id().is_some() { "yes" } else { "no" },
            if vintage.is_verified() { "yes" } else { "no" },
            vintage.tables().count(),
            series.len()
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "A content id is `<name>@<sha256>` over the documents as stored; it claims nothing about \
verification or immutability. Only a vintage `params/VINTAGES.lock` covers is locked, and only a \
locked, verified vintage is for decisions.\n"
    );
    for (name, vintage) in vintages {
        let _ = writeln!(out, "## Vintage `{name}`\n");
        let _ = writeln!(out, "Parameter ids:\n");
        for table in vintage.tables() {
            let _ = writeln!(
                out,
                "- `{}` ({}, as of {})",
                table.id(),
                table.verification().wire().unwrap_or("unstated"),
                table.as_of()
            );
        }
        let _ = writeln!(out);
        for table in vintage.tables() {
            table_section(&mut out, table, vintage);
        }
        for series_name in series_of(vintage) {
            let Some(series) = vintage.index_series(&series_name) else {
                continue;
            };
            let _ = writeln!(out, "### `{}` (index series)\n", series.id());
            let _ = writeln!(out, "- Name tables read it by: {}", series.index_series());
            let _ = writeln!(
                out,
                "- Publisher's series id: {}",
                series.series_id().unwrap_or("(not stated)")
            );
            let _ = writeln!(
                out,
                "- Verification: {}",
                series.verification().wire().unwrap_or("unstated")
            );
            let _ = writeln!(out, "- As of: {}", series.as_of());
            let years: Vec<String> = series.years().map(|y| y.to_string()).collect();
            let _ = writeln!(
                out,
                "- Calendar years covered: {}",
                if years.is_empty() {
                    "none".to_owned()
                } else {
                    format!("{} to {}", years[0], years[years.len() - 1])
                }
            );
            let _ = writeln!(out, "\nSources:\n");
            source_lines(&mut out, series.sources());
            let _ = writeln!(out);
            if !series.open_items().is_empty() {
                let _ = writeln!(out, "Open questions a person still owes this series:\n");
                for item in series.open_items() {
                    let _ = writeln!(out, "- {item}");
                }
                let _ = writeln!(out);
            }
        }
    }
    out
}

/// The `index_series` names the vintage's tables read, sorted and unique.
fn series_of(vintage: &Vintage) -> Vec<String> {
    let mut names: Vec<String> = vintage
        .tables()
        .filter_map(|t| t.projection().index_series.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Parses every vintage in the tree through the hygiene gate's loader.
pub(crate) fn vintages() -> Result<std::collections::BTreeMap<String, Vintage>, String> {
    let root = repo::root();
    let mut documents = Vec::new();
    for path in repo::files(&root)? {
        let rel = repo::relative(&root, &path);
        if hygiene::is_vintage_document(&rel) {
            documents.push((
                rel,
                String::from_utf8_lossy(&repo::read(&path)?).into_owned(),
            ));
        }
    }
    let (violations, vintages) = hygiene::parse_vintages(&documents);
    if violations.is_empty() {
        Ok(vintages)
    } else {
        Err(format!(
            "params/ does not pass the loader:\n  {}",
            violations.join("\n  ")
        ))
    }
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let mut check = false;
    for arg in args {
        match arg.as_str() {
            "--check" => check = true,
            other => return Err(format!("assumption-catalogue: unknown argument `{other}`")),
        }
    }
    let fresh = render(&vintages()?);
    let path = repo::root().join(CATALOGUE_PATH);
    if check {
        let committed = path
            .is_file()
            .then(|| repo::read(&path))
            .transpose()?
            .map(|b| String::from_utf8_lossy(&b).into_owned());
        if committed.as_deref() == Some(fresh.as_str()) {
            eprintln!("assumption-catalogue: {CATALOGUE_PATH} is fresh");
            return Ok(true);
        }
        eprintln!("assumption-catalogue: {CATALOGUE_PATH} differs from a fresh build (run `cargo xtask assumption-catalogue`)");
        return Ok(false);
    }
    fs::write(&path, fresh).map_err(|e| format!("{}: {e}", path.display()))?;
    eprintln!("assumption-catalogue: wrote {CATALOGUE_PATH}");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_catalogue_is_fresh() {
        let fresh = render(&vintages().unwrap());
        let path = repo::root().join(CATALOGUE_PATH);
        let committed = String::from_utf8_lossy(&repo::read(&path).unwrap()).into_owned();
        assert!(
            committed == fresh,
            "{CATALOGUE_PATH} is stale: run `cargo xtask assumption-catalogue`"
        );
        // Deterministic, and every table id of every vintage is in it.
        assert_eq!(fresh, render(&vintages().unwrap()));
        for vintage in vintages().unwrap().values() {
            for table in vintage.tables() {
                assert!(fresh.contains(&format!("### `{}`", table.id())));
            }
        }
    }

    #[test]
    fn dollars_are_grouped_and_exact() {
        assert_eq!(dollars(Cents(2_480_000)), "$24,800");
        assert_eq!(dollars(Cents(123_456_789)), "$1,234,567.89");
        assert_eq!(dollars(Cents(5)), "$0.05");
        assert_eq!(dollars(Cents(-2500)), "-$25");
        assert_eq!(dollars(Cents(0)), "$0");
    }
}
