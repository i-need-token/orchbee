//! Board row names follow the Tab strip's naming rule, so a row and the Tab it jumps to agree.
use super::PresentationPlacement;
use crate::localization::Locale;
use crate::terminal_title::{ChromeName, Program};
use crate::workbench_projection::ClientTerminalBinding;
use orchbee_workbench_protocol::TerminalPresentationId;
use std::borrow::Cow;

/// Snapshot facts that name one Presentation by the Tab strip's rule: its custom label, then the
/// title the Client view shows for its terminal, then the program the Server committed with its
/// binding or its launch failure.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct NameFacts {
    pub(crate) custom_label: Option<String>,
    pub(crate) launch_program: Option<String>,
    pub(crate) presentation: Option<TerminalPresentationId>,
    pub(crate) binding: Option<ClientTerminalBinding>,
}

impl NameFacts {
    pub(crate) fn of(member: &crate::workbench_projection::ClientPresentation) -> Self {
        Self {
            custom_label: member.custom_label().map(str::to_owned),
            launch_program: member
                .launch_program()
                .or_else(|| member.launch_failure()?.executable.as_deref())
                .map(str::to_owned),
            presentation: Some(member.presentation_id().clone()),
            binding: member.binding().cloned(),
        }
    }

    /// The name these facts give, with `live` supplying the terminal's title when a Client view
    /// is present.
    pub(crate) fn chrome<'s>(
        &'s self,
        live: Option<&'s dyn LivePlacementNames>,
    ) -> Option<ChromeName<'s>> {
        self.custom_label
            .as_deref()
            .map(ChromeName::Text)
            .or_else(|| live?.terminal_title(self.presentation.as_ref()?, self.binding.as_ref()))
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

    /// The name the title of a Presentation's terminal gives it, in any Workspace, by the rule
    /// the strip uses; `None` when the Presentation is named by its program.
    fn terminal_title(
        &self,
        presentation: &TerminalPresentationId,
        binding: Option<&ClientTerminalBinding>,
    ) -> Option<ChromeName<'_>>;
}

impl PresentationPlacement {
    /// The row title and Tab name. A live Tab uses the strip's own names; any other Tab applies
    /// the same rule to its snapshot facts and the titles the view shows for its terminals. A
    /// Presentation without a name of its own takes its Tab's name.
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
            Some(facts) => facts.chrome(live).map_or_else(
                || locale.automatic_tab_label(self.tab_position).into(),
                |name| locale.chrome_name(name),
            ),
        };
        let title = self
            .presentation_name
            .chrome(live)
            .map_or_else(|| tab.clone(), |name| locale.chrome_name(name));
        (title.into_owned(), tab.into_owned())
    }
}
