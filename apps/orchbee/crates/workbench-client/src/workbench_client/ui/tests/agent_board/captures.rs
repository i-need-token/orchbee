//! Opt-in full-shell Start Agent picker captures and their real protocol-backed fixtures.

use super::*;
use crate::agent_board::AgentBoardControl;
use orchbee_local_client_protocol::{LocalClientMessage, write_message};
use ratatui::{Terminal, backend::TestBackend};
use std::io::{self, Read, Write};
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

mod new_tab_menu;

fn admit_fake_client<Stream: Read + Write>(stream: &mut Stream) -> io::Result<()> {
    crate::local_client::test_support::handshake::expect_hello(stream)?;
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}

/// Gives the UI a local origin whose connection was admitted, as a started Session does.
fn install_origin(ui: &mut WorkbenchClientUi) {
    let server = crate::local_client::test_support::server::FakeServer::start_connections(
        1,
        |_, mut stream| admit_fake_client(&mut stream),
    )
    .unwrap();
    let client = crate::local_client::LocalClient::connect(server.endpoint()).unwrap();
    ui.set_agent_board_origin(client.origin());
    drop(client);
    server.finish().unwrap();
}

struct CapturedBoardFrame {
    terminal: Terminal<TestBackend>,
    hits: crate::tui_hit_map::FrameHitMap,
    receipt: DrawReceipt,
    width: u16,
    height: u16,
}

impl CapturedBoardFrame {
    fn text(&self) -> String {
        let buffer = self.terminal.backend().buffer();
        let mut text = String::new();
        for y in 0..self.height {
            let mut x = 0;
            while x < self.width {
                let symbol = buffer[(x, y)].symbol();
                text.push_str(symbol);
                let columns = crate::terminal_text::display_width(symbol).max(1);
                x = x.saturating_add(u16::try_from(columns).unwrap_or(u16::MAX));
            }
            text.push('\n');
        }
        text
    }

    fn control_hit(
        &self,
        control: AgentBoardControl,
    ) -> Option<crate::agent_board::AgentBoardControlTarget> {
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(crate::tui_hit_map::HitTarget::AgentBoardControl(target)) =
                    self.hits.hit_target_at(x, y)
                    && target.control() == control
                {
                    return Some(target.clone());
                }
            }
        }
        None
    }

    fn settle(&mut self, ui: &mut WorkbenchClientUi) {
        ui.after_close_draw(std::mem::take(&mut self.receipt));
    }
}

fn render_board_frame(
    ui: &WorkbenchClientUi,
    width: u16,
    height: u16,
    locale: crate::localization::Locale,
    theme: &crate::tui_theme::Theme,
) -> CapturedBoardFrame {
    let shell = crate::client_view::ClientView::new(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = crate::tui_hit_map::HitMapBuilder::new();
    let mut receipt = DrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &shell,
                locale,
                Some(ui.view()),
                theme,
                &mut hits,
            );
        })
        .unwrap();
    CapturedBoardFrame {
        terminal,
        hits: hits.finish(),
        receipt,
        width,
        height,
    }
}

/// The picker's index of `provider`'s row.
fn provider_row(ui: &WorkbenchClientUi, provider: &str) -> usize {
    ui.agent_board_view()
        .unwrap()
        .picker_items()
        .iter()
        .position(|item| {
            matches!(item, crate::agent_board::picker::Item::Provider { id, .. } if *id == provider)
        })
        .expect("the provider has a row")
}

fn press(ui: &mut WorkbenchClientUi, code: crate::client_view::KeyCode) -> bool {
    let context = ui
        .agent_board_input_context()
        .expect("the picker owns input");
    ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(code, crate::client_view::Modifiers::NONE),
    )
}

