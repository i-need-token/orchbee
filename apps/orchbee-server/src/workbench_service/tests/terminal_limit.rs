use super::super::{WorkbenchExecution, terminal_limit_for_client};
use orchbee_workbench_protocol::{
    ProtocolVersion, WorkbenchCommandOutcome as Outcome, WorkbenchCommandResult,
    WorkbenchCommandUnavailableReason as Reason, WorkbenchRequestId, WorkbenchResponse,
};

/// A Client whose handshake answer lacks `terminal-limit-reached`, such as a 0.1.0-beta.19 one,
/// cannot decode that reason, so it reads the `server-busy` its release answered with; one that
/// lists it reads the limit, and no other outcome changes for either.
#[test]
fn terminal_limit_reaches_only_a_client_that_negotiated_it() {
    let unavailable = |reason| {
        WorkbenchExecution::Response(WorkbenchResponse::Command(WorkbenchCommandResult {
            protocol_version: ProtocolVersion,
            request_id: WorkbenchRequestId::parse(&format!("{:032x}", 7)).unwrap(),
            outcome: Outcome::Unavailable { reason },
        }))
    };
    assert_eq!(
        terminal_limit_for_client(unavailable(Reason::TerminalLimitReached), true),
        unavailable(Reason::TerminalLimitReached)
    );
    assert_eq!(
        terminal_limit_for_client(unavailable(Reason::TerminalLimitReached), false),
        unavailable(Reason::ServerBusy)
    );
    for reason in [
        Reason::RequestLedgerFull,
        Reason::NoCompatibleWorker,
        Reason::TerminalBusy,
        Reason::TerminalRuntimeUnavailable,
        Reason::ServerBusy,
        Reason::ServiceUnavailable,
        Reason::DirectoryUnavailable,
        Reason::DirectoryMissing,
        Reason::NotADirectory,
        Reason::DirectoryInaccessible,
    ] {
        for named in [false, true] {
            assert_eq!(
                terminal_limit_for_client(unavailable(reason), named),
                unavailable(reason)
            );
        }
    }
    for named in [false, true] {
        assert_eq!(
            terminal_limit_for_client(WorkbenchExecution::OutcomeUnknown, named),
            WorkbenchExecution::OutcomeUnknown
        );
    }
}
