//! Decoding against VS Code's iconv-lite, recorded in `iconv-lite.json`, whose `notes` say how
//! the cases were made. Every VS Code encoding but UTF-8 with a byte order mark has a case.

use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    encoding: String,
    /// iconv-lite's characters for the bytes 0x80 to 0xFF.
    high: Option<String>,
    bytes: Option<String>,
    text: Option<String>,
}

fn decode(encoding: &Encoding, bytes: &[u8]) -> String {
    let mut text = String::new();
    // Byte by byte, so sequences cross piece boundaries.
    let mut decoder = Decoder::new(encoding);
    for byte in bytes {
        decoder.decode(std::slice::from_ref(byte), false, &mut text);
    }
    decoder.decode(&[], true, &mut text);
    text
}

/// Where WHATWG holds against iconv-lite for an encoding encoding_rs decodes, as the module
/// states: a byte iconv-lite leaves undefined, Apple's current `macroman` mapping, and the two
/// letters WHATWG's KOI8-U takes from KOI8-RU.
fn whatwg_holds(encoding: &str, byte: u8, iconv: char) -> bool {
    iconv == '\u{FFFD}'
        || (encoding == "macroman" && matches!(byte, 0xBD | 0xDB))
        || (encoding == "koi8u" && matches!(byte, 0xAE | 0xBE))
}

#[test]
fn decoding_matches_vs_code() {
    let fixture: Fixture = serde_json::from_str(include_str!("iconv-lite.json")).unwrap();
    let mut differences = Vec::new();
    for case in &fixture.cases {
        let encoding = match case.encoding.as_str() {
            "utf8" => &UTF8,
            name => find(name),
        };
        assert_eq!(
            encoding.name(),
            case.encoding,
            "a table entry for every case"
        );
        if let Some(high) = &case.high {
            let bytes: Vec<u8> = (0x80..=0xFF).collect();
            let decoded: Vec<char> = decode(encoding, &bytes).chars().collect();
            let expected: Vec<char> = high.chars().collect();
            assert_eq!(decoded.len(), 128, "{}", case.encoding);
            for ((&byte, &ours), &iconv) in bytes.iter().zip(&decoded).zip(&expected) {
                let table = matches!(encoding.decoding, Decoding::Table(_));
                if ours != iconv && (table || !whatwg_holds(&case.encoding, byte, iconv)) {
                    differences.push(format!(
                        "{} {byte:#04X}: {:04X} for {:04X}",
                        case.encoding, ours as u32, iconv as u32
                    ));
                }
            }
            let ascii: Vec<u8> = (0..0x80).collect();
            assert_eq!(
                decode(encoding, &ascii).as_bytes(),
                ascii,
                "{}",
                case.encoding
            );
        }
        if let (Some(bytes), Some(text)) = (&case.bytes, &case.text) {
            let bytes: Vec<u8> = (0..bytes.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&bytes[index..index + 2], 16).unwrap())
                .collect();
            if decode(encoding, &bytes) != *text {
                differences.push(format!("{} sample {text:?}", case.encoding));
            }
        }
    }
    assert!(differences.is_empty(), "{differences:#?}");
}
