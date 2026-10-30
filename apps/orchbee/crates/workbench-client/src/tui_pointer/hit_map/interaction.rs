//! Painted interaction targets and the transient pointer projection a renderer reads.
//!
//! A feature renderer attaches a typed target and its primary-button contract to the exact region
//! it painted. Feature effects remain with the dispatcher and their domain owners.

use ratatui::layout::Rect;

use super::{HitLayer, HitTarget, InteractionSpec, ProjectedPointer};

/// Closed semantic identities admitted to the generic interaction pipeline.
///
/// Each production variant carries the owning feature's complete painted
/// witness. A new feature must add an equally exact identity here before its
/// renderer can publish generic activation metadata; broad hit targets whose
/// meaning depends on current context are deliberately excluded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteractionTarget {
    Help(crate::client_view::help::DrawnHelpControl),
    GlobalChrome(super::GlobalChromeAction),
    SettingsRow {
        token: crate::native_settings::Token,
        row: crate::native_settings::Row,
    },
    SettingsControl {
        token: crate::native_settings::Token,
        control: crate::native_settings::Control,
    },
    ActionMenuRow {
        source: crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
        row: usize,
    },
    ActionMenuControl {
        source: crate::action_menu::MenuTarget,
        token: crate::action_menu::MenuToken,
        control: crate::action_menu::MenuControl,
    },
    AgentBoardRow(crate::workbench_client::AgentBoardRowTarget),
    AgentBoardControl(crate::workbench_client::AgentBoardControlTarget),
    Presentation(crate::workbench_client::DrawnPresentationTarget),

    Sidebar(crate::machine_picker::DrawnSidebarTarget),
    SidebarFixed(crate::machine_picker::DrawnSidebarFixedTarget),
    MachinePicker(crate::machine_picker::DrawnPickerTarget),
    NavigatorRow(crate::client_view::navigator::Row),
    NavigatorClose(crate::client_view::navigator::DrawSource),
    TabCreate(crate::action_menu::DrawnTabCreate),
    TabCloseRequest {
        source: crate::action_menu::DrawnTabCreate,
        tab: orchbee_workbench_protocol::WorkbenchTabId,
    },
    TabStripPage {
        source: crate::action_menu::DrawnTabCreate,
        start: crate::workbench_client::state::TabStripStart,
    },
    PopupClose(crate::workbench_client::DrawnPopupClose),
    #[cfg(any(test, feature = "test-support"))]
    Fixture(HitTarget),
}

impl InteractionTarget {
    pub(in crate::tui_pointer) const fn is_help(&self) -> bool {
        matches!(self, Self::Help(_))
    }

    pub(in crate::tui_pointer) const fn is_settings(&self) -> bool {
        matches!(
            self,
            Self::SettingsRow { .. } | Self::SettingsControl { .. }
        )
    }

    pub(in crate::tui_pointer) const fn is_agent_board(&self) -> bool {
        matches!(self, Self::AgentBoardRow(_) | Self::AgentBoardControl(_))
    }

    pub(in crate::tui_pointer) const fn is_action_menu(&self) -> bool {
        matches!(
            self,
            Self::ActionMenuRow { .. } | Self::ActionMenuControl { .. }
        )
    }

    pub(in crate::tui_pointer) const fn is_navigator(&self) -> bool {
        matches!(self, Self::NavigatorRow(_) | Self::NavigatorClose(_))
    }

    pub(in crate::tui_pointer) const fn is_machine_picker(&self) -> bool {
        matches!(self, Self::MachinePicker(_))
    }

    pub(in crate::tui_pointer) const fn is_popup(&self) -> bool {
        matches!(self, Self::PopupClose(_))
    }

    pub(in crate::tui_pointer) const fn is_global_chrome(&self) -> bool {
        if matches!(self, Self::Presentation(_)) {
            return true;
        }

        if matches!(
            self,
            Self::GlobalChrome(_)
                | Self::Sidebar(_)
                | Self::SidebarFixed(_)
                | Self::TabCreate(_)
                | Self::TabCloseRequest { .. }
                | Self::TabStripPage { .. }
        ) {
            return true;
        }
        #[cfg(any(test, feature = "test-support"))]
        if matches!(self, Self::Fixture(_)) {
            return true;
        }
        false
    }

