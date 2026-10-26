//! Compose membership and launch owners under exact snapshots and one recovery budget.
mod closing;
mod view;
use super::{error, exchange, ownership, receipt::Receipt, targeting::Destination};
use crate::{
    cli::{
        Failure, FailureKind,
        layouts::{export, launch, template, workflow},
        mutation, query,
        source::{ArmedSignals, SignalWatch},
    },
    invocation::{ClientLayoutTarget, LayoutApplyCommand, LayoutExportCommand, Target},
    local_client::LocalClient,
};
use orchbee_client_server_protocol::{
    TerminalId, TerminalLaunchReadOutcome, TerminalLaunchReadRequest, TerminalLaunchState,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ClientViewRecord, Snapshot, Tab, WorkbenchAction, WorkbenchCommandOutcome,
    WorkbenchDirectoryPath, WorkbenchPanelId, WorkbenchPanelTerminalTarget, WorkbenchRevision,
    WorkbenchTabId, WorkbenchTerminalGeometry, WorkbenchTerminalOutcome,
};
use serde_json::json;
use std::time::{Duration, Instant};

pub(super) struct Engine<'a> {
    command: &'a LayoutApplyCommand,
    plan: &'a template::Plan,
    prepared: Vec<Vec<Option<launch::Prepared>>>,
    destination: Destination,
    client: LocalClient,
    snapshot: Snapshot,
    selected: Option<ClientViewRecord>,
    default_directory: Option<WorkbenchDirectoryPath>,
    expected: Option<Tab>,
    panels: Vec<WorkbenchPanelId>,
    splits: Vec<ClientSplitIdentity>,
    deadline: Instant,
    total_deadline: Instant,
    signals: &'a mut ArmedSignals,
    receipt: &'a mut Receipt,
}
impl<'a> Engine<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        command: &'a LayoutApplyCommand,
        plan: &'a template::Plan,
        prepared: Vec<Vec<Option<launch::Prepared>>>,
        destination: Destination,
        client: LocalClient,
        snapshot: Snapshot,
        selected: Option<ClientViewRecord>,
        deadline: Instant,
        total_deadline: Instant,
        signals: &'a mut ArmedSignals,
        receipt: &'a mut Receipt,
    ) -> Self {
        Self {
            command,
            plan,
            prepared,
            destination,
            client,
            snapshot,
            selected,
            default_directory: None,
            expected: None,
            panels: vec![],
            splits: vec![],
            deadline,
            total_deadline,
            signals,
            receipt,
        }
    }
    fn check(&mut self) -> Result<(), Failure> {
        self.signals.check_before(self.deadline)
    }
    fn new_tab(&self) -> Result<WorkbenchTabId, Failure> {
        self.receipt.new_tab.clone().ok_or_else(internal)
    }
    fn refresh(&mut self, minimum: WorkbenchRevision) -> Result<(), Failure> {
        self.snapshot = query::refresh_snapshot(
            &mut self.client,
            &self.snapshot.session_id,
            minimum,
            self.deadline,
            self.signals,
        )?;
        self.fence()
    }
    fn fence(&self) -> Result<(), Failure> {
        if let Some(old) = &self.destination.old {
            ownership::unchanged(&self.snapshot, &self.destination.context, old)?;
        }
        if let Some(expected) = &self.expected {
            ownership::unchanged(&self.snapshot, &self.destination.context, expected)?;
        }
        Ok(())
    }
    fn submit(
        &mut self,
        phase: &'static str,
        action: WorkbenchAction,
    ) -> Result<mutation::Receipt, Failure> {
        self.receipt.stage = phase;
        self.check()?;
        self.refresh(self.snapshot.revision)?;
        let result = mutation::submit_action(
            &mut self.client,
            &self.snapshot,
            action,
            self.deadline,
            self.signals.poll_fd(),
        );
        match &result {
            Ok(receipt) => self.receipt.record(phase, receipt.document()),
            Err(failure) => self.receipt.record(phase, failure.document()),
        }
        result
    }
    fn committed(&mut self, revision: WorkbenchRevision) -> Result<(), Failure> {
        let fresh = query::refresh_snapshot(
            &mut self.client,
            &self.snapshot.session_id,
            revision,
            self.deadline,
            self.signals,
        )?;
        if fresh.revision != revision {
            return Err(ownership::changed());
        }
        if let Some(old) = &self.destination.old {
            ownership::unchanged(&fresh, &self.destination.context, old)?;
        }
        self.snapshot = fresh;
        Ok(())
    }
    fn directory(
        &mut self,
        panel: &WorkbenchPanelId,
        member: usize,
    ) -> Result<Option<WorkbenchDirectoryPath>, Failure> {
        let presentation = self
            .snapshot
            .terminal_contexts
            .iter()
            .flat_map(|context| &context.tabs)
            .find_map(|tab| ownership::member(&tab.root, panel, member))
            .ok_or_else(ownership::changed)?;
        let Some(binding) = &presentation.binding else {
            return Ok(None);
        };
        let target = Target::Id(binding.terminal_id.to_string());
        let result = query::read_terminal_processes_cancellable(
            &mut self.client,
            self.deadline,
            &self.snapshot,
            &target,
            self.signals.poll_fd(),
        );
        self.check()?;
        match result {
            Ok((_, sample)) => Ok(sample
                .info
                .processes
                .iter()
                .find(|process| process.pid == sample.info.root_pid)
                .and_then(|process| process.current_directory.as_ref())
                .filter(|path| std::path::Path::new(path).is_absolute())
                .and_then(|path| WorkbenchDirectoryPath::parse(path).ok())),
            Err(failure) if failure.kind == FailureKind::ServiceUnavailable => Ok(None),
            Err(failure) => Err(failure),
        }
    }
    pub(super) fn prepare_directory(&mut self) -> Result<(), Failure> {
        if self.prepared.iter().flatten().all(Option::is_none) {
            return Ok(());
        }
        if self.prepared[0][0]
            .as_ref()
            .is_some_and(|prepared| prepared.options.cwd.is_some())
        {
            return Ok(());
        }
        let panel = if let Some(old) = self.destination.old.clone() {
            if self.selected.is_some() {
                self.read_layout(&old)?.focused_panel_id
            } else {
                crate::cli::results::panel_ids_in(&old.root)
                    .first()
                    .cloned()
                    .cloned()
            }
        } else {
            self.selected
                .as_ref()
                .and_then(|client| client.view.selection.as_ref())
                .filter(|selection| selection.terminal_context_id == self.destination.context)
                .and_then(|selection| selection.panel_id.clone())
        };
        if let Some(panel) = panel {
            self.default_directory = self.directory(&panel, 0)?;
        }
        Ok(())
    }
    fn pause(&self) {
        std::thread::sleep(
            Duration::from_millis(30).min(self.deadline.saturating_duration_since(Instant::now())),
        );
    }
}
fn internal() -> Failure {
    error(FailureKind::InternalFailure, "invalid-receipt")
}
fn geometry() -> Result<WorkbenchTerminalGeometry, Failure> {
    WorkbenchTerminalGeometry::new(80, 24, 0, 0).map_err(|_| internal())
}
impl Engine<'_> {
    fn inherited_directory(
        &mut self,
        panel: usize,
        member: usize,
    ) -> Result<Option<WorkbenchDirectoryPath>, Failure> {
        if member > 0 {
            let id = self.panels[panel].clone();
            if let Some(path) = self.directory(&id, 0)? {
                return Ok(Some(path));
            }
            if let Some(path) = self.prepared[panel][0]
                .as_ref()
                .and_then(|prepared| prepared.options.cwd.clone())
            {
                return Ok(Some(path));
            }
            return self.inherited_directory(panel, 0);
        }
        if panel == 0 {
            return Ok(self.default_directory.clone());
        }
        let source = self
            .plan
            .splits
            .iter()
            .find(|split| split.created == panel)
            .ok_or_else(internal)?
            .source;
        let id = self.panels[source].clone();
        if let Some(path) = self.directory(&id, 0)? {
            return Ok(Some(path));
        }
        if let Some(path) = self.prepared[source][0]
            .as_ref()
            .and_then(|prepared| prepared.options.cwd.clone())
        {
            return Ok(Some(path));
        }
        self.inherited_directory(source, 0)
    }
    fn layout_command(
        &mut self,
        phase: &'static str,
        action: WorkbenchAction,
    ) -> Result<WorkbenchCommandOutcome, Failure> {
        self.receipt.stage = phase;
        self.check()?;
        self.refresh(self.snapshot.revision)?;
        let (document, result) = exchange::submit(
            &mut self.client,
            &self.snapshot,
            action,
            self.deadline,
            self.signals.poll_fd(),
        )?;
        self.receipt.record(phase, document);
        result
    }
}
impl workflow::Port for Engine<'_> {
    fn create(&mut self) -> Result<(), Failure> {
        let outcome = self.layout_command(
            "create",
            WorkbenchAction::TabCreateLayout {
                terminal_context_id: self.destination.context.clone(),
                label: self.destination.label.clone(),
                automatic_label: self.destination.automatic,
                layout: self.plan.layout.clone(),
            },
        )?;
        let WorkbenchCommandOutcome::TabLayoutCreated {
            revision, tab_id, ..
        } = outcome
        else {
            return Err(internal());
        };
        self.receipt.new_tab = Some(tab_id);
        self.receipt.recovery = json!({"outcome":"preserved"});
        self.committed(revision)?;
        let tab = ownership::tab(&self.snapshot, &self.destination.context, &self.new_tab()?)?;
        if tab.automatic_label != self.destination.automatic
            || tab.label != self.destination.label.as_str()
            || tab.pinned
            || tab.group_id.is_some()
        {
            return Err(ownership::changed());
        }
        let (panels, splits) = ownership::materialized(self.plan, &tab.root)?;
        self.panels = panels;
        self.splits = splits;
        self.expected = Some(tab.clone());
        self.check()
    }
    fn launch(&mut self, panel: usize, member: usize) -> Result<(), Failure> {
        let id = self.panels[panel].clone();
        let presentation = ownership::member(
            &self.expected.as_ref().ok_or_else(internal)?.root,
            &id,
            member,
        )
        .ok_or_else(internal)?
        .presentation_id
        .clone();
        let cwd = if self.prepared[panel][member]
            .as_ref()
            .ok_or_else(internal)?
            .options
            .cwd
            .is_none()
        {
            self.inherited_directory(panel, member)?
        } else {
            None
        };
        let prepared = self.prepared[panel][member].as_mut().ok_or_else(internal)?;
        if prepared.options.cwd.is_none() {
            prepared.options.cwd = cwd;
        }
        let action = WorkbenchAction::PanelCreateTerminal {
            terminal_context_id: self.destination.context.clone(),
            tab_id: self.receipt.new_tab.clone().ok_or_else(internal)?,
            panel_id: id.clone(),
            target: WorkbenchPanelTerminalTarget::Unbound {
                presentation_id: presentation.clone(),
            },
            geometry: geometry()?,
            launch: prepared.launch.clone(),
            profile: None,
            options: Box::new(prepared.options.clone()),
        };
        let outcome = self.layout_command("launch", action)?;
        let (revision, failure) = match outcome {
            WorkbenchCommandOutcome::PanelTerminalPending { revision, .. } => (revision, None),
            WorkbenchCommandOutcome::PanelTerminalCreated {
                revision,
                terminal_outcome,
                ..
            } => (
                revision,
                match terminal_outcome {
                    WorkbenchTerminalOutcome::Started { .. } => None,
                    WorkbenchTerminalOutcome::Unavailable { .. } => {
                        Some(FailureKind::ServiceUnavailable)
                    }
                    WorkbenchTerminalOutcome::Failed { .. } => Some(FailureKind::Rejected),
                    WorkbenchTerminalOutcome::OutcomeUnknown {} => {
                        Some(FailureKind::OutcomeUnknown)
                    }
                },
            ),
            // This step consumes its specific mutation receipt; other command results cannot confirm the requested postcondition.
            _ => return Err(internal()),
        };
        let observation = (|| {
            self.committed(revision)?;
            let fresh =
                ownership::tab(&self.snapshot, &self.destination.context, &self.new_tab()?)?;
            self.expected = Some(ownership::bind(
                self.expected.as_ref().ok_or_else(internal)?,
                fresh,
                &id,
                &presentation,
            )?);
            Ok(())
        })();
        if failure == Some(FailureKind::OutcomeUnknown) {
            return Err(error(FailureKind::OutcomeUnknown, "launch-unavailable"));
        }
        observation?;
        if let Some(kind) = failure {
            return Err(error(kind, "launch-unavailable"));
        }
        self.check()
    }
    fn settle(&mut self, leaf: usize, member: usize) -> Result<(), Failure> {
        self.receipt.stage = "launch";
        let tab = self.expected.as_ref().ok_or_else(internal)?;
        let primary_presentation =
            ownership::member(&tab.root, &self.panels[leaf], member).ok_or_else(internal)?;
        let binding = primary_presentation.binding.clone().ok_or_else(internal)?;
        let presentation = primary_presentation.presentation_id.clone();
        loop {
            self.check()?;
            let request = TerminalLaunchReadRequest::new(
                query::obcs_request_id()?,
                TerminalId::parse(binding.terminal_id.as_str()).map_err(|_| internal())?,
            );
            let read = self.client.submit_terminal_launch_read_cancellable(
                request,
                self.deadline,
                self.signals.poll_fd(),
            );
            self.check()?;
            let read = read.map_err(query::classify_launch_read)?;
            let TerminalLaunchReadOutcome::Found {
                session_id,
                terminal,
                launch,
                ..
            } = read.outcome()
            else {
                return Err(error(FailureKind::ServiceUnavailable, "launch-unavailable"));
            };
            let route = terminal.bound_to();
            if session_id.as_str() != self.snapshot.session_id.as_str()
                || terminal.worker_id().as_str() != binding.worker_id.as_str()
                || terminal.terminal_id().as_str() != binding.terminal_id.as_str()
                || route.presentation_id().as_str() != presentation.as_str()
                || route.panel_id().as_str() != self.panels[leaf].as_str()
                || route.tab_id().as_str() != self.new_tab()?.as_str()
                || route.terminal_context_id().as_str() != self.destination.context.as_str()
            {
                return Err(ownership::changed());
            }
            match launch.state() {
                TerminalLaunchState::Started => return Ok(()),
                TerminalLaunchState::Unavailable => {
                    return Err(error(FailureKind::Rejected, "launch-failed"));
                }
                TerminalLaunchState::Unknown => {
                    return Err(error(FailureKind::OutcomeUnknown, "launch-unavailable"));
                }
                TerminalLaunchState::NotTracked => {
                    return Err(error(FailureKind::ServiceUnavailable, "launch-unavailable"));
                }
                TerminalLaunchState::Creating
                | TerminalLaunchState::AwaitingStart
                | TerminalLaunchState::Starting => self.pause(),
            }
        }
    }
    fn ratio(&mut self, split: usize) -> Result<(), Failure> {
        self.set_ratio(split)
    }
    fn ready(&mut self) -> Result<(), Failure> {
        self.receipt.stage = "ready";
        self.refresh(self.snapshot.revision)?;
        for leaf in 0..self.panels.len() {
            for member in 0..self.prepared[leaf].len() {
                if self.prepared[leaf][member].is_some() {
                    self.settle(leaf, member)?;
                }
            }
        }
        self.refresh(self.snapshot.revision)?;
        self.receipt.stage = "ready";
        Ok(())
    }
    fn focus(&mut self) -> Result<(), Failure> {
        self.select_created()
    }
    fn replace(&mut self) -> Result<(), Failure> {
        self.close_old()
    }
    fn observe(&mut self) -> Result<(), Failure> {
        self.receipt.stage = "observation";
        self.refresh(self.snapshot.revision)?;
        if self.selected.is_some() {
            self.current_view()?;
        }
        let document = export::from_snapshot(
            &LayoutExportCommand {
                target: ClientLayoutTarget::Tab(Target::Id(self.new_tab()?.to_string())),
                client: self
                    .selected
                    .as_ref()
                    .map(|client| client.client_id.clone()),
                timeout_ms: self.command.timeout_ms,
            },
            &mut self.client,
            &self.snapshot,
            self.deadline,
            self.signals,
        )?;
        self.receipt.layout = Some(document["result"]["layout"].clone());
        self.refresh(self.snapshot.revision)?;
        if document["source"]["revision"] != json!(self.snapshot.revision)
            || !super::ownership::matches_template(
                self.plan,
                &self.panels,
                &document["result"]["layout"]["root"],
            )
        {
            return Err(ownership::changed());
        }
        self.receipt.stage = "complete";
        Ok(())
    }
    fn rollback(&mut self) {
        self.recover();
    }
}
