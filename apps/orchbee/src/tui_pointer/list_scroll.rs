//! Bounded wheel work for painted lists. Tokens follow only this stream's own
//! local movement; a new owner, keyboard edit or replacement frame revokes it.
use super::{
    FrameHitMap, HitTarget, MouseAction, MouseButton, MouseEvent, PointerState, TerminalInputPort,
    contains,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    Setup(crate::terminal_session::NativeSetupToken),
    #[cfg(windows)]
    Worktree(crate::terminal_session::WorktreeToken),
    #[cfg(windows)]
    Copy(crate::terminal_copy_mode::CopyModeToken),
    AgentBoard(crate::terminal_session::AgentBoardToken),
    Help {
        page: usize,
        last: usize,
    },
    Settings(crate::native_settings::Token),
    #[cfg(windows)]
    Menu(crate::action_menu::MenuToken),
    #[cfg(windows)]
    Navigator(crate::client_view::navigator::Row),
}

pub(super) struct Pending {
    target: Target,
    area: ratatui::layout::Rect,
    up: bool,
    lines: usize,
}

impl Pending {
    pub(super) fn exposed_in(&self, map: &FrameHitMap) -> bool {
        map.regions.iter().any(|region| {
            region.rect == self.area && region.target == HitTarget::ListScroll(self.target.clone())
        })
    }
}

impl PointerState {
    pub(crate) fn cancel_list_scroll(&mut self) {
        self.list_scroll = None;
    }

    /// Rows and their blank list cells share the same painted scroll rectangle.
    pub(crate) fn queue_list_scroll(&mut self, event: MouseEvent) -> bool {
        if !self.capture
            || event.mods != crate::client_view::Modifiers::NONE
            || event.action != MouseAction::Press
            || !matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
        {
            return false;
        }
        let Some(region) = self.map.as_ref().and_then(|map| {
            map.regions.iter().find(|region| {
                matches!(region.target, HitTarget::ListScroll(_))
                    && contains(region.rect, event.column, event.row)
            })
        }) else {
            return false;
        };
        let HitTarget::ListScroll(target) = &region.target else {
            return false;
        };
        #[cfg(windows)]
        if matches!(target, Target::Copy(_))
            && !self.map.as_ref().is_some_and(|map| {
                matches!(
                    map.hit(event.column, event.row),
                    super::HitResolution::Region(HitTarget::CopySurface { .. })
                )
            })
        {
            return false;
        }
        let up = event.button == MouseButton::WheelUp;
        let lines = self
            .list_scroll
            .as_ref()
            .filter(|pending| {
                pending.target == *target && pending.area == region.rect && pending.up == up
            })
            .map_or(0, |pending| pending.lines);
        // Help is a paginated document: one notch requests one page. Lists use lines.
        let amount = if matches!(target, Target::Help { .. }) {
            1
        } else {
            3
        };
        self.list_scroll = Some(Pending {
            target: target.clone(),
            area: region.rect,
            up,
            lines: lines.saturating_add(amount).min(65_535),
        });
        self.cancel_modal_press();
        true
    }

    pub(crate) fn flush_list_scroll(
        &mut self,
        view: &mut crate::client_view::ClientView,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) -> bool {
        let Some(mut pending) = self.list_scroll.take() else {
            return false;
        };
        if !self.capture {
            return false;
        }
        let Some(map) = &self.map else {
            self.list_scroll = Some(pending);
            return false;
        };
        if !pending.exposed_in(map) {
            return false;
        }
        let capacity = if matches!(pending.target, Target::Help { .. }) {
            1
        } else {
            usize::from(pending.area.height)
        };
        let lines = pending.lines.min(capacity);
        #[cfg(windows)]
        let lines = if matches!(pending.target, Target::Worktree(_)) {
            pending.lines.min(capacity / 2)
        } else {
            lines
        };
        #[cfg(windows)]
        let previous_focus = view.navigator.focus.clone();
        let next = match &pending.target {
            Target::Setup(token) => port
                .scroll_setup_projects(*token, pending.up, lines)
                .map(Target::Setup),
            #[cfg(windows)]
            Target::Worktree(token) => port
                .scroll_worktree(*token, pending.up, lines)
                .map(Target::Worktree),
            #[cfg(windows)]
            Target::Copy(token) => port
                .scroll_copy_mode(*token, pending.up, lines)
                .map(Target::Copy),
            Target::AgentBoard(token) => port
                .scroll_agent_board(token, pending.up, lines)
                .map(Target::AgentBoard),
            Target::Help { page, last }
                if view.overlay() == Some(crate::client_view::Overlay::ShortcutHelp)
                    && view.help_page() == *page =>
            {
                view.settle_help_page(if pending.up {
                    page.saturating_sub(1)
                } else {
                    page.saturating_add(1).min(*last)
                });
                Some(Target::Help {
                    page: view.help_page(),
                    last: *last,
                })
            }
            Target::Help { .. } => None,
            Target::Settings(token) => port
                .scroll_settings(*token, pending.up, lines)
                .map(Target::Settings),
            #[cfg(windows)]
            Target::Menu(token) => port
                .scroll_action_menu(*token, pending.up, lines)
                .map(Target::Menu),
            #[cfg(windows)]
            Target::Navigator(row) => view
                .navigator
                .scroll(row, pending.up, lines)
                .map(Target::Navigator),
        };
        let Some(next) = next else {
            return false;
        };
        let changed = next != pending.target;
        #[cfg(windows)]
        let changed = changed
            || matches!(pending.target, Target::Navigator(_))
                && previous_focus != view.navigator.focus;
        if !changed {
            return false;
        }
        pending.target = next;
        pending.lines -= lines;
        if pending.lines > 0 {
            self.list_scroll = Some(pending);
        }
        self.stale_map();
        true
    }
}
