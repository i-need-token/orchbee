//! The outbound queue under a Server that reads late. Seeded cases interleave Updates of a watch,
//! an SCM and a relay subscription, chunk replies and the writer taking frames on a queue of a few
//! KiB, let the writer take the rest, and check what a full queue keeps: an Update joins the queue
//! exactly when it fits and its subscription has no collapsed place, and otherwise folds into that
//! place; one take holds at most one rescan; a watch ends at its latest root and watch state, and
//! each change it produced arrives as produced or under the folder of a later rescan; an SCM
//! subscription delivers its summaries in order and ends at its latest; every prompt and reply
//! arrives whole and in order; and each call closes once, last. `ORCHBEE_FILES_OUTBOUND_SEED`
//! explores another seed; a failure prints its seed and case. A collapsed watch's one rescan is
//! checked on its own against its folder and byte bounds, which only many lost folders reach.

use std::{collections::HashMap, path::MAIN_SEPARATOR, sync::Arc};

use orchbee_files_protocol::{
    CallId, Change, ChangeKind, Chunk, ConnectionId, FilePath, FilesNotice, FilesRecord, FrameKind,
    FsSnapshot, FsUpdate, GitProgram, GitState, LaneId, LinkFrame, LinkMessage, LinkSide, List,
    NoticeKind, Operation, PromptId, RecordPlace, RelaySnapshot, RelayUpdate, Root, ScmSummary,
    TransferId, WatchState, decode_files_record, encode_notice, parse_notice, read_link_message,
};

use super::{Outbound, collapse::Folded, frame};
use crate::{
    calls::Calls,
    scope::path::{self, NativePath},
    seeded::{self, SplitMix64},
};

const DEFAULT_SEED: u64 = 0x0b0f_0000_0000_4001;
const CASES: u64 = 120;
const STEPS: usize = 160;
/// A queue a few Updates fill, so a case collapses often.
const CAPACITY: usize = 6 * 1024;

const WATCH: u64 = 1;
const SCM: u64 = 2;
const RELAY: u64 = 3;
/// The first call a reply takes.
const REPLIES: u64 = 16;

#[test]
fn a_full_queue_collapses_subscriptions_and_keeps_every_reply() {
    let seed = seeded::seed("ORCHBEE_FILES_OUTBOUND_SEED", DEFAULT_SEED);
    for case in 0..CASES {
        let mut random = SplitMix64::new(seed ^ case);
        let log = run(&mut random, &format!("seed {seed:#x} case {case}"));
        check(&log, &format!("seed {seed:#x} case {case}"));
    }
}

/// What one case produced, and what the writer took.
#[derive(Default)]
struct Log {
    /// The watch's Updates, each with the step that produced it.
    watch: Vec<(usize, FsUpdate)>,
    summaries: Vec<ScmSummary>,
    prompts: Vec<RelayUpdate>,
    replies: Vec<(CallId, Vec<u8>)>,
    /// Each record the writer took, with the step that took it.
    taken: Vec<(usize, CallId, Record)>,
}

#[derive(Debug)]
enum Record {
    Snapshot,
    Watch(FsUpdate),
    Summary(ScmSummary),
    Prompt(RelayUpdate),
    Reply(Vec<u8>),
    Close,
}

