//! Signature admission over the test key table: the checked-in release tooling output, changed
//! bytes, unknown signers, domain separation, and document and manifest bounds.
use super::super::signature::{
    ALL_TABLES, DOCUMENT_LIMIT, MANIFEST_LIMIT, Refusal, TEST_TABLE, verify,
};
use super::{entry, sign, signed_bytes, stranger, test_key};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::VerifyingKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// `scripts/sign-release-manifest.mjs` produced this signature with the test key.
const MANIFEST: &[u8] = include_bytes!("fixtures/release.json");
const SIGNATURE: &[u8] = include_bytes!("fixtures/release.json.sig");

fn refusal(document: &Value, manifest: &[u8]) -> Option<Refusal> {
    verify(&serde_json::to_vec(document).unwrap(), manifest, TEST_TABLE).err()
}

#[test]
fn the_release_tooling_signature_verifies_and_one_changed_byte_is_refused() {
    let trusted = verify(SIGNATURE, MANIFEST, TEST_TABLE).unwrap();
    assert_eq!(trusted.bytes(), MANIFEST);
    for index in [0, MANIFEST.len() / 2, MANIFEST.len() - 1] {
        let mut changed = MANIFEST.to_vec();
        changed[index] ^= 1;
        assert_eq!(
            verify(SIGNATURE, &changed, TEST_TABLE).err(),
            Some(Refusal::Invalid),
            "{index}"
        );
    }
    let mut document: Value = serde_json::from_slice(SIGNATURE).unwrap();
    let mut signature = STANDARD
        .decode(document["signatures"][0]["signature"].as_str().unwrap())
        .unwrap();
    signature[10] ^= 1;
    document["signatures"][0]["signature"] = json!(STANDARD.encode(signature));
    assert_eq!(refusal(&document, MANIFEST), Some(Refusal::Invalid));
}

#[test]
fn only_a_compiled_key_over_the_domain_separated_bytes_admits_a_manifest() {
    // The verifier never reads the manifest, so any bytes stand in for one.
    let manifest = b"{\"schemaVersion\":2}";
    let unknown = entry(&stranger(), &signed_bytes(manifest));
    let trusted = entry(&test_key(), &signed_bytes(manifest));
    assert_eq!(
        refusal(&json!({"signatures": [unknown]}), manifest),
        Some(Refusal::Untrusted)
    );
    assert_eq!(
        refusal(&json!({"signatures": [unknown, trusted]}), manifest),
        None
    );
    // The trusted key's signature without the prefix, or under another prefix, never verifies.
    for signed in [
        manifest.to_vec(),
        [b"orchbee-release-manifest-v1\n".as_slice(), manifest].concat(),
        [b"orchbee-release-manifest-v2".as_slice(), manifest].concat(),
    ] {
        let document = json!({"signatures": [entry(&test_key(), &signed)]});
        assert_eq!(refusal(&document, manifest), Some(Refusal::Invalid));
    }
    // A stranger's signature relabelled with the trusted key identifier is still refused.
    let mut relabelled = unknown;
    relabelled["keyId"] = trusted["keyId"].clone();
    assert_eq!(
        refusal(&json!({"signatures": [relabelled]}), manifest),
        Some(Refusal::Invalid)
    );
    // A build without a trusted key refuses even a valid signature.
    assert_eq!(
        verify(&sign(manifest), manifest, &[]).err(),
        Some(Refusal::NoTrustedKeys)
    );
}

