//! Executes admitted terminal creation workflows by resolving captured targets, submitting
//! canonical operations, and observing requested Client presentation under one budget. It does
//! not invent membership from local state or roll back unknown physical effects.

use super::{
    receipt::{Receipt, applied},
    targeting::{self, Created},
};
use crate::{
    cli::{
        Failure, FailureKind, Output,
        caller_context::PanelTarget,
        client_control::{Answer, exchange as control, selection},
        command_interrupt::{ArmedSignals, Finish},
        console_interrupt::SignalWatch,
        failure_output, mutation, query,
    },
    invocation::{CreationCommand, CreationKind, OutputFormat, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ClientSplitPath, ClientViewAction, ClientViewAvailability,
    ClientViewControl, ClientViewIntent, ClientViewOutcome, ClientViewPhase, ClientViewRecord,
    PanelNode, ProtocolVersion, SessionId, Snapshot, SnapshotReadIntent, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchRevision, WorkbenchSnapshotOutcome, WorkbenchSplitId,
    WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) fn run(command: &CreationCommand, format: OutputFormat, locale: Locale) -> Output {
    let deadline = Instant::now() + Duration::from_millis(u64::from(command.timeout_ms));
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
    let mut receipt = None;
    let mut result = execute(command, deadline, &mut signals, &mut receipt);
    let mut finalization = None;
    let mut cancelled = false;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            cancelled = true;
            // A cancellation exit must not erase an uncertain canonical submission.
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) => {
            if !matches!(&result, Err(error) if error.kind == FailureKind::OutcomeUnknown) {
                result = Err(Failure::new(FailureKind::InternalFailure));
            }
        }
    }
    let mut output = if let Some(mut receipt) = receipt {
        if let Err(error) = result {
            receipt.failure = Some(error.kind);
        }
        receipt.emit(format, locale)
    } else {
        failure_output(result.err().unwrap_or_else(internal), Some(format), locale)
    };
    if cancelled {
        output.status = 130;
    }
    output.finalization = finalization;
    output
}
mod overlay;
fn internal() -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
pub(super) fn check(signals: &mut ArmedSignals, deadline: Instant) -> Result<(), Failure> {
    if signals.take().map_err(|_| internal())?.is_some() {
        Err(Failure::new(FailureKind::Cancelled))
    } else if Instant::now() >= deadline {
        Err(Failure::new(FailureKind::Timeout))
    } else {
        Ok(())
    }
}

