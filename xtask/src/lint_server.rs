//! `lint-server`: the workspace-wide source rules of the server half of M0. It
//! runs standalone (`cargo xtask lint-server`) and as part of `data-hygiene`, which
//! CI and the pre-commit hook already run, so neither rule depends on anybody
//! remembering a new command.
//!
//! **Rule A — capability bans (`SECURITY.md` §11).** In the shipped source
//! (`crates/<name>/src/**`, outside `#[cfg(test)]` modules) of every workspace
//! crate:
//!
//! * file-system access (`std::fs`, `tokio::fs`, `std::os::unix::fs`) only in
//!   [`FS_CRATES`];
//! * sockets (`std::net`, `tokio::net`, `std::os::unix::net`) only in
//!   [`NET_CRATES`].
//!
//! The rule is keyed on the crate's directory name, so a crate added tomorrow is
//! covered with no edit here, and acquires neither capability unless it is added to
//! a list in this file — which is a reviewed change. `xtask` is outside `crates/`
//! and is a build tool, not shipped code. Tests, benches, examples and build
//! scripts are not shipped code either. The HTTP-client half of §11 is
//! `deny.toml`'s. `pfp-server/tests/source_rules.rs` stays as the crate-local fast
//! path and additionally holds the single-TLS-acceptor rule.
//!
//! **Rule B — no credential on the command line or in the environment
//! (`SECURITY.md` §3.7, §7.1, §13.4).** In *every* `.rs` file under
//! `crates/pfp-app/`, tests included ("in any build profile"):
//!
//! * no string literal that is a credential-shaped flag: `--<name>` where the name
//!   contains one of [`CREDENTIAL_WORDS`] — except the spellings in
//!   [`REFUSAL_TEST_ALLOWLIST`], inside the test module of `cli.rs`, where the
//!   parser's refusal of each is asserted;
//! * no environment read (`env::var`, `env::var_os`, `env::vars`, `env::vars_os`)
//!   other than the exact `(file, argument)` pairs in [`ENV_READ_ALLOWLIST`];
//! * no environment write (`env::set_var`, or `.env(…)` on a child process) whose
//!   variable is credential-shaped.
//!
//! Comments are skipped; string literals are what rule B is about, so they are not.

use std::path::Path;

use crate::repo;

/// Crates (directory names under `crates/`) that may touch the file system.
const FS_CRATES: [&str; 2] = ["pfp-vault", "pfp-app"];

/// Crates that may open sockets.
const NET_CRATES: [&str; 1] = ["pfp-server"];

const FS_PATHS: [&str; 3] = ["std::fs", "tokio::fs", "std::os::unix::fs"];
const NET_PATHS: [&str; 3] = ["std::net", "tokio::net", "std::os::unix::net"];

/// The crate rule B covers.
const CLI_CRATE: &str = "crates/pfp-app";

/// A flag or variable whose lower-cased name contains one of these carries, or
/// invites, a credential.
const CREDENTIAL_WORDS: [&str; 6] = ["pass", "secret", "token", "key", "credential", "pwd"];

/// Every permitted environment read in `pfp-app`: the file and the exact argument
/// text. Each entry says why it is not a credential.
const ENV_READ_ALLOWLIST: [(&str, &str); 2] = [
    // Locates the default state directory. A path, never a secret.
    ("crates/pfp-app/src/main.rs", "\"HOME\""),
    // A test re-executes itself and tells the child which role to play.
    ("crates/pfp-app/tests/panic_hook.rs", "MARKER"),
];

/// Credential-shaped flags that may appear as literals, and only inside the test
/// module of the named file: the parser's own refusal test, which proves that each
/// of them is rejected. A flag listed here that the parser accepted would fail that
/// test, so the list cannot be used to admit one.
const REFUSAL_TEST_ALLOWLIST: (&str, [&str; 6]) = (
    "crates/pfp-app/src/cli.rs",
    [
        "token",
        "launch-token",
        "passphrase",
        "password",
        "secret",
        "key",
    ],
);

