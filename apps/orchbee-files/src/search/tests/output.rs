//! rg's JSON output read against VS Code's own parser, however the output is cut into reads.

use serde::Deserialize;

use super::super::{
    json::{Event, Parser},
    preview::{PreviewOptions, Range},
};
use crate::seeded::{self, SplitMix64};

const DEFAULT_SEED: u64 = 0x5eed_0e7b;

#[derive(Deserialize)]
struct Corpus {
    streams: Vec<Stream>,
}

#[derive(Deserialize)]
struct Stream {
    name: String,
    /// rg's output, in base64.
    input: String,
    runs: Vec<Run>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Run {
    max_results: u64,
    preview: (u32, u32),
    events: Vec<Expected>,
    hit_limit: bool,
    malformed: bool,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Expected {
    Match {
        #[serde(rename = "match")]
        path: String,
        ranges: Vec<[[i64; 4]; 2]>,
        preview: String,
    },
    /// A context line, numbered from 1 as VS Code's parser numbers it.
    Context {
        context: String,
        line: u64,
        text: String,
    },
}

fn span(range: &Range) -> [i64; 4] {
    [
        range.start_line,
        range.start_column,
        range.end_line,
        range.end_column,
    ]
}

/// What the parser reads from `input` cut at `cuts`: its results in VS Code's terms, whether it
/// hit its limit, and whether it found a line that is not JSON.
fn parse(input: &[u8], cuts: &[usize], run: &Run) -> (Vec<Expected>, bool, bool) {
    let preview = PreviewOptions {
        match_lines: run.preview.0,
        chars_per_line: run.preview.1,
    };
    let mut parser = Parser::new(preview, run.max_results);
    let mut events = Vec::new();
    let mut start = 0;
    let mut malformed = false;
    for end in cuts.iter().copied().chain([input.len()]) {
        let fed = parser.feed(&input[start..end], &mut |event| match event {
            Event::Match { path, found } => events.push(Expected::Match {
                path: path.as_str().to_owned(),
                ranges: found
                    .ranges
                    .iter()
                    .map(|located| [span(&located.source), span(&located.preview)])
                    .collect(),
                preview: found.preview,
            }),
            Event::Context { path, lines } => {
                for line in lines {
                    events.push(Expected::Context {
                        context: path.as_str().to_owned(),
                        line: line.line + 1,
                        text: line.text,
                    });
                }
            }
        });
        start = end;
        if fed.is_err() {
            malformed = true;
            break;
        }
    }
    (events, parser.hit_limit(), malformed)
}

/// Node's base64.
fn decode(encoded: &str) -> Vec<u8> {
    let mut decoded = Vec::new();
    let (mut buffer, mut bits) = (0_u32, 0);
    for byte in encoded.bytes().take_while(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            _ => 63,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            decoded.push((buffer >> bits) as u8);
        }
    }
    decoded
}

#[test]
fn output_reads_as_vs_code_reads_it_in_any_cut() {
    let corpus: Corpus = serde_json::from_str(include_str!("output.json")).unwrap();
    let seed = seeded::seed("ORCHBEE_FILES_SEARCH_OUTPUT_SEED", DEFAULT_SEED);
    let mut random = SplitMix64::new(seed);
    for stream in &corpus.streams {
        let input = decode(&stream.input);
        let mut cuttings: Vec<Vec<usize>> = vec![Vec::new(), (1..input.len()).collect()];
        for _ in 0..4 {
            let mut cuts: Vec<usize> = (0..random.below(12))
                .map(|_| random.below(input.len()))
                .collect();
            cuts.sort_unstable();
            cuttings.push(cuts);
        }
        for run in &stream.runs {
            for cuts in &cuttings {
                let (events, hit_limit, malformed) = parse(&input, cuts, run);
                let context = format!(
                    "{} with {} results and preview {:?}, cut at {cuts:?}, seed {seed:#x}",
                    stream.name, run.max_results, run.preview
                );
                assert_eq!(events, run.events, "{context}");
                assert_eq!(hit_limit, run.hit_limit, "{context}");
                assert_eq!(malformed, run.malformed, "{context}");
            }
        }
    }
}
