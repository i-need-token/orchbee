use super::*;
use std::time::Instant;

pub(crate) fn run_native_rename_resize(
    session: &mut crate::terminal_session::TerminalSession,
) -> io::Result<Vec<bool>> {
    struct TimedInput {
        now: Instant,
        inputs: VecDeque<(u64, Option<Vec<u8>>)>,
        origin: Instant,
    }
    impl InputSource for TimedInput {
        fn next_input(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            let (milliseconds, bytes) = self
                .inputs
                .pop_front()
                .ok_or_else(|| io::Error::other("Rename loop did not exit"))?;
            self.now = self.origin + Duration::from_millis(milliseconds);
            Ok(bytes)
        }
        fn sample_time(&self) -> Instant {
            self.now
        }
    }
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let wide = super::super::terminal_geometry(initial, &ClientView::new(100), None)?
        .expect("a presentable viewport must produce pane geometry");
    let narrow = super::super::terminal_geometry(resized, &ClientView::new(70), None)?
        .expect("a presentable viewport must produce pane geometry");
    assert_eq!(wide.columns(), narrow.columns());
    let mut terminal = test_terminal(initial);
    let origin = Instant::now();
    let mut inputs = TimedInput {
        now: origin,
        origin,
        inputs: VecDeque::from([
            (0, Some(b"\x02,\x15renamed".to_vec())),
            (1, Some(b"\r".to_vec())),
            (2, Some(b"\x1b".to_vec())),
            (52, None),
            (102, Some(b"\x02q".to_vec())),
        ]),
    };
    let viewport = QueueViewportSource {
        dimensions: std::cell::RefCell::new(VecDeque::from([
            Ok(Some(initial)),
            Ok(Some(initial)),
            Ok(Some(resized)),
            Ok(Some(resized)),
            Ok(Some(resized)),
            Ok(Some(resized)),
        ])),
    };
    let mut leases = Vec::new();
    match super::super::run_event_loop_with_modes(
        &mut terminal,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        (&mut inputs, &mut |enabled| {
            leases.push(enabled);
            Ok(())
        }),
        session,
        &FixedTermination(false),
        &viewport,
        initial,
    ) {
        Ok(LoopExit::User) => Ok(leases),
        Ok(other) => Err(io::Error::other(format!(
            "unexpected Rename loop exit {other:?}"
        ))),
        Err(LoopFailure::Input(error) | LoopFailure::Viewport(error)) => Err(error),
        Err(LoopFailure::Terminal(error)) => match error {},
    }
}
