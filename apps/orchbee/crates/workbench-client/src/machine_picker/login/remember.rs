//! The overlay's remember box: whether it holds the keyboard and whether it is ticked. Tab moves
//! between the field and the box, Space toggles the box while it holds the keyboard, and a press
//! on the painted box toggles it. The episode learns the choice before the answer is sent.

use super::LoginOverlay;
use crate::client_view::{KeyCode, Modifiers};
use crate::machine_login::{Presented, RememberBox};

/// The box's state in one overlay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Choice {
    /// The box, not the field, holds the keyboard.
    focused: bool,
    ticked: bool,
}

impl Choice {
    /// Unticked unless the episode's earlier question was answered with the box ticked.
    pub(super) const fn new(presented: &Presented) -> Self {
        Self {
            focused: false,
            ticked: matches!(
                presented.remember,
                RememberBox::Offered { ticked: true, .. }
            ),
        }
    }
}

impl LoginOverlay {
    const fn offered(&self) -> bool {
        matches!(self.presented.remember, RememberBox::Offered { .. })
    }

    /// Whether the box is ticked, while the overlay offers it.
    pub(crate) const fn ticked(&self) -> Option<bool> {
        if self.offered() {
            Some(self.remember.ticked)
        } else {
            None
        }
    }

    /// Whether the box holds the keyboard.
    pub(crate) const fn remember_focused(&self) -> bool {
        self.offered() && self.remember.focused
    }

    pub(super) const fn toggle_remember(&mut self) {
        if self.offered() && self.editable() {
            self.remember.ticked = !self.remember.ticked;
        }
    }

    /// Takes Tab, and every printable key while the box holds the keyboard, Space toggling it;
    /// false for a key the field or the buttons take.
    pub(super) fn remember_key(&mut self, code: KeyCode, mods: Modifiers) -> bool {
        if !self.offered() || !self.editable() {
            return false;
        }
        // Tab and Shift+Tab arrive as their canonical Ctrl+I chords; with two stops both move.
        let tab = code == KeyCode::Char('i')
            && (mods == Modifiers::CTRL || mods == Modifiers::CTRL.with_shift());
        if tab {
            self.remember.focused = !self.remember.focused;
            return true;
        }
        if !self.remember.focused {
            return false;
        }
        match code {
            KeyCode::Char(' ') if mods == Modifiers::NONE => {
                self.toggle_remember();
                true
            }
            // Nothing typed while the box holds the keyboard reaches the field.
            KeyCode::Char(_) => !(mods == Modifiers::CTRL),
            KeyCode::Backspace => true,
            _ => false,
        }
    }
}
