//! Deterministic registry and pacing checks under an explicit clock: reads start at most once per
//! interval, a page continues at once, a reset rereads from 0, failures and fencing wait, a newer
//! generation or a retirement makes titles unavailable, and published changes wake only the
//! title-following streams.
use super::*;
use orchbee_capability_node_protocol::{
    HostCreationId, HostEpoch, HostTerminalId, TerminalTitleChangePage, TerminalTitleRow,
};
use orchbee_workbench_protocol::MAXIMUM_PRESENTATION_TITLE_BYTES;

const WORKER: [u8; 16] = [9; 16];

fn worker() -> WorkerId {
    WorkerId::from_bytes(WORKER).unwrap()
}

fn generation(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}

fn identity(terminal: u8, creation: u8) -> HostTerminalIdentity {
    HostTerminalIdentity::new(
        HostCreationId::from_bytes([creation; 16]).unwrap(),
        HostEpoch::from_bytes([2; 16]).unwrap(),
        HostTerminalId::from_bytes([terminal; 16]).unwrap(),
    )
}

fn observed(identity: HostTerminalIdentity, title: Option<&str>) -> TerminalTitleRow {
    TerminalTitleRow {
        identity,
        observation: TerminalTitleObservation::Observed {
            revision: NonZeroU64::MIN,
            title: title.map(str::to_owned),
        },
    }
}

fn page(rows: Vec<TerminalTitleRow>, next: u64, complete: bool) -> TerminalTitleChangesOutcome {
    TerminalTitleChangesOutcome::Page(TerminalTitleChangePage {
        rows,
        next,
        complete,
    })
}

fn read(titles: &TerminalTitles, now: Instant) -> TerminalTitleChangesRequest {
    match titles.next_read(now) {
        NextRead::Read(request) => request,
        other => panic!("a read was due, got {other:?}"),
    }
}

fn shown(titles: &TerminalTitles, value: u64, identity: HostTerminalIdentity) -> PresentationTitle {
    titles.title(WorkerGeneration::new(generation(value)), identity)
}

fn text(title: &str) -> PresentationTitle {
    PresentationTitle::Observed {
        title: Some(title.to_owned()),
    }
}

#[test]
fn reads_are_paced_continued_reset_and_retried() {
    let titles = TerminalTitles::default();
    let start = Instant::now();
    assert_eq!(titles.next_read(start), NextRead::Wait(None));
    titles.begin(worker(), generation(1));

    // A new generation reads afresh at once; an incomplete page continues without pacing.
    let first = read(&titles, start);
    assert_eq!(first.after, 0);
    assert_eq!(
        first.worker_generation,
        WorkerGeneration::new(generation(1))
    );
    let (a, b) = (identity(1, 1), identity(2, 1));
    titles.apply(
        &first,
        Some(page(vec![observed(a, Some("\u{280b} build"))], 4, false)),
        start,
    );
    let next = read(&titles, start + Duration::from_millis(1));
    assert_eq!(next.after, 4);
    titles.apply(&next, Some(page(vec![observed(b, None)], 6, true)), start);
    assert_eq!(shown(&titles, 1, a), text("build"));
    assert_eq!(
        shown(&titles, 1, b),
        PresentationTitle::Observed { title: None }
    );
    assert_eq!(titles.next_read(start), NextRead::Wait(None));

    // A note within the interval waits for it; a stale or foreign note asks for nothing.
    titles.note(generation(1), generation(6));
    assert_eq!(titles.next_read(start), NextRead::Wait(None));
    titles.note(generation(2), generation(9));
    assert_eq!(titles.next_read(start), NextRead::Wait(None));
    titles.note(generation(1), generation(8));
    let allowed = start + READ_INTERVAL;
    assert_eq!(
        titles.next_read(start + Duration::from_millis(10)),
        NextRead::Wait(Some(allowed))
    );
    let paced = read(&titles, allowed);
    assert_eq!(paced.after, 6);
    // One read is outstanding at a time.
    assert_eq!(titles.next_read(allowed), NextRead::Wait(None));

    // A reset discards every title and rereads from 0 after a full interval.
    titles.apply(&paced, Some(TerminalTitleChangesOutcome::Reset), allowed);
    assert_eq!(shown(&titles, 1, a), PresentationTitle::Unavailable {});
    let later = allowed + READ_INTERVAL;
    assert_eq!(titles.next_read(allowed), NextRead::Wait(Some(later)));
    let fresh = read(&titles, later);
    assert_eq!(fresh.after, 0);

    // An answer to another request changes nothing; a failure retries a full interval later.
    titles.apply(
        &paced,
        Some(page(vec![observed(a, Some("late"))], 7, true)),
        later,
    );
    assert_eq!(shown(&titles, 1, a), PresentationTitle::Unavailable {});
    let failed = later + Duration::from_millis(40);
    titles.apply(&fresh, None, failed);
    assert_eq!(
        titles.next_read(failed),
        NextRead::Wait(Some(failed + READ_INTERVAL))
    );
    let retry = read(&titles, failed + READ_INTERVAL);
    assert_eq!(retry.after, 0);

    // A Worker refusing its own generation is not read again until it notes a change.
    titles.apply(
        &retry,
        Some(TerminalTitleChangesOutcome::RejectedFencing),
        failed,
    );
    let much_later = failed + READ_INTERVAL * 8;
    assert_eq!(titles.next_read(much_later), NextRead::Wait(None));
    titles.note(generation(1), generation(1));
    assert_eq!(read(&titles, much_later).after, 0);

    titles.stop();
    assert_eq!(titles.next_read(much_later), NextRead::Stopped);
}