const ENV_READS: [&str; 4] = ["env::var(", "env::var_os(", "env::vars(", "env::vars_os("];
const ENV_WRITES: [&str; 2] = ["env::set_var(", ".env("];

pub(crate) fn run() -> Result<bool, String> {
    let violations = violations(&repo::root())?;
    for v in &violations {
        eprintln!("lint-server: {v}");
    }
    if violations.is_empty() {
        eprintln!("lint-server: clean");
        Ok(true)
    } else {
        eprintln!("lint-server: {} violation(s)", violations.len());
        Ok(false)
    }
}

/// Both rules over the checkout at `root`, as `path:line: message` strings.
pub(crate) fn violations(root: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let crates = root.join("crates");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&crates).map_err(|e| format!("{}: {e}", crates.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.path().is_dir() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    for name in &names {
        let src = crates.join(name).join("src");
        if !src.is_dir() {
            continue;
        }
        for path in repo::walk(&src)? {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = String::from_utf8_lossy(&repo::read(&path)?).into_owned();
            let rel = repo::relative(root, &path);
            for (line, message) in capability_violations(name, &text) {
                out.push(format!("{rel}:{line}: {message}"));
            }
        }
    }
    let cli = root.join(CLI_CRATE);
    if cli.is_dir() {
        for path in repo::walk(&cli)? {
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = String::from_utf8_lossy(&repo::read(&path)?).into_owned();
            let rel = repo::relative(root, &path);
            for (line, message) in credential_violations(&rel, &text) {
                out.push(format!("{rel}:{line}: {message}"));
            }
        }
    }
    Ok(out)
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// The shipped part of a source file: everything before the first column-zero
/// `#[cfg(test)]`, which by this repository's convention opens the trailing test
/// module.
fn shipped_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .take_while(|(_, line)| !line.starts_with("#[cfg(test)]"))
        .map(|(i, line)| (i + 1, line))
        .filter(|(_, line)| !is_comment(line))
}

/// Whether `line` names the module `path` (for example `std::fs`), directly or
/// through a grouped import such as `use std::{fs, io};`.
fn names_module(line: &str, path: &str) -> bool {
    let mut from = 0;
    while let Some(at) = line[from..].find(path) {
        let end = from + at + path.len();
        // `std::fs` but not `std::fs_extra` or `std::network`.
        let boundary = line[end..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if boundary {
            return true;
        }
        from = end;
    }
    let Some((root, leaf)) = path.rsplit_once("::") else {
        return false;
    };
    let group = format!("{root}::{{");
    line.find(&group).is_some_and(|at| {
        line[at + group.len()..]
            .split([',', '}'])
            .map(str::trim)
            .any(|item| item == leaf || item.starts_with(&format!("{leaf}::")))
    })
}

/// Rule A for one shipped source file of the crate in `crates/<crate_dir>`.
pub(crate) fn capability_violations(crate_dir: &str, text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let bans = [
        (
            &FS_PATHS,
            FS_CRATES.contains(&crate_dir),
            "file-system access",
            "pfp-vault or pfp-app",
        ),
        (
            &NET_PATHS,
            NET_CRATES.contains(&crate_dir),
            "socket access",
            "pfp-server",
        ),
    ];
    for (number, line) in shipped_lines(text) {
        for (paths, allowed, what, where_) in &bans {
            if *allowed {
                continue;
            }
            if let Some(path) = paths.iter().find(|p| names_module(line, p)) {
                out.push((
                    number,
                    format!("`{path}`: {what} belongs only in {where_} (SECURITY.md §11 capability bans)"),
                ));
            }
        }
    }
    out
}

fn is_credential_shaped(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    CREDENTIAL_WORDS.iter().any(|word| lower.contains(word))
}

/// The text between the `(` that ends `call` and the next `)` or `,`.
fn first_argument<'a>(line: &'a str, call: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find(call) {
        let start = from + at + call.len();
        let rest = &line[start..];
        let end = rest.find([')', ',']).unwrap_or(rest.len());
        out.push(rest[..end].trim());
        from = start;
    }
    out
}

/// Rule B for one file of `pfp-app`, tests included.
pub(crate) fn credential_violations(rel: &str, text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_test_module = false;
    for (index, line) in text.lines().enumerate() {
        in_test_module |= line.starts_with("#[cfg(test)]");
        if is_comment(line) {
            continue;
        }
        let number = index + 1;
        // Every `"--name` string literal on the line.
        for (at, _) in line.match_indices("\"--") {
            let name: String = line[at + 3..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            let refusal_test = in_test_module
                && rel == REFUSAL_TEST_ALLOWLIST.0
                && REFUSAL_TEST_ALLOWLIST.1.contains(&name.as_str());
            if is_credential_shaped(&name) && !refusal_test {
                out.push((
                    number,
                    format!("`--{name}` is a credential-shaped flag: a passphrase or token never travels in argv (SECURITY.md §3.7, §7.1)"),
                ));
            }
        }
        for call in ENV_READS {
            for argument in first_argument(line, call) {
                if !ENV_READ_ALLOWLIST.contains(&(rel, argument)) {
                    out.push((
                        number,
                        format!("`{call}{argument})` is not an allowlisted environment read: nothing secret is read from the environment, and every read is named in xtask/src/lint_server.rs (SECURITY.md §3.7)"),
                    ));
                }
            }
        }
        for call in ENV_WRITES {
            for argument in first_argument(line, call) {
                if is_credential_shaped(argument) {
                    out.push((
                        number,
                        format!("`{call}{argument}…)` puts a credential-shaped variable into an environment block (SECURITY.md §7.1)"),
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_working_tree_is_clean() {
        assert_eq!(violations(&repo::root()).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn a_new_crate_gets_neither_capability() {
        for line in [
            "use std::fs;",
            "use std::{fs, io};",
            "use std::{io, fs::File};",
            "let f = std::fs::read(path)?;",
            "use tokio::fs::File;",
            "use std::os::unix::fs::PermissionsExt;",
        ] {
            assert_eq!(capability_violations("pfp-new", line).len(), 1, "{line}");
            assert_eq!(capability_violations("pfp-server", line).len(), 1, "{line}");
            assert!(capability_violations("pfp-app", line).is_empty(), "{line}");
            assert!(
                capability_violations("pfp-vault", line).is_empty(),
                "{line}"
            );
        }
        for line in [
            "use std::net::TcpListener;",
            "use std::{io, net::SocketAddr};",
            "use tokio::net::TcpStream;",
            "use std::os::unix::net::UnixStream;",
        ] {
            assert_eq!(capability_violations("pfp-new", line).len(), 1, "{line}");
            assert_eq!(capability_violations("pfp-app", line).len(), 1, "{line}");
            assert_eq!(capability_violations("pfp-tax", line).len(), 1, "{line}");
            assert!(
                capability_violations("pfp-server", line).is_empty(),
                "{line}"
            );
        }
    }

    #[test]
    fn comments_test_modules_and_lookalikes_are_not_capabilities() {
        let text = "// use std::fs;\n/// std::net in a doc comment\nuse std::fs_extra;\nuse std::network;\nuse std::{fmt, io};\n#[cfg(test)]\nmod tests {\n    use std::net::TcpListener;\n    use std::fs;\n}\n";
        assert!(capability_violations("pfp-new", text).is_empty());
    }

    #[test]
    fn the_violation_names_the_line() {
        let text = "use std::io;\n\nuse std::net::TcpStream;\n";
        let found = capability_violations("pfp-app", text);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, 3);
        assert!(found[0].1.contains("std::net"));
    }

    #[test]
    fn credential_shaped_flags_are_refused_anywhere_in_the_cli_crate() {
        for flag in [
            "--passphrase",
            "--pass",
            "--password",
            "--secret",
            "--secret-file",
            "--key",
            "--launch-token",
            "--TOKEN",
            "--api_key",
        ] {
            let line = format!("            \"{flag}\" => out.x = value(&mut rest)?,");
            let found = credential_violations("crates/pfp-app/src/cli.rs", &line);
            assert_eq!(found.len(), 1, "{flag}");
            // Tests are not exempt: "in any build profile".
            let found = credential_violations("crates/pfp-app/tests/serve.rs", &line);
            assert_eq!(found.len(), 1, "{flag}");
        }
        for flag in [
            "--port",
            "--no-open",
            "--no-trust",
            "--install-trust",
            "--state-dir",
            "--version",
            "--help",
        ] {
            let line = format!("\"{flag}\" => {{}}");
            assert!(
                credential_violations("crates/pfp-app/src/cli.rs", &line).is_empty(),
                "{flag}"
            );
        }
    }

    #[test]
    fn the_refusal_test_may_name_the_flags_it_proves_are_refused() {
        let test_module = "#[cfg(test)]\nmod tests {\n    &[\"--passphrase\", \"x\"],\n}\n";
        assert!(credential_violations("crates/pfp-app/src/cli.rs", test_module).is_empty());
        // Not in shipped code of that file, not in another file, not another spelling.
        let shipped = "    \"--passphrase\" => {}\n#[cfg(test)]\nmod tests {}\n";
        assert_eq!(
            credential_violations("crates/pfp-app/src/cli.rs", shipped).len(),
            1
        );
        assert_eq!(
            credential_violations("crates/pfp-app/src/launch.rs", test_module).len(),
            1
        );
        let other = "#[cfg(test)]\nmod tests {\n    &[\"--passphrase-file\", \"x\"],\n}\n";
        assert_eq!(
            credential_violations("crates/pfp-app/src/cli.rs", other).len(),
            1
        );
    }

    #[test]
    fn environment_reads_are_allowlisted_by_file_and_argument() {
        let home = "    let home = std::env::var_os(\"HOME\").map(PathBuf::from);";
        assert!(credential_violations("crates/pfp-app/src/main.rs", home).is_empty());
        // The same read anywhere else is not the allowlisted one.
        assert_eq!(
            credential_violations("crates/pfp-app/src/launch.rs", home).len(),
            1
        );
        for line in [
            "let p = std::env::var(\"PFP_PASSPHRASE\").ok();",
            "let p = env::var_os(\"PFP_LAUNCH_TOKEN\");",
            "let p = env::var(name)?;",
            "for (k, v) in std::env::vars() {}",
            "for (k, v) in std::env::vars_os() {}",
        ] {
            assert_eq!(
                credential_violations("crates/pfp-app/src/main.rs", line).len(),
                1,
                "{line}"
            );
        }
        // Not environment reads.
        for line in [
            "let dir = std::env::temp_dir();",
            "let args = std::env::args_os();",
            "let exe = std::env::current_exe()?;",
            "// std::env::var(\"PFP_PASSPHRASE\") in a comment",
        ] {
            assert!(
                credential_violations("crates/pfp-app/src/main.rs", line).is_empty(),
                "{line}"
            );
        }
    }

    #[test]
    fn a_credential_is_never_written_into_an_environment_block() {
        for line in [
            "std::env::set_var(\"PFP_TOKEN\", token);",
            "    .env(\"PFP_PASSPHRASE\", \"x\")",
            "    .env(LAUNCH_TOKEN_VAR, token)",
        ] {
            assert_eq!(
                credential_violations("crates/pfp-app/tests/support/mod.rs", line).len(),
                1,
                "{line}"
            );
        }
        assert!(credential_violations(
            "crates/pfp-app/tests/support/mod.rs",
            "    .env(marker, \"1\")"
        )
        .is_empty());
    }
}
