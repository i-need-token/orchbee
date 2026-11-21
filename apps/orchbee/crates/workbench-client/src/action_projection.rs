//! Shared semantic metadata for native action discovery.
//!
//! Execution owners keep the typed `ClientAction` / `MenuAction` input and exact target. This
//! module declares what menus and bindings must agree on for each action: its title, its stable
//! identity and its discovery metadata. Availability is supplied by the owning UI snapshot; it is
//! never inferred here.

use crate::localization::Words;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionIdentity<'a> {
    BuiltIn(&'a str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionCategory {
    Application,
    Client,
    Workspace,
    Project,
    Tab,
    Pane,
    Navigation,
    Agent,
    Input,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActionMetadata<'a> {
    pub(crate) identity: ActionIdentity<'a>,
    pub(crate) category: ActionCategory,
    pub(crate) search_aliases: &'static [Words],
}

/// Intent tone is independent of reading hierarchy, availability and pointer state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionTone {
    Ordinary,
    Destructive,
}

impl ActionTone {
    pub(crate) const fn content_role(
        self,
        ordinary: crate::tui_theme::Role,
    ) -> crate::tui_theme::Role {
        match self {
            Self::Ordinary => ordinary,
            Self::Destructive => crate::tui_theme::Role::AccentDestructive,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ActionAvailability<R> {
    Enabled,
    Disabled(R),
    Hidden,
}

impl<R> ActionAvailability<R> {
    pub(crate) fn from_disabled_reason(reason: Option<R>) -> Self {
        reason.map_or(Self::Enabled, Self::Disabled)
    }

    pub(crate) const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub(crate) const fn is_hidden(&self) -> bool {
        matches!(self, Self::Hidden)
    }
}

mod menu;
mod scoped;

#[cfg(test)]
mod tests;