    pub(in crate::tui_pointer) fn hit_target(&self) -> HitTarget {
        match self {
            Self::Help(target) => HitTarget::Help {
                page: target.source().page(),
                action: target.control(),
            },
            Self::GlobalChrome(action) => match action {
                super::GlobalChromeAction::Onboarding(action) => HitTarget::Onboarding(*action),
                super::GlobalChromeAction::RestoreSidebar => HitTarget::SidebarRestoreButton,
            },
            Self::SettingsRow { token, row } => HitTarget::SettingsRow {
                token: *token,
                row: *row,
            },
            Self::SettingsControl { token, control } => HitTarget::SettingsControl {
                token: *token,
                control: *control,
            },
            Self::AgentBoardRow(target) => HitTarget::AgentBoardRow(target.clone()),
            Self::AgentBoardControl(target) => HitTarget::AgentBoardControl(target.clone()),
            Self::Presentation(target) => HitTarget::Presentation(target.clone()),

            Self::ActionMenuRow { token, row, .. } => HitTarget::ActionMenuRow {
                token: *token,
                row: *row,
            },
            Self::ActionMenuControl { token, control, .. } => HitTarget::ActionMenuControl {
                token: *token,
                back: *control == crate::action_menu::MenuControl::Back,
            },
            Self::Sidebar(target) => {
                use crate::machine_picker::{SidebarPart, SidebarTarget};
                match target.part() {
                    SidebarPart::Primary => match target.target() {
                        SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => {
                            HitTarget::MachineGroupRow {
                                target: target.target().clone(),
                                window: target.window(),
                            }
                        }
                        SidebarTarget::Context { .. } => HitTarget::MachineWorkspaceRow {
                            selected: target
                                .selection()
                                .expect("a drawn Context target has a selection"),
                            window: target.window(),
                        },
                        SidebarTarget::Agent { .. } => HitTarget::MachineAgentRow {
                            selected: target
                                .selection()
                                .expect("a drawn Agent target has a selection"),
                            window: target.window(),
                        },
                    },
                    SidebarPart::Disclosure => {
                        HitTarget::SidebarDisclosure(target.target().clone())
                    }
                    SidebarPart::Menu => HitTarget::SidebarMenu(target.target().clone()),
                    SidebarPart::Create => HitTarget::CreateMenu(Some(target.target().clone())),
                    SidebarPart::Close => HitTarget::SidebarClose(target.target().clone()),
                }
            }
            Self::SidebarFixed(target) => {
                use crate::machine_picker::SidebarFixedAction;
                match target.action() {
                    SidebarFixedAction::NewWorkspace => HitTarget::NewWorkspaceButton,
                    SidebarFixedAction::Search => HitTarget::SearchButton,
                    SidebarFixedAction::Settings => HitTarget::SettingsButton,
                    SidebarFixedAction::Browse => HitTarget::OpenMachineWorkspaces,
                }
            }
            Self::MachinePicker(target) => HitTarget::MachinePicker(target.clone()),
            Self::NavigatorRow(row) => HitTarget::NavigatorRow(row.clone()),
            Self::NavigatorClose(source) => HitTarget::NavigatorClose(source.episode()),
            Self::TabCreate(target) => HitTarget::TabCreate(target.clone()),
            Self::TabCloseRequest { source, tab } => HitTarget::TabCloseRequest {
                source: source.clone(),
                tab: tab.clone(),
            },
            Self::TabStripPage { source, start } => HitTarget::TabStripPage {
                source: source.clone(),
                start: start.clone(),
            },
            Self::PopupClose(target) => HitTarget::PopupClose(target.clone()),
            #[cfg(any(test, feature = "test-support"))]
            Self::Fixture(target) => target.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) struct PaintedInteraction {
    pub(in crate::tui_pointer) target: InteractionTarget,
    pub(in crate::tui_pointer) paint_rect: Rect,
    pub(in crate::tui_pointer) spec: InteractionSpec,
}

impl PaintedInteraction {
    pub(in crate::tui_pointer) const fn target(&self) -> &InteractionTarget {
        &self.target
    }
    pub(in crate::tui_pointer) const fn new(
        target: InteractionTarget,
        paint_rect: Rect,
        spec: InteractionSpec,
    ) -> Self {
        Self {
            target,
            paint_rect,
            spec,
        }
    }

    pub(in crate::tui_pointer) fn fingerprint(
        &self,
        layer: HitLayer,
        hit_rect: Rect,
        exposure: ExposureSignature,
    ) -> Fingerprint {
        Fingerprint {
            target: self.target.clone(),
            layer,
            hit_rect,
            paint_rect: self.paint_rect,
            spec: self.spec,
            exposure,
        }
    }

    pub(in crate::tui_pointer) const fn paint_rect(&self) -> Rect {
        self.paint_rect
    }
}

/// Canonical frontmost winners within one interaction's complete painted
/// extent. Presentation-only content is absent; only geometry and semantic
/// ownership that can change the target's effective hit shape participate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::tui_pointer) struct ExposureSignature(pub(in crate::tui_pointer) Vec<ExposurePatch>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) struct ExposurePatch {
    pub(in crate::tui_pointer) rect: Rect,
    pub(in crate::tui_pointer) winner: ExposureWinner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) enum ExposureWinner {
    Region(Box<ExposureRegion>),
    OverlayBackdrop,
    Dead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) struct ExposureRegion {
    pub(in crate::tui_pointer) layer: HitLayer,
    pub(in crate::tui_pointer) target: HitTarget,
    pub(in crate::tui_pointer) interaction: Option<PaintedInteraction>,
}

impl ExposureWinner {
    pub(in crate::tui_pointer) fn region(
        layer: HitLayer,
        target: HitTarget,
        interaction: Option<PaintedInteraction>,
    ) -> Self {
        Self::Region(Box::new(ExposureRegion {
            layer,
            target,
            interaction,
        }))
    }
}

/// Stable semantic identity plus every interaction property whose change
/// invalidates an armed click. Frame sequence is deliberately absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::tui_pointer) struct Fingerprint {
    pub(in crate::tui_pointer) target: InteractionTarget,
    pub(in crate::tui_pointer) layer: HitLayer,
    pub(in crate::tui_pointer) hit_rect: Rect,
    pub(in crate::tui_pointer) paint_rect: Rect,
    pub(in crate::tui_pointer) spec: InteractionSpec,
    pub(in crate::tui_pointer) exposure: ExposureSignature,
}

