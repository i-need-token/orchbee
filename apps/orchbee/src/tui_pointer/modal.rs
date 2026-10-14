//! Painted modal controls retain their press authority until matching release.
//! The state owner then validates that exact token before any shared effect.
use super::{
    FrameHitMap, Gesture, HitLayer, HitResolution, HitTarget, InteractionAvailability,
    InteractionTarget, ModalControl, Modifiers, MotionObservability, MouseAction, MouseButton,
    MouseEvent, PointerOutcome, PointerState, ProjectedPointer, ReleaseOwnership,
    TerminalInputPort,
};
use ratatui::layout::Rect;

#[derive(Clone, Debug, Eq, PartialEq)]
enum ControlTarget {
    #[cfg(windows)]
    Copy(
        crate::terminal_copy_mode::CopyModeToken,
        crate::terminal_copy_mode::CopyControl,
    ),
    #[cfg(windows)]
    Resize(super::ResizeControl),
    TabClose(crate::terminal_session::NativeCloseConfirmationToken, bool),
    PanelClose(
        crate::terminal_session::NativePanelCloseConfirmationToken,
        bool,
    ),
    Rename(crate::terminal_session::NativeRenameDraftToken, bool),
    Setup(crate::terminal_session::NativeSetupToken, ModalControl),
    #[cfg(windows)]
    Worktree(crate::terminal_session::WorktreeToken, ModalControl),
    Close(crate::terminal_session::WorkspaceCloseToken, bool),
}

impl ControlTarget {
    fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            #[cfg(windows)]
            HitTarget::CopyControl { token, control } => Self::Copy(*token, *control),
            #[cfg(windows)]
            HitTarget::Resize(control) => Self::Resize(control.clone()),
            HitTarget::TabClose { token, confirm } => Self::TabClose(*token, *confirm),
            HitTarget::PanelClose { token, confirm } => Self::PanelClose(*token, *confirm),
            HitTarget::Rename { token, submit } => Self::Rename(*token, *submit),
            HitTarget::Setup { token, control } => Self::Setup(*token, *control),
            #[cfg(windows)]
            HitTarget::Worktree { token, control } => Self::Worktree(*token, *control),
            HitTarget::WorkspaceClose { token, confirm } => Self::Close(*token, *confirm),
            _ => return None,
        })
    }

    fn projectable(&self) -> bool {
        if matches!(self, Self::Setup(_, ModalControl::Surface)) {
            return false;
        }
        #[cfg(windows)]
        if matches!(self, Self::Worktree(_, ModalControl::Surface)) {
            return false;
        }
        true
    }
}

#[cfg(windows)]
fn accepts_copy(target: &ControlTarget) -> bool {
    matches!(target, ControlTarget::Copy(..))
}

#[cfg(windows)]
fn accepts_resize(target: &ControlTarget) -> bool {
    matches!(target, ControlTarget::Resize(_))
}

fn accepts_confirmation(target: &ControlTarget) -> bool {
    matches!(
        target,
        ControlTarget::Close(..) | ControlTarget::TabClose(..) | ControlTarget::PanelClose(..)
    )
}

fn accepts_modal(target: &ControlTarget) -> bool {
    match target {
        ControlTarget::Rename(..) => true,
        ControlTarget::Setup(_, control) => *control != ModalControl::Surface,
        #[cfg(windows)]
        ControlTarget::Worktree(_, control) => *control != ModalControl::Surface,
        _ => false,
    }
}

/// Exact paint identity for one specialized modal control. The target holds
/// its opaque owner token; geometry and layer prevent a repaint from silently
/// reusing press or hover presentation for a different painted control.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Fingerprint {
    target: ControlTarget,
    layer: HitLayer,
    area: Rect,
    availability: InteractionAvailability,
}

impl Fingerprint {
    pub(super) fn from_hit(
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

    pub(super) const fn enabled(&self) -> bool {
        matches!(self.availability, InteractionAvailability::Enabled)
    }

    pub(super) fn same_control(&self, other: &Self) -> bool {
        self.target == other.target && self.layer == other.layer && self.area == other.area
    }
}

/// Immutable specialized-modal pointer facts captured before one candidate
/// frame. Effects remain with `control_event`; this value is paint-only.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Projection {
    hovered: Option<Fingerprint>,
    pressed: Option<Fingerprint>,
}

