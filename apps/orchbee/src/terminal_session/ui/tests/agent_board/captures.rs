//! Opt-in full-shell Agent Board captures and their real protocol-backed fixtures.

use super::*;
use orchbee_agent_observation_protocol::{
    AgentExecutionLocation, AgentExecutionLocationId, AgentLifecycle, AgentListOutcome,
    AgentListOutcomeEnvelope, AgentObservation, AgentObservationFidelity,
    AgentObservationFreshness, AgentObservationSource, AgentPositiveInteger,
    AgentPresentationAttachment, AgentProviderId, AgentReadRequest, AgentReadResult, AgentRequest,
    AgentResult, AgentSafeInteger, AgentSemanticStatus, AgentSessionId, AgentSessionSummary,
    AgentTurnPhase, read_agent_request, write_agent_result,
};
use orchbee_local_client_protocol::{LocalClientMessage, read_message, write_message};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::io::{self, Read, Write};
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

fn board_summary(
    session: &str,
    provider: &str,
    status: AgentSemanticStatus,
    lifecycle: AgentLifecycle,
    freshness: AgentObservationFreshness,
    attachments: Vec<AgentPresentationAttachment>,
) -> AgentSessionSummary {
    AgentSessionSummary {
        execution_location: AgentExecutionLocation {
            execution_location_id: AgentExecutionLocationId::parse("local").unwrap(),
        },
        lifecycle,
        observation: AgentObservation {
            fidelity: AgentObservationFidelity::Rich,
            freshness,
            generation: AgentSafeInteger::new(7),
            sequence: AgentSafeInteger::new(3),
            source: AgentObservationSource::Native,
        },
        parent_session_id: None,
        pending_request_count: AgentSafeInteger::new(0),
        presentation_attachments: attachments,
        provider_id: AgentProviderId::parse(provider).unwrap(),
        session_id: AgentSessionId::parse(session).unwrap(),
        status,
    }
}

fn capture_rows() -> Vec<AgentSessionSummary> {
    vec![
        board_summary(
            "agent-review",
            "grok",
            AgentSemanticStatus::NeedsAttention {
                count: AgentPositiveInteger::new(2).unwrap(),
            },
            AgentLifecycle::Live,
            AgentObservationFreshness::Fresh,
            vec![attachment(0x3ea)],
        ),
        board_summary(
            "agent-implementation",
            "codex",
            AgentSemanticStatus::Working {
                activity: Some(orchbee_agent_observation_protocol::AgentActivity::Editing),
                phase: AgentTurnPhase::Working,
            },
            AgentLifecycle::Live,
            AgentObservationFreshness::Fresh,
            vec![attachment(0x3ea)],
        ),
        board_summary(
            "agent-validation",
            "devin",
            AgentSemanticStatus::Succeeded {},
            AgentLifecycle::Live,
            AgentObservationFreshness::Stale,
            Vec::new(),
        ),
        board_summary(
            "agent-offline",
            "codex",
            AgentSemanticStatus::Ended {},
            AgentLifecycle::Ended,
            AgentObservationFreshness::Disconnected,
            Vec::new(),
        ),
    ]
}

fn admit_fake_client<Stream: Read + Write>(
    stream: &mut Stream,
    expected_cohort: &str,
) -> io::Result<()> {
    let LocalClientMessage::ClientHello(hello) = read_message(stream).map_err(io::Error::other)?
    else {
        return Err(io::Error::other("expected ClientHello"));
    };
    if hello.cohort() != expected_cohort {
        return Err(io::Error::other("unexpected ClientHello cohort"));
    }
    write_message(
        stream,
        &LocalClientMessage::ClientAccepted {
            generation: NonZeroU64::new(1).unwrap(),
        },
    )
    .map_err(io::Error::other)
}

fn install_capture_rows(ui: &mut NativeUi) {
    install_capture_read(ui, capture_rows(), 4);
}

fn install_capture_empty_read(ui: &mut NativeUi) {
    install_capture_read(ui, Vec::new(), 0);
}

