//! Bounded Unicode fields and stable-ID Project/checkout choices for one Worktree confirmation.
use super::{Discovery, Launcher, Origin, Phase, Plan, WorktreeMode, WorktreeNotice};
use crate::tui_presentation::PresentationPhase;
use crate::workbench_client::state::{
    field_window::{NativeFieldWindow, field_window},
    line_editor::{LengthLimit, LineEditor},
};
use orchbee_workbench_protocol::{
    WorkbenchRequestId, WorkbenchTerminalGeometry, WorkbenchWorktreeOutcome, WorktreeEntryText,
    WorktreeIntent, WorktreeReference, WorktreeSelector, WorktreeSource,
};
/// One non-reused draw/input authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorktreeToken(pub(in crate::workbench_client) u64);
#[cfg(test)]
impl WorktreeToken {
    pub(crate) fn for_test(value: u64) -> Self {
        Self(value)
    }
}
/// The current keyboard owner inside the Worktree surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorktreeStage {
    Preparing,
    Branch,
    Base,
    Checkout,
    Remove,
    ResumeRemove,
    Waiting,
    Result,
}
/// Single-line edits and choice movement; never interpreted as terminal input.
pub(crate) enum WorktreeEdit<'a> {
    ReadStatus,
    NextField,
    Append(&'a str),
    Paste(&'a str),
    Backspace,
    Clear,
    Left,
    Right,
    Home,
    End,
    Delete,
    WordBackspace,
    Fault,
    Completeness(bool),
    Previous,
    Next,
    ScrollChoices { up: bool, lines: usize },
}
pub(super) struct Draft {
    pub removal: Option<super::Removal>,
    pub sidebar: Option<Box<super::addressed::SidebarDiscovery>>,
    pub token: WorktreeToken,
    pub origin: Origin,
    source: WorktreeSource,
    discovery: Discovery,
    mode: WorktreeMode,
    stage: WorktreeStage,
    selected_checkout: usize,
    branch: LineEditor,
    base: LineEditor,
    pub complete: bool,
    fault: bool,
}
impl Draft {
    pub(super) fn choose_checkout(&mut self, index: usize) -> bool {
        if self.stage != WorktreeStage::Checkout || !self.complete || self.fault {
            return false;
        }
        let Some(entry) = self.discovery.entries.get(index) else {
            return false;
        };
        if entry.checkout.bare
            || !matches!(&entry.checkout.path, WorktreeEntryText::Utf8(path)
            if orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path).is_ok())
        {
            return false;
        }
        self.selected_checkout = index;
        true
    }

    pub fn new(
        origin: Origin,
        source: WorktreeSource,
        discovery: Discovery,
        mode: WorktreeMode,
    ) -> Self {
        let stage = match mode {
            WorktreeMode::Create => WorktreeStage::Branch,
            WorktreeMode::Open => WorktreeStage::Checkout,
            WorktreeMode::Remove => WorktreeStage::Remove,
        };
        Self {
            removal: None,
            sidebar: None,
            token: WorktreeToken(0),
            origin,
            source,
            discovery,
            mode,
            stage,
            selected_checkout: 0,
            branch: LineEditor::new(String::new(), LengthLimit::Bytes(1024)),
            base: LineEditor::new("HEAD".into(), LengthLimit::Bytes(1024)),
            complete: true,
            fault: false,
        }
    }
    pub fn is_create(&self) -> bool {
        self.mode == WorktreeMode::Create
    }
    pub(super) fn is_checkout(&self) -> bool {
        self.stage == WorktreeStage::Checkout
    }
    fn field(&self) -> &LineEditor {
        if self.stage == WorktreeStage::Base {
            &self.base
        } else {
            &self.branch
        }
    }
    fn field_mut(&mut self) -> &mut LineEditor {
        if self.stage == WorktreeStage::Base {
            &mut self.base
        } else {
            &mut self.branch
        }
    }
    pub fn edit(&mut self, edit: WorktreeEdit<'_>) -> Result<bool, WorktreeNotice> {
        if self.stage == WorktreeStage::Remove {
            return Ok(false);
        }
        match edit {
            WorktreeEdit::Completeness(complete) => {
                self.complete = complete;
                return Ok(false);
            }
            WorktreeEdit::Fault => {
                self.fault = true;
                return Err(WorktreeNotice::InvalidInput);
            }
            _ => {}
        }
        if self.stage == WorktreeStage::Checkout {
            let (selected, count) = (&mut self.selected_checkout, self.discovery.entries.len());
            let old = *selected;
            match edit {
                WorktreeEdit::ScrollChoices { up: true, lines } => {
                    *selected = selected.saturating_sub(lines)
                }
                WorktreeEdit::ScrollChoices { up: false, lines } => {
                    *selected = selected.saturating_add(lines).min(count.saturating_sub(1))
                }
                WorktreeEdit::Previous | WorktreeEdit::Left => {
                    *selected = selected.saturating_sub(1)
                }
                WorktreeEdit::Next | WorktreeEdit::Right | WorktreeEdit::NextField => {
                    *selected = selected.saturating_add(1).min(count.saturating_sub(1))
                }
                WorktreeEdit::Home => *selected = 0,
                WorktreeEdit::End => *selected = count.saturating_sub(1),
                _ => {}
            }
            return Ok(old != *selected);
        }
        let changed = match edit {
            WorktreeEdit::NextField => {
                self.stage = if self.stage == WorktreeStage::Branch {
                    WorktreeStage::Base
                } else {
                    WorktreeStage::Branch
                };
                true
            }
            WorktreeEdit::Append(text) | WorktreeEdit::Paste(text) => {
                if text.chars().any(char::is_control) {
                    self.fault = true;
                    return Err(WorktreeNotice::InvalidInput);
                }
                self.field_mut()
                    .insert(text)
                    .map_err(|_| WorktreeNotice::InputLimit)?;
                true
            }
            WorktreeEdit::Backspace => self.field_mut().backspace(),
            WorktreeEdit::Clear => {
                self.field_mut().clear();
                true
            }
            WorktreeEdit::Left => self.field_mut().left(),
            WorktreeEdit::Right => self.field_mut().right(),
            WorktreeEdit::Home => self.field_mut().home(),
            WorktreeEdit::End => self.field_mut().end(),
            WorktreeEdit::Delete => self.field_mut().delete(),
            WorktreeEdit::WordBackspace => self.field_mut().word_back(),
            _ => false,
        };
        if changed {
            self.fault = false;
        }
        Ok(changed)
    }
    pub fn plan(&self, operation_id: WorkbenchRequestId) -> Option<Plan> {
        if !self.complete || self.fault {
            return None;
        }
        let g = self.origin.geometry;
        let geometry = WorkbenchTerminalGeometry::new(
            g.columns(),
            g.rows(),
            g.cell_pixel_width(),
            g.cell_pixel_height(),
        )
        .ok()?;
        let generated_branch = format!("worktree/{}", operation_id.as_str());
        let intent = match self.mode {
            WorktreeMode::Create => WorktreeIntent::Create {
                source: self.source.clone(),
                branch: if self.branch.text().is_empty() {
                    None
                } else {
                    Some(WorktreeReference::parse(self.branch.text()).ok()?)
                },
                base: if self.base.text().is_empty() {
                    None
                } else {
                    Some(WorktreeReference::parse(self.base.text()).ok()?)
                },
                path: match &self.discovery.configured_root {
                    Some(root) => Some(
                        crate::worktree_destination::destination(
                            root,
                            &self.discovery.source.repository_root,
                            if self.branch.text().is_empty() {
                                &generated_branch
                            } else {
                                self.branch.text()
                            },
                        )
                        .ok()?,
                    ),
                    None => None,
                },
                label: None,
                geometry,
                trust_repository: false,
            },
            WorktreeMode::Open => {
                let selected = self.discovery.entries.get(self.selected_checkout)?;
                if selected.checkout.bare {
                    return None;
                }
                let WorktreeEntryText::Utf8(path) = &selected.checkout.path else {
                    return None;
                };
                WorktreeIntent::Open {
                    source: self.source.clone(),
                    selector: WorktreeSelector::Path {
                        path: orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path)
                            .ok()?,
                    },
                    label: None,
                    geometry,
                    trust_repository: false,
                }
            }
            WorktreeMode::Remove => WorktreeIntent::Remove {
                workspace_id: self.removal.as_ref()?.workspace_id.clone(),
                force: false,
                trust_repository: false,
            },
        };
        Some(Plan {
            removal: self.removal.clone(),
            sidebar: self.sidebar.clone(),
            operation_id,
            origin: self.origin.clone(),
            source: self.source.clone(),
            discovery: self.discovery.clone(),
            project: self
                .sidebar
                .as_ref()
                .filter(|_| self.mode != WorktreeMode::Remove)
                .filter(|_| matches!(self.source, WorktreeSource::Directory { .. }))
                .and_then(|source| source.addressed.cache.project_id().cloned()),
            intent,
        })
    }
}
/// Read-only projection consumed by renderer and input adapters.
#[derive(Clone, Copy)]
pub(crate) struct WorktreeView<'a> {
    pub(super) launcher: &'a Launcher,
}
impl<'a> WorktreeView<'a> {
    /// Rows with no usable checkout identity remain visible but cannot open.
    pub(crate) fn choice_openable(&self, index: usize) -> bool {
        let Phase::Draft(draft) = &self.launcher.phase else {
            return false;
        };
        draft.discovery.entries.get(index).is_some_and(|entry| {
            !entry.checkout.bare
                && matches!(&entry.checkout.path, WorktreeEntryText::Utf8(path)
                if orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(path).is_ok())
        })
    }

