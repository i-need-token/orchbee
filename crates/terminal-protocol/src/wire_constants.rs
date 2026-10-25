//! Private binary-format bindings to schemas/terminal/v1/constants.json.
//! Decoders consume these values; the registry remains the language-neutral authority.

/// Fixed prefix identity used by the owning strict binary decoder.
pub(crate) struct BinaryFormat {
    /// Four ASCII bytes identifying the record family.
    pub(crate) magic: &'static [u8; 4],
    /// Exact major, minor and patch accepted by the decoder.
    pub(crate) version: [u8; 3],
    /// Minimum complete fixed prefix before variable records can be read.
    pub(crate) header_bytes: usize,
}

/// Transport record family shared by envelope encoding and decoding.
pub(crate) const TRANSPORT_MAGIC: &[u8; 4] = b"OBTP";
/// Terminal frame prefix accepted before route and text admission.
pub(crate) const FRAME: BinaryFormat = BinaryFormat {
    magic: b"OBTF",
    version: [0, 1, 0],
    header_bytes: 160,
};
/// Packed projection prefix accepted before column validation.
pub(crate) const PROJECTION_BUFFER: BinaryFormat = BinaryFormat {
    magic: b"OBPB",
    version: [0, 1, 0],
    header_bytes: 320,
};
/// Shared row-window prefix for text and render detail.
pub(crate) const ROW_WINDOW: BinaryFormat = BinaryFormat {
    magic: b"OBRW",
    version: [0, 1, 0],
    header_bytes: 112,
};
/// Graphics extension prefix checked by render-window and text-frame admission.
pub(crate) const GRAPHICS: BinaryFormat = BinaryFormat {
    magic: b"OBKG",
    version: [0, 1, 0],
    header_bytes: 40,
};

#[cfg(test)]
mod tests;