/// A ready Workbench with a local origin and a found catalog, so `Ctrl+B G` opens the picker on
/// its own.
fn picker_fixture(found: &[&'static str]) -> WorkbenchClientUi {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    install_origin(&mut ui);
    ui.board.install_start_catalog_t(found);
    ui.board.accept_start_without_task_t();
    ui.open_agent_picker();
    ui
}

#[test]
fn picker_lists_found_providers_and_enter_starts_in_the_focused_idle_panel() {
    let mut ui = picker_fixture(&["codex", "amp", "muse"]);
    let theme = crate::tui_theme::Theme::terminal_native();
    for (locale, width, height) in [
        (crate::localization::Locale::EnUs, 120, 36),
        (crate::localization::Locale::ZhCn, 120, 36),
        (crate::localization::Locale::EnUs, 60, 20),
        (crate::localization::Locale::ZhCn, 60, 20),
    ] {
        ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
        let mut frame = render_board_frame(&ui, width, height, locale, &theme);
        crate::tui_render::test_capture::capture_screen(
            &mut frame.terminal,
            &format!("start-agent-picker-{width}x{height}-{locale:?}"),
            locale,
            &theme,
        );
        let text = frame.text();
        for provider in ["Codex", "Amp", "Muse"] {
            assert!(text.contains(provider), "{locale:?} {width}: {text:?}");
        }
        assert!(
            frame
                .control_hit(AgentBoardControl::Pick(provider_row(&ui, "codex")))
                .is_some()
        );
        assert!(frame.control_hit(AgentBoardControl::Close).is_some());
        frame.settle(&mut ui);
    }

    let expected_panel = ui
        .state
        .as_ref()
        .and_then(|state| state.focus_panel_id().cloned())
        .expect("fixture has one exact focused Panel");
    let context = ui.agent_board_input_context().unwrap();
    assert!(press(&mut ui, crate::client_view::KeyCode::Enter));
    assert_eq!(
        ui.board.start_operation_panel_t(),
        Some(&expected_panel),
        "an idle focused Panel is the start's place"
    );
    assert!(
        !ui.board.is_open(),
        "Enter closes the picker on the same frame"
    );
    assert!(ui.agent_board_input_context().is_none());
    assert!(
        !ui.capture_agent_board_key(
            context.episode(),
            &crate::client_view::KeyEvent::press(
                crate::client_view::KeyCode::Char('c'),
                crate::client_view::Modifiers::NONE
            ),
        ),
        "the retired picker cannot consume a terminal key"
    );
}

#[test]
fn narrow_picker_suspends_without_publishing_a_start_target() {
    use crate::{localization::RecoveryVerb, tui_presentation::PresentationPhase};

    let mut ui = picker_fixture(&["codex", "amp"]);
    let theme = crate::tui_theme::Theme::terminal_native();
    let locale = crate::localization::Locale::ZhCn;

    ui.observe_geometry(Geometry::new(80, 24, 8, 16).unwrap());
    let mut admitted = render_board_frame(&ui, 80, 24, locale, &theme);
    assert!(admitted.control_hit(AgentBoardControl::Pick(1)).is_some());
    admitted.settle(&mut ui);

    // The first height that cannot paint the picker's list suspends it in place.
    let mut narrow = (4..24)
        .rev()
        .map(|height| {
            ui.observe_geometry(Geometry::new(58, height, 8, 16).unwrap());
            render_board_frame(&ui, 58, height, locale, &theme)
        })
        .find(|frame| frame.control_hit(AgentBoardControl::Pick(1)).is_none())
        .expect("a short enough host cannot paint the list");
    let text = narrow.text();
    assert!(text.contains(locale.presentation_notice(true)), "{text:?}");
    assert!(
        text.contains(locale.recovery_verb(RecoveryVerb::Close)),
        "{text:?}"
    );
    assert!(narrow.control_hit(AgentBoardControl::Pick(1)).is_none());
    narrow.settle(&mut ui);

    let context = ui
        .agent_board_input_context()
        .expect("the admitted picker is retained for local recovery");
    assert_eq!(context.presentation(), PresentationPhase::Suspended);
    assert!(!context.interactive());
    assert!(press(&mut ui, crate::client_view::KeyCode::Enter));
    assert_eq!(
        ui.board.start_operation_panel_t(),
        None,
        "a suspended picker starts nothing"
    );
    assert!(press(&mut ui, crate::client_view::KeyCode::Esc));
    assert!(!ui.board.is_open(), "Esc closes the suspended picker");
}

#[test]
fn picker_mouse_start_requires_the_current_exact_draw() {
    let mut ui = picker_fixture(&["codex", "amp"]);
    let theme = crate::tui_theme::Theme::terminal_native();
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());

    let mut stale_frame =
        render_board_frame(&ui, 120, 40, crate::localization::Locale::EnUs, &theme);
    stale_frame.settle(&mut ui);
    let stale = stale_frame
        .control_hit(AgentBoardControl::Pick(provider_row(&ui, "codex")))
        .expect("the Codex row is painted");
    assert!(press(&mut ui, crate::client_view::KeyCode::Char('c')));
    assert!(
        !ui.click_agent_board_control(&stale),
        "a query change invalidates the old pointer target"
    );
    assert_eq!(ui.board.start_operation_panel_t(), None);

    let mut current_frame =
        render_board_frame(&ui, 120, 40, crate::localization::Locale::EnUs, &theme);
    current_frame.settle(&mut ui);
    let current = current_frame
        .control_hit(AgentBoardControl::Pick(provider_row(&ui, "codex")))
        .expect("the Codex row is repainted for the new query");
    let expected_panel = ui
        .state
        .as_ref()
        .and_then(|state| state.focus_panel_id().cloned())
        .unwrap();
    assert!(ui.click_agent_board_control(&current));
    assert!(!ui.board.is_open());
    assert!(ui.agent_board_input_context().is_none());
    assert_eq!(ui.board.start_operation_panel_t(), Some(&expected_panel));
}

