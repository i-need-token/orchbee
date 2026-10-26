//! Resolve one canonical mutation against rendered and canonical split identities.
use super::{
    ClientPanelLayout, ClientPanelNeighbors, ClientViewControl, ClientViewIntent, NativeUi, Reason,
    Rect, SidebarSurface, Snapshot, cell, project_geometry,
};
use crate::workbench_client::NativePanelTree;
use orchbee_workbench_protocol::{
    ClientLayoutAdjustment, ClientLayoutChange, ClientSplitChange, ClientSplitPath,
    ClientViewObservation, PanelNode, WorkbenchPanelSlotId, WorkbenchSplitId,
};

impl NativeUi {
    pub(in crate::workbench_client) fn prepare_keyboard_adjustment(
        &self,
        direction: orchbee_workbench_protocol::ClientPanelDirection,
        fine: bool,
        viewport: Option<(Rect, SidebarSurface)>,
    ) -> Result<
        (
            orchbee_workbench_protocol::SiteId,
            ClientLayoutAdjustment,
            ClientViewObservation,
        ),
        Reason,
    > {
        let (site, before) = self
            .control_observation(false)
            .ok_or(Reason::ProjectionUnavailable)?;
        if before.phase != orchbee_workbench_protocol::ClientViewPhase::Ready {
            return Err(Reason::Busy);
        }
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        let tab = state
            .active()
            .and_then(|active| state.cache().tabs().get(active.index()))
            .ok_or(Reason::TargetUnavailable)?;
        let source = state
            .focus_panel_id()
            .cloned()
            .ok_or(Reason::TargetUnavailable)?;
        let (viewport, sidebar) = viewport.ok_or(Reason::ProjectionUnavailable)?;
        let focused = tab.panel_index(&source);
        let (area, panels, splits) = project_geometry(
            viewport,
            sidebar,
            tab,
            focused,
            state.zoom_active(),
            &|split, first, second| {
                tab.panel_tree()
                    .split_ratio(split, first, second)
                    .unwrap_or_default()
            },
        )?;
        let layout = ClientPanelLayout {
            terminal_context_id: state
                .cache()
                .terminal_context_id()
                .ok_or(Reason::TargetUnavailable)?
                .clone(),
            tab_id: tab.tab_id().clone(),
            source_panel_id: source.clone(),
            workbench_revision: state.cache().revision(),
            viewport: cell(viewport),
            area: cell(area),
            selected: true,
            zoomed: state.zoom_active(),
            focused_panel_id: Some(source.clone()),
            source_available: panels
                .iter()
                .any(|panel| panel.panel_id == source && panel.unzoomed.is_some()),
            neighbors: ClientPanelNeighbors::from_panels(&source, &panels),
            panels,
            splits,
        };
        layout
            .validate()
            .map_err(|_| Reason::ProjectionUnavailable)?;
        let amount =
            orchbee_workbench_protocol::ClientResizeAmount::new(if fine { 0.01 } else { 0.05 })
                .expect("fixed admitted keyboard step");
        let change = layout
            .resize_split(direction)
            .map_err(|_| Reason::ProjectionUnavailable)?
            .map_or(ClientLayoutChange::NoSplit {}, |split| {
                ClientLayoutChange::Split {
                    split: ClientSplitChange {
                        split_id: split.split_id.clone(),
                        first_slot_id: split.first_slot_id.clone(),
                        second_slot_id: split.second_slot_id.clone(),
                        previous_ratio: split.ratio,
                        ratio: amount.apply(split.ratio, direction),
                    },
                }
            });
        Ok((
            site,
            ClientLayoutAdjustment {
                terminal_context_id: layout.terminal_context_id,
                tab_id: layout.tab_id,
                workbench_revision: layout.workbench_revision,
                selected: true,
                change,
            },
            before,
        ))
    }

    pub(in crate::workbench_client) fn local_adjustment_request_id(
        &self,
    ) -> Result<orchbee_workbench_protocol::WorkbenchRequestId, Reason> {
        self.mailbox
            .as_ref()
            .ok_or(Reason::ProjectionUnavailable)?
            .allocate_workbench_request_id()
            .map_err(|_| Reason::ProjectionUnavailable)
    }

