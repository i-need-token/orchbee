use super::{Answer, Check, Store, UpdateCheck};
use orchbee_workbench_protocol::{UpdateCheckFinish, UpdateCheckOutcome};
use std::time::{Duration, Instant};

fn finished(result: UpdateCheckFinish) -> Answer {
    Answer::Outcome(UpdateCheckOutcome::Finished { result })
}

const NO_READING: Store = Store {
    read: false,
    ready: false,
};

/// Only a run this TUI asked for reports its end; the end of a run About merely saw under way,
/// a scheduled one, shows nothing, so a failed background run changes nothing on screen.
#[test]
fn only_an_asked_run_reports_its_end() {
    let now = Instant::now();
    for (asked, result, shown) in [
        (true, UpdateCheckFinish::UpToDate, UpdateCheck::UpToDate),
        (true, UpdateCheckFinish::Failed, UpdateCheck::Failed),
        (false, UpdateCheckFinish::UpToDate, UpdateCheck::Idle),
        (false, UpdateCheckFinish::Failed, UpdateCheck::Idle),
        (false, UpdateCheckFinish::Installed, UpdateCheck::Idle),
    ] {
        let mut check = Check {
            asked,
            shown: UpdateCheck::Checking,
            ..Check::default()
        };
        assert!(!check.settle(finished(result), now));
        assert_eq!(check.shown, shown, "{asked} {result:?}");
        // A lost answer ends an asked check as failed and leaves an unasked one silent.
        let mut check = Check {
            asked,
            shown: UpdateCheck::Checking,
            ..Check::default()
        };
        check.settle(Answer::Lost, now);
        let lost = if asked {
            UpdateCheck::Failed
        } else {
            UpdateCheck::Idle
        };
        assert_eq!(check.shown, lost, "lost, {asked}");
    }
}

/// An asked run that installed keeps Checking until the store's reading arrives, never showing
/// up to date before the Update control can; without a reading it settles when the wait ends.
#[test]
fn an_install_waits_for_the_store_before_it_reports() {
    let now = Instant::now();
    let installed = || {
        let mut check = Check {
            asked: true,
            shown: UpdateCheck::Checking,
            ..Check::default()
        };
        assert!(check.settle(finished(UpdateCheckFinish::Installed), now));
        check
    };
    let mut check = installed();
    assert!(check.wait_for_store(NO_READING, now));
    assert_eq!(check.shown, UpdateCheck::Checking);
    assert!(!check.wait_for_store(
        Store {
            read: true,
            ready: true
        },
        now
    ));
    assert_eq!(check.shown, UpdateCheck::Idle);

    // The store names nothing newer than this TUI, which is then already up to date.
    let mut check = installed();
    assert!(!check.wait_for_store(
        Store {
            read: true,
            ready: false
        },
        now
    ));
    assert_eq!(check.shown, UpdateCheck::UpToDate);

    let mut check = installed();
    assert!(!check.wait_for_store(NO_READING, now + Duration::from_secs(60)));
    assert_eq!(check.shown, UpdateCheck::UpToDate);
}
