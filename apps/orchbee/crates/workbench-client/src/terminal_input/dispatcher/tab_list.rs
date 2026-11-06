//! Keys reaching the Mobile tier's Tab list. The list covers the terminal, so keys move through
//! its rows and act through the Tab strip's own actions, and paste is dropped; the configured
//! prefix still opens the shell binding layer.

use std::time::Instant;

use super::{
    ClientView, DecodedItem, DecodedKind, DispatchOutcome, InputDispatcher, KeyKind, ModeRequest,
    TerminalInputPort,
};
use crate::client_view::{ClientAction, tab_list::Intent};
use crate::tui_hit_map::chrome::TabTarget;

impl InputDispatcher {
    /// Whether this item belongs to the open Tab list rather than to the terminal it covers.
    pub(super) fn tab_list_owns(view: &ClientView, item: &DecodedItem) -> bool {
        view.tab_list_open()
            && matches!(
                item.kind,
                DecodedKind::Key(_) | DecodedKind::Paste { .. } | DecodedKind::RejectedPaste
            )
    }

    pub(super) fn tab_list_item<Port: TerminalInputPort + ?Sized, Error>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        item: DecodedItem,
        after_layout: &mut impl FnMut(&mut ClientView, &mut Port) -> Result<bool, Error>,
        mode: &mut impl FnMut(ModeRequest) -> Result<(), Error>,
    ) -> Result<DispatchOutcome, Error> {
        self.flush_pending(view, port);
        let DecodedKind::Key(key) = item.kind else {
            return Ok(DispatchOutcome::Continue);
        };
        if view.prefix_active() || view.bindings().prefix.matches(key) {
            if key.kind == KeyKind::Release {
                return Ok(DispatchOutcome::Continue);
            }
            return self.decide(view, port, now, item, after_layout, mode);
        }
        let painted = self.pointer.tab_list_paint().cloned();
        let Some(intent) = view
            .tab_list_mut()
            .map(|list| list.key(key, painted.as_ref()))
        else {
            return Ok(DispatchOutcome::Continue);
        };
        // A menu opens beside its row as the last frame painted it.
        let menu = match &intent {
            Intent::OpenMenu(target) => self.pointer.tab_list_menu(target),
            _ => None,
        };
        // Focus and window changes move the rows until the next frame paints them.
        self.pointer.invalidate_map_with_port(port);
        let action = match intent {
            Intent::None => None,
            Intent::Close => {
                view.close_tab_list();
                self.machine_packet = true;
                self.keys.capture_pending();
                after_layout(view, port)?;
                None
            }
            Intent::Activate(TabTarget::Tab(tab)) => {
                let index = port
                    .pointer_probe()
                    .tab_index(&tab)
                    .and_then(|index| u8::try_from(index + 1).ok());
                index.map(|index| {
                    view.close_tab_list();
                    self.machine_packet = true;
                    self.keys.capture_pending();
                    ClientAction::SelectTab(index)
                })
            }
            Intent::Activate(TabTarget::Group { group, .. }) => {
                Some(ClientAction::ToggleTabGroup(group))
            }
            Intent::Move { tab, previous } => Some(ClientAction::TabMenu {
                action: if previous {
                    crate::action_menu::MenuAction::MoveTabPrevious
                } else {
                    crate::action_menu::MenuAction::MoveTabNext
                },
                tab_id: Some(tab),
            }),
            Intent::NewTab => {
                self.machine_packet = true;
                self.keys.capture_pending();
                Some(ClientAction::CreateTab)
            }
            Intent::OpenMenu(_) => {
                if let Some((target, anchor)) = menu {
                    mode(ModeRequest::BracketedPaste(true))?;
                    if port.open_action_menu(target, Some(anchor), view.pointer_released()) {
                        self.menu_packet = true;
                        self.keys.capture_pending();
                    }
                }
                None
            }
        };
        if let Some(action) = action {
            let transition = view.decide_action(action);
            return self.apply_transition(view, port, now, transition, None, after_layout, mode);
        }
        self.reconcile(view, port, now);
        Ok(DispatchOutcome::Continue)
    }
}
