//! The certificate scheme of `SECURITY.md` §6.1: a name-constrained local CA that
//! signs exactly one leaf and whose private key does not outlive [`issue`].
//!
//! ```text
//! CA    ECDSA P-256; self-signed
//!       basicConstraints CA:TRUE, pathLen 0                         critical
//!       nameConstraints  permitted dNSName "localhost",
//!                        iPAddress 127.0.0.1/32, iPAddress ::1/128  critical
//!       keyUsage         keyCertSign | cRLSign                      critical
//!       validity         now - 5 min .. now + 825 days
//! leaf  ECDSA P-256; signed by the CA key
//!       subjectAltName   dNSName "localhost", iPAddress 127.0.0.1, iPAddress ::1
//!       extendedKeyUsage serverAuth;  keyUsage digitalSignature     critical
//!       authorityKeyIdentifier;  validity now - 5 min .. now + 820 days
//! ```
//!
//! "Excluded: everything else" is expressed as permitted subtrees only: RFC 5280
//! makes any dNSName or iPAddress outside a permitted subtree invalid. Other name
//! forms are unconstrained, which is moot with `pathLen 0` and a discarded key.
//!
//! **Validity depends on a browser policy this project does not control.** 820/825
//! days exceeds the 398-day limit browsers enforce on publicly trusted chains; it is
//! accepted only because user-added roots are exempt (Apple caps those at 825 days,
//! which is where the number comes from). The three-browser trust spike must confirm
//! that the exemption still holds.
//!
//! **What "discarded" means.** The CA key is a local of [`issue`]. After the one
//! signature its serialized form is wiped (`zeroize`) and the key is dropped; it is
//! never returned, stored or serialized, and no type outside this module can name
//! it. Honest residual: `ring` keeps its own parsed copy of the scalar, which has no
//! wipe hook and is merely freed. The property that matters — no second certificate
//! can ever be issued under the trusted anchor — holds because the key is
//! unreachable and gone when the function returns.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints, SanType, SerialNumber, PKCS_ECDSA_P256_SHA256,
};
use rustls::pki_types::PrivatePkcs8KeyDer;
use sha2::{Digest, Sha256};
use time::{Duration, OffsetDateTime};
use zeroize::Zeroize;

use super::store::TlsMaterial;
use crate::redact::Redacted;

/// Both certificates are back-dated by this much, so a clock that is slightly
/// behind does not see a not-yet-valid certificate.
pub const BACKDATE: Duration = Duration::minutes(5);
/// CA validity (`SECURITY.md` §6.1).
pub const CA_VALIDITY: Duration = Duration::days(825);
/// Leaf validity (`SECURITY.md` §6.1); shorter than the CA so the leaf never outlives it.
pub const LEAF_VALIDITY: Duration = Duration::days(820);

/// The only DNS name in the scheme.
pub const DNS_NAME: &str = "localhost";

const CA_COMMON_NAME_PREFIX: &str = "Personal Financial Modeling local CA";
const LEAF_COMMON_NAME: &str = "Personal Financial Modeling (this computer)";

/// Issuance failed. Carries the generator's error, which never contains key bytes.
#[derive(Debug)]
pub struct IssueError(rcgen::Error);

impl fmt::Display for IssueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot issue the local TLS certificates: {}", self.0)
    }
}

impl std::error::Error for IssueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Generates a fresh CA and leaf, valid from `now` (wall clock), and discards the
/// CA private key before returning.
///
/// # Errors
/// [`IssueError`] when key generation or signing fails.
pub fn issue(now: OffsetDateTime) -> Result<TlsMaterial, IssueError> {
    let mut ca_key = generate_key()?;
    issue_and_dispose(&mut ca_key, now)
    // `ca_key` — already wiped — is dropped here. Nothing else ever held it.
}

/// Signs with `ca_key`, then wipes it — on success **and** on failure.
fn issue_and_dispose(ca_key: &mut KeyPair, now: OffsetDateTime) -> Result<TlsMaterial, IssueError> {
    let result = sign_ca_and_leaf(ca_key, now, loopback_sans());
    ca_key.zeroize();
    result
}

