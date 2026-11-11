//! Test-only presentation of the Settings page, as the native painter reports it.

use super::{Editor, SettingsDraw, State, Token};

/// List lines a test frame shows; enough for every category's rows.
const TEST_LINES: usize = 40;

impl Editor {
    /// Presents the page as a two-column frame would.
    pub(crate) fn draw(&mut self, token: Token) {
        self.draw_with(token, false);
    }

    /// Presents the page as a frame of the given width tier would.
    pub(crate) fn draw_with(&mut self, token: Token, narrow: bool) {
        let draw = match &self.state {
            State::Open(page) => page.presentation(TEST_LINES, narrow),
            _ => SettingsDraw::surface(token),
        };
        self.presented(draw);
    }
}
