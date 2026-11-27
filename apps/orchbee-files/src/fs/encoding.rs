//! The encodings a read decodes, under VS Code's encoding identifiers. encoding_rs decodes the
//! encodings the WHATWG Encoding Standard defines; `cp950` and `big5hkscs` decode as Big5 and
//! `gb2312` as GBK. Malformed input decodes to U+FFFD. A read that names an identifier this table
//! lacks decodes UTF-8 and reports `utf8`, as VS Code falls back for an encoding it cannot load.

use encoding_rs::CoderResult;

/// One encoding a read can decode.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Encoding {
    name: &'static str,
    decoding: &'static encoding_rs::Encoding,
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

const fn whatwg(name: &'static str, decoding: &'static encoding_rs::Encoding) -> Encoding {
    Encoding { name, decoding }
}

/// The encodings a read decodes besides UTF-8 and UTF-16.
static ENCODINGS: [Encoding; 34] = [
    whatwg("windows1252", encoding_rs::WINDOWS_1252),
    whatwg("iso88593", encoding_rs::ISO_8859_3),
    whatwg("iso885915", encoding_rs::ISO_8859_15),
    whatwg("macroman", encoding_rs::MACINTOSH),
    whatwg("windows1256", encoding_rs::WINDOWS_1256),
    whatwg("iso88596", encoding_rs::ISO_8859_6),
    whatwg("windows1257", encoding_rs::WINDOWS_1257),
    whatwg("iso88594", encoding_rs::ISO_8859_4),
    whatwg("iso885914", encoding_rs::ISO_8859_14),
    whatwg("windows1250", encoding_rs::WINDOWS_1250),
    whatwg("iso88592", encoding_rs::ISO_8859_2),
    whatwg("windows1251", encoding_rs::WINDOWS_1251),
    whatwg("cp866", encoding_rs::IBM866),
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
    whatwg("windows1258", encoding_rs::WINDOWS_1258),
    whatwg("gbk", encoding_rs::GBK),
    whatwg("gb18030", encoding_rs::GB18030),
    whatwg("cp950", encoding_rs::BIG5),
    whatwg("big5hkscs", encoding_rs::BIG5),
    whatwg("shiftjis", encoding_rs::SHIFT_JIS),
    whatwg("eucjp", encoding_rs::EUC_JP),
    whatwg("euckr", encoding_rs::EUC_KR),
    whatwg("windows874", encoding_rs::WINDOWS_874),
    whatwg("gb2312", encoding_rs::GBK),
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
pub(crate) struct Decoder(encoding_rs::Decoder);

impl Decoder {
    pub(crate) fn new(encoding: &Encoding) -> Self {
        Self(encoding.decoding.new_decoder_without_bom_handling())
    }

    /// Appends the decoding of `input` to `output`. With `last`, an incomplete sequence at the
    /// end decodes to U+FFFD; without it, the sequence waits for the next piece.
    pub(crate) fn decode(&mut self, mut input: &[u8], last: bool, output: &mut String) {
        loop {
            // The bound exists for every piece a read hands over, which is far below overflow.
            let room = self
                .0
                .max_utf8_buffer_length(input.len())
                .unwrap_or(usize::MAX / 2);
            output.reserve(room);
            let (result, read, _) = self.0.decode_to_string(input, output, last);
            input = &input[read..];
            if result == CoderResult::InputEmpty {
                return;
            }
        }
    }
}
