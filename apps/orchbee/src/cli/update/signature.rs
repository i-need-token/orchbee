//! Release manifest trust: the compiled public key tables, the detached signature document, and
//! Ed25519 verification over the domain-separated manifest bytes. A manifest is parsed only through
//! the [`TrustedManifest`] this module returns. No I/O; acquisition and installation live elsewhere.
use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use orchbee_release_records::{check_extensions, closed_object, parse_object};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The most manifest bytes an updater reads or verifies. This bound never shrinks.
pub(super) const MANIFEST_LIMIT: usize = 1024 * 1024;
/// The most signature document bytes an updater reads. This bound never shrinks.
pub(super) const DOCUMENT_LIMIT: usize = 4 * 1024;
const MAXIMUM_SIGNATURES: usize = 4;
/// Signed bytes are this prefix followed by the exact manifest bytes.
const DOMAIN: &[u8] = b"orchbee-release-manifest-v2\n";

/// One compiled Ed25519 public key that may sign release manifests.
pub(super) struct TrustedKey(pub(super) [u8; 32]);

impl TrustedKey {
    /// Decodes 64 lowercase hexadecimal digits at compile time; anything else fails the build.
    const fn from_hex(hex: &str) -> Self {
        let digits = hex.as_bytes();
        assert!(digits.len() == 64, "a public key is 64 hexadecimal digits");
        let mut bytes = [0_u8; 32];
        let mut index = 0;
        while index < 32 {
            bytes[index] = (nibble(digits[2 * index]) << 4) | nibble(digits[2 * index + 1]);
            index += 1;
        }
        Self(bytes)
    }

    /// The first 16 hexadecimal digits of SHA-256 over the 32-byte public key.
    fn key_id(&self) -> String {
        format!("{:x}", Sha256::digest(self.0))[..16].to_owned()
    }
}

const fn nibble(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        _ => panic!("a public key uses lowercase hexadecimal digits"),
    }
}

/// Keys that sign released manifests. Adding a key is one `TrustedKey::from_hex("<64 hex>"),`
/// line; a release adds the next key before signing with it and removes a key only after it
/// retires.
const RELEASE_KEYS: &[TrustedKey] = &[
    // The key the release workflow signs with.
    TrustedKey::from_hex("16bcd2b392d5427892787568853a7ff2d2e40393761c25f187823f2818ec1497"),
    // The spare kept offline, which signs once the first key retires.
    TrustedKey::from_hex("16bf2794f79721de5621c2f0d3c23afc96bc91d5183bfaef71b26bae93c14839"),
];

/// The test key, whose private half `fixtures/test-only-signing-key.pem` is public in the
/// repository. Only development and test builds trust it.
const TEST_KEYS: &[TrustedKey] = &[TrustedKey::from_hex(
    "02c5eb6b1228c0f555fabd1295d89777920595bae4cfe384f28c4b57edff3518",
)];

/// The keys this build trusts. The release environment compiles only the release table and every
/// other environment only the test table; no runtime input changes the choice. POSIX builds keep the
/// release environment until ORC-328, and their update effects stay unsupported.
pub(super) const TRUSTED_KEYS: &[TrustedKey] = if orchbee_local_runtime::is_release() {
    RELEASE_KEYS
} else {
    TEST_KEYS
};

/// The test table for tests, which never verify against the release table.
#[cfg(test)]
pub(super) const TEST_TABLE: &[TrustedKey] = TEST_KEYS;
/// Every compiled table, for checks that each entry is a usable key.
#[cfg(test)]
pub(super) const ALL_TABLES: [&[TrustedKey]; 2] = [RELEASE_KEYS, TEST_KEYS];

/// Manifest bytes that a compiled key verified, or that an earlier verified installation stored.
pub(super) struct TrustedManifest<'a>(&'a [u8]);

impl<'a> TrustedManifest<'a> {
    /// Bytes of an installed version directory, which only a verified installation publishes and
    /// whose digest the store pointer records. The caller checks that binding.
    pub(super) fn installed(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    pub(super) fn bytes(&self) -> &'a [u8] {
        self.0
    }
}

/// Why a manifest was not trusted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    /// The build trusts no key, so it refuses every manifest.
    NoTrustedKeys,
    /// The manifest exceeds [`MANIFEST_LIMIT`].
    ManifestTooLarge,
    /// The signature document is oversized or malformed.
    Malformed,
    /// No signature names a compiled key.
    Untrusted,
    /// A signature names a compiled key, but none verifies.
    Invalid,
}

/// Admits `manifest` when one signature from a compiled key in `keys` verifies over the
/// domain-separated bytes. Entries naming other keys are skipped.
///
/// # Errors
/// Returns the [`Refusal`] a person needs to tell an untrusting build, a malformed document, an
/// unknown signer and a failed verification apart.
pub(super) fn verify<'a>(
    document: &[u8],
    manifest: &'a [u8],
    keys: &[TrustedKey],
) -> Result<TrustedManifest<'a>, Refusal> {
    if keys.is_empty() {
        return Err(Refusal::NoTrustedKeys);
    }
    if manifest.len() > MANIFEST_LIMIT {
        return Err(Refusal::ManifestTooLarge);
    }
    let entries = parse_document(document).ok_or(Refusal::Malformed)?;
    let mut signed = Vec::with_capacity(DOMAIN.len() + manifest.len());
    signed.extend_from_slice(DOMAIN);
    signed.extend_from_slice(manifest);
    let mut named = false;
    for (key_id, signature) in &entries {
        for key in keys.iter().filter(|key| key.key_id() == *key_id) {
            named = true;
            // A table entry that is not a curve point verifies nothing; tests reject such entries.
            let Ok(public) = VerifyingKey::from_bytes(&key.0) else {
                continue;
            };
            if public.verify_strict(&signed, signature).is_ok() {
                return Ok(TrustedManifest(manifest));
            }
        }
    }
    Err(if named {
        Refusal::Invalid
    } else {
        Refusal::Untrusted
    })
}

/// The `(keyId, signature)` entries of a closed signature document.
fn parse_document(document: &[u8]) -> Option<Vec<(String, Signature)>> {
    if document.len() > DOCUMENT_LIMIT {
        return None;
    }
    let object = parse_object(document)?;
    let root = Value::Object(object);
    let object = closed_object(&root, &["signatures"], &["extensions"])?;
    check_extensions(object)?;
    let signatures = object.get("signatures")?.as_array()?;
    if signatures.is_empty() || signatures.len() > MAXIMUM_SIGNATURES {
        return None;
    }
    signatures
        .iter()
        .map(|entry| {
            let entry = closed_object(entry, &["keyId", "algorithm", "signature"], &[])?;
            if entry.get("algorithm")?.as_str()? != "ed25519" {
                return None;
            }
            let key_id = entry.get("keyId")?.as_str()?;
            if key_id.len() != 16
                || !key_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return None;
            }
            // The standard engine requires canonical padding and rejects trailing bits.
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(entry.get("signature")?.as_str()?)
                .ok()?;
            let bytes: [u8; 64] = bytes.try_into().ok()?;
            Some((key_id.to_owned(), Signature::from_bytes(&bytes)))
        })
        .collect()
}
