//! Human-only terminal-list enrichment on one admitted Session and deadline.
use super::{
    Failure, FailureKind, Output, failure_output, query,
    results::{self, Projection, TerminalListDetails, TerminalListState},
    source::{ArmedSignals, Finish, SignalWatch},
    success_output,
};
use crate::{
    invocation::{OutputFormat, QueryCommand, Target},
    local_client::{LocalClient, workbench::metadata_exchange::MetadataExchangeReason},
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
/// after this prefix render unknown enrichment values.
const MAXIMUM_ENRICHED_TERMINALS: usize = 64;

struct Reports {
    rows: BTreeMap<String, PanelAgentView>,
    revision: WorkbenchRevision,
}

fn sampled_rows(rows: &[results::TerminalRow]) -> &[results::TerminalRow] {
    &rows[..rows.len().min(MAXIMUM_ENRICHED_TERMINALS)]
}

pub(crate) fn run(command: QueryCommand, format: OutputFormat, locale: Locale) -> Output {
    if format == OutputFormat::Json {
        // Keep the established machine path byte-for-byte and avoid optional
        // observations for callers that asked for identity JSON only.
        return super::network_query(&command, format, locale);
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
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    signals.check_before(deadline)?;
    let Projection::TerminalList(results::TerminalListDocument { rows, details }) =
        results::project(command, &snapshot)
            .map_err(|failure| failure.source(results::snapshot_source(&snapshot)))?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let details = observe_rows(&mut client, &snapshot, &rows, details, deadline, signals)?;
    let refreshed = query::refresh_snapshot(
        &mut client,
        &snapshot.site_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    if refreshed.revision != snapshot.revision {
        return Err(super::workbench_agents::failure("workbench-changed"));
    }
    signals.check_before(deadline)?;
    Ok((
        results::snapshot_source(&snapshot),
        Projection::TerminalListHuman(results::TerminalListDocument { rows, details }),
    ))
}

fn observe_rows(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    rows: &[results::TerminalRow],
    mut details: Vec<TerminalListDetails>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Vec<TerminalListDetails>, Failure> {
    let sampled = sampled_rows(rows);
    let reports = read_reports(client, snapshot, sampled, deadline, signals)?;
    details.resize_with(rows.len(), TerminalListDetails::default);

    for (index, row) in sampled.iter().enumerate() {
        signals.check_before(deadline)?;
        let outcome = query::terminal_metadata::read(
            client,
            snapshot,
            &Target::Id(row.terminal_id.clone()),
            deadline,
            signals.poll_fd(),
        );
        // Local I/O can surface cancellation or expiry as a transport error;
        // preserve the command's own total-deadline decision on that path.
        signals.check_before(deadline)?;
        let observation = observation_for(outcome)?;
        let fields = observation.fields();
        let report = reports
            .as_ref()
            .and_then(|reports| reports.rows.get(&row.presentation_id));
        let observed = matches!(
            observation,
            super::terminal_observation::Observation::Observed(_)
        );
        let details_row = &mut details[index];
        details_row.title = fields["terminalTitleStripped"].as_str().map(str::to_owned);
        details_row.cwd = fields["cwd"].as_str().map(str::to_owned);
        details_row.agent = report
            .and_then(|report| report.agent.as_deref())
            .map(str::to_owned)
            .or_else(|| process_agent(&observation));
        details_row.state = terminal_list_state(report, details_row.agent.is_some(), observed);
    }
    if let Some(reports) = &reports {
        verify_reports(client, snapshot, reports, deadline, signals)?;
    }
    Ok(details)
}

fn process_agent(observation: &super::terminal_observation::Observation) -> Option<String> {
    let super::terminal_observation::Observation::Observed(read) = observation else {
        return None;
    };
    read.processes.as_ref().and_then(|(_, processes)| {
        super::processes::interpret(&processes.info)
            .foreground
            .provider
            .map(str::to_owned)
    })
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
) -> Result<Option<Reports>, Failure> {
    if rows.is_empty() {
        return Ok(None);
    }
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
            site_id: snapshot.site_id.clone(),
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
                return Ok(None);
            }
            PanelAgentOutcome::Failed { reason } => {
                return Err(panel_agent_failure(reason));
            }
            PanelAgentOutcome::PresentationsRead {
                workbench_revision,
                report_revision: current_report_revision,
                presentations,
            } => {
                if workbench_revision != snapshot.revision || presentations.len() != chunk.len() {
                    return Err(super::workbench_agents::failure("workbench-changed"));
                }
                if report_revision.is_some_and(|revision| revision != current_report_revision) {
                    return Err(super::workbench_agents::failure("metadata-changed"));
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
            _ => return Err(Failure::new(FailureKind::ProtocolInvalid)),
        }
    }
    Ok(Some(Reports {
        rows: reports,
        revision: report_revision.ok_or_else(|| Failure::new(FailureKind::InternalFailure))?,
    }))
}

fn verify_reports(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    reports: &Reports,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(), Failure> {
    signals.check_before(deadline)?;
    let request = PanelAgentRequest {
        protocol_version: snapshot.protocol_version,
        request_id: query::request_id()?,
        site_id: snapshot.site_id.clone(),
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
            Ok(())
        }
        PanelAgentOutcome::Failed {
            reason: PanelAgentFailure::Unavailable,
        } => Err(Failure::new(FailureKind::ServiceUnavailable)),
        PanelAgentOutcome::Failed { reason } => Err(panel_agent_failure(reason)),
        _ => Err(super::workbench_agents::failure("metadata-changed")),
    }
}

fn observation_for(
    outcome: Result<query::terminal_metadata::Read, Failure>,
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

fn panel_agent_failure(reason: PanelAgentFailure) -> Failure {
    Failure::new(match reason {
        PanelAgentFailure::Denied => FailureKind::Denied,
        PanelAgentFailure::NotFound
        | PanelAgentFailure::SiteMismatch
        | PanelAgentFailure::BindingChanged => FailureKind::Rejected,
        PanelAgentFailure::Unbound
        | PanelAgentFailure::InvalidRequest
        | PanelAgentFailure::SourceLimit
        | PanelAgentFailure::HistoryLimit
        | PanelAgentFailure::ResourceLimit => FailureKind::ProtocolInvalid,
        PanelAgentFailure::RevisionExhausted | PanelAgentFailure::ClockOverflow => {
            FailureKind::InternalFailure
        }
        PanelAgentFailure::Unavailable => FailureKind::ServiceUnavailable,
        PanelAgentFailure::ResultTooLarge => FailureKind::OutputTooLarge,
    })
}

fn metadata_failure(reason: MetadataExchangeReason) -> FailureKind {
    match reason {
        MetadataExchangeReason::ConnectionUnavailable => FailureKind::ServiceUnavailable,
        MetadataExchangeReason::Encoding => FailureKind::InternalFailure,
        MetadataExchangeReason::Deadline | MetadataExchangeReason::ResponseDeadline => {
            FailureKind::Timeout
        }
        MetadataExchangeReason::Correlation | MetadataExchangeReason::Protocol => {
            FailureKind::ProtocolInvalid
        }
        MetadataExchangeReason::NotSubmitted
        | MetadataExchangeReason::Write
        | MetadataExchangeReason::Read => FailureKind::TransportLoss,
    }
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
            query::terminal_metadata::Read {
                frame: query::terminal_metadata::Metadata {
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
                    image: Some("cmd.exe".to_owned()),
                    command_line: Some("cmd.exe".to_owned()),
                    current_directory: None,
                },
                orchbee_terminal_protocol::TerminalProcess {
                    pid: 20,
                    parent_pid: Some(10),
                    creation_time: 20,
                    image: Some("codex.exe".to_owned()),
                    command_line: Some("codex.exe".to_owned()),
                    current_directory: None,
                },
            ],
        };
        observed_frame(Some((
            serde_json::json!({
                "kind": "terminal-process-query",
                "siteId": "1".repeat(32),
                "snapshotRevision": 1,
                "requestId": "7".repeat(32),
                "hostRequestId": "8".repeat(32),
                "hostEpoch": orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16])
                    .unwrap()
                    .to_text(),
                "attachmentId": attachment_id.to_text(),
                "hostTerminalId": terminal_id.to_text(),
            }),
            results::PanelProcessInfoDoc {
                terminal: terminal_row(terminal_id.to_text()),
                info,
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
            metadata_failure(MetadataExchangeReason::Read),
            FailureKind::TransportLoss
        );
        assert_eq!(
            metadata_failure(MetadataExchangeReason::Protocol),
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
