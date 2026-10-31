//! Board row names follow the Tab strip's naming rule, so a row and the Tab it jumps to agree.
use super::PresentationPlacement;
use crate::localization::Locale;
use crate::terminal_title::{ChromeName, Program};
use std::borrow::Cow;

/// Snapshot facts that name one Presentation when no program title is observed: its custom
/// label, then the program the Server committed with its binding or its launch failure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NameFacts {
    pub(crate) custom_label: Option<String>,
    pub(crate) launch_program: Option<String>,
}

impl NameFacts {
    pub(crate) fn of(member: &crate::workbench_projection::ClientPresentation) -> Self {
        Self {
            custom_label: member.custom_label().map(str::to_owned),
            launch_program: member
                .launch_program()
                .or_else(|| member.launch_failure()?.executable.as_deref())
                .map(str::to_owned),
        }
    }

    pub(crate) fn chrome(&self) -> Option<ChromeName<'_>> {
        self.custom_label
            .as_deref()
            .map(ChromeName::Text)
            .or_else(|| {
                Program::from_path(self.launch_program.as_deref()?).map(|program| {
                    ChromeName::Program {
                        program,
                        command: None,
                    }
                })
            })
    }
}

/// The Tab strip's live names for a placement while the Client view shows its Tab.
pub(crate) trait LivePlacementNames {
    /// The named Presentation's name and its Tab's name, or `None` when the view does not show
    /// that Tab.
    fn placement_names(
        &self,
        presentation_id: &str,
        placement: &PresentationPlacement,
        locale: Locale,
    ) -> Option<(String, String)>;
}

impl PresentationPlacement {
    /// The row title and Tab name. A live Tab uses the strip's own names, which include program
    /// titles; any other Tab applies the same rule to its snapshot facts. A Presentation without
    /// a name of its own takes its Tab's name.
    pub(crate) fn names(
        &self,
        presentation_id: &str,
        live: Option<&dyn LivePlacementNames>,
        locale: Locale,
    ) -> (String, String) {
        if let Some(names) =
            live.and_then(|live| live.placement_names(presentation_id, self, locale))
        {
            return names;
        }
        let tab = match &self.automatic_tab_name {
            None => Cow::Borrowed(self.tab_label.as_str()),
            Some(facts) => facts.chrome().map_or_else(
                || locale.automatic_tab_label(self.tab_position).into(),
                |name| locale.chrome_name(name),
            ),
        };
        let title = self
            .presentation_name
            .chrome()
            .map_or_else(|| tab.clone(), |name| locale.chrome_name(name));
        (title.into_owned(), tab.into_owned())
    }
}
