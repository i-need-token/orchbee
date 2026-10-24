//! Canonical members of one Panel; child terminals never become layout leaves.
use super::{NativeProjectionError, NativeTerminalBinding};
use orchbee_workbench_protocol::{
    PanelCustomLabel, PanelRightClickTarget, Presentation, PresentationLaunchFailure,
    TerminalPresentationId, WorkbenchLabel, WorkbenchPanelId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativePresentation {
    pub(super) presentation_id: TerminalPresentationId,
    pub(super) presentation_label: WorkbenchLabel,
    pub(super) custom_label: Option<PanelCustomLabel>,
    pub(super) binding: Option<NativeTerminalBinding>,
    pub(super) launch_failure: Option<PresentationLaunchFailure>,
}

impl NativePresentation {
    pub(super) fn project(value: &Presentation) -> Result<Self, NativeProjectionError> {
        Ok(Self {
            presentation_id: value.presentation_id.clone(),
            presentation_label: WorkbenchLabel::parse(&value.label)
                .map_err(|_| NativeProjectionError)?,
            custom_label: value.custom_label.clone(),
            binding: value.binding.as_ref().map(|binding| NativeTerminalBinding {
                worker_id: binding.worker_id.clone(),
                terminal_id: binding.terminal_id.clone(),
            }),
            launch_failure: value.launch_failure.clone(),
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
    pub(in crate::terminal_session) fn binding(&self) -> Option<&NativeTerminalBinding> {
        self.binding.as_ref()
    }
    /// The Server's committed failure for the current binding, visible to every Client.
    pub(in crate::terminal_session) fn launch_failure(&self) -> Option<&PresentationLaunchFailure> {
        self.launch_failure.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativePanel {
    pub(super) right_click: PanelRightClickTarget,
    pub(super) panel_id: WorkbenchPanelId,
    pub(super) primary: NativePresentation,
    pub(super) children: Box<[NativePresentation]>,
}

impl NativePanel {
    pub(crate) fn primary(&self) -> &NativePresentation {
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
    pub(in crate::terminal_session) fn binding(&self) -> Option<&NativeTerminalBinding> {
        self.primary.binding()
    }
    pub(crate) fn presentation_count(&self) -> usize {
        1 + self.children.len()
    }
    pub(crate) fn presentation(&self, index: usize) -> Option<&NativePresentation> {
        if index == 0 {
            Some(&self.primary)
        } else {
            self.children.get(index - 1)
        }
    }
    pub(crate) fn presentations(&self) -> impl Iterator<Item = &NativePresentation> {
        std::iter::once(&self.primary).chain(self.children.iter())
    }
    pub(crate) fn presentation_index(&self, id: &TerminalPresentationId) -> Option<usize> {
        self.presentations()
            .position(|member| member.presentation_id() == id)
    }
    pub(in crate::terminal_session) fn bound_presentation(
        &self,
        binding: &NativeTerminalBinding,
    ) -> Option<&NativePresentation> {
        self.presentations()
            .find(|member| member.binding() == Some(binding))
    }
}
