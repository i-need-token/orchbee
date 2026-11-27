//! Seeded evidence that the status reader yields the same status from any split of the output, for
//! paths holding spaces, newlines and any byte but NUL, renames, copies and unmerged entries, and
//! that it stops exactly past its limit. `ORCHBEE_GIT_STATUS_SEED` explores another seed; a failure
//! prints its seed, case and cuts.
use super::*;

const DEFAULT_SEED: u64 = 0x5ca1_ab1e_0000_0215;
const CASES: u64 = 1500;

#[test]
fn every_split_of_generated_output_reads_as_the_written_status() {
    let seed = seed();
    for case in 0..CASES {
        let mut random = SplitMix64(seed ^ case);
        let status = generated(&mut random);
        let bytes = written(&status);
        let limit = random.below(status.entries.len() + 3);
        let cuts = cuts(&mut random, bytes.len());
        let expected = Status {
            branch: status.branch.clone(),
            entries: status.entries.iter().take(limit).cloned().collect(),
            truncated: status.entries.len() > limit,
        };
        let context = format!("seed {seed:#x} case {case} limit {limit} cuts {cuts:?}");
        assert_eq!(
            read(&bytes, &cuts, limit),
            Ok(expected.clone()),
            "{context}"
        );
        assert_eq!(read(&bytes, &[], limit), Ok(expected), "{context}");
    }
}

#[test]
fn output_that_ends_inside_a_record_is_malformed_unless_the_limit_was_passed() {
    let status = Status {
        branch: branch(),
        entries: vec![
            changed(b".M", b"a b\nc"),
            renamed(b"R.", b"new name", b"old\nname"),
        ],
        truncated: false,
    };
    let bytes = written(&status);
    for end in 0..bytes.len() {
        let partial = bytes.get(..end).unwrap_or_default();
        let whole_records = partial.last().is_none_or(|byte| *byte == 0)
            // The rename's two records end together: cutting between them leaves it open.
            && !partial.ends_with(b"new name\0");
        let result = read(partial, &[], usize::MAX);
        assert_eq!(result.is_ok(), whole_records, "end {end}");
    }
    // An entry past the limit ends the read, so whatever follows it may be cut anywhere.
    let mut longer = bytes.clone();
    longer.extend_from_slice(b"? untracked\0");
    longer.truncate(longer.len() - 3);
    assert_eq!(
        read(&longer, &[], 1),
        Ok(Status {
            branch: branch(),
            entries: vec![changed(b".M", b"a b\nc")],
            truncated: true,
        })
    );
}

#[test]
fn records_that_are_not_porcelain_v2_are_malformed() {
    for record in [
        &b"1 .M N... 100644 100644 100644 0000 0000\0"[..],
        b"1 .MX N... 100644 100644 100644 0000 0000 path\0",
        b"1 .M N.. 100644 100644 100644 0000 0000 path\0",
        b"? \0",
        b"?path\0",
        b"x unknown\0",
        b"# branch.ab +1\0",
        b"# branch.ab 1 -2\0",
        b"2 R. N... 100644 100644 100644 0000 0000 R100 path\0\0",
    ] {
        assert_eq!(read(record, &[], usize::MAX), Err(Malformed), "{record:?}");
    }
    assert_eq!(
        read(&vec![b'?'; RECORD_LIMIT + 1], &[], usize::MAX),
        Err(Malformed)
    );
}

fn read(bytes: &[u8], cuts: &[usize], limit: usize) -> Result<Status, Malformed> {
    let mut reader = StatusReader::new(limit);
    let mut start = 0;
    for end in cuts.iter().copied().chain([bytes.len()]) {
        if reader.read(bytes.get(start..end).unwrap_or_default())? == Progress::Full {
            break;
        }
        start = end;
    }
    reader.finish()
}

fn cuts(random: &mut SplitMix64, length: usize) -> Vec<usize> {
    let mut cuts = match random.below(4) {
        // Every byte alone exercises each record boundary.
        0 => (1..length).collect(),
        _ => (0..random.below(16))
            .map(|_| random.below(length + 1))
            .collect::<Vec<_>>(),
    };
    cuts.sort_unstable();
    cuts
}

