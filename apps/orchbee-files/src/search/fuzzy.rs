//! VS Code's fuzzy scorer at ef5ca45 (`base/common/fuzzyScorer.ts`), which ranks Quick Open's
//! files: `prepareQuery`, `scoreFuzzy`, the item score over a file's label, description and path,
//! and `compareItemsByFuzzyScore` with its fallbacks, over UTF-16 code units, so scores and match
//! ranges equal VS Code's. Only the last fallback differs: VS Code orders otherwise equal names
//! with the ICU collator, which this approximates with the root collation's order of
//! punctuation, digits and letters, comparing digit runs by value.

use std::cmp::Ordering;

use super::js::{self, BACKSLASH, SEPARATOR, SLASH};

const PATH_IDENTITY_SCORE: i64 = 1 << 18;
const LABEL_PREFIX_SCORE_THRESHOLD: i64 = 1 << 17;
const LABEL_SCORE_THRESHOLD: i64 = 1 << 16;

/// A match range in UTF-16 code units, end exclusive.
pub(super) type Span = (usize, usize);

/// One space-separated piece of a query, or the whole query.
pub(super) struct Piece {
    path_normalized: Vec<u16>,
    normalized: Vec<u16>,
    normalized_lower: Vec<u16>,
    expect_contiguous: bool,
}

/// `IPreparedQuery`.
pub(super) struct Query {
    whole: Piece,
    values: Vec<Piece>,
    contains_path_separator: bool,
}

impl Query {
    /// `prepareQuery`.
    pub(super) fn new(original: &str) -> Self {
        let whole = piece(original);
        let contains_path_separator = whole.path_normalized.contains(&SEPARATOR);
        let split: Vec<&str> = original.split(' ').collect();
        let values = if split.len() > 1 {
            split
                .into_iter()
                .map(piece)
                .filter(|piece| !piece.normalized.is_empty())
                .collect()
        } else {
            Vec::new()
        };
        Self {
            whole,
            values,
            contains_path_separator,
        }
    }

    /// The lowercased normalized query, which a candidate path must contain in order.
    pub(super) fn normalized_lower(&self) -> &[u16] {
        &self.whole.normalized_lower
    }
}

fn piece(original: &str) -> Piece {
    let units = js::units(original);
    let path_normalized: Vec<u16> = units
        .iter()
        .map(|unit| {
            if (SEPARATOR == BACKSLASH && *unit == SLASH)
                || (SEPARATOR == SLASH && *unit == BACKSLASH)
            {
                SEPARATOR
            } else {
                *unit
            }
        })
        .collect();
    let mut normalized: Vec<u16> = path_normalized
        .iter()
        .copied()
        .filter(|unit| !matches!(unit, 0x2A | 0x2026 | 0x22) && !js::is_space(*unit))
        .collect();
    if normalized.len() > 1 && normalized.last() == Some(&u16::from(b'#')) {
        normalized.pop();
    }
    Piece {
        normalized_lower: js::to_lower(&normalized),
        path_normalized,
        normalized,
        expect_contiguous: units.first() == Some(&0x22) && units.last() == Some(&0x22),
    }
}

/// VS Code's `fuzzyContains`: whether the target, `parts` joined, lowercased, holds `query`'s
/// units in order.
pub(super) fn contains(parts: &[&[u16]], query_lower: &[u16]) -> bool {
    let length: usize = parts.iter().map(|part| part.len()).sum();
    if length == 0 || query_lower.is_empty() || length < query_lower.len() {
        return false;
    }
    // Most paths are ASCII, whose lowercase keeps every unit in place.
    if parts
        .iter()
        .all(|part| part.iter().all(|unit| *unit < 0x80))
    {
        let lowered = parts
            .iter()
            .flat_map(|part| part.iter().map(|unit| js::ascii_lower(*unit)));
        return in_order(lowered, query_lower);
    }
    in_order(js::to_lower(&parts.concat()).into_iter(), query_lower)
}

/// Whether `lowered` holds `query_lower`'s units in order, each after the one before.
fn in_order(mut lowered: impl Iterator<Item = u16>, query_lower: &[u16]) -> bool {
    query_lower
        .iter()
        .all(|unit| lowered.any(|candidate| candidate == *unit))
}

