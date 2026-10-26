//! Select and fence a plugin Pane's canonical source without changing Client navigation.
use super::{CurrentPanel, infer, infer_context, invalid};
use crate::{
    cli::{
        Failure, FailureKind,
        caller_context::PanelTarget,
        client_control::{exchange as control, selection},
        console_interrupt::SignalWatch,
        query, results, workbench_agents, workbench_query,
    },
    invocation::{MutationCommand, TabContext, Target},
    local_client::LocalClient,
};
use orchbee_workbench_protocol::{
    ClientViewAction, ClientViewAvailability, ClientViewControl, ClientViewControlStatus,
    ClientViewIntent, ClientViewOutcome, ClientViewRecord, PluginInvocationContext, Snapshot,
    WorkbenchClientId, WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use std::time::Instant;

enum TargetSource {
    Current,
    Panel(Target),
    Context(WorkbenchTerminalContextId),
}

pub(in crate::cli) struct Source<'a> {
    explicit: Option<&'a WorkbenchClientId>,
    target: TargetSource,
}

pub(super) struct Observation {
    pub(super) context: PluginInvocationContext,
    pub(super) panel: Option<CurrentPanel>,
    pub(super) selected: Option<ClientViewRecord>,
    pub(super) fence: Option<Fence>,
}

impl<'a> Source<'a> {
    pub(in crate::cli) fn current(explicit: Option<&'a WorkbenchClientId>) -> Self {
        Self {
            explicit,
            target: TargetSource::Current,
        }
    }

    pub(in crate::cli) fn creation(
        command: &MutationCommand,
        snapshot: &Snapshot,
        overlay: bool,
        explicit: Option<&'a WorkbenchClientId>,
    ) -> Result<Self, Failure> {
        let target = if overlay {
            TargetSource::Current
        } else {
            match command {
                MutationCommand::PanelSplit {
                    target: Target::Id(id),
                    ..
                } => TargetSource::Panel(Target::Id(id.clone())),
                MutationCommand::TabCreate { context, .. } => {
                    let id = match context {
                        TabContext::Workspace(Target::Id(id)) => snapshot
                            .navigation
                            .workspaces
                            .iter()
                            .find(|w| w.workspace_id.as_str() == id)
                            .ok_or_else(invalid)?
                            .terminal_context_id
                            .clone(),
                        _ => return Err(invalid()),
                    };
                    TargetSource::Context(id)
                }
                _ => return Err(invalid()),
            }
        };
        Ok(Self { explicit, target })
    }

    fn contains(&self, snapshot: &Snapshot, panel: &Target) -> bool {
        match &self.target {
            TargetSource::Current => true,
            TargetSource::Panel(target) => target == panel,
            TargetSource::Context(id) => results::panel_entries(snapshot)
                .into_iter()
                .any(|(p, c, _)| p.as_str() == panel.text() && &c.terminal_context_id == id),
        }
    }

    pub(super) fn prepare(
        self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        deadline: Instant,
        signals: &mut impl SignalWatch,
    ) -> Result<Observation, Failure> {
        let caller = if self.explicit.is_none() {
            PanelTarget::prepare_optional(|name| std::env::var_os(name))?
                .map(|caller| caller.resolve_exact(snapshot))
                .transpose()?
                .filter(|(panel, _)| self.contains(snapshot, panel))
        } else {
            None
        };
        let selected = if caller.is_none() {
            let response = control::exchange(
                client,
                &snapshot.session_id,
                ClientViewAction::List {},
                deadline,
                signals,
            )?;
            let ClientViewOutcome::Listed { clients } = response.outcome else {
                return Err(Failure::new(
                    if matches!(response.outcome, ClientViewOutcome::Denied {}) {
                        FailureKind::Denied
                    } else {
                        FailureKind::ServiceUnavailable
                    },
                ));
            };
            if self.explicit.is_none()
                && !clients
                    .iter()
                    .any(|c| c.availability == ClientViewAvailability::Current)
            {
                None
            } else {
                Some(selection::select(&clients, self.explicit, None)?.clone())
            }
        } else {
            None
        };
        let (current, panel) = infer(snapshot, selected.as_ref(), caller)?;
        let uses_current = match &self.target {
            TargetSource::Current => true,
            TargetSource::Panel(target) => panel.as_ref().is_some_and(|(p, _)| p == target),
            TargetSource::Context(id) => {
                panel
                    .as_ref()
                    .is_some_and(|(p, _)| self.contains(snapshot, p))
                    || selected
                        .as_ref()
                        .and_then(|r| r.view.selection.as_ref())
                        .is_some_and(|s| &s.terminal_context_id == id)
            }
        };
        if uses_current {
            return Ok(Observation {
                context: current,
                panel,
                selected,
                fence: None,
            });
        }
        let (context, panel, memory) = match self.target {
            TargetSource::Panel(target) => {
                let (context, panel) = infer(snapshot, None, Some((target, None)))?;
                (context, panel, None)
            }
            TargetSource::Context(id) => {
                let mut context = infer_context(snapshot, &id)?;
                let populated = snapshot
                    .terminal_contexts
                    .iter()
                    .find(|c| c.terminal_context_id == id)
                    .ok_or_else(invalid)?;
                let memory = if let Some(record) = &selected
                    && !populated.tabs.is_empty()
                {
                    Some(Memory::read(
                        client, snapshot, record, &id, deadline, signals,
                    )?)
                } else {
                    None
                };
                let panel = if let Some(memory) = &memory {
                    if let Some(panel) = &memory.focus {
                        let (focused, panel) =
                            infer(snapshot, None, Some((Target::Id(panel.to_string()), None)))?;
                        context = focused;
                        panel
                    } else {
                        if let Some(id) = &memory.tab {
                            let tab = populated
                                .tabs
                                .iter()
                                .find(|t| &t.tab_id == id)
                                .ok_or_else(invalid)?;
                            context.tab_id = Some(id.to_string());
                            context.tab_label = Some(tab.label.clone());
                        }
                        None
                    }
                } else {
                    None
                };
                (context, panel, memory)
            }
            TargetSource::Current => return Err(invalid()),
        };
        Ok(Observation {
            context,
            panel,
            selected: None,
            fence: selected.map(|record| Fence { record, memory }),
        })
    }
}