fn install_capture_read(
    ui: &mut NativeUi,
    initial_rows: Vec<AgentSessionSummary>,
    expected_rows: usize,
) {
    const COHORT: &str = "agent-board-capture";
    let mut rows = Some(initial_rows);
    let server = crate::local_client::tests::server::FakeServer::start_connections(
        2,
        move |index, mut stream| {
            admit_fake_client(&mut stream, COHORT)?;
            if index == 0 {
                return Ok(());
            }
            let AgentRequest::Read(AgentReadRequest::List {
                request_id,
                include_ended: true,
                cursor: None,
                ..
            }) = read_agent_request(&mut stream).map_err(io::Error::other)?
            else {
                return Err(io::Error::other("expected initial Agent list request"));
            };
            write_agent_result(
                &mut stream,
                &AgentResult::Read(AgentReadResult::List {
                    request_id,
                    result: AgentListOutcomeEnvelope {
                        outcome: AgentListOutcome::Listed {
                            items: rows.take().expect("one Agent list response"),
                            next_cursor: None,
                            revision: AgentSafeInteger::new(11),
                        },
                    },
                }),
            )
            .map_err(io::Error::other)
        },
    )
    .unwrap();
    let client = crate::local_client::LocalClient::connect(server.endpoint(), COHORT).unwrap();
    ui.set_agent_board_origin(client.origin());
    drop(client);

    ui.board.open();
    assert!(ui.drive_agent_board(true));
    server.finish().unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    while ui.agent_board_view().is_none_or(|board| {
        board.rows().len() != expected_rows
            || !matches!(
                board.read(),
                crate::terminal_session::AgentBoardRead::Ready { .. }
            )
    }) {
        assert!(
            Instant::now() < deadline,
            "Agent list outcome did not settle"
        );
        let _changed = ui.drive_agent_board(true);
        std::thread::yield_now();
    }
}

struct CapturedBoardFrame {
    terminal: Terminal<TestBackend>,
    hits: crate::tui_pointer::FrameHitMap,
    receipt: NativeDrawReceipt,
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

    fn row_hit(
        &self,
        session: &str,
    ) -> Option<(u16, u16, crate::terminal_session::AgentBoardRowTarget)> {
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(crate::tui_pointer::HitTarget::AgentBoardRow(target)) =
                    self.hits.hit_target_at(x, y)
                    && target.session().as_str() == session
                {
                    return Some((x, y, target.clone()));
                }
            }
        }
        None
    }

    fn control_hit(
        &self,
        control: AgentBoardControl,
    ) -> Option<crate::terminal_session::AgentBoardControlTarget> {
        for y in 0..self.height {
            for x in 0..self.width {
                if let Some(crate::tui_pointer::HitTarget::AgentBoardControl(target)) =
                    self.hits.hit_target_at(x, y)
                    && target.control() == control
                {
                    return Some(target.clone());
                }
            }
        }
        None
    }

    fn buffer(&self) -> &Buffer {
        self.terminal.backend().buffer()
    }

    fn settle(&mut self, ui: &mut NativeUi) {
        ui.after_close_draw(std::mem::take(&mut self.receipt));
    }
}

