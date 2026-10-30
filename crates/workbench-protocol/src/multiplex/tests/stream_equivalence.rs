//! Differential evidence for the two OBWP stream decoders. For the same bytes, the blocking reader,
//! one whole-buffer push and a seeded partition of pushes must yield the same messages and the
//! same ending, never panic, and only accept records that re-encode to exactly the bytes they
//! consumed. Cases derive from a fixed seed; `ORCHBEE_PROTOCOL_MUTATION_SEED` explores another,
//! and a failure prints its minimal edits and cuts.

use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

const DEFAULT_SEED: u64 = 0x0b7d_0238_5eed_0002;
const RANDOM_CASES: u64 = 2048;
const BOUNDARY_BYTES: [u8; 6] = [0x00, 0x01, 0x7f, 0x80, 0xfe, 0xff];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ending {
    Clean,
    Incomplete,
    Invalid,
}

#[derive(Clone, Copy, Debug)]
enum Edit {
    Set(usize, u8),
    Xor(usize, u8),
    Insert(usize, u8),
    Delete(usize),
    Truncate(usize),
}

#[derive(Clone, Debug)]
struct Case {
    messages: Vec<usize>,
    edits: Vec<Edit>,
    cuts: Vec<usize>,
}

#[test]
fn every_single_byte_mutation_decodes_identically_across_readers_and_partitions() {
    let messages = (0..corpus().len()).collect::<Vec<_>>();
    let length = stream(&Case {
        messages: messages.clone(),
        edits: Vec::new(),
        cuts: Vec::new(),
    })
    .len();
    let every_byte = (1..length).collect::<Vec<_>>();
    for position in 0..length {
        let flips = (0..8).map(|bit| Edit::Xor(position, 1 << bit));
        let values = BOUNDARY_BYTES
            .iter()
            .map(|value| Edit::Set(position, *value));
        for edit in flips.chain(values) {
            run(
                Case {
                    messages: messages.clone(),
                    edits: vec![edit],
                    cuts: every_byte.clone(),
                },
                "single-byte sweep",
            );
        }
    }
}

#[test]
fn seeded_streams_decode_identically_across_readers_and_partitions() {
    let seed = seed();
    let corpus = corpus().len();
    for case in 0..RANDOM_CASES {
        let mut random = SplitMix64(seed ^ case);
        let messages = (0..=random.below(12))
            .map(|_| random.below(corpus))
            .collect::<Vec<_>>();
        // A quarter of the cases stay unmutated so pure partition equivalence is always sampled.
        let edits = if random.below(4) == 0 {
            Vec::new()
        } else {
            let length = stream(&Case {
                messages: messages.clone(),
                edits: Vec::new(),
                cuts: Vec::new(),
            })
            .len();
            (0..=random.below(3))
                .map(|_| random_edit(&mut random, length))
                .collect()
        };
        let mut case_value = Case {
            messages,
            edits,
            cuts: Vec::new(),
        };
        let length = stream(&case_value).len();
        let mut cuts = (0..random.below(length + 1))
            .map(|_| random.below(length + 1))
            .collect::<Vec<_>>();
        cuts.sort_unstable();
        cuts.dedup();
        case_value.cuts = cuts;
        run(case_value, &format!("seed {seed:#018x}, case {case}"));
    }
}

/// Every tag value and record kind appears once, so mutation reaches each decoder branch.
fn corpus() -> Vec<MultiplexMessage> {
    let stream_id = MultiplexStreamId::new(3).unwrap();
    let lane_id = MultiplexLaneId::new(2).unwrap();
    let mut messages = [
        StreamMode::Request,
        StreamMode::State,
        StreamMode::Events,
        StreamMode::Invalidations,
    ]
    .into_iter()
    .map(|mode| MultiplexMessage::StreamOpen {
        stream_id,
        lane_id,
        mode,
        request: b"OBWP request".to_vec(),
    })
    .collect::<Vec<_>>();
    messages.extend(
        [
            StreamFrameKind::Result,
            StreamFrameKind::Snapshot,
            StreamFrameKind::Update,
        ]
        .into_iter()
        .map(|kind| MultiplexMessage::StreamFrame {
            stream_id,
            sequence: NonZeroU64::new(2).unwrap(),
            kind,
            record: vec![0, 255],
        }),
    );
    messages.extend(
        [
            StreamCloseReason::Complete,
            StreamCloseReason::Cancelled,
            StreamCloseReason::Unsupported,
            StreamCloseReason::Capacity,
            StreamCloseReason::InvalidRequest,
        ]
        .into_iter()
        .map(|reason| MultiplexMessage::StreamClose { stream_id, reason }),
    );
    messages.extend(
        [
            MultiplexDisconnectReason::SlowConsumer,
            MultiplexDisconnectReason::Capacity,
            MultiplexDisconnectReason::InvalidStream,
            MultiplexDisconnectReason::Unavailable,
        ]
        .into_iter()
        .map(|reason| MultiplexMessage::Disconnect { reason }),
    );
    messages.extend([
        MultiplexMessage::Gap {
            stream_id,
            through: NonZeroU64::MIN,
        },
        MultiplexMessage::Resnapshot { stream_id },
        MultiplexMessage::LaneOpen { lane_id },
        MultiplexMessage::LaneClose { lane_id },
    ]);
    messages
}

fn run(case: Case, origin: &str) {
    if let Err(failure) = check(&case) {
        let (case, failure) = shrink(case, failure);
        panic!(
            "multiplex stream ({origin}): {failure}\nminimal case: {case:?}\nstream: {:02x?}",
            stream(&case)
        );
    }
}

