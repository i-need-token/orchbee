//! The encoders a write turns its UTF-8 text into a file's bytes with, under the encodings a read
//! decodes. VS Code encodes a save with iconv-lite, and so do these, with one rule over it: a save
//! writes back unchanged every character a read decoded from the file. A character the encoding
//! cannot represent becomes `?`, as iconv-lite writes it: one per UTF-16 code unit in a
//! single-byte encoding, so a character beyond the Basic Multilingual Plane becomes `??`, and one
//! per character in a multi-byte encoding.
//!
//! - UTF-8 copies the text; UTF-16 is encoded here, since encoding_rs has no UTF-16 encoder. UTF-8
//!   starts with a byte order mark when the write asks for one or names `utf8bom`, UTF-16 always
//!   does, and no other encoding ever does, as VS Code writes them.
//! - The in-house tables encode by iconv-lite's own rule over the same tables: the last byte that
//!   decodes to a character encodes it, so U+FFFD becomes the table's last undefined byte.
//! - The encodings decoded through encoding_rs encode through its encoders, where WHATWG holds
//!   against iconv-lite as it does for decoding, choices among equivalent sequences included. A
//!   character the decoder produces but its WHATWG encoder refuses is written back as bytes it
//!   decodes from: U+FFFD as a single-byte encoding's last undefined byte, as iconv-lite writes it;
//!   Big5's HKSCS characters, Shift_JIS's private use area and EUC-JP's JIS X 0212, found by
//!   decoding every sequence of the encoding once; and GBK's four-byte sequences, encoded as
//!   GB18030.

use std::{collections::HashMap, sync::LazyLock};

use encoding_rs::EncoderResult;

use super::{Decoding, Encoding, find};

/// The most input one piece encodes, in bytes of UTF-8.
const PIECE: usize = 256 * 1024;

/// What a write encodes its text into: an encoding, and whether a byte order mark starts it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    encoding: &'static Encoding,
    bom: Option<&'static [u8]>,
}

impl Target {
    /// The target a write names: the encoding [`find`] resolves `name` to, so UTF-8 for a name
    /// this table lacks, as VS Code writes an encoding it cannot load; a byte order mark for UTF-8
    /// with `bom` or the name `utf8bom`, and always for UTF-16.
    pub(crate) fn new(name: &str, bom: bool) -> Self {
        let encoding = find(name);
        let bom: Option<&'static [u8]> = match encoding.name {
            "utf8" if bom || name == "utf8bom" => Some(&[0xEF, 0xBB, 0xBF]),
            "utf16le" => Some(&[0xFF, 0xFE]),
            "utf16be" => Some(&[0xFE, 0xFF]),
            _ => None,
        };
        Self { encoding, bom }
    }
}

/// An encoding of one text into pieces, each of at most [`PIECE`] bytes of input.
pub(crate) struct Encoder<'a> {
    text: &'a str,
    /// The text before it is encoded.
    position: usize,
    bom: Option<&'static [u8]>,
    replaced: u64,
    kind: Kind,
}

enum Kind {
    Utf8,
    Utf16 {
        big_endian: bool,
    },
    Whatwg {
        encoder: encoding_rs::Encoder,
        single_byte: bool,
        fallback: Fallback,
    },
    /// Each character's byte; bytes below 0x80 are ASCII.
    Table(HashMap<char, u8>),
}

