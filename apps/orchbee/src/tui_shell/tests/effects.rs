//! Output trace at the actual draw/flush boundary, without an inherited terminal.
use super::*;
use crate::tui_shell::InputDispatcher;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, PartialEq)]
enum Event {
    Bytes(Vec<u8>),
    Flush,
    AfterDraw,
    Confirmation(bool),
}

#[derive(Clone)]
struct Writer {
    events: Rc<RefCell<Vec<Event>>>,
    fail: Rc<RefCell<bool>>,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if *self.fail.borrow() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.events.borrow_mut().push(Event::Bytes(bytes.to_vec()));
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if *self.fail.borrow() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.events.borrow_mut().push(Event::Flush);
        Ok(())
    }
}
struct Surface {
    events: Rc<RefCell<Vec<Event>>>,
    pending: bool,
    receipts: Rc<Cell<usize>>,
}
impl TerminalInputPort for Surface {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        Ok(())
    }
    fn click_workspace_close(
        &mut self,
        _: crate::workbench_client::WorkspaceCloseToken,
        confirm: bool,
    ) {
        self.events.borrow_mut().push(Event::Confirmation(confirm));
    }
}

#[test]
fn partial_frame_retires_old_press_authority_without_receipt_effect_or_replay() {
    #[derive(Clone)]
    struct PartialWriter {
        events: Rc<RefCell<Vec<Event>>>,
        remaining: Rc<std::cell::Cell<usize>>,
    }
    impl Write for PartialWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len().min(self.remaining.get());
            if count == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.remaining.set(self.remaining.get() - count);
            self.events
                .borrow_mut()
                .push(Event::Bytes(bytes[..count].to_vec()));
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> {
            if self.remaining.get() == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            self.events.borrow_mut().push(Event::Flush);
            Ok(())
        }
    }

    let events = Rc::new(RefCell::new(Vec::new()));
    let remaining = Rc::new(std::cell::Cell::new(usize::MAX));
    let dimensions = dimensions(40, 12);
    let lease = Arc::new(crate::terminal_lifecycle::test_support::synchronized_update_lease());
    let backend = FixedViewportBackend::new(
        PartialWriter {
            events: events.clone(),
            remaining: remaining.clone(),
        },
        dimensions,
        lease.clone(),
    );
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .unwrap();
    let mut view = ClientView::new(dimensions.columns);
    let mut surface = Surface {
        events: events.clone(),
        pending: true,
        receipts: Rc::new(Cell::new(0)),
    };
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut view, &mut surface, true);
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    hits.start_modal();
    hits.record_modal_control(
        crate::tui_pointer::HitLayer::Overlay,
        ratatui::layout::Rect::new(10, 5, 8, 1),
        crate::tui_pointer::HitTarget::WorkspaceClose {
            token: crate::workbench_client::WorkspaceCloseToken::for_test(1),
            confirm: true,
        },
        crate::tui_pointer::InteractionAvailability::Enabled,
    );
    dispatcher.publish_hit_map(hits.finish());
    let now = std::time::Instant::now();
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[<0;13;6M",
            &mut surface,
            now,
            |_, _| Ok::<_, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    events.borrow_mut().clear();
    remaining.set(7);
    let theme = crate::tui_theme::Theme::terminal_native();
    let result = super::super::draw_surface(
        &mut terminal,
        &mut view,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &theme,
        },
        &mut surface,
        &mut dispatcher,
        &mut |_| Ok(()),
    );
    assert!(matches!(result, Err(LoopFailure::Terminal(_))));
    assert!(surface.pending);
    assert!(!events.borrow().contains(&Event::AfterDraw));
    let observed = events
        .borrow()
        .iter()
        .filter_map(|event| match event {
            Event::Bytes(bytes) => Some(bytes.len()),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(observed, 7, "the host received a partial candidate frame");
    // Probe the retired dispatcher without drawing or retrying any ambiguous
    // output: the release must consume the old gesture without committing it.
    dispatcher
        .dispatch(
            &mut view,
            b"\x1b[<0;13;6m",
            &mut surface,
            now,
            |_, _| Ok::<_, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert!(!events.borrow().contains(&Event::Confirmation(true)));
    assert_eq!(remaining.get(), 0);
    lease.block_frame_output();
}
impl super::super::TerminalSurface for Surface {
    fn after_close_draw(&mut self, _receipt: crate::workbench_client::DrawReceipt) {
        self.receipts.set(self.receipts.get() + 1);
    }
    fn after_draw(&mut self) {
        self.events.borrow_mut().push(Event::AfterDraw);
    }
    fn deliver_terminal_effect(
        &mut self,
        bell: &mut dyn FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        if std::mem::take(&mut self.pending) {
            bell()?;
        }
        Ok(())
    }
}

#[test]
fn bell_is_outside_the_flushed_frame_and_failed_drawing_dispatches_nothing() {
    for fail_draw in [false, true] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let fail = Rc::new(RefCell::new(false));
        let mut writer = Writer {
            events: events.clone(),
            fail: fail.clone(),
        };
        let dimensions = dimensions(40, 12);
        let lease = Arc::new(crate::terminal_lifecycle::test_support::synchronized_update_lease());
        let backend = FixedViewportBackend::new(writer.clone(), dimensions, lease.clone());
        let mut terminal = Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Fixed(to_area(dimensions)),
            },
        )
        .unwrap();
        events.borrow_mut().clear();
        *fail.borrow_mut() = fail_draw;
        let mut surface = Surface {
            events: events.clone(),
            pending: true,
            receipts: Rc::new(Cell::new(0)),
        };
        let mut dispatcher = crate::terminal_input::InputDispatcher::default();
        let mut view = ClientView::new(dimensions.columns);
        let theme = crate::tui_theme::Theme::terminal_native();
        let mut mode = |request| {
            assert_eq!(request, ModeRequest::Bell);
            assert!(!lease.frame_open());
            writer.write_all(b"\x07")?;
            writer.flush()
        };
        let result = super::super::draw_surface(
            &mut terminal,
            &mut view,
            super::super::Chrome {
                locale: Locale::EnUs,
                theme: &theme,
            },
            &mut surface,
            &mut dispatcher,
            &mut mode,
        );
        if fail_draw {
            assert!(matches!(result, Err(LoopFailure::Terminal(_))));
            assert!(surface.pending);
            assert!(!events.borrow().contains(&Event::AfterDraw));
            assert!(!events.borrow().contains(&Event::Bytes(b"\x07".to_vec())));
        } else {
            result.unwrap();
            let trace = events.borrow();
            let after = trace.iter().position(|e| *e == Event::AfterDraw).unwrap();
            assert_eq!(trace[after - 1], Event::Flush);
            assert_eq!(trace[after + 1], Event::Bytes(b"\x07".to_vec()));
            assert_eq!(trace[after + 2], Event::Flush);
            let bytes: Vec<u8> = trace[..after]
                .iter()
                .filter_map(|event| {
                    if let Event::Bytes(bytes) = event {
                        Some(bytes.as_slice())
                    } else {
                        None
                    }
                })
                .flatten()
                .copied()
                .collect();
            assert!(bytes.ends_with(crate::terminal_lifecycle::SYNCHRONIZED_UPDATE_END));
            drop(trace);
            super::super::draw_surface(
                &mut terminal,
                &mut view,
                super::super::Chrome {
                    locale: Locale::EnUs,
                    theme: &theme,
                },
                &mut surface,
                &mut dispatcher,
                &mut mode,
            )
            .unwrap();
            assert_eq!(
                events
                    .borrow()
                    .iter()
                    .filter(|e| **e == Event::Bytes(b"\x07".to_vec()))
                    .count(),
                1
            );
        }
        lease.block_frame_output();
    }
}

#[test]
fn machine_cover_settles_one_covered_native_receipt() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let fail = Rc::new(RefCell::new(false));
    let dimensions = dimensions(40, 12);
    let lease = Arc::new(crate::terminal_lifecycle::test_support::synchronized_update_lease());
    let backend = FixedViewportBackend::new(
        Writer {
            events: events.clone(),
            fail,
        },
        dimensions,
        lease.clone(),
    );
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(to_area(dimensions)),
        },
    )
    .unwrap();
    let receipts = Rc::new(Cell::new(0));
    let mut surface = Surface {
        events,
        pending: false,
        receipts: receipts.clone(),
    };
    let mut dispatcher = InputDispatcher::default();
    let mut view = ClientView::new(dimensions.columns);
    view.machines.available = true;
    view.machines.show();
    let theme = crate::tui_theme::Theme::terminal_native();

    super::super::draw_surface(
        &mut terminal,
        &mut view,
        super::super::Chrome {
            locale: Locale::EnUs,
            theme: &theme,
        },
        &mut surface,
        &mut dispatcher,
        &mut |_| Ok(()),
    )
    .unwrap();

    assert_eq!(receipts.get(), 1);
    lease.block_frame_output();
}
