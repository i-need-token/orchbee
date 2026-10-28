//! Transient pointer presentation and release-activation authority.
//!
//! This module owns only the generic interaction facts projected into a
//! renderer. Feature effects remain with the dispatcher and their domain
//! owners, while specialized drags keep their dedicated state machines.

use ratatui::layout::Rect;

use super::{FrameHitMap, HitLayer, HitTarget};
use crate::{
    client_view::Modifiers,
    terminal_input::{MouseAction, MouseButton, MouseEvent},
};

/// Whether a painted interaction may currently accept its declared gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InteractionAvailability {
    Enabled,
    Disabled,
}

/// The explicit primary-button contract declared by a feature renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimaryContract {
    /// Arm on press and emit the semantic intent on a matching release.
    ActivateOnRelease,
    /// Emit one provisional navigation intent on press; release only ends ownership.
    SelectOnPress,
    /// Hover may project here, but a dedicated gesture state machine owns presses.
    Specialized,
}

/// Typed interaction metadata supplied by the feature that owns the target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InteractionSpec {
    primary: PrimaryContract,
    availability: InteractionAvailability,
}

impl InteractionSpec {
    pub(crate) const fn new(
        primary: PrimaryContract,
        availability: InteractionAvailability,
    ) -> Self {
        Self {
            primary,
            availability,
        }
    }

    pub(crate) const fn enabled(primary: PrimaryContract) -> Self {
        Self::new(primary, InteractionAvailability::Enabled)
    }

    pub(crate) const fn disabled(primary: PrimaryContract) -> Self {
        Self::new(primary, InteractionAvailability::Disabled)
    }

    pub(super) const fn enabled_for_pointer(self) -> bool {
        matches!(self.availability, InteractionAvailability::Enabled)
    }
}

/// The renderer-facing pointer axis. Domain focus, selection, current, checked,
/// availability, and operation state stay independent in the component model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ProjectedPointer {
    #[default]
    Rest,
    Hovered,
    Pressed,
}

/// Whether the settled outer transport can report button-free movement.
///
/// Lifecycle code changes this only after its mode effect settles. Re-admission
/// never supplies a synthetic position; a real movement report must arrive.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum MotionObservability {
    #[default]
    ButtonOnly,
    AnyMotion,
}

/// Closed semantic identities admitted to the generic interaction pipeline.
///
/// Each production variant carries the owning feature's complete painted
/// witness. A new feature must add an equally exact identity here before its
/// renderer can publish generic activation metadata; broad hit targets whose
/// meaning depends on current context are deliberately excluded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InteractionTarget {
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
    PopupClose(crate::workbench_client::DrawnPopupClose),
    #[cfg(test)]
    Fixture(HitTarget),
}

impl InteractionTarget {
    pub(super) const fn is_help(&self) -> bool {
        matches!(self, Self::Help(_))
    }

    pub(super) const fn is_settings(&self) -> bool {
        matches!(
            self,
            Self::SettingsRow { .. } | Self::SettingsControl { .. }
        )
    }

    pub(super) const fn is_agent_board(&self) -> bool {
        matches!(self, Self::AgentBoardRow(_) | Self::AgentBoardControl(_))
    }

    pub(super) const fn is_action_menu(&self) -> bool {
        matches!(
            self,
            Self::ActionMenuRow { .. } | Self::ActionMenuControl { .. }
        )
    }

    pub(super) const fn is_navigator(&self) -> bool {
        matches!(self, Self::NavigatorRow(_) | Self::NavigatorClose(_))
    }

    pub(super) const fn is_machine_picker(&self) -> bool {
        matches!(self, Self::MachinePicker(_))
    }

    pub(super) const fn is_popup(&self) -> bool {
        matches!(self, Self::PopupClose(_))
    }