/// Hands every routed input to the UI the way the production `WorkbenchClient` port does.
struct RoutedInput<'a> {
    ui: &'a mut WorkbenchClientUi,
    forwarded: &'a mut Vec<u8>,
}

impl crate::terminal_input::TerminalInputPort for RoutedInput<'_> {
    fn native_capture_claims(&self) -> crate::tui_composition::CaptureClaims {
        self.ui.capture_claims()
    }

    fn native_capture_context(
        &self,
        owner: crate::tui_composition::WorkbenchCaptureOwner,
    ) -> Option<crate::workbench_client::CaptureContext> {
        self.ui.capture_context(owner)
    }

    fn forward(
        &mut self,
        input: &[u8],
    ) -> Result<(), crate::terminal_input::TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }

    fn apply_client_action(
        &mut self,
        action: crate::workbench_client::InputAction,
    ) -> Option<ClientActionDisposition> {
        Some(self.ui.apply_client_action(action))
    }

    fn take_action_menu_action(
        &mut self,
        pointer_released: bool,
    ) -> Option<crate::workbench_client::InputAction> {
        self.ui.take_action_menu_action(pointer_released)
    }

    fn surface_input(&mut self, input: crate::terminal_input::SurfaceInput<'_>) -> bool {
        self.ui.surface_input(input)
    }

    fn surface_action(
        &mut self,
        action: crate::terminal_input::SurfaceAction<'_>,
    ) -> Option<ClientActionDisposition> {
        self.ui.surface_action(action)
    }
}