/// The buffers one scorer reuses across targets.
#[derive(Default)]
pub(super) struct Scratch {
    scores: Vec<i64>,
    matches: Vec<u32>,
}

/// `scoreFuzzy`: the score of `query` in `target` and the positions it matched.
fn score_fuzzy(
    scratch: &mut Scratch,
    target: &[u16],
    query: &[u16],
    query_lower: &[u16],
    allow_non_contiguous: bool,
) -> (i64, Vec<usize>) {
    if target.is_empty() || query.is_empty() || target.len() < query.len() {
        return (0, Vec::new());
    }
    let target_lower = js::to_lower(target);
    // A unit past the end of a lowercased string is JavaScript's `undefined`, equal to nothing.
    let equal = |a: Option<&u16>, b: Option<&u16>| match (a, b) {
        (Some(a), Some(b)) => a == b || (is_separator(*a) && is_separator(*b)),
        _ => false,
    };
    let mut query_index = 0;
    for target_index in 0..target.len() {
        if query_index >= query.len() {
            break;
        }
        if equal(query_lower.get(query_index), target_lower.get(target_index)) {
            query_index += 1;
        }
    }
    if query_index < query.len() {
        return (0, Vec::new());
    }
    let (query_length, target_length) = (query.len(), target.len());
    let cells = query_length * target_length;
    scratch.scores.clear();
    scratch.scores.resize(cells, 0);
    scratch.matches.clear();
    scratch.matches.resize(cells, 0);
    let (scores, matches) = (&mut scratch.scores, &mut scratch.matches);
    for (query_index, &query_unit) in query.iter().enumerate() {
        let offset = query_index * target_length;
        let query_lower_unit = query_lower.get(query_index).copied();
        let query_is_separator = query_lower_unit.is_some_and(is_separator);
        for target_index in 0..target_length {
            let current = offset + target_index;
            let left_score = if target_index > 0 {
                scores[current - 1]
            } else {
                0
            };
            scores[current] = left_score;
            matches[current] = 0;
            let target_lower_unit = target_lower.get(target_index).copied();
            let same =
                matches!((query_lower_unit, target_lower_unit), (Some(a), Some(b)) if a == b);
            if !same && !(query_is_separator && target_lower_unit.is_some_and(is_separator)) {
                continue;
            }
            let diagonal =
                (query_index > 0 && target_index > 0).then(|| current - target_length - 1);
            let diagonal_score = diagonal.map_or(0, |diagonal| scores[diagonal]);
            if query_index > 0 && diagonal_score == 0 {
                continue;
            }
            let sequence = diagonal.map_or(0, |diagonal| matches[diagonal]);
            let score = diagonal_score + char_score(query_unit, target, target_index, sequence);
            if score >= left_score
                && (allow_non_contiguous
                    || query_index > 0
                    || target_lower
                        .get(target_index..)
                        .is_some_and(|rest| rest.starts_with(query_lower)))
            {
                matches[current] = sequence + 1;
                scores[current] = score;
            }
        }
    }
    let mut positions = Vec::new();
    let (mut query_index, mut target_index) = (query_length, target_length);
    while query_index > 0 && target_index > 0 {
        if matches[(query_index - 1) * target_length + target_index - 1] == 0 {
            target_index -= 1;
        } else {
            positions.push(target_index - 1);
            query_index -= 1;
            target_index -= 1;
        }
    }
    positions.reverse();
    (scores[cells - 1], positions)
}

fn is_separator(unit: u16) -> bool {
    unit == SLASH || unit == BACKSLASH
}

/// `computeCharScore`.
fn char_score(query_unit: u16, target: &[u16], target_index: usize, sequence: u32) -> i64 {
    let sequence = i64::from(sequence);
    let mut score = 1;
    if sequence > 0 {
        score += sequence.min(3) * 6 + (sequence - 3).max(0) * 3;
    }
    if query_unit == target[target_index] {
        score += 1;
    }
    if target_index == 0 {
        score += 8;
    } else {
        let separator_bonus = match target[target_index - 1] {
            0x2F | 0x5C => 5,
            0x5F | 0x2D | 0x2E | 0x20 | 0x27 | 0x22 | 0x3A => 4,
            _ => 0,
        };
        if separator_bonus > 0 {
            score += separator_bonus;
        } else if js::is_ascii_upper(target[target_index]) && sequence == 0 {
            score += 2;
        }
    }
    score
}

