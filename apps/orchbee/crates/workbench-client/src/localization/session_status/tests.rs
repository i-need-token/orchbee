use super::Locale;

#[test]
fn native_prebootstrap_starting_remains_connecting_without_a_selected_tab() {
    use crate::localization::TerminalStatus;

    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert_eq!(
            locale.native_starting_status(None),
            locale.terminal_status(TerminalStatus::Connecting)
        );
        assert_eq!(
            locale.native_switching_status(None),
            locale.terminal_status(TerminalStatus::Unavailable)
        );
    }
}

#[test]
fn native_prebootstrap_retirement_overrides_the_synthetic_starting_phase() {
    use crate::localization::{DisplayPhase, TerminalStatus};
    use crate::workbench_client::DisplayRetirement;

    for locale in [Locale::EnUs, Locale::ZhCn] {
        let unavailable = locale.terminal_status(TerminalStatus::Unavailable);
        let outcome_unknown = locale.terminal_status(TerminalStatus::OutcomeUnknown);
        let status = |retirement| {
            locale.native_phase_status_values(
                DisplayPhase::Starting,
                None,
                None,
                None,
                Some(retirement),
            )
        };
        assert_eq!(status(DisplayRetirement::Unavailable), unavailable);
        assert_eq!(status(DisplayRetirement::OutcomeUnknown), outcome_unknown);
    }
}
