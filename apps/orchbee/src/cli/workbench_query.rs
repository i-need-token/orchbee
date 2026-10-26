//! Read-only Workbench queries with Agent facts and optional exact-Client memory.
use super::client_control::exchange as control;
use super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish, SignalWatch},
    failure_output,
    output::{Bounded, TooLarge},
    query,
    results::{self, Projection},
    success_output,
};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, QueryCommand},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientContextSelection, ClientViewAction, ClientViewAvailability, ClientViewControl,
    ClientViewControlStatus, ClientViewIntent, ClientViewOutcome, ClientViewPhase,
    ClientViewRecord, MAXIMUM_CLIENT_SELECTION_CONTEXTS, Snapshot, WorkbenchClientId,
    WorkbenchRequestId, WorkbenchTerminalContextId,
};
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(crate) struct Document {
    pub(super) projection: Box<Projection>,
    client: Option<ClientViewRecord>,
    current_member: Option<super::current_presentation::Resolved>,
    agents: Option<super::workbench_agents::Document>,
    terminals: Option<super::workbench_terminals::Document>,
    viewports: Option<super::workbench_viewports::Document>,
    worktrees: Option<super::workbench_worktrees::Document>,
    metadata: Option<super::workbench_metadata::Document>,
    panel_metadata: Option<super::panel_metadata_query::Document>,
    operations: Vec<WorkbenchRequestId>,
    contexts: Vec<ClientContextSelection>,
    panel_table: Option<super::panel_list::Document>,
}

impl Document {
    fn new(projection: Projection) -> Self {
        Self {
            projection: Box::new(projection),
            client: None,
            current_member: None,
            agents: None,
            terminals: None,
            viewports: None,
            worktrees: None,
            metadata: None,
            panel_metadata: None,
            operations: vec![],
            contexts: vec![],
            panel_table: None,
        }
    }
}

fn failure(kind: FailureKind, client: &WorkbenchClientId) -> Failure {
    Failure::new(kind)
        .failure_schema("orchbee.cli.failure.client-query")
        .details(Map::from_iter([
            ("stage".into(), json!("client-query")),
            ("clientId".into(), json!(client)),
        ]))
}

pub(crate) fn run(
    command: &QueryCommand,
    selected: Option<&WorkbenchClientId>,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) = super::execution::admit(super::execution::Capability::WorkbenchInspection)
    {
        return failure_output(failure, Some(format), locale);
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                selected.map_or_else(
                    || Failure::new(FailureKind::InternalFailure),
                    |id| failure(FailureKind::InternalFailure, id),
                ),
                Some(format),
                locale,
            );
        }
    };
    let result = execute(command, selected, format, deadline, &mut signals);
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
        Ok((source, document)) => success_output(
            Projection::WorkbenchQuery(Box::new(document)),
            source,
            command,
            format,
            locale,
        ),
        Err(error) => {
            // Preserve canonical target ambiguity and admission diagnostics.
            let error = if error.details.is_some() || error.failure_schema.is_some() {
                error
            } else if let Some(selected) = selected {
                failure(error.kind, selected)
            } else {
                error
            };
            failure_output(error, Some(format), locale)
        }
    };
    output.finalization = finalization;
    output
}

fn current(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    selected: &WorkbenchClientId,
    deadline: Instant,
    signals: &mut impl SignalWatch,
    allow_busy: bool,
) -> Result<ClientViewRecord, Failure> {
    signals.check_before(deadline)?;
    let result = control::exchange(
        client,
        &snapshot.session_id,
        ClientViewAction::Get {
            client_id: selected.clone(),
        },
        deadline,
        signals,
    )?;
    signals.check_before(deadline)?;
    match result.outcome {
        ClientViewOutcome::Found { client } if client.client_id == *selected => {
            if client.availability != ClientViewAvailability::Current
                || !(client.view.phase == ClientViewPhase::Ready
                    || (allow_busy && client.view.phase == ClientViewPhase::Busy))
            {
                return Err(Failure::new(FailureKind::ServiceUnavailable));
            }
            Ok(*client)
        }
        ClientViewOutcome::NotFound {} => Err(Failure::new(FailureKind::NotFound)),
        ClientViewOutcome::Denied {} => Err(Failure::new(FailureKind::Denied)),
        ClientViewOutcome::Unavailable { .. } => Err(Failure::new(FailureKind::ServiceUnavailable)),
        ClientViewOutcome::Synchronized { .. }
        | ClientViewOutcome::Listed { .. }
        | ClientViewOutcome::Found { .. }
        | ClientViewOutcome::Operation { .. }
        | ClientViewOutcome::Conflict { .. } => Err(Failure::new(FailureKind::ProtocolInvalid)),
    }
}

