//! Pure projection of a validated Workbench `Snapshot` into the CLI result
//! model. Everything here is total over a snapshot the transport already
//! verified; no fabricated fields appear where no authoritative source exists.

use std::collections::HashMap;

use orchbee_workbench_protocol::{
    PanelNode, PanelSlot, Presentation, Project, Snapshot, Tab, TerminalContext,
    WorkbenchProtocolDocument, Workspace,
};
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, target_details};
use crate::invocation::{QueryCommand, Resource, Target};

pub(crate) fn schema_id(command: &QueryCommand) -> &'static str {
    match command {
        QueryCommand::Status => "orchbee.status",
        QueryCommand::ProjectList => "orchbee.project.list",
        QueryCommand::ProjectGet(_) => "orchbee.project.get",
        QueryCommand::WorkspaceList { .. } => "orchbee.workspace.list",
        QueryCommand::WorkspaceGet(_) => "orchbee.workspace.get",
        QueryCommand::TabList { .. } => "orchbee.tab.list",
        QueryCommand::TabGet(_) => "orchbee.tab.get",
        QueryCommand::PanelList { .. } => "orchbee.panel.list",
        QueryCommand::PanelGet(_) => "orchbee.panel.get",
        QueryCommand::TerminalList => "orchbee.terminal.list",
        QueryCommand::TerminalGet(_) => "orchbee.terminal.get",
        QueryCommand::ApiSnapshot => "orchbee.api.snapshot",
        QueryCommand::ApiSchema { .. } => "orchbee.api.schema",
        QueryCommand::ConfigCheck => "orchbee.config.check",
    }
}