fn run(random: &mut SplitMix64, context: &str) -> Log {
    let outbound = Arc::new(Outbound::with_capacity(CAPACITY));
    let calls = Calls::new(Arc::clone(&outbound));
    let open = |id| {
        calls
            .open(
                call(id),
                ConnectionId::new(1).unwrap(),
                LaneId::new(id).unwrap(),
            )
            .unwrap()
            .unwrap()
    };
    let watch = open(WATCH).snapshot(&FilesNotice::FsSnapshot(FsSnapshot {
        root: Root::Present(file_path(root())),
        os: path::OPERATING_SYSTEM,
        separator: path::SEPARATOR,
        case_sensitive: path::CASE_SENSITIVE,
        watch: WatchState::Full,
    }));
    let scm = open(SCM).snapshot(&FilesNotice::ScmSummary(Box::new(summary(0, 0))));
    let relay = open(RELAY).snapshot(&FilesNotice::RelaySnapshot(RelaySnapshot {
        prompts: List::new(Vec::new()).unwrap(),
    }));
    let mut log = Log::default();
    for step in 0..STEPS {
        match random.below(12) {
            0..=3 => {
                let update = watch_update(random);
                let notice = FilesNotice::FsUpdate(update.clone());
                offer(&outbound, WATCH, &notice, context, || watch.update(&notice));
                log.watch.push((step, update));
            }
            4 | 5 => {
                let largest = if random.below(8) == 0 { 256 } else { 40 };
                let size = random.below(largest);
                let summary = summary(log.summaries.len() + 1, size);
                let notice = FilesNotice::ScmSummary(Box::new(summary.clone()));
                offer(&outbound, SCM, &notice, context, || scm.update(&notice));
                log.summaries.push(summary);
            }
            6 => {
                let prompt = RelayUpdate::Closed {
                    id: prompt(log.prompts.len() + 1),
                };
                let notice = FilesNotice::RelayUpdate(prompt.clone());
                offer(&outbound, RELAY, &notice, context, || relay.update(&notice));
                log.prompts.push(prompt);
            }
            7 | 8 => {
                let id = REPLIES + log.replies.len() as u64;
                let bytes = vec![(id % 251) as u8; 1 + random.below(3000)];
                open(id).chunk(Chunk {
                    transfer: TransferId::new(id).unwrap(),
                    offset: 0,
                    end: None,
                    bytes: bytes.clone(),
                });
                log.replies.push((call(id), bytes));
            }
            _ => {
                for _ in 0..random.below(6) {
                    take(&outbound, step, &mut log, context);
                }
            }
        }
    }
    drop((watch, scm, relay));
    while take(&outbound, STEPS, &mut log, context) {}
    let state = outbound.lock();
    assert!(
        state.collapsed.is_empty() && state.bytes == 0,
        "{context}: a drained queue holds nothing"
    );
    drop(state);
    log
}

/// Queues `notice` of subscription `id` through `queue`, checking the rule it met: a relay prompt
/// joins the queue; a watch's or an SCM Update joins when it fits and its subscription has no
/// collapsed place, and otherwise folds into that place, which it opens when there was none.
fn offer(outbound: &Outbound, id: u64, notice: &FilesNotice, context: &str, queue: impl FnOnce()) {
    let record = FilesRecord::Notice(encode_notice(notice));
    let length = frame(call(id), FrameKind::Update, &record).len();
    let (bytes, placed) = state(outbound, id);
    queue();
    let after = state(outbound, id);
    let collapses = !matches!(notice, FilesNotice::RelayUpdate(_));
    if !collapses || (!placed && bytes + length <= CAPACITY) {
        assert_eq!(
            after,
            (bytes + length, placed),
            "{context}: it fits and joins"
        );
    } else {
        assert_eq!(after, (bytes, true), "{context}: it does not fit and folds");
    }
}

/// The bytes the queue holds and whether subscription `id` has a collapsed place.
fn state(outbound: &Outbound, id: u64) -> (usize, bool) {
    let state = outbound.lock();
    (state.bytes, state.collapsed.contains_key(&call(id)))
}

/// Lets the writer take what comes next, when anything is queued.
fn take(outbound: &Outbound, step: usize, log: &mut Log, context: &str) -> bool {
    if outbound.lock().queued.is_empty() {
        return false;
    }
    let frames = outbound.next().unwrap();
    outbound.written();
    let mut reader = frames.as_slice();
    let mut rescans = 0;
    while !reader.is_empty() {
        let (call, record) = decoded(read_link_message(&mut reader, LinkSide::Files).unwrap());
        rescans += usize::from(matches!(record, Record::Watch(FsUpdate::Rescan { .. })));
        log.taken.push((step, call, record));
    }
    assert!(rescans <= 1, "{context}: one take holds at most one rescan");
    true
}

