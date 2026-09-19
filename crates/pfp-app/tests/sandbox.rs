//! S-07 (`SECURITY.md` §7.5): the server starts and serves under a deny-outbound
//! sandbox profile — it needs no outbound connection for anything, including its
//! first launch (fresh state directory, certificate issuance).
//!
//! `sandbox-exec` constrains only the child it starts and changes no setting. It
//! goes through the single spawn helper: same constant flags, own process group,
//! group-wide teardown with its assertions. Skipped, with a printed reason, where
//! `sandbox-exec` does not exist or cannot apply a profile (for example when the
//! test run is itself sandboxed, which forbids nesting).

// Not an engine crate: spawning the built binary, sockets and temp files are the point.
#![forbid(unsafe_code)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

mod support;

#[test]
fn serves_under_a_deny_outbound_profile() {
    if !cfg!(target_os = "macos") {
        eprintln!("skipped: sandbox-exec is a macOS tool");
        return;
    }
    let Some(mut pfp) = support::spawn_pfp_sandboxed() else {
        eprintln!("skipped: /usr/bin/sandbox-exec is absent");
        return;
    };
    let ready = match pfp.wait_ready() {
        Ok(ready) => ready,
        Err(status) => {
            let stderr = pfp.stderr();
            assert!(
                stderr.contains("sandbox"),
                "pfp failed under the sandbox for a reason of its own ({status:?}): {stderr}"
            );
            eprintln!(
                "skipped: sandbox-exec could not apply a profile here: {}",
                stderr.trim()
            );
            return;
        }
    };
    assert!(ready.line.contains("trust=declined open=skipped"));

    let client = pfp.client();
    let document = client.navigate("/");
    assert_eq!(document.status, 200);
    let refused = client.api("/api/v1/session/status", None);
    assert_eq!(refused.status, 401);

    assert!(pfp.terminate().success(), "graceful exit under the sandbox");
}