/// Where an encoding finds a character its WHATWG encoder refuses.
enum Fallback {
    None,
    /// A single-byte encoding's last undefined byte, which decodes to U+FFFD.
    Undefined(u8),
    /// GBK's decoder is GB18030's, so it decodes four-byte sequences GB18030 encodes.
    Gb18030,
    /// The sequences that decode to such characters.
    Sequences(&'static Sequences),
}

/// The byte sequences of one encoding that decode to characters its WHATWG encoder refuses, each
/// character's first in byte order.
#[derive(Default)]
struct Sequences {
    single: HashMap<char, Box<[u8]>>,
    /// Two characters one sequence decodes to, as Big5's `0x8862` decodes to `Ê̄`.
    pairs: Vec<([char; 2], Box<[u8]>)>,
}

impl<'a> Encoder<'a> {
    pub(crate) fn new(target: Target, text: &'a str) -> Self {
        let kind = match target.encoding.decoding {
            Decoding::Table(characters) => {
                let mut table = HashMap::with_capacity(256);
                let ascii = (0..0x80_u8).map(|byte| (char::from(byte), byte));
                // The table holds 128 characters, so each high byte fits.
                let high = characters.chars().zip(0x80..=0xFF_u8);
                table.extend(ascii.chain(high));
                Kind::Table(table)
            }
            Decoding::Whatwg(encoding) if encoding == encoding_rs::UTF_8 => Kind::Utf8,
            Decoding::Whatwg(encoding) if encoding == encoding_rs::UTF_16LE => {
                Kind::Utf16 { big_endian: false }
            }
            Decoding::Whatwg(encoding) if encoding == encoding_rs::UTF_16BE => {
                Kind::Utf16 { big_endian: true }
            }
            Decoding::Whatwg(encoding) => Kind::Whatwg {
                encoder: encoding.new_encoder(),
                single_byte: encoding.is_single_byte(),
                fallback: fallback(encoding),
            },
        };
        Self {
            text,
            position: 0,
            bom: target.bom,
            replaced: 0,
            kind,
        }
    }

    /// Replaces `output` with the next piece of the encoding; false once the whole text was
    /// encoded, which leaves `output` empty.
    pub(crate) fn next(&mut self, output: &mut Vec<u8>) -> bool {
        output.clear();
        if let Some(bom) = self.bom.take() {
            output.extend_from_slice(bom);
        }
        let start = self.position;
        let mut end = (start + PIECE).min(self.text.len());
        while !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let last = end == self.text.len();
        match &mut self.kind {
            Kind::Utf8 => {
                output.extend_from_slice(&self.text.as_bytes()[start..end]);
                self.position = end;
            }
            Kind::Utf16 { big_endian } => {
                let mut units = [0; 2];
                for character in self.text[start..end].chars() {
                    for unit in character.encode_utf16(&mut units) {
                        let bytes = if *big_endian {
                            unit.to_be_bytes()
                        } else {
                            unit.to_le_bytes()
                        };
                        output.extend_from_slice(&bytes);
                    }
                }
                self.position = end;
            }
            Kind::Table(table) => {
                for character in self.text[start..end].chars() {
                    match table.get(&character) {
                        Some(&byte) => output.push(byte),
                        None => {
                            self.replaced += 1;
                            output.extend_from_slice(question_marks(character, true));
                        }
                    }
                }
                self.position = end;
            }
            Kind::Whatwg {
                encoder,
                single_byte,
                fallback,
            } => {
                let mut position = start;
                while position < end {
                    let input = &self.text[position..end];
                    // Every encoder's bound exists for a piece, which is far below overflow.
                    let room = encoder
                        .max_buffer_length_from_utf8_without_replacement(input.len())
                        .unwrap_or(PIECE * 4);
                    output.reserve(room);
                    let (result, read) =
                        encoder.encode_from_utf8_to_vec_without_replacement(input, output, last);
                    position += read;
                    let EncoderResult::Unmappable(character) = result else {
                        continue;
                    };
                    let following = &self.text[position..];
                    match fallback.find(character, following) {
                        Some((bytes, also)) => {
                            output.extend_from_slice(&bytes);
                            position += also;
                        }
                        None => {
                            self.replaced += 1;
                            output.extend_from_slice(question_marks(character, *single_byte));
                        }
                    }
                }
                self.position = position;
            }
        }
        start < self.text.len() || !output.is_empty()
    }

    /// The characters written as `?` so far.
    pub(crate) const fn replaced(&self) -> u64 {
        self.replaced
    }
}

/// What iconv-lite writes for a character the encoding cannot represent.
fn question_marks(character: char, single_byte: bool) -> &'static [u8] {
    if single_byte && u32::from(character) > 0xFFFF {
        b"??"
    } else {
        b"?"
    }
}

