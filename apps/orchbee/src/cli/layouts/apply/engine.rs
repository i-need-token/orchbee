//! Compose membership and launch owners under exact snapshots and one recovery budget.
mod closing;
mod view;
use super::{error, ownership, receipt::Receipt, targeting::Destination};
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
    ClientSplitIdentity, ClientViewRecord, PanelNode, Snapshot, Tab, WorkbenchAction,
    WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchPanelId, WorkbenchPanelSlotId,
    WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalGeometry,
};
use serde_json::json;
use std::time::{Duration, Instant};

pub(super) struct Engine<'a> {
    command: &'a LayoutApplyCommand,
    plan: &'a template::Plan,
    prepared: Vec<launch::Prepared>,
    destination: Destination,
    client: LocalClient,
    snapshot: Snapshot,
    selected: ClientViewRecord,
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
        prepared: Vec<launch::Prepared>,
        destination: Destination,
        client: LocalClient,
        snapshot: Snapshot,
        selected: ClientViewRecord,
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
            &self.snapshot.site_id,
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
    fn committed(&mut self, receipt: &mutation::Receipt) -> Result<(), Failure> {
        let revision = receipt.committed_revision().ok_or_else(internal)?;
        let fresh = query::refresh_snapshot(
            &mut self.client,
            &self.snapshot.site_id,
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
    ) -> Result<Option<WorkbenchDirectoryPath>, Failure> {
        let target = query::primary_terminal(&self.snapshot, &Target::Id(panel.to_string()))?;
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
        if self.prepared[0].options.cwd.is_some() {
            return Ok(());
        }
        let panel = if let Some(old) = self.destination.old.clone() {
            self.read_layout(&old)?.focused_panel_id
        } else {
            self.selected
                .view
                .selection
                .as_ref()
                .filter(|selection| selection.terminal_context_id == self.destination.context)
                .and_then(|selection| selection.panel_id.clone())
        };
        if let Some(panel) = panel {
            self.prepared[0].options.cwd = self.directory(&panel)?;
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
fn id<'a>(receipt: &'a mutation::Receipt, name: &str) -> Result<&'a str, Failure> {
    receipt.id_value(name).ok_or_else(internal)
}

impl workflow::Port for Engine<'_> {
    fn create(&mut self) -> Result<(), Failure> {
        let receipt = self.submit(
            "create",
            WorkbenchAction::TabCreate {
                automatic_label: self.destination.automatic,
                terminal_context_id: self.destination.context.clone(),
                label: self.destination.label.clone(),
                geometry: geometry()?,
                launch: self.prepared[0].launch.clone(),
                profile: None,
                options: Box::new(self.prepared[0].options.clone()),
            },
        )?;
        self.receipt.new_tab =
            Some(WorkbenchTabId::parse(id(&receipt, "tabId")?).map_err(|_| internal())?);
        self.receipt.recovery = json!({"outcome":"preserved"});
        let panel = WorkbenchPanelId::parse(id(&receipt, "panelId")?).map_err(|_| internal())?;
        self.committed(&receipt)?;
        let tab = ownership::tab(&self.snapshot, &self.destination.context, &self.new_tab()?)?;
        if tab.automatic_label != self.destination.automatic
            || tab.label != self.destination.label.as_str()
            || tab.pinned
            || tab.group_id.is_some()
        {
            return Err(ownership::changed());
        }
        ownership::validate_leaf(&tab.root, id(&receipt, "presentationId")?, &panel)?;
        self.expected = Some(tab.clone());
        self.panels.push(panel);
        if let Some(kind) = receipt.launch_failure() {
            return Err(error(kind, "launch-unavailable"));
        }
        self.check()
    }
    fn split(&mut self, index: usize) -> Result<(), Failure> {
        let split = &self.plan.splits[index];
        let source = self.panels[split.source].clone();
        let created = split.created;
        let direction = split.direction;
        if self.prepared[created].options.cwd.is_none() {
            self.settle(split.source)?;
            self.prepared[created].options.cwd = self.directory(&source)?;
        }
        let receipt = self.submit(
            "split",
            WorkbenchAction::PanelSplit {
                terminal_context_id: self.destination.context.clone(),
                tab_id: self.new_tab()?,
                panel_id: source.clone(),
                direction,
                right_click: Default::default(),
                label: WorkbenchLabel::parse(&(created + 1).to_string()).map_err(|_| internal())?,
                geometry: geometry()?,
                launch: self.prepared[created].launch.clone(),
                profile: None,
                options: Box::new(self.prepared[created].options.clone()),
            },
        )?;
        let panel = WorkbenchPanelId::parse(id(&receipt, "newPanelId")?).map_err(|_| internal())?;
        let identity = ClientSplitIdentity {
            split_id: WorkbenchSplitId::parse(id(&receipt, "splitId")?).map_err(|_| internal())?,
            first_slot_id: WorkbenchPanelSlotId::parse(id(&receipt, "firstSlotId")?)
                .map_err(|_| internal())?,
            second_slot_id: WorkbenchPanelSlotId::parse(id(&receipt, "secondSlotId")?)
                .map_err(|_| internal())?,
        };
        self.committed(&receipt)?;
        let fresh = ownership::tab(&self.snapshot, &self.destination.context, &self.new_tab()?)?;
        let mut expected = self.expected.clone().ok_or_else(internal)?;
        ownership::graft(
            &mut expected.root,
            &fresh.root,
            &source,
            &panel,
            &identity,
            direction,
            id(&receipt, "presentationId")?,
        )?;
        if &expected != fresh {
            return Err(ownership::changed());
        }
        self.expected = Some(expected);
        self.panels.push(panel);
        self.splits.push(identity);
        if let Some(kind) = receipt.launch_failure() {
            return Err(error(kind, "launch-unavailable"));
        }
        self.check()
    }
    fn settle(&mut self, leaf: usize) -> Result<(), Failure> {
        self.receipt.stage = "launch";
        let tab = self.expected.as_ref().ok_or_else(internal)?;
        let PanelNode::Panel {
            primary_presentation,
            ..
        } = ownership::leaf(&tab.root, &self.panels[leaf]).ok_or_else(internal)?
        else {
            return Err(internal());
        };
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
                site_id,
                terminal,
                launch,
                ..
            } = read.outcome()
            else {
                return Err(error(FailureKind::ServiceUnavailable, "launch-unavailable"));
            };
            let route = terminal.bound_to();
            if site_id.as_str() != self.snapshot.site_id.as_str()
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
                _ => self.pause(),
            }
        }
    }
    fn label(&mut self, leaf: usize) -> Result<(), Failure> {
        let label = self.plan.leaves[leaf].label.clone();
        if label.custom_label().is_none() {
            return Ok(());
        }
        let panel = self.panels[leaf].clone();
        let receipt = self.submit(
            "label",
            WorkbenchAction::PanelRename {
                terminal_context_id: self.destination.context.clone(),
                tab_id: self.new_tab()?,
                panel_id: panel.clone(),
                label: label.clone(),
            },
        )?;
        self.committed(&receipt)?;
        let mut expected = self.expected.clone().ok_or_else(internal)?;
        ownership::label(&mut expected.root, &panel, &label)?;
        ownership::unchanged(&self.snapshot, &self.destination.context, &expected)?;
        self.expected = Some(expected);
        self.check()
    }
    fn ratio(&mut self, split: usize) -> Result<(), Failure> {
        self.set_ratio(split)
    }
    fn ready(&mut self) -> Result<(), Failure> {
        self.receipt.stage = "ready";
        self.refresh(self.snapshot.revision)?;
        for leaf in 0..self.panels.len() {
            self.settle(leaf)?;
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
        self.current_view()?;
        let document = export::execute(
            &LayoutExportCommand {
                target: ClientLayoutTarget::Tab(Target::Id(self.new_tab()?.to_string())),
                client: Some(self.selected.client_id.clone()),
                timeout_ms: self.command.timeout_ms,
            },
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
