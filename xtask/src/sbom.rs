//! `cargo xtask sbom`: the `CycloneDX` SBOM and the no-copyleft assertion
//! (`ARCHITECTURE.md` §9 item 7; `SECURITY.md` §11 "SBOM" row; ADR-004).
//!
//! ```text
//! cargo xtask sbom [--out DIR] [--target TRIPLE]...
//!     writes DIR/sbom-rust-<target>.cdx.json (cargo-cyclonedx, pinned) and
//!     DIR/sbom-web.cdx.json (generated here from web/package-lock.json), then
//!     runs the assertion over them
//! cargo xtask sbom --check FILE...
//!     the assertion alone, over any CycloneDX JSON documents
//! ```
//!
//! **Rust half: the named tool.** `cargo-cyclonedx` is installed at an exact
//! version with `cargo install --locked` (its own lockfile pins its graph), and
//! this command refuses any other version. Its output is kept byte-for-byte as
//! the tool wrote it. One observed property matters for the assertion: for a
//! workspace it lists the **dev-dependencies of path dependencies** too
//! (`x509-parser`, `num-bigint`, … — test-only crates that are not in the
//! executable). The assertion therefore classifies every Rust component against
//! `pfp-app`'s own dependency graph for the target, read from `cargo tree`:
//! reachable over normal edges = *linked*; only over build edges = *build-time*;
//! not reachable = *not shipped*. Linked and build-time are held to the shipped
//! standard. With `--check` alone there is no graph, and every Rust component is
//! treated as shipped — the stricter reading.
//!
//! **npm half: generated here, from `web/package-lock.json`.** The named tool,
//! `cyclonedx-npm`, can be pinned only at its own top level: `npx
//! @cyclonedx/cyclonedx-npm@<exact>` resolves that package's dependencies from
//! caret ranges at run time, with no lockfile, which is precisely the unpinned
//! install `SECURITY.md` §11 forbids on a release path. The lockfile already holds
//! everything an SBOM states — name, exact version, declared licence, the
//! `sha512` integrity digest, and whether the package is a `devDependency` — so
//! it is transcribed here with no dependency and no network. Packages that are
//! not `dev` are bundled into `web/dist` (shipped, `scope: required`);
//! `devDependencies` are build and test tools (`scope: excluded`, property
//! `pfp:shipped = false`). Vite's own small runtime helpers can be emitted into
//! the bundle; Vite is MIT and is listed, so nothing is hidden by that edge.
//!
//! **The assertion.** Every component's licence is an SPDX expression (the
//! legacy `/` is read as `OR`, `WITH <exception>` as its base licence). It fails:
//!
//! * for **any** component, shipped or not, whose expression names a strong
//!   copyleft or non-open licence family (GPL, AGPL, LGPL, `PolyForm`, Parity,
//!   SSPL, BUSL, Commons Clause, non-commercial or no-derivatives Creative
//!   Commons) **anywhere**, including one arm of an `OR` — a dual licence with a
//!   copyleft arm is a decision for a human and `docs/licence-watchlist.md`, not
//!   for an evaluator;
//! * for **any** component whose licence is absent, `NOASSERTION`, `NONE`,
//!   `UNLICENSED`, a `LicenseRef-` or otherwise unparseable (unknown);
//! * for a **shipped** component, when the expression names a weak/file-level
//!   copyleft family (MPL, EPL, CDDL, EUPL, OSL, CPL) anywhere, or is not
//!   satisfiable from the ADR-004 allowlist.
//!
//! A not-shipped component outside the allowlist but in none of the families
//! above is reported as a warning.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

use crate::repo;

/// The pinned `cargo-cyclonedx`. The release workflow installs exactly this.
pub(crate) const CARGO_CYCLONEDX_VERSION: &str = "0.5.9";

/// The ADR-004 allowlist, identical to `deny.toml` and `web/scripts/check-licences.mjs`.
const ALLOWED: [&str; 8] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "CC0-1.0",
    "Unicode-3.0",
    "Zlib",
];

/// Families refused for every component, shipped or not (prefix match, case-insensitive).
const FORBIDDEN_ANYWHERE: [&str; 11] = [
    "GPL",
    "AGPL",
    "LGPL",
    "PolyForm",
    "Parity",
    "SSPL",
    "BUSL",
    "Commons-Clause",
    "CC-BY-NC",
    "CC-BY-ND",
    "CC-BY-NC-SA",
];

/// Families refused for shipped components.
const FORBIDDEN_SHIPPED: [&str; 7] = ["MPL", "EPL", "CDDL", "EUPL", "OSL", "CPL", "CC-BY-SA"];

