use super::*;
use crate::workbench_client::TerminalStatus;
use orchbee_workbench_client::test_support::workbench_client::{
    RenameServer, TestResult, fixture_geometry, native_client, native_workbench_client,
    spin_with_draw,
};
use orchbee_workbench_protocol::WorkbenchAction;
use std::time::Instant;

fn run_native_rename_resize(
    session: &mut crate::workbench_client::WorkbenchClient,
    server: &RenameServer,
) -> io::Result<Vec<bool>> {
    struct TimedInput<'a> {
        now: Instant,
        inputs: VecDeque<(u64, Option<Vec<u8>>)>,
        origin: Instant,
        server: &'a RenameServer,
        quit_deadline: Option<Instant>,
    }
    impl InputSource for TimedInput<'_> {
        fn next_input(&mut self, _timeout: Duration) -> io::Result<Option<Vec<u8>>> {
            // Scripted time passes instantly, but Exit stops the worker and discards an unsent
            // command. Idle ticks keep the loop serving the worker until the Server observes
            // the Rename; only then is the final quit delivered.
            if self.inputs.len() == 1 && self.server.commands.lock().unwrap().is_empty() {
                let deadline = *self
                    .quit_deadline
                    .get_or_insert_with(|| Instant::now() + Duration::from_secs(10));
                if Instant::now() >= deadline {
                    return Err(io::Error::other("Rename was not submitted before quit"));
                }
                std::thread::sleep(Duration::from_millis(1));
                return Ok(None);
            }
            let (milliseconds, bytes) = self
                .inputs
                .pop_front()
                .ok_or_else(|| io::Error::other("Rename loop did not exit"))?;
            if self.inputs.is_empty() {
                // Quitting may leave the Rename result unread; the Server accepts that close.
                self.server
                    .teardown
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            self.now = self.origin + Duration::from_millis(milliseconds);
            Ok(bytes)
        }
        fn sample_time(&self) -> Instant {
            self.now
        }
    }
    /// Replays the scripted host sizes, then stays at the last one for any idle tick.
    struct SettledViewport {
        script: std::cell::RefCell<VecDeque<WindowDimensions>>,
        settled: WindowDimensions,
    }
    impl ViewportSource for SettledViewport {
        fn window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
            Ok(Some(
                self.script.borrow_mut().pop_front().unwrap_or(self.settled),
            ))
        }
    }
    let initial = dimensions(100, 24);
    let resized = dimensions(70, 24);
    let wide = super::super::terminal_geometry(initial, &ClientView::new(100))?
        .expect("a presentable viewport must produce pane geometry");
    let narrow = super::super::terminal_geometry(resized, &ClientView::new(70))?
        .expect("a presentable viewport must produce pane geometry");
    assert_eq!(wide.columns(), narrow.columns());
    let mut terminal = test_terminal(initial);
    let origin = Instant::now();
    let mut inputs = TimedInput {
        now: origin,
        origin,
        server,
        quit_deadline: None,
        inputs: VecDeque::from([
            (0, Some(b"\x02,".to_vec())),
            (1, Some(b"\x15renamed".to_vec())),
            (2, Some(b"\r".to_vec())),
            (52, None),
            (53, Some(b"\r".to_vec())),
            (103, None),
            (153, Some(b"\x02q".to_vec())),
        ]),
    };
    let viewport = SettledViewport {
        script: std::cell::RefCell::new(VecDeque::from([
            initial, initial, initial, resized, initial, initial, initial, initial,
        ])),
        settled: initial,
    };
    let mut leases = Vec::new();
    match super::super::run_event_loop_with_modes(
        &mut terminal,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &crate::tui_theme::Theme::terminal_native(),
        },
        (&mut inputs, &mut |request| {
            if let ModeRequest::BracketedPaste(enabled) = request {
                leases.push(enabled);
            }
            Ok(())
        }),
        session,
        &FixedTermination(false),
        &viewport,
        initial,
        true,
        crate::client_view::ClientView::new(initial.columns),
    ) {
        Ok(LoopExit::User) => Ok(leases),
        Ok(other) => Err(io::Error::other(format!(
            "unexpected Rename loop exit {other:?}"
        ))),
        Err(LoopFailure::Input(error) | LoopFailure::Viewport(error)) => Err(error),
        Err(LoopFailure::Terminal(error)) => match error {},
    }
}

#[test]
fn actual_loop_outer_resize_blocks_hidden_enter_then_restores_the_draft() -> TestResult {
    let geometry = fixture_geometry()?;
    let server = RenameServer::start(geometry)?;
    let mut workbench_client = native_workbench_client(native_client(&server.server)?, geometry)?;
    assert!(spin_with_draw(
        &mut workbench_client,
        |workbench_client| workbench_client.status() == TerminalStatus::Controlling
    ));
    let leases = run_native_rename_resize(&mut workbench_client, &server)?;
    assert_eq!(leases.iter().filter(|enabled| **enabled).count(), 1);
    assert_eq!(leases.last(), Some(&false));
    let commands = server.commands.lock().unwrap();
    assert!(
        matches!(&commands[..], [command] if matches!(&command.action, WorkbenchAction::TabRename { label, .. } if label.as_str() == "renamed"))
    );
    drop(commands);
    assert!(server.inputs.lock().unwrap().is_empty());
    assert!(workbench_client.rename_input_context().is_none());
    server.finish(workbench_client)?;
    Ok(())
}
