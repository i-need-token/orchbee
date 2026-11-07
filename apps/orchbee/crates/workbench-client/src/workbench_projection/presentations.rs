//! Canonical members of one Panel; child terminals never become layout leaves.
use super::{ClientProjectionError, ClientTerminalBinding};
use orchbee_workbench_protocol::{
    PanelCustomLabel, PanelRightClickTarget, Presentation, PresentationLaunchFailure,
    PresentationLaunchProgram, TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientPresentation {
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) presentation_label: WorkbenchLabel,
    pub(super) custom_label: Option<PanelCustomLabel>,
    pub(super) binding: Option<ClientTerminalBinding>,
    pub(super) launch_failure: Option<PresentationLaunchFailure>,
    pub(super) launch_program: Option<PresentationLaunchProgram>,
}

impl ClientPresentation {
    pub(super) fn project(value: &Presentation) -> Result<Self, ClientProjectionError> {
        Ok(Self {
            presentation_id: value.presentation_id.clone(),
            presentation_label: WorkbenchLabel::parse(&value.label)
                .map_err(|_| ClientProjectionError)?,
            custom_label: value.custom_label.clone(),
            binding: value.binding.as_ref().map(|binding| ClientTerminalBinding {
                worker_id: binding.worker_id.clone(),
                terminal_id: binding.terminal_id.clone(),
            }),
            launch_failure: value.launch_failure.clone(),
            launch_program: value.launch_program.clone(),
        })
    }

    pub(crate) fn presentation_id(&self) -> &TerminalPresentationId {
        &self.presentation_id
    }
    pub(crate) fn presentation_label(&self) -> &WorkbenchLabel {
        &self.presentation_label
    }
    pub(crate) fn custom_label(&self) -> Option<&str> {
        self.custom_label.as_ref().map(|value| value.as_str())
    }
    pub(crate) fn display_label(&self) -> &str {
        self.custom_label()
            .unwrap_or(self.presentation_label.as_str())
    }
    pub(crate) fn binding(&self) -> Option<&ClientTerminalBinding> {
        self.binding.as_ref()
    }
    /// The Server's committed failure for the current binding, or the held restore of an unbound
    /// member, visible to every Client.
    pub(crate) fn launch_failure(&self) -> Option<&PresentationLaunchFailure> {
        self.launch_failure.as_ref()
    }
    /// The Server's committed failure for the current binding alone.
    pub(crate) fn creation_failure(&self) -> Option<&PresentationLaunchFailure> {
        self.launch_failure
            .as_ref()
            .filter(|failure| failure.unavailable_directory().is_none())
    }
    /// The saved directory a held restore waits for, and whether a retained Agent conversation
    /// resumes there.
    pub(crate) fn held_directory(
        &self,
    ) -> Option<(&orchbee_workbench_protocol::WorkbenchDirectoryPath, bool)> {
        let failure = self.launch_failure.as_ref()?;
        failure
            .unavailable_directory()
            .map(|directory| (directory, failure.agent_resume))
    }
    /// The file name of the program the Server committed with the current binding.
    pub(crate) fn launch_program(&self) -> Option<&str> {
        self.launch_program
            .as_ref()
            .map(PresentationLaunchProgram::as_str)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientPanel {
    pub(super) right_click: PanelRightClickTarget,
    pub(super) panel_id: WorkbenchPanelId,
    pub(super) primary: ClientPresentation,
    pub(super) children: Box<[ClientPresentation]>,
}

impl ClientPanel {
    pub(crate) fn primary(&self) -> &ClientPresentation {
        &self.primary
    }
    pub(crate) const fn right_click(&self) -> PanelRightClickTarget {
        self.right_click
    }
    pub(crate) fn panel_id(&self) -> &WorkbenchPanelId {
        &self.panel_id
    }
    pub(crate) fn presentation_id(&self) -> &TerminalPresentationId {
        self.primary.presentation_id()
    }
    pub(crate) fn presentation_label(&self) -> &WorkbenchLabel {
        self.primary.presentation_label()
    }
    pub(crate) fn binding(&self) -> Option<&ClientTerminalBinding> {
        self.primary.binding()
    }
    pub(crate) fn presentation_count(&self) -> usize {
        1 + self.children.len()
    }
    pub(crate) fn presentation(&self, index: usize) -> Option<&ClientPresentation> {
        if index == 0 {
            Some(&self.primary)
        } else {
            self.children.get(index - 1)
        }
    }
    pub(crate) fn presentations(&self) -> impl Iterator<Item = &ClientPresentation> {
        std::iter::once(&self.primary).chain(self.children.iter())
    }
    pub(crate) fn presentation_index(&self, id: &TerminalPresentationId) -> Option<usize> {
        self.presentations()
            .position(|member| member.presentation_id() == id)
    }
    pub(crate) fn bound_presentation(
        &self,
        binding: &ClientTerminalBinding,
    ) -> Option<&ClientPresentation> {
        self.presentations()
            .find(|member| member.binding() == Some(binding))
    }
}