    pub(in crate::workbench_client) fn prepare_layout_adjustment(
        &self,
        command: &ClientViewControl,
        snapshot: &Snapshot,
        viewport: Option<(Rect, SidebarSurface)>,
    ) -> Result<ClientLayoutAdjustment, Reason> {
        let state = self.state.as_ref().ok_or(Reason::ProjectionUnavailable)?;
        if snapshot.site_id != *state.cache().site_id()
            || snapshot.revision < state.cache().revision()
            || snapshot.revision < command.minimum_workbench_revision
        {
            return Err(Reason::StaleView);
        }
        match &command.intent {
            ClientViewIntent::Resize {
                terminal_context_id,
                tab_id,
                panel_id,
                direction,
                amount,
            } => {
                let mut read = command.clone();
                read.intent = ClientViewIntent::Layout {
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    panel_id: panel_id.clone(),
                };
                let (viewport, sidebar) = viewport.ok_or(Reason::ProjectionUnavailable)?;
                let layout = self.read_client_layout(&read, snapshot, viewport, sidebar)?;
                if let Some(split) = layout
                    .resize_split(*direction)
                    .map_err(|_| Reason::ProjectionUnavailable)?
                {
                    let canonical = snapshot
                        .terminal_contexts
                        .iter()
                        .find(|context| &context.terminal_context_id == terminal_context_id)
                        .and_then(|context| context.tabs.iter().find(|tab| &tab.tab_id == tab_id))
                        .and_then(|tab| canonical_split_by_id(&tab.root, &split.split_id))
                        .ok_or(Reason::TargetUnavailable)?;
                    if canonical
                        != (
                            &split.split_id,
                            &split.first_slot_id,
                            &split.second_slot_id,
                            split.ratio,
                        )
                    {
                        return Err(Reason::StaleView);
                    }
                }
                let change = layout
                    .resize_split(*direction)
                    .map_err(|_| Reason::ProjectionUnavailable)?
                    .map_or(ClientLayoutChange::NoSplit {}, |split| {
                        ClientLayoutChange::Split {
                            split: ClientSplitChange {
                                split_id: split.split_id.clone(),
                                first_slot_id: split.first_slot_id.clone(),
                                second_slot_id: split.second_slot_id.clone(),
                                previous_ratio: split.ratio,
                                ratio: amount.apply(split.ratio, *direction),
                            },
                        }
                    });
                Ok(ClientLayoutAdjustment {
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    workbench_revision: layout.workbench_revision,
                    selected: layout.selected,
                    change,
                })
            }
            ClientViewIntent::SetSplitRatio {
                terminal_context_id,
                tab_id,
                path,
                ratio,
                expected_split,
            } => {
                let canonical = snapshot
                    .terminal_contexts
                    .iter()
                    .find(|context| &context.terminal_context_id == terminal_context_id)
                    .and_then(|context| context.tabs.iter().find(|tab| &tab.tab_id == tab_id))
                    .ok_or(Reason::TargetUnavailable)?;
                let canonical_split =
                    canonical_split_at(&canonical.root, path).ok_or(Reason::TargetUnavailable)?;
                if expected_split.as_ref().is_some_and(|identity| {
                    (canonical_split.0, canonical_split.1, canonical_split.2)
                        != (
                            &identity.split_id,
                            &identity.first_slot_id,
                            &identity.second_slot_id,
                        )
                }) {
                    return Err(Reason::StaleView);
                }
                let selected = state.cache().terminal_context_id() == Some(terminal_context_id)
                    && state
                        .active()
                        .is_some_and(|active| active.key().anchor().tab_id() == tab_id);
                let revision = if selected {
                    if state.cache().revision() < command.minimum_workbench_revision {
                        return Err(Reason::StaleView);
                    }
                    let tab = state
                        .cache()
                        .tab_index(tab_id)
                        .and_then(|index| state.cache().tabs().get(index))
                        .ok_or(Reason::TargetUnavailable)?;
                    let rendered =
                        native_split_at(tab.panel_tree(), path).ok_or(Reason::TargetUnavailable)?;
                    if rendered != canonical_split {
                        return Err(Reason::StaleView);
                    }
                    state.cache().revision()
                } else {
                    snapshot.revision
                };
                let (split, first, second, previous_ratio) = canonical_split;
                Ok(ClientLayoutAdjustment {
                    terminal_context_id: terminal_context_id.clone(),
                    tab_id: tab_id.clone(),
                    workbench_revision: revision,
                    selected,
                    change: ClientLayoutChange::Split {
                        split: ClientSplitChange {
                            split_id: split.clone(),
                            first_slot_id: first.clone(),
                            second_slot_id: second.clone(),
                            previous_ratio,
                            ratio: *ratio,
                        },
                    },
                })
            }
            _ => Err(Reason::Unsupported),
        }
    }