/// `source` for every snapshot-derived answer.
pub(crate) fn snapshot_source(snapshot: &Snapshot) -> Value {
    json!({
        "kind": "workbench-snapshot",
        "siteId": snapshot.site_id.as_str(),
        "revision": snapshot.revision,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct StatusDoc {
    pub(crate) site_id: String,
    pub(crate) revision: String,
    pub(crate) projects: usize,
    pub(crate) workspaces: usize,
    pub(crate) tabs: usize,
    pub(crate) panels: usize,
    pub(crate) bound_terminals: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectRow {
    pub(crate) project_id: String,
    pub(crate) label: String,
    pub(crate) directory_count: usize,
    pub(crate) workspace_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct DirectoryRow {
    pub(crate) directory_id: String,
    pub(crate) worker_id: String,
    pub(crate) path: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRef {
    pub(crate) workspace_id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ProjectDoc {
    pub(crate) row: ProjectRow,
    pub(crate) directories: Vec<DirectoryRow>,
    pub(crate) workspaces: Vec<WorkspaceRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRow {
    pub(crate) workspace_id: String,
    pub(crate) label: String,
    pub(crate) project_id: String,
    pub(crate) primary_directory_id: Option<String>,
    pub(crate) terminal_context_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TabRef {
    pub(crate) tab_id: String,
    pub(crate) label: String,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceDoc {
    pub(crate) row: WorkspaceRow,
    pub(crate) tabs: Vec<TabRef>,
}

#[derive(Clone, Debug)]
pub(crate) struct ContextRef {
    pub(crate) standalone: bool,
    pub(crate) terminal_context_id: String,
    pub(crate) workspace_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TabRow {
    pub(crate) tab_id: String,
    pub(crate) label: String,
    pub(crate) pinned: bool,
    pub(crate) group_id: Option<String>,
    pub(crate) context: ContextRef,
}

#[derive(Clone, Debug)]
pub(crate) struct TabDoc {
    pub(crate) row: TabRow,
    /// Semantic re-encode of the whole PanelNode subtree.
    pub(crate) root: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct PanelRow {
    pub(crate) panel_id: String,
    pub(crate) tab_id: String,
    pub(crate) terminal_context_id: String,
    pub(crate) primary_presentation_id: String,
    pub(crate) presentation_count: usize,
    pub(crate) primary_terminal_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PanelDoc {
    pub(crate) row: PanelRow,
    pub(crate) context: ContextRef,
    pub(crate) slot_path: Vec<String>,
    pub(crate) primary_presentation: PresentationRow,
    pub(crate) child_presentations: Vec<PresentationRow>,
}

#[derive(Clone, Debug)]
pub(crate) struct PresentationRow {
    pub(crate) presentation_id: String,
    pub(crate) label: String,
    pub(crate) terminal_id: Option<String>,
    pub(crate) worker_id: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalRow {
    pub(crate) terminal_id: String,
    pub(crate) worker_id: String,
    pub(crate) presentation_id: String,
    pub(crate) panel_id: String,
    pub(crate) tab_id: String,
    pub(crate) terminal_context_id: String,
}

pub(crate) enum Projection {
    Status(StatusDoc),
    ProjectList(Vec<ProjectRow>),
    ProjectGet(ProjectDoc),
    WorkspaceList(Vec<WorkspaceRow>),
    WorkspaceGet(WorkspaceDoc),
    TabList(Vec<TabRow>),
    TabGet(TabDoc),
    PanelList(Vec<PanelRow>),
    PanelGet(PanelDoc),
    TerminalList(Vec<TerminalRow>),
    TerminalGet(TerminalRow),
    ApiSnapshot(Value),
}

impl Projection {
    /// The `result` member of the machine envelope.
    pub(crate) fn result(&self) -> Value {
        match self {
            Self::Status(doc) => json!({
                "site": {
                    "reachable": true,
                    "siteId": doc.site_id,
                    "revision": doc.revision,
                },
                "counts": {
                    "projects": doc.projects,
                    "workspaces": doc.workspaces,
                    "tabs": doc.tabs,
                    "panels": doc.panels,
                    "boundTerminals": doc.bound_terminals,
                },
                "worker": {
                    "status": "unknown",
                    "reason": "not-in-workbench-snapshot",
                },
            }),
            Self::ProjectList(rows) => json!({
                "projects": rows
                    .iter()
                    .map(|row| {
                        json!({
                            "projectId": row.project_id,
                            "label": row.label,
                            "directoryCount": row.directory_count,
                            "workspaceCount": row.workspace_count,
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            Self::ProjectGet(doc) => json!({
                "project": {
                    "projectId": doc.row.project_id,
                    "label": doc.row.label,
                    "directories": doc
                        .directories
                        .iter()
                        .map(|directory| {
                            json!({
                                "directoryId": directory.directory_id,
                                "workerId": directory.worker_id,
                                "path": directory.path,
                            })
                        })
                        .collect::<Vec<_>>(),
                    "workspaces": doc
                        .workspaces
                        .iter()
                        .map(|workspace| {
                            json!({
                                "workspaceId": workspace.workspace_id,
                                "label": workspace.label,
                            })
                        })
                        .collect::<Vec<_>>(),
                },
            }),
            Self::WorkspaceList(rows) => json!({
                "workspaces": rows.iter().map(workspace_row_json).collect::<Vec<_>>(),
            }),
            Self::WorkspaceGet(doc) => {
                let mut workspace = match workspace_row_json(&doc.row) {
                    Value::Object(object) => object,
                    _ => unreachable!("workspace rows always encode as objects"),
                };
                workspace.insert(
                    "tabs".to_owned(),
                    json!(
                        doc.tabs
                            .iter()
                            .map(|tab| {
                                json!({
                                    "tabId": tab.tab_id,
                                    "label": tab.label,
                                })
                            })
                            .collect::<Vec<_>>()
                    ),
                );
                json!({ "workspace": workspace })
            }
            Self::TabList(rows) => json!({
                "tabs": rows.iter().map(tab_row_json).collect::<Vec<_>>(),
            }),
            Self::TabGet(doc) => {
                let mut tab = match tab_row_json(&doc.row) {
                    Value::Object(object) => object,
                    _ => unreachable!("tab rows always encode as objects"),
                };
                tab.insert("root".to_owned(), doc.root.clone());
                json!({ "tab": tab })
            }
            Self::PanelList(rows) => json!({
                "panels": rows
                    .iter()
                    .map(|row| {
                        json!({
                            "panelId": row.panel_id,
                            "tabId": row.tab_id,
                            "terminalContextId": row.terminal_context_id,
                            "primaryPresentationId": row.primary_presentation_id,
                            "presentationCount": row.presentation_count,
                            "primaryTerminalId": row.primary_terminal_id,
                        })
                    })
                    .collect::<Vec<_>>(),
            }),
            Self::PanelGet(doc) => json!({
                "panel": {
                    "panelId": doc.row.panel_id,
                    "tabId": doc.row.tab_id,
                    "context": context_json(&doc.context),
                    "slotPath": doc.slot_path,
                    "primaryPresentation": presentation_json(&doc.primary_presentation),
                    "childPresentations": doc
                        .child_presentations
                        .iter()
                        .map(presentation_json)
                        .collect::<Vec<_>>(),
                },
            }),
            Self::TerminalList(rows) => json!({
                "terminals": rows.iter().map(terminal_row_json).collect::<Vec<_>>(),
            }),
            Self::TerminalGet(row) => json!({ "terminal": terminal_row_json(row) }),
            Self::ApiSnapshot(document) => json!({ "snapshot": document }),
        }
    }
}

fn workspace_row_json(row: &WorkspaceRow) -> Value {
    json!({
        "workspaceId": row.workspace_id,
        "label": row.label,
        "projectId": row.project_id,
        "primaryDirectoryId": row.primary_directory_id,
        "terminalContextId": row.terminal_context_id,
    })
}

fn context_json(context: &ContextRef) -> Value {
    let mut object = Map::from_iter([
        (
            "kind".to_owned(),
            json!(if context.standalone {
                "standalone"
            } else {
                "workspace"
            }),
        ),
        (
            "terminalContextId".to_owned(),
            json!(context.terminal_context_id),
        ),
    ]);
    if let Some(workspace_id) = &context.workspace_id {
        object.insert("workspaceId".to_owned(), json!(workspace_id));
    }
    Value::Object(object)
}

fn tab_row_json(row: &TabRow) -> Value {
    json!({
        "tabId": row.tab_id,
        "label": row.label,
        "pinned": row.pinned,
        "groupId": row.group_id,
        "context": context_json(&row.context),
    })
}

fn presentation_json(row: &PresentationRow) -> Value {
    json!({
        "presentationId": row.presentation_id,
        "label": row.label,
        "binding": match (&row.terminal_id, &row.worker_id) {
            (Some(terminal_id), Some(worker_id)) => json!({
                "terminalId": terminal_id,
                "workerId": worker_id,
            }),
            _ => Value::Null,
        },
    })
}

fn terminal_row_json(row: &TerminalRow) -> Value {
    json!({
        "terminalId": row.terminal_id,
        "workerId": row.worker_id,
        "boundTo": {
            "presentationId": row.presentation_id,
            "panelId": row.panel_id,
            "tabId": row.tab_id,
            "terminalContextId": row.terminal_context_id,
        },
    })
}

/// context id → owning workspace id for every workspace-bound context.
struct ContextIndex<'a> {
    standalone_id: &'a str,
    owners: HashMap<&'a str, &'a str>,
}

impl<'a> ContextIndex<'a> {
    fn new(snapshot: &'a Snapshot) -> Self {
        let mut owners = HashMap::new();
        for project in &snapshot.navigation.projects {
            for workspace in &project.workspaces {
                owners.insert(
                    workspace.terminal_context_id.as_str(),
                    workspace.workspace_id.as_str(),
                );
            }
        }
        Self {
            standalone_id: snapshot.navigation.standalone_terminal_context_id.as_str(),
            owners,
        }
    }

    fn reference(&self, context: &'a TerminalContext) -> ContextRef {
        let workspace_id = self.owners.get(context.terminal_context_id.as_str());
        ContextRef {
            standalone: context.terminal_context_id.as_str() == self.standalone_id,
            terminal_context_id: context.terminal_context_id.as_str().to_owned(),
            workspace_id: workspace_id.map(|id| (*id).to_owned()),
        }
    }
}

/// A typed target hits identity first; only a non-ID token becomes an exact
/// case-sensitive label search.
fn resolve<'a, Item>(
    mut items: impl Iterator<Item = (&'a str, &'a str, Item)>,
    target: &Target,
) -> Result<Item, Vec<&'a str>> {
    match target {
        Target::Id(identity) => items
            .find(|(id, _label, _item)| *id == identity.as_str())
            .map(|(_id, _label, item)| item)
            .ok_or_else(Vec::new),
        Target::Label(label) => {
            let mut matches = items
                .filter(|(_id, item_label, _item)| *item_label == label.as_str())
                .collect::<Vec<_>>();
            match matches.len() {
                0 => Err(Vec::new()),
                1 => Ok(matches.pop().expect("len checked").2),
                _ => Err(matches.into_iter().map(|(id, _label, _item)| id).collect()),
            }
        }
    }
}

fn not_found(resource: Resource, target: &Target) -> Failure {
    Failure::new(FailureKind::NotFound).details(target_details(resource, target.text()))
}

fn ambiguous(resource: Resource, target: &Target, mut candidates: Vec<String>) -> Failure {
    let total = candidates.len();
    candidates.truncate(8);
    Failure::new(FailureKind::Ambiguous).details(Map::from_iter([
        ("resource".to_owned(), json!(resource.as_str())),
        ("target".to_owned(), json!(target.text())),
        ("candidates".to_owned(), json!(candidates)),
        ("totalCount".to_owned(), json!(total)),
    ]))
}

fn resolve_or_fail<'a, Item>(
    items: impl Iterator<Item = (&'a str, &'a str, Item)>,
    target: &Target,
    resource: Resource,
) -> Result<Item, Failure> {
    match resolve(items, target) {
        Ok(item) => Ok(item),
        Err(candidates) if candidates.is_empty() => Err(not_found(resource, target)),
        Err(candidates) => Err(ambiguous(
            resource,
            target,
            candidates.into_iter().map(str::to_owned).collect(),
        )),
    }
}

/// Run one snapshot-derived command over a validated snapshot.
pub(crate) fn project(command: &QueryCommand, snapshot: &Snapshot) -> Result<Projection, Failure> {
    let index = ContextIndex::new(snapshot);
    match command {
        QueryCommand::Status => Ok(Projection::Status(status(snapshot))),
        QueryCommand::ProjectList => Ok(Projection::ProjectList(
            snapshot
                .navigation
                .projects
                .iter()
                .map(project_row)
                .collect(),
        )),
        QueryCommand::ProjectGet(target) => {
            let project =
                resolve_or_fail(
                    snapshot.navigation.projects.iter().map(|project| {
                        (project.project_id.as_str(), project.label.as_str(), project)
                    }),
                    target,
                    Resource::Project,
                )?;
            Ok(Projection::ProjectGet(project_doc(project)))
        }
        QueryCommand::WorkspaceList { project } => {
            let projects: Vec<&Project> = match project {
                Some(target) => vec![resolve_or_fail(
                    snapshot.navigation.projects.iter().map(|project| {
                        (project.project_id.as_str(), project.label.as_str(), project)
                    }),
                    target,
                    Resource::Project,
                )?],
                None => snapshot.navigation.projects.iter().collect(),
            };
            Ok(Projection::WorkspaceList(
                projects
                    .iter()
                    .flat_map(|project| {
                        project
                            .workspaces
                            .iter()
                            .map(|workspace| workspace_row(project, workspace))
                    })
                    .collect(),
            ))
        }
        QueryCommand::WorkspaceGet(target) => {
            let (project, workspace) = resolve_or_fail(
                snapshot.navigation.projects.iter().flat_map(|project| {
                    project.workspaces.iter().map(move |workspace| {
                        (
                            workspace.workspace_id.as_str(),
                            workspace.label.as_str(),
                            (project, workspace),
                        )
                    })
                }),
                target,
                Resource::Workspace,
            )?;
            Ok(Projection::WorkspaceGet(workspace_doc(
                snapshot, project, workspace,
            )?))
        }
        QueryCommand::TabList {
            workspace,
            standalone,
        } => {
            let contexts: Vec<&TerminalContext> = if *standalone {
                snapshot
                    .terminal_contexts
                    .iter()
                    .filter(|context| context.terminal_context_id.as_str() == index.standalone_id)
                    .collect()
            } else if let Some(target) = workspace {
                let (_project, workspace) = resolve_or_fail(
                    snapshot.navigation.projects.iter().flat_map(|project| {
                        project.workspaces.iter().map(move |workspace| {
                            (
                                workspace.workspace_id.as_str(),
                                workspace.label.as_str(),
                                (project, workspace),
                            )
                        })
                    }),
                    target,
                    Resource::Workspace,
                )?;
                snapshot
                    .terminal_contexts
                    .iter()
                    .filter(|context| context.terminal_context_id == workspace.terminal_context_id)
                    .collect()
            } else {
                snapshot.terminal_contexts.iter().collect()
            };
            Ok(Projection::TabList(
                contexts
                    .iter()
                    .flat_map(|context| {
                        context
                            .tabs
                            .iter()
                            .map(|tab| tab_row(tab, index.reference(context)))
                    })
                    .collect(),
            ))
        }
        QueryCommand::TabGet(target) => {
            let (context, tab) = resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context
                        .tabs
                        .iter()
                        .map(move |tab| (tab.tab_id.as_str(), tab.label.as_str(), (context, tab)))
                }),
                target,
                Resource::Tab,
            )?;
            Ok(Projection::TabGet(TabDoc {
                row: tab_row(tab, index.reference(context)),
                root: serde_json::to_value(&tab.root)
                    .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
            }))
        }
        QueryCommand::PanelList { tab } => {
            let tabs: Vec<(&TerminalContext, &Tab)> = match tab {
                Some(target) => {
                    let resolved = resolve_or_fail(
                        snapshot.terminal_contexts.iter().flat_map(|context| {
                            context.tabs.iter().map(move |tab| {
                                (tab.tab_id.as_str(), tab.label.as_str(), (context, tab))
                            })
                        }),
                        target,
                        Resource::Tab,
                    )?;
                    vec![resolved]
                }
                None => snapshot
                    .terminal_contexts
                    .iter()
                    .flat_map(|context| context.tabs.iter().map(move |tab| (context, tab)))
                    .collect(),
            };
            let mut rows = Vec::new();
            for (context, tab) in tabs {
                let mut path = Vec::new();
                collect_panels(&tab.root, context, tab, &mut path, &mut |row, _path| {
                    rows.push(row);
                });
            }
            Ok(Projection::PanelList(rows))
        }
        QueryCommand::PanelGet(target) => {
            let identity = match target {
                Target::Id(identity) => identity.as_str(),
                Target::Label(_) => {
                    return Err(Failure::new(FailureKind::InternalFailure));
                }
            };
            for context in &snapshot.terminal_contexts {
                for tab in &context.tabs {
                    let mut path = Vec::new();
                    let mut found = None;
                    collect_panels(&tab.root, context, tab, &mut path, &mut |row, slot_path| {
                        if row.panel_id == identity {
                            found = Some((row, slot_path.to_vec()));
                        }
                    });
                    if let Some((row, slot_path)) = found {
                        let Some((primary, children)) = panel_presentations(&tab.root, identity)
                        else {
                            return Err(Failure::new(FailureKind::InternalFailure));
                        };
                        return Ok(Projection::PanelGet(PanelDoc {
                            row,
                            context: index.reference(context),
                            slot_path,
                            primary_presentation: primary,
                            child_presentations: children,
                        }));
                    }
                }
            }
            Err(not_found(Resource::Panel, target))
        }
        QueryCommand::TerminalList => Ok(Projection::TerminalList(terminal_rows(snapshot))),
        QueryCommand::TerminalGet(target) => {
            let identity = match target {
                Target::Id(identity) => identity.as_str(),
                Target::Label(_) => {
                    return Err(Failure::new(FailureKind::InternalFailure));
                }
            };
            terminal_rows(snapshot)
                .into_iter()
                .find(|row| row.terminal_id == identity)
                .map(Projection::TerminalGet)
                .ok_or_else(|| not_found(Resource::Terminal, target))
        }
        QueryCommand::ApiSnapshot => Ok(Projection::ApiSnapshot(
            serde_json::to_value(WorkbenchProtocolDocument::Snapshot(snapshot.clone()))
                .map_err(|_| Failure::new(FailureKind::InternalFailure))?,
        )),
        QueryCommand::ApiSchema { .. } | QueryCommand::ConfigCheck => {
            Err(Failure::new(FailureKind::InternalFailure))
        }
    }
}

fn status(snapshot: &Snapshot) -> StatusDoc {
    let mut tabs = 0_usize;
    let mut panels = 0_usize;
    let mut bound_terminals = 0_usize;
    for context in &snapshot.terminal_contexts {
        tabs += context.tabs.len();
        for tab in &context.tabs {
            let mut path = Vec::new();
            collect_panel_nodes(&tab.root, &mut path, &mut |node| {
                panels += 1;
                if let PanelNode::Panel {
                    primary_presentation,
                    child_presentations,
                    ..
                } = node
                {
                    bound_terminals += primary_presentation
                        .binding
                        .iter()
                        .chain(child_presentations.iter().flat_map(|p| p.binding.iter()))
                        .count();
                }
            });
        }
    }
    StatusDoc {
        site_id: snapshot.site_id.as_str().to_owned(),
        revision: snapshot.revision.to_string(),
        projects: snapshot.navigation.projects.len(),
        workspaces: snapshot
            .navigation
            .projects
            .iter()
            .map(|project| project.workspaces.len())
            .sum(),
        tabs,
        panels,
        bound_terminals,
    }
}

fn project_row(project: &Project) -> ProjectRow {
    ProjectRow {
        project_id: project.project_id.as_str().to_owned(),
        label: project.label.clone(),
        directory_count: project.directories.len(),
        workspace_count: project.workspaces.len(),
    }
}

fn project_doc(project: &Project) -> ProjectDoc {
    ProjectDoc {
        row: project_row(project),
        directories: project
            .directories
            .iter()
            .map(|directory| DirectoryRow {
                directory_id: directory.directory_id.as_str().to_owned(),
                worker_id: directory.worker_id.as_str().to_owned(),
                path: directory.path.as_str().to_owned(),
            })
            .collect(),
        workspaces: project
            .workspaces
            .iter()
            .map(|workspace| WorkspaceRef {
                workspace_id: workspace.workspace_id.as_str().to_owned(),
                label: workspace.label.clone(),
            })
            .collect(),
    }
}

fn workspace_row(project: &Project, workspace: &Workspace) -> WorkspaceRow {
    WorkspaceRow {
        workspace_id: workspace.workspace_id.as_str().to_owned(),
        label: workspace.label.clone(),
        project_id: project.project_id.as_str().to_owned(),
        primary_directory_id: workspace
            .primary_directory_id
            .as_ref()
            .map(|id| id.as_str().to_owned()),
        terminal_context_id: workspace.terminal_context_id.as_str().to_owned(),
    }
}

fn workspace_doc(
    snapshot: &Snapshot,
    project: &Project,
    workspace: &Workspace,
) -> Result<WorkspaceDoc, Failure> {
    let context_id = workspace.terminal_context_id.as_str();
    let context = snapshot
        .terminal_contexts
        .iter()
        .find(|context| context.terminal_context_id.as_str() == context_id)
        .ok_or_else(|| Failure::new(FailureKind::InternalFailure))?;
    Ok(WorkspaceDoc {
        row: workspace_row(project, workspace),
        tabs: context
            .tabs
            .iter()
            .map(|tab| TabRef {
                tab_id: tab.tab_id.as_str().to_owned(),
                label: tab.label.clone(),
            })
            .collect(),
    })
}

fn tab_row(tab: &Tab, context: ContextRef) -> TabRow {
    TabRow {
        tab_id: tab.tab_id.as_str().to_owned(),
        label: tab.label.clone(),
        pinned: tab.pinned,
        group_id: tab.group_id.as_ref().map(|id| id.as_str().to_owned()),
        context,
    }
}

fn presentation_row(presentation: &Presentation) -> PresentationRow {
    PresentationRow {
        presentation_id: presentation.presentation_id.as_str().to_owned(),
        label: presentation.label.clone(),
        terminal_id: presentation
            .binding
            .as_ref()
            .map(|binding| binding.terminal_id.as_str().to_owned()),
        worker_id: presentation
            .binding
            .as_ref()
            .map(|binding| binding.worker_id.as_str().to_owned()),
    }
}

fn collect_panel_nodes<'a>(
    node: &'a PanelNode,
    path: &mut Vec<String>,
    visit: &mut impl FnMut(&'a PanelNode),
) {
    match node {
        PanelNode::Panel { .. } => visit(node),
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                let PanelSlot { node, slot_id } = slot;
                path.push(slot_id.as_str().to_owned());
                collect_panel_nodes(node, path, visit);
                path.pop();
            }
        }
    }
}

fn collect_panels<'a>(
    node: &'a PanelNode,
    context: &'a TerminalContext,
    tab: &'a Tab,
    path: &mut Vec<String>,
    visit: &mut impl FnMut(PanelRow, &[String]),
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
        } => visit(
            PanelRow {
                panel_id: panel_id.as_str().to_owned(),
                tab_id: tab.tab_id.as_str().to_owned(),
                terminal_context_id: context.terminal_context_id.as_str().to_owned(),
                primary_presentation_id: primary_presentation.presentation_id.as_str().to_owned(),
                presentation_count: 1 + child_presentations.len(),
                primary_terminal_id: primary_presentation
                    .binding
                    .as_ref()
                    .map(|binding| binding.terminal_id.as_str().to_owned()),
            },
            path,
        ),
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                path.push(slot.slot_id.as_str().to_owned());
                collect_panels(&slot.node, context, tab, path, visit);
                path.pop();
            }
        }
    }
}

/// Find the Panel node by id and extract its presentation rows.
fn panel_presentations(
    node: &PanelNode,
    panel_id: &str,
) -> Option<(PresentationRow, Vec<PresentationRow>)> {
    match node {
        PanelNode::Panel {
            panel_id: id,
            primary_presentation,
            child_presentations,
        } if id.as_str() == panel_id => Some((
            presentation_row(primary_presentation),
            child_presentations.iter().map(presentation_row).collect(),
        )),
        PanelNode::Split { children, .. } => children
            .iter()
            .find_map(|slot| panel_presentations(&slot.node, panel_id)),
        PanelNode::Panel { .. } => None,
    }
}

/// Every bound terminal exactly once, in canonical traversal order. Validation
/// already guarantees bound terminal ids never repeat across the snapshot.
fn terminal_rows(snapshot: &Snapshot) -> Vec<TerminalRow> {
    let mut rows = Vec::new();
    for context in &snapshot.terminal_contexts {
        for tab in &context.tabs {
            collect_terminal_rows(&tab.root, context, tab, &mut rows);
        }
    }
    rows
}

fn collect_terminal_rows(
    node: &PanelNode,
    context: &TerminalContext,
    tab: &Tab,
    rows: &mut Vec<TerminalRow>,
) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            child_presentations,
        } => {
            for presentation in
                std::iter::once(primary_presentation).chain(child_presentations.iter())
            {
                if let Some(binding) = &presentation.binding {
                    rows.push(TerminalRow {
                        terminal_id: binding.terminal_id.as_str().to_owned(),
                        worker_id: binding.worker_id.as_str().to_owned(),
                        presentation_id: presentation.presentation_id.as_str().to_owned(),
                        panel_id: panel_id.as_str().to_owned(),
                        tab_id: tab.tab_id.as_str().to_owned(),
                        terminal_context_id: context.terminal_context_id.as_str().to_owned(),
                    });
                }
            }
        }
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                collect_terminal_rows(&slot.node, context, tab, rows);
            }
        }
    }
}