fn render_board_frame(
    ui: &NativeUi,
    width: u16,
    height: u16,
    locale: crate::localization::Locale,
    theme: &crate::tui_theme::Theme,
) -> CapturedBoardFrame {
    let shell = crate::client_view::ClientView::new(width);
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = crate::tui_pointer::HitMapBuilder::new();
    let mut receipt = NativeDrawReceipt::default();
    terminal
        .draw(|frame| {
            receipt = crate::tui_render::render_terminal_shell(
                frame,
                &shell,
                locale,
                Some(crate::terminal_session::TerminalSessionView::Native(
                    ui.view(),
                )),
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

fn capture_board_frame(
    ui: &NativeUi,
    width: u16,
    height: u16,
    locale: crate::localization::Locale,
    theme: &crate::tui_theme::Theme,
    name: &str,
) -> CapturedBoardFrame {
    let mut frame = render_board_frame(ui, width, height, locale, theme);
    crate::tui_render::test_capture::capture_screen(&mut frame.terminal, name, locale, theme);
    frame
}

#[cfg(windows)]
#[test]
fn empty_board_renders_the_client_path_candidate_route_in_both_locales() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    install_capture_empty_read(&mut ui);
    ui.board
        .install_start_draft_t(&["codex", "amp", "muse"], "alpha");
    ui.board.accept_start_without_task_t();
    let theme = crate::tui_theme::Theme::terminal_native();

    for (locale, width, height, empty_line) in [
        (
            crate::localization::Locale::EnUs,
            120,
            40,
            "No Agent sessions",
        ),
        (crate::localization::Locale::ZhCn, 80, 24, "没有智能体会话"),
    ] {
        ui.observe_geometry(Geometry::new(width, height, 8, 16).unwrap());
        let mut frame = render_board_frame(&ui, width, height, locale, &theme);
        let text = frame.text();
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let board_text = locale.shell().board;
        assert!(normalized.contains(empty_line), "{locale:?}: {text:?}");
        assert!(
            normalized.contains(board_text.empty_start),
            "{locale:?}: {text:?}"
        );
        for provider in ["codex", "amp", "muse"] {
            assert!(normalized.contains(provider), "{locale:?}: {text:?}");
        }
        assert!(!normalized.contains("claude-code"), "{locale:?}: {text:?}");
        assert!(normalized.contains("alpha"), "{locale:?}: {text:?}");
        assert!(
            normalized.contains(board_text.start_button),
            "{locale:?}: {text:?}"
        );
        frame.settle(&mut ui);
    }

    let expected_panel = ui
        .state
        .as_ref()
        .and_then(|state| state.focus_panel_id().cloned())
        .expect("fixture has one exact focused Panel");
    let context = ui
        .agent_board_input_context()
        .expect("the Board remains the active input owner");
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Enter,
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert!(ui.board.start_running());
    assert_eq!(
        ui.board.start_operation_panel_t(),
        Some(&expected_panel),
        "keyboard activation injects the exact focused Panel"
    );
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('c'),
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert_eq!(
        ui.agent_board_view().unwrap().start_phase(),
        Some(crate::terminal_session::agent_board::AgentStartPhase::Starting),
        "Cancel in the same input packet cannot act before Starting is painted"
    );

    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    let mut cancelling_frame =
        render_board_frame(&ui, 120, 40, crate::localization::Locale::EnUs, &theme);
    cancelling_frame.settle(&mut ui);
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('c'),
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert_eq!(
        ui.agent_board_view().unwrap().start_phase(),
        Some(crate::terminal_session::agent_board::AgentStartPhase::Cancelling),
    );
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Esc,
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert!(
        ui.board.start_running(),
        "closing the Board does not cancel accepted startup"
    );
}

#[cfg(windows)]
#[test]
fn narrow_zh_empty_board_suspends_start_without_publishing_a_start_target() {
    use crate::{localization::RecoveryVerb, tui_presentation::PresentationPhase};

    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    install_capture_empty_read(&mut ui);
    ui.board
        .install_start_draft_t(&["codex", "amp", "muse"], "alpha");
    ui.board.accept_start_without_task_t();
    let theme = crate::tui_theme::Theme::terminal_native();
    let locale = crate::localization::Locale::ZhCn;

    ui.observe_geometry(Geometry::new(80, 24, 8, 16).unwrap());
    let mut admitted = render_board_frame(&ui, 80, 24, locale, &theme);
    assert!(admitted.control_hit(AgentBoardControl::Start).is_some());
    admitted.settle(&mut ui);

    // This is the exact narrow boundary that exposed the original false
    // full-form assertion: the admitted Board retains local close ownership,
    // but no incomplete Start form or effect target may escape.
    ui.observe_geometry(Geometry::new(58, 12, 8, 16).unwrap());
    let mut narrow = render_board_frame(&ui, 58, 12, locale, &theme);
    let text = narrow.text();
    assert!(text.contains(locale.presentation_notice(true)), "{text:?}");
    assert!(
        text.contains(locale.recovery_verb(RecoveryVerb::Close)),
        "{text:?}"
    );
    assert!(text.contains("Esc"), "{text:?}");
    assert!(narrow.control_hit(AgentBoardControl::Start).is_none());
    narrow.settle(&mut ui);

    let context = ui
        .agent_board_input_context()
        .expect("the admitted Board is retained for local recovery");
    assert_eq!(context.presentation(), PresentationPhase::Suspended);
    assert!(!context.interactive());
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Esc,
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert!(!ui.board.is_open(), "Esc closes the suspended Board");
}

#[cfg(windows)]
#[test]
fn empty_board_without_a_client_path_candidate_keeps_the_manual_route_in_both_locales() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    install_capture_empty_read(&mut ui);
    ui.board.install_start_draft_t(&[], "");
    let theme = crate::tui_theme::Theme::terminal_native();

    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
        let mut frame = render_board_frame(&ui, 120, 40, locale, &theme);
        let normalized = frame
            .text()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let board_text = locale.shell().board;
        assert!(normalized.contains(board_text.start_none_found));
        assert!(normalized.contains(board_text.empty_start_command));
        assert!(!ui.agent_board_view().unwrap().can_start_agent());
        frame.settle(&mut ui);
    }
}

#[cfg(windows)]
#[test]
fn empty_board_mouse_start_requires_the_current_exact_draw() {
    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    install_capture_empty_read(&mut ui);
    ui.board.install_start_draft_t(&["codex", "amp"], "alpha");
    ui.board.accept_start_without_task_t();
    let theme = crate::tui_theme::Theme::terminal_native();
    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());

    let mut stale_frame =
        render_board_frame(&ui, 120, 40, crate::localization::Locale::EnUs, &theme);
    stale_frame.settle(&mut ui);
    let stale_start = stale_frame
        .control_hit(AgentBoardControl::Start)
        .expect("enabled Start Agent control is painted");
    let context = ui.agent_board_input_context().unwrap();
    assert!(ui.capture_agent_board_key(
        context.episode(),
        &crate::client_view::KeyEvent::press(
            crate::client_view::KeyCode::Char('z'),
            crate::client_view::Modifiers::NONE,
        ),
    ));
    assert!(
        !ui.click_agent_board_control(&stale_start),
        "a draft change invalidates the old pointer target"
    );
    assert!(!ui.board.start_running());

    let mut current_frame =
        render_board_frame(&ui, 120, 40, crate::localization::Locale::EnUs, &theme);
    current_frame.settle(&mut ui);
    let start = current_frame
        .control_hit(AgentBoardControl::Start)
        .expect("Start Agent is repainted for the new draft");
    let expected_panel = ui
        .state
        .as_ref()
        .and_then(|state| state.focus_panel_id().cloned())
        .unwrap();
    assert!(ui.click_agent_board_control(&start));
    assert_eq!(ui.board.start_operation_panel_t(), Some(&expected_panel));
}

