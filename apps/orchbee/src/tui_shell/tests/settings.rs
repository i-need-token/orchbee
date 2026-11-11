//! Exercises Settings through the real whole-frame drawing boundary.
use super::*;
use crate::workbench_client::{DrawReceipt, WorkbenchClientView};
use orchbee_workbench_client::test_support::workbench_client::{
    self as test_support, SettingsFrameSource,
};

fn settings_frame(
    size: (u16, u16),
    view: &mut ClientView,
    locale: Locale,
    session: &dyn SettingsFrameSource,
) -> (ratatui::buffer::Buffer, DrawReceipt) {
    struct Surface<'a> {
        session: &'a dyn SettingsFrameSource,
        receipt: DrawReceipt,
    }
    impl TerminalInputPort for Surface<'_> {
        fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("drawing Settings cannot forward input");
        }
    }
    impl super::super::TerminalSurface for Surface<'_> {
        fn view(&self) -> Option<WorkbenchClientView<'_>> {
            Some(self.session.settings_view())
        }
        fn after_close_draw(&mut self, receipt: DrawReceipt) {
            self.receipt = receipt;
        }
    }
    let mut terminal = test_terminal(dimensions(size.0, size.1));
    let mut surface = Surface {
        session,
        receipt: DrawReceipt::default(),
    };
    super::super::draw_surface(
        &mut terminal,
        view,
        super::super::Chrome {
            locale,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        &mut surface,
        &mut crate::terminal_input::InputDispatcher::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    (terminal.backend().buffer().clone(), surface.receipt)
}

#[test]
fn a_candidate_previews_on_the_whole_page_without_runtime_effects() {
    test_support::a_candidate_previews_on_the_whole_page_without_runtime_effects(&settings_frame);
}

#[test]
fn a_change_made_elsewhere_shows_on_the_open_page_at_once() {
    test_support::a_change_made_elsewhere_shows_on_the_open_page_at_once(&settings_frame);
}

#[test]
fn covering_client_owner_uses_confirmed_appearance_until_settings_returns() {
    test_support::covering_client_owner_uses_confirmed_appearance_until_settings_returns(
        &settings_frame,
    );
}
