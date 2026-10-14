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
    const COHORT: &str = "agent-board-capture";
    let mut rows = Some(capture_rows());
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
    while ui
        .agent_board_view()
        .is_none_or(|board| board.rows().len() != 4)
    {
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

fn capture_board_frame(
    ui: &NativeUi,
    width: u16,
    height: u16,
    locale: crate::localization::Locale,
    theme: &crate::tui_theme::Theme,
    name: &str,
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
    crate::tui_render::test_capture::capture_screen(&mut terminal, name, locale, theme);
    CapturedBoardFrame {
        terminal,
        hits: hits.finish(),
        receipt,
        width,
        height,
    }
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
