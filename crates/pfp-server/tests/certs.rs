//! The certificate scheme is what `SECURITY.md` §6.1 says it is. The generated
//! certificates are parsed with an independent X.509 parser, and verified the way a
//! TLS client verifies them (`rustls`/webpki).
//!
//! What this proves: the scheme verifies against webpki. It does not prove that
//! Safari, Chrome and Firefox enforce IP-address name constraints or accept the
//! 820/825-day validity under a user-added root; the browser trust spike does.

// Not an engine crate: tests here legitimately touch sockets, the clock and files.
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::disallowed_macros
)]

use pfp_server::tls::issue::{BACKDATE, CA_VALIDITY, LEAF_VALIDITY};
use pfp_server::{
    issue, leaf_fingerprint, server_config, LeafStore, MaterialError, MemoryLeafStore, Redacted,
    TlsMaterial,
};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use time::{Duration, OffsetDateTime};
use x509_parser::extensions::{GeneralName, ParsedExtension};
use x509_parser::prelude::{FromDer, X509Certificate};

const ECDSA_WITH_SHA256: &str = "1.2.840.10045.4.3.2";
const EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
const PRIME256V1: &str = "1.2.840.10045.3.1.7";

/// A fixed issuance instant with a sub-second part, to show it is truncated.
fn fixed_now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(1_789_819_200_500_000_000).unwrap()
}

fn parse<'a>(der: &'a CertificateDer<'_>) -> X509Certificate<'a> {
    let (rest, cert) = X509Certificate::from_der(der.as_ref()).expect("valid DER");
    assert!(rest.is_empty(), "no trailing bytes");
    cert
}

fn lower_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn common_name(name: &x509_parser::x509::X509Name<'_>) -> String {
    let mut values = name.iter_common_name();
    let value = values
        .next()
        .expect("a CN")
        .as_str()
        .expect("text")
        .to_owned();
    assert!(values.next().is_none(), "exactly one CN");
    assert_eq!(name.iter_attributes().count(), 1, "the DN is the CN only");
    value
}

#[test]
fn ca_is_constrained_and_critical() {
    let material = issue(fixed_now()).unwrap();
    let ca = parse(&material.ca_cert_der);

    assert_eq!(ca.version().0, 2, "X.509 v3");
    assert_eq!(ca.subject(), ca.issuer(), "self-signed");
    assert_eq!(
        ca.signature_algorithm.algorithm.to_id_string(),
        ECDSA_WITH_SHA256
    );

    let basic = ca.basic_constraints().unwrap().expect("basicConstraints");
    assert!(basic.critical);
    assert!(basic.value.ca);
    assert_eq!(basic.value.path_len_constraint, Some(0));

    let usage = ca.key_usage().unwrap().expect("keyUsage");
    assert!(usage.critical);
    assert!(usage.value.key_cert_sign());
    assert!(usage.value.crl_sign());
    // keyCertSign is bit 5 and cRLSign bit 6; nothing else is asserted.
    assert_eq!(usage.value.flags, (1 << 5) | (1 << 6));

    let constraints = ca.name_constraints().unwrap().expect("nameConstraints");
    assert!(constraints.critical);
    assert!(
        constraints.value.excluded_subtrees.is_none(),
        "permitted subtrees only: everything else is excluded by RFC 5280"
    );
    let permitted = constraints.value.permitted_subtrees.as_ref().unwrap();
    let mut v4 = vec![127, 0, 0, 1];
    v4.extend([0xff; 4]); // 127.0.0.1/32
    let mut v6 = vec![0; 15];
    v6.push(1);
    v6.extend([0xff; 16]); // ::1/128
    let expected = [
        GeneralName::DNSName("localhost"),
        GeneralName::IPAddress(&v4),
        GeneralName::IPAddress(&v6),
    ];
    assert_eq!(permitted.len(), expected.len());
    for (subtree, want) in permitted.iter().zip(&expected) {
        assert_eq!(&subtree.base, want);
    }

    // A CA has no SAN and no EKU; the CN carries the first eight hex digits of its serial.
    assert!(ca.subject_alternative_name().unwrap().is_none());
    assert!(ca.extended_key_usage().unwrap().is_none());
    let serial = ca.raw_serial();
    assert_eq!(serial.len(), 16);
    let tag = lower_hex(&serial[..4]);
    assert_eq!(
        common_name(ca.subject()),
        format!("Personal Financial Modeling local CA {tag}")
    );
}