impl Fingerprint {
    pub(in crate::tui_pointer) fn enabled(&self) -> bool {
        self.spec.enabled_for_pointer()
    }
}

/// Immutable transient facts captured before one candidate render begins.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PointerProjection {
    pub(in crate::tui_pointer) revision: u64,
    pub(in crate::tui_pointer) hovered: Option<Fingerprint>,
    pub(in crate::tui_pointer) pressed: Option<Fingerprint>,
    pub(in crate::tui_pointer) modal: super::modal::Projection,
    pub(in crate::tui_pointer) chrome: super::chrome::Projection,
    pub(in crate::tui_pointer) tooltip: Option<super::tooltip::Shown>,
}

impl PointerProjection {
    pub(in crate::tui_pointer) fn with_specialized_owner(
        mut self,
        pressed: Option<Fingerprint>,
    ) -> Self {
        self.pressed = pressed;
        self.hovered = None;
        self
    }

    pub(in crate::tui_pointer) const fn empty() -> Self {
        Self {
            revision: 0,
            hovered: None,
            pressed: None,
            modal: super::modal::Projection::empty(),
            chrome: super::chrome::Projection::empty(),
            tooltip: None,
        }
    }

    pub(in crate::tui_pointer) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(in crate::tui_pointer) fn with_modal(mut self, modal: super::modal::Projection) -> Self {
        self.modal = modal;
        self
    }

    pub(in crate::tui_pointer) const fn modal(&self) -> &super::modal::Projection {
        &self.modal
    }

    pub(in crate::tui_pointer) fn project_modal(
        &self,
        fingerprint: &super::modal::Fingerprint,
    ) -> ProjectedPointer {
        self.modal.project(fingerprint)
    }

    pub(in crate::tui_pointer) fn with_chrome(mut self, chrome: super::chrome::Projection) -> Self {
        self.chrome = chrome;
        self
    }

    pub(in crate::tui_pointer) const fn chrome(&self) -> &super::chrome::Projection {
        &self.chrome
    }

    pub(in crate::tui_pointer) fn with_tooltip(
        mut self,
        tooltip: Option<super::tooltip::Shown>,
    ) -> Self {
        self.tooltip = tooltip;
        self
    }

    pub(in crate::tui_pointer) const fn tooltip(&self) -> Option<&super::tooltip::Shown> {
        self.tooltip.as_ref()
    }

    pub(in crate::tui_pointer) fn project_chrome(
        &self,
        fingerprint: &super::chrome::Fingerprint,
    ) -> ProjectedPointer {
        self.chrome.project(fingerprint)
    }

    pub(in crate::tui_pointer) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
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
