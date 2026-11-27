//! The encodings a read decodes, under VS Code's encoding identifiers. encoding_rs decodes the
//! encodings the WHATWG Encoding Standard defines; `cp950` and `big5hkscs` decode as Big5 and
//! `gb2312` as GBK. In-house tables decode the single-byte encodings WHATWG maps to another
//! encoding or lacks, as VS Code's iconv-lite does. Malformed input decodes to U+FFFD. A read that
//! names an identifier this table lacks decodes UTF-8 and reports `utf8`, as VS Code falls back
//! for an encoding it cannot load.
//!
//! Where WHATWG and iconv-lite differ for an encoding decoded through encoding_rs, WHATWG holds: a
//! byte iconv-lite leaves undefined decodes to WHATWG's character, such as a C1 control in
//! `windows1252`, which a save writes back unchanged; `macroman` decodes 0xBD as U+03A9 and 0xDB
//! as U+20AC, Apple's current mapping; and `koi8u` decodes 0xAE and 0xBE as KOI8-RU's ў and Ў
//! rather than as box drawing.

use encoding_rs::CoderResult;

mod tables;

/// One encoding a read can decode.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Encoding {
    name: &'static str,
    decoding: Decoding,
}

#[derive(Debug, Eq, PartialEq)]
enum Decoding {
    Whatwg(&'static encoding_rs::Encoding),
    /// The characters of the bytes 0x80 to 0xFF; lower bytes are ASCII.
    Table(&'static str),
}

impl Encoding {
    /// The VS Code identifier a read reports.
    pub(crate) const fn name(&self) -> &'static str {
        self.name
    }
}

pub(crate) static UTF8: Encoding = whatwg("utf8", encoding_rs::UTF_8);
pub(crate) static UTF16LE: Encoding = whatwg("utf16le", encoding_rs::UTF_16LE);
pub(crate) static UTF16BE: Encoding = whatwg("utf16be", encoding_rs::UTF_16BE);

const fn whatwg(name: &'static str, encoding: &'static encoding_rs::Encoding) -> Encoding {
    Encoding {
        name,
        decoding: Decoding::Whatwg(encoding),
    }
}

const fn table(name: &'static str, characters: &'static str) -> Encoding {
    Encoding {
        name,
        decoding: Decoding::Table(characters),
    }
}

/// The encodings a read decodes besides UTF-8 and UTF-16.
static ENCODINGS: [Encoding; 45] = [
    whatwg("windows1252", encoding_rs::WINDOWS_1252),
    table("iso88591", tables::ISO88591),
    whatwg("iso88593", encoding_rs::ISO_8859_3),
    whatwg("iso885915", encoding_rs::ISO_8859_15),
    whatwg("macroman", encoding_rs::MACINTOSH),
    table("cp437", tables::CP437),
    whatwg("windows1256", encoding_rs::WINDOWS_1256),
    whatwg("iso88596", encoding_rs::ISO_8859_6),
    whatwg("windows1257", encoding_rs::WINDOWS_1257),
    whatwg("iso88594", encoding_rs::ISO_8859_4),
    whatwg("iso885914", encoding_rs::ISO_8859_14),
    whatwg("windows1250", encoding_rs::WINDOWS_1250),
    whatwg("iso88592", encoding_rs::ISO_8859_2),
    table("cp852", tables::CP852),
    whatwg("windows1251", encoding_rs::WINDOWS_1251),
    whatwg("cp866", encoding_rs::IBM866),
    table("cp1125", tables::CP1125),
    whatwg("iso88595", encoding_rs::ISO_8859_5),
    whatwg("koi8r", encoding_rs::KOI8_R),
    whatwg("koi8u", encoding_rs::KOI8_U),
    whatwg("iso885913", encoding_rs::ISO_8859_13),
    whatwg("windows1253", encoding_rs::WINDOWS_1253),
    whatwg("iso88597", encoding_rs::ISO_8859_7),
    whatwg("windows1255", encoding_rs::WINDOWS_1255),
    whatwg("iso88598", encoding_rs::ISO_8859_8),
    whatwg("iso885910", encoding_rs::ISO_8859_10),
    whatwg("iso885916", encoding_rs::ISO_8859_16),
    whatwg("windows1254", encoding_rs::WINDOWS_1254),
    table("iso88599", tables::ISO88599),
    table("cp857", tables::CP857),
    whatwg("windows1258", encoding_rs::WINDOWS_1258),
    whatwg("gbk", encoding_rs::GBK),
    whatwg("gb18030", encoding_rs::GB18030),
    whatwg("cp950", encoding_rs::BIG5),
    whatwg("big5hkscs", encoding_rs::BIG5),
    whatwg("shiftjis", encoding_rs::SHIFT_JIS),
    whatwg("eucjp", encoding_rs::EUC_JP),
    whatwg("euckr", encoding_rs::EUC_KR),
    whatwg("windows874", encoding_rs::WINDOWS_874),
    table("iso885911", tables::ISO885911),
    table("koi8ru", tables::KOI8RU),
    table("koi8t", tables::KOI8T),
    whatwg("gb2312", encoding_rs::GBK),
    table("cp865", tables::CP865),
    table("cp850", tables::CP850),
];

/// The encoding VS Code names `name`, or UTF-8 when this table lacks it. `utf8bom` is UTF-8,
/// whose byte order mark detection finds.
pub(crate) fn find(name: &str) -> &'static Encoding {
    match name {
        "utf16le" => &UTF16LE,
        "utf16be" => &UTF16BE,
        _ => ENCODINGS
            .iter()
            .find(|encoding| encoding.name == name)
            .unwrap_or(&UTF8),
    }
}

/// A decoder of one encoding into UTF-8, across the pieces of one input. Byte order marks are not
/// its concern: detection finds them and the read skips them.
pub(crate) struct Decoder(Inner);

enum Inner {
    Whatwg(encoding_rs::Decoder),
    Table(Box<[char; 128]>),
}

impl Decoder {
    pub(crate) fn new(encoding: &Encoding) -> Self {
        Self(match encoding.decoding {
            Decoding::Whatwg(encoding) => {
                Inner::Whatwg(encoding.new_decoder_without_bom_handling())
            }
            Decoding::Table(characters) => {
                let mut table = Box::new(['\u{FFFD}'; 128]);
                for (slot, character) in table.iter_mut().zip(characters.chars()) {
                    *slot = character;
                }
                Inner::Table(table)
            }
        })
    }

    /// Appends the decoding of `input` to `output`. With `last`, an incomplete sequence at the
    /// end decodes to U+FFFD; without it, the sequence waits for the next piece.
    pub(crate) fn decode(&mut self, mut input: &[u8], last: bool, output: &mut String) {
        let decoder = match &mut self.0 {
            Inner::Whatwg(decoder) => decoder,
            Inner::Table(table) => {
                output.reserve(input.len());
                output.extend(input.iter().map(|&byte| match byte.checked_sub(0x80) {
                    Some(high) => table[usize::from(high)],
                    None => char::from(byte),
                }));
                return;
            }
        };
        loop {
            // The bound exists for every piece a read hands over, which is far below overflow.
            let room = decoder
                .max_utf8_buffer_length(input.len())
                .unwrap_or(usize::MAX / 2);
            output.reserve(room);
            let (result, read, _) = decoder.decode_to_string(input, output, last);
            input = &input[read..];
            if result == CoderResult::InputEmpty {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;