    pub(super) const fn is_global_chrome(&self) -> bool {
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
        ) {
            return true;
        }
        #[cfg(test)]
        if matches!(self, Self::Fixture(_)) {
            return true;
        }
        false
    }

    pub(super) fn hit_target(&self) -> HitTarget {
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
                        SidebarTarget::Project { .. } => HitTarget::MachineProjectRow {
                            target: target.target().clone(),
                            window: target.window(),
                        },
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
            Self::PopupClose(target) => HitTarget::PopupClose(target.clone()),
            #[cfg(test)]
            Self::Fixture(target) => target.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PaintedInteraction {
    target: InteractionTarget,
    paint_rect: Rect,
    spec: InteractionSpec,
}

impl PaintedInteraction {
    pub(super) const fn target(&self) -> &InteractionTarget {
        &self.target
    }
    pub(super) const fn new(
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

    pub(super) fn fingerprint(
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

    pub(super) const fn paint_rect(&self) -> Rect {
        self.paint_rect
    }
}

/// Canonical frontmost winners within one interaction's complete painted
/// extent. Presentation-only content is absent; only geometry and semantic
/// ownership that can change the target's effective hit shape participate.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct ExposureSignature(pub(super) Vec<ExposurePatch>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExposurePatch {
    pub(super) rect: Rect,
    pub(super) winner: ExposureWinner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum ExposureWinner {
    Region(Box<ExposureRegion>),
    OverlayBackdrop,
    Dead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExposureRegion {
    layer: HitLayer,
    target: HitTarget,
    interaction: Option<PaintedInteraction>,
}

impl ExposureWinner {
    pub(super) fn region(
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
pub(super) struct Fingerprint {
    target: InteractionTarget,
    layer: HitLayer,
    hit_rect: Rect,
    paint_rect: Rect,
    spec: InteractionSpec,
    exposure: ExposureSignature,
}

impl Fingerprint {
    fn enabled(&self) -> bool {
        self.spec.enabled_for_pointer()
    }
}

/// Immutable transient facts captured before one candidate render begins.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PointerProjection {
    revision: u64,
    hovered: Option<Fingerprint>,
    pressed: Option<Fingerprint>,
    modal: super::modal::Projection,
    chrome: super::chrome::Projection,
}

impl PointerProjection {
    pub(super) fn with_specialized_owner(mut self, pressed: Option<Fingerprint>) -> Self {
        self.pressed = pressed;
        self.hovered = None;
        self
    }

    pub(super) const fn empty() -> Self {
        Self {
            revision: 0,
            hovered: None,
            pressed: None,
            modal: super::modal::Projection::empty(),
            chrome: super::chrome::Projection::empty(),
        }
    }

    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn with_modal(mut self, modal: super::modal::Projection) -> Self {
        self.modal = modal;
        self
    }

    pub(super) const fn modal(&self) -> &super::modal::Projection {
        &self.modal
    }

    pub(super) fn project_modal(
        &self,
        fingerprint: &super::modal::Fingerprint,
    ) -> ProjectedPointer {
        self.modal.project(fingerprint)
    }

    pub(super) fn with_chrome(mut self, chrome: super::chrome::Projection) -> Self {
        self.chrome = chrome;
        self
    }

    pub(super) const fn chrome(&self) -> &super::chrome::Projection {
        &self.chrome
    }

    pub(super) fn project_chrome(
        &self,
        fingerprint: &super::chrome::Fingerprint,
    ) -> ProjectedPointer {
        self.chrome.project(fingerprint)
    }

    pub(super) fn project(&self, fingerprint: &Fingerprint) -> ProjectedPointer {
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

#[derive(Clone, Debug)]
struct Arm {
    fingerprint: Fingerprint,
    backdrop: bool,
    column: u16,
    row: u16,
    cancelled: bool,
}

impl Arm {
    fn matches_map(&self, map: &FrameHitMap, column: u16, row: u16) -> bool {
        let current = if self.backdrop {
            map.backdrop_interaction_at(column, row)
        } else {
            map.interaction_at(column, row)
        };
        current.as_ref() == Some(&self.fingerprint)
    }

    fn active(&self) -> bool {
        !self.cancelled
    }

    fn cancel(&mut self) -> bool {
        let changed = !self.cancelled;
        self.cancelled = true;
        changed
    }
}

pub(super) enum Event {
    Ignored,
    Captured,
    /// An already accepted primary press reached its cleanup boundary
    /// without activating. The dispatcher must fence the physical packet
    /// that carried the matching release.
    Ended,
    Intent {
        target: Box<InteractionTarget>,
        contract: PrimaryContract,
    },
}

/// Pointer-side ownership of a packet-level primary-release latch.
///
/// Active owners must see the release before the dispatcher fences its packet.
/// Cleanup owners have already lost semantic authority but still consume that
/// exact release once without consulting current hit geometry.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ReleaseOwnership {
    #[default]
    None,
    Active,
    Cleanup,
}

impl ReleaseOwnership {
    pub(super) const fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Active, _) | (_, Self::Active) => Self::Active,
            (Self::Cleanup, _) | (_, Self::Cleanup) => Self::Cleanup,
            (Self::None, Self::None) => Self::None,
        }
    }
}

/// One generic pointer owner. A cancelled arm remains as a cleanup tombstone
/// until its matching release or a later press retires it.
#[derive(Default)]
pub(super) struct State {
    position: Option<(u16, u16)>,
    hovered: Option<Fingerprint>,
    arm: Option<Arm>,
    revision: u64,
    redraw_after_publish: bool,
}

impl State {
    pub(super) fn hovered_target(&self) -> Option<&InteractionTarget> {
        self.hovered.as_ref().map(|fingerprint| &fingerprint.target)
    }

    pub(super) const fn revision(&self) -> u64 {
        self.revision
    }

    pub(super) fn projection(&self) -> PointerProjection {
        PointerProjection {
            revision: self.revision,
            hovered: self.hovered.clone(),
            pressed: self
                .arm
                .as_ref()
                .filter(|arm| arm.active() && !arm.backdrop)
                .map(|arm| arm.fingerprint.clone()),
            modal: super::modal::Projection::empty(),
            chrome: super::chrome::Projection::empty(),
        }
    }

    pub(super) fn release_ownership(&self) -> ReleaseOwnership {
        match self.arm.as_ref() {
            Some(arm) if arm.active() => ReleaseOwnership::Active,
            Some(_) => ReleaseOwnership::Cleanup,
            None => ReleaseOwnership::None,
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    fn replace_hover(&mut self, next: Option<Fingerprint>) -> bool {
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        self.changed();
        true
    }

    fn cancel_arm(&mut self) -> bool {
        let changed = self.arm.as_mut().is_some_and(Arm::cancel);
        if changed {
            self.changed();
        }
        changed
    }

    /// Applies a real pointer report to hover presentation. Only movement
    /// can establish position after transport loss; later reports may refine
    /// an already observed position while any-motion remains admitted.
    pub(super) fn observe_motion(
        &mut self,
        map: Option<&FrameHitMap>,
        observability: MotionObservability,
        event: MouseEvent,
        accepts: fn(&InteractionTarget) -> bool,
    ) -> bool {
        if observability != MotionObservability::AnyMotion
            || event.mods.shift
            || (event.action != MouseAction::Motion && self.position.is_none())
        {
            return false;
        }
        self.position = Some((event.column, event.row));
        let next = map
            .and_then(|map| map.interaction_at(event.column, event.row))
            .filter(|fingerprint| accepts(&fingerprint.target))
            .filter(Fingerprint::enabled);
        self.replace_hover(next)
    }

    /// Gives a declared generic interaction first refusal over its press,
    /// ordered boundary crossings, and matching release.
    pub(super) fn event(
        &mut self,
        map: Option<&FrameHitMap>,
        event: MouseEvent,
        admit_new: bool,
        accepts: fn(&InteractionTarget) -> bool,
    ) -> Event {
        if self
            .arm
            .as_ref()
            .is_some_and(|arm| !accepts(&arm.fingerprint.target))
        {
            self.cancel_arm();
        }
        if let Some(arm) = &mut self.arm {
            let was_active = arm.active();
            if event.mods != Modifiers::NONE {
                arm.cancel();
            }
            if event.action == MouseAction::Motion
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                if !was_active {
                    return Event::Captured;
                }
                arm.column = event.column;
                arm.row = event.row;
                let current = map.is_some_and(|map| arm.matches_map(map, event.column, event.row));
                if !current {
                    arm.cancel();
                }
                if was_active && !arm.active() {
                    self.changed();
                }
                return Event::Captured;
            }
            if event.action == MouseAction::Release
                && matches!(event.button, MouseButton::Left | MouseButton::None)
            {
                let arm = self.arm.take().expect("checked above");
                let current = map.is_some_and(|map| arm.matches_map(map, event.column, event.row));
                let admitted = arm.active()
                    && current
                    && (arm.backdrop
                        || super::contains(arm.fingerprint.paint_rect, event.column, event.row));
                if was_active {
                    self.changed();
                }
                return if admitted
                    && arm.fingerprint.spec.primary == PrimaryContract::ActivateOnRelease
                {
                    Event::Intent {
                        target: Box::new(arm.fingerprint.target),
                        contract: arm.fingerprint.spec.primary,
                    }
                } else {
                    Event::Ended
                };
            }
            if event.action == MouseAction::Press
                && matches!(
                    event.button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                )
            {
                arm.cancel();
                if was_active && !arm.active() {
                    self.changed();
                }
                return Event::Captured;
            }
            let remains_active = arm.active();
            if was_active && !remains_active {
                self.changed();
            }
            if event.action != MouseAction::Press {
                return if remains_active {
                    Event::Ignored
                } else {
                    Event::Captured
                };
            }
            let previous = self.arm.take().expect("checked above");
            if previous.active() {
                self.changed();
            }
        }

        if event.action != MouseAction::Press
            || event.button != MouseButton::Left
            || event.mods != Modifiers::NONE
            || !admit_new
        {
            return Event::Ignored;
        }
        let Some(map) = map else {
            return Event::Ignored;
        };
        let backdrop = matches!(
            map.hit(event.column, event.row),
            super::HitResolution::OverlayBackdrop
        );
        let Some(fingerprint) = map
            .interaction_at(event.column, event.row)
            .or_else(|| map.backdrop_interaction_at(event.column, event.row))
            .filter(|fingerprint| accepts(&fingerprint.target))
        else {
            return Event::Ignored;
        };
        if !fingerprint.enabled() {
            // A disabled painted control remains an explicit interaction
            // boundary. Retain cleanup-only ownership so its matching
            // release cannot fall through to a containing row or pane, but
            // never project Pressed or emit a semantic intent.
            self.arm = Some(Arm {
                fingerprint,
                backdrop,
                column: event.column,
                row: event.row,
                cancelled: true,
            });
            return Event::Captured;
        }
        if fingerprint.spec.primary == PrimaryContract::Specialized {
            return Event::Ignored;
        }

        let contract = fingerprint.spec.primary;
        let target = fingerprint.target.clone();
        self.arm = Some(Arm {
            fingerprint,
            backdrop,
            column: event.column,
            row: event.row,
            cancelled: false,
        });
        self.changed();
        if contract == PrimaryContract::SelectOnPress {
            Event::Intent {
                target: Box::new(target),
                contract,
            }
        } else {
            Event::Captured
        }
    }

    /// Settles only cleanup ownership after the transport or host focus has
    /// gone away. It cannot re-hit, arm, or activate a target. A fresh
    /// non-wheel press retires the old debt and belongs to the new transport
    /// state; a matching late release ends the accepted gesture exactly once.
    pub(super) fn cleanup_event(&mut self, event: MouseEvent) -> Event {
        let Some(arm) = self.arm.as_ref() else {
            return Event::Ignored;
        };
        if arm.active() {
            return Event::Ignored;
        }
        if event.action == MouseAction::Press
            && !matches!(
                event.button,
                MouseButton::WheelUp
                    | MouseButton::WheelDown
                    | MouseButton::WheelLeft
                    | MouseButton::WheelRight
            )
        {
            self.arm = None;
            return Event::Ignored;
        }
        if event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None)
        {
            self.arm = None;
            return Event::Ended;
        }
        Event::Captured
    }

    /// Reconciles only after the candidate frame was successfully presented.
    /// Returns whether that frame used an older projection and needs one later
    /// bounded repaint.
    pub(super) fn publish(&mut self, map: &FrameHitMap) -> bool {
        if self.arm.as_ref().is_some_and(Arm::active) {
            let still_current = self
                .arm
                .as_ref()
                .is_some_and(|arm| arm.matches_map(map, arm.column, arm.row));
            if !still_current {
                self.cancel_arm();
            }
        }
        if self.arm.as_ref().is_none_or(|arm| !arm.active()) {
            let next = self
                .position
                .and_then(|(column, row)| map.interaction_at(column, row))
                .filter(Fingerprint::enabled);
            self.replace_hover(next);
        }
        self.redraw_after_publish = map.projection_revision != self.revision;
        self.redraw_after_publish
    }

    /// Structural invalidation clears presented authority but retains an
    /// observed position for resolution against the next successful frame.
    pub(super) fn invalidate_map(&mut self) {
        self.replace_hover(None);
        self.cancel_arm();
        self.redraw_after_publish = false;
    }

    /// Capture, focus, or motion-observability loss also forgets position so
    /// restoration cannot synthesize hover.
    pub(super) fn clear_transport(&mut self) {
        self.position = None;
        self.replace_hover(None);
        self.cancel_arm();
        self.redraw_after_publish = false;
    }

    pub(super) fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}
