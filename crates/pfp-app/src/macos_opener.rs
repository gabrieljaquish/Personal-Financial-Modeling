//! The macOS browser opener: `LSOpenCFURLRef`, called from inside this process.
//!
//! The launch URL carries the launch token in its fragment, so it must reach the
//! browser without ever appearing in argv, the environment, a file or the
//! clipboard (`SECURITY.md` §7.1). Launch Services hands the URL to the default
//! browser directly; no `open(1)` process is spawned and no other process can
//! read the token from a process listing.
//!
//! This module is compiled only into the `pfp` binary on macOS. No test reaches
//! it: every test that starts the binary passes `--no-open`
//! (`tests/support/mod.rs`, `SERVE_FLAGS`), and the library's own tests use
//! `Platform::unsupported()` or in-memory fakes. Trust installation and native
//! alerts stay unsupported; only the opener is real.

// The one place in `pfp-app` that calls into the operating system without a safe
// wrapper crate: three C functions, each call justified below.
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

use pfp_app::platform::{BrowserOpener, Platform, PlatformError};
use pfp_server::Redacted;

type CfTypeRef = *const c_void;
type CfUrlRef = *const c_void;
type CfAllocatorRef = *const c_void;
type CfIndex = isize;
type CfStringEncoding = u32;
type OsStatus = i32;

/// `kCFStringEncodingUTF8`.
const CF_STRING_ENCODING_UTF8: CfStringEncoding = 0x0800_0100;

/// The only URLs this opener will hand to the browser: the application's own
/// loopback origin. Anything else is refused before any system call.
const LOOPBACK_PREFIX: &str = "https://127.0.0.1:";

/// `paramErr`, reported when the URL is refused or cannot be converted.
const PARAM_ERR: OsStatus = -50;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFURLCreateWithBytes(
        allocator: CfAllocatorRef,
        bytes: *const u8,
        length: CfIndex,
        encoding: CfStringEncoding,
        base_url: CfUrlRef,
    ) -> CfUrlRef;
    fn CFRelease(cf: CfTypeRef);
}

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSOpenCFURLRef(in_url: CfUrlRef, out_launched_url: *mut CfUrlRef) -> OsStatus;
}

/// Opens the launch URL in the user's default browser through Launch Services.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LaunchServicesOpener;

impl BrowserOpener for LaunchServicesOpener {
    fn available(&self) -> bool {
        true
    }

    fn open(&self, url: &Redacted<String>) -> Result<(), PlatformError> {
        let url = url.expose();
        if !url.starts_with(LOOPBACK_PREFIX) {
            return Err(PlatformError::Os(PARAM_ERR));
        }
        let length = CfIndex::try_from(url.len()).map_err(|_| PlatformError::Os(PARAM_ERR))?;

        // SAFETY: `url` is a live `&String`, so the pointer is valid for `length`
        // bytes of UTF-8 for the duration of the call; a null allocator selects
        // the default allocator and a null base URL makes the URL absolute.
        // CoreFoundation copies the bytes and returns an owned object or null.
        let cf_url = unsafe {
            CFURLCreateWithBytes(
                ptr::null(),
                url.as_ptr(),
                length,
                CF_STRING_ENCODING_UTF8,
                ptr::null(),
            )
        };
        if cf_url.is_null() {
            return Err(PlatformError::Os(PARAM_ERR));
        }

        // SAFETY: `cf_url` is the non-null CFURL created above and still owned by
        // this function; a null `out_launched_url` is documented as "not wanted".
        let status = unsafe { LSOpenCFURLRef(cf_url, ptr::null_mut()) };

        // SAFETY: `cf_url` came from a Create function, so this function owns
        // exactly one reference; it is released once and never used again.
        unsafe { CFRelease(cf_url) };

        if status == 0 {
            Ok(())
        } else {
            Err(PlatformError::Os(status))
        }
    }
}

/// The platform of the macOS binary: a real browser opener; trust settings and
/// native alerts remain unsupported until the trust spike lands.
pub(crate) fn platform() -> Platform {
    let mut platform = Platform::unsupported();
    platform.opener = Arc::new(LaunchServicesOpener);
    platform
}

#[cfg(test)]
mod tests {
    use super::{BrowserOpener, LaunchServicesOpener, PlatformError, Redacted, PARAM_ERR};

    // Only the refusal path is tested: it returns before any system call, so no
    // browser can open. The success path is exercised by a person running `pfp`.
    #[test]
    fn a_url_outside_the_loopback_origin_is_refused_before_any_system_call() {
        for url in [
            "http://127.0.0.1:47443/#t=x",
            "https://localhost:47443/#t=x",
            "https://example.com/#t=x",
            "file:///etc/passwd",
            "",
        ] {
            assert_eq!(
                LaunchServicesOpener.open(&Redacted::new(url.to_owned())),
                Err(PlatformError::Os(PARAM_ERR)),
                "{url}"
            );
        }
    }
}
