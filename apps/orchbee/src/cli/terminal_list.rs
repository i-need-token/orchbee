//! Terminal-list observation on one admitted Session and deadline, for text and JSON alike.
//!
//! The identity snapshot answers the listing; observations are best effort. Only cancellation,
//! a local fault or a failed snapshot read fails the command. Every returned observation was
//! verified against the snapshot whose rows are returned, and every other row is not sampled.
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output, query,
    query::{metadata_failure, panel_agent_failure},
    results::{self, Projection, TerminalListDetails, TerminalListObservation},
    success_output,
};
use crate::cli_outcome::TerminalListState;
use crate::{
    invocation::{OutputFormat, QueryCommand, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    MAXIMUM_PANEL_AGENT_READ_ROWS, PanelAgentAction, PanelAgentFailure, PanelAgentOutcome,
    PanelAgentRequest, PanelAgentState, PanelAgentView, Snapshot, TerminalPresentationId,
    WorkbenchRevision,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

/// Keep optional Host observation work finite even when the snapshot is at its
/// protocol size limit. The snapshot still lists every bound terminal; rows
/// after this prefix are reported as not sampled.
const MAXIMUM_ENRICHED_TERMINALS: usize = 64;

/// Observation stops this long before the command deadline, leaving time to verify what it read.
const VERIFICATION_RESERVE: Duration = Duration::from_secs(1);

struct Reports {
    rows: BTreeMap<String, PanelAgentView>,
    revision: WorkbenchRevision,
}

/// The Agent reports of one sampled prefix, or proof that the Workbench moved under them.
enum ReportRead {
    Read(Option<Reports>),
    Changed,
}

/// One observation pass over one snapshot.
enum Attempt {
    /// Every observation it returns was verified against the snapshot it observed.
    Verified(Vec<TerminalListDetails>),
    /// The Workbench or its reports moved; carries the newer snapshot when it could be read.
    Changed(Option<Snapshot>),
    /// What it observed could not be verified.
    Unverified,
}

fn sampled_rows(rows: &[results::TerminalRow]) -> &[results::TerminalRow] {
    &rows[..rows.len().min(MAXIMUM_ENRICHED_TERMINALS)]
}

pub(crate) fn run(command: QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::WorkbenchInspection)
    {
        return failure_output(failure, Some(format), locale);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let result = execute(&command, deadline, &mut signals);
    let mut finalization = None;
    let result = match signals.finish() {
        Ok(Finish::Clear) => result,
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            Err(Failure::new(FailureKind::Cancelled))
        }
        Err(_) => Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut output = match result {
        Ok((source, projection)) => success_output(projection, source, &command, format, locale),
        Err(error) => failure_output(error, Some(format), locale),
    };
    output.finalization = finalization;
    output
}

fn execute(
    command: &QueryCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(serde_json::Value, Projection), Failure> {
    let (mut client, mut snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    check_cancelled(signals)?;
    let (mut rows, mut details) = list(command, &snapshot)?;
    // A moved Workbench is observed once more on the newer snapshot, under the same deadline.
    for retry in [true, false] {
        match observe(
            &mut client,
            &snapshot,
            &rows,
            details.clone(),
            deadline,
            signals,
        )? {
            Attempt::Verified(observed) => return Ok(listing(&snapshot, rows, observed)),
            Attempt::Changed(Some(newer)) => {
                // A newer snapshot that no longer answers a filter keeps the last one that did.
                let Ok((newer_rows, newer_details)) = list(command, &newer) else {
                    break;
                };
                (snapshot, rows, details) = (newer, newer_rows, newer_details);
                if !retry {
                    break;
                }
            }
            Attempt::Changed(None) | Attempt::Unverified => break,
        }
    }
    let details = unobserved(rows.len(), details);
    Ok(listing(&snapshot, rows, details))
}

fn list(
    command: &QueryCommand,
    snapshot: &Snapshot,
) -> Result<(Vec<results::TerminalRow>, Vec<TerminalListDetails>), Failure> {
    match results::project(command, snapshot)
        .map_err(|failure| failure.source(results::snapshot_source(snapshot)))?
    {
        Projection::TerminalList(results::TerminalListDocument { rows, details }) => {
            Ok((rows, details))
        }
        // `terminal list` projects only its own document.
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    }
}

fn listing(
    snapshot: &Snapshot,
    rows: Vec<results::TerminalRow>,
    details: Vec<TerminalListDetails>,
) -> (serde_json::Value, Projection) {
    (
        results::snapshot_source(snapshot),
        Projection::TerminalList(results::TerminalListDocument { rows, details }),
    )
}

/// Every row keeps its snapshot placement and carries no observation.
fn unobserved(rows: usize, mut details: Vec<TerminalListDetails>) -> Vec<TerminalListDetails> {
    details.resize_with(rows, TerminalListDetails::default);
    for row in &mut details {
        *row = TerminalListDetails {
            placement: row.placement.take(),
            observation: TerminalListObservation::NotSampled,
            ..TerminalListDetails::default()
        };
    }
    details
}

/// Fails only on cancellation; an expired deadline only ends optional observation.
fn check_cancelled(signals: &mut ArmedSignals) -> Result<(), Failure> {
    let requested = SignalWatch::take(signals)
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?
        .is_some();
    if requested {
        Err(Failure::new(FailureKind::Cancelled))
    } else {
        Ok(())
    }
}

/// An observation read that fails for any reason but cancellation or a local fault answers
/// nothing.
fn best_effort<T>(result: Result<T, Failure>) -> Result<Option<T>, Failure> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(failure)
            if matches!(
                failure.kind,
                FailureKind::Cancelled | FailureKind::InternalFailure
            ) =>
        {
            Err(failure)
        }
        Err(_) => Ok(None),
    }
}

fn observe(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    rows: &[results::TerminalRow],
    details: Vec<TerminalListDetails>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Attempt, Failure> {
    let mut details = unobserved(rows.len(), details);
    let sampled = sampled_rows(rows);
    if sampled.is_empty() {
        return Ok(Attempt::Verified(details));
    }
    let observe_until = deadline
        .checked_sub(VERIFICATION_RESERVE)
        .unwrap_or(deadline);
    let read = read_reports(client, snapshot, sampled, observe_until, signals);
    check_cancelled(signals)?;
    let reports = match best_effort(read)? {
        Some(ReportRead::Read(reports)) => reports,
        Some(ReportRead::Changed) => {
            return Ok(Attempt::Changed(newer_snapshot(
                client, snapshot, deadline, signals,
            )?));
        }
        // Without its reports a row's Agent facts are unknown, so nothing is observed.
        None => return Ok(Attempt::Verified(details)),
    };

    for (index, row) in sampled.iter().enumerate() {
        check_cancelled(signals)?;
        if Instant::now() >= observe_until {
            break;
        }
        let outcome = query::terminal_metadata::read(
            client,
            snapshot,
            &Target::Id(row.terminal_id.clone()),
            observe_until,
            signals.poll_fd(),
        );
        // Local I/O can surface cancellation as a transport error.
        check_cancelled(signals)?;
        let Some(observation) = best_effort(observation_for(outcome))? else {
            continue;
        };
        let fields = observation.fields();
        let report = reports
            .as_ref()
            .and_then(|reports| reports.rows.get(&row.presentation_id));
        let observed = matches!(
            observation,
            super::terminal_observation::Observation::Observed(_)
        );
        let details_row = &mut details[index];
        details_row.observation = if observed {
            TerminalListObservation::Observed
        } else {
            TerminalListObservation::FrameUnavailable
        };
        details_row.title = fields["terminalTitle"].as_str().map(str::to_owned);
        details_row.cwd = fields["cwd"].as_str().map(str::to_owned);
        details_row.agent = report
            .and_then(|report| report.agent.as_deref())
            .map(str::to_owned)
            .or_else(|| process_agent(&observation));
        details_row.state = terminal_list_state(report, details_row.agent.is_some(), observed);
    }

    if let Some(reports) = &reports {
        let verified = verify_reports(client, snapshot, reports, deadline, signals);
        check_cancelled(signals)?;
        match best_effort(verified)? {
            Some(true) => {}
            Some(false) => {
                return Ok(Attempt::Changed(newer_snapshot(
                    client, snapshot, deadline, signals,
                )?));
            }
            None => return Ok(Attempt::Unverified),
        }
    }
    let refreshed = query::refresh_snapshot(
        client,
        &snapshot.session_id,
        snapshot.revision,
        deadline,
        signals,
    );
    check_cancelled(signals)?;
    Ok(match best_effort(refreshed)? {
        Some(refreshed) if refreshed.revision == snapshot.revision => Attempt::Verified(details),
        Some(refreshed) => Attempt::Changed(Some(refreshed)),
        None => Attempt::Unverified,
    })
}

/// The Session's current snapshot after `snapshot`, when it can still be read.
fn newer_snapshot(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<Snapshot>, Failure> {
    let refreshed = query::refresh_snapshot(
        client,
        &snapshot.session_id,
        snapshot.revision,
        deadline,
        signals,
    );
    check_cancelled(signals)?;
    best_effort(refreshed)
}

fn process_agent(observation: &super::terminal_observation::Observation) -> Option<String> {
    let super::terminal_observation::Observation::Observed(read) = observation else {
        return None;
    };
    read.processes
        .as_ref()
        .and_then(|(_, processes)| processes.foreground().provider.map(str::to_owned))
}

fn terminal_list_state(
    report: Option<&PanelAgentView>,
    agent_identified: bool,
    observed_frame: bool,
) -> Option<TerminalListState> {
    if let Some(report) = report
        && report.state != PanelAgentState::Unknown
    {
        return Some(TerminalListState::Agent(report.state));
    }
    if agent_identified {
        return Some(TerminalListState::Agent(PanelAgentState::Unknown));
    }
    if observed_frame {
        return Some(TerminalListState::Observed);
    }
    None
}

fn read_reports(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    rows: &[results::TerminalRow],
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ReportRead, Failure> {
    let mut reports = BTreeMap::new();
    let mut report_revision = None;
    for chunk in rows.chunks(MAXIMUM_PANEL_AGENT_READ_ROWS) {
        signals.check_before(deadline)?;
        let presentation_ids = chunk
            .iter()
            .map(|row| {
                TerminalPresentationId::parse(&row.presentation_id)
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let request = PanelAgentRequest {
            protocol_version: snapshot.protocol_version,
            request_id: query::request_id()?,
            session_id: snapshot.session_id.clone(),
            action: PanelAgentAction::ReadPresentations {
                presentation_ids: presentation_ids.clone(),
            },
        };
        let outcome = client.panel_agent(request, deadline, signals.poll_fd());
        signals.check_before(deadline)?;
        let result = outcome.map_err(|error| Failure::new(metadata_failure(error.reason)))?;
        match result.outcome {
            PanelAgentOutcome::Failed {
                reason: PanelAgentFailure::Unavailable,
            } if optional_report_unavailable(PanelAgentFailure::Unavailable) => {
                return Ok(ReportRead::Read(None));
            }
            PanelAgentOutcome::Failed { reason } => {
                return Err(panel_agent_failure(reason));
            }
            PanelAgentOutcome::PresentationsRead {
                workbench_revision,
                report_revision: current_report_revision,
                presentations,
            } => {
                if workbench_revision != snapshot.revision
                    || presentations.len() != chunk.len()
                    || report_revision.is_some_and(|revision| revision != current_report_revision)
                {
                    return Ok(ReportRead::Changed);
                }
                report_revision = Some(current_report_revision);
                let expected: BTreeMap<_, _> = chunk
                    .iter()
                    .map(|row| (row.presentation_id.as_str(), row))
                    .collect();
                let mut seen = BTreeSet::<String>::new();
                for presentation in presentations {
                    let id = presentation.presentation_id.as_str();
                    let row = expected
                        .get(id)
                        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
                    let binding = presentation.panel.binding.as_ref();
                    if !seen.insert(id.to_owned())
                        || presentation.panel.panel_id.as_str() != row.panel_id.as_str()
                        || binding.is_none_or(|binding| {
                            binding.terminal_id.as_str() != row.terminal_id.as_str()
                                || binding.worker_id.as_str() != row.worker_id.as_str()
                        })
                    {
                        return Err(Failure::new(FailureKind::ProtocolInvalid));
                    }
                    let report = presentation
                        .panel
                        .report
                        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
                    reports.insert(id.to_owned(), report);
                }
                if seen.len() != chunk.len() {
                    return Err(Failure::new(FailureKind::ProtocolInvalid));
                }
            }
            // This query consumes only its requested metadata/report payload; other receipt classes cannot answer that query.
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        }
    }
    Ok(ReportRead::Read(Some(Reports {
        rows: reports,
        revision: report_revision.ok_or_else(|| Failure::new(FailureKind::InternalFailure))?,
    })))
}

/// Whether the Workbench and its reports still stand where `reports` read them.
fn verify_reports(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    reports: &Reports,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<bool, Failure> {
    signals.check_before(deadline)?;
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        session_id: snapshot.session_id.clone(),
        action: PanelAgentAction::ReadPresentations {
            presentation_ids: vec![],
        },
    };
    let outcome = client.panel_agent(request, deadline, signals.poll_fd());
    signals.check_before(deadline)?;
    let result = outcome.map_err(|error| Failure::new(metadata_failure(error.reason)))?;
    match result.outcome {
        PanelAgentOutcome::PresentationsRead {
            workbench_revision,
            report_revision,
            presentations,
        } if workbench_revision == snapshot.revision
            && report_revision == reports.revision
            && presentations.is_empty() =>
        {
            Ok(true)
        }
        PanelAgentOutcome::PresentationsRead { .. } => Ok(false),
        PanelAgentOutcome::Failed {
            reason: PanelAgentFailure::Unavailable,
        } => Err(Failure::new(FailureKind::ServiceUnavailable)),
        PanelAgentOutcome::Failed { reason } => Err(panel_agent_failure(reason)),
        // This query consumes only its requested metadata/report payload; other receipt classes cannot answer that query.
        _ => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}

fn observation_for(
    outcome: Result<crate::cli::terminal_observation::Read, Failure>,
) -> Result<super::terminal_observation::Observation, Failure> {
    match outcome {
        Ok(read) => Ok(super::terminal_observation::Observation::Observed(
            Box::new(read),
        )),
        Err(error) if error.kind == FailureKind::ServiceUnavailable => {
            Ok(super::terminal_observation::Observation::Unavailable)
        }
        Err(error) => Err(error),
    }
}

fn optional_report_unavailable(reason: PanelAgentFailure) -> bool {
    reason == PanelAgentFailure::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_frame(
        processes: Option<(serde_json::Value, results::PanelProcessInfoDoc)>,
    ) -> super::super::terminal_observation::Observation {
        let route = orchbee_terminal_protocol::TerminalFrameRoute {
            attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
            host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16]).unwrap(),
            sequence: std::num::NonZeroU64::new(1).unwrap(),
            generation: std::num::NonZeroU64::new(1).unwrap(),
            revision: 0,
            base_revision: None,
        };
        super::super::terminal_observation::Observation::Observed(Box::new(
            crate::cli::terminal_observation::Read {
                frame: crate::cli::terminal_observation::Metadata {
                    route,
                    columns: 80,
                    rows: 24,
                    alternate_screen: false,
                    scrollback_rows: 0,
                    title: None,
                    current_directory: None,
                },
                processes,
            },
        ))
    }

    fn terminal_row(terminal_id: String) -> results::TerminalRow {
        results::TerminalRow {
            terminal_id,
            worker_id: "2".repeat(32),
            presentation_id: "3".repeat(32),
            panel_id: "4".repeat(32),
            tab_id: "5".repeat(32),
            terminal_context_id: "6".repeat(32),
        }
    }

    fn observed_frame_with_process_agent() -> super::super::terminal_observation::Observation {
        let attachment_id = orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap();
        let terminal_id = orchbee_terminal_protocol::TerminalId::from_bytes([3; 16]).unwrap();
        let info = orchbee_terminal_protocol::TerminalProcessInfo {
            attachment_id,
            terminal_id,
            root_pid: 10,
            processes: vec![
                orchbee_terminal_protocol::TerminalProcess {
                    pid: 10,
                    parent_pid: None,
                    creation_time: 10,
                    console: orchbee_terminal_protocol::TerminalProcessConsole::Attached,
                    image: Some("cmd.exe".to_owned()),
                    command_line: Some("cmd.exe".to_owned()),
                    current_directory: None,
                },
                orchbee_terminal_protocol::TerminalProcess {
                    pid: 20,
                    parent_pid: Some(10),
                    creation_time: 20,
                    console: orchbee_terminal_protocol::TerminalProcessConsole::Attached,
                    image: Some("codex.exe".to_owned()),
                    command_line: Some("codex.exe".to_owned()),
                    current_directory: None,
                },
            ],
            launch_kind: None,
        };
        observed_frame(Some((
            serde_json::json!({
                "kind": "terminal-process-query",
                "sessionId": "1".repeat(32),
                "snapshotRevision": 1,
                "requestId": "7".repeat(32),
                "hostRequestId": "8".repeat(32),
                "hostEpoch": orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16])
                    .unwrap()
                    .to_text(),
                "attachmentId": attachment_id.to_text(),
                "hostTerminalId": terminal_id.to_text(),
            }),
            // The Agent comes from the Server's published run, not from the sampled codex row.
            results::PanelProcessInfoDoc {
                terminal: terminal_row(terminal_id.to_text()),
                info,
                server: crate::cli::processes::ServerIdentity {
                    identity: orchbee_agent_observation_protocol::AgentProcessIdentity::Agent {
                        provider: orchbee_agent_observation_protocol::AgentReportProvider::Codex,
                        grade: orchbee_agent_observation_protocol::AgentIdentityGrade::Recognized,
                    },
                    anchor: Some(orchbee_agent_observation_protocol::AgentProcessAnchor {
                        pid: 20,
                        provider: orchbee_agent_observation_protocol::AgentReportProvider::Codex,
                    }),
                },
            },
        )))
    }

    #[test]
    fn observed_frame_without_agent_or_process_is_labeled_observed() {
        let observation = observed_frame(None);
        assert!(process_agent(&observation).is_none());
        assert_eq!(
            terminal_list_state(None, false, true),
            Some(TerminalListState::Observed)
        );
    }

    #[test]
    fn explicit_agent_unknown_state_survives_frame_observation() {
        let observation = observed_frame(None);
        let report = PanelAgentView {
            agent: Some("codex".to_owned()),
            state: PanelAgentState::Unknown,
            ..PanelAgentView::default()
        };
        assert_eq!(
            terminal_list_state(Some(&report), true, true),
            Some(TerminalListState::Agent(PanelAgentState::Unknown))
        );
        assert_eq!(
            terminal_list_state(
                Some(&PanelAgentView {
                    state: PanelAgentState::Working,
                    ..PanelAgentView::default()
                }),
                false,
                true
            ),
            Some(TerminalListState::Agent(PanelAgentState::Working))
        );
        assert!(process_agent(&observation).is_none());
    }

    #[test]
    fn empty_unknown_report_uses_observation_or_stays_unknown() {
        let report = PanelAgentView::default();
        assert_eq!(
            terminal_list_state(Some(&report), false, true),
            Some(TerminalListState::Observed)
        );
        assert_eq!(terminal_list_state(Some(&report), false, false), None);
    }

    #[test]
    fn process_only_agent_without_a_known_report_state_stays_unknown() {
        let observation = observed_frame_with_process_agent();
        let agent = process_agent(&observation);
        assert_eq!(agent.as_deref(), Some("codex"));

        for report in [None, Some(PanelAgentView::default())] {
            assert_eq!(
                terminal_list_state(report.as_ref(), agent.is_some(), true),
                Some(TerminalListState::Agent(PanelAgentState::Unknown))
            );
        }
    }

    #[test]
    fn panel_agent_unavailable_is_optional_but_denial_and_transport_faults_are_not() {
        assert_eq!(
            panel_agent_failure(PanelAgentFailure::Denied).kind,
            FailureKind::Denied
        );
        assert_eq!(
            panel_agent_failure(PanelAgentFailure::BindingChanged).kind,
            FailureKind::Rejected
        );
        assert_eq!(
            panel_agent_failure(PanelAgentFailure::Unavailable).kind,
            FailureKind::ServiceUnavailable
        );
        assert!(optional_report_unavailable(PanelAgentFailure::Unavailable));
        assert!(!optional_report_unavailable(PanelAgentFailure::Denied));
        assert_eq!(
            metadata_failure(crate::local_client::MetadataExchangeReason::Read),
            FailureKind::TransportLoss
        );
        assert_eq!(
            metadata_failure(crate::local_client::MetadataExchangeReason::Protocol),
            FailureKind::ProtocolInvalid
        );
    }

    #[test]
    fn terminal_observation_only_suppresses_semantic_unavailability() {
        assert!(matches!(
            observation_for(Err(Failure::new(FailureKind::ServiceUnavailable))),
            Ok(super::super::terminal_observation::Observation::Unavailable)
        ));
        for kind in [FailureKind::Denied, FailureKind::TransportLoss] {
            assert_eq!(
                observation_for(Err(Failure::new(kind))).unwrap_err().kind,
                kind
            );
        }
    }

    #[test]
    fn enrichment_limit_is_finite_and_leaves_the_tail_for_unknown_rendering() {
        let row = results::TerminalRow {
            terminal_id: "1".repeat(32),
            worker_id: "2".repeat(32),
            presentation_id: "3".repeat(32),
            panel_id: "4".repeat(32),
            tab_id: "5".repeat(32),
            terminal_context_id: "6".repeat(32),
        };
        let rows = vec![row; MAXIMUM_ENRICHED_TERMINALS + 7];
        assert_eq!(sampled_rows(&rows).len(), MAXIMUM_ENRICHED_TERMINALS);
        assert_eq!(rows.len() - sampled_rows(&rows).len(), 7);
    }
}
