//! Painted controls exercise the board owner through the normal input decoder.
use super::*;
use crate::{
    client_view::ClientView,
    localization::Locale,
    terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable},
    tui_pointer::{FrameHitMap, HitMapBuilder, HitTarget},
    tui_theme::{Role, Theme},
};

struct Port {
    board: AgentBoard,
    forwarded: Vec<u8>,
}
impl Port {
    fn capture_agent_board_key_for(
        &mut self,
        episode: AgentBoardEpisodeToken,
        event: crate::client_view::KeyEvent,
    ) -> bool {
        let Some(context) = self.board.input_context() else {
            return false;
        };
        if context.episode() != episode {
            return false;
        }
        if event.kind == crate::client_view::KeyKind::Release {
            return true;
        }
        if board_key_action(event) == Some(AgentBoardKeyAction::Close) {
            self.board.close();
            return true;
        }
        if !context.interactive() {
            return true;
        }
        match board_key_action(event) {
            Some(AgentBoardKeyAction::SelectDown) => self.board.move_selection(1),
            Some(AgentBoardKeyAction::SelectUp) => self.board.move_selection(-1),
            Some(AgentBoardKeyAction::Jump) if self.board.selected_is_shown() => {
                self.board.request_jump();
            }
            Some(AgentBoardKeyAction::Refresh) => self.board.request_refresh(),
            Some(AgentBoardKeyAction::Jump | AgentBoardKeyAction::Close) => {}
            None => {}
        }
        true
    }
}

impl TerminalInputPort for Port {
    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        use crate::terminal_input::SurfaceInput;
        match input {
            SurfaceInput::AgentBoard { episode, key } => {
                self.capture_agent_board_key_for(episode, key)
            }
            _ => false,
        }
    }

    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        crate::tui_composition::CaptureClaims::default().with_agent_board(self.board.owns_input())
    }
    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        match owner {
            crate::tui_composition::WorkbenchCaptureOwner::AgentBoard => self
                .board
                .input_context()
                .map(crate::workbench_client::CaptureContext::AgentBoard),
            _ => None,
        }
    }
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
    fn click_agent_board_control(&mut self, target: &AgentBoardControlTarget) -> bool {
        self.board.control_drawn(target)
    }
    fn scroll_list(
        &mut self,
        target: &crate::tui_pointer::ListScrollTarget,
        up: bool,
        lines: usize,
    ) -> Option<crate::tui_pointer::ListScrollTarget> {
        let crate::tui_pointer::ListScrollTarget::AgentBoard(token) = target else {
            return None;
        };
        self.board
            .scroll_painted(token, up, lines)
            .map(crate::tui_pointer::ListScrollTarget::AgentBoard)
    }
    fn agent_board_select(&mut self, target: &AgentBoardRowTarget) -> bool {
        self.board.select_drawn_row(target)
    }
    fn agent_board_dismiss(&mut self) {
        self.board.close();
    }
}