fn requested_contexts(
    projection: &Projection,
    selected: &ClientViewRecord,
) -> Result<Vec<WorkbenchTerminalContextId>, Failure> {
    let ids: Vec<&str> = match projection {
        Projection::WorkspaceList(rows) => rows
            .iter()
            .map(|row| row.terminal_context_id.as_str())
            .collect(),
        Projection::WorkspaceGet(doc) => vec![&doc.row.terminal_context_id],
        Projection::TabList(_) | Projection::TabGet(_) => vec![],
        Projection::PanelList(_) | Projection::PanelGet(_) => return Ok(vec![]),
        _ => return Err(Failure::new(FailureKind::InternalFailure)),
    };
    let mut contexts = ids
        .into_iter()
        .map(|id| {
            WorkbenchTerminalContextId::parse(id)
                .map_err(|_| Failure::new(FailureKind::InternalFailure))
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Always corroborate the current selection too, including an empty Workspace list.
    let current = &selected
        .view
        .selection
        .as_ref()
        .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?
        .terminal_context_id;
    if !contexts.contains(current) {
        contexts.push(current.clone());
    }
    Ok(contexts)
}

pub(super) fn submission_failure(
    code: u8,
    outcome: Option<&ClientViewOutcome>,
) -> Option<FailureKind> {
    Some(match code {
        0 => return None,
        3 => match outcome {
            Some(ClientViewOutcome::Denied {}) => FailureKind::Denied,
            Some(ClientViewOutcome::NotFound {}) => FailureKind::NotFound,
            // This step consumes its requested ClientView receipt class; other replies cannot confirm that observation.
            _ => FailureKind::Rejected,
        },
        4 => FailureKind::ServiceUnavailable,
        5 | 8 => FailureKind::TransportLoss,
        7 => FailureKind::Timeout,
        130 => FailureKind::Cancelled,
        _ => FailureKind::ProtocolInvalid,
    })
}

fn execute(
    command: &QueryCommand,
    selected: Option<&WorkbenchClientId>,
    format: OutputFormat,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<(Value, Document), Failure> {
    let admitted = query::stream_snapshot(deadline, signals.poll_fd());
    signals.check_before(deadline)?;
    let (mut client, snapshot) = admitted?;
    let projection = results::project(command, &snapshot)?;
    if format == OutputFormat::Text
        && let Projection::PanelList(rows) = &projection
    {
        let initial = selected
            .map(|id| current(&mut client, &snapshot, id, deadline, signals, true))
            .transpose()?;
        let table = super::panel_list::read(&mut client, &snapshot, rows, deadline, signals)?;
        if let Some(initial) = initial {
            let final_client = current(
                &mut client,
                &snapshot,
                &initial.client_id,
                deadline,
                signals,
                true,
            )?;
            if initial.view_revision != final_client.view_revision
                || initial.view != final_client.view
            {
                return Err(super::query::workbench_failure("client-changed"));
            }
        }
        let mut document = Document::new(projection);
        document.panel_table = Some(table);
        return Ok((results::snapshot_source(&snapshot), document));
    }
    if snapshot_only_list_text(command, format) {
        if let Some(selected) = selected {
            let client = current(&mut client, &snapshot, selected, deadline, signals, false)?;
            verify_membership(&snapshot, &client.view, &[])?;
        }
        signals.check_before(deadline)?;
        return Ok((
            results::snapshot_source(&snapshot),
            Document::new(projection),
        ));
    }
    let panel_read = matches!(
        projection,
        Projection::PanelList(_) | Projection::PanelGet(_)
    );
    let original = selected
        .map(|id| current(&mut client, &snapshot, id, deadline, signals, panel_read))
        .transpose()?;
    collect(
        &mut client,
        &snapshot,
        projection,
        original,
        None,
        deadline,
        signals,
    )
}

fn snapshot_only_list_text(command: &QueryCommand, format: OutputFormat) -> bool {
    matches!(format, OutputFormat::Text)
        && matches!(
            command,
            QueryCommand::WorkspaceList { .. } | QueryCommand::TabList { .. }
        )
}

/// Enrich an already resolved projection using its original snapshot and Client selection.
/// The caller owns admission, the absolute deadline, and signal finalization.
pub(super) fn collect(
    client: &mut crate::local_client::LocalClient,
    snapshot: &Snapshot,
    projection: Projection,
    original: Option<ClientViewRecord>,
    current_member: Option<super::current_presentation::Resolved>,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<(Value, Document), Failure> {
    signals.check_before(deadline)?;
    let panel_read = matches!(
        projection,
        Projection::PanelList(_) | Projection::PanelGet(_)
    );
    if let Some(record) = &original {
        verify_membership(snapshot, &record.view, &[])?;
    }
    let requested = original
        .as_ref()
        .map(|record| requested_contexts(&projection, record))
        .transpose()?
        .unwrap_or_default();
    if let Some(member) = &current_member
        && !matches!(&projection, Projection::PanelGet(panel) if panel.row.panel_id == member.panel_id.as_str())
    {
        return Err(Failure::new(FailureKind::InternalFailure));
    }
    let mut document = Document::new(projection);
    document.current_member = current_member;
    document.client = original;
    if let Some(read) = document
        .current_member
        .as_ref()
        .and_then(|member| member.client())
    {
        document.operations.push(read.operation_id.clone());
    }
    for ids in requested.chunks(MAXIMUM_CLIENT_SELECTION_CONTEXTS) {
        let record = document
            .client
            .as_ref()
            .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
        signals.check_before(deadline)?;
        let request = ClientViewControl {
            operation_id: query::request_id()?,
            client_id: record.client_id.clone(),
            expected_view_revision: record.view_revision,
            minimum_workbench_revision: snapshot.revision,
            intent: ClientViewIntent::Selections {
                terminal_context_ids: ids.to_vec(),
            },
        };
        let answer = control::submit_control(
            client,
            &snapshot.session_id,
            request.clone(),
            deadline,
            signals,
        )?;
        signals.check_before(deadline)?;
        let outcome = answer.reply.as_ref().map(|reply| &reply.outcome);
        if let Some(ClientViewOutcome::Operation { operation }) = outcome
            && operation.command != request
        {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        }
        if let Some(kind) = submission_failure(answer.exit_code(), outcome) {
            return Err(Failure::new(kind));
        }
        let Some(reply) = answer.reply else {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        };
        let ClientViewOutcome::Operation { operation } = reply.outcome else {
            return Err(Failure::new(FailureKind::ProtocolInvalid));
        };
        let ClientViewControlStatus::NavigationRead { navigation, view } = operation.status else {
            return Err(Failure::new(FailureKind::Rejected));
        };
        if navigation.workbench_revision != snapshot.revision || view != record.view {
            return Err(Failure::new(FailureKind::Rejected));
        }
        verify_membership(snapshot, &view, &navigation.contexts)?;
        document.operations.push(request.operation_id);
        document.contexts.extend(navigation.contexts);
    }
    document.agents = Some(super::workbench_agents::read(
        client,
        snapshot,
        &document.projection,
        document.client.as_ref(),
        document.current_member.as_ref(),
        deadline,
        signals,
    )?);
    document.worktrees = super::workbench_worktrees::read(
        client,
        snapshot,
        &document.projection,
        deadline,
        signals,
    )?;
    document.metadata =
        super::workbench_metadata::read(client, snapshot, &document.projection, deadline, signals)?;
    document.terminals = super::workbench_terminals::read(
        client,
        snapshot,
        &document.projection,
        document.current_member.as_ref(),
        deadline,
        signals,
    )?;
    document.viewports = if let Some(member) = &document.current_member {
        super::workbench_viewports::from_current(member)
    } else {
        super::workbench_viewports::read(
            client,
            snapshot,
            &document.projection,
            document.client.as_ref(),
            deadline,
            signals,
        )?
    };
    if let Some(viewports) = &document.viewports {
        document
            .operations
            .extend(viewports.operations.iter().cloned());
    }
    let panel_ids = match document.projection.as_ref() {
        Projection::PanelList(rows) => Some(
            rows.iter()
                .map(|row| row.panel_id.as_str())
                .collect::<Vec<_>>(),
        ),
        Projection::PanelGet(panel) => Some(vec![panel.row.panel_id.as_str()]),
        _ => None,
    };
    if let Some(ids) = panel_ids {
        let ids = ids
            .into_iter()
            .map(|id| {
                orchbee_workbench_protocol::WorkbenchPanelId::parse(id)
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut metadata = if let Some(member) = &document.current_member {
            super::panel_metadata_query::read_presentation(
                client, snapshot, member, deadline, signals,
            )?
        } else {
            super::panel_metadata_query::read(client, snapshot, &ids, deadline, signals)?
        };
        metadata.verify(client, snapshot, deadline, signals)?;
        document.panel_metadata = Some(metadata);
    }
    if let Some(metadata) = &mut document.metadata {
        metadata.verify(client, snapshot, deadline, signals)?;
    }
    let final_snapshot = query::refresh_snapshot(
        client,
        &snapshot.session_id,
        snapshot.revision,
        deadline,
        signals,
    )?;
    if final_snapshot.revision != snapshot.revision {
        return Err(super::query::workbench_failure("workbench-changed"));
    }
    if let (Some(original), Some(member), Some(first)) = (
        &document.client,
        &document.current_member,
        document
            .current_member
            .as_ref()
            .and_then(|member| member.client()),
    ) {
        let last = super::current_presentation::read_with_receipt(
            client,
            snapshot,
            original,
            &member.panel_id,
            deadline,
            signals,
        )?;
        if !first.member.same_member(&last.member) {
            return Err(super::query::workbench_failure("client-changed"));
        }
        document.operations.push(last.operation_id);
    } else if let Some(original) = &document.client {
        let final_client = current(
            client,
            snapshot,
            &original.client_id,
            deadline,
            signals,
            panel_read,
        )?;
        if final_client.view_revision != original.view_revision
            || final_client.view != original.view
        {
            return Err(super::query::workbench_failure("client-changed"));
        }
    }
    signals.check_before(deadline)?;
    Ok((results::snapshot_source(snapshot), document))
}

fn verify_membership(
    snapshot: &Snapshot,
    view: &orchbee_workbench_protocol::ClientViewObservation,
    entries: &[ClientContextSelection],
) -> Result<(), Failure> {
    let invalid = || Failure::new(FailureKind::ProtocolInvalid);
    let selected = view.selection.as_ref().ok_or_else(invalid)?;
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == selected.terminal_context_id)
        .ok_or_else(invalid)?;
    match &selected.tab_id {
        Some(id) => {
            let tab = context
                .tabs
                .iter()
                .find(|tab| &tab.tab_id == id)
                .ok_or_else(invalid)?;
            let panels = results::panel_ids_in(&tab.root);
            if panels.len() != usize::from(selected.panel_count)
                || selected
                    .panel_id
                    .as_ref()
                    .is_none_or(|id| !panels.contains(&id))
            {
                return Err(invalid());
            }
        }
        None if !context.tabs.is_empty() => return Err(invalid()),
        None => {}
    }
    for entry in entries {
        let context = snapshot
            .terminal_contexts
            .iter()
            .find(|context| context.terminal_context_id == entry.terminal_context_id)
            .ok_or_else(invalid)?;
        if entry
            .active_tab_id
            .as_ref()
            .map_or(!context.tabs.is_empty(), |id| {
                !context.tabs.iter().any(|tab| &tab.tab_id == id)
            })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

impl Document {
    pub(super) fn result(&self) -> Value {
        let mut value = self.projection.result();
        if let Some(member) = &self.current_member {
            value["panel"]["currentPresentation"] = member.value();
        }
        if let Some(agents) = &self.agents {
            agents.enrich(&mut value);
        }
        if let Some(terminals) = &self.terminals {
            terminals.enrich(&mut value);
        }
        if let Some(metadata) = &self.panel_metadata {
            metadata.enrich(&mut value);
        }
        if let Some(metadata) = &self.metadata {
            metadata.enrich(&mut value);
        }
        if let Some(worktrees) = &self.worktrees {
            worktrees.enrich(&mut value);
        }
        super::workbench_viewports::enrich(
            self.client.as_ref().and(self.viewports.as_ref()),
            &mut value,
        );
        let Some(client) = &self.client else {
            return value;
        };
        let selected = client.view.selection.as_ref();
        let enrich = |row: &mut Value, family: &str| {
            let context = if family == "workspace" || family == "panel-list" {
                row["terminalContextId"].as_str()
            } else {
                row["context"]["terminalContextId"].as_str()
            };
            let focused = selected.is_some_and(|selection| {
                context == Some(selection.terminal_context_id.as_str())
                    && (family == "workspace"
                        || row["tabId"].as_str() == selection.tab_id.as_ref().map(|id| id.as_str()))
                    && (!family.starts_with("panel")
                        || row["panelId"].as_str()
                            == selection.panel_id.as_ref().map(|id| id.as_str()))
            });
            let active = (family == "workspace").then(|| {
                self.contexts
                    .iter()
                    .find(|entry| Some(entry.terminal_context_id.as_str()) == context)
                    .and_then(|entry| entry.active_tab_id.as_ref())
                    .map(ToString::to_string)
            });
            row["focused"] = json!(focused);
            if let Some(active) = active {
                row["activeTabId"] = json!(active);
            }
        };
        for (key, family) in [
            ("workspaces", "workspace"),
            ("tabs", "tab"),
            ("panels", "panel-list"),
        ] {
            if let Some(rows) = value.get_mut(key).and_then(Value::as_array_mut) {
                for row in rows {
                    enrich(row, family);
                }
            }
        }
        for key in ["workspace", "tab", "panel"] {
            if value.get(key).is_some() {
                enrich(&mut value[key], key);
            }
        }
        value["client"] = json!({ "clientId": client.client_id, "viewRevision": client.view_revision, "operationIds": self.operations, "view": client.view });
        value
    }
    pub(super) fn render(
        &self,
        out: &mut Bounded,
        command: &QueryCommand,
        locale: Locale,
    ) -> Result<(), TooLarge> {
        if let Some(table) = &self.panel_table {
            return table.render(out, locale);
        }
        // A Workspace detail opens with the Workspace itself; its Agent and
        // worktree supplements follow.
        let lead = matches!(*self.projection, Projection::WorkspaceGet(_));
        if lead {
            super::present::render(out, &self.projection, command, locale)?;
        }
        if let Some(member) = &self.current_member {
            out.write(format_args!(
                "{}: {}\n",
                CliText::new(locale).cli_current_panel_label(),
                member.panel_id
            ))?;
            member.render(out, locale)?;
        }
        if let Some(agents) = &self.agents {
            agents.render(out, locale)?;
        }
        if let Some(terminals) = &self.terminals {
            terminals.render(out, locale)?;
        }
        if let Some(worktrees) = &self.worktrees {
            worktrees.render(out, locale)?;
        }
        if let Some(metadata) = &self.panel_metadata {
            metadata.render(out, locale)?;
        }
        if let Some(metadata) = &self.metadata {
            metadata.render(out, locale)?;
        }
        if let Some(viewports) = &self.viewports {
            viewports.render(out, locale)?;
        }
        let Some(client) = &self.client else {
            return if lead {
                Ok(())
            } else {
                super::present::render(out, &self.projection, command, locale)
            };
        };
        let labels = CliText::new(locale).cli_query_client_labels();
        out.write(format_args!("{}: {}\n", labels[0], client.client_id))?;
        if let Some(selection) = &client.view.selection {
            out.write(format_args!(
                "{}: {}\n{}: {}\n",
                labels[1],
                selection.tab_id.as_ref().map_or("-", |id| id.as_str()),
                labels[2],
                selection.panel_id.as_ref().map_or("-", |id| id.as_str())
            ))?;
        }
        out.write(format_args!("{}\n", labels[3]))?;
        for context in &self.contexts {
            out.write(format_args!(
                "{}  {}\n",
                context.terminal_context_id,
                context.active_tab_id.as_ref().map_or("-", |id| id.as_str())
            ))?;
        }
        if lead {
            return Ok(());
        }
        super::present::render(out, &self.projection, command, locale)
    }
}