fn decoded(message: LinkMessage) -> (CallId, Record) {
    let LinkMessage::Frame(LinkFrame { call, kind, record }) = message else {
        let LinkMessage::Close { call, .. } = message else {
            panic!("the queue carries frames and closes, not {message:?}");
        };
        return (call, Record::Close);
    };
    if kind == FrameKind::Result {
        let FilesRecord::Chunk(chunk) =
            decode_files_record(&record, RecordPlace::ResultFrame).unwrap()
        else {
            panic!("every reply here is a chunk");
        };
        return (call, Record::Reply(chunk.bytes));
    }
    let FilesRecord::Notice(json) = decode_files_record(&record, RecordPlace::NoticeFrame).unwrap()
    else {
        panic!("a subscription's frame carries a notice");
    };
    let operation = match call.get() {
        WATCH => Operation::FsSubscribe,
        SCM => Operation::ScmSubscribe,
        _ => Operation::RelaySubscribe,
    };
    if kind == FrameKind::Snapshot {
        parse_notice(operation, NoticeKind::Snapshot, &json).unwrap();
        return (call, Record::Snapshot);
    }
    let record = match parse_notice(operation, NoticeKind::Update, &json).unwrap() {
        FilesNotice::FsUpdate(update) => Record::Watch(update),
        FilesNotice::ScmSummary(summary) => Record::Summary(*summary),
        FilesNotice::RelayUpdate(update) => Record::Prompt(update),
        other => panic!("no Update is {other:?}"),
    };
    (call, record)
}

fn check(log: &Log, context: &str) {
    let mut calls: HashMap<CallId, Vec<&Record>> = HashMap::new();
    for (_, call, record) in &log.taken {
        calls.entry(*call).or_default().push(record);
    }
    for (call, records) in &calls {
        let closes = records
            .iter()
            .filter(|record| matches!(record, Record::Close))
            .count();
        assert!(
            closes == 1 && matches!(records.last(), Some(Record::Close)),
            "{context}: {call:?} closes once, last"
        );
        assert!(
            matches!(records.first(), Some(Record::Snapshot | Record::Reply(_))),
            "{context}: {call:?} starts with its Snapshot or reply"
        );
    }

    let replies: Vec<(CallId, Vec<u8>)> = log
        .taken
        .iter()
        .filter_map(|(_, call, record)| match record {
            Record::Reply(bytes) => Some((*call, bytes.clone())),
            _ => None,
        })
        .collect();
    assert!(replies == log.replies, "{context}: every reply arrives");
    let prompts: Vec<&RelayUpdate> = log
        .taken
        .iter()
        .filter_map(|(_, _, record)| match record {
            Record::Prompt(prompt) => Some(prompt),
            _ => None,
        })
        .collect();
    assert!(
        prompts.into_iter().eq(&log.prompts),
        "{context}: every prompt arrives"
    );

    let summaries: Vec<&ScmSummary> = log
        .taken
        .iter()
        .filter_map(|(_, _, record)| match record {
            Record::Summary(summary) => Some(summary),
            _ => None,
        })
        .collect();
    let mut produced = log.summaries.iter();
    assert!(
        summaries
            .iter()
            .all(|summary| produced.any(|candidate| candidate == *summary)),
        "{context}: summaries arrive in order"
    );
    assert_eq!(
        summaries.last().copied(),
        log.summaries.last(),
        "{context}: the latest summary arrives"
    );

    let watched: Vec<(usize, &FsUpdate)> = log
        .taken
        .iter()
        .filter_map(|(step, _, record)| match record {
            Record::Watch(update) => Some((*step, update)),
            _ => None,
        })
        .collect();
    let roots = |updates: &mut dyn Iterator<Item = &FsUpdate>| {
        updates
            .filter_map(|update| match update {
                FsUpdate::Root { root } => Some(root.clone()),
                _ => None,
            })
            .last()
    };
    let states = |updates: &mut dyn Iterator<Item = &FsUpdate>| {
        updates
            .filter_map(|update| match update {
                FsUpdate::Watch { watch } => Some(*watch),
                _ => None,
            })
            .last()
    };
    assert_eq!(
        roots(&mut watched.iter().map(|(_, update)| *update)),
        roots(&mut log.watch.iter().map(|(_, update)| update)),
        "{context}: the latest root arrives"
    );
    assert_eq!(
        states(&mut watched.iter().map(|(_, update)| *update)),
        states(&mut log.watch.iter().map(|(_, update)| update)),
        "{context}: the latest watch state arrives"
    );
    // The steps that took each change as produced, and each folder a rescan named, by its key.
    let mut changed: HashMap<(u8, &str, bool), Vec<usize>> = HashMap::new();
    let mut rescanned: HashMap<String, Vec<usize>> = HashMap::new();
    for (step, update) in &watched {
        match update {
            FsUpdate::Changes { changes } => {
                for change in changes.as_slice() {
                    changed.entry(identity(change)).or_default().push(*step);
                }
            }
            FsUpdate::Rescan { paths } => {
                for path in paths.as_slice() {
                    rescanned
                        .entry(native(path.as_str()).key())
                        .or_default()
                        .push(*step);
                }
            }
            FsUpdate::Root { .. } | FsUpdate::Watch { .. } => {}
        }
    }
    let later = |steps: Option<&Vec<usize>>, produced: usize| {
        steps.is_some_and(|steps| steps.iter().any(|step| *step > produced))
    };
    for (produced, update) in &log.watch {
        let lost: Vec<(Option<&Change>, NativePath)> = match update {
            FsUpdate::Changes { changes } => changes
                .as_slice()
                .iter()
                .map(|change| (Some(change), native(change.path.as_str()).parent().unwrap()))
                .collect(),
            FsUpdate::Rescan { paths } => paths
                .as_slice()
                .iter()
                .map(|path| (None, native(path.as_str())))
                .collect(),
            FsUpdate::Root { .. } | FsUpdate::Watch { .. } => Vec::new(),
        };
        for (change, folder) in lost {
            let as_produced =
                change.is_some_and(|change| later(changed.get(&identity(change)), *produced));
            let under_rescan = std::iter::successors(Some(folder.clone()), NativePath::parent)
                .any(|named| later(rescanned.get(&named.key()), *produced));
            assert!(
                as_produced || under_rescan,
                "{context}: {change:?} in {folder:?} from step {produced} arrives"
            );
        }
    }
}

