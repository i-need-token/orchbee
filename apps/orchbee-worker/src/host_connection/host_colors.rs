//! Default colors for terminals launched in Host display mode. The most recent controlling
//! Client's observed outer-terminal colors win; each following terminal keeps its last
//! committed theme so the next update can compare-and-set against the Host revision, and the
//! theme slots captured at its launch so a recolor takes the cursor and ANSI colors of the same
//! selection.
use orchbee_local_runtime::{UserColorThemes, UserTerminalColors};
use orchbee_terminal_protocol::{TerminalTheme, TerminalThemeState};
use std::collections::BTreeMap;
const MAXIMUM_FOLLOWERS: usize = crate::terminal_authorization::MAXIMUM_RETAINED_TERMINAL_BINDINGS;
#[derive(Default)]
pub(crate) struct HostColors {
    latest: Option<UserTerminalColors>,
    followers: BTreeMap<[u8; 16], (TerminalThemeState, UserColorThemes)>,
}
impl HostColors {
    pub(crate) const fn latest(&self) -> Option<UserTerminalColors> {
        self.latest
    }
    pub(crate) fn observe(&mut self, colors: UserTerminalColors) {
        self.latest = Some(colors);
    }
    /// A full table drops the new follower; it keeps its launch colors.
    pub(crate) fn follow(
        &mut self,
        terminal: [u8; 16],
        terminal_theme: TerminalTheme,
        themes: UserColorThemes,
    ) {
        if self.followers.len() < MAXIMUM_FOLLOWERS || self.followers.contains_key(&terminal) {
            self.followers.insert(
                terminal,
                (
                    TerminalThemeState {
                        operation_id: None,
                        revision: 0,
                        terminal: terminal_theme,
                    },
                    themes,
                ),
            );
        }
    }
    pub(crate) fn follower(
        &self,
        terminal: [u8; 16],
    ) -> Option<&(TerminalThemeState, UserColorThemes)> {
        self.followers.get(&terminal)
    }
    pub(crate) fn commit(&mut self, terminal: [u8; 16], state: TerminalThemeState) {
        if let Some((current, _themes)) = self.followers.get_mut(&terminal) {
            *current = state;
        }
    }
    pub(crate) fn remove(&mut self, terminal: [u8; 16]) {
        self.followers.remove(&terminal);
    }
}