impl Projection {
    pub(super) const fn empty() -> Self {
        Self {
            hovered: None,
            pressed: None,
        }
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

/// Live pointer position and hover for specialized modal controls. Press
/// authority stays in `PaintedPress`; snapshots combine both without moving
/// any semantic effect into the renderer.
#[derive(Default)]
pub(super) struct ProjectionState {
    position: Option<(u16, u16)>,
    hovered: Option<Fingerprint>,
    redraw_after_publish: bool,
}

impl ProjectionState {
    fn replace_hover(&mut self, next: Option<Fingerprint>) -> bool {
        if self.hovered == next {
            return false;
        }
        self.hovered = next;
        true
    }

    fn observe_motion(
        &mut self,
        map: Option<&FrameHitMap>,
        observability: MotionObservability,
        event: MouseEvent,
        accepts: impl Fn(&ControlTarget) -> bool,
    ) -> bool {
        if observability != MotionObservability::AnyMotion
            || event.mods.shift
            || (event.action != MouseAction::Motion && self.position.is_none())
        {
            return false;
        }
        self.position = Some((event.column, event.row));
        let next = map
            .and_then(|map| map.modal_fingerprint_at(event.column, event.row))
            .filter(|fingerprint| accepts(&fingerprint.target))
            .filter(Fingerprint::enabled);
        self.replace_hover(next)
    }

    fn snapshot(&self, pressed: Option<Fingerprint>, suppress_hover: bool) -> Projection {
        Projection {
            hovered: if suppress_hover {
                None
            } else {
                self.hovered.clone()
            },
            pressed,
        }
    }

    fn publish(
        &mut self,
        map: &FrameHitMap,
        pressed: Option<Fingerprint>,
        suppress_hover: bool,
    ) -> bool {
        if pressed.is_none() {
            let next = self
                .position
                .and_then(|(column, row)| map.modal_fingerprint_at(column, row))
                .filter(Fingerprint::enabled);
            self.replace_hover(next);
        }
        self.redraw_after_publish = map.modal_projection != self.snapshot(pressed, suppress_hover);
        self.redraw_after_publish
    }

    fn invalidate_map(&mut self) {
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    fn clear_transport(&mut self) {
        self.position = None;
        self.hovered = None;
        self.redraw_after_publish = false;
    }

    fn take_redraw_after_publish(&mut self) -> bool {
        std::mem::take(&mut self.redraw_after_publish)
    }
}

pub(super) struct PaintedPress {
    fingerprint: Fingerprint,
    column: u16,
    row: u16,
    cancelled: bool,
}

/// Cleanup-only disposition for a painted press whose semantic effect was
/// already revoked. It never consults current geometry or replays the target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CancelledModalEvent {
    Ignored,
    Captured,
    Retired,
    Ended,
}

impl PaintedPress {
    pub(super) fn active(&self) -> bool {
        !self.cancelled
    }
    fn active_fingerprint(&self) -> Option<Fingerprint> {
        if self.active() {
            Some(self.fingerprint.clone())
        } else {
            None
        }
    }
    fn release_ownership(&self) -> ReleaseOwnership {
        if self.active() {
            ReleaseOwnership::Active
        } else {
            ReleaseOwnership::Cleanup
        }
    }
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub(super) fn exposed_in(&self, map: &FrameHitMap) -> bool {
        map.modal_fingerprint_at(self.column, self.row).as_ref() == Some(&self.fingerprint)
    }
}

enum ControlEvent {
    Ignored,
    Captured,
    Released(Option<ControlTarget>),
}

fn cancelled_press_event(
    press: &mut Option<PaintedPress>,
    event: MouseEvent,
) -> CancelledModalEvent {
    let Some(pending) = press.as_ref() else {
        return CancelledModalEvent::Ignored;
    };
    if pending.active() {
        return CancelledModalEvent::Ignored;
    }
    let fresh_press = event.action == MouseAction::Press
        && !matches!(
            event.button,
            MouseButton::WheelUp
                | MouseButton::WheelDown
                | MouseButton::WheelLeft
                | MouseButton::WheelRight
        );
    if fresh_press {
        *press = None;
        return CancelledModalEvent::Retired;
    }
    if event.action == MouseAction::Release
        && matches!(event.button, MouseButton::Left | MouseButton::None)
    {
        *press = None;
        return CancelledModalEvent::Ended;
    }
    CancelledModalEvent::Captured
}

fn control_event(
    map: Option<&FrameHitMap>,
    capture: bool,
    admit_new: bool,
    press: &mut Option<PaintedPress>,
    event: MouseEvent,
    accepts: impl Fn(&ControlTarget) -> bool,
) -> ControlEvent {
    if !capture {
        if let Some(press) = press {
            press.cancel();
        }
        return ControlEvent::Ignored;
    }
    if let Some(pending) = press {
        if event.mods != Modifiers::NONE {
            pending.cancel();
        }
        if event.action == MouseAction::Motion {
            let current = map.and_then(|map| map.modal_fingerprint_at(event.column, event.row));
            pending.column = event.column;
            pending.row = event.row;
            if current.as_ref() != Some(&pending.fingerprint)
                || !super::contains(pending.fingerprint.area, event.column, event.row)
            {
                // Leaving this exact painted target ends the click intent.
                // Moving back cannot re-arm it without a new press.
                pending.cancel();
            }
        }
        if matches!(event.button, MouseButton::Left | MouseButton::None) {
            if event.action == MouseAction::Motion {
                return ControlEvent::Captured;
            }
            if event.action == MouseAction::Release {
                let Some(pending) = press.take() else {
                    return ControlEvent::Ignored;
                };
                let current = map
                    .and_then(|map| map.modal_fingerprint_at(event.column, event.row))
                    .as_ref()
                    == Some(&pending.fingerprint);
                let admitted = !pending.cancelled
                    && current
                    && super::contains(pending.fingerprint.area, event.column, event.row);
                return ControlEvent::Released(admitted.then_some(pending.fingerprint.target));
            }
        }
        if event.action == MouseAction::Press {
            *press = None;
        }
    }
    if event.action != MouseAction::Press
        || event.button != MouseButton::Left
        || event.mods != Modifiers::NONE
        || !admit_new
    {
        return ControlEvent::Ignored;
    }
    let Some(map) = map else {
        return ControlEvent::Ignored;
    };
    let Some(fingerprint) = map
        .modal_fingerprint_at(event.column, event.row)
        .filter(Fingerprint::enabled)
        .filter(|fingerprint| accepts(&fingerprint.target))
    else {
        return ControlEvent::Ignored;
    };
    *press = Some(PaintedPress {
        fingerprint,
        column: event.column,
        row: event.row,
        cancelled: false,
    });
    ControlEvent::Captured
}

impl PointerState {
    pub(super) fn modal_projection(&self) -> Projection {
        let close = self
            .close_press
            .as_ref()
            .and_then(PaintedPress::active_fingerprint);
        let modal = self
            .modal_press
            .as_ref()
            .and_then(PaintedPress::active_fingerprint);
        let pressed = match (close, modal) {
            (Some(fingerprint), None) | (None, Some(fingerprint)) => Some(fingerprint),
            (None, None) | (Some(_), Some(_)) => None,
        };
        let suppress_hover = self.close_press.is_some()
            || self.modal_press.is_some()
            || self.interactions.release_ownership() != ReleaseOwnership::None;
        self.modal_projection.snapshot(pressed, suppress_hover)
    }

    fn observe_modal_motion(
        &mut self,
        event: MouseEvent,
        accepts: impl Fn(&ControlTarget) -> bool,
    ) -> bool {
        if !self.capture || self.host_unfocused {
            return false;
        }
        self.modal_projection.observe_motion(
            self.map.as_ref(),
            self.motion_observability,
            event,
            accepts,
        )
    }

    pub(super) fn publish_modal_projection(&mut self, map: &FrameHitMap) -> bool {
        let projection = self.modal_projection();
        let suppress_hover = self.close_press.is_some()
            || self.modal_press.is_some()
            || self.interactions.release_ownership() != ReleaseOwnership::None;
        self.modal_projection
            .publish(map, projection.pressed, suppress_hover)
    }

    pub(super) fn invalidate_modal_projection(&mut self) {
        self.modal_projection.invalidate_map();
    }

    pub(super) fn clear_modal_transport(&mut self) {
        self.modal_projection.clear_transport();
    }

    pub(super) fn take_modal_projection_redraw(&mut self) -> bool {
        self.modal_projection.take_redraw_after_publish()
    }

    fn can_admit_modal_press(&self) -> bool {
        self.close_press.is_none()
            && self.modal_press.is_none()
            && self.interactions.release_ownership() == ReleaseOwnership::None
            && matches!(self.gesture, Gesture::Idle)
    }

    /// Settles only already-cancelled painted presses. Active controls keep
    /// their normal owner so an exact release can still run its declared
    /// effect. A late cancelled release is taken once without a hit lookup.
    pub(crate) fn cancelled_modal_event(&mut self, event: MouseEvent) -> CancelledModalEvent {
        let close = cancelled_press_event(&mut self.close_press, event);
        let modal = cancelled_press_event(&mut self.modal_press, event);
        let outcome = if close == CancelledModalEvent::Ended || modal == CancelledModalEvent::Ended
        {
            CancelledModalEvent::Ended
        } else if close == CancelledModalEvent::Retired || modal == CancelledModalEvent::Retired {
            CancelledModalEvent::Retired
        } else if close == CancelledModalEvent::Captured || modal == CancelledModalEvent::Captured {
            CancelledModalEvent::Captured
        } else {
            CancelledModalEvent::Ignored
        };
        if matches!(
            outcome,
            CancelledModalEvent::Ended | CancelledModalEvent::Retired
        ) {
            self.modal_projection.clear_transport();
        }
        outcome
    }

    /// Exact pointer-side owner of the dispatcher's matching release latch.
    ///
    /// Active generic, confirmation and legacy modal controls must process
    /// their release before the packet latch fences its tail. Cancelled arms
    /// retain cleanup ownership without retaining semantic authority.
    pub(crate) fn modal_release_ownership(&self) -> ReleaseOwnership {
        let mut ownership = self.interactions.release_ownership();
        for press in [&self.close_press, &self.modal_press].into_iter().flatten() {
            ownership = ownership.merge(press.release_ownership());
        }
        #[cfg(windows)]
        if self.copy_drag.is_some() {
            ownership = ownership.merge(ReleaseOwnership::Active);
        }
        ownership
    }

    #[cfg(windows)]
    pub(super) fn copy_control_event(
        &mut self,
        view: &mut crate::client_view::ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        self.observe_modal_motion(event, accepts_copy);
        let admit_new = self.can_admit_modal_press();
        let ControlEvent::Released(target) = control_event(
            self.map.as_ref(),
            self.capture,
            admit_new,
            &mut self.modal_press,
            event,
            accepts_copy,
        ) else {
            return false;
        };
        if target.is_none() {
            self.modal_projection.clear_transport();
        }
        if let Some(ControlTarget::Copy(token, control)) = target {
            if control == crate::terminal_copy_mode::CopyControl::Help {
                let transition =
                    view.decide_action(crate::client_view::ClientAction::OpenShortcutHelp);
                view.commit(
                    transition,
                    crate::client_view::SessionActionDisposition::KeepNavigator,
                );
            } else {
                port.click_copy_control(token, control);
            }
        }
        self.stale_map();
        true
    }
    #[cfg(windows)]
    pub(crate) fn resize_event(
        &mut self,
        view: &mut crate::client_view::ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        self.observe_modal_motion(event, accepts_resize);
        let admit_new = self.can_admit_modal_press();
        let ControlEvent::Released(target) = control_event(
            self.map.as_ref(),
            self.capture,
            admit_new,
            &mut self.modal_press,
            event,
            accepts_resize,
        ) else {
            return false;
        };
        if target.is_none() {
            self.modal_projection.clear_transport();
        }
        if view.resize_mode()
            && let Some(ControlTarget::Resize(control)) = target
        {
            match control {
                super::ResizeControl::Adjust {
                    target,
                    direction,
                    fine,
                } => {
                    view.clear_resize_notice();
                    port.click_resize_control(&target, direction, fine);
                }
                super::ResizeControl::Step(previous) if view.resize_pointer_fine == previous => {
                    view.resize_pointer_fine = !previous;
                }
                super::ResizeControl::Help => {
                    let transition =
                        view.decide_action(crate::client_view::ClientAction::OpenShortcutHelp);
                    view.commit(
                        transition,
                        crate::client_view::SessionActionDisposition::KeepNavigator,
                    );
                }
                super::ResizeControl::Exit => {
                    port.apply_client_action(crate::client_view::ClientAction::ExitResizeMode);
                    view.exit_resize_mode();
                }
                _ => {}
            }
        }
        self.stale_map();
        true
    }

    pub(crate) fn board_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> PointerOutcome {
        if super::plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_agent_board);
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        if !self.capture || event.mods != Modifiers::NONE || event.action != MouseAction::Press {
            return PointerOutcome::Ignored;
        }
        let Some(map) = &self.map else {
            return PointerOutcome::Ignored;
        };
        match map.hit(event.column, event.row) {
            HitResolution::OverlayBackdrop
                if matches!(
                    event.button,
                    MouseButton::Left | MouseButton::Right | MouseButton::Middle
                ) =>
            {
                port.agent_board_dismiss();
                PointerOutcome::RegionsChanged
            }
            _ => PointerOutcome::Ignored,
        }
    }

    /// Revokes pending effects while retaining ownership of their late releases.
    pub(crate) fn cancel_modal_press(&mut self) {
        #[cfg(windows)]
        self.cancel_copy_drag();
        for press in [&mut self.close_press, &mut self.modal_press]
            .into_iter()
            .flatten()
        {
            press.cancel();
        }
        self.modal_projection.clear_transport();
    }

    /// Some consumes a close-control gesture; true also consumes the decision
    /// packet tail. Shared close effects wait for the matching release.
    pub(crate) fn confirmation_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> Option<bool> {
        self.observe_modal_motion(event, accepts_confirmation);
        let admit_new = self.can_admit_modal_press();
        match control_event(
            self.map.as_ref(),
            self.capture,
            admit_new,
            &mut self.close_press,
            event,
            accepts_confirmation,
        ) {
            ControlEvent::Ignored => None,
            ControlEvent::Captured => Some(false),
            ControlEvent::Released(target) => {
                if target.is_none() {
                    self.modal_projection.clear_transport();
                }
                match target {
                    Some(ControlTarget::Close(token, confirm)) => {
                        port.click_workspace_close(token, confirm)
                    }
                    Some(ControlTarget::TabClose(token, confirm)) => {
                        port.click_tab_close(token, confirm)
                    }
                    Some(ControlTarget::PanelClose(token, confirm)) => {
                        port.click_panel_close(token, confirm)
                    }
                    _ => {}
                }
                Some(true)
            }
        }
    }

    /// Returns true on an owned release, whose entire remaining packet must be
    /// captured even if current state refuses its stale painted authority.
    pub(crate) fn modal_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        if self.queue_list_scroll(event) {
            return false;
        }
        if super::plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        self.observe_modal_motion(event, accepts_modal);
        let admit_new = self.can_admit_modal_press();
        let result = control_event(
            self.map.as_ref(),
            self.capture,
            admit_new,
            &mut self.modal_press,
            event,
            accepts_modal,
        );
        let ControlEvent::Released(target) = result else {
            return false;
        };
        if target.is_none() {
            self.modal_projection.clear_transport();
        }
        match target {
            Some(ControlTarget::Rename(token, submit)) => {
                if submit {
                    port.submit_rename_draft(token);
                } else {
                    port.cancel_rename_draft(token);
                }
            }
            Some(ControlTarget::Setup(token, control)) => match control {
                ModalControl::NextField => {
                    port.edit_setup_draft(
                        token,
                        crate::terminal_session::NativeSetupEdit::NextField,
                    );
                }
                ModalControl::Choice(index) => port.choose_setup_project(token, index),
                ModalControl::Submit => {
                    port.submit_setup_draft(token);
                }
                ModalControl::Cancel => port.cancel_setup_draft(token),
                ModalControl::Surface | ModalControl::Refresh => {}
            },
            #[cfg(windows)]
            Some(ControlTarget::Worktree(token, control)) => match control {
                ModalControl::NextField => {
                    port.edit_worktree(token, crate::terminal_session::WorktreeEdit::NextField);
                }
                ModalControl::Refresh => {
                    port.edit_worktree(token, crate::terminal_session::WorktreeEdit::ReadStatus);
                }
                ModalControl::Choice(index) => port.choose_worktree(token, index),
                ModalControl::Submit => port.submit_worktree(token),
                ModalControl::Cancel => port.dismiss_worktree(token),
                ModalControl::Surface => {}
            },
            _ => {}
        }
        true
    }
}