#[test]
fn signature_documents_are_closed_and_hold_at_most_four_signatures() {
    let manifest = b"manifest";
    let trusted = entry(&test_key(), &signed_bytes(manifest));
    let unknown = entry(&stranger(), &signed_bytes(manifest));
    let four = json!({"signatures": [unknown, unknown, unknown, trusted]});
    assert_eq!(refusal(&four, manifest), None);
    let five = json!({"signatures": [unknown, unknown, unknown, unknown, trusted]});
    assert_eq!(refusal(&five, manifest), Some(Refusal::Malformed));
    let extended = json!({"signatures": [trusted], "extensions": {"note": {"ignored": [1]}}});
    assert_eq!(refusal(&extended, manifest), None);

    let changed = |member: &str, value: Value| {
        let mut entry = trusted.clone();
        entry[member] = value;
        json!({"signatures": [entry]})
    };
    let bytes = |count: usize| json!(STANDARD.encode(vec![0_u8; count]));
    let base64 = trusted["signature"].as_str().unwrap();
    for document in [
        json!({}),
        json!({"signatures": []}),
        json!({"signatures": trusted}),
        json!({"signatures": [trusted], "extensions": []}),
        json!({"signatures": [trusted], "comment": "unknown member"}),
        changed("algorithm", json!("Ed25519")),
        changed("algorithm", json!("ed448")),
        changed(
            "keyId",
            json!(trusted["keyId"].as_str().unwrap().to_uppercase()),
        ),
        changed("keyId", json!(&trusted["keyId"].as_str().unwrap()[..15])),
        changed("signature", bytes(63)),
        changed("signature", bytes(65)),
        changed("signature", json!(base64.trim_end_matches('='))),
        changed("signature", json!(format!(" {base64}"))),
        changed("extra", json!(true)),
    ] {
        assert_eq!(
            refusal(&document, manifest),
            Some(Refusal::Malformed),
            "{document}"
        );
    }
    let entry = trusted.to_string();
    for text in [
        format!(r#"{{"signatures":[{entry}],"signatures":[{entry}]}}"#),
        format!(r#"{{"signatures":[{entry}]}} {{}}"#),
        format!(r#"[{entry}]"#),
    ] {
        assert_eq!(
            verify(text.as_bytes(), manifest, TEST_TABLE).err(),
            Some(Refusal::Malformed),
            "{text}"
        );
    }
}

#[test]
fn documents_and_manifests_are_read_only_within_their_bounds() {
    let manifest = b"manifest";
    let trusted = entry(&test_key(), &signed_bytes(manifest));
    let padded = |length: usize| {
        let base = json!({"signatures": [trusted], "extensions": {"pad": ""}}).to_string();
        let text = base.replace(
            r#""pad":"""#,
            &format!(r#""pad":"{}""#, "x".repeat(length - base.len())),
        );
        assert_eq!(text.len(), length);
        text
    };
    assert!(verify(padded(DOCUMENT_LIMIT).as_bytes(), manifest, TEST_TABLE).is_ok());
    assert_eq!(
        verify(padded(DOCUMENT_LIMIT + 1).as_bytes(), manifest, TEST_TABLE).err(),
        Some(Refusal::Malformed)
    );
    let largest = vec![b' '; MANIFEST_LIMIT];
    assert!(verify(&sign(&largest), &largest, TEST_TABLE).is_ok());
    let oversized = vec![b' '; MANIFEST_LIMIT + 1];
    assert_eq!(
        verify(&sign(&oversized), &oversized, TEST_TABLE).err(),
        Some(Refusal::ManifestTooLarge)
    );
}

#[test]
fn every_compiled_key_is_a_strong_distinct_ed25519_key_and_tables_share_none() {
    let mut identifiers = BTreeSet::new();
    for table in ALL_TABLES {
        for key in table {
            let public = VerifyingKey::from_bytes(&key.0).expect("an Ed25519 curve point");
            assert!(!public.is_weak());
            assert!(identifiers.insert(format!("{:x}", Sha256::digest(key.0))[..16].to_owned()));
        }
    }
    let test_identifier = format!(
        "{:x}",
        Sha256::digest(test_key().verifying_key().to_bytes())
    )[..16]
        .to_owned();
    assert_eq!(test_identifier, "b41b7cd3822bd0c2");
    assert!(
        TEST_TABLE
            .iter()
            .any(|key| key.0 == test_key().verifying_key().to_bytes())
    );
}