/// A terminal sends Enter as a bare CR, which decodes to the canonical `Ctrl+M`. Whatever opened
/// the picker, that byte must start the provider under its cursor.
#[test]
fn terminal_enter_starts_the_picked_provider_from_every_entry() {
    #[derive(Clone, Copy, Debug)]
    enum Entry {
        PrefixKey,
        ActionMenu,
    }
    fn present(ui: &mut WorkbenchClientUi) {
        let theme = crate::tui_theme::Theme::terminal_native();
        render_board_frame(ui, 120, 36, crate::localization::Locale::EnUs, &theme).settle(ui);
    }
    for entry in [Entry::PrefixKey, Entry::ActionMenu] {
        let (mut ui, mailbox, clock) = fixture();
        bootstrap(&mut ui, &mailbox, &clock);
        ready(&mut ui);
        install_origin(&mut ui);
        ui.board.install_start_catalog_t(&["codex"]);
        ui.board.accept_start_without_task_t();
        ui.observe_geometry(Geometry::new(120, 36, 8, 16).unwrap());
        let mut dispatcher = crate::terminal_input::InputDispatcher::default();
        let mut view = crate::client_view::ClientView::new(120);
        let mut forwarded = Vec::new();
        let mut route = |ui: &mut WorkbenchClientUi, bytes: &[u8]| {
            dispatcher
                .dispatch(
                    &mut view,
                    bytes,
                    &mut RoutedInput {
                        ui,
                        forwarded: &mut forwarded,
                    },
                    Instant::now(),
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
        };
        match entry {
            Entry::PrefixKey => route(&mut ui, b"\x02G"),
            Entry::ActionMenu => {
                // The action menu's Start Agent row commits this action.
                let _disposition =
                    ui.apply_client_action(crate::client_view::ClientAction::StartAgent);
            }
        }
        assert!(ui.board.picker().is_some(), "{entry:?} opens the picker");
        present(&mut ui);
        route(&mut ui, b"\r");

        let focused = ui
            .state
            .as_ref()
            .and_then(|state| state.focus_panel_id().cloned())
            .expect("fixture has one exact focused Panel");
        assert_eq!(
            ui.board.start_operation_panel_t(),
            Some(&focused),
            "{entry:?}: Enter starts Codex in the focused idle Panel"
        );
        assert!(
            !ui.board.is_open(),
            "{entry:?}: the start closes the picker"
        );
        assert!(forwarded.is_empty(), "{entry:?}: no key reached the Panel");
    }
}

/// Raw terminal bytes through the production dispatcher, each followed by a complete frame.
struct TerminalRoute {
    dispatcher: crate::terminal_input::InputDispatcher,
    view: crate::client_view::ClientView,
    forwarded: Vec<u8>,
    theme: crate::tui_theme::Theme,
}

impl TerminalRoute {
    /// A ready Workbench with a found catalog of `found` and no picker open.
    fn ready(found: &[&'static str]) -> (WorkbenchClientUi, Self) {
        let (mut ui, mailbox, clock) = fixture();
        bootstrap(&mut ui, &mailbox, &clock);
        ready(&mut ui);
        install_origin(&mut ui);
        ui.board.install_start_catalog_t(found);
        ui.board.accept_start_without_task_t();
        ui.observe_geometry(Geometry::new(120, 36, 8, 16).unwrap());
        let route = Self {
            dispatcher: crate::terminal_input::InputDispatcher::default(),
            view: crate::client_view::ClientView::new(120),
            forwarded: Vec::new(),
            theme: crate::tui_theme::Theme::terminal_native(),
        };
        (ui, route)
    }

    fn send(&mut self, ui: &mut WorkbenchClientUi, bytes: &[u8]) {
        self.send_at(ui, bytes, Instant::now());
    }

    fn send_at(&mut self, ui: &mut WorkbenchClientUi, bytes: &[u8], now: Instant) {
        self.dispatcher
            .dispatch(
                &mut self.view,
                bytes,
                &mut RoutedInput {
                    ui,
                    forwarded: &mut self.forwarded,
                },
                now,
                |_, _| Ok::<bool, io::Error>(false),
                &mut |_| Ok(()),
            )
            .unwrap();
        render_board_frame(ui, 120, 36, crate::localization::Locale::EnUs, &self.theme).settle(ui);
    }

    /// A lone ESC byte, which the decoder resolves only once its wait for a sequence has passed.
    fn escape(&mut self, ui: &mut WorkbenchClientUi) {
        let now = Instant::now();
        self.send_at(ui, b"\x1b", now);
        self.send_at(ui, b"", now + Duration::from_secs(1));
    }
}

fn picker_place(ui: &WorkbenchClientUi) -> Option<crate::agent_board::start::StartPlace> {
    ui.board.picker().map(|picker| picker.place().clone())
}

/// A terminal sends Tab as a bare HT and Shift+Tab as `CSI Z`. In the picker both cycle the place
/// and reach neither the Panel nor any other owner; Enter on a split asks for that split with the
/// Agent as its new shell's first command instead of typing into the focused Panel.
#[test]
fn terminal_tab_cycles_the_place_and_enter_starts_a_split() {
    use crate::agent_board::start::StartPlace::{
        NewTab, NewWorktree, SplitDown, SplitRight, ThisPanel,
    };
    let (mut ui, mut route) = TerminalRoute::ready(&["codex"]);
    route.send(&mut ui, b"\x02G");
    let focused = ui
        .state
        .as_ref()
        .and_then(|state| state.focus_panel_id().cloned())
        .expect("fixture has one exact focused Panel");
    assert_eq!(picker_place(&ui), Some(ThisPanel(focused.clone())));
    for (bytes, expected) in [
        (&b"\t"[..], NewTab),
        (b"\t", SplitRight),
        (b"\x1b[Z", NewTab),
        (b"\x1b[Z", ThisPanel(focused.clone())),
        (b"\x1b[Z", NewWorktree),
        (b"\x1b[Z", SplitDown),
        (b"\t", NewWorktree),
        (b"\t", ThisPanel(focused.clone())),
        (b"\x1b[Z", NewWorktree),
        (b"\x1b[Z", SplitDown),
    ] {
        route.send(&mut ui, bytes);
        assert_eq!(picker_place(&ui), Some(expected), "{bytes:?}");
    }
    route.send(&mut ui, b"\r");

    let forwarded = &route.forwarded;
    assert!(!ui.board.is_open(), "the start closes the picker");
    assert!(forwarded.is_empty(), "no key reached the Panel");
    assert_eq!(
        ui.board.start_operation_panel_t(),
        None,
        "nothing is typed into the focused Panel"
    );
    let operation = ui
        .state
        .as_ref()
        .and_then(|state| state.pending_panel())
        .expect("Enter asks for the split");
    assert!(matches!(
        operation.intent(),
        crate::workbench_state::PanelIntent::Split { direction, .. }
            if *direction == orchbee_workbench_protocol::WorkbenchPanelSplitDirection::Below
    ));
    assert_eq!(operation.panel_id(), &focused);
    assert_eq!(operation.agent_start(), Some("codex"));
}

/// Enter on New Worktree opens the New Worktree form over the picker for the picked Agent and
/// starts nothing yet; Esc in the form returns to the picker with its query, cursor and place.
#[test]
fn terminal_enter_on_new_worktree_opens_the_form_and_esc_restores_the_picker() {
    use crate::agent_board::start::StartPlace::NewWorktree;
    use crate::workbench_state::worktree_launch::WorktreeMode;
    let (mut ui, mut route) = TerminalRoute::ready(&["codex", "gemini"]);
    route.send(&mut ui, b"\x02G");
    route.send(&mut ui, b"co");
    for _ in 0..4 {
        route.send(&mut ui, b"\t");
    }
    assert_eq!(picker_place(&ui), Some(NewWorktree));
    let picker_state = |ui: &WorkbenchClientUi| {
        let items = ui.agent_board_view()?.picker_items();
        let picker = ui.board.picker()?;
        Some((
            picker.query().to_owned(),
            picker
                .cursor_index(&items)
                .and_then(|index| items.get(index).copied()),
            picker.place().clone(),
        ))
    };
    let before = picker_state(&ui);
    assert!(
        matches!(&before, Some((query, Some(crate::agent_board::picker::Item::Provider { id: "codex", .. }), _)) if query == "co"),
        "{before:?}"
    );
    route.send(&mut ui, b"\r");

    assert!(ui.worktrees.launcher.visible(), "Enter opens the form");
    assert_eq!(
        ui.worktrees
            .launcher
            .preparation()
            .map(|preparation| (preparation.mode, preparation.agent)),
        Some((WorktreeMode::Create, Some("codex")))
    );
    assert_eq!(picker_state(&ui), before, "the picker waits under the form");
    assert_eq!(ui.board.start_operation_panel_t(), None, "nothing is typed");
    assert!(
        ui.state
            .as_ref()
            .is_some_and(|state| state.pending_panel().is_none()),
        "nothing is created before the form submits"
    );
    route.escape(&mut ui);

    assert!(!ui.worktrees.launcher.visible(), "Esc cancels the form");
    assert_eq!(picker_state(&ui), before, "the picker returns as it was");
    assert!(route.forwarded.is_empty(), "no key reached the Panel");
}
