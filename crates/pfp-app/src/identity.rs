//! The application's identity: the one place its identifier and names are written.
//!
//! Everything that names the application derives from these constants: the
//! default state directory ([`crate::state::default_under`]), and — through
//! `cargo xtask dist`, which compiles this very file in with `#[path]` — the
//! `.app` wrapper's `Info.plist` and the code-signing identifier. Nothing else in
//! the repository may spell the identifier out; the tests of `pfp-app` and
//! `xtask` read it from here.
//!
//! This file must stay dependency-free and hold nothing but constants and pure
//! functions, because `xtask` includes it verbatim.

/// The application identifier (reverse-DNS, as `CFBundleIdentifier` and the
/// code-signing identifier require).
///
/// **PLACEHOLDER — replace before the first signed release.** It is clearly not
/// a domain anyone controls on the project's behalf. The first Developer ID
/// signature binds the identifier into the code-signing requirement, and every
/// install thereafter keys its state directory on it, so it must be the real one
/// before anything is signed with a real identity. Until then only unsigned or
/// ad-hoc signed builds exist, and nothing has been installed anywhere, so
/// changing it costs nothing but this line (and the state directory it names).
pub const APP_IDENTIFIER: &str = "io.github.personal-financial-modeling.pfp";

/// The executable's file name, in both release channels.
pub const EXECUTABLE_NAME: &str = "pfp";

/// The `.app` wrapper's name, without the `.app` suffix (`CFBundleName`).
pub const BUNDLE_NAME: &str = "pfp";

/// Whether `id` is a well-formed reverse-DNS bundle identifier: at least two
/// dot-separated labels, each non-empty and made only of ASCII letters, digits
/// and `-` (the character set `CFBundleIdentifier` allows).
#[must_use]
pub const fn is_reverse_dns(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut labels = 1;
    let mut label_len = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' {
            if label_len == 0 {
                return false;
            }
            labels += 1;
            label_len = 0;
        } else if b.is_ascii_alphanumeric() || b == b'-' {
            label_len += 1;
        } else {
            return false;
        }
        i += 1;
    }
    label_len > 0 && labels >= 2
}

// A malformed identifier is a compile error, in `pfp-app` and in `xtask` alike.
const _: () = assert!(is_reverse_dns(APP_IDENTIFIER));