/// A file as Quick Open scores it: its name, the folder above it relative to the searched folder
/// (`.` at the top), and its relative path.
pub(super) struct Item<'a> {
    pub(super) label: &'a [u16],
    pub(super) description: &'a [u16],
    pub(super) path: &'a [u16],
}

/// `IItemScore`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ItemScore {
    pub(super) score: i64,
    pub(super) label: Vec<Span>,
    pub(super) description: Vec<Span>,
}

/// `scoreItemFuzzy` with non-contiguous matches allowed, as Quick Open's file search sorts.
pub(super) fn score_item(scratch: &mut Scratch, item: &Item<'_>, query: &Query) -> ItemScore {
    if query.whole.normalized.is_empty() || item.label.is_empty() {
        return ItemScore::default();
    }
    let prefer_label = !query.contains_path_separator;
    let identical = if cfg!(target_os = "linux") {
        query.whole.path_normalized == item.path
    } else {
        js::equals_ignore_case(&query.whole.path_normalized, item.path)
    };
    if identical {
        return ItemScore {
            score: PATH_IDENTITY_SCORE,
            label: vec![(0, item.label.len())],
            description: if item.description.is_empty() {
                Vec::new()
            } else {
                vec![(0, item.description.len())]
            },
        };
    }
    if query.values.len() > 1 {
        let mut total = ItemScore::default();
        for piece in &query.values {
            let score = score_piece(scratch, item, piece, prefer_label);
            if score.score == 0 {
                return ItemScore::default();
            }
            total.score += score.score;
            total.label.extend(score.label);
            total.description.extend(score.description);
        }
        total.label = normalize(total.label);
        total.description = normalize(total.description);
        return total;
    }
    score_piece(scratch, item, &query.whole, prefer_label)
}

/// `doScoreItemFuzzySingle`.
fn score_piece(
    scratch: &mut Scratch,
    item: &Item<'_>,
    piece: &Piece,
    prefer_label: bool,
) -> ItemScore {
    let allow = !piece.expect_contiguous;
    if prefer_label || item.description.is_empty() {
        let (score, positions) = score_fuzzy(
            scratch,
            item.label,
            &piece.normalized,
            &piece.normalized_lower,
            allow,
        );
        if score != 0 {
            let prefix = item.label.len() >= piece.normalized.len()
                && js::starts_with_ignore_case(item.label, &piece.normalized);
            let base = if prefix {
                let ratio = piece.normalized.len() as f64 / item.label.len() as f64;
                LABEL_PREFIX_SCORE_THRESHOLD + (ratio * 100.0).round() as i64
            } else {
                LABEL_SCORE_THRESHOLD
            };
            return ItemScore {
                score: base + score,
                label: if prefix {
                    vec![(0, piece.normalized.len())]
                } else {
                    spans(&positions)
                },
                description: Vec::new(),
            };
        }
    }
    if item.description.is_empty() {
        return ItemScore::default();
    }
    let mut target = item.description.to_vec();
    target.push(SEPARATOR);
    let prefix_length = target.len();
    target.extend_from_slice(item.label);
    let (score, positions) = score_fuzzy(
        scratch,
        &target,
        &piece.normalized,
        &piece.normalized_lower,
        allow,
    );
    if score == 0 {
        return ItemScore::default();
    }
    let mut result = ItemScore {
        score,
        ..ItemScore::default()
    };
    for (start, end) in spans(&positions) {
        if start < prefix_length && end > prefix_length {
            result.label.push((0, end - prefix_length));
            result.description.push((start, prefix_length));
        } else if start >= prefix_length {
            result
                .label
                .push((start - prefix_length, end - prefix_length));
        } else {
            result.description.push((start, end));
        }
    }
    result
}

