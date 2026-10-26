use super::*;
use orchbee_workbench_protocol::{WindowTitle, WindowTitleSnapshot, WorkbenchRevision};
#[test]
fn window_title_control_tick_reaches_owner_once_without_consuming_input() {
    struct Surface {
        pending: Option<WindowTitleSnapshot>,
        forwarded: Vec<u8>,
        observed_notice: Option<crate::client_view::Notice>,
        previous: Option<crate::client_view::Notice>,
    }
    impl TerminalInputPort for Surface {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.extend(bytes);
            Ok(())
        }
    }
    impl super::super::TerminalSurface for Surface {
        fn client_control_tick(
            &mut self,
            view: &mut ClientView,
            _: ratatui::layout::Rect,
            _: Locale,
        ) -> bool {
            if let Some(previous) = self.previous.take() {
                view.configuration_notice(previous);
            }
            if view.notice().is_some() {
                self.observed_notice = view.notice();
            }
            if let Some(snapshot) = self.pending.take() {
                view.window_title = Some(snapshot);
            }
            false
        }
    }
    for title in [None, Some(WindowTitle::normalize("Build title").unwrap())] {
        for (fail, previous) in [
            (false, None),
            (true, None),
            (true, Some(crate::client_view::Notice::ConfigurationFailed)),
        ] {
            let initial = dimensions(100, 24);
            let mut terminal = test_terminal(initial);
            let mut inputs = QueueInputSource {
                inputs: VecDeque::from([
                    Ok(Some(b"x".to_vec())),
                    Ok(None),
                    Ok(Some(b"\x02q".to_vec())),
                ]),
            };
            let viewport = QueueViewportSource {
                dimensions: std::cell::RefCell::new(VecDeque::from([
                    Ok(Some(initial)),
                    Ok(Some(initial)),
                    Ok(Some(initial)),
                    Ok(Some(initial)),
                ])),
            };
            let mut surface = Surface {
                pending: Some(WindowTitleSnapshot {
                    revision: WorkbenchRevision::new(1),
                    title: title.clone(),
                }),
                forwarded: Vec::new(),
                observed_notice: None,
                previous,
            };
            let mut written = Vec::new();
            let result = super::super::run_event_loop_with_modes(
                &mut terminal,
                super::super::Chrome {
                    locale: Locale::EnUs,
                    theme: &crate::tui_theme::Theme::terminal_native(),
                },
                (&mut inputs, &mut |request| {
                    if let ModeRequest::WindowTitle(title) = request {
                        written.push(title);
                        if fail {
                            return Err(io::ErrorKind::BrokenPipe.into());
                        }
                    }
                    Ok(())
                }),
                &mut surface,
                &FixedTermination(false),
                &viewport,
                initial,
                true,
                ClientView::new(initial.columns),
            );
            assert!(matches!(result, Ok(LoopExit::User)));
            assert_eq!(
                surface.observed_notice,
                previous.or_else(|| {
                    fail.then_some(crate::client_view::Notice::WindowTitleUnavailable)
                })
            );
            assert_eq!(written, vec![title.clone()]);
            assert_eq!(surface.forwarded, b"x");
        }
    }
}

#[test]
fn notification_effect_reaches_owner_once_and_failure_keeps_input_and_exit_working() {
    struct Surface {
        pending: bool,
        forwarded: Vec<u8>,
        notice: bool,
    }
    impl TerminalInputPort for Surface {
        fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
            self.forwarded.extend(bytes);
            Ok(())
        }
    }
    impl super::super::TerminalSurface for Surface {
        fn client_control_tick(
            &mut self,
            view: &mut ClientView,
            _: ratatui::layout::Rect,
            _: Locale,
        ) -> bool {
            self.notice |=
                view.notice() == Some(crate::client_view::Notice::NotificationUnavailable);
            if std::mem::take(&mut self.pending) {
                let policy = orchbee_local_runtime::NotificationSettings::from_profile(
                    "[ui.toast]\ndelivery='terminal'",
                )
                .unwrap();
                view.notifications.configure(policy);
                view.notifications.install(
                    orchbee_workbench_protocol::Notification::normalize(
                        "Ready",
                        None,
                        None,
                        orchbee_workbench_protocol::NotificationSound::None,
                    )
                    .unwrap(),
                    Instant::now(),
                );
                return true;
            }
            false
        }
    }
    for fail in [false, true] {
        let initial = dimensions(100, 24);
        let mut terminal = test_terminal(initial);
        let mut inputs = QueueInputSource {
            inputs: VecDeque::from([
                Ok(Some(b"x".to_vec())),
                Ok(None),
                Ok(Some(b"\x02q".to_vec())),
            ]),
        };
        let viewport = QueueViewportSource {
            dimensions: std::cell::RefCell::new(VecDeque::from([
                Ok(Some(initial)),
                Ok(Some(initial)),
                Ok(Some(initial)),
                Ok(Some(initial)),
                Ok(Some(initial)),
            ])),
        };
        let mut surface = Surface {
            pending: true,
            forwarded: vec![],
            notice: false,
        };
        let mut writes = 0;
        let result = super::super::run_event_loop_with_modes(
            &mut terminal,
            super::super::Chrome {
                locale: Locale::EnUs,
                theme: &crate::tui_theme::Theme::terminal_native(),
            },
            (&mut inputs, &mut |request| {
                if let ModeRequest::Notification(effect) = request {
                    assert_eq!(effect.message.title, "Ready");
                    writes += 1;
                    if fail {
                        return Err(io::ErrorKind::Unsupported.into());
                    }
                }
                Ok(())
            }),
            &mut surface,
            &FixedTermination(false),
            &viewport,
            initial,
            true,
            ClientView::new(initial.columns),
        );
        assert!(matches!(result, Ok(LoopExit::User)), "{result:?}");
        assert_eq!(writes, 1);
        assert_eq!(surface.forwarded, b"x");
        assert_eq!(surface.notice, fail);
    }
}
