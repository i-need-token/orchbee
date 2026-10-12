//! Painted modal controls retain their press authority until matching release.
//! The state owner then validates that exact token before any shared effect.
use super::{
    FrameHitMap, HitResolution, HitTarget, ModalControl, Modifiers, MouseAction, MouseButton,
    MouseEvent, PointerState, TerminalInputPort,
};

#[derive(Clone, Eq, PartialEq)]
enum ControlTarget {
    #[cfg(windows)]
    Copy(
        crate::terminal_copy_mode::CopyModeToken,
        crate::terminal_copy_mode::CopyControl,
    ),
    #[cfg(windows)]
    Resize(super::ResizeControl),
    Board(
        crate::terminal_session::AgentBoardToken,
        crate::terminal_session::AgentBoardControl,
    ),
    #[cfg(windows)]
    MenuRow(crate::action_menu::MenuToken, usize),
    #[cfg(windows)]
    MenuControl(crate::action_menu::MenuToken, bool),
    Settings(
        crate::native_settings::Token,
        crate::native_settings::Control,
    ),
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
            HitTarget::AgentBoardControl { token, control } => Self::Board(token.clone(), *control),
            #[cfg(windows)]
            HitTarget::ActionMenuRow { token, row } => Self::MenuRow(*token, *row),
            #[cfg(windows)]
            HitTarget::ActionMenuControl { token, back } => Self::MenuControl(*token, *back),
            HitTarget::SettingsRow { token, row } => {
                Self::Settings(*token, crate::native_settings::Control::Row(*row))
            }
            HitTarget::SettingsControl { token, control } => Self::Settings(*token, *control),
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
}

pub(super) struct PaintedPress {
    target: ControlTarget,
    area: ratatui::layout::Rect,
    cancelled: bool,
}

impl PaintedPress {
    pub(super) fn active(&self) -> bool {
        !self.cancelled
    }
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub(super) fn exposed_in(&self, map: &FrameHitMap) -> bool {
        map.regions.iter().any(|region| {
            region.rect == self.area
                && ControlTarget::from_hit(&region.target) == Some(self.target.clone())
        }) && matches!(map.hit(self.area.x, self.area.y), HitResolution::Region(target) if ControlTarget::from_hit(target) == Some(self.target.clone()))
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
                        && matches!(map.hit(event.column, event.row), HitResolution::Region(target) if ControlTarget::from_hit(target) == Some(pending.target.clone()))
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
    #[cfg(windows)]
    pub(super) fn copy_control_event(
        &mut self,
        view: &mut crate::client_view::ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        let ControlEvent::Released(target) = control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| matches!(target, ControlTarget::Copy(..)),
        ) else {
            return false;
        };
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
        let ControlEvent::Released(target) = control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| matches!(target, ControlTarget::Resize(_)),
        ) else {
            return false;
        };
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
    ) -> bool {
        if self.queue_list_scroll(event) {
            return false;
        }
        if super::plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        if let ControlEvent::Released(target) = control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| matches!(target, ControlTarget::Board(..)),
        ) {
            if let Some(ControlTarget::Board(token, control)) = target {
                port.click_agent_board_control(&token, control);
            }
            return true;
        }
        if !self.capture || event.mods != Modifiers::NONE || event.action != MouseAction::Press {
            return false;
        }
        let Some(map) = &self.map else {
            return false;
        };
        match map.hit(event.column, event.row) {
            HitResolution::Region(HitTarget::AgentBoardRow(session))
                if event.button == MouseButton::Left =>
            {
                if port.agent_board_select(session) {
                    self.stale_map();
                }
            }
            HitResolution::OverlayBackdrop
                if matches!(
                    event.button,
                    MouseButton::Left | MouseButton::Right | MouseButton::Middle
                ) =>
            {
                port.agent_board_dismiss();
                return true;
            }
            _ => {}
        }
        false
    }

    #[cfg(windows)]
    pub(super) fn menu_control_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        match control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| {
                matches!(
                    target,
                    ControlTarget::MenuRow(..) | ControlTarget::MenuControl(..)
                )
            },
        ) {
            ControlEvent::Released(target) => {
                match target {
                    Some(ControlTarget::MenuRow(token, row)) => port.click_action_menu(token, row),
                    Some(ControlTarget::MenuControl(token, back)) => {
                        port.click_action_menu_control(token, back)
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }
    /// Settings may save shared preferences; all painted rows and buttons commit on release.
    pub(super) fn settings_control_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> bool {
        match control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| matches!(target, ControlTarget::Settings(..)),
        ) {
            ControlEvent::Released(target) => {
                match target {
                    Some(ControlTarget::Settings(
                        token,
                        crate::native_settings::Control::Row(row),
                    )) => port.click_settings(token, row),
                    Some(ControlTarget::Settings(token, control)) => {
                        port.click_settings_control(token, control)
                    }
                    _ => {}
                }
                true
            }
            _ => false,
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
    }

    /// Some consumes a close-control gesture; true also consumes the decision
    /// packet tail. Shared close effects wait for the matching release.
    pub(crate) fn confirmation_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> Option<bool> {
        match control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.close_press,
            event,
            |target| {
                matches!(
                    target,
                    ControlTarget::Close(..)
                        | ControlTarget::TabClose(..)
                        | ControlTarget::PanelClose(..)
                )
            },
        ) {
            ControlEvent::Ignored => None,
            ControlEvent::Captured => Some(false),
            ControlEvent::Released(target) => {
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
        let result = control_event(
            self.map.as_ref(),
            self.capture,
            &mut self.modal_press,
            event,
            |target| match target {
                ControlTarget::Rename(..) => true,
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