fn sign_ca_and_leaf(
    ca_key: &KeyPair,
    now: OffsetDateTime,
    leaf_sans: Vec<SanType>,
) -> Result<TlsMaterial, IssueError> {
    // X.509 times have one-second resolution.
    let now = now.replace_nanosecond(0).unwrap_or(now);

    let ca_params = ca_params(now, ca_key);
    let ca_cert = ca_params.self_signed(ca_key).map_err(IssueError)?;

    let mut leaf_key = generate_key()?;
    let leaf_params = leaf_params(now, &leaf_key, leaf_sans);
    let issuer = Issuer::from_params(&ca_params, ca_key);
    let leaf_cert = leaf_params.signed_by(&leaf_key, &issuer);

    // The PKCS#8 bytes move into the returned material; the generator's own copy is wiped.
    let leaf_key_der = PrivatePkcs8KeyDer::from(leaf_key.serialize_der());
    leaf_key.zeroize();

    Ok(TlsMaterial {
        ca_cert_der: ca_cert.der().clone(),
        leaf_cert_der: leaf_cert.map_err(IssueError)?.der().clone(),
        leaf_key: Redacted::new(leaf_key_der),
    })
}

fn generate_key() -> Result<KeyPair, IssueError> {
    // ECDSA P-256 / SHA-256 from the `ring` CSPRNG.
    KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(IssueError)
}

/// A positive 16-byte serial derived from the public key: unique per key, and no
/// second entropy source is needed.
fn serial_for(key: &KeyPair) -> [u8; 16] {
    let digest = Sha256::digest(key.public_key_raw());
    let mut serial = [0_u8; 16];
    serial.copy_from_slice(&digest[..16]);
    // Clear the sign bit and keep the leading byte non-zero: a positive INTEGER of
    // fixed length.
    serial[0] = (serial[0] & 0x7f) | 0x40;
    serial
}

fn ca_params(now: OffsetDateTime, ca_key: &KeyPair) -> CertificateParams {
    let serial = serial_for(ca_key);
    let tag = u32::from_be_bytes([serial[0], serial[1], serial[2], serial[3]]);

    let mut params = CertificateParams::default();
    params.not_before = now - BACKDATE;
    params.not_after = now + CA_VALIDITY;
    params.serial_number = Some(SerialNumber::from_slice(&serial));
    params.distinguished_name = common_name(&format!("{CA_COMMON_NAME_PREFIX} {tag:08x}"));
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![
            GeneralSubtree::DnsName(DNS_NAME.to_owned()),
            GeneralSubtree::IpAddress(CidrSubnet::V4(Ipv4Addr::LOCALHOST.octets(), [0xff; 4])),
            GeneralSubtree::IpAddress(CidrSubnet::V6(Ipv6Addr::LOCALHOST.octets(), [0xff; 16])),
        ],
        excluded_subtrees: Vec::new(),
    });
    params
}

fn leaf_params(now: OffsetDateTime, leaf_key: &KeyPair, sans: Vec<SanType>) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.not_before = now - BACKDATE;
    params.not_after = now + LEAF_VALIDITY;
    params.serial_number = Some(SerialNumber::from_slice(&serial_for(leaf_key)));
    params.distinguished_name = common_name(LEAF_COMMON_NAME);
    params.is_ca = IsCa::NoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.use_authority_key_identifier_extension = true;
    params.subject_alt_names = sans;
    params
}