/// What tells two changes apart.
fn identity(change: &Change) -> (u8, &str, bool) {
    let kind = match change.kind {
        ChangeKind::Added => 0,
        ChangeKind::Changed => 1,
        ChangeKind::Deleted => 2,
    };
    (kind, change.path.as_str(), change.directory)
}

#[test]
fn a_collapsed_watch_rescans_every_lost_folder_in_one_update() {
    let segments = |segments: &[String]| {
        std::iter::once(root().to_owned())
            .chain(segments.iter().cloned())
            .collect::<Vec<_>>()
            .join(std::path::MAIN_SEPARATOR_STR)
    };
    // While 500 folders fit the record, the rescan names each.
    let few: Vec<String> = (0..300)
        .map(|index| segments(&[format!("a{}", index / 20), format!("b{}", index % 20)]))
        .collect();
    let named = folded(&few);
    let keys = |folders: &mut dyn Iterator<Item = NativePath>| {
        folders
            .map(|folder| folder.key())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        keys(&mut named.into_iter()),
        keys(&mut few.iter().map(|folder| native(folder)))
    );
    // Past 500 folders, or past the record with long names, fewer folders hold every one.
    let many: Vec<String> = (0..1200)
        .map(|index| segments(&[format!("c{}", index / 40), format!("d{}", index % 40)]))
        .collect();
    let long: Vec<String> = (0..400)
        .map(|index| {
            let name = |number: usize| format!("{number:x>120}");
            segments(&[name(index / 100), name(index / 10 % 10), name(index % 10)])
        })
        .collect();
    for lost in [many, long] {
        let named = folded(&lost);
        for folder in &lost {
            assert!(
                named.iter().any(|named| native(folder).is_within(named)),
                "{folder} lies under a folder the rescan names"
            );
        }
    }
}

