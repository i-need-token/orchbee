//! The detection rule against VS Code's own answers, recorded in `corpus.json`, whose `notes` say
//! how the cases were made.

use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    head: String,
    #[serde(default)]
    repeat: String,
    #[serde(default)]
    length: usize,
    #[serde(default)]
    tail: String,
    encoding: Option<String>,
    binary: bool,
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

impl Case {
    fn buffer(&self) -> Vec<u8> {
        let mut buffer = bytes(&self.head);
        let tail = bytes(&self.tail);
        let repeat = bytes(&self.repeat);
        for &byte in repeat.iter().cycle() {
            if buffer.len() + tail.len() >= self.length {
                break;
            }
            buffer.push(byte);
        }
        buffer.extend(tail);
        buffer
    }
}

#[test]
fn detection_matches_vs_code() {
    let corpus: Corpus = serde_json::from_str(include_str!("corpus.json")).unwrap();
    for case in &corpus.cases {
        let buffer = case.buffer();
        let detection = detect(&buffer);
        let expected = match case.encoding.as_deref() {
            None => (None, 0),
            // VS Code names UTF-8 with a byte order mark apart; a read reports it as `utf8` with
            // `bom`.
            Some("utf8bom") => (Some("utf8"), 3),
            Some(name)
                if buffer.starts_with(&[0xFF, 0xFE]) || buffer.starts_with(&[0xFE, 0xFF]) =>
            {
                (Some(name), 2)
            }
            Some(name) => (Some(name), 0),
        };
        assert_eq!(
            (
                detection.encoding.map(Encoding::name),
                detection.bom,
                detection.binary
            ),
            (expected.0, expected.1, case.binary),
            "case {}",
            case.head
        );
    }
}
