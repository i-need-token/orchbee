//! One stateless composition contract for exclusive TUI input surfaces.
//!
//! Feature owners report only whether their surface currently intends to
//! capture input. This module selects the same single top surface for paint
//! settlement and input dispatch; revision tokens and presentation readiness
//! remain with the selected owner and are sampled separately for each item.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkbenchCaptureOwner {
    Settings,
    ActionMenu,
    Popup,
    Worktree,
    Setup,
    Rename,
    PanelClose,
    TabClose,
    WorkspaceClose,
    AgentBoard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaptureOwner {
    Machine,
    Native(WorkbenchCaptureOwner),
    Copy,
    Resize,
}

/// Pure capture-intent facts projected by the native surface owners.
///
/// These facts deliberately contain no draw token or interactive verdict.
/// An admitted surface can remain the top recovery capture while its current
/// presentation proof is pending, covered, or suspended.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CaptureClaims {
    settings: bool,
    action_menu: bool,
    popup: bool,
    worktree: bool,
    setup: bool,
    rename: bool,
    panel_close: bool,
    tab_close: bool,
    workspace_close: bool,
    agent_board: bool,
    copy: bool,
}

impl CaptureClaims {
    pub(crate) const fn with_settings(mut self, captured: bool) -> Self {
        self.settings = captured;
        self
    }

    pub(crate) const fn with_action_menu(mut self, captured: bool) -> Self {
        self.action_menu = captured;
        self
    }

    pub(crate) const fn with_popup(mut self, captured: bool) -> Self {
        self.popup = captured;
        self
    }

    pub(crate) const fn with_worktree(mut self, captured: bool) -> Self {
        self.worktree = captured;
        self
    }

    pub(crate) const fn with_setup(mut self, captured: bool) -> Self {
        self.setup = captured;
        self
    }

    pub(crate) const fn with_rename(mut self, captured: bool) -> Self {
        self.rename = captured;
        self
    }

    pub(crate) const fn with_panel_close(mut self, captured: bool) -> Self {
        self.panel_close = captured;
        self
    }

    pub(crate) const fn with_tab_close(mut self, captured: bool) -> Self {
        self.tab_close = captured;
        self
    }

    pub(crate) const fn with_workspace_close(mut self, captured: bool) -> Self {
        self.workspace_close = captured;
        self
    }

    pub(crate) const fn with_agent_board(mut self, captured: bool) -> Self {
        self.agent_board = captured;
        self
    }

    pub(crate) const fn with_copy(mut self, captured: bool) -> Self {
        self.copy = captured;
        self
    }

    pub(crate) const fn contains(self, owner: WorkbenchCaptureOwner) -> bool {
        match owner {
            WorkbenchCaptureOwner::Settings => self.settings,
            WorkbenchCaptureOwner::ActionMenu => self.action_menu,
            WorkbenchCaptureOwner::Popup => self.popup,
            WorkbenchCaptureOwner::Worktree => self.worktree,
            WorkbenchCaptureOwner::Setup => self.setup,
            WorkbenchCaptureOwner::Rename => self.rename,
            WorkbenchCaptureOwner::PanelClose => self.panel_close,
            WorkbenchCaptureOwner::TabClose => self.tab_close,
            WorkbenchCaptureOwner::WorkspaceClose => self.workspace_close,
            WorkbenchCaptureOwner::AgentBoard => self.agent_board,
        }
    }

