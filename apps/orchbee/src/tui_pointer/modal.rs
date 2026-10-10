//! Painted modal controls retain their press authority until matching release.
//! The state owner then validates that exact token before any shared effect.
use super::{
    FrameHitMap, HitResolution, HitTarget, ModalControl, Modifiers, MouseAction, MouseButton,
    MouseEvent, PointerState, TerminalInputPort,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum ControlTarget {
    Setup(crate::terminal_session::NativeSetupToken, ModalControl),
    #[cfg(windows)]
    Worktree(crate::terminal_session::WorktreeToken, ModalControl),
    Close(crate::terminal_session::WorkspaceCloseToken, bool),
}

impl ControlTarget {
    fn from_hit(target: &HitTarget) -> Option<Self> {
        Some(match target {
            HitTarget::Setup { token, control } => Self::Setup(*token, *control),
            #[cfg(windows)]
            HitTarget::Worktree { token, control } => Self::Worktree(*token, *control),
            HitTarget::WorkspaceClose { token, confirm } => Self::Close(*token, *confirm),
            _ => return None,
        })
    }
}

pub(super) struct PaintedPress {
    target: ControlTarget,
    area: ratatui::layout::Rect,
    cancelled: bool,
}

impl PaintedPress {
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub(super) fn exposed_in(&self, map: &FrameHitMap) -> bool {
        map.regions.iter().any(|region| {
            region.rect == self.area && ControlTarget::from_hit(&region.target) == Some(self.target)
        }) && matches!(map.hit(self.area.x, self.area.y), HitResolution::Region(target) if ControlTarget::from_hit(target) == Some(self.target))
    }
}

enum ControlEvent {
    Ignored,
    Captured,
    Released(Option<ControlTarget>),
}

fn control_event(
    map: Option<&FrameHitMap>,
    capture: bool,
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
        if matches!(event.button, MouseButton::Left | MouseButton::None) {
            if event.action == MouseAction::Motion {
                return ControlEvent::Captured;
            }
            if event.action == MouseAction::Release {
                let Some(pending) = press.take() else {
                    return ControlEvent::Ignored;
                };
                let current = map.is_some_and(|map| {
                    pending.exposed_in(map)
                        && matches!(map.hit(event.column, event.row), HitResolution::Region(target) if ControlTarget::from_hit(target) == Some(pending.target))
                });
                let admitted = !pending.cancelled
                    && current
                    && super::contains(pending.area, event.column, event.row);
                return ControlEvent::Released(admitted.then_some(pending.target));
            }
        }
        if event.action == MouseAction::Press {
            *press = None;
        }
    }
    if event.action != MouseAction::Press
        || event.button != MouseButton::Left
        || event.mods != Modifiers::NONE
    {
        return ControlEvent::Ignored;
    }
    let Some(map) = map else {
        return ControlEvent::Ignored;
    };
    let HitResolution::Region(target) = map.hit(event.column, event.row) else {
        return ControlEvent::Ignored;
    };
    let Some(control) = ControlTarget::from_hit(target).filter(|control| accepts(control)) else {
        return ControlEvent::Ignored;
    };
    let Some(region) = map.regions.iter().find(|region| {
        &region.target == target && super::contains(region.rect, event.column, event.row)
    }) else {
        return ControlEvent::Ignored;
    };
    *press = Some(PaintedPress {
        target: control,
        area: region.rect,
        cancelled: false,
    });
    ControlEvent::Captured
}

impl PointerState {
    /// Revokes pending effects while retaining ownership of their late releases.
    pub(crate) fn cancel_modal_press(&mut self) {
        for press in [&mut self.close_press, &mut self.modal_press]
            .into_iter()
            .flatten()
        {
            press.cancel();
        }
    }

    /// Some consumes a close-control gesture; true also consumes the decision
    /// packet tail. Shared close effects wait for the matching release.
    pub(crate) fn workspace_close_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> Option<bool> {
        match control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.close_press,
            event,
            |target| matches!(target, ControlTarget::Close(..)),
        ) {
            ControlEvent::Ignored => None,
            ControlEvent::Captured => Some(false),
            ControlEvent::Released(target) => {
                if let Some(ControlTarget::Close(token, confirm)) = target {
                    port.click_workspace_close(token, confirm);
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
        if event.action == MouseAction::Press
            && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
        {
            self.cancel_modal_press();
            if self.capture
                && event.mods == Modifiers::NONE
                && let Some(map) = &self.map
                && let HitResolution::Region(target) = map.hit(event.column, event.row)
            {
                let up = event.button == MouseButton::WheelUp;
                match target {
                    HitTarget::Setup {
                        token,
                        control: ModalControl::Choice(_) | ModalControl::Surface,
                    } => port.scroll_setup_projects(*token, up),
                    #[cfg(windows)]
                    HitTarget::Worktree {
                        token,
                        control: ModalControl::Choice(_) | ModalControl::Surface,
                    } => port.scroll_worktree(*token, up),
                    _ => {}
                }
            }
            return false;
        }
        let result = control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| match target {
                ControlTarget::Setup(_, control) => *control != ModalControl::Surface,
                #[cfg(windows)]
                ControlTarget::Worktree(_, control) => *control != ModalControl::Surface,
                _ => false,
            },
        );
        let ControlEvent::Released(target) = result else {
            return false;
        };
        match target {
            Some(ControlTarget::Setup(token, control)) => match control {
                ModalControl::Choice(index) => port.choose_setup_project(token, index),
                ModalControl::Submit => {
                    port.submit_setup_draft(token);
                }
                ModalControl::Cancel => port.cancel_setup_draft(token),
                ModalControl::Surface => {}
            },
            #[cfg(windows)]
            Some(ControlTarget::Worktree(token, control)) => match control {
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
