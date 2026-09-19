//! The leaf fingerprint printed in the status console and compared by the user in
//! decline mode (`SECURITY.md` §6.2 point 3).

use std::fmt::Write as _;

use rustls::pki_types::CertificateDer;
use sha2::{Digest, Sha256};

/// SHA-256 of the certificate's DER encoding, as colon-separated upper-case hex
/// (`AA:BB:…`, 95 characters) — the form browsers show in their certificate viewers.
///
/// The colons are deliberate beyond readability: the value can never be mistaken
/// for a 64-character hex token by a log scanner.
#[must_use]
pub fn leaf_fingerprint(leaf: &CertificateDer<'_>) -> String {
    let digest = Sha256::digest(leaf.as_ref());
    let mut text = String::with_capacity(digest.len() * 3);
    for (index, byte) in digest.iter().enumerate() {
        if index > 0 {
            text.push(':');
        }
        // Writing to a `String` cannot fail.
        let _ = write!(text, "{byte:02X}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::leaf_fingerprint;
    use rustls::pki_types::CertificateDer;

    #[test]
    fn known_answer_for_the_empty_input() {
        // SHA-256 of the empty string (FIPS 180-4 test vector).
        let empty = CertificateDer::from(Vec::new());
        assert_eq!(
            leaf_fingerprint(&empty),
            "E3:B0:C4:42:98:FC:1C:14:9A:FB:F4:C8:99:6F:B9:24:\
             27:AE:41:E4:64:9B:93:4C:A4:95:99:1B:78:52:B8:55"
        );
    }

    #[test]
    fn shape_is_32_upper_hex_pairs_joined_by_colons() {
        let text = leaf_fingerprint(&CertificateDer::from(vec![1, 2, 3]));
        assert_eq!(text.len(), 32 * 2 + 31);
        let pairs: Vec<&str> = text.split(':').collect();
        assert_eq!(pairs.len(), 32);
        assert!(pairs.iter().all(|pair| pair.len() == 2
            && pair
                .chars()
                .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c))));
    }

    #[test]
    fn different_certificates_have_different_fingerprints() {
        let a = leaf_fingerprint(&CertificateDer::from(vec![1]));
        let b = leaf_fingerprint(&CertificateDer::from(vec![2]));
        assert_ne!(a, b);
    }
}
