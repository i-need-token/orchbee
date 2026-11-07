//! Painted Workbench presentation selector targets.

/// One presentation control exactly as it was drawn for a live pane source.
///
/// The destination alone is not authority. Attached controls retain the
/// physical pane incarnation that owned the header; unbound controls retain
/// the current logical pane and selected member. Release revalidates both the
/// source and the destination against current membership before switching.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DrawnPresentationTarget {
    Attached {
        pane: crate::workbench_state::PaneRef,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
    },
    Unbound {
        selected: PointerUnboundPane,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
    },
}

impl DrawnPresentationTarget {
    pub(crate) const fn presentation(&self) -> &orchbee_workbench_protocol::TerminalPresentationId {
        match self {
            Self::Attached { presentation, .. } | Self::Unbound { presentation, .. } => {
                presentation
            }
        }
    }

    pub(crate) fn panel(&self) -> &orchbee_workbench_protocol::WorkbenchPanelId {
        match self {
            Self::Attached { pane, .. } => pane.pane().panel(),
            Self::Unbound { selected, .. } => selected.key.panel(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerUnboundPane {
    pub(crate) key: crate::workbench_state::PaneKey,
    pub(crate) presentation: orchbee_workbench_protocol::TerminalPresentationId,
}

/// One painted action of a Panel a restore held for an unavailable directory, by stable Panel and
/// Presentation identity. The owner refuses it once that Presentation is no longer held there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldRestoreTarget {
    pub(crate) panel: orchbee_workbench_protocol::WorkbenchPanelId,
    pub(crate) presentation: orchbee_workbench_protocol::TerminalPresentationId,
    pub(crate) action: crate::localization::HeldRestoreAction,
}
