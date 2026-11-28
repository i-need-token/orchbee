//! Encoding against VS Code's iconv-lite, recorded in `iconv-lite.json`, whose `notes` say how the
//! cases were made, and the module's rule that a save writes back what a read decoded.

use serde::Deserialize;

use super::*;
use crate::fs::encoding::{Decoder, ENCODINGS};

#[derive(Deserialize)]
struct Fixture {
    characters: Vec<Case>,
    texts: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    encoding: String,
    text: String,
    bytes: String,
}

fn encode(target: Target, text: &str) -> (Vec<u8>, u64) {
    let mut encoder = Encoder::new(target, text);
    let (mut bytes, mut piece) = (Vec::new(), Vec::new());
    while encoder.next(&mut piece) {
        bytes.extend_from_slice(&piece);
    }
    (bytes, encoder.replaced())
}

fn decode(encoding: &Encoding, bytes: &[u8]) -> String {
    let mut text = String::new();
    let mut decoder = Decoder::new(encoding);
    decoder.decode(bytes, true, &mut text);
    text
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).unwrap())
        .collect()
}

fn single_byte(encoding: &Encoding) -> bool {
    match encoding.decoding {
        Decoding::Table(_) => true,
        Decoding::Whatwg(whatwg) => whatwg.is_single_byte(),
    }
}

/// Whether a write may depart from iconv-lite's `expected` bytes for `character`. An encoding
/// decoded through encoding_rs may write other bytes the read decodes back to the character, as
/// WHATWG chooses them, or the `?` iconv-lite writes when even iconv-lite's bytes decode to
/// something else; the in-house tables never depart.
fn departs(encoding: &Encoding, character: char, bytes: &[u8], expected: &[u8]) -> bool {
    let text = character.to_string();
    let replacement = question_marks(character, single_byte(encoding));
    !matches!(encoding.decoding, Decoding::Table(_))
        && (decode(encoding, bytes) == text
            || (bytes == replacement && decode(encoding, expected) != text))
}

#[test]
fn encoding_matches_vs_code() {
    let fixture: Fixture = serde_json::from_str(include_str!("iconv-lite.json")).unwrap();
    let mut differences = Vec::new();
    for case in &fixture.texts {
        let (bytes, _) = encode(Target::new(&case.encoding, false), &case.text);
        if hex(&bytes) != case.bytes {
            let (encoding, text) = (&case.encoding, &case.text);
            differences.push(format!(
                "{encoding} {text:?}: {} for {}",
                hex(&bytes),
                case.bytes
            ));
        }
    }
    for case in &fixture.characters {
        let (target, encoding) = (Target::new(&case.encoding, false), find(&case.encoding));
        for (character, expected) in case.text.chars().zip(case.bytes.split(' ')) {
            let (bytes, _) = encode(target, &character.to_string());
            if hex(&bytes) != expected && !departs(encoding, character, &bytes, &unhex(expected)) {
                let name = &case.encoding;
                differences.push(format!(
                    "{name} {character:?}: {} for {expected}",
                    hex(&bytes)
                ));
            }
        }
    }
    assert!(differences.is_empty(), "{differences:#?}");
}

/// Every byte sequence a read decodes, with its text: each byte of a single-byte encoding, and
/// each two-byte sequence of a multi-byte one, besides EUC-JP's JIS X 0212, and GB18030's
/// four-byte sequences of the Basic Multilingual Plane with a sample of those beyond it.
fn sequences(encoding: &Encoding) -> Vec<(Vec<u8>, String)> {
    let mut sequences: Vec<Vec<u8>> = if single_byte(encoding) {
        (0x80..=0xFF_u8).map(|byte| vec![byte]).collect()
    } else {
        (0x81..=0xFE_u8)
            .flat_map(|lead| (0x40..=0xFE_u8).map(move |trail| vec![lead, trail]))
            .collect()
    };
    if encoding.name() == "eucjp" {
        let rows = (0xA1..=0xFE_u8)
            .flat_map(|lead| (0xA1..=0xFE_u8).map(move |trail| vec![0x8F, lead, trail]));
        sequences.extend(rows);
    }
    if matches!(encoding.name(), "gbk" | "gb18030" | "gb2312") {
        let four = |first: u8, third: usize, fourth: usize| {
            (0x30..=0x39_u8).flat_map(move |second| {
                (0x81..=0xFE_u8).step_by(third).flat_map(move |third| {
                    (0x30..=0x39_u8)
                        .step_by(fourth)
                        .map(move |fourth| vec![first, second, third, fourth])
                })
            })
        };
        let plane = (0x81..=0x84_u8).flat_map(|first| four(first, 1, 1));
        let beyond = (0x90..=0xE3_u8).flat_map(|first| four(first, 25, 3));
        sequences.extend(plane.chain(beyond));
    }
    sequences
        .into_iter()
        .filter_map(|bytes| {
            let text = decode(encoding, &bytes);
            let valid = !text.contains('\u{FFFD}') && !text.is_ascii();
            valid.then_some((bytes, text))
        })
        .collect()
}

#[test]
fn a_save_writes_back_what_a_read_decoded() {
    let mut lost = Vec::new();
    for encoding in &ENCODINGS {
        let target = Target::new(encoding.name(), false);
        for (bytes, text) in sequences(encoding) {
            let (written, replaced) = encode(target, &text);
            if replaced != 0 || decode(encoding, &written) != text {
                lost.push(format!("{} {}: {text:?}", encoding.name(), hex(&bytes)));
            }
        }
    }
    assert!(lost.is_empty(), "{} lost: {lost:#?}", lost.len());
}

#[test]
fn a_long_text_encodes_across_pieces_as_it_does_whole() {
    // A pair that decodes from one sequence, a character of two UTF-8 bytes, one of four, and one
    // no legacy encoding holds; the sample starts where a piece ends inside or after a character.
    let sample = "\u{CA}\u{304}\u{4E2D}\u{1F600}\u{10FFFF}";
    for name in [
        "big5hkscs",
        "gbk",
        "windows1252",
        "iso88591",
        "utf8",
        "utf16be",
    ] {
        let target = Target::new(name, false);
        let (alone, alone_replaced) = encode(target, sample);
        let bom = target.bom.map_or(0, <[u8]>::len);
        let a = &encode(target, "a").0[bom..];
        for before in [PIECE - 2, PIECE - 1] {
            let text = format!("{}{sample}{}", "a".repeat(before), "a".repeat(PIECE));
            let (bytes, replaced) = encode(target, &text);
            let mut expected = alone[..bom].to_vec();
            expected.extend(a.repeat(before));
            expected.extend_from_slice(&alone[bom..]);
            expected.extend(a.repeat(PIECE));
            assert!(bytes == expected, "{name} after {before}");
            assert_eq!(replaced, alone_replaced, "{name}");
        }
    }
}
