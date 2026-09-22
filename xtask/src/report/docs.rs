//! Reading the tables of a design document. The validation report prints the
//! `unverified` and `refused` blocks, the invariant inventory, the security
//! areas and the performance budgets *as the design states them* (`TESTING.md`
//! §5.2, §5.3, §4, §8, §10), so those rows are read from the document rather than
//! copied into code where they would drift from it.
//!
//! The reader knows exactly what it needs: the section under one heading, the
//! Markdown tables in it, and the cells of each row with the emphasis, links and
//! code spans stripped. A heading that is not found is an error, not an empty
//! block, so a renumbered section fails the build instead of emptying the report.

/// The lines of the section that starts at the heading beginning with
/// `heading_prefix` (e.g. `### 5.2`) and ends before the next heading of the
/// same or a higher level.
pub(crate) fn section<'a>(text: &'a str, heading_prefix: &str) -> Result<Vec<&'a str>, String> {
    let mut lines = text.lines();
    let level = heading_prefix.bytes().take_while(|b| *b == b'#').count();
    let start = lines
        .by_ref()
        .find(|line| line.starts_with(heading_prefix))
        .ok_or_else(|| format!("no heading starts with `{heading_prefix}`"))?;
    let mut out = vec![start];
    for line in lines {
        let hashes = line.bytes().take_while(|b| *b == b'#').count();
        if hashes > 0 && hashes <= level && line.as_bytes().get(hashes) == Some(&b' ') {
            break;
        }
        out.push(line);
    }
    Ok(out)
}

/// The data rows of the first Markdown table in `lines`: the header row and
/// the `|---|` separator are dropped, and each row is its cleaned cells.
pub(crate) fn table_rows(lines: &[&str]) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if in_table {
                break;
            }
            continue;
        }
        let cells = split_cells(trimmed);
        if !in_table {
            // The header row; the separator follows.
            in_table = true;
            continue;
        }
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.bytes().all(|b| b == b'-' || b == b':'))
        {
            continue;
        }
        rows.push(cells.iter().map(|c| clean(c)).collect());
    }
    rows
}

/// Cells of one `| a | b |` row, split on pipes outside code spans.
fn split_cells(row: &str) -> Vec<String> {
    let inner = row
        .strip_prefix('|')
        .unwrap_or(row)
        .strip_suffix('|')
        .unwrap_or(row);
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for ch in inner.chars() {
        match ch {
            '`' => {
                in_code = !in_code;
                current.push(ch);
            }
            '|' if !in_code => {
                cells.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    cells.push(current.trim().to_owned());
    cells
}

/// Plain text of a cell: `**bold**` and `` `code` `` markers dropped, `[text](url)`
/// reduced to `text`, HTML line breaks turned into spaces, whitespace collapsed.
pub(crate) fn clean(cell: &str) -> String {
    let mut s = cell.replace("**", "").replace('`', "");
    s = s.replace("<br>", " ").replace("<br/>", " ");
    // `[text](url)` → `text`, repeatedly.
    while let Some(close) = s.find("](") {
        let Some(open) = s[..close].rfind('[') else {
            break;
        };
        let Some(end) = s[close..].find(')') else {
            break;
        };
        let text = s[open + 1..close].to_owned();
        s.replace_range(open..=(close + end), &text);
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first milestone token (`M0` … `M99`, or `1.1`) in `text`, or `not stated`.
pub(crate) fn milestone_in(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'M' && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric()) {
            let digits = bytes[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_digit())
                .count();
            let after = bytes.get(i + 1 + digits);
            if digits > 0 && !after.is_some_and(u8::is_ascii_alphanumeric) {
                return text[i..i + 1 + digits].to_owned();
            }
        }
        i += 1;
    }
    if text
        .split_whitespace()
        .any(|w| w.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.') == "1.1")
    {
        return "1.1".to_owned();
    }
    "not stated".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "\
# Title

## 5. Verification

### 5.1 Done

| A | B |
|---|---|
| done | x |

### 5.2 Open

Some prose.

| Item (why) | Gate |
|---|---|
| **$500** uprating in `code` ([link](https://example.invalid/x)) | **M1**, before the lock |
| A row with a pipe in code `a | b` | Out of v1 scope |

### 5.3 Refused

| Refused target | Why |
|---|---|
| \"**4.15%**\" | Nowhere ([FPA](https://example.invalid)) |

## 6. Next
";

    #[test]
    fn a_section_runs_to_the_next_heading_of_its_level_or_higher() {
        let s = section(DOC, "### 5.2").unwrap();
        assert!(s[0].starts_with("### 5.2"));
        assert!(s.iter().any(|l| l.contains("Out of v1 scope")));
        assert!(!s.iter().any(|l| l.contains("Refused target")));
        let whole = section(DOC, "## 5.").unwrap();
        assert!(whole.iter().any(|l| l.contains("Refused target")));
        assert!(!whole.iter().any(|l| l.contains("## 6.")));
        assert!(section(DOC, "### 9.9").is_err());
    }

    #[test]
    fn rows_are_cleaned_cells_and_code_spans_keep_their_pipes() {
        let rows = table_rows(&section(DOC, "### 5.2").unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], "$500 uprating in code (link)");
        assert_eq!(rows[0][1], "M1, before the lock");
        assert_eq!(rows[1][0], "A row with a pipe in code a | b");
        assert_eq!(rows[1][1], "Out of v1 scope");
        let refused = table_rows(&section(DOC, "### 5.3").unwrap());
        assert_eq!(refused[0][0], "\"4.15%\"");
        assert_eq!(refused[0][1], "Nowhere (FPA)");
    }

    #[test]
    fn the_milestone_is_the_first_m_token() {
        assert_eq!(milestone_in("M1, before the lock"), "M1");
        assert_eq!(milestone_in("M10, and only as an override"), "M10");
        assert_eq!(milestone_in("Out of v1 scope; refit"), "not stated");
        assert_eq!(milestone_in("1.1 (with the disable bundle)"), "1.1");
        assert_eq!(milestone_in("MFJ row moves; M3, with the table"), "M3");
        assert_eq!(milestone_in("M2 reported, M3 gating"), "M2");
    }
}
