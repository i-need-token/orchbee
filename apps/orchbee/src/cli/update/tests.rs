//! Release evidence: feed, signature and manifest admission against the test key table only, and
//! the install and update engine driven in process with fixture roots. No test verifies with the
//! release key table, contacts a host, runs the release-build check, or changes a user PATH.
#[cfg(windows)]
mod channel_persistence;
mod feed_admission;
#[cfg(windows)]
mod install_fixture;
mod manifest_admission;
#[cfg(windows)]
mod offered_update;
#[cfg(windows)]
mod offline_install;
mod path_value;
mod signature_admission;

use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// The checked-in test key, which only development and test builds trust.
const TEST_KEY: &str = include_str!("tests/fixtures/test-only-signing-key.pem");

/// The test signing key decoded from its PKCS #8 PEM.
fn test_key() -> SigningKey {
    let body: String = TEST_KEY
        .lines()
        .skip_while(|line| *line != "-----BEGIN PRIVATE KEY-----")
        .skip(1)
        .take_while(|line| *line != "-----END PRIVATE KEY-----")
        .collect();
    let der = STANDARD.decode(body).unwrap();
    // PKCS #8 wraps an Ed25519 seed in a fixed 16-byte prefix.
    let (prefix, seed) = der.split_at(16);
    assert_eq!(
        prefix,
        [
            0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22,
            0x04, 0x20
        ]
    );
    SigningKey::from_bytes(&seed.try_into().unwrap())
}

/// A key that no compiled table holds.
fn stranger() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

/// The bytes a release signature covers: the domain prefix, then the exact manifest.
fn signed_bytes(manifest: &[u8]) -> Vec<u8> {
    [b"orchbee-release-manifest-v2\n".as_slice(), manifest].concat()
}

/// One signature document entry by `key` over exactly `signed`.
fn entry(key: &SigningKey, signed: &[u8]) -> Value {
    json!({
        "keyId": format!("{:x}", Sha256::digest(key.verifying_key().to_bytes()))[..16],
        "algorithm": "ed25519",
        "signature": STANDARD.encode(key.sign(signed).to_bytes()),
    })
}

/// A signature document carrying one test-key signature over `manifest`.
fn sign(manifest: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({"signatures": [entry(&test_key(), &signed_bytes(manifest))]}))
        .unwrap()
}

/// The bytes a feed signature covers: the feed domain prefix, then the exact feed.
fn feed_signed_bytes(feed: &[u8]) -> Vec<u8> {
    [b"orchbee-update-feed-v1\n".as_slice(), feed].concat()
}

/// A signature document carrying one test-key signature over the feed `feed`.
fn sign_feed(feed: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&json!({"signatures": [entry(&test_key(), &feed_signed_bytes(feed))]}))
        .unwrap()
}
