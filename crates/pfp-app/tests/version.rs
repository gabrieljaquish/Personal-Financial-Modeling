//! The commands of `pfp` that start nothing: `--version`, `--help`, `openapi`.

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

#[test]
fn prints_its_version_and_exits_zero() {
    let output = support::run_pfp_inert("--version");
    assert!(output.status.success(), "exit status {:?}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("version output is UTF-8");
    assert_eq!(stdout, format!("pfp {}\n", env!("CARGO_PKG_VERSION")));
    assert!(output.stderr.is_empty());
}

#[test]
fn help_names_every_flag_and_no_way_to_obtain_a_token() {
    let output = support::run_pfp_inert("--help");
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for flag in [
        "--port",
        "--no-open",
        "--no-trust",
        "--install-trust",
        "--state-dir",
        "trust remove",
        "openapi",
    ] {
        assert!(text.contains(flag), "{flag}");
    }
    assert!(!text.to_lowercase().contains("token"));
}

#[test]
fn openapi_prints_the_document_the_server_generates_and_starts_nothing() {
    let output = support::run_pfp_inert("openapi");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let printed = String::from_utf8(output.stdout).unwrap();
    assert_eq!(printed, pfp_server::openapi_json());
    let document: serde_json::Value = serde_json::from_str(&printed).unwrap();
    assert!(document["paths"]["/api/v1/tax/rate-schedule"]["post"].is_object());
}

#[test]
fn usage_errors_exit_2_without_echoing_the_argument() {
    // In-process: the same `run` the binary calls, so no process is started.
    let secret = "zz-not-a-flag-zz";
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = pfp_app::run(
        &[secret.to_owned()],
        &pfp_app::platform::Platform::unsupported(),
        None,
        &mut out,
        &mut err,
    );
    assert_eq!(code, 2);
    assert!(out.is_empty());
    let err = String::from_utf8(err).unwrap();
    assert!(err.contains("usage:") && !err.contains(secret));
}
