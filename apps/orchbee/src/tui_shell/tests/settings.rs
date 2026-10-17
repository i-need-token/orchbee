//! Exercises Settings through the real whole-frame drawing boundary.
use super::*;
use crate::terminal_session::{NativeDrawReceipt, TerminalSessionView};

pub(crate) fn settings_frame<'a>(
    size: (u16, u16),
    view: &mut ClientView,
    locale: Locale,
    session: impl Fn() -> TerminalSessionView<'a>,
) -> (ratatui::buffer::Buffer, NativeDrawReceipt) {
    struct Surface<F> {
        session: F,
        receipt: NativeDrawReceipt,
    }
    impl<F> TerminalInputPort for Surface<F> {
        fn forward(&mut self, _bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            panic!("drawing Settings cannot forward input");
        }
    }
    impl<'a, F: Fn() -> TerminalSessionView<'a>> super::super::TerminalSurface for Surface<F> {
        fn view(&self) -> Option<TerminalSessionView<'_>> {
            Some((self.session)())
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