#[test]
fn leaf_has_the_three_sans_and_server_auth() {
    let material = issue(fixed_now()).unwrap();
    let ca = parse(&material.ca_cert_der);
    let leaf = parse(&material.leaf_cert_der);

    assert_eq!(leaf.issuer(), ca.subject(), "issued by the local CA");
    assert_ne!(leaf.subject(), leaf.issuer());
    assert_eq!(
        common_name(leaf.subject()),
        "Personal Financial Modeling (this computer)"
    );
    assert_eq!(
        leaf.signature_algorithm.algorithm.to_id_string(),
        ECDSA_WITH_SHA256
    );

    // ECDSA P-256, and not the CA's key.
    for cert in [&ca, &leaf] {
        let algorithm = &cert.public_key().algorithm;
        assert_eq!(algorithm.algorithm.to_id_string(), EC_PUBLIC_KEY);
        let curve = algorithm.parameters.as_ref().unwrap().as_oid().unwrap();
        assert_eq!(curve.to_id_string(), PRIME256V1);
    }
    assert_ne!(leaf.public_key().raw, ca.public_key().raw);

    let san = leaf
        .subject_alternative_name()
        .unwrap()
        .expect("subjectAltName");
    let v6 = std::net::Ipv6Addr::LOCALHOST.octets();
    assert_eq!(
        san.value.general_names,
        [
            GeneralName::DNSName("localhost"),
            GeneralName::IPAddress(&[127, 0, 0, 1]),
            GeneralName::IPAddress(&v6),
        ]
    );

    let eku = leaf
        .extended_key_usage()
        .unwrap()
        .expect("extendedKeyUsage");
    assert!(eku.value.server_auth);
    assert!(!eku.value.any && !eku.value.client_auth && !eku.value.code_signing);
    assert!(!eku.value.email_protection && !eku.value.time_stamping && !eku.value.ocsp_signing);
    assert!(eku.value.other.is_empty());

    let usage = leaf.key_usage().unwrap().expect("keyUsage");
    assert!(usage.critical);
    assert_eq!(usage.value.flags, 1, "digitalSignature (bit 0) only");

    // Not a CA, and it carries no name constraints of its own.
    assert!(leaf
        .basic_constraints()
        .unwrap()
        .is_none_or(|b| !b.value.ca));
    assert!(leaf.name_constraints().unwrap().is_none());

    // authorityKeyIdentifier on the leaf names the CA's subjectKeyIdentifier.
    let ca_ski = ca
        .iter_extensions()
        .find_map(|e| match e.parsed_extension() {
            ParsedExtension::SubjectKeyIdentifier(id) => Some(id.0.to_vec()),
            _ => None,
        })
        .expect("CA subjectKeyIdentifier");
    let leaf_aki = leaf
        .iter_extensions()
        .find_map(|e| match e.parsed_extension() {
            ParsedExtension::AuthorityKeyIdentifier(aki) => {
                aki.key_identifier.as_ref().map(|id| id.0.to_vec())
            }
            _ => None,
        })
        .expect("leaf authorityKeyIdentifier");
    assert_eq!(leaf_aki, ca_ski);
}

#[test]
fn validity_is_825_and_820_days() {
    assert_eq!(CA_VALIDITY, Duration::days(825));
    assert_eq!(LEAF_VALIDITY, Duration::days(820));
    assert_eq!(BACKDATE, Duration::minutes(5));

    let now = fixed_now();
    let whole_second = now.unix_timestamp();
    let material = issue(now).unwrap();
    let ca = parse(&material.ca_cert_der);
    let leaf = parse(&material.leaf_cert_der);

    // Numbers typed here, not derived from the constants under test.
    let five_minutes = 300;
    let day = 86_400;
    assert_eq!(
        ca.validity().not_before.timestamp(),
        whole_second - five_minutes
    );
    assert_eq!(
        ca.validity().not_after.timestamp(),
        whole_second + 825 * day
    );
    assert_eq!(
        leaf.validity().not_before.timestamp(),
        whole_second - five_minutes
    );
    assert_eq!(
        leaf.validity().not_after.timestamp(),
        whole_second + 820 * day
    );
    assert!(leaf.validity().not_after < ca.validity().not_after);
}

#[test]
fn fresh_material_validates_and_builds_a_server_config() {
    let now = OffsetDateTime::now_utc();
    let material = issue(now).unwrap();
    assert_eq!(material.validate(now), Ok(()));

    let config = server_config(&material).unwrap();
    assert_eq!(config.alpn_protocols, [b"http/1.1".to_vec()]);
    assert_eq!(leaf_fingerprint(&material.leaf_cert_der).len(), 95);
}