fn check(case: &Case) -> Result<(), String> {
    let bytes = stream(case);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        (
            blocking(&bytes),
            incremental(&bytes, &[]),
            incremental(&bytes, &case.cuts),
        )
    }));
    let Ok((blocking, whole, partitioned)) = outcome else {
        return Err("a decoder panicked".to_owned());
    };
    if blocking != whole || blocking != partitioned {
        return Err(format!(
            "decoders disagree: blocking {blocking:?}, whole {whole:?}, partitioned {partitioned:?}"
        ));
    }
    let (messages, ending) = blocking;
    let consumed = messages
        .iter()
        .flat_map(|message| encode_multiplex_message(message).unwrap())
        .collect::<Vec<_>>();
    if !bytes.starts_with(&consumed) || (ending == Ending::Clean && consumed != bytes) {
        return Err("an accepted record does not re-encode to the bytes it consumed".to_owned());
    }
    if case.edits.is_empty() {
        let expected = case
            .messages
            .iter()
            .map(|index| corpus()[*index].clone())
            .collect::<Vec<_>>();
        if messages != expected || ending != Ending::Clean {
            return Err("an unmutated stream did not decode to its messages".to_owned());
        }
    }
    Ok(())
}

/// Greedily removes cuts, edits and messages while the case still fails.
fn shrink(mut case: Case, mut failure: String) -> (Case, String) {
    loop {
        let mut reduced = false;
        for part in 0..3 {
            let mut index = 0;
            while index < part_length(&case, part) {
                let mut candidate = case.clone();
                match part {
                    0 => {
                        candidate.cuts.remove(index);
                    }
                    1 => {
                        candidate.edits.remove(index);
                    }
                    _ => {
                        candidate.messages.remove(index);
                    }
                }
                match check(&candidate) {
                    Err(message) => {
                        case = candidate;
                        failure = message;
                        reduced = true;
                    }
                    Ok(()) => index += 1,
                }
            }
        }
        if !reduced {
            return (case, failure);
        }
    }
}

fn part_length(case: &Case, part: usize) -> usize {
    match part {
        0 => case.cuts.len(),
        1 => case.edits.len(),
        _ => case.messages.len(),
    }
}

fn stream(case: &Case) -> Vec<u8> {
    let corpus = corpus();
    let mut bytes = case
        .messages
        .iter()
        .flat_map(|index| encode_multiplex_message(&corpus[*index]).unwrap())
        .collect::<Vec<_>>();
    for edit in &case.edits {
        let length = bytes.len();
        match *edit {
            Edit::Set(position, value) => {
                if let Some(byte) = bytes.get_mut(position % length.max(1)) {
                    *byte = value;
                }
            }
            Edit::Xor(position, mask) => {
                if let Some(byte) = bytes.get_mut(position % length.max(1)) {
                    *byte ^= mask;
                }
            }
            Edit::Insert(position, value) => bytes.insert(position % (length + 1), value),
            Edit::Delete(position) => {
                if length > 0 {
                    bytes.remove(position % length);
                }
            }
            Edit::Truncate(retained) => bytes.truncate(retained % (length + 1)),
        }
    }
    bytes
}

fn blocking(bytes: &[u8]) -> (Vec<MultiplexMessage>, Ending) {
    let mut reader = Cursor::new(bytes);
    let mut messages = Vec::new();
    loop {
        if usize::try_from(reader.position()).unwrap() == bytes.len() {
            return (messages, Ending::Clean);
        }
        match read_multiplex_message(&mut reader) {
            Ok(message) => messages.push(message),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return (messages, Ending::Incomplete);
            }
            Err(_) => return (messages, Ending::Invalid),
        }
    }
}

fn incremental(bytes: &[u8], cuts: &[usize]) -> (Vec<MultiplexMessage>, Ending) {
    let mut decoder = MultiplexDecoder::default();
    let mut messages = Vec::new();
    let mut start = 0;
    for end in cuts
        .iter()
        .copied()
        .filter(|cut| *cut <= bytes.len())
        .chain([bytes.len()])
    {
        if decoder.push(&bytes[start..end]).is_err() {
            return (messages, Ending::Invalid);
        }
        start = end;
        loop {
            match decoder.next_message() {
                Ok(Some(message)) => messages.push(message),
                Ok(None) => break,
                Err(_) => return (messages, Ending::Invalid),
            }
        }
    }
    let ending = if decoder.is_empty() {
        Ending::Clean
    } else {
        Ending::Incomplete
    };
    (messages, ending)
}

fn random_edit(random: &mut SplitMix64, length: usize) -> Edit {
    let position = random.below(length);
    match random.below(9) {
        0..=2 => Edit::Set(position, random.byte()),
        3..=5 => Edit::Xor(position, 1 << random.below(8)),
        6 => Edit::Insert(position, random.byte()),
        7 => Edit::Delete(position),
        _ => Edit::Truncate(random.below(length + 1)),
    }
}

fn seed() -> u64 {
    std::env::var("ORCHBEE_PROTOCOL_MUTATION_SEED")
        .ok()
        .and_then(|value| {
            let value = value.trim();
            value.strip_prefix("0x").map_or_else(
                || value.parse().ok(),
                |hex| u64::from_str_radix(hex, 16).ok(),
            )
        })
        .unwrap_or(DEFAULT_SEED)
}

/// Deterministic SplitMix64 stream; reproducibility needs no external generator.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound.max(1) as u64) as usize
    }

    fn byte(&mut self) -> u8 {
        if self.below(2) == 0 {
            BOUNDARY_BYTES[self.below(BOUNDARY_BYTES.len())]
        } else {
            self.next().to_le_bytes()[0]
        }
    }
}
