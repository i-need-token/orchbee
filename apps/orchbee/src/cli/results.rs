//! Pure projection of a validated Workbench `Snapshot` into the CLI result
//! model. Everything here is total over a snapshot the transport already
//! verified; no fabricated fields appear where no authoritative source exists.

use std::collections::HashMap;

use orchbee_terminal_protocol::MAXIMUM_FRAME_ROWS;
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
        QueryCommand::TerminalRead(_) => "orchbee.terminal.read",
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

/// Routing metadata of the applied projection, re-encoded as decimal strings
/// so no u64 ever loses precision inside a machine document.
#[derive(Clone, Debug)]
pub(crate) struct TerminalFrameDoc {
    pub(crate) revision: u64,
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) alternate_screen: bool,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    pub(crate) cursor_row: u16,
    pub(crate) cursor_column: u16,
    pub(crate) cursor_visible: bool,
}

/// One grid row emitted in the answer: absolute index, joined cell text, and
/// the soft-wrap continuation flags the frame carried.
#[derive(Clone, Debug)]
pub(crate) struct TerminalContentRow {
    pub(crate) index: u16,
    pub(crate) text: String,
    pub(crate) from_previous: bool,
    pub(crate) into_next: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TerminalReadDoc {
    pub(crate) terminal: TerminalRow,
    pub(crate) frame: TerminalFrameDoc,
    /// Emitted rows only; `row_count` counts every row in the applied frame.
    pub(crate) rows: Vec<TerminalContentRow>,
    pub(crate) row_count: usize,
    pub(crate) omitted_rows: usize,
}

/// The read bound: the summed JSON-encoded `text` member length of emitted
/// rows never exceeds this budget, so every format stays far under the
/// 8 MiB preparation cap even on hostile cell content.
pub(crate) const READ_TEXT_BYTES: usize = 4 * 1024 * 1024;

/// Project the applied frame into the bounded answer: rows are emitted in
/// frame order until the encoded-text budget would overflow, and every
/// omitted row is counted honestly.
#[cfg(unix)]
pub(crate) fn terminal_read_doc(
    terminal: TerminalRow,
    frame: &orchbee_terminal_protocol::TerminalTextFrame,
) -> Result<TerminalReadDoc, Failure> {
    let row_count = frame.rows.len();
    let mut budget = READ_TEXT_BYTES;
    let mut rows = Vec::new();
    for row in &frame.rows {
        let mut text = String::new();
        for cell in &row.cells {
            // A spacer tail is already covered by its wide cell; a spacer
            // head or an empty cell renders as one space.
            if cell.width == 2 {
                continue;
            }
            if cell.width == 3 || cell.text.is_empty() {
                text.push(' ');
            } else {
                text.push_str(&cell.text);
            }
        }
        // Trailing cells are grid padding, never content: a blank-only row
        // emits an empty line.
        while text.ends_with(' ') {
            text.pop();
        }
        let encoded =
            serde_json::to_string(&text).map_err(|_| Failure::new(FailureKind::InternalFailure))?;
        if encoded.len() > budget {
            break;
        }
        budget -= encoded.len();
        rows.push(TerminalContentRow {
            index: row.index,
            text,
            // Wire bits: bit 0 is `wrap` (this row continues into the next),
            // bit 1 is `wrap_continuation` (this row continues from the
            // previous); the decoder pairs them across adjacent rows.
            from_previous: row.flags & 2 != 0,
            into_next: row.flags & 1 != 0,
        });
    }
    let doc = TerminalFrameDoc {
        revision: frame.route.revision,
        generation: frame.route.generation.get(),
        sequence: frame.route.sequence.get(),
        alternate_screen: frame.alternate_screen,
        columns: frame.geometry.columns,
        rows: frame.geometry.rows,
        cursor_row: frame.cursor.row,
        cursor_column: frame.cursor.column,
        cursor_visible: frame.cursor.visible,
    };
    Ok(TerminalReadDoc {
        terminal,
        frame: doc,
        omitted_rows: row_count - rows.len(),
        rows,
        row_count,
    })
}

/// The bound terminal row for a typed identity, or `None` when no
/// presentation binds it — the same visibility rule as `terminal get`.
pub(crate) fn terminal_binding(snapshot: &Snapshot, identity: &str) -> Option<TerminalRow> {
    terminal_rows(snapshot)
        .into_iter()
        .find(|row| row.terminal_id == identity)
}

#[derive(Debug)]
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
    TerminalRead(TerminalReadDoc),
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
            Self::TerminalRead(doc) => json!({
                "terminal": terminal_row_json(&doc.terminal),
                "frame": {
                    "revision": doc.frame.revision.to_string(),
                    "generation": doc.frame.generation.to_string(),
                    "sequence": doc.frame.sequence.to_string(),
                    "alternateScreen": doc.frame.alternate_screen,
                    "geometry": {
                        "columns": doc.frame.columns,
                        "rows": doc.frame.rows,
                    },
                    "cursor": {
                        "row": doc.frame.cursor_row,
                        "column": doc.frame.cursor_column,
                        "visible": doc.frame.cursor_visible,
                    },
                },
                "content": {
                    "rows": doc
                        .rows
                        .iter()
                        .map(|row| {
                            json!({
                                "index": row.index,
                                "text": row.text,
                                "continuation": {
                                    "fromPrevious": row.from_previous,
                                    "intoNext": row.into_next,
                                },
                            })
                        })
                        .collect::<Vec<_>>(),
                    "rowCount": doc.row_count,
                    "truncated": doc.omitted_rows > 0,
                    "omittedRows": doc.omitted_rows,
                    "limits": {
                        "rows": MAXIMUM_FRAME_ROWS,
                        "textBytes": READ_TEXT_BYTES,
                    },
                },
            }),
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

pub(crate) fn not_found(resource: Resource, target: &Target) -> Failure {
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

pub(crate) fn resolve_or_fail<'a, Item>(
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
        // `terminal read` projects a live frame, not snapshot data alone; its
        // answer is built by the query layer. `api schema`/`config check`
        // never reach a snapshot projection.
        QueryCommand::TerminalRead(_)
        | QueryCommand::ApiSchema { .. }
        | QueryCommand::ConfigCheck => Err(Failure::new(FailureKind::InternalFailure)),
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

#[cfg(test)]
mod tests {
    //! Resolver verdicts over real validated snapshots: bounded ambiguity with
    //! the true total, traversal-order candidates, per-resource ambiguity, and
    //! filter-target hit/miss/ambiguity for every filter arm.

    use super::*;
    use orchbee_workbench_protocol::parse_snapshot;

    fn hex(n: u8) -> String {
        format!("{n:032x}")
    }

    const WORKER: &str = "6f7263686265652d6c6f63616c2d7631";

    /// Deliberately unsorted document order: candidate ordering must follow
    /// canonical traversal order, never id sort.
    const DUP_TABS: [u8; 9] = [0x29, 0x25, 0x27, 0x22, 0x28, 0x24, 0x26, 0x23, 0x21];

    fn presentation(id: u8, terminal: Option<u8>) -> Value {
        json!({
            "presentationId": hex(id),
            "label": "p",
            "binding": terminal.map(|terminal| json!({
                "terminalId": hex(terminal),
                "workerId": WORKER,
            })),
        })
    }

    fn panel(id: u8, primary: u8, children: &[u8], terminal: Option<u8>) -> Value {
        json!({
            "kind": "panel",
            "panelId": hex(id),
            "primaryPresentation": presentation(primary, terminal),
            "childPresentations": children
                .iter()
                .map(|&child| presentation(child, None))
                .collect::<Vec<_>>(),
        })
    }

    fn tab(id: u8, label: &str, root: Value) -> Value {
        json!({
            "tabId": hex(id),
            "label": label,
            "pinned": false,
            "groupId": null,
            "root": root,
        })
    }

    /// Two same-label projects (`alpha`), each owning one same-label
    /// workspace (`ws`) on its own context; nine same-label `dup` tabs in
    /// scrambled id order under the standalone context; one `main` tab under
    /// each workspace context (a split root under the first).
    fn snapshot() -> Snapshot {
        let document = json!({
            "kind": "workbench.snapshot",
            "protocolVersion": 1,
            "revision": "9",
            "siteId": hex(0xaa),
            "navigation": {
                "projects": [
                    {
                        "projectId": hex(0x10),
                        "label": "alpha",
                        "directories": [{
                            "directoryId": hex(0x30),
                            "workerId": WORKER,
                            "path": "/a",
                        }],
                        "workspaces": [{
                            "workspaceId": hex(0x40),
                            "label": "ws",
                            "primaryDirectoryId": hex(0x30),
                            "terminalContextId": hex(0x50),
                        }],
                    },
                    {
                        "projectId": hex(0x11),
                        "label": "alpha",
                        "directories": [{
                            "directoryId": hex(0x31),
                            "workerId": WORKER,
                            "path": "/b",
                        }],
                        "workspaces": [{
                            "workspaceId": hex(0x41),
                            "label": "ws",
                            "primaryDirectoryId": hex(0x31),
                            "terminalContextId": hex(0x51),
                        }],
                    },
                ],
                "standaloneTerminalContextId": hex(0x01),
            },
            "terminalContexts": [
                {
                    "terminalContextId": hex(0x01),
                    "groups": [],
                    "tabs": DUP_TABS
                        .iter()
                        .map(|&n| tab(n, "dup", panel(0x80 + n, 0xa0 + n, &[], None)))
                        .collect::<Vec<_>>(),
                },
                {
                    "terminalContextId": hex(0x50),
                    "groups": [],
                    "tabs": [tab(0x52, "main", json!({
                        "kind": "split",
                        "splitId": hex(0x53),
                        "orientation": "horizontal",
                        "children": [
                            {"slotId": hex(0x54), "node": panel(0x55, 0x56, &[], None)},
                            {"slotId": hex(0x57), "node": panel(0x58, 0x59, &[0x5b], Some(0x5a))},
                        ],
                    }))],
                },
                {
                    "terminalContextId": hex(0x51),
                    "groups": [],
                    "tabs": [tab(0x62, "main", panel(0x63, 0x64, &[], None))],
                },
            ],
        });
        parse_snapshot(&serde_json::to_vec(&document).expect("fixture encodes"))
            .expect("fixture snapshot validates")
    }

    fn failure(command: &QueryCommand) -> Failure {
        project(command, &snapshot()).expect_err("resolver failure expected")
    }

    fn details(failure: &Failure) -> &Map<String, Value> {
        failure.details.as_ref().expect("typed failure details")
    }

    fn candidates(failure: &Failure) -> Vec<String> {
        details(failure)["candidates"]
            .as_array()
            .expect("candidate list")
            .iter()
            .map(|id| id.as_str().expect("typed candidate id").to_owned())
            .collect()
    }

    #[test]
    fn ambiguity_truncates_candidates_but_keeps_the_true_total() {
        let failure = failure(&QueryCommand::TabGet(Target::Label("dup".into())));
        assert_eq!(failure.kind, FailureKind::Ambiguous);
        let details = details(&failure);
        assert_eq!(details["resource"], json!("tab"));
        assert_eq!(details["target"], json!("dup"));
        assert_eq!(details["totalCount"], json!(DUP_TABS.len()));
        // At most the first eight candidate typed ids, in canonical traversal
        // order — context order then tab order — not sorted identity order.
        assert_eq!(
            candidates(&failure),
            DUP_TABS[..8].iter().map(|&n| hex(n)).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn label_lookup_is_ambiguous_per_resource() {
        for (command, resource, expected) in [
            (
                QueryCommand::ProjectGet(Target::Label("alpha".into())),
                "project",
                vec![hex(0x10), hex(0x11)],
            ),
            (
                QueryCommand::WorkspaceGet(Target::Label("ws".into())),
                "workspace",
                vec![hex(0x40), hex(0x41)],
            ),
        ] {
            let failure = failure(&command);
            assert_eq!(failure.kind, FailureKind::Ambiguous, "{resource}");
            let details = details(&failure);
            assert_eq!(details["resource"], json!(resource));
            assert_eq!(details["totalCount"], json!(expected.len()));
            assert_eq!(candidates(&failure), expected, "{resource}");
        }
    }

    #[test]
    fn unmatched_targets_are_not_found_without_candidates() {
        let missing = hex(0x7f);
        for (command, resource, target) in [
            (
                QueryCommand::TabGet(Target::Label("ghost".into())),
                "tab",
                "ghost".to_owned(),
            ),
            (
                QueryCommand::TabGet(Target::Id(missing.clone())),
                "tab",
                missing.clone(),
            ),
            (
                QueryCommand::WorkspaceGet(Target::Label("ghost".into())),
                "workspace",
                "ghost".to_owned(),
            ),
        ] {
            let failure = failure(&command);
            assert_eq!(failure.kind, FailureKind::NotFound, "{resource}");
            let details = details(&failure);
            assert_eq!(details["resource"], json!(resource));
            assert_eq!(details["target"], json!(target));
            assert!(details.get("candidates").is_none(), "{resource}");
        }
    }

    #[test]
    fn project_filter_hit_miss_and_ambiguity() {
        // Hit: only the resolved project's workspaces survive the filter.
        let projection = project(
            &QueryCommand::WorkspaceList {
                project: Some(Target::Id(hex(0x10))),
            },
            &snapshot(),
        )
        .expect("resolved project filter");
        let Projection::WorkspaceList(rows) = projection else {
            panic!("workspace list projection");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].workspace_id, hex(0x40));
        assert_eq!(rows[0].project_id, hex(0x10));
        assert_eq!(rows[0].terminal_context_id, hex(0x50));

        // Ambiguous filter target: the list itself is never built.
        let ambiguous = failure(&QueryCommand::WorkspaceList {
            project: Some(Target::Label("alpha".into())),
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("project"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x10), hex(0x11)]);

        // Miss: a typed not-found, never an empty list.
        let missed = failure(&QueryCommand::WorkspaceList {
            project: Some(Target::Label("ghost".into())),
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("project"));
        assert_eq!(details(&missed)["target"], json!("ghost"));
    }

    #[test]
    fn workspace_filter_hit_miss_and_ambiguity() {
        // Hit: tabs from exactly the resolved workspace's own context, each
        // carrying the workspace context reference.
        let projection = project(
            &QueryCommand::TabList {
                workspace: Some(Target::Id(hex(0x41))),
                standalone: false,
            },
            &snapshot(),
        )
        .expect("resolved workspace filter");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tab_id, hex(0x62));
        assert!(!rows[0].context.standalone);
        assert_eq!(rows[0].context.terminal_context_id, hex(0x51));
        assert_eq!(rows[0].context.workspace_id, Some(hex(0x41)));

        // Ambiguous: two workspaces share the label across projects.
        let ambiguous = failure(&QueryCommand::TabList {
            workspace: Some(Target::Label("ws".into())),
            standalone: false,
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("workspace"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x40), hex(0x41)]);

        // Miss.
        let missed = failure(&QueryCommand::TabList {
            workspace: Some(Target::Label("ghost".into())),
            standalone: false,
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("workspace"));
    }

    #[test]
    fn tab_filter_hit_miss_and_ambiguity() {
        // Hit: panels under exactly the resolved tab, in traversal order.
        let projection = project(
            &QueryCommand::PanelList {
                tab: Some(Target::Id(hex(0x52))),
            },
            &snapshot(),
        )
        .expect("resolved tab filter");
        let Projection::PanelList(rows) = projection else {
            panic!("panel list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.panel_id.clone())
                .collect::<Vec<_>>(),
            vec![hex(0x55), hex(0x58)],
        );
        assert!(rows.iter().all(|row| row.tab_id == hex(0x52)));
        assert!(rows.iter().all(|row| row.terminal_context_id == hex(0x50)));

        // Ambiguous: "main" names a tab in each workspace context.
        let ambiguous = failure(&QueryCommand::PanelList {
            tab: Some(Target::Label("main".into())),
        });
        assert_eq!(ambiguous.kind, FailureKind::Ambiguous);
        assert_eq!(details(&ambiguous)["resource"], json!("tab"));
        assert_eq!(candidates(&ambiguous), vec![hex(0x52), hex(0x62)]);

        // Miss.
        let missed = failure(&QueryCommand::PanelList {
            tab: Some(Target::Label("ghost".into())),
        });
        assert_eq!(missed.kind, FailureKind::NotFound);
        assert_eq!(details(&missed)["resource"], json!("tab"));
    }

    #[test]
    fn unfiltered_lists_follow_canonical_traversal_order() {
        // Workspaces flatten across projects in project/workspace order.
        let projection = project(&QueryCommand::WorkspaceList { project: None }, &snapshot())
            .expect("workspace list");
        let Projection::WorkspaceList(rows) = projection else {
            panic!("workspace list projection");
        };
        assert_eq!(
            rows.iter()
                .map(|row| row.workspace_id.clone())
                .collect::<Vec<_>>(),
            vec![hex(0x40), hex(0x41)],
        );

        // Tabs concatenate contexts in context order, then document order:
        // the scrambled standalone ids lead, then each workspace's tab.
        let projection = project(
            &QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            &snapshot(),
        )
        .expect("tab list");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), DUP_TABS.len() + 2);
        assert_eq!(
            rows.iter()
                .map(|row| row.tab_id.clone())
                .collect::<Vec<_>>(),
            DUP_TABS
                .iter()
                .chain([0x52, 0x62].iter())
                .map(|&n| hex(n))
                .collect::<Vec<_>>(),
        );
        // Context references pin the owning workspace only where one exists.
        assert!(
            rows[..9]
                .iter()
                .all(|row| row.context.standalone && row.context.workspace_id.is_none())
        );
        assert_eq!(rows[9].context.workspace_id, Some(hex(0x40)));
        assert_eq!(rows[10].context.workspace_id, Some(hex(0x41)));

        // `--standalone` selects exactly the standalone context's tabs.
        let projection = project(
            &QueryCommand::TabList {
                workspace: None,
                standalone: true,
            },
            &snapshot(),
        )
        .expect("standalone tab list");
        let Projection::TabList(rows) = projection else {
            panic!("tab list projection");
        };
        assert_eq!(rows.len(), DUP_TABS.len());
        assert!(rows.iter().all(|row| row.context.standalone));
    }
}