/// Licence exceptions understood after `WITH`.
const KNOWN_EXCEPTIONS: [&str; 1] = ["LLVM-exception"];

/// The two macOS targets of the universal build.
pub(crate) const MAC_TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

// ---------------------------------------------------------------------------
// SPDX expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    Id(String),
    With(String, String),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

fn tokenize(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in text.chars() {
        match c {
            '(' | ')' | '/' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(if c == '/' {
                    "OR".to_owned()
                } else {
                    c.to_string()
                });
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

struct Parser {
    tokens: Vec<String>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.at).map(String::as_str)
    }
    fn next(&mut self) -> Option<String> {
        let t = self.tokens.get(self.at).cloned();
        self.at += 1;
        t
    }
    fn or(&mut self) -> Result<Expr, String> {
        let mut left = self.and()?;
        while self.peek().is_some_and(|t| t.eq_ignore_ascii_case("OR")) {
            self.next();
            left = Expr::Or(Box::new(left), Box::new(self.and()?));
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<Expr, String> {
        let mut left = self.with()?;
        while self.peek().is_some_and(|t| t.eq_ignore_ascii_case("AND")) {
            self.next();
            left = Expr::And(Box::new(left), Box::new(self.with()?));
        }
        Ok(left)
    }
    fn with(&mut self) -> Result<Expr, String> {
        let base = self.primary()?;
        if self.peek().is_some_and(|t| t.eq_ignore_ascii_case("WITH")) {
            self.next();
            let Expr::Id(id) = base else {
                return Err("WITH must follow a licence identifier".to_owned());
            };
            let exception = self.next().ok_or("WITH needs an exception")?;
            return Ok(Expr::With(id, exception));
        }
        Ok(base)
    }
    fn primary(&mut self) -> Result<Expr, String> {
        match self.next() {
            Some(t) if t == "(" => {
                let inner = self.or()?;
                if self.next().as_deref() != Some(")") {
                    return Err("unbalanced parenthesis".to_owned());
                }
                Ok(inner)
            }
            Some(t)
                if t == ")"
                    || ["AND", "OR", "WITH"]
                        .iter()
                        .any(|k| t.eq_ignore_ascii_case(k)) =>
            {
                Err(format!("unexpected `{t}`"))
            }
            Some(t) => Ok(Expr::Id(t)),
            None => Err("empty expression".to_owned()),
        }
    }
}

fn parse_expr(text: &str) -> Result<Expr, String> {
    let mut p = Parser {
        tokens: tokenize(text),
        at: 0,
    };
    let e = p.or()?;
    if p.at != p.tokens.len() {
        return Err(format!("trailing tokens in `{text}`"));
    }
    Ok(e)
}

fn ids(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Id(id) | Expr::With(id, _) => out.push(id.clone()),
        Expr::And(a, b) | Expr::Or(a, b) => {
            ids(a, out);
            ids(b, out);
        }
    }
}

fn exceptions_known(e: &Expr) -> bool {
    match e {
        Expr::Id(_) => true,
        Expr::With(_, ex) => KNOWN_EXCEPTIONS.contains(&ex.as_str()),
        Expr::And(a, b) | Expr::Or(a, b) => exceptions_known(a) && exceptions_known(b),
    }
}

fn satisfiable(e: &Expr) -> bool {
    match e {
        Expr::Id(id) | Expr::With(id, _) => ALLOWED.contains(&id.as_str()),
        Expr::And(a, b) => satisfiable(a) && satisfiable(b),
        Expr::Or(a, b) => satisfiable(a) || satisfiable(b),
    }
}

fn in_family(id: &str, families: &[&str]) -> bool {
    let lower = id.to_ascii_lowercase();
    families
        .iter()
        .any(|f| lower.starts_with(&f.to_ascii_lowercase()))
}

/// Where a component's code ends up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shipped {
    /// Linked into the executable, or bundled into `web/dist`.
    Linked,
    /// Runs at build time (a build-script dependency); held to the shipped standard.
    BuildTime,
    /// A development or test tool that reaches no artifact.
    No,
}

impl Shipped {
    fn label(self) -> &'static str {
        match self {
            Self::Linked => "shipped",
            Self::BuildTime => "build-time (held to the shipped standard)",
            Self::No => "not shipped",
        }
    }
}

/// The assertion's verdict on one licence expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verdict {
    Ok,
    Warn(String),
    Fail(String),
}