fn input(d: &mut InputDispatcher, view: &mut ClientView, port: &mut Port, bytes: &[u8]) {
    d.dispatch(
        view,
        bytes,
        port,
        Instant::now(),
        |_, _| Ok::<_, std::io::Error>(false),
        &mut |_| Ok(()),
    )
    .unwrap();
}
fn paint(
    d: &mut InputDispatcher,
    port: &mut Port,
    locale: Locale,
    width: u16,
    height: u16,
) -> FrameHitMap {
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::with_pointer(d.pointer_projection());
    let index = PresentationIndex::default();
    let mut receipt = None;
    let theme = Theme::one_light();
    terminal
        .draw(|frame| {
            receipt = Some(crate::tui_render::render_agent_board_for_test(
                frame,
                locale,
                &AgentBoardView::new(&port.board, &index),
                &theme,
                &mut hits,
            ));
        })
        .unwrap();
    port.board.settle_draw(receipt);
    let map = hits.finish();
    assert_ne!(
        theme.color(Role::SurfaceOverlay),
        theme.color(Role::SurfaceChrome)
    );
    let close_cells = (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            matches!(
                map.hit_target_at(x, y),
                Some(HitTarget::AgentBoardControl(target))
                    if target.control() == AgentBoardControl::Close
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(close_cells.len(), 3, "fixed [x] close extent");
    assert!(close_cells.iter().all(|&(x, y)| {
        terminal.backend().buffer()[(x, y)].bg == theme.color(Role::SurfaceOverlay)
    }));
    d.publish_hit_map(map.clone());
    map
}
fn cell(
    map: &FrameHitMap,
    width: u16,
    height: u16,
    wanted: impl Fn(&HitTarget) -> bool,
) -> (u16, u16) {
    (0..height)
        .find_map(|y| {
            (0..width).find_map(|x| {
                map.hit_target_at(x, y)
                    .filter(|target| wanted(target))
                    .map(|_| (x + 1, y + 1))
            })
        })
        .expect("visible control")
}
fn mouse(code: u16, cell: (u16, u16), release: bool) -> String {
    format!(
        "\x1b[<{code};{};{}{}",
        cell.0,
        cell.1,
        if release { 'm' } else { 'M' }
    )
}

#[test]
fn mouse_scroll_persists_and_control_release_cannot_retarget_a_new_selection() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(36, 12), (80, 24)] {
            let mut board = board_with(&[]);
            board.rows = (0..40)
                .map(|i| {
                    AgentBoardRow::from_summary(summary(
                        &format!("session-{i}"),
                        vec![attachment(&hex(9), true)],
                    ))
                })
                .collect();
            board.open();
            board.intent = None;
            let mut port = Port {
                board,
                forwarded: Vec::new(),
            };
            let mut d = InputDispatcher::default();
            let mut view = ClientView::new(width);
            d.set_pointer_capture(&mut view, &mut port, true);
            d.synchronize(&mut view, &mut port, Instant::now(), &mut |_| {
                Ok::<_, std::io::Error>(())
            })
            .unwrap();
            let map = paint(&mut d, &mut port, locale, width, height);
            let row = cell(&map, width, height, |target| {
                matches!(target, HitTarget::AgentBoardRow(_))
            });
            input(
                &mut d,
                &mut view,
                &mut port,
                mouse(65, row, false).repeat(20).as_bytes(),
            );
            assert_eq!(
                port.board.scroll, port.board.viewport_rows,
                "one viewport per flush"
            );
            let before = port.board.scroll;
            paint(&mut d, &mut port, locale, width, height);
            assert_eq!(
                port.board.scroll, before,
                "repaint cannot snap back to the old selection"
            );
            input(
                &mut d,
                &mut view,
                &mut port,
                mouse(64, row, false).as_bytes(),
            );
            assert_eq!(port.board.scroll, before.saturating_sub(3));
            let map = paint(&mut d, &mut port, locale, width, height);
            let open = cell(&map, width, height, |target| {
                matches!(
                    target,
                    HitTarget::AgentBoardControl(target)
                        if target.control() == AgentBoardControl::Open
                )
            });
            input(
                &mut d,
                &mut view,
                &mut port,
                mouse(0, open, false).as_bytes(),
            );
            assert!(port.board.intent.is_none(), "press has no shared effect");
            input(&mut d, &mut view, &mut port, b"\x1b[B");
            paint(&mut d, &mut port, locale, width, height);
            input(
                &mut d,
                &mut view,
                &mut port,
                mouse(0, open, true).as_bytes(),
            );
            assert!(
                port.board.intent.is_none(),
                "keyboard cancels the pending old selection"
            );
            let map = paint(&mut d, &mut port, locale, width, height);
            let open = cell(&map, width, height, |target| {
                matches!(
                    target,
                    HitTarget::AgentBoardControl(target)
                        if target.control() == AgentBoardControl::Open
                )
            });
            let selected = port.board.selected_row().unwrap().session_id.clone();
            input(
                &mut d,
                &mut view,
                &mut port,
                format!("{}{}tail", mouse(0, open, false), mouse(0, open, true)).as_bytes(),
            );
            assert!(
                matches!(&port.board.intent, Some(BoardIntent::Jump { target }) if *target == selected)
            );
            port.board.intent = None;
            let map = paint(&mut d, &mut port, locale, width, height);
            let refresh = cell(&map, width, height, |target| {
                matches!(
                    target,
                    HitTarget::AgentBoardControl(target)
                        if target.control() == AgentBoardControl::Refresh
                )
            });
            input(
                &mut d,
                &mut view,
                &mut port,
                format!("{}{}", mouse(0, refresh, false), mouse(0, refresh, true)).as_bytes(),
            );
            assert!(matches!(port.board.intent, Some(BoardIntent::Refresh)));
            let map = paint(&mut d, &mut port, locale, width, height);
            let close = cell(&map, width, height, |target| {
                matches!(
                    target,
                    HitTarget::AgentBoardControl(target)
                        if target.control() == AgentBoardControl::Close
                )
            });
            input(
                &mut d,
                &mut view,
                &mut port,
                format!("{}{}tail", mouse(0, close, false), mouse(0, close, true)).as_bytes(),
            );
            assert!(!port.board.is_open());
            assert!(port.forwarded.is_empty());
            d.publish_hit_map(HitMapBuilder::new().finish());
            input(&mut d, &mut view, &mut port, b"fresh");
            assert_eq!(port.forwarded, b"fresh");
        }
    }
}
