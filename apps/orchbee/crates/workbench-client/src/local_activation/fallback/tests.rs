use super::{Seen, observe};
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