fn loopback_sans() -> Vec<SanType> {
    let mut sans = Vec::with_capacity(3);
    // "localhost" is ASCII, so the IA5String conversion cannot fail; if it ever
    // did, the leaf would lack the name and every verification test would fail.
    if let Ok(name) = DNS_NAME.try_into() {
        sans.push(SanType::DnsName(name));
    }
    sans.push(SanType::IpAddress(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    sans.push(SanType::IpAddress(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    sans
}

fn common_name(value: &str) -> DistinguishedName {
    let mut name = DistinguishedName::new();
    name.push(DnType::CommonName, value);
    name
}

#[cfg(test)]
mod tests {
    use super::{generate_key, issue_and_dispose, sign_ca_and_leaf};
    use crate::tls::store::{MaterialError, TlsMaterial};
    use rcgen::SanType;
    use rustls::client::danger::ServerCertVerifier;
    use rustls::client::WebPkiServerVerifier;
    use rustls::pki_types::{ServerName, UnixTime};
    use rustls::RootCertStore;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;
    use time::OffsetDateTime;

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn ca_key_is_wiped_and_appears_nowhere_in_the_output() {
        let mut ca_key = generate_key().unwrap();
        let ca_key_der = ca_key.serialize_der();
        let public_point = ca_key.public_key_raw().to_vec();

        // `ring` writes P-256 PKCS#8 as a fixed 36-byte prefix, the 32-byte private
        // scalar, then a wrapper around the public point. Confirm the layout rather
        // than assume it, so the scalar slice below really is the scalar.
        let scalar = ca_key_der[36..68].to_vec();
        let point_at = ca_key_der
            .windows(public_point.len())
            .position(|w| w == public_point)
            .unwrap();
        assert!(point_at >= 68);
        assert!(!contains(&public_point, &scalar));

        let issued = issue_and_dispose(&mut ca_key, OffsetDateTime::now_utc()).unwrap();

        // The generator's serialized copy is wiped, not merely dropped later.
        assert!(ca_key.serialized_der().iter().all(|b| *b == 0));
        assert!(ca_key.serialized_der().is_empty());

        // Exhaustive destructuring: a new field in `TlsMaterial` fails to compile
        // here, which forces whoever adds it to look at this test.
        let TlsMaterial {
            ca_cert_der,
            leaf_cert_der,
            leaf_key,
        } = issued;
        for output in [
            ca_cert_der.as_ref(),
            leaf_cert_der.as_ref(),
            leaf_key.expose().secret_pkcs8_der(),
        ] {
            assert!(!contains(output, &scalar));
            assert!(!contains(output, &ca_key_der));
        }
        // The leaf key is a different key from the CA key.
        assert_ne!(leaf_key.expose().secret_pkcs8_der(), ca_key_der.as_slice());
        // Sanity: the CA's *public* point is in the CA certificate, so the search works.
        assert!(contains(ca_cert_der.as_ref(), &public_point));
    }

    /// What a TLS client does: verify `material`'s leaf under its CA for `name`.
    fn verify_for(material: &TlsMaterial, name: &str, now: OffsetDateTime) -> Result<(), String> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut roots = RootCertStore::empty();
        roots.add(material.ca_cert_der.clone()).unwrap();
        let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(roots), provider)
            .build()
            .unwrap();
        let at = UnixTime::since_unix_epoch(std::time::Duration::from_secs(
            u64::try_from(now.unix_timestamp()).unwrap(),
        ));
        verifier
            .verify_server_cert(
                &material.leaf_cert_der,
                &[],
                &ServerName::try_from(name.to_owned()).unwrap(),
                &[],
                at,
            )
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    #[test]
    fn out_of_constraint_leaf_from_the_same_ca_params_is_rejected() {
        // A test-local CA with the production CA parameters signs leaves the design
        // forbids. Each is verified for a name it really carries, so the refusal is
        // the CA's name constraints and not a mere name mismatch.
        let now = OffsetDateTime::now_utc();
        let v4 = |a, b, c, d| SanType::IpAddress(IpAddr::V4(Ipv4Addr::new(a, b, c, d)));
        let dns = |name: &str| SanType::DnsName(name.try_into().unwrap());
        let cases: Vec<(Vec<SanType>, &str)> = vec![
            (vec![dns("evil.example")], "evil.example"),
            (vec![v4(10, 0, 0, 1)], "10.0.0.1"),
            (vec![v4(127, 0, 0, 2)], "127.0.0.2"),
            (vec![dns("localhost.example")], "localhost.example"),
            // One permitted and one forbidden name: invalid even for the permitted one.
            (vec![v4(127, 0, 0, 1), dns("evil.example")], "127.0.0.1"),
        ];
        for (sans, name) in cases {
            let ca_key = generate_key().unwrap();
            let material = sign_ca_and_leaf(&ca_key, now, sans).unwrap();
            let error = verify_for(&material, name, now).unwrap_err();
            assert!(error.contains("NameConstraintViolation"), "{name}: {error}");
            assert_eq!(material.validate(now), Err(MaterialError::ChainInvalid));
        }
    }

    #[test]
    fn the_same_helper_accepts_the_production_names() {
        // Guards the test above against passing for the wrong reason.
        let now = OffsetDateTime::now_utc();
        let material = super::issue(now).unwrap();
        for name in ["127.0.0.1", "localhost", "::1"] {
            assert_eq!(verify_for(&material, name, now), Ok(()));
        }
    }

    #[test]
    fn in_constraint_material_validates() {
        let mut ca_key = generate_key().unwrap();
        let now = OffsetDateTime::now_utc();
        let material = issue_and_dispose(&mut ca_key, now).unwrap();
        assert_eq!(material.validate(now), Ok(()));
    }

    #[test]
    fn each_issuance_has_fresh_keys_and_serials() {
        let now = OffsetDateTime::now_utc();
        let a = super::issue(now).unwrap();
        let b = super::issue(now).unwrap();
        assert_ne!(a.ca_cert_der, b.ca_cert_der);
        assert_ne!(a.leaf_cert_der, b.leaf_cert_der);
        assert_ne!(
            a.leaf_key.expose().secret_pkcs8_der(),
            b.leaf_key.expose().secret_pkcs8_der()
        );
    }
}
