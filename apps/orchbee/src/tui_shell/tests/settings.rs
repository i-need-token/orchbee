//! Exercises Settings through the real whole-frame drawing boundary.
use super::*;
use crate::workbench_client::{DrawReceipt, WorkbenchClientView};

pub(crate) trait SettingsFrameSource {
    fn settings_view(&self) -> WorkbenchClientView<'_>;
}

pub(crate) fn settings_frame(
    size: (u16, u16),
    view: &mut ClientView,
    locale: Locale,
    session: &impl SettingsFrameSource,
) -> (ratatui::buffer::Buffer, DrawReceipt) {
    struct Surface<'a, S> {
        session: &'a S,
        receipt: DrawReceipt,
    }
    impl<S> TerminalInputPort for Surface<'_, S> {
        fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("drawing Settings cannot forward input");
        }
    }
    impl<S: SettingsFrameSource> super::super::TerminalSurface for Surface<'_, S> {
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
