//! The welcome card is gone. The binary's startup still loads this empty state into its first
//! view, so it stays until that call is removed.

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State;

impl State {
    pub fn load() -> Self {
        Self
    }
}