fn execute(
    command: &CreationCommand,
    deadline: Instant,
    signals: &mut ArmedSignals,
    output: &mut Option<Receipt>,
) -> Result<(), Failure> {
    check(signals, deadline)?;
    let ratio = match command.kind {
        CreationKind::Split { ratio, .. } => ratio,
        _ => None,
    };
    let caller = if command.client.is_none()
        && (needs_client(command) || targeting::needs_source(command))
    {
        PanelTarget::prepare_optional(|name| std::env::var_os(name))?
    } else {
        None
    };
    let (mut client, _, mut snapshot) = query::snapshot_session_until(deadline)?;
    let mut configured = command.clone();
    if configured.options.plugin_panel.is_none() {
        crate::cli::creation_defaults::apply_directory(
            &mut client,
            &snapshot.session_id,
            deadline,
            signals,
            &mut configured.directory,
            &mut configured.options,
        )?;
    }
    let command = &configured;
    // One correlation identity names the logical creation, so preparing it again derives the
    // same plugin context.
    let correlation = query::request_id()?;
    let (prepared, canonical) = mutation::submit_creation(
        &mut client,
        &mut snapshot,
        deadline,
        signals,
        |client, snapshot, signals| {
            let (prepared, canonical) = prepare(
                command,
                caller.as_ref(),
                correlation.as_str(),
                client,
                snapshot,
                deadline,
                signals,
            )?;
            // Besides the request itself, a resubmission must find the same Client, overlay
            // return and directory provenance.
            let derived = (
                prepared
                    .selected
                    .as_ref()
                    .map(|record| record.client_id.clone()),
                prepared.overlay_return.clone(),
                prepared.directory.document(),
            );
            Ok((prepared, canonical, derived))
        },
    )?;
    let Prepared {
        selected,
        overlay_return,
        directory,
    } = prepared;
    *output = Some(Receipt {
        directory,
        canonical,
        client_id: selected.as_ref().map(|client| client.client_id.clone()),
        requested_ratio: ratio,
        requested_focus: command.focus,
        requested_maximize: command.zoom,
        requested_overlay: command.overlay,
        ratio: None,
        focus: None,
        stage: "created",
        failure: None,
    });
    let receipt = output.as_mut().ok_or_else(internal)?;
    check(signals, deadline)?;
    if ratio.is_none() && !command.focus {
        receipt.stage = "complete";
        return Ok(());
    }
    let selected = selected.as_ref().ok_or_else(internal)?;
    let revision = receipt
        .canonical
        .committed_revision()
        .ok_or_else(internal)?;
    let created = created_from_receipt(
        &receipt.canonical,
        matches!(command.kind, CreationKind::Split { .. }),
    )?;
    if let Some(ratio) = ratio {
        let get = |name| receipt.canonical.id_value(name).ok_or_else(internal);
        let identity = ClientSplitIdentity {
            split_id: WorkbenchSplitId::parse(get("splitId")?).map_err(|_| internal())?,
            first_slot_id: WorkbenchPanelSlotId::parse(get("firstSlotId")?)
                .map_err(|_| internal())?,
            second_slot_id: WorkbenchPanelSlotId::parse(get("secondSlotId")?)
                .map_err(|_| internal())?,
        };
        receipt.stage = "ratio-resolution";
        let fresh = read_snapshot(
            &mut client,
            &snapshot.session_id,
            revision,
            deadline,
            signals,
        )?;
        let tree = fresh
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == created.context)
            .and_then(|context| context.tabs.iter().find(|tab| tab.tab_id == created.tab))
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        let path = split_path(&tree.root, &identity, &mut vec![])
            .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
        let current = observe(
            &mut client,
            &snapshot.session_id,
            selected,
            &created,
            fresh.revision,
            deadline,
            signals,
        )?;
        receipt.stage = "ratio";
        receipt.ratio = Some(control::submit_control(
            &mut client,
            &snapshot.session_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: selected.client_id.clone(),
                expected_view_revision: current.view_revision,
                minimum_workbench_revision: fresh.revision,
                intent: ClientViewIntent::SetSplitRatio {
                    terminal_context_id: created.context.clone(),
                    tab_id: created.tab.clone(),
                    path,
                    ratio,
                    expected_split: Some(identity),
                },
            },
            deadline,
            signals,
        )?);
        if !receipt
            .ratio
            .as_ref()
            .is_some_and(|answer| applied(answer, true))
        {
            return Ok(());
        }
    }
    if command.focus {
        let focus = match overlay_return {
            Some(return_to) => Focus::Overlay(return_to),
            None if command.zoom => Focus::Zoom,
            None => Focus::Panel,
        };
        let answer = focus_created(
            &mut client,
            &snapshot.session_id,
            FocusAim {
                selected,
                created: &created,
                revision,
                focus,
            },
            deadline,
            signals,
        )
        .map_err(|stop| {
            receipt.stage = stop.stage;
            stop.failure
        })?;
        receipt.focus = Some(answer);
        receipt.stage = "focus";
        if !receipt
            .focus
            .as_ref()
            .is_some_and(|answer| applied(answer, false))
        {
            return Ok(());
        }
    }
    receipt.stage = "complete";
    Ok(())
}

