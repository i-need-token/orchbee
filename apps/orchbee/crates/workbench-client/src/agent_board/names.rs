//! Presentation names follow the Tab strip's naming rule, so a notification and the Tab it names
//! agree.
use crate::terminal_title::{ChromeName, Program};
use crate::workbench_projection::ClientTerminalBinding;
use orchbee_workbench_protocol::TerminalPresentationId;

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

/// The titles the Client view shows for its terminals, by the Tab strip's rule.
pub(crate) trait LivePlacementNames {
    /// The name the title of a Presentation's terminal gives it, in any Workspace, by the rule
    /// the strip uses; `None` when the Presentation is named by its program.
    fn terminal_title(
        &self,
        presentation: &TerminalPresentationId,
        binding: Option<&ClientTerminalBinding>,
    ) -> Option<ChromeName<'_>>;
}
