use super::{record, run};
use crate::invocation::{Request, parse_selected_request};
use orchbee_local_runtime::{CLIENT_FAILURE_HELPER, FallbackEvent};
use std::ffi::OsString;

// The helper's grammar is frozen: its word and exactly one version, with no output format. It
// records one Client failure of that version, and outside a store version folder, as here, it
// records nothing; either way it writes nothing and exits 0.
#[test]
fn the_helper_records_one_client_failure_and_answers_nothing() {
    let words = |words: &[&str]| words.iter().map(OsString::from).collect::<Vec<_>>();
    let selected =
        parse_selected_request(words(&[CLIENT_FAILURE_HELPER.current, "0.2.0"]).into_iter())
            .unwrap();
    assert_eq!(
        (selected.request, selected.format),
        (
            Request::RecordClientFailure {
                version: "0.2.0".into()
            },
            None
        )
    );
    for other in [
        &[CLIENT_FAILURE_HELPER.current][..],
        &[CLIENT_FAILURE_HELPER.current, "0.2.0", "0.3.0"],
        &[CLIENT_FAILURE_HELPER.current, "0.2.0", "--format", "json"],
    ] {
        assert!(
            !parse_selected_request(words(other).into_iter()).is_ok_and(|selected| matches!(
                selected.request,
                Request::RecordClientFailure { .. }
            )),
            "{other:?}"
        );
    }

    let mut recorded = Vec::new();
    record("0.2.0", |event| recorded.push(format!("{event:?}")));
    assert_eq!(
        recorded,
        [format!(
            "{:?}",
            FallbackEvent::ClientFailure { version: "0.2.0" }
        )]
    );

    let output = run("0.2.0");
    assert_eq!(output.status, 0);
    assert!(output.stdout.is_empty() && output.stderr.is_empty());
    assert!(output.finalization.is_none());
}
