//! S-20: the panic hook prints where, never what. `harness = false`: with the
//! marker variable set this executable *is* the panicking child; without it, it is
//! the parent that re-executes **itself** (never `pfp`, never a server) and
//! inspects the child's output. Test profiles unwind, so the child exits 101 and
//! leaves no crash report.

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

const MARKER: &str = "PFP_TEST_PANIC_CHILD";
const CANARY: &str = "canary-7f3a-do-not-print";

fn main() {
    if std::env::var_os(MARKER).is_some() {
        pfp_app::harden::install_panic_hook();
        let value = String::from(CANARY);
        panic!("a payload formatted from a value: {value}");
    }

    let output = support::spawn_self(MARKER);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(101), "unwinding panic exit code");
    assert!(
        !stderr.contains(CANARY) && !stdout.contains(CANARY),
        "the payload never appears"
    );
    assert!(
        !stderr.contains("payload"),
        "no part of the message appears"
    );
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one line: {stderr}");
    let location = lines[0]
        .strip_prefix(pfp_app::harden::PANIC_MARKER)
        .expect("the marker")
        .trim();
    let (file, line) = location.rsplit_once(':').expect("file:line");
    assert!(file.ends_with("panic_hook.rs"), "{file}");
    assert!(line.parse::<u32>().is_ok(), "{line}");
    println!("test panic hook prints location only ... ok");
}