    /// Whether a native surface above the Copy/Resize modes owns composition.
    pub(crate) const fn exclusive_any(self) -> bool {
        project_native_capture(self).is_some()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientCaptureClaims {
    machine: bool,
    resize: bool,
}

impl ClientCaptureClaims {
    pub const fn with_machine(mut self, captured: bool) -> Self {
        self.machine = captured;
        self
    }

    pub const fn with_resize(mut self, captured: bool) -> Self {
        self.resize = captured;
        self
    }
}

/// Selects the top native capture in the exact product composition order.
///
/// This function is the sole native z-order authority. Callers must not rank
/// or combine owner claims independently.
pub(crate) const fn project_native_capture(claims: CaptureClaims) -> Option<WorkbenchCaptureOwner> {
    if claims.settings {
        Some(WorkbenchCaptureOwner::Settings)
    } else {
        if claims.action_menu {
            return Some(WorkbenchCaptureOwner::ActionMenu);
        }
        if claims.popup {
            return Some(WorkbenchCaptureOwner::Popup);
        }
        if claims.worktree {
            return Some(WorkbenchCaptureOwner::Worktree);
        }
        if claims.setup {
            Some(WorkbenchCaptureOwner::Setup)
        } else if claims.rename {
            Some(WorkbenchCaptureOwner::Rename)
        } else if claims.panel_close {
            Some(WorkbenchCaptureOwner::PanelClose)
        } else if claims.tab_close {
            Some(WorkbenchCaptureOwner::TabClose)
        } else if claims.workspace_close {
            Some(WorkbenchCaptureOwner::WorkspaceClose)
        } else if claims.agent_board {
            Some(WorkbenchCaptureOwner::AgentBoard)
        } else {
            None
        }
    }
}

/// Composes Client chrome over the native surface without treating a covered
/// retained native owner as a conflict.
pub const fn project_capture(
    client: ClientCaptureClaims,
    native: CaptureClaims,
) -> Option<CaptureOwner> {
    if client.machine {
        return Some(CaptureOwner::Machine);
    }
    if let Some(owner) = project_native_capture(native) {
        return Some(CaptureOwner::Native(owner));
    }
    if native.copy {
        return Some(CaptureOwner::Copy);
    }
    if client.resize {
        return Some(CaptureOwner::Resize);
    }
    None
}

/// Whether Client chrome may paint and handle pointer input over the selected
/// owner. Help can cover Copy and Resize while retaining those modes for
/// resumption; native task captures remain above Client chrome.
pub(crate) const fn client_chrome_overlay_allowed(owner: Option<CaptureOwner>) -> bool {
    match owner {
        None => true,
        Some(CaptureOwner::Copy | CaptureOwner::Resize) => true,
        Some(CaptureOwner::Native(_)) => false,
        Some(CaptureOwner::Machine) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_projection_uses_the_single_surface_composition_order() {
        let claims = CaptureClaims::default()
            .with_agent_board(true)
            .with_workspace_close(true)
            .with_tab_close(true)
            .with_panel_close(true)
            .with_rename(true)
            .with_setup(true)
            .with_settings(true);
        assert_eq!(
            project_native_capture(claims),
            Some(WorkbenchCaptureOwner::Settings)
        );

        let claims = CaptureClaims::default()
            .with_agent_board(true)
            .with_workspace_close(true);
        assert_eq!(
            project_native_capture(claims),
            Some(WorkbenchCaptureOwner::WorkspaceClose)
        );
    }

    #[test]
    fn client_chrome_overlay_never_covers_an_exclusive_owner() {
        assert!(client_chrome_overlay_allowed(None));
        assert!(!client_chrome_overlay_allowed(Some(CaptureOwner::Native(
            WorkbenchCaptureOwner::Settings
        ))));
        {
            assert!(client_chrome_overlay_allowed(Some(CaptureOwner::Copy)));
            assert!(client_chrome_overlay_allowed(Some(CaptureOwner::Resize)));
            assert!(!client_chrome_overlay_allowed(Some(CaptureOwner::Machine)));
        }
    }

    #[test]
    fn machine_legitimately_covers_a_retained_native_capture() {
        let client = ClientCaptureClaims::default().with_machine(true);
        let native = CaptureClaims::default()
            .with_settings(true)
            .with_worktree(true);
        assert_eq!(project_capture(client, native), Some(CaptureOwner::Machine));
        assert_eq!(
            project_capture(ClientCaptureClaims::default(), native),
            Some(CaptureOwner::Native(WorkbenchCaptureOwner::Settings))
        );
    }

    #[test]
    fn windows_native_order_matches_the_compositor() {
        let claims = CaptureClaims::default()
            .with_agent_board(true)
            .with_workspace_close(true)
            .with_tab_close(true)
            .with_panel_close(true)
            .with_rename(true)
            .with_setup(true)
            .with_worktree(true)
            .with_popup(true)
            .with_action_menu(true);
        assert_eq!(
            project_native_capture(claims),
            Some(WorkbenchCaptureOwner::ActionMenu)
        );
    }

    #[test]
    fn lower_client_modes_follow_native_exclusives_in_one_total_order() {
        let copy = CaptureClaims::default().with_copy(true);
        let resize = ClientCaptureClaims::default().with_resize(true);
        assert!(!copy.exclusive_any(), "Copy is below native exclusives");
        assert_eq!(project_native_capture(copy), None);
        assert_eq!(
            project_capture(ClientCaptureClaims::default(), copy),
            Some(CaptureOwner::Copy)
        );
        assert_eq!(
            project_capture(resize, CaptureClaims::default()),
            Some(CaptureOwner::Resize)
        );
        assert_eq!(project_capture(resize, copy), Some(CaptureOwner::Copy));

        let native = copy.with_agent_board(true);
        assert_eq!(
            project_capture(resize, native),
            Some(CaptureOwner::Native(WorkbenchCaptureOwner::AgentBoard))
        );
        assert_eq!(
            project_capture(resize.with_machine(true), native),
            Some(CaptureOwner::Machine)
        );
    }
}