    pub(in crate::workbench_client) fn refresh_layout_adjustment(
        &mut self,
        requested: &mut bool,
    ) -> crate::workbench_client::client_control::ControlStep {
        use crate::workbench_client::client_control::ControlStep;
        if self.state.as_ref().is_some_and(|state| {
            state.pending_refresh_generation().is_some() || state.publication_debt_pending()
        }) {
            return ControlStep::Waiting;
        }
        if *requested {
            // An older read does not invalidate the commit. Publication advances independently,
            // while the owning adjustment bounds this wait without another read or mutation.
            return ControlStep::Waiting;
        }
        let Some(mailbox) = self.mailbox.as_ref().map(std::sync::Arc::clone) else {
            return ControlStep::Failed(Reason::ProjectionUnavailable);
        };
        if let Some(state) = self.state.as_mut() {
            state.note_publication_hint();
        }
        self.drain_publication_refresh(&mailbox);
        *requested = true;
        if self
            .state
            .as_ref()
            .is_some_and(|state| state.pending_refresh_generation().is_some())
        {
            ControlStep::Changed
        } else if self
            .state
            .as_ref()
            .is_some_and(|state| state.publication_debt_pending())
        {
            // The publication owner retains Busy/Full demand; its later drain is a read,
            // never a retry of the committed mutation. The adjustment keeps its deadline.
            ControlStep::Waiting
        } else {
            ControlStep::Failed(Reason::Busy)
        }
    }

    pub(in crate::workbench_client) fn adjustment_observed(
        &self,
        adjustment: &ClientLayoutAdjustment,
        before: &ClientViewObservation,
    ) -> bool {
        let Some((_, view)) = self.control_observation(false) else {
            return false;
        };
        let Some(state) = self.state.as_ref() else {
            return false;
        };
        let ClientLayoutChange::Split { split } = &adjustment.change else {
            return false;
        };
        view.selection == before.selection
            && view.phase == before.phase
            && view.workbench_revision >= adjustment.workbench_revision
            && state.split_ratio(
                &adjustment.terminal_context_id,
                &adjustment.tab_id,
                &split.split_id,
                &split.first_slot_id,
                &split.second_slot_id,
            ) == Some(split.ratio)
    }
}
type SplitIdentity<'a> = (
    &'a WorkbenchSplitId,
    &'a WorkbenchPanelSlotId,
    &'a WorkbenchPanelSlotId,
    orchbee_workbench_protocol::ClientSplitRatio,
);
fn canonical_split_by_id<'a>(
    node: &'a PanelNode,
    id: &WorkbenchSplitId,
) -> Option<SplitIdentity<'a>> {
    let PanelNode::Split {
        split_id,
        children,
        ratio,
        ..
    } = node
    else {
        return None;
    };
    if split_id == id {
        return Some((split_id, &children[0].slot_id, &children[1].slot_id, *ratio));
    }
    children
        .iter()
        .find_map(|child| canonical_split_by_id(&child.node, id))
}
fn canonical_split_at<'a>(
    mut node: &'a PanelNode,
    path: &ClientSplitPath,
) -> Option<SplitIdentity<'a>> {
    for index in path.indices() {
        let PanelNode::Split { children, .. } = node else {
            return None;
        };
        node = &children[usize::from(*index)].node;
    }
    let PanelNode::Split {
        split_id,
        children,
        ratio,
        ..
    } = node
    else {
        return None;
    };
    Some((split_id, &children[0].slot_id, &children[1].slot_id, *ratio))
}
fn native_split_at<'a>(
    mut node: &'a NativePanelTree,
    path: &ClientSplitPath,
) -> Option<SplitIdentity<'a>> {
    for index in path.indices() {
        let NativePanelTree::Split { first, second, .. } = node else {
            return None;
        };
        node = if *index == 0 { first } else { second };
    }
    let NativePanelTree::Split {
        split_id,
        first_slot_id,
        second_slot_id,
        ratio,
        ..
    } = node
    else {
        return None;
    };
    Some((split_id, first_slot_id, second_slot_id, *ratio))
}