/// The folders of the one rescan a collapsed watch sends for Rescan Updates naming `folders`, a
/// hundred at a time. Its frame decodes, so it holds at most 500 within the record's bound.
fn folded(folders: &[String]) -> Vec<NativePath> {
    let mut folded = Folded::default();
    for batch in folders.chunks(100) {
        let paths = batch.iter().map(|folder| file_path(folder)).collect();
        let paths = List::new(paths).unwrap();
        folded.fold(
            &FilesNotice::FsUpdate(FsUpdate::Rescan { paths }),
            Vec::new(),
        );
    }
    let frames = folded.frames(call(WATCH));
    let mut reader = frames.as_slice();
    let (_, record) = decoded(read_link_message(&mut reader, LinkSide::Files).unwrap());
    assert!(reader.is_empty(), "the watch sends one Update");
    let Record::Watch(FsUpdate::Rescan { paths }) = record else {
        panic!("the watch sends a rescan, not {record:?}");
    };
    paths
        .as_slice()
        .iter()
        .map(|path| native(path.as_str()))
        .collect()
}

fn watch_update(random: &mut SplitMix64) -> FsUpdate {
    match random.below(16) {
        0..=7 => {
            let count = 1 + random.below(4);
            changes(random, count, false)
        }
        // Deep and long, so its frame alone passes the queue's capacity.
        8 => {
            let count = 12 + random.below(9);
            changes(random, count, true)
        }
        9..=11 => {
            let paths = (0..=random.below(2))
                .map(|_| file_path(folder(random, false).as_str()))
                .collect();
            FsUpdate::Rescan {
                paths: List::new(paths).unwrap(),
            }
        }
        12 | 13 => {
            let moved = format!("{}{MAIN_SEPARATOR}moved", root());
            let root = match random.below(3) {
                0 => Root::Missing(file_path(root())),
                1 => Root::Present(file_path(&moved)),
                _ => Root::Present(file_path(root())),
            };
            FsUpdate::Root { root }
        }
        _ => FsUpdate::Watch {
            watch: [WatchState::Full, WatchState::Degraded, WatchState::Stopped][random.below(3)],
        },
    }
}

fn changes(random: &mut SplitMix64, count: usize, long: bool) -> FsUpdate {
    let changes = (0..count)
        .map(|_| Change {
            kind: [ChangeKind::Added, ChangeKind::Changed, ChangeKind::Deleted][random.below(3)],
            path: file_path(&format!(
                "{}{MAIN_SEPARATOR}f{}",
                folder(random, long).as_str(),
                random.below(8)
            )),
            directory: random.below(4) == 0,
        })
        .collect();
    FsUpdate::Changes {
        changes: List::new(changes).unwrap(),
    }
}

/// A folder of the root; a long one is deep with 120-character names.
fn folder(random: &mut SplitMix64, long: bool) -> NativePath {
    let mut folder = root().to_owned();
    let depth = if long {
        2 + random.below(3)
    } else {
        random.below(5)
    };
    for _ in 0..depth {
        folder.push(MAIN_SEPARATOR);
        if long {
            folder.push_str(&format!("{:x>120}", random.below(40)));
        } else {
            folder.push(char::from(b"abcd"[random.below(4)]));
        }
    }
    NativePath::parse(&folder).unwrap()
}

/// The `number`th summary, `size` candidates long, which no other summary equals.
fn summary(number: usize, size: usize) -> ScmSummary {
    let candidates = (0..=size)
        .map(|index| {
            file_path(&format!(
                "{}{MAIN_SEPARATOR}summary-{number}-{index}",
                root()
            ))
        })
        .collect();
    ScmSummary {
        git: GitProgram {
            state: GitState::Ok,
            version: None,
        },
        repositories: List::new(Vec::new()).unwrap(),
        candidates: List::new(candidates).unwrap(),
        unsafe_repositories: List::new(Vec::new()).unwrap(),
        repositories_truncated: false,
    }
}

fn prompt(number: usize) -> PromptId {
    let mut bytes = [0; 16];
    bytes[8..].copy_from_slice(&(number as u64).to_be_bytes());
    PromptId::from_bytes(bytes).unwrap()
}

fn root() -> &'static str {
    if cfg!(windows) { r"C:\w" } else { "/w" }
}

fn call(id: u64) -> CallId {
    CallId::new(id).unwrap()
}

fn file_path(path: &str) -> FilePath {
    FilePath::new(path).unwrap()
}

fn native(path: &str) -> NativePath {
    NativePath::parse(path).unwrap()
}
