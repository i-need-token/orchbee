//! Failed launch guidance wraps independently of terminal bytes and retains effective action hints.
use crate::{
    client_view::ClientView,
    localization::Locale,
    tui_theme::{Role, Theme},
    workbench_client::NativeLaunchFailure,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Wrap},
};

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    failure: NativeLaunchFailure<'_>,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
) {
    let mut lines = Vec::new();
    {
        use crate::client_view::PrefixDesc;
        let binding = |desc| view.bindings().labels(desc).join(" / ");
        let settings = binding(PrefixDesc::Settings);
        let copy = binding(PrefixDesc::CopyMode);
        lines.extend(
            locale
                .launch_failure_actions(&settings, &copy)
                .lines()
                .map(|line| Line::styled(line.to_owned(), theme.style(Role::AccentFocus))),
        );
    }

    lines.extend(
        locale
            .terminal_launch_failure(failure)
            .lines()
            .map(|line| Line::styled(line.to_owned(), theme.style(Role::TextPrimary))),
    );
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}
