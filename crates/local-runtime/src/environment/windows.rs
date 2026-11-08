//! The Windows environment stem compiled into this build.

/// The stem of this build's cohort file names and environment. Staging sets
/// `ORCHBEE_ARTIFACT_STEM` while it compiles the cohort, so each environment ships its own names
/// (ORC-382). A build without it is this checkout's test environment, `ob-test-<checkout digest>`,
/// so only a build that names `orchbee` can act on the release installation; `build.rs` resolves
/// the stem.
pub(super) const ARTIFACT_STEM: &str = env!("ORCHBEE_ENVIRONMENT_STEM");
// A malformed stem would name files no staging produces, so it fails the build instead.
const _: () = assert!(
    valid_artifact_stem(ARTIFACT_STEM.as_bytes()),
    "ORCHBEE_ARTIFACT_STEM must be orchbee, ob-dev-<16 lowercase hex>, or ob-test-<16 lowercase hex>"
);
/// Mirrors `ARTIFACT_STEM_PATTERN` in `scripts/stage-native-windows.mjs`.
const fn valid_artifact_stem(stem: &[u8]) -> bool {
    let digest = match stem {
        b"orchbee" => return true,
        [b'o', b'b', b'-', b'd', b'e', b'v', b'-', digest @ ..]
        | [b'o', b'b', b'-', b't', b'e', b's', b't', b'-', digest @ ..] => digest,
        _ => return false,
    };
    if digest.len() != 16 {
        return false;
    }
    let mut index = 0;
    while index < digest.len() {
        if !matches!(digest[index], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        index += 1;
    }
    true
}
