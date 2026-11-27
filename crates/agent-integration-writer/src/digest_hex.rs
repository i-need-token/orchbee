//! The lowercase hexadecimal spelling of a digest, the form every recorded SHA-256 takes.

pub(crate) fn lower_hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