fn read_snapshot(
    client: &mut LocalClient,
    session: &SessionId,
    minimum: WorkbenchRevision,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Snapshot, Failure> {
    check(signals, deadline)?;
    let result = client.submit_workbench_snapshot_read_before(
        SnapshotReadIntent {
            protocol_version: ProtocolVersion,
            request_id: query::request_id()?,
        },
        deadline,
    );
    check(signals, deadline)?;
    match result
        .map_err(|_| Failure::new(FailureKind::ServiceUnavailable))?
        .outcome
    {
        WorkbenchSnapshotOutcome::Available { snapshot }
            if &snapshot.session_id == session && snapshot.revision >= minimum =>
        {
            Ok(snapshot)
        }
        WorkbenchSnapshotOutcome::Available { .. } => {
            Err(Failure::new(FailureKind::ProtocolInvalid))
        }
        WorkbenchSnapshotOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        WorkbenchSnapshotOutcome::Unavailable { .. } => {
            Err(Failure::new(FailureKind::ServiceUnavailable))
        }
    }
}

/// What one attempt prepared from its snapshot besides the canonical request.
struct Prepared {
    selected: Option<ClientViewRecord>,
    overlay_return: Option<orchbee_workbench_protocol::ClientOverlayMember>,
    directory: super::directory::Resolution,
}

/// Whether routing, a ratio or focus needs one selected Client.
fn needs_client(command: &CreationCommand) -> bool {
    command.client.is_some()
        || command.focus
        || matches!(command.kind, CreationKind::Split { ratio: Some(_), .. })
}

fn created_from_receipt(receipt: &mutation::Receipt, split: bool) -> Result<Created, Failure> {
    let get = |name| receipt.id_value(name).ok_or_else(internal);
    Ok(Created {
        context: WorkbenchTerminalContextId::parse(get("terminalContextId")?)
            .map_err(|_| internal())?,
        tab: WorkbenchTabId::parse(get("tabId")?).map_err(|_| internal())?,
        panel: WorkbenchPanelId::parse(get(if split { "newPanelId" } else { "panelId" })?)
            .map_err(|_| internal())?,
        split,
    })
}

pub(in crate::cli) fn select_ready_client(
    client: &mut LocalClient,
    session: &SessionId,
    explicit: Option<&orchbee_workbench_protocol::WorkbenchClientId>,
    caller: Option<&Target>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    let reply = control::exchange(
        client,
        session,
        ClientViewAction::List {},
        deadline,
        signals,
    )?;
    let clients = match reply.outcome {
        ClientViewOutcome::Listed { clients } => clients,
        ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
        ClientViewOutcome::Unavailable { .. } => {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        ClientViewOutcome::Synchronized { .. }
        | ClientViewOutcome::Found { .. }
        | ClientViewOutcome::Operation { .. }
        | ClientViewOutcome::NotFound { .. }
        | ClientViewOutcome::Conflict { .. } => {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
    };
    let selected = selection::select(&clients, explicit, caller)?.clone();
    if selected.view.phase != ClientViewPhase::Ready {
        return Err(Failure::new(FailureKind::ServiceUnavailable));
    }
    Ok(selected)
}

/// The Client view a focus request asks for. An overlay return takes precedence over zoom.
enum Focus {
    Panel,
    Zoom,
    Overlay(orchbee_workbench_protocol::ClientOverlayMember),
}

/// A focus request that stopped before its answer: the stage it reached and the failure.
struct FocusStop {
    stage: &'static str,
    failure: Failure,
}

/// What a focus request aims at: the Client that shows the view, the created Panel, the revision
/// its creation committed, and the view to show.
struct FocusAim<'a> {
    selected: &'a ClientViewRecord,
    created: &'a Created,
    revision: WorkbenchRevision,
    focus: Focus,
}

/// The focus request `execute` and `agent start` both make: observe the Client, then submit the
/// intent for the created Panel. The answer comes back whether or not it applied, so each caller
/// decides what a miss means.
fn focus_created(
    client: &mut LocalClient,
    session: &SessionId,
    aim: FocusAim<'_>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Answer, FocusStop> {
    let FocusAim {
        selected,
        created,
        revision,
        focus,
    } = aim;
    let current = observe(
        client, session, selected, created, revision, deadline, signals,
    )
    .map_err(|failure| FocusStop {
        stage: "focus-observation",
        failure,
    })?;
    check(signals, deadline).map_err(|failure| FocusStop {
        stage: "focus-observation",
        failure,
    })?;
    let intent = match focus {
        Focus::Overlay(return_to) => {
            let fresh =
                read_snapshot(client, session, revision, deadline, signals).map_err(|failure| {
                    FocusStop {
                        stage: "focus",
                        failure,
                    }
                })?;
            overlay::intent(&fresh, created, return_to, selected).map_err(|failure| FocusStop {
                stage: "focus",
                failure,
            })?
        }
        Focus::Zoom => ClientViewIntent::Zoom {
            terminal_context_id: created.context.clone(),
            tab_id: created.tab.clone(),
            panel_id: created.panel.clone(),
            enabled: true,
        },
        Focus::Panel => ClientViewIntent::Panel {
            terminal_context_id: created.context.clone(),
            tab_id: created.tab.clone(),
            panel_id: created.panel.clone(),
        },
    };
    let operation_id = query::request_id().map_err(|failure| FocusStop {
        stage: "focus",
        failure,
    })?;
    control::submit_control(
        client,
        session,
        ClientViewControl {
            operation_id,
            client_id: selected.client_id.clone(),
            expected_view_revision: current.view_revision,
            minimum_workbench_revision: revision,
            intent,
        },
        deadline,
        signals,
    )
    .map_err(|failure| FocusStop {
        stage: "focus",
        failure,
    })
}

/// A focus that did not show the created Panel. The Agent still runs in that Panel, so `agent
/// start` reports the miss beside its result; `answer` keeps the Client's own answer when one came
/// back.
#[derive(Debug)]
pub(in crate::cli) struct FocusMiss {
    stage: &'static str,
    reason: FailureKind,
    answer: Option<serde_json::Value>,
}

impl FocusMiss {
    /// The miss as `result.focus` or `details.focus` names it.
    pub(in crate::cli) fn document(&self) -> serde_json::Value {
        serde_json::json!({
            "stage": self.stage,
            "reason": self.reason.as_str(),
            "answer": self.answer,
        })
    }
}

/// A focus that halted the start: the failure, and the Client's answer when one came back first,
/// so the failure can still name what the Client answered.
pub(in crate::cli) struct FocusHalt {
    pub(in crate::cli) failure: Failure,
    pub(in crate::cli) miss: Option<FocusMiss>,
}

/// A [`FocusHalt`] with no Client answer, boxed so the `Result` it travels in stays small.
fn halt(failure: Failure) -> Box<FocusHalt> {
    Box::new(FocusHalt {
        failure,
        miss: None,
    })
}

/// The typed reason a focus answer did not apply, read from the Client's own wait and observation
/// labels. Exit codes are never mapped to kinds, since one code can name more than one fault.
fn answer_reason(answer: &Answer) -> FailureKind {
    let document = answer.document();
    match document["result"]["wait"].as_str() {
        Some("transport-lost") => FailureKind::TransportLoss,
        Some("timed-out") => FailureKind::Timeout,
        Some("cancelled") => FailureKind::Cancelled,
        Some("observation-failed") => FailureKind::InternalFailure,
        _ if document["result"]["observation"]["kind"] == "unknown" => FailureKind::OutcomeUnknown,
        _ => FailureKind::Rejected,
    }
}

/// Focus the Panel `agent start` created. A focus that does not show it is a [`FocusMiss`] and
/// does not fail the start. A cancelled, expired or transport-lost focus halts it, as it stops
/// everything else; the [`FocusHalt`] keeps the Panel the start created, so the receipt stays,
/// and the Client's answer when it came back.
pub(in crate::cli) fn focus_new_panel(
    client: &mut LocalClient,
    session: &SessionId,
    selected: &ClientViewRecord,
    receipt: &mutation::Receipt,
    split: bool,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<FocusMiss>, Box<FocusHalt>> {
    let created = created_from_receipt(receipt, split).map_err(halt)?;
    let revision = receipt
        .committed_revision()
        .ok_or_else(internal)
        .map_err(halt)?;
    let miss = match focus_created(
        client,
        session,
        FocusAim {
            selected,
            created: &created,
            revision,
            focus: Focus::Panel,
        },
        deadline,
        signals,
    ) {
        Ok(answer) if applied(&answer, false) => return Ok(None),
        Ok(answer) => FocusMiss {
            stage: "focus",
            reason: answer_reason(&answer),
            answer: Some(answer.document()),
        },
        Err(stop) if fatal(stop.failure.kind) => return Err(halt(stop.failure)),
        Err(stop) => FocusMiss {
            stage: stop.stage,
            reason: stop.failure.kind,
            answer: None,
        },
    };
    if fatal(miss.reason) {
        return Err(Box::new(FocusHalt {
            failure: Failure::new(miss.reason),
            miss: Some(miss),
        }));
    }
    Ok(Some(miss))
}

/// A focus that ends in one of these halts the start. Any other miss leaves the Agent running in
/// the created Panel, so the start reports it instead of failing.
fn fatal(kind: FailureKind) -> bool {
    matches!(
        kind,
        FailureKind::Cancelled | FailureKind::Timeout | FailureKind::TransportLoss
    )
}

/// Prepare the canonical creation and everything it takes from `snapshot`: the caller and Client
/// targets, the label, the directory, the overlay return and the plugin context. A stale-view
/// retry prepares all of it again from a fresh snapshot (ORC-413).
fn prepare(
    command: &CreationCommand,
    caller: Option<&PanelTarget>,
    correlation: &str,
    client: &mut LocalClient,
    snapshot: &Snapshot,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(Prepared, crate::invocation::MutationCommand), Failure> {
    let caller = caller.map(|caller| caller.resolve(snapshot)).transpose()?;
    check(signals, deadline)?;
    let selected =
        if needs_client(command) || (targeting::needs_source(command) && caller.is_none()) {
            Some(select_ready_client(
                client,
                &snapshot.session_id,
                command.client.as_ref(),
                caller.as_ref(),
                deadline,
                signals,
            )?)
        } else {
            None
        };
    let mut canonical = targeting::resolve(
        command,
        snapshot,
        if command.overlay {
            None
        } else {
            caller.as_ref()
        },
        selected.as_ref(),
    )?;
    let (overlay_return, overlay_source) = if command.overlay {
        let selected = selected.as_ref().ok_or_else(internal)?;
        let panel = selected
            .view
            .selection
            .as_ref()
            .and_then(|s| s.panel_id.clone())
            .ok_or_else(internal)?;
        let answer = control::submit_control(
            client,
            &snapshot.session_id,
            ClientViewControl {
                operation_id: query::request_id()?,
                client_id: selected.client_id.clone(),
                expected_view_revision: selected.view_revision,
                minimum_workbench_revision: snapshot.revision,
                intent: ClientViewIntent::Presentation { panel_id: panel },
            },
            deadline,
            signals,
        )?;
        let member = match answer.reply.as_ref().map(|reply| &reply.outcome) {
            Some(ClientViewOutcome::Operation { operation }) => match &operation.status {
                orchbee_workbench_protocol::ClientViewControlStatus::PresentationRead {
                    presentation,
                    ..
                } => presentation
                    .presentation
                    .as_ref()
                    .ok_or_else(|| Failure::new(FailureKind::ServiceUnavailable))?,
                orchbee_workbench_protocol::ClientViewControlStatus::PopupOpened { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::PopupClosed { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::ViewportRead { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::NavigationRead { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::TerminalAttentionRead {
                    ..
                }
                | orchbee_workbench_protocol::ClientViewControlStatus::AttentionRead { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::LayoutAdjusted { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::DirectionFocused {
                    ..
                }
                | orchbee_workbench_protocol::ClientViewControlStatus::DirectionUnchanged {
                    ..
                }
                | orchbee_workbench_protocol::ClientViewControlStatus::LayoutRead { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::Queued { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::Delivered { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::Applied { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::ZoomUnchanged { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::ZoomToggled { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::Rejected { .. }
                | orchbee_workbench_protocol::ClientViewControlStatus::Incomplete { .. } => {
                    return Err(Failure::new(FailureKind::ServiceUnavailable));
                }
            },
            // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
            _ => return Err(Failure::new(FailureKind::ServiceUnavailable)),
        };
        (
            Some(overlay::capture_return(snapshot, selected, member)?),
            Some(member.clone()),
        )
    } else {
        (None, None)
    };
    let directory = super::directory::resolve(
        command,
        &mut canonical,
        snapshot,
        (caller.as_ref(), selected.as_ref()),
        client,
        deadline,
        signals,
    )?;
    if command.options.plugin_panel.is_some() {
        let (context, observed_member) = crate::cli::plugin_preparation::context::observe(
            client,
            snapshot,
            crate::cli::plugin_preparation::context::Source::creation(
                &canonical,
                snapshot,
                command.overlay,
                selected.as_ref().map(|record| &record.client_id),
            )?,
            Default::default(),
            correlation,
            deadline,
            signals,
        )
        .map_err(crate::cli::plugin_preparation::failure::client_refusal)?;
        if let Some(pinned) = overlay_source
            && !observed_member
                .as_ref()
                .is_some_and(|member| pinned.same_member(member))
        {
            return Err(crate::cli::query::workbench_failure("client-changed"));
        }
        if let Some(original) = &selected {
            // The source reads may move the Client's view revision; only its selection fences
            // the plugin context.
            crate::cli::client_control::view_fence::ViewFence::Selection.next_record(
                client,
                &snapshot.session_id,
                original,
                deadline,
                signals,
            )?;
        }
        match &mut canonical {
            crate::invocation::MutationCommand::TabCreate { options, .. }
            | crate::invocation::MutationCommand::PanelSplit { options, .. } => {
                options.plugin_context = Some(Box::new(context));
            }
            _ => return Err(internal()),
        }
    }
    check(signals, deadline)?;
    Ok((
        Prepared {
            selected,
            overlay_return,
            directory,
        },
        canonical,
    ))
}

fn split_path(
    node: &PanelNode,
    identity: &ClientSplitIdentity,
    path: &mut Vec<u8>,
) -> Option<ClientSplitPath> {
    let PanelNode::Split {
        split_id, children, ..
    } = node
    else {
        return None;
    };
    if *split_id == identity.split_id {
        return (children[0].slot_id == identity.first_slot_id
            && children[1].slot_id == identity.second_slot_id)
            .then(|| ClientSplitPath::new(path.clone()).ok())
            .flatten();
    }
    for (index, child) in children.iter().enumerate() {
        path.push(u8::try_from(index).ok()?);
        let found = split_path(&child.node, identity, path);
        path.pop();
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Waits until the original Client shows the creation's commit and returns that record, whose view
/// revision a later control of the created Panel carries: installing the commit steps the Client's
/// view revision. Only the count increase this creation caused may change the selection.
pub(in crate::cli) fn observe(
    client: &mut LocalClient,
    session: &SessionId,
    original: &ClientViewRecord,
    created: &Created,
    minimum: WorkbenchRevision,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<ClientViewRecord, Failure> {
    loop {
        check(signals, deadline)?;
        let reply = control::exchange(
            client,
            session,
            ClientViewAction::Get {
                client_id: original.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        let current = match reply.outcome {
            ClientViewOutcome::Found { client } => *client,
            ClientViewOutcome::Denied {} => return Err(Failure::new(FailureKind::Denied)),
            ClientViewOutcome::NotFound {} | ClientViewOutcome::Unavailable { .. } => {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            ClientViewOutcome::Synchronized { .. }
            | ClientViewOutcome::Listed { .. }
            | ClientViewOutcome::Operation { .. }
            | ClientViewOutcome::Conflict { .. } => {
                return Err(Failure::new(FailureKind::ProtocolInvalid));
            }
        };
        if current.availability != ClientViewAvailability::Current
            || current.view.phase == ClientViewPhase::Unavailable
        {
            return Err(Failure::new(FailureKind::ServiceUnavailable));
        }
        let can_refresh_empty = current.view.workbench_revision >= original.view.workbench_revision
            && targeting::unchanged_empty_selection(
                &original.view.selection,
                &current.view.selection,
            );
        if current.view.phase == ClientViewPhase::Ready
            && (current.view.workbench_revision >= minimum || can_refresh_empty)
        {
            if !targeting::unchanged_selection(
                &original.view.selection,
                &current.view.selection,
                created,
            ) {
                return Err(Failure::new(FailureKind::Denied));
            }
            return Ok(current);
        }
        thread::sleep(
            Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}