#[test]
fn expiring_or_mismatched_material_fails_validate() {
    let now = fixed_now();
    let material = issue(now).unwrap();
    let other = issue(now).unwrap();

    // Time: fine until 30 days before the leaf's end, "soon" after, invalid outside.
    assert_eq!(material.validate(now + Duration::days(789)), Ok(()));
    assert_eq!(
        material.validate(now + Duration::days(791)),
        Err(MaterialError::ExpiresSoon)
    );
    assert_eq!(
        material.validate(now + Duration::days(821)),
        Err(MaterialError::ChainInvalid)
    );
    assert_eq!(
        material.validate(now - Duration::days(1)),
        Err(MaterialError::ChainInvalid),
        "not yet valid"
    );
    assert_eq!(
        material.validate(OffsetDateTime::UNIX_EPOCH - Duration::days(1)),
        Err(MaterialError::ClockOutOfRange)
    );

    // A key that is valid but is not the leaf's.
    let swapped_key = TlsMaterial {
        leaf_key: Redacted::new(other.leaf_key.expose().clone_key()),
        ..material.clone()
    };
    assert_eq!(swapped_key.validate(now), Err(MaterialError::KeyMismatch));
    assert!(server_config(&swapped_key).is_err());

    // A CA generated by the same code that did not sign this leaf.
    let swapped_ca = TlsMaterial {
        ca_cert_der: other.ca_cert_der.clone(),
        ..material.clone()
    };
    assert_eq!(swapped_ca.validate(now), Err(MaterialError::ChainInvalid));

    // Bytes that are not what they claim to be.
    let bad_key = TlsMaterial {
        leaf_key: Redacted::new(PrivatePkcs8KeyDer::from(vec![0x30, 0x00])),
        ..material.clone()
    };
    assert_eq!(bad_key.validate(now), Err(MaterialError::KeyUnusable));
    assert!(server_config(&bad_key).is_err());

    let bad_ca = TlsMaterial {
        ca_cert_der: CertificateDer::from(vec![0x30, 0x00]),
        ..material.clone()
    };
    assert_eq!(bad_ca.validate(now), Err(MaterialError::CaUnusable));

    // The leaf presented as its own anchor is not a CA for anything.
    let leaf_as_ca = TlsMaterial {
        ca_cert_der: material.leaf_cert_der.clone(),
        ..material.clone()
    };
    assert!(leaf_as_ca.validate(now).is_err());
}

#[test]
fn issued_material_holds_no_ca_key() {
    let now = OffsetDateTime::now_utc();
    let material = issue(now).unwrap();

    // Exhaustive destructuring: two public certificates and one private key. A new
    // field fails to compile here and has to be justified.
    let TlsMaterial {
        ca_cert_der,
        leaf_cert_der,
        leaf_key,
    } = material.clone();

    // The one private key is the leaf's (it matches the leaf, `validate` says so) and
    // the leaf's public key is not the CA's — so it is not the CA key.
    assert_eq!(material.validate(now), Ok(()));
    assert_ne!(
        parse(&leaf_cert_der).public_key().raw,
        parse(&ca_cert_der).public_key().raw
    );

    // And it never prints.
    let debug = format!("{material:?} {leaf_key:?}");
    assert!(debug.contains("<redacted>"));
    let key_bytes = leaf_key.expose().secret_pkcs8_der();
    let hex = lower_hex(&key_bytes[36..68]);
    assert!(!debug.to_lowercase().contains(&hex));
    assert!(!debug.contains(&format!("{:?}", &key_bytes[36..44])));
}

#[test]
fn memory_leaf_store_round_trips_and_clears() {
    let store = MemoryLeafStore::new();
    assert!(store.load().unwrap().is_none());
    assert_eq!(store.clear(), Ok(()), "clearing an empty store succeeds");

    let first = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&first).unwrap();
    let loaded = store.load().unwrap().expect("stored");
    assert_eq!(loaded.ca_cert_der, first.ca_cert_der);
    assert_eq!(loaded.leaf_cert_der, first.leaf_cert_der);
    assert_eq!(
        loaded.leaf_key.expose().secret_pkcs8_der(),
        first.leaf_key.expose().secret_pkcs8_der()
    );

    // Saving replaces; the store never holds two leaves.
    let second = issue(OffsetDateTime::now_utc()).unwrap();
    store.save(&second).unwrap();
    assert_eq!(
        store.load().unwrap().unwrap().leaf_cert_der,
        second.leaf_cert_der
    );

    store.clear().unwrap();
    assert!(store.load().unwrap().is_none());
    assert!(!format!("{store:?}").contains("leaf"));
}
