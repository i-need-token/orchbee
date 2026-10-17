//! Exercises Settings through the real whole-frame drawing boundary.
use super::*;
use crate::terminal_session::{NativeDrawReceipt, TerminalSessionView};

pub(crate) trait SettingsFrameSource {
    fn settings_view(&self) -> TerminalSessionView<'_>;
}

pub(crate) fn settings_frame(
    size: (u16, u16),
    view: &mut ClientView,
    locale: Locale,
    session: &impl SettingsFrameSource,
) -> (ratatui::buffer::Buffer, NativeDrawReceipt) {
    struct Surface<'a, S> {
        session: &'a S,
        receipt: NativeDrawReceipt,
    }
    impl<S> TerminalInputPort for Surface<'_, S> {
        fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("drawing Settings cannot forward input");
        }
    }
    impl<S: SettingsFrameSource> super::super::TerminalSurface for Surface<'_, S> {
        fn view(&self) -> Option<TerminalSessionView<'_>> {
            Some(self.session.settings_view())
        }
        fn after_close_draw(&mut self, receipt: NativeDrawReceipt) {
            self.receipt = receipt;
        }
    }
    let mut terminal = test_terminal(dimensions(size.0, size.1));
    let mut surface = Surface {
        session,
        receipt: NativeDrawReceipt::default(),
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