#[test]
fn titles_follow_identity_generation_and_removal() {
    let titles = TerminalTitles::default();
    let now = Instant::now();
    titles.begin(worker(), generation(1));
    let (a, reused) = (identity(1, 1), identity(1, 2));
    let request = read(&titles, now);
    let hostile = format!("\u{1b}]0;x\u{7}{}", "\u{6807}".repeat(1024));
    titles.apply(
        &request,
        Some(page(
            vec![
                observed(a, Some(&hostile)),
                observed(identity(3, 1), Some("\u{280b} ")),
            ],
            2,
            true,
        )),
        now,
    );
    // Untrusted text is projected: no control characters and within the published bound.
    let PresentationTitle::Observed { title: Some(title) } = shown(&titles, 1, a) else {
        panic!("a projected title expected")
    };
    assert!(title.len() <= MAXIMUM_PRESENTATION_TITLE_BYTES);
    assert!(!title.chars().any(char::is_control));
    // A title the projection empties is observed as null.
    assert_eq!(
        shown(&titles, 1, identity(3, 1)),
        PresentationTitle::Observed { title: None }
    );
    // Another creation of the same Host terminal, or another generation, has no title.
    assert_eq!(shown(&titles, 1, reused), PresentationTitle::Unavailable {});
    assert_eq!(shown(&titles, 2, a), PresentationTitle::Unavailable {});

    // An unavailable row removes the title.
    titles.note(generation(1), generation(3));
    let removal = read(&titles, now + READ_INTERVAL);
    titles.apply(
        &removal,
        Some(page(
            vec![TerminalTitleRow {
                identity: a,
                observation: TerminalTitleObservation::Unavailable,
            }],
            3,
            true,
        )),
        now,
    );
    assert_eq!(shown(&titles, 1, a), PresentationTitle::Unavailable {});

    // A newer generation replaces the old one; retiring it leaves nothing.
    titles.begin(worker(), generation(2));
    assert_eq!(
        shown(&titles, 1, identity(3, 1)),
        PresentationTitle::Unavailable {}
    );
    let fresh = read(&titles, now);
    assert_eq!((fresh.after, fresh.worker_generation.get()), (0, 2));
    titles.apply(
        &fresh,
        Some(page(vec![observed(a, Some("vim"))], 1, true)),
        now,
    );
    assert_eq!(shown(&titles, 2, a), text("vim"));
    titles.retire(generation(1));
    assert_eq!(shown(&titles, 2, a), text("vim"));
    titles.retire(generation(2));
    assert_eq!(shown(&titles, 2, a), PresentationTitle::Unavailable {});
    assert_eq!(titles.next_read(now + READ_INTERVAL), NextRead::Wait(None));
}

#[test]
fn the_registry_stays_bounded_and_publishes_only_title_changes() {
    let titles = TerminalTitles::default();
    let signal = Arc::new(SessionChangeSignal::default());
    titles.watch(&signal);
    let quiet = (signal.revision(), signal.commit_revision());
    let now = Instant::now();
    titles.begin(worker(), generation(1));
    // An empty generation replacing nothing publishes nothing.
    assert_eq!(signal.titles_revision(), 0);

    // More terminals than the Worker itself retains never grow the registry past the bound.
    let mut next = 0;
    let mut published = 0;
    for chunk in 0..5_u16 {
        let request = read(&titles, now + READ_INTERVAL * u32::from(chunk));
        let rows = (0..256_u16)
            .map(|index| {
                let terminal = chunk * 256 + index;
                let mut bytes = [0; 16];
                bytes[14..].copy_from_slice(&(terminal + 1).to_be_bytes());
                observed(
                    HostTerminalIdentity::new(
                        HostCreationId::from_bytes([1; 16]).unwrap(),
                        HostEpoch::from_bytes([2; 16]).unwrap(),
                        HostTerminalId::from_bytes(bytes).unwrap(),
                    ),
                    Some("sh"),
                )
            })
            .collect();
        next += 256;
        titles.apply(&request, Some(page(rows, next, chunk == 4)), now);
        // The last chunk is beyond the bound, so it changes nothing.
        published += u64::from(chunk < 4);
        assert_eq!(signal.titles_revision(), published);
    }
    let held = titles
        .state
        .lock()
        .unwrap()
        .current
        .as_ref()
        .unwrap()
        .titles
        .len();
    assert_eq!(held, MAXIMUM_TERMINALS);

    // A page that changes nothing publishes nothing; output and commits never moved.
    titles.note(generation(1), generation(next + 1));
    let request = read(&titles, now + READ_INTERVAL * 6);
    titles.apply(&request, Some(page(Vec::new(), next + 1, true)), now);
    assert_eq!(signal.titles_revision(), published);
    assert_eq!((signal.revision(), signal.commit_revision()), quiet);
    assert!(signal.wake_revision() > 0);
}
