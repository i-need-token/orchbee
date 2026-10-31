use super::Locale;

#[test]
fn native_prebootstrap_starting_remains_connecting_without_a_selected_tab() {
    use crate::localization::{ClientProjectionStatus, TerminalStatus};

    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert_eq!(
            locale.native_starting_status(None, ClientProjectionStatus::Empty),
            locale.terminal_status(TerminalStatus::Connecting)
        );
        assert_eq!(
            locale.native_switching_status(None, ClientProjectionStatus::Empty),
            locale.terminal_status(TerminalStatus::Unavailable)
        );
    }
    assert_eq!(
        Locale::EnUs
            .native_starting_status(Some("Tab 1"), ClientProjectionStatus::WaitingForFullFrame),
        "Tab 1 · Connecting · waiting for full frame"
    );
}

#[test]
fn native_prebootstrap_retirement_overrides_the_synthetic_starting_phase() {
    use crate::localization::{ClientProjectionStatus, DisplayPhase};
    use crate::workbench_client::DisplayRetirement;

    for (locale, unavailable, outcome_unknown) in [
        (
            Locale::EnUs,
            "Terminal disconnected",
            "Terminal operation result unknown · no replay",
        ),
        (Locale::ZhCn, "终端已断连", "终端操作结果未知 · 不会重放"),
    ] {
        let status = |retirement| {
            locale.native_phase_status_values(
                DisplayPhase::Starting,
                None,
                None,
                None,
                ClientProjectionStatus::Empty,
                Some(retirement),
            )
        };
        assert_eq!(status(DisplayRetirement::Unavailable), unavailable);
        assert_eq!(status(DisplayRetirement::OutcomeUnknown), outcome_unknown);
    }
}