pub(crate) fn judge(licence: Option<&str>, shipped: Shipped) -> Verdict {
    let text = licence.map_or("", str::trim);
    if text.is_empty() {
        return Verdict::Fail("no licence declared (unknown)".to_owned());
    }
    let expr = match parse_expr(text) {
        Ok(e) => e,
        Err(e) => return Verdict::Fail(format!("unparseable licence `{text}` (unknown): {e}")),
    };
    let mut names = Vec::new();
    ids(&expr, &mut names);
    for id in &names {
        let upper = id.to_ascii_uppercase();
        if upper == "NOASSERTION"
            || upper == "NONE"
            || upper == "UNLICENSED"
            || upper.starts_with("LICENSEREF-")
        {
            return Verdict::Fail(format!("`{text}`: {id} is an unknown licence"));
        }
        if in_family(id, &FORBIDDEN_ANYWHERE) {
            return Verdict::Fail(format!("`{text}` names {id}, which no component may carry"));
        }
    }
    if !exceptions_known(&expr) {
        return Verdict::Fail(format!("`{text}`: unknown licence exception (unknown)"));
    }
    let weak = names.iter().find(|id| in_family(id, &FORBIDDEN_SHIPPED));
    let allowed = satisfiable(&expr);
    match shipped {
        Shipped::Linked | Shipped::BuildTime => {
            if let Some(id) = weak {
                Verdict::Fail(format!("`{text}` names {id}, which may not ship"))
            } else if !allowed {
                Verdict::Fail(format!(
                    "`{text}` is not satisfiable from the ADR-004 allowlist"
                ))
            } else {
                Verdict::Ok
            }
        }
        Shipped::No => {
            if let Some(id) = weak {
                Verdict::Warn(format!(
                    "`{text}` names {id}; acceptable only because it does not ship"
                ))
            } else if !allowed {
                Verdict::Warn(format!(
                    "`{text}` is outside the ADR-004 allowlist; it does not ship"
                ))
            } else {
                Verdict::Ok
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Reading CycloneDX documents
// ---------------------------------------------------------------------------

/// One component as the assertion sees it.
#[derive(Debug, Clone)]
pub(crate) struct Component {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) licence: Option<String>,
    /// From the `pfp:shipped` property, when the document carries one.
    pub(crate) shipped_property: Option<bool>,
    pub(crate) ecosystem: String,
}

fn licence_of(c: &Value) -> Option<String> {
    let list = c.get("licenses")?.as_array()?;
    let parts: Vec<String> = list
        .iter()
        .filter_map(|l| {
            l.get("expression")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    let lic = l.get("license")?;
                    lic.get("id")
                        .or_else(|| lic.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
        })
        .collect();
    match parts.len() {
        0 => None,
        1 => Some(parts[0].clone()),
        // Several licence entries on one component apply together.
        _ => Some(
            parts
                .iter()
                .map(|p| format!("({p})"))
                .collect::<Vec<_>>()
                .join(" AND "),
        ),
    }
}

fn collect_components(v: &Value, out: &mut Vec<Component>) {
    let Some(list) = v.get("components").and_then(Value::as_array) else {
        return;
    };
    for c in list {
        let purl = c.get("purl").and_then(Value::as_str).unwrap_or("");
        let is_target = purl.contains('#');
        // cargo-cyclonedx lists a crate's Cargo targets (lib, bin) as
        // sub-components without licences; they are the crate itself.
        if !is_target {
            let group = c.get("group").and_then(Value::as_str).unwrap_or("");
            let name = c.get("name").and_then(Value::as_str).unwrap_or("?");
            let shipped_property = c
                .get("properties")
                .and_then(Value::as_array)
                .and_then(|props| {
                    props
                        .iter()
                        .find(|p| p.get("name").and_then(Value::as_str) == Some("pfp:shipped"))
                })
                .and_then(|p| p.get("value").and_then(Value::as_str))
                .map(|v| v == "true");
            out.push(Component {
                name: if group.is_empty() {
                    name.to_owned()
                } else {
                    format!("{group}/{name}")
                },
                version: c
                    .get("version")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .to_owned(),
                licence: licence_of(c),
                shipped_property,
                ecosystem: purl
                    .strip_prefix("pkg:")
                    .and_then(|p| p.split('/').next())
                    .unwrap_or("unknown")
                    .to_owned(),
            });
        }
        collect_components(c, out);
    }
}

/// Every component of a `CycloneDX` JSON document, the described root included.
pub(crate) fn components(doc: &Value) -> Result<Vec<Component>, String> {
    if doc.get("bomFormat").and_then(Value::as_str) != Some("CycloneDX") {
        return Err("not a CycloneDX document (bomFormat)".to_owned());
    }
    let mut out = Vec::new();
    if let Some(root) = doc.get("metadata").and_then(|m| m.get("component")) {
        collect_components(&json!({ "components": [root.clone()] }), &mut out);
    }
    collect_components(doc, &mut out);
    Ok(out)
}

/// The Rust graph of `pfp-app` for one target: `(name, version)` → where it ends up.
pub(crate) type Graph = BTreeMap<(String, String), Shipped>;

/// Runs the assertion over `docs`, printing one line per component. `graph`
/// classifies Rust components that carry no `pfp:shipped` property.
pub(crate) fn assert_docs(docs: &[(String, Value)], graph: Option<&Graph>) -> Result<bool, String> {
    let mut failures = 0;
    let mut warnings = 0;
    let mut total = 0;
    let mut by_class: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (label, doc) in docs {
        println!("sbom: {label}");
        for c in components(doc)? {
            total += 1;
            let shipped = match c.shipped_property {
                Some(true) => Shipped::Linked,
                Some(false) => Shipped::No,
                None => match graph {
                    Some(g) if c.ecosystem == "cargo" => g
                        .get(&(c.name.clone(), c.version.clone()))
                        .copied()
                        .unwrap_or(Shipped::No),
                    _ => Shipped::Linked,
                },
            };
            *by_class.entry(shipped.label()).or_default() += 1;
            let verdict = judge(c.licence.as_deref(), shipped);
            let (tag, note) = match &verdict {
                Verdict::Ok => ("ok  ", String::new()),
                Verdict::Warn(w) => {
                    warnings += 1;
                    ("WARN", format!(" — {w}"))
                }
                Verdict::Fail(f) => {
                    failures += 1;
                    ("FAIL", format!(" — {f}"))
                }
            };
            println!(
                "  {tag} {}@{} [{}] {}{note}",
                c.name,
                c.version,
                c.licence.as_deref().unwrap_or("(none)"),
                shipped.label()
            );
        }
    }
    let classes: Vec<String> = by_class.iter().map(|(k, v)| format!("{v} {k}")).collect();
    println!(
        "sbom: {total} component(s): {}; {failures} failure(s), {warnings} warning(s)",
        classes.join(", ")
    );
    if failures == 0 {
        println!("sbom: no GPL/AGPL/LGPL/PolyForm/Parity/unknown component; no MPL-family or non-allowlisted licence in shipped code");
    }
    Ok(failures == 0)
}

// ---------------------------------------------------------------------------
// The Rust graph, from `cargo tree`
// ---------------------------------------------------------------------------
//
// `cargo tree` and not `cargo metadata`: metadata resolves features once for the
// whole workspace, dev-dependencies included, so a feature that only a test
// enables shows up there as a normal edge (`rcgen` -> `x509-parser`). `cargo
// tree -p pfp-app --target T -e normal` runs the real feature resolver for the
// build that ships.

fn cargo_tree(root: &Path, target: &str, edges: &str) -> Result<String, String> {
    let out = Command::new("cargo")
        .current_dir(root)
        .args([
            "tree",
            "--locked",
            "-p",
            "pfp-app",
            "--target",
            target,
            "-e",
            edges,
            "--prefix",
            "none",
            "--no-dedupe",
            "-f",
            "{p}",
        ])
        .output()
        .map_err(|e| format!("cargo tree: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo tree failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `(name, version)` of every line of `cargo tree --prefix none -f {p}` output
/// (`name vX.Y.Z` optionally followed by a path or `(proc-macro)`).
fn tree_packages(text: &str) -> Result<BTreeSet<(String, String)>, String> {
    let mut out = BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let mut parts = line.split_whitespace();
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            return Err(format!("cargo tree: unexpected line `{line}`"));
        };
        let version = version
            .strip_prefix('v')
            .ok_or_else(|| format!("cargo tree: no version in `{line}`"))?;
        out.insert((name.to_owned(), version.to_owned()));
    }
    Ok(out)
}

/// Classifies from the two listings: over normal edges = linked; only over
/// normal + build edges = build-time.
pub(crate) fn graph_from_tree(normal: &str, normal_and_build: &str) -> Result<Graph, String> {
    let linked = tree_packages(normal)?;
    let all = tree_packages(normal_and_build)?;
    if !linked.is_subset(&all) {
        return Err("cargo tree: the normal graph is not inside the normal+build graph".to_owned());
    }
    Ok(all
        .into_iter()
        .map(|key| {
            let class = if linked.contains(&key) {
                Shipped::Linked
            } else {
                Shipped::BuildTime
            };
            (key, class)
        })
        .collect())
}

/// `pfp-app`'s dependency graph for `target`, classified by edge kind.
pub(crate) fn rust_graph(root: &Path, target: &str) -> Result<Graph, String> {
    graph_from_tree(
        &cargo_tree(root, target, "normal")?,
        &cargo_tree(root, target, "normal,build")?,
    )
}

// ---------------------------------------------------------------------------
// The npm SBOM, from the lockfile
// ---------------------------------------------------------------------------

fn base64_to_hex(text: &str) -> Option<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bits: u32 = 0;
    let mut nbits = 0;
    let mut out = String::new();
    for c in text.trim_end_matches('=').bytes() {
        let v = u32::try_from(ALPHABET.iter().position(|&a| a == c)?).ok()?;
        bits = (bits << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            let _ = write!(out, "{:02x}", (bits >> nbits) & 0xff);
        }
    }
    Some(out)
}

fn purl_npm(name: &str, version: &str) -> String {
    format!("pkg:npm/{}@{version}", name.replace('@', "%40"))
}

/// The `CycloneDX` 1.5 document for the npm packages of `web/package-lock.json`.
/// Deterministic: no timestamp, no serial number, components in lockfile-key order.
#[allow(clippy::too_many_lines)]
pub(crate) fn web_sbom(lock: &Value) -> Result<Value, String> {
    if lock["lockfileVersion"].as_u64().unwrap_or(0) < 2 {
        return Err("web/package-lock.json: lockfileVersion 2 or later is required".to_owned());
    }
    let packages = lock["packages"]
        .as_object()
        .ok_or("web/package-lock.json: no `packages` map")?;
    let root = packages
        .get("")
        .ok_or("web/package-lock.json: no root package")?;
    let root_name = lock["name"].as_str().unwrap_or("web");
    let root_version = root["version"].as_str().unwrap_or("0.0.0");
    let mut components = Vec::new();
    for (key, entry) in packages {
        if key.is_empty() {
            continue;
        }
        if entry["link"].as_bool() == Some(true) {
            return Err(format!(
                "{key}: a linked package has no lockfile metadata to transcribe"
            ));
        }
        let full = key
            .rsplit_once("node_modules/")
            .map_or(key.as_str(), |(_, n)| n);
        let (group, name) = match full.split_once('/') {
            Some((g, n)) if g.starts_with('@') => (Some(g), n),
            _ => (None, full),
        };
        let version = entry["version"]
            .as_str()
            .ok_or_else(|| format!("{key}: no version"))?;
        let dev = entry["dev"].as_bool() == Some(true);
        let mut c = serde_json::Map::new();
        c.insert("type".into(), json!("library"));
        c.insert(
            "bom-ref".into(),
            json!(format!("{}#{key}", purl_npm(full, version))),
        );
        if let Some(g) = group {
            c.insert("group".into(), json!(g));
        }
        c.insert("name".into(), json!(name));
        c.insert("version".into(), json!(version));
        c.insert(
            "scope".into(),
            json!(if dev { "excluded" } else { "required" }),
        );
        if let Some(l) = entry["license"].as_str() {
            c.insert("licenses".into(), json!([{ "expression": l }]));
        }
        c.insert("purl".into(), json!(purl_npm(full, version)));
        if let Some(sri) = entry["integrity"].as_str() {
            if let Some(b64) = sri.strip_prefix("sha512-") {
                let hex = base64_to_hex(b64).ok_or_else(|| format!("{key}: bad integrity"))?;
                c.insert(
                    "hashes".into(),
                    json!([{ "alg": "SHA-512", "content": hex }]),
                );
            }
        }
        if let Some(url) = entry["resolved"].as_str() {
            c.insert(
                "externalReferences".into(),
                json!([{ "type": "distribution", "url": url }]),
            );
        }
        let mut props = vec![
            json!({ "name": "pfp:shipped", "value": if dev { "false" } else { "true" } }),
            json!({
                "name": "pfp:classification",
                "value": if dev {
                    "devDependency: a build or test tool; not bundled into web/dist"
                } else {
                    "runtime dependency: bundled into web/dist, which is embedded in the executable"
                }
            }),
        ];
        if dev {
            props.push(json!({ "name": "cdx:npm:package:development", "value": "true" }));
        }
        if entry["optional"].as_bool() == Some(true) {
            props.push(json!({ "name": "cdx:npm:package:optional", "value": "true" }));
        }
        c.insert("properties".into(), Value::Array(props));
        components.push(Value::Object(c));
    }
    Ok(json!({
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "tools": { "components": [{
                "type": "application",
                "name": "xtask sbom (transcribed from web/package-lock.json)",
                "version": env!("CARGO_PKG_VERSION"),
            }]},
            "component": {
                "type": "application",
                "bom-ref": purl_npm(root_name, root_version),
                "name": root_name,
                "version": root_version,
                "scope": "required",
                "licenses": root["license"].as_str().map(|l| json!([{ "expression": l }])),
                "purl": purl_npm(root_name, root_version),
                "properties": [{ "name": "pfp:shipped", "value": "true" }],
            },
        },
        "components": components,
    }))
}

// ---------------------------------------------------------------------------
// The command
// ---------------------------------------------------------------------------

/// Removes files when dropped. `cargo cyclonedx` writes its output beside the
/// manifest of **every** workspace member (observed with 0.5.9: `crates/*/` and
/// `xtask/`, whatever `--manifest-path` names), and nothing it writes may be
/// left in the source tree, on success or on error.
struct Remove(Vec<PathBuf>);
impl Drop for Remove {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

/// The directory of every workspace member's manifest, and the root.
fn member_dirs(root: &Path) -> Result<Vec<PathBuf>, String> {
    let out = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let meta: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    let mut dirs = vec![root.to_path_buf()];
    for p in meta["packages"].as_array().map_or(&[][..], Vec::as_slice) {
        if let Some(dir) = p["manifest_path"]
            .as_str()
            .and_then(|m| Path::new(m).parent())
        {
            dirs.push(dir.to_path_buf());
        }
    }
    Ok(dirs)
}

fn cargo_cyclonedx_version() -> Result<String, String> {
    let out = Command::new("cargo")
        .args(["cyclonedx", "--version"])
        .output()
        .map_err(|e| format!("cargo cyclonedx: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo-cyclonedx is not installed: cargo install --locked --version {CARGO_CYCLONEDX_VERSION} cargo-cyclonedx"
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .last()
        .unwrap_or("")
        .to_owned())
}

fn rust_sbom(
    root: &Path,
    target: &str,
    out_dir: &Path,
    epoch: Option<u64>,
) -> Result<PathBuf, String> {
    let stem = format!("xtask-sbom-{}-{target}", std::process::id());
    let written = root
        .join("crates")
        .join("pfp-app")
        .join(format!("{stem}.json"));
    let file = format!("{stem}.json");
    let guard = Remove(member_dirs(root)?.iter().map(|d| d.join(&file)).collect());
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args([
        "cyclonedx",
        "--manifest-path",
        "crates/pfp-app/Cargo.toml",
        "--format",
        "json",
        "--spec-version",
        "1.5",
        "--target",
        target,
        "--override-filename",
        &stem,
        "-q",
    ]);
    if let Some(e) = epoch {
        cmd.env("SOURCE_DATE_EPOCH", e.to_string());
    }
    let status = cmd.status().map_err(|e| format!("cargo cyclonedx: {e}"))?;
    if !status.success() {
        return Err(format!("cargo cyclonedx failed ({status})"));
    }
    let dest = out_dir.join(format!("sbom-rust-{target}.cdx.json"));
    fs::copy(&written, &dest).map_err(|e| format!("{}: {e}", written.display()))?;
    let written_anywhere: Vec<PathBuf> = guard.0.clone();
    drop(guard);
    if let Some(left) = written_anywhere.iter().find(|p| p.exists()) {
        return Err(format!(
            "{}: cargo cyclonedx output left in the source tree",
            left.display()
        ));
    }
    Ok(dest)
}

fn read_doc(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&repo::read(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

pub(crate) fn run(args: &[String]) -> Result<bool, String> {
    let root = repo::root();
    let mut out_dir = None;
    let mut targets: Vec<String> = Vec::new();
    let mut check_files = None;
    let mut epoch = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => out_dir = Some(PathBuf::from(it.next().ok_or("--out needs a directory")?)),
            "--target" => targets.push(it.next().ok_or("--target needs a triple")?.clone()),
            "--source-date-epoch" => {
                epoch = Some(
                    it.next()
                        .ok_or("--source-date-epoch needs a value")?
                        .parse::<u64>()
                        .map_err(|_| "--source-date-epoch: not a number")?,
                );
            }
            "--check" => {
                check_files = Some(it.by_ref().map(PathBuf::from).collect::<Vec<_>>());
            }
            other => return Err(format!("sbom: unknown argument `{other}`")),
        }
    }
    if let Some(files) = check_files {
        if files.is_empty() {
            return Err("sbom --check needs at least one CycloneDX JSON file".to_owned());
        }
        let docs = files
            .iter()
            .map(|f| Ok((f.display().to_string(), read_doc(f)?)))
            .collect::<Result<Vec<_>, String>>()?;
        return assert_docs(&docs, None);
    }

    let version = cargo_cyclonedx_version()?;
    if version != CARGO_CYCLONEDX_VERSION {
        return Err(format!(
            "cargo-cyclonedx {version} is installed; this pipeline is pinned to {CARGO_CYCLONEDX_VERSION}: \
             cargo install --locked --version {CARGO_CYCLONEDX_VERSION} cargo-cyclonedx"
        ));
    }
    if targets.is_empty() {
        targets = MAC_TARGETS.iter().map(|t| (*t).to_owned()).collect();
    }
    let out_dir =
        out_dir.unwrap_or_else(|| crate::dist::target_dir(&root).join("dist").join("sbom"));
    fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let epoch = match epoch {
        Some(e) => Some(e),
        None => crate::dist::commit_epoch(&root).ok(),
    };

    let mut ok = true;
    for target in &targets {
        let path = rust_sbom(&root, target, &out_dir, epoch)?;
        let graph = rust_graph(&root, target)?;
        eprintln!("sbom: wrote {}", path.display());
        ok &= assert_docs(
            &[(path.display().to_string(), read_doc(&path)?)],
            Some(&graph),
        )?;
    }
    let lock = read_doc(&root.join("web").join("package-lock.json"))?;
    let web = web_sbom(&lock)?;
    let web_path = out_dir.join("sbom-web.cdx.json");
    let mut text = serde_json::to_string_pretty(&web).map_err(|e| e.to_string())?;
    text.push('\n');
    fs::write(&web_path, text).map_err(|e| format!("{}: {e}", web_path.display()))?;
    eprintln!("sbom: wrote {}", web_path.display());
    ok &= assert_docs(&[(web_path.display().to_string(), web)], None)?;
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spdx_expressions_evaluate() {
        let ok = |t: &str| judge(Some(t), Shipped::Linked);
        assert_eq!(ok("MIT"), Verdict::Ok);
        assert_eq!(ok("MIT OR Apache-2.0"), Verdict::Ok);
        assert_eq!(ok("MIT/Apache-2.0"), Verdict::Ok);
        assert_eq!(ok("Unlicense/MIT"), Verdict::Ok);
        assert_eq!(ok("(MIT OR Apache-2.0) AND Unicode-3.0"), Verdict::Ok);
        assert_eq!(
            ok("Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT"),
            Verdict::Ok
        );
        assert_eq!(ok("Apache-2.0 AND ISC"), Verdict::Ok);
        // Not satisfiable from the allowlist.
        assert!(matches!(ok("Unlicense"), Verdict::Fail(_)));
        assert!(matches!(ok("MIT AND Unlicense"), Verdict::Fail(_)));
        // Malformed.
        assert!(matches!(ok("MIT OR"), Verdict::Fail(_)));
        assert!(matches!(ok("(MIT"), Verdict::Fail(_)));
        assert!(matches!(
            ok("Apache-2.0 WITH Some-exception"),
            Verdict::Fail(_)
        ));
    }

    #[test]
    fn copyleft_and_unknown_fail_everywhere() {
        for shipped in [Shipped::Linked, Shipped::BuildTime, Shipped::No] {
            for text in [
                "GPL-3.0-only",
                "AGPL-3.0-or-later",
                "LGPL-2.1",
                "MIT OR GPL-2.0",
                "PolyForm-Noncommercial-1.0.0",
                "Parity-7.0.0",
                "NOASSERTION",
                "LicenseRef-proprietary",
                "UNLICENSED",
                "",
            ] {
                assert!(
                    matches!(judge(Some(text), shipped), Verdict::Fail(_)),
                    "{text} / {shipped:?}"
                );
            }
            assert!(matches!(judge(None, shipped), Verdict::Fail(_)));
        }
    }

    #[test]
    fn weak_copyleft_fails_only_when_shipped() {
        assert!(matches!(
            judge(Some("MPL-2.0"), Shipped::Linked),
            Verdict::Fail(_)
        ));
        assert!(matches!(
            judge(Some("MIT OR MPL-2.0"), Shipped::BuildTime),
            Verdict::Fail(_)
        ));
        assert!(matches!(
            judge(Some("MPL-2.0"), Shipped::No),
            Verdict::Warn(_)
        ));
        assert!(matches!(
            judge(Some("Unlicense"), Shipped::No),
            Verdict::Warn(_)
        ));
        assert_eq!(judge(Some("MIT"), Shipped::No), Verdict::Ok);
    }

    fn lock() -> Value {
        json!({
            "name": "web",
            "lockfileVersion": 3,
            "packages": {
                "": { "name": "web", "version": "0.0.0", "license": "Apache-2.0" },
                "node_modules/react": {
                    "version": "1.0.0",
                    "license": "MIT",
                    "integrity": "sha512-AAEC",
                    "resolved": "https://registry.npmjs.org/react/-/react-1.0.0.tgz"
                },
                "node_modules/@types/react": { "version": "2.0.0", "license": "MIT", "dev": true },
                "node_modules/tool/node_modules/inner": { "version": "3.0.0", "license": "ISC", "dev": true, "optional": true }
            }
        })
    }

    #[test]
    fn web_sbom_classifies_dev_dependencies() {
        let doc = web_sbom(&lock()).unwrap();
        let comps = components(&doc).unwrap();
        let find = |n: &str| comps.iter().find(|c| c.name == n).unwrap();
        assert_eq!(find("web").shipped_property, Some(true));
        assert_eq!(find("react").shipped_property, Some(true));
        assert_eq!(find("@types/react").shipped_property, Some(false));
        assert_eq!(find("inner").version, "3.0.0");
        assert_eq!(find("react").ecosystem, "npm");
        let react = &doc["components"][1];
        assert_eq!(react["name"], "react");
        assert_eq!(react["scope"], "required");
        assert_eq!(react["hashes"][0]["content"], "000102");
        assert_eq!(doc["components"][0]["purl"], "pkg:npm/%40types/react@2.0.0");
        assert_eq!(doc["components"][0]["scope"], "excluded");
        // No clock and no random serial number: the same lockfile, the same bytes.
        assert_eq!(doc, web_sbom(&lock()).unwrap());
        assert!(doc.get("serialNumber").is_none());
        assert!(assert_docs(&[("t".into(), doc)], None).unwrap());
    }

    #[test]
    fn a_shipped_copyleft_package_fails_the_assertion() {
        let mut l = lock();
        l["packages"]["node_modules/react"]["license"] = json!("GPL-3.0-only");
        let doc = web_sbom(&l).unwrap();
        assert!(!assert_docs(&[("t".into(), doc)], None).unwrap());
    }

    #[test]
    fn cargo_components_are_classified_by_the_graph() {
        let normal = "pfp-app v0.0.1 (/somewhere)\nlib v1.0.0\nderive v2.0.0 (proc-macro)\n";
        let with_build = format!("{normal}bld v1.0.0\n");
        let g = graph_from_tree(normal, &with_build).unwrap();
        assert_eq!(g[&("lib".into(), "1.0.0".into())], Shipped::Linked);
        assert_eq!(g[&("derive".into(), "2.0.0".into())], Shipped::Linked);
        assert_eq!(g[&("bld".into(), "1.0.0".into())], Shipped::BuildTime);
        assert!(graph_from_tree("x", "x").is_err());
        assert!(graph_from_tree("a v1\n", "b v1\n").is_err());
        assert!(!g.contains_key(&("dev".into(), "1.0.0".into())));

        // A cargo-cyclonedx-shaped document: the dev crate carries a licence
        // outside the allowlist but not in a forbidden family, and does not ship.
        let doc = json!({
            "bomFormat": "CycloneDX",
            "metadata": { "component": {
                "name": "pfp-app", "version": "0.0.1", "purl": "pkg:cargo/pfp-app@0.0.1",
                "licenses": [{ "expression": "Apache-2.0" }],
                "components": [{ "name": "pfp", "purl": "pkg:cargo/pfp-app@0.0.1#src/main.rs" }]
            }},
            "components": [
                { "name": "lib", "version": "1.0.0", "purl": "pkg:cargo/lib@1.0.0", "licenses": [{ "expression": "MIT/Apache-2.0" }] },
                { "name": "dev", "version": "1.0.0", "purl": "pkg:cargo/dev@1.0.0", "licenses": [{ "expression": "Unlicense" }] }
            ]
        });
        assert!(assert_docs(&[("t".into(), doc.clone())], Some(&g)).unwrap());
        // Without the graph every Rust component is held to the shipped standard.
        assert!(!assert_docs(&[("t".into(), doc)], None).unwrap());
    }

    #[test]
    fn base64_decodes() {
        assert_eq!(base64_to_hex("AAEC").as_deref(), Some("000102"));
        assert_eq!(base64_to_hex("/w==").as_deref(), Some("ff"));
        assert!(base64_to_hex("!!").is_none());
    }
}