fn generated(random: &mut SplitMix64) -> Status {
    let upstream = random.below(2) == 0;
    let branch = Branch {
        commit: (random.below(4) != 0).then(|| hex(random, 40)),
        head: (random.below(4) != 0).then(|| format!("topic/{}", random.below(1000))),
        upstream: upstream.then(|| format!("origin/{}", random.below(1000))),
        divergence: (upstream && random.below(3) != 0)
            .then(|| (random.below(1 << 20) as u64, random.below(3) as u64)),
    };
    let entries = (0..random.below(24))
        .map(|_| {
            let path = path(random);
            match random.below(6) {
                0 => changed(
                    pick(
                        random,
                        &[*b".M", *b"M.", *b"MM", *b"A.", *b"D.", *b".D", *b".T"],
                    ),
                    &path,
                ),
                1 => renamed(
                    pick(random, &[*b"R.", *b"RM", *b"C.", *b"CD"]),
                    &path,
                    &self::path(random),
                ),
                2 => Entry {
                    kind: EntryKind::Unmerged,
                    ..changed(
                        pick(
                            random,
                            &[*b"UU", *b"AA", *b"DD", *b"AU", *b"UD", *b"UA", *b"DU"],
                        ),
                        &path,
                    )
                },
                3 => Entry {
                    submodule: true,
                    ..changed(b".M", &path)
                },
                _ => Entry {
                    kind: EntryKind::Untracked,
                    x: b'?',
                    y: b'?',
                    submodule: false,
                    path,
                    original: None,
                },
            }
        })
        .collect();
    Status {
        branch,
        entries,
        truncated: false,
    }
}

/// The output Git writes for `status`, in its header and field layout.
fn written(status: &Status) -> Vec<u8> {
    let mut bytes = Vec::new();
    let branch = &status.branch;
    let mut header = |text: String| {
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
    };
    header(format!(
        "# branch.oid {}",
        branch.commit.as_deref().unwrap_or("(initial)")
    ));
    header(format!(
        "# branch.head {}",
        branch.head.as_deref().unwrap_or("(detached)")
    ));
    if let Some(upstream) = &branch.upstream {
        header(format!("# branch.upstream {upstream}"));
    }
    if let Some((ahead, behind)) = branch.divergence {
        header(format!("# branch.ab +{ahead} -{behind}"));
    }
    let object = "0123456789abcdef0123456789abcdef01234567";
    for entry in &status.entries {
        let submodule = if entry.submodule { "SC.." } else { "N..." };
        let letters = [entry.x, entry.y];
        let letters = String::from_utf8_lossy(&letters);
        let fields = match entry.kind {
            EntryKind::Changed => {
                format!("1 {letters} {submodule} 100644 100644 100644 {object} {object} ")
            }
            EntryKind::Renamed => format!(
                "2 {letters} {submodule} 100644 100644 100644 {object} {object} {}87 ",
                char::from(entry.x)
            ),
            EntryKind::Unmerged => format!(
                "u {letters} {submodule} 100644 100644 100644 100644 {object} {object} {object} "
            ),
            EntryKind::Untracked => "? ".to_owned(),
            EntryKind::Ignored => "! ".to_owned(),
        };
        bytes.extend_from_slice(fields.as_bytes());
        bytes.extend_from_slice(&entry.path);
        bytes.push(0);
        if let Some(original) = &entry.original {
            bytes.extend_from_slice(original);
            bytes.push(0);
        }
    }
    bytes
}

fn branch() -> Branch {
    Branch {
        commit: Some("0123456789abcdef0123456789abcdef01234567".to_owned()),
        head: Some("main".to_owned()),
        upstream: Some("origin/main".to_owned()),
        divergence: Some((2, 0)),
    }
}

fn changed(letters: &[u8; 2], path: &[u8]) -> Entry {
    Entry {
        kind: EntryKind::Changed,
        x: letters[0],
        y: letters[1],
        submodule: false,
        path: path.to_vec(),
        original: None,
    }
}

fn renamed(letters: &[u8; 2], path: &[u8], original: &[u8]) -> Entry {
    Entry {
        kind: EntryKind::Renamed,
        original: Some(original.to_vec()),
        ..changed(letters, path)
    }
}

/// A path of one to forty bytes from every byte but NUL, weighted toward the separators, spaces and
/// line breaks that a record's layout could confuse.
fn path(random: &mut SplitMix64) -> Vec<u8> {
    (0..=random.below(40))
        .map(|_| match random.below(6) {
            0 => *pick(random, b" \n\r/\t"),
            1 => 1 + random.below(255) as u8,
            _ => b'a' + random.below(26) as u8,
        })
        .collect()
}

fn hex(random: &mut SplitMix64, length: usize) -> String {
    (0..length)
        .map(|_| char::from(b"0123456789abcdef"[random.below(16)]))
        .collect()
}

fn pick<'a, T>(random: &mut SplitMix64, values: &'a [T]) -> &'a T {
    &values[random.below(values.len())]
}

fn seed() -> u64 {
    std::env::var("ORCHBEE_GIT_STATUS_SEED")
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
}