/// Only remembered source identities participate; unrelated geometry is not a focus change.
#[derive(PartialEq)]
struct Memory {
    context: WorkbenchTerminalContextId,
    tab: Option<WorkbenchTabId>,
    focus: Option<WorkbenchPanelId>,
}

impl Memory {
    fn read(
        client: &mut LocalClient,
        snapshot: &Snapshot,
        record: &ClientViewRecord,
        context: &WorkbenchTerminalContextId,
        deadline: Instant,
        signals: &mut impl SignalWatch,
    ) -> Result<Self, Failure> {
        let ClientViewControlStatus::NavigationRead { navigation, view } = read_status(
            client,
            snapshot,
            record,
            ClientViewIntent::Selections {
                terminal_context_ids: vec![context.clone()],
            },
            deadline,
            signals,
        )?
        else {
            return Err(invalid());
        };
        if view != record.view || navigation.workbench_revision != snapshot.revision {
            return Err(query::workbench_failure("client-changed"));
        }
        if !navigation.answers(std::slice::from_ref(context), &view, snapshot.revision) {
            return Err(invalid());
        }
        let tab = navigation
            .contexts
            .into_iter()
            .next()
            .ok_or_else(invalid)?
            .active_tab_id;
        let focus = if let Some(id) = &tab {
            let canonical = snapshot
                .terminal_contexts
                .iter()
                .find(|c| &c.terminal_context_id == context)
                .and_then(|c| c.tabs.iter().find(|t| &t.tab_id == id))
                .ok_or_else(invalid)?;
            let panels = results::panel_ids_in(&canonical.root);
            let anchor = panels.first().ok_or_else(invalid)?;
            let ClientViewControlStatus::LayoutRead { layout, view } = read_status(
                client,
                snapshot,
                record,
                ClientViewIntent::Layout {
                    terminal_context_id: context.clone(),
                    tab_id: id.clone(),
                    panel_id: (*anchor).clone(),
                },
                deadline,
                signals,
            )?
            else {
                return Err(invalid());
            };
            if view != record.view || layout.workbench_revision != snapshot.revision {
                return Err(query::workbench_failure("client-changed"));
            }
            if layout.validate().is_err()
                || layout.terminal_context_id != *context
                || layout.tab_id != *id
                || layout.source_panel_id != **anchor
                || layout.panels.len() != panels.len()
                || !layout.panels.iter().all(|p| panels.contains(&&p.panel_id))
                || layout
                    .focused_panel_id
                    .as_ref()
                    .is_some_and(|p| !panels.contains(&p))
            {
                return Err(invalid());
            }
            layout.focused_panel_id
        } else {
            None
        };
        Ok(Self {
            context: context.clone(),
            tab,
            focus,
        })
    }
}

pub(super) struct Fence {
    record: ClientViewRecord,
    memory: Option<Memory>,
}
impl Fence {
    pub(super) fn verify(
        self,
        client: &mut LocalClient,
        snapshot: &Snapshot,
        deadline: Instant,
        signals: &mut impl SignalWatch,
    ) -> Result<(), Failure> {
        if let Some(memory) = self.memory {
            let fresh = Memory::read(
                client,
                snapshot,
                &self.record,
                &memory.context,
                deadline,
                signals,
            )?;
            if fresh != memory {
                return Err(query::workbench_failure("client-changed"));
            }
        }
        let response = control::exchange(
            client,
            &snapshot.session_id,
            ClientViewAction::Get {
                client_id: self.record.client_id.clone(),
            },
            deadline,
            signals,
        )?;
        if !matches!(response.outcome, ClientViewOutcome::Found { client }
            if client.availability == ClientViewAvailability::Current
                && client.view_revision == self.record.view_revision && client.view == self.record.view)
        {
            return Err(query::workbench_failure("client-changed"));
        }
        Ok(())
    }
}

fn read_status(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    record: &ClientViewRecord,
    intent: ClientViewIntent,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<ClientViewControlStatus, Failure> {
    signals.check_before(deadline)?;
    let command = ClientViewControl {
        operation_id: query::request_id()?,
        client_id: record.client_id.clone(),
        expected_view_revision: record.view_revision,
        minimum_workbench_revision: snapshot.revision,
        intent,
    };
    let answer = control::submit_control(
        client,
        &snapshot.session_id,
        command.clone(),
        deadline,
        signals,
    )?;
    signals.check_before(deadline)?;
    let outcome = answer.reply.as_ref().map(|r| &r.outcome);
    if let Some(ClientViewOutcome::Operation { operation }) = outcome
        && operation.command != command
    {
        return Err(invalid());
    }
    if let Some(kind) = workbench_query::submission_failure(answer.exit_code(), outcome) {
        return Err(Failure::new(kind));
    }
    let Some(reply) = answer.reply else {
        return Err(invalid());
    };
    let ClientViewOutcome::Operation { operation } = reply.outcome else {
        return Err(invalid());
    };
    Ok(operation.status)
}
