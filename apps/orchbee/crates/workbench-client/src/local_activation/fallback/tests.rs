use super::{Seen, damaged, observe};
use orchbee_local_runtime::FallbackEvent;

// A generation's start and each of its failures name it by the same text, the 32 lowercase digits
// the fallback record lists, so the recorder counts a failure only for a start it saw.
#[test]
fn a_start_and_every_failure_name_the_generation_alike() {
    let activation = [0x0f; 16];
    let text = "0f".repeat(16);
    for (seen, expected) in [
        (
            Seen::Started,
            FallbackEvent::SessionStart { activation: &text },
        ),
        (
            Seen::FailedToStart,
            FallbackEvent::SessionFailure { activation: &text },
        ),
        (
            Seen::Crashed,
            FallbackEvent::SessionFailure { activation: &text },
        ),
    ] {
        let mut recorded = Vec::new();
        observe(&activation, seen, |event| {
            recorded.push(format!("{event:?}"))
        });
        assert_eq!(recorded, [format!("{expected:?}")], "{seen:?}");
    }
}

// A damaged store version fails the cohort check before any generation exists; unless that counts
// as a generation that started and failed, three such starts never fall back and the version stays
// active (E1 finding, 2026-11-16). Only damage counts, never a deadline or a refused open.
#[test]
fn a_damaged_cohort_counts_as_a_generation_that_failed_to_start() {
    use std::io::ErrorKind;
    let activation = [0xa5; 16];
    let text = "a5".repeat(16);
    let mut recorded = Vec::new();
    observe(&activation, Seen::Damaged, |event| {
        recorded.push(format!("{event:?}"))
    });
    assert_eq!(
        recorded,
        [
            format!("{:?}", FallbackEvent::SessionStart { activation: &text }),
            format!("{:?}", FallbackEvent::SessionFailure { activation: &text }),
        ]
    );
    for (kind, counts) in [
        (ErrorKind::InvalidData, true),
        (ErrorKind::NotFound, true),
        (ErrorKind::TimedOut, false),
        (ErrorKind::PermissionDenied, false),
        (ErrorKind::Other, false),
    ] {
        assert_eq!(damaged(kind), counts, "{kind:?}");
    }
}
