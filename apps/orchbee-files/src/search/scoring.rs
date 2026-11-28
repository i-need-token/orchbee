//! A Quick Open query over a file list, answered as VS Code ef5ca45's search process answers one
//! from its cache (`rawSearchService.ts`): the files whose path, after its folder's name when the
//! list has several folders, holds the query's characters in order (`isFilePatternMatch`), then
//! the best `maxResults` of them by `compareItemsByFuzzyScore`, kept as `topAsync` keeps them. A
//! query without characters to look for matches nothing. Results that do not fit one Result
//! record are left out, which reports `limitHit` as more matches do.
//!
//! VS Code's comparison is not a consistent order in every case, so the first matches are sorted
//! with V8's own sort and later ones placed by `topStep`'s halving, which also never panics on such
//! an order as the standard library's sorts may.

use std::cmp::Ordering;

use orchbee_files_protocol::{FilePath, FoundFile, MAXIMUM_REPLY_BYTES, SafeInteger};

use super::{
    array_sort,
    file_list::{File, FileList},
    fuzzy::{self, Item, ItemScore, Query, Scratch, Span},
    js,
};

/// How many files are read between looks at the call's cancellation.
const CHECK_EVERY: usize = 4096;
/// The result without files, at its longest.
const ENVELOPE: usize = r#"{"results":[],"truncated":false,"limitHit":false}"#.len();

/// The answer to one query.
pub(super) struct Found {
    /// The best files, best first.
    pub(super) results: Vec<FoundFile>,
    /// More files matched than were returned.
    pub(super) limit_hit: bool,
}

/// The best `maximum` files of `list` for `query`; `None` once `cancelled` holds.
pub(super) fn find(
    list: &FileList,
    query: &str,
    maximum: usize,
    cancelled: impl Fn() -> bool,
) -> Option<Found> {
    let query = Query::new(query);
    let lower = query.normalized_lower();
    let mut scratch = Scratch::default();
    let mut best: Vec<(usize, ItemScore)> = Vec::with_capacity(maximum.min(512));
    let mut sorted = false;
    let mut matched = 0_usize;
    for index in 0..list.len() {
        if index % CHECK_EVERY == 0 && cancelled() {
            return None;
        }
        let file = list.file(index);
        let named = list.named() && !file.folder.name.is_empty();
        let hit = if named {
            fuzzy::contains(&[&file.folder.name, &[js::SEPARATOR], file.path], lower)
        } else {
            fuzzy::contains(&[file.path], lower)
        };
        if !hit {
            continue;
        }
        matched += 1;
        let score = fuzzy::score_item(&mut scratch, &item(&file), &query);
        // `topAsync`: the first `maximum` matches, sorted, then each later one that is better
        // than the worst kept replaces it.
        if best.len() < maximum {
            best.push((index, score));
            continue;
        }
        if !sorted {
            sort(list, &query, &mut best);
            sorted = true;
        }
        keep(list, &query, &mut best, (index, score));
    }
    if !sorted {
        sort(list, &query, &mut best);
    }
    let mut size = ENVELOPE;
    let mut results = Vec::with_capacity(best.len());
    let mut cut = false;
    for (index, score) in best {
        let Some(found) = found(&list.file(index), &score) else {
            continue;
        };
        let bytes = json_length(&found) + usize::from(!results.is_empty());
        if size + bytes > MAXIMUM_REPLY_BYTES {
            cut = true;
            break;
        }
        size += bytes;
        results.push(found);
    }
    Some(Found {
        results,
        limit_hit: matched > maximum || cut,
    })
}

/// A file as `FileMatchItemAccessor` presents it.
fn item<'a>(file: &File<'a>) -> Item<'a> {
    Item {
        label: file.label,
        description: file.description,
        path: file.path,
    }
}

fn order(
    list: &FileList,
    query: &Query,
    a: &(usize, ItemScore),
    b: &(usize, ItemScore),
) -> Ordering {
    let (file_a, file_b) = (list.file(a.0), list.file(b.0));
    fuzzy::compare((&item(&file_a), &a.1), (&item(&file_b), &b.1), query)
}

/// Sorts `best` as `Array.prototype.sort` does.
fn sort(list: &FileList, query: &Query, best: &mut Vec<(usize, ItemScore)>) {
    let mut positions: Vec<usize> = (0..best.len()).collect();
    array_sort::sort(&mut positions, |a, b| {
        order(list, query, &best[*a], &best[*b])
    });
    let mut unsorted: Vec<Option<(usize, ItemScore)>> =
        std::mem::take(best).into_iter().map(Some).collect();
    best.extend(
        positions
            .into_iter()
            .filter_map(|position| unsorted[position].take()),
    );
}

/// `topStep` for one element: when it is better than the worst kept, the worst goes and the
/// element is placed before the first kept one it is better than.
fn keep(
    list: &FileList,
    query: &Query,
    best: &mut Vec<(usize, ItemScore)>,
    element: (usize, ItemScore),
) {
    let Some(worst) = best.last() else {
        return;
    };
    if order(list, query, &element, worst) != Ordering::Less {
        return;
    }
    best.pop();
    let at = first_after(best, |kept| {
        order(list, query, &element, kept) == Ordering::Less
    });
    best.insert(at, element);
}

/// `findFirstIdxMonotonousOrArrLen`: the first index whose element `better` holds for, by the
/// same halving, so an order that is not consistent places elements as VS Code does.
fn first_after<T>(items: &[T], better: impl Fn(&T) -> bool) -> usize {
    let (mut low, mut high) = (0, items.len());
    while low < high {
        let middle = (low + high) / 2;
        if better(&items[middle]) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

/// The file on the wire; none for a path the wire cannot carry.
fn found(file: &File<'_>, score: &ItemScore) -> Option<FoundFile> {
    let folder = file.folder.path.as_str();
    let mut path = String::with_capacity(folder.len() + 1 + file.path.len());
    path.push_str(folder);
    let separator = if cfg!(windows) { '\\' } else { '/' };
    if !path.ends_with(separator) {
        path.push(separator);
    }
    path.push_str(&js::text(file.path));
    Some(FoundFile {
        path: FilePath::new(path).ok()?,
        score: SafeInteger::new(u64::try_from(score.score).unwrap_or(0)).unwrap_or_default(),
        label_matches: spans(&score.label),
        description_matches: spans(&score.description),
    })
}

fn spans(spans: &[Span]) -> Vec<[SafeInteger; 2]> {
    let at = |value: usize| SafeInteger::new(value as u64).unwrap_or_default();
    spans
        .iter()
        .map(|(start, end)| [at(*start), at(*end)])
        .collect()
}

/// The length of `value` as the reply encodes it.
fn json_length(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX / 4, |json| json.len())
}
