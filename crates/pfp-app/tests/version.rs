//! The `pfp` binary reports its version and exits successfully.

// Not an engine crate: spawning the built binary is the point of this test.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use std::process::Command;

#[test]
fn prints_its_version_and_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_pfp"))
        .output()
        .expect("the pfp binary runs");
    assert!(output.status.success(), "exit status {:?}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("version output is UTF-8");
    assert_eq!(stdout, format!("pfp {}\n", env!("CARGO_PKG_VERSION")));
    assert!(output.stderr.is_empty());
}