impl Fallback {
    /// The bytes that decode to `character`, followed by the text `following`, and how many bytes
    /// of `following` they also stand for.
    fn find(&self, character: char, following: &str) -> Option<(Box<[u8]>, usize)> {
        match self {
            Self::None => None,
            Self::Undefined(byte) => (character == '\u{FFFD}').then(|| (Box::from([*byte]), 0)),
            Self::Gb18030 => {
                let mut encoder = encoding_rs::GB18030.new_encoder();
                let mut bytes = Vec::with_capacity(4);
                let mut units = [0; 4];
                let input = character.encode_utf8(&mut units);
                let (result, _) =
                    encoder.encode_from_utf8_to_vec_without_replacement(input, &mut bytes, true);
                (result == EncoderResult::InputEmpty).then(|| (bytes.into_boxed_slice(), 0))
            }
            Self::Sequences(sequences) => {
                if let Some(next) = following.chars().next() {
                    let pair = sequences
                        .pairs
                        .iter()
                        .find(|(characters, _)| *characters == [character, next]);
                    if let Some((_, bytes)) = pair {
                        return Some((bytes.clone(), next.len_utf8()));
                    }
                }
                let bytes = sequences.single.get(&character)?;
                Some((bytes.clone(), 0))
            }
        }
    }
}

/// The fallback of `encoding`'s WHATWG encoder.
fn fallback(encoding: &'static encoding_rs::Encoding) -> Fallback {
    static BIG5: LazyLock<Sequences> = LazyLock::new(|| sequences(encoding_rs::BIG5));
    static SHIFT_JIS: LazyLock<Sequences> = LazyLock::new(|| sequences(encoding_rs::SHIFT_JIS));
    static EUC_JP: LazyLock<Sequences> = LazyLock::new(|| sequences(encoding_rs::EUC_JP));
    static EUC_KR: LazyLock<Sequences> = LazyLock::new(|| sequences(encoding_rs::EUC_KR));
    if encoding == encoding_rs::GBK {
        Fallback::Gb18030
    } else if encoding == encoding_rs::BIG5 {
        Fallback::Sequences(&BIG5)
    } else if encoding == encoding_rs::SHIFT_JIS {
        Fallback::Sequences(&SHIFT_JIS)
    } else if encoding == encoding_rs::EUC_JP {
        Fallback::Sequences(&EUC_JP)
    } else if encoding == encoding_rs::EUC_KR {
        Fallback::Sequences(&EUC_KR)
    } else if encoding.is_single_byte() {
        let undefined = (0x80..=0xFF_u8).rev().find(|&byte| {
            encoding
                .decode_without_bom_handling_and_without_replacement(&[byte])
                .is_none()
        });
        undefined.map_or(Fallback::None, Fallback::Undefined)
    } else {
        // GB18030 encodes every character its decoder produces.
        Fallback::None
    }
}

/// Decodes every two-byte sequence of `encoding`, and EUC-JP's three-byte JIS X 0212 sequences,
/// keeping those that decode to what its encoder refuses: one character, or two whose first it
/// refuses.
fn sequences(encoding: &'static encoding_rs::Encoding) -> Sequences {
    let mut found = Sequences::default();
    let pairs =
        (0x81..=0xFE_u8).flat_map(|lead| (0x40..=0xFE_u8).map(move |trail| vec![lead, trail]));
    let jis_x_0212 = (0xA1..=0xFE_u8)
        .flat_map(|lead| (0xA1..=0xFE_u8).map(move |trail| vec![0x8F, lead, trail]))
        .filter(|_| encoding == encoding_rs::EUC_JP);
    for bytes in pairs.chain(jis_x_0212) {
        let Some(decoded) = encoding.decode_without_bom_handling_and_without_replacement(&bytes)
        else {
            continue;
        };
        let mut characters = decoded.chars();
        match (characters.next(), characters.next(), characters.next()) {
            (Some(character), None, None)
                if !character.is_ascii() && !encodes(encoding, character) =>
            {
                found
                    .single
                    .entry(character)
                    .or_insert_with(|| bytes.into_boxed_slice());
            }
            (Some(first), Some(second), None)
                if !first.is_ascii() && !second.is_ascii() && !encodes(encoding, first) =>
            {
                found
                    .pairs
                    .push(([first, second], bytes.into_boxed_slice()));
            }
            _ => {}
        }
    }
    found
}

/// Whether `encoding`'s WHATWG encoder represents `character`.
fn encodes(encoding: &'static encoding_rs::Encoding, character: char) -> bool {
    let mut units = [0; 4];
    let mut bytes = Vec::with_capacity(8);
    let (result, _) = encoding
        .new_encoder()
        .encode_from_utf8_to_vec_without_replacement(
            character.encode_utf8(&mut units),
            &mut bytes,
            true,
        );
    result == EncoderResult::InputEmpty
}

#[cfg(test)]
mod tests;
