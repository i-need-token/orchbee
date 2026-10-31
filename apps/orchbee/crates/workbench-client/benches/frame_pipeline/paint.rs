//! Shell painting through the public render entry into ratatui's `TestBackend`. A complete draw is
//! one `Terminal::draw`: the paint closure, the diff against the previous frame, the backend's cell
//! writes, and the buffer swap. Initialization is a new terminal's first draw, whose diff covers
//! every cell; steady state is the next draw of an unchanged shell; `buffer` is the paint closure
//! alone. No Workbench Client view has a stable public constructor, so the shell paints its
//! unavailable-Workbench content: Workbench Tabs, pane frames and dividers, terminal cells, and
//! Workbench overlays are outside this benchmark.

use std::cell::RefCell;

use crate::{
    decode_apply::SIZES,
    harness::{Case, Harness, Phase},
    obtf::Size,
};
use orchbee_workbench_client::application::{
    client_view::ClientView, localization::Locale, tui_layout::ResizeProjection,
    tui_pointer::HitMapBuilder, tui_render::render_terminal_shell_with_resize, tui_theme::Theme,
    workbench_client::DrawReceipt,
};
use ratatui::{Frame, Terminal, backend::TestBackend};

/// The Client shell without a Workbench Client, as the shell loop paints it for one window.
pub struct Shell {
    view: ClientView,
    theme: Theme,
    resize: ResizeProjection,
}

impl Shell {
    pub fn new(size: Size) -> Self {
        Self {
            view: ClientView::new(size.columns),
            theme: Theme::terminal_native(),
            resize: ResizeProjection::default(),
        }
    }

    pub fn terminal(size: Size) -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(size.columns, size.rows)).expect("test backend terminal")
    }

    /// One complete draw; the receipt and hit map it produces are returned for release outside
    /// the timed region.
    pub fn draw(&self, terminal: &mut Terminal<TestBackend>) -> (DrawReceipt, HitMapBuilder) {
        let mut hits = HitMapBuilder::default();
        let mut receipt = DrawReceipt::default();
        terminal
            .draw(|frame| receipt = self.paint(frame, &mut hits))
            .expect("test backend draws");
        (receipt, hits)
    }

    fn paint(&self, frame: &mut Frame<'_>, hits: &mut HitMapBuilder) -> DrawReceipt {
        render_terminal_shell_with_resize(
            frame,
            &self.view,
            Locale::EnUs,
            None,
            &self.theme,
            hits,
            &self.resize,
        )
    }
}

pub fn register(harness: &mut Harness) {
    for size in SIZES {
        let shell = Shell::new(size);
        harness.run(
            &Case {
                name: &format!("paint/init/{}", size.label()),
                phase: Phase::Init,
                bytes: 0,
            },
            || {
                let mut terminal = Shell::terminal(size);
                let drawn = shell.draw(&mut terminal);
                (terminal, drawn)
            },
        );

        let mut terminal = Shell::terminal(size);
        shell.draw(&mut terminal);
        harness.run(
            &Case {
                name: &format!("paint/frame/{}", size.label()),
                phase: Phase::Steady,
                bytes: 0,
            },
            || shell.draw(&mut terminal),
        );

        // A draw resets the next buffer after its swap; this case pays that reset untimed.
        let terminal = RefCell::new(terminal);
        harness.run_with_input(
            &Case {
                name: &format!("paint/buffer/{}", size.label()),
                phase: Phase::Steady,
                bytes: 0,
            },
            || terminal.borrow_mut().current_buffer_mut().reset(),
            |()| {
                let mut terminal = terminal.borrow_mut();
                let mut hits = HitMapBuilder::default();
                let receipt = shell.paint(&mut terminal.get_frame(), &mut hits);
                (receipt, hits)
            },
        );
    }
}