/// `createMatches`: consecutive positions joined into ranges.
fn spans(positions: &[usize]) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for position in positions {
        match spans.last_mut() {
            Some(last) if last.1 == *position => last.1 += 1,
            _ => spans.push((*position, position + 1)),
        }
    }
    spans
}

/// `normalizeMatches`: sorted by start, touching or overlapping ranges merged.
fn normalize(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by_key(|span| span.0);
    let mut merged: Vec<Span> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if !(last.1 < span.0 || span.1 < last.0) => {
                last.0 = last.0.min(span.0);
                last.1 = last.1.max(span.1);
            }
            _ => merged.push(span),
        }
    }
    merged
}

/// `compareItemScores`: the order of two scored items, the better first.
pub(super) fn compare(
    a: (&Item<'_>, &ItemScore),
    b: (&Item<'_>, &ItemScore),
    query: &Query,
) -> Ordering {
    let ((item_a, score_a), (item_b, score_b)) = (a, b);
    let (value_a, value_b) = (score_a.score, score_b.score);
    if (value_a == PATH_IDENTITY_SCORE || value_b == PATH_IDENTITY_SCORE) && value_a != value_b {
        return if value_a == PATH_IDENTITY_SCORE {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if value_a > LABEL_SCORE_THRESHOLD || value_b > LABEL_SCORE_THRESHOLD {
        if value_a != value_b {
            return value_b.cmp(&value_a);
        }
        if value_a < LABEL_PREFIX_SCORE_THRESHOLD && value_b < LABEL_PREFIX_SCORE_THRESHOLD {
            let by_length = compare_match_length(&score_a.label, &score_b.label);
            if by_length != Ordering::Equal {
                return by_length;
            }
        }
        if item_a.label.len() != item_b.label.len() {
            return item_a.label.len().cmp(&item_b.label.len());
        }
    }
    if value_a != value_b {
        return value_b.cmp(&value_a);
    }
    match (score_a.label.is_empty(), score_b.label.is_empty()) {
        (false, true) => return Ordering::Less,
        (true, false) => return Ordering::Greater,
        _ => {}
    }
    let distance_a = match_distance(item_a, score_a);
    let distance_b = match_distance(item_b, score_b);
    if distance_a != 0 && distance_b != 0 && distance_a != distance_b {
        return if distance_b > distance_a {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    fallback(item_a, item_b, query)
}

/// `computeLabelAndDescriptionMatchDistance`.
fn match_distance(item: &Item<'_>, score: &ItemScore) -> i64 {
    let to_i64 = |value: usize| i64::try_from(value).unwrap_or(i64::MAX);
    let start = match (score.description.first(), score.label.first()) {
        (Some(first), _) | (None, Some(first)) => to_i64(first.0),
        (None, None) => -1,
    };
    let end = match (score.label.last(), score.description.last()) {
        (Some(last), description) => {
            let mut end = to_i64(last.1);
            if description.is_some() && !item.description.is_empty() {
                end += to_i64(item.description.len());
            }
            end
        }
        (None, Some(last)) => to_i64(last.1),
        (None, None) => -1,
    };
    end - start
}

/// `compareByMatchLength`.
fn compare_match_length(a: &[Span], b: &[Span]) -> Ordering {
    match (a.first().zip(a.last()), b.first().zip(b.last())) {
        (None, None) => Ordering::Equal,
        (_, None) => Ordering::Less,
        (None, _) => Ordering::Greater,
        (Some((first_a, last_a)), Some((first_b, last_b))) => {
            (last_a.1 - first_a.0).cmp(&(last_b.1 - first_b.0))
        }
    }
}

/// `fallbackCompare`.
fn fallback(a: &Item<'_>, b: &Item<'_>, query: &Query) -> Ordering {
    let length_a = a.label.len() + a.description.len();
    let length_b = b.label.len() + b.description.len();
    if length_a != length_b {
        return length_a.cmp(&length_b);
    }
    let paths = !a.path.is_empty() && !b.path.is_empty();
    if paths && a.path.len() != b.path.len() {
        return a.path.len().cmp(&b.path.len());
    }
    let look_for = &query.whole.normalized;
    if a.label != b.label {
        return compare_anything(a.label, b.label, look_for);
    }
    if !a.description.is_empty() && !b.description.is_empty() && a.description != b.description {
        return compare_anything(a.description, b.description, look_for);
    }
    if paths && a.path != b.path {
        return compare_anything(a.path, b.path, look_for);
    }
    Ordering::Equal
}

/// `compareAnything`.
fn compare_anything(one: &[u16], other: &[u16], look_for: &[u16]) -> Ordering {
    let (a, b) = (js::to_lower(one), js::to_lower(other));
    let (prefix_a, prefix_b) = (a.starts_with(look_for), b.starts_with(look_for));
    if prefix_a != prefix_b {
        return if prefix_a {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    if prefix_a && a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    let (suffix_a, suffix_b) = (a.ends_with(look_for), b.ends_with(look_for));
    if suffix_a != suffix_b {
        return if suffix_a {
            Ordering::Less
        } else {
            Ordering::Greater
        };
    }
    match collate(&a, &b) {
        Ordering::Equal if a != b => a.cmp(&b),
        order => order,
    }
}

/// The ICU root collation at base strength with numeric ordering, approximated: whitespace,
/// then punctuation and symbols in the root order, then digit runs by value, then letters
/// without case or accent, then everything else by code unit.
fn collate(a: &[u16], b: &[u16]) -> Ordering {
    let (mut i, mut j) = (0, 0);
    loop {
        let (Some(&x), Some(&y)) = (a.get(i), b.get(j)) else {
            return (a.len() - i.min(a.len())).cmp(&(b.len() - j.min(b.len())));
        };
        if js::is_ascii_digit(x) && js::is_ascii_digit(y) {
            let end_a = i + a[i..]
                .iter()
                .take_while(|unit| js::is_ascii_digit(**unit))
                .count();
            let end_b = j + b[j..]
                .iter()
                .take_while(|unit| js::is_ascii_digit(**unit))
                .count();
            let digits = |run: &[u16]| -> Vec<u16> {
                let start = run
                    .iter()
                    .position(|unit| *unit != u16::from(b'0'))
                    .unwrap_or(run.len());
                run[start..].to_vec()
            };
            let (run_a, run_b) = (digits(&a[i..end_a]), digits(&b[j..end_b]));
            let order = run_a
                .len()
                .cmp(&run_b.len())
                .then_with(|| run_a.cmp(&run_b));
            if order != Ordering::Equal {
                return order;
            }
            (i, j) = (end_a, end_b);
            continue;
        }
        let order = weight(x).cmp(&weight(y));
        if order != Ordering::Equal {
            return order;
        }
        i += 1;
        j += 1;
    }
}

/// A unit's primary group and rank in the approximated root collation.
fn weight(unit: u16) -> (u8, u32) {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if js::is_space(unit) {
        return (0, u32::from(unit));
    }
    if let Some(rank) = char::from_u32(u32::from(unit))
        .and_then(|ch| PUNCTUATION.find(ch))
        .and_then(|rank| u32::try_from(rank).ok())
    {
        return (1, rank);
    }
    if js::is_ascii_digit(unit) {
        return (2, u32::from(unit));
    }
    if let Some(base) = latin_base(unit) {
        return (3, u32::from(base));
    }
    (4, u32::from(unit))
}

/// The base letter of an ASCII or Latin-1 letter, lowercase.
fn latin_base(unit: u16) -> Option<u8> {
    let byte = u8::try_from(unit).ok()?;
    if byte.is_ascii_alphabetic() {
        return Some(byte.to_ascii_lowercase());
    }
    let base = match byte {
        0xC0..=0xC5 | 0xE0..=0xE5 => b'a',
        0xC7 | 0xE7 => b'c',
        0xC8..=0xCB | 0xE8..=0xEB => b'e',
        0xCC..=0xCF | 0xEC..=0xEF => b'i',
        0xD1 | 0xF1 => b'n',
        0xD2..=0xD6 | 0xD8 | 0xF2..=0xF6 | 0xF8 => b'o',
        0xD9..=0xDC | 0xF9..=0xFC => b'u',
        0xDD | 0xFD | 0xFF => b'y',
        _ => return None,
    };
    Some(base)
}
