//! Output trace at the actual draw/flush boundary, without an inherited terminal.
use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, PartialEq)]
enum Event {
    Bytes(Vec<u8>),
    Flush,
    AfterDraw,
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
}
impl TerminalInputPort for Surface {
    fn forward(&mut self, _: &[u8]) -> Result<(), TerminalInputUnavailable> {
        Ok(())
    }
}
impl super::super::TerminalSurface for Surface {
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
        let lease = Arc::new(SynchronizedUpdateLease::new());
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
