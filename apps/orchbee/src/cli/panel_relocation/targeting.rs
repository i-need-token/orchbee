//! Snapshot-derived relocation identity and navigation preservation decisions.
use crate::{
    cli::{Failure, FailureKind, results},
    invocation::{Resource, TabContext, Target},
};
use orchbee_workbench_protocol::{
    ClientViewSelection, Snapshot, WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Panel {
    pub(super) terminal_context_id: WorkbenchTerminalContextId,
    pub(super) tab_id: WorkbenchTabId,
    pub(super) panel_id: WorkbenchPanelId,
}
impl Panel {
    pub(super) fn same_tab(&self, other: &Self) -> bool {
        self.terminal_context_id == other.terminal_context_id && self.tab_id == other.tab_id
    }
    pub(super) fn exists(&self, snapshot: &Snapshot) -> bool {
        results::panel_entries(snapshot)
            .into_iter()
            .any(|(panel, context, tab)| {
                panel == &self.panel_id
                    && context.terminal_context_id == self.terminal_context_id
                    && tab.tab_id == self.tab_id
            })
    }
}
pub(super) fn panel(snapshot: &Snapshot, target: &Target) -> Result<Panel, Failure> {
    let (id, context, tab) = results::resolve_or_fail(
        results::panel_entries(snapshot)
            .into_iter()
            .map(|(id, context, tab)| (id.as_str(), "", (id, context, tab))),
        target,
        Resource::Panel,
    )?;
    Ok(Panel {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        panel_id: id.clone(),
    })
}
pub(super) fn tab(snapshot: &Snapshot, target: &Target) -> Result<Panel, Failure> {
    let (context, tab) = results::resolve_or_fail(
        snapshot.terminal_contexts.iter().flat_map(|context| {
            context.tabs.iter().enumerate().map(move |(index, tab)| {
                (
                    tab.tab_id.as_str(),
                    tab.display_label(index),
                    (context, tab),
                )
            })
        }),
        target,
        Resource::Tab,
    )?;
    let panel = results::panel_ids_in(&tab.root)
        .first()
        .cloned()
        .cloned()
        .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
    Ok(Panel {
        terminal_context_id: context.terminal_context_id.clone(),
        tab_id: tab.tab_id.clone(),
        panel_id: panel,
    })
}
pub(super) fn context(
    snapshot: &Snapshot,
    target: Option<&TabContext>,
    source: &Panel,
) -> Result<WorkbenchTerminalContextId, Failure> {
    match target {
        None => Ok(source.terminal_context_id.clone()),
        Some(TabContext::Standalone) => {
            Ok(snapshot.navigation.standalone_terminal_context_id.clone())
        }
        Some(TabContext::Workspace(target)) => results::resolve_or_fail(
            snapshot.navigation.workspaces.iter().map(|workspace| {
                (
                    workspace.workspace_id.as_str(),
                    workspace.label.as_str(),
                    workspace,
                )
            }),
            target,
            Resource::Workspace,
        )
        .map(|workspace| workspace.terminal_context_id.clone()),
    }
}
fn selected_panel(selection: &ClientViewSelection) -> Option<Panel> {
    Some(Panel {
        terminal_context_id: selection.terminal_context_id.clone(),
        tab_id: selection.tab_id.clone()?,
        panel_id: selection.panel_id.clone()?,
    })
}
/// Only the same empty view can enter a control that refreshes canonical membership.
pub(super) fn unchanged_empty_selection(
    before: Option<&ClientViewSelection>,
    current: Option<&ClientViewSelection>,
) -> bool {
    before == current
        && before.is_none_or(|selection| {
            selection.tab_id.is_none()
                && selection.panel_id.is_none()
                && selection.panel_count == 0
                && !selection.zoomed
        })
}

/// Preserve stable selection identities; topology repair may only leave a vanished selection.
pub(super) fn follows_original(
    before: Option<&ClientViewSelection>,
    current: Option<&ClientViewSelection>,
    snapshot: &Snapshot,
) -> bool {
    let Some(before) = before else {
        return current.is_none();
    };
    if let Some(panel) = selected_panel(before).filter(|panel| panel.exists(snapshot)) {
        return current.is_some_and(|current| {
            selected_panel(current).as_ref() == Some(&panel) && current.zoomed == before.zoomed
        });
    }
    let old_context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id == before.terminal_context_id);
    let Some(current) = current else {
        return old_context.is_some_and(|context| context.tabs.is_empty());
    };
    let valid = selected_panel(current).is_some_and(|panel| panel.exists(snapshot))
        || (current.tab_id.is_none()
            && current.panel_id.is_none()
            && snapshot.terminal_contexts.iter().any(|context| {
                context.terminal_context_id == current.terminal_context_id
                    && context.tabs.is_empty()
            }));
    if !valid {
        return false;
    }
    match old_context {
        Some(context) => {
            current.terminal_context_id == context.terminal_context_id
                && (before
                    .tab_id
                    .as_ref()
                    .is_none_or(|id| !context.tabs.iter().any(|tab| &tab.tab_id == id))
                    || current.tab_id == before.tab_id)
        }
        // Removed Workspace repair lands in standalone, not an arbitrary peer Workspace.
        None => current.terminal_context_id == snapshot.navigation.standalone_terminal_context_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn id(n: u8) -> String {
        format!("{n:032x}")
    }
    fn fixture() -> Snapshot {
        let tab = |t, p| {
            json!({"tabId":id(t),"label":"shell","pinned":false,"groupId":null,
            "root":{"kind":"panel","panelId":id(p),"primaryPresentation":{"presentationId":id(p+40),"label":"shell","binding":null},"childPresentations":[]}})
        };
        serde_json::from_value(json!({"protocolVersion":1,"siteId":id(1),"revision":"2",
            "navigation":{"standaloneTerminalContextId":id(2), "projects":[{"projectId":id(10), "label":"Project"}], "directories":[], "workspaces":[{"workspaceId":id(11), "label":"Work", "terminalContextId":id(12), "primaryDirectoryId":null, "projectId":id(10)}]},
            "terminalContexts":[{"terminalContextId":id(2),"groups":[],"tabs":[tab(3,4),tab(5,6)]},
                {"terminalContextId":id(12),"groups":[],"tabs":[tab(13,14)]}]})).unwrap()
    }
    fn selected(c: u8, t: u8, p: u8) -> ClientViewSelection {
        ClientViewSelection {
            terminal_context_id: WorkbenchTerminalContextId::parse(&id(c)).unwrap(),
            tab_id: Some(WorkbenchTabId::parse(&id(t)).unwrap()),
            panel_id: Some(WorkbenchPanelId::parse(&id(p)).unwrap()),
            zoomed: false,
            panel_count: 1,
        }
    }
    #[test]
    fn panel_relocation_resolves_across_workspaces_and_rejects_ambiguous_tabs() {
        let snapshot = fixture();
        let source = panel(&snapshot, &Target::Id(id(4))).unwrap();
        let target = tab(&snapshot, &Target::Id(id(13))).unwrap();
        assert!(!source.same_tab(&target));
        assert!(target.exists(&snapshot));
        assert_eq!(
            context(
                &snapshot,
                Some(&TabContext::Workspace(Target::Label("Work".into()))),
                &source
            )
            .unwrap(),
            target.terminal_context_id
        );
        assert_eq!(
            tab(&snapshot, &Target::Label("shell".into()))
                .unwrap_err()
                .kind,
            FailureKind::Ambiguous
        );
        assert!(
            !Panel {
                tab_id: source.tab_id,
                ..target
            }
            .exists(&snapshot)
        );
    }
    #[test]
    fn panel_relocation_preserves_unaffected_selection_and_restricts_removal_repair() {
        let mut snapshot = fixture();
        let before = selected(2, 3, 4);
        assert!(follows_original(Some(&before), Some(&before), &snapshot));
        let other = selected(2, 5, 6);
        assert!(!follows_original(Some(&before), Some(&other), &snapshot));
        let mut zoomed = before.clone();
        zoomed.zoomed = true;
        assert!(!follows_original(Some(&before), Some(&zoomed), &snapshot));
        snapshot.terminal_contexts[0].tabs.remove(0);
        assert!(follows_original(Some(&before), Some(&other), &snapshot));
        assert!(!follows_original(
            Some(&before),
            Some(&selected(12, 13, 14)),
            &snapshot
        ));
        let workspace_before = selected(12, 13, 14);
        snapshot.terminal_contexts.remove(1);
        snapshot.navigation.workspaces.clear();
        assert!(follows_original(
            Some(&workspace_before),
            Some(&other),
            &snapshot
        ));
        assert!(!follows_original(
            Some(&workspace_before),
            Some(&workspace_before),
            &snapshot
        ));
        snapshot.terminal_contexts[0].tabs.clear();
        let empty = ClientViewSelection {
            tab_id: None,
            panel_id: None,
            panel_count: 0,
            ..other
        };
        assert!(unchanged_empty_selection(None, None));
        assert!(unchanged_empty_selection(Some(&empty), Some(&empty)));
        assert!(!unchanged_empty_selection(Some(&before), Some(&before)));
        assert!(!unchanged_empty_selection(Some(&empty), None));
        let other_empty = ClientViewSelection {
            terminal_context_id: workspace_before.terminal_context_id.clone(),
            ..empty.clone()
        };
        assert!(!unchanged_empty_selection(Some(&empty), Some(&other_empty)));
        assert!(follows_original(Some(&before), Some(&empty), &snapshot));
        assert!(follows_original(
            Some(&workspace_before),
            Some(&empty),
            &snapshot
        ));
    }
}