#[test]
fn captures_whole_shell_agent_board_alignment_frames() {
    use crate::tui_presentation::PresentationPhase;

    let (mut ui, mailbox, clock) = fixture();
    bootstrap(&mut ui, &mailbox, &clock);
    ready(&mut ui);
    super::super::viewport::install_projection(&mut ui, Some((9, false, 24)), 0);
    install_capture_rows(&mut ui);
    let theme = crate::tui_theme::Theme::terminal_native();

    ui.observe_geometry(Geometry::new(120, 40, 8, 16).unwrap());
    let mut wide = capture_board_frame(
        &ui,
        120,
        40,
        crate::localization::Locale::EnUs,
        &theme,
        "alignment-agent-board-en-us-120x40-terminal-native",
    );
    let wide_text = wide.text();
    assert!(wide_text.contains(crate::localization::Locale::EnUs.shell().board.title));
    assert!(wide_text.contains("agent-review"));
    assert!(wide_text.contains("agent-validation"));
    assert!(wide_text.contains(crate::localization::Locale::EnUs.shell().board.standalone));
    let board = ui.agent_board_view().unwrap();
    assert!(
        wide_text
            .contains(&crate::localization::Locale::EnUs.agent_status(board.rows()[0].status()))
    );
    assert!(
        wide.control_hit(AgentBoardControl::Open).is_some(),
        "the selected navigable row paints the primary Open action"
    );
    assert!(wide.control_hit(AgentBoardControl::Refresh).is_some());
    assert!(wide.control_hit(AgentBoardControl::Close).is_some());
    let (selected_x, selected_y, _) = wide.row_hit("agent-review").expect("selected row hit");
    assert_eq!(
        wide.buffer()[(selected_x, selected_y)].symbol(),
        theme.glyphs.navigation_marker,
        "the hit-tested selected row owns the provisional navigation cue"
    );
    let (_, _, implementation) = wide
        .row_hit("agent-implementation")
        .expect("second visible row hit");
    wide.settle(&mut ui);
    let context = ui.agent_board_input_context().expect("admitted Board");
    assert_eq!(context.presentation(), PresentationPhase::Admitted);
    assert!(context.interactive());
    assert!(
        ui.agent_board_select(&implementation),
        "a target from the real hit map selects its exact row"
    );
    assert_eq!(
        ui.agent_board_view().unwrap().rows()[ui.board.selection()]
            .session_id()
            .as_str(),
        "agent-implementation"
    );

    ui.observe_geometry(Geometry::new(80, 24, 8, 16).unwrap());
    let mut compact = capture_board_frame(
        &ui,
        80,
        24,
        crate::localization::Locale::ZhCn,
        &theme,
        "alignment-agent-board-zh-cn-80x24-terminal-native",
    );
    assert!(
        compact
            .text()
            .contains(crate::localization::Locale::ZhCn.shell().board.title)
    );
    let (selected_x, selected_y, _) = compact
        .row_hit("agent-implementation")
        .expect("compact selected row hit");
    assert_eq!(
        compact.buffer()[(selected_x, selected_y)].symbol(),
        theme.glyphs.navigation_marker
    );
    compact.settle(&mut ui);
    assert!(ui.agent_board_input_context().unwrap().interactive());

    // The production Board deliberately still fits at 58x12. Capture and
    // assert that real compact contract rather than manufacturing a false
    // NotPresented receipt for an otherwise paintable frame.
    ui.observe_geometry(Geometry::new(58, 12, 8, 16).unwrap());
    let mut narrow = capture_board_frame(
        &ui,
        58,
        12,
        crate::localization::Locale::EnUs,
        &theme,
        "alignment-agent-board-narrow-en-us-58x12-terminal-native",
    );
    assert!(narrow.row_hit("agent-implementation").is_some());
    assert!(narrow.control_hit(AgentBoardControl::Open).is_some());
    narrow.settle(&mut ui);
    assert_eq!(
        ui.agent_board_input_context().unwrap().presentation(),
        PresentationPhase::Admitted
    );

    // A fresh episode at the first actually unpaintable height proves the
    // Pending -> Refused path through the same compositor and draw receipt.
    ui.board.close();
    ui.board.open();
    assert_eq!(
        ui.agent_board_input_context().unwrap().presentation(),
        PresentationPhase::Pending
    );
    ui.observe_geometry(Geometry::new(58, 8, 8, 16).unwrap());
    let mut refused = capture_board_frame(
        &ui,
        58,
        8,
        crate::localization::Locale::EnUs,
        &theme,
        "alignment-agent-board-refused-en-us-58x8-terminal-native",
    );
    assert!(refused.row_hit("agent-review").is_none());
    assert!(refused.control_hit(AgentBoardControl::Close).is_none());
    assert!(
        !refused
            .text()
            .contains(crate::localization::Locale::EnUs.shell().board.title)
    );
    assert!(
        refused
            .text()
            .contains(crate::localization::Locale::EnUs.presentation_notice(false))
    );
    refused.settle(&mut ui);
    assert!(
        !ui.board.is_open(),
        "the real geometry verdict refuses a never-presented episode"
    );
}