    pub(crate) fn token(&self) -> WorktreeToken {
        WorktreeToken(self.launcher.last_token)
    }
    pub(crate) const fn presentation(&self) -> PresentationPhase {
        self.launcher.presentation
    }
    pub(crate) fn stage(&self) -> WorktreeStage {
        match &self.launcher.phase {
            Phase::Preparing(_) | Phase::Querying { .. } => WorktreeStage::Preparing,
            Phase::Draft(d) => d.stage,
            Phase::ConfirmResume(_) => WorktreeStage::ResumeRemove,
            Phase::Starting(_) | Phase::Observing(_) | Phase::Resuming(_) => WorktreeStage::Waiting,
            _ => WorktreeStage::Result,
        }
    }
    pub(crate) fn mode(&self) -> WorktreeMode {
        match &self.launcher.phase {
            Phase::Preparing(p) | Phase::Querying { preparation: p, .. } => p.mode,
            Phase::Draft(d) => d.mode,
            Phase::Starting(plan) => intent_mode(&plan.intent),
            _ => self
                .launcher
                .operation()
                .map_or(WorktreeMode::Create, |op| intent_mode(&op.plan.intent)),
        }
    }
    pub(crate) fn notice(&self) -> WorktreeNotice {
        self.launcher.notice
    }
    pub(crate) fn text(&self) -> &'a str {
        match &self.launcher.phase {
            Phase::Draft(d) => d.field().text(),
            _ => "",
        }
    }
    pub(crate) fn remaining_bytes(&self) -> usize {
        1024usize.saturating_sub(self.text().len())
    }
    pub(crate) fn field_window(&self, cells: usize) -> Option<NativeFieldWindow<'_>> {
        match &self.launcher.phase {
            Phase::Draft(d) => field_window(d.field(), cells),
            _ => None,
        }
    }
    pub(crate) fn branch(&self) -> &'a str {
        match &self.launcher.phase {
            Phase::Draft(d) => d.branch.text(),
            _ => "",
        }
    }
    pub(crate) fn base(&self) -> &'a str {
        match &self.launcher.phase {
            Phase::Draft(d) => d.base.text(),
            _ => "",
        }
    }
    pub(crate) fn source_path(&self) -> Option<&'a str> {
        match &self.launcher.phase {
            Phase::Draft(d) => Some(d.discovery.source.requested_path.as_str()),
            _ => None,
        }
    }
    pub(crate) fn choice_count(&self) -> usize {
        match &self.launcher.phase {
            Phase::Draft(d) if d.stage == WorktreeStage::Checkout => d.discovery.entries.len(),
            _ => 0,
        }
    }
    pub(crate) fn selected(&self) -> usize {
        match &self.launcher.phase {
            Phase::Draft(d) => d.selected_checkout,
            _ => 0,
        }
    }
    pub(crate) fn choice(&self, index: usize) -> Option<(String, &'a str)> {
        let Phase::Draft(d) = &self.launcher.phase else {
            return None;
        };
        let e = d.discovery.entries.get(index)?;
        let WorktreeEntryText::Utf8(path) = &e.checkout.path else {
            return None;
        };
        let label = match &e.checkout.branch {
            Some(WorktreeEntryText::Utf8(branch)) => branch
                .strip_prefix("refs/heads/")
                .unwrap_or(branch)
                .to_owned(),
            // This operation needs UTF-8 text operands; opaque path bytes cannot supply them.
            _ => String::new(),
        };
        Some((label, path))
    }
    pub(crate) fn operation_id(&self) -> Option<&'a str> {
        self.launcher
            .operation()
            .map(|op| op.plan.operation_id.as_str())
            .or_else(|| match &self.launcher.phase {
                Phase::Starting(p) => Some(p.operation_id.as_str()),
                _ => None,
            })
    }
    pub(crate) fn workspace(&self) -> Option<&'a str> {
        let WorkbenchWorktreeOutcome::Available { status, .. } =
            self.launcher.operation()?.outcome.as_ref()?
        else {
            return None;
        };
        match status.as_ref() {
            orchbee_workbench_protocol::WorktreeOperationStatus::Opened {
                workspace_id, ..
            } => Some(workspace_id.as_str()),
            orchbee_workbench_protocol::WorktreeOperationStatus::Pending { progress, .. }
            | orchbee_workbench_protocol::WorktreeOperationStatus::Failed { progress, .. } => {
                match &progress.workspace {
                    orchbee_workbench_protocol::WorktreeWorkspaceDisposition::Open {
                        workspace_id,
                    }
                    | orchbee_workbench_protocol::WorktreeWorkspaceDisposition::Closed {
                        workspace_id,
                        ..
                    }
                    | orchbee_workbench_protocol::WorktreeWorkspaceDisposition::Closing {
                        workspace_id,
                        ..
                    } => Some(workspace_id.as_str()),
                    orchbee_workbench_protocol::WorktreeWorkspaceDisposition::Unassociated {
                        ..
                    } => None,
                }
            }
            orchbee_workbench_protocol::WorktreeOperationStatus::Removed { .. } => None,
        }
    }
}
fn intent_mode(intent: &WorktreeIntent) -> WorktreeMode {
    match intent {
        WorktreeIntent::Open { .. } => WorktreeMode::Open,
        WorktreeIntent::Create { .. } => WorktreeMode::Create,
        WorktreeIntent::Remove { .. } => WorktreeMode::Remove,
    }
}
