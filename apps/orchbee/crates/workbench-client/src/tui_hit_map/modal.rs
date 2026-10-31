//! Painted specialized modal controls and their projected pointer state.

use super::{
    FrameHitMap, HitLayer, HitResolution, HitTarget, InteractionAvailability, ModalControl,
    ProjectedPointer,
};
use ratatui::layout::Rect;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ControlTarget {
    Copy(
        crate::terminal_copy_mode::CopyModeToken,
        crate::terminal_copy_mode::CopyControl,
    ),
    Resize(super::ResizeControl),
    TabClose(crate::workbench_state::CloseConfirmationToken, bool),
    PanelClose(crate::workbench_state::PanelCloseConfirmationToken, bool),
    Rename(crate::workbench_state::rename_draft::RenameDraftToken, bool),
    Setup(
        crate::workbench_state::workspace_setup::SetupToken,
        ModalControl,
    ),
    Worktree(
        crate::workbench_state::worktree_launch::WorktreeToken,
        ModalControl,
    ),
    Close(
        crate::workbench_state::workspace_close::WorkspaceCloseToken,
        bool,
    ),
}

impl ControlTarget {
    pub(crate) fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            HitTarget::CopyControl { token, control } => Self::Copy(*token, *control),
            HitTarget::Resize(control) => Self::Resize(control.clone()),
            HitTarget::TabClose { token, confirm } => Self::TabClose(*token, *confirm),
            HitTarget::PanelClose { token, confirm } => Self::PanelClose(*token, *confirm),
            HitTarget::Rename { token, submit } => Self::Rename(*token, *submit),
            HitTarget::Setup { token, control } => Self::Setup(*token, *control),
            HitTarget::Worktree { token, control } => Self::Worktree(*token, *control),
            HitTarget::WorkspaceClose { token, confirm } => Self::Close(*token, *confirm),
            _ => return None,
        })
    }

    pub(crate) fn projectable(&self) -> bool {
        if matches!(self, Self::Setup(_, ModalControl::Surface)) {
            return false;
        }
        if matches!(self, Self::Worktree(_, ModalControl::Surface)) {
            return false;
        }
        true
    }
}

pub(crate) fn accepts_copy(target: &ControlTarget) -> bool {
    matches!(target, ControlTarget::Copy(..))
}

pub(crate) fn accepts_resize(target: &ControlTarget) -> bool {
    matches!(target, ControlTarget::Resize(_))
}

pub(crate) fn accepts_confirmation(target: &ControlTarget) -> bool {
    matches!(
        target,
        ControlTarget::Close(..) | ControlTarget::TabClose(..) | ControlTarget::PanelClose(..)
    )
}

pub(crate) fn accepts_modal(target: &ControlTarget) -> bool {
    match target {
        ControlTarget::Rename(..) => true,
        ControlTarget::Setup(_, control) => *control != ModalControl::Surface,
        ControlTarget::Worktree(_, control) => *control != ModalControl::Surface,
        _ => false,
    }
}

/// Exact paint identity for one specialized modal control. The target holds
/// its opaque owner token; geometry and layer prevent a repaint from silently
/// reusing press or hover presentation for a different painted control.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fingerprint {
    pub(crate) target: ControlTarget,
    pub(crate) layer: HitLayer,
    pub(crate) area: Rect,
    pub(crate) availability: InteractionAvailability,
}

impl Fingerprint {
    pub(crate) fn from_hit(
        layer: HitLayer,
        area: Rect,
        target: &HitTarget,
        availability: InteractionAvailability,
    ) -> Option<Self> {
        let target = ControlTarget::from_hit(target)?;
        if !target.projectable() {
            return None;
        }
        Some(Self {
            target,
            layer,
            area,
            availability,
        })
    }

    pub(crate) const fn enabled(&self) -> bool {
        matches!(self.availability, InteractionAvailability::Enabled)
    }

    pub(crate) fn same_control(&self, other: &Self) -> bool {
        self.target == other.target && self.layer == other.layer && self.area == other.area
    }
}

/// Immutable specialized-modal pointer facts captured before one candidate
/// frame. Effects remain with `control_event`; this value is paint-only.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Projection {
    pub(crate) hovered: Option<Fingerprint>,
    pub(crate) pressed: Option<Fingerprint>,
}

impl Projection {
    pub(crate) const fn empty() -> Self {
        Self {
            hovered: None,
            pressed: None,
        }
    }

    pub(crate) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
        if !fingerprint.enabled() {
            return ProjectedPointer::Rest;
        }
        if self.pressed.as_ref() == Some(fingerprint) {
            ProjectedPointer::Pressed
        } else if self.hovered.as_ref() == Some(fingerprint) {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    }
}

impl FrameHitMap {
    pub(crate) fn modal_backdrop_fingerprint_at(
        &self,
        column: u16,
        row: u16,
    ) -> Option<Fingerprint> {
        if !matches!(self.hit(column, row), HitResolution::OverlayBackdrop) {
            return None;
        }
        let target = self.backdrop_dismissal.as_ref()?;
        let region = self
            .regions
            .iter()
            .find(|region| region.layer == HitLayer::Overlay && &region.target == target)?;
        self.modal_fingerprint_for_region(region)
            .filter(Fingerprint::enabled)
    }
}
