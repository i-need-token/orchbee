use super::super::configured_commands::for_client;
use orchbee_workbench_protocol::{
    ConfiguredCommandFailure as Failure, ConfiguredCommandOutcome as Outcome,
};

/// A Client whose handshake answer lacks `configured-command-source-exited`, such as a 0.1.0-beta.14
/// one, cannot decode that reason, so it reads the `unavailable` its release answered with; one that
/// lists it reads the cause, and no other outcome changes for either.
#[test]
fn source_exited_reaches_only_a_client_that_negotiated_it() {
    let failed = |reason| Outcome::Failed { reason };
    assert_eq!(
        for_client(failed(Failure::SourceExited), true),
        failed(Failure::SourceExited)
    );
    assert_eq!(
        for_client(failed(Failure::SourceExited), false),
        failed(Failure::Unavailable)
    );
    for reason in [
        Failure::Denied,
        Failure::InvalidIntent,
        Failure::SessionMismatch,
        Failure::Unavailable,
        Failure::Busy,
        Failure::Conflict,
        Failure::NotFound,
        Failure::StaleSource,
        Failure::PhysicalUnknown,
    ] {
        for named in [false, true] {
            assert_eq!(for_client(failed(reason), named), failed(reason));
        }
    }
}
