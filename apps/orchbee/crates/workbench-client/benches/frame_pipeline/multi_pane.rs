//! Several panes updating in one UI turn: every visible pane's projection receives its record,
//! then the shell draws once. Pane sizes divide the split-tree root area of each window into an
//! even grid with one shared divider cell, approximating the panel tree the renderer projects.
//! Initialization is new projections receiving their first snapshots and a new terminal's first
//! draw; steady state is every live projection receiving a one-row delta (a spinner or status line
//! in each pane) and the next draw. The draw has the same scope as the `paint` cases: pane cells
//! are applied but not painted.

use crate::{
    decode_apply::{SIZES, Stream, applied},
    harness::{Case, Harness, Phase},
    obtf::{Record, Size, attachment, epoch},
    paint::Shell,
};
use orchbee_workbench_client::application::{
    client_view::ClientView,
    terminal_projection::TerminalProjection,
    tui_layout::{PaneFrame, native_terminal_area, project_shell_areas},
};

/// Visible panes per Tab, laid out as columns × rows.
const GRIDS: [(u16, u16); 4] = [(1, 1), (2, 1), (2, 2), (4, 2)];

pub fn register(harness: &mut Harness) {
    for window in SIZES {
        for grid in GRIDS {
            let panes = grid.0 * grid.1;
            let pane = pane_size(window, grid);
            initial(harness, window, panes, pane);
            steady(harness, window, panes, pane);
        }
    }
}

fn pane_size(window: Size, (columns, rows): (u16, u16)) -> Size {
    let areas = project_shell_areas(
        window.area(),
        ClientView::new(window.columns).sidebar_surface(),
    );
    let root = native_terminal_area(
        areas.panel,
        PaneFrame::of(
            areas.tier.shows_panel_frame(),
            usize::from(columns * rows),
            false,
        ),
    );
    Size {
        columns: (root.width - (columns - 1)) / columns,
        rows: (root.height - (rows - 1)) / rows,
    }
}

fn initial(harness: &mut Harness, window: Size, panes: u16, pane: Size) {
    let geometry = pane.geometry();
    let snapshots: Vec<Record> = (0..panes)
        .map(|index| Record::snapshot(pane, 1, 1, u64::from(index)))
        .collect();
    let shell = Shell::new(window);
    harness.run(
        &Case {
            name: &format!("multi/{panes}/init/{}", window.label()),
            phase: Phase::Init,
            bytes: snapshots.iter().map(|record| record.bytes.len()).sum(),
        },
        || {
            let projections: Vec<TerminalProjection> = snapshots
                .iter()
                .map(|snapshot| {
                    let mut projection = TerminalProjection::new(attachment(), epoch());
                    applied(projection.receive_for_geometry(&snapshot.bytes, Some(geometry)));
                    projection
                })
                .collect();
            let mut terminal = Shell::terminal(window);
            let drawn = shell.draw(&mut terminal);
            (projections, terminal, drawn)
        },
    );
}

fn steady(harness: &mut Harness, window: Size, panes: u16, pane: Size) {
    let mut streams: Vec<Stream> = (0..panes)
        .map(|index| Stream::new(pane, Record::delta(pane, 1, 2, 1, u64::from(index) + 1)))
        .collect();
    let shell = Shell::new(window);
    let mut terminal = Shell::terminal(window);
    shell.draw(&mut terminal);
    harness.run(
        &Case {
            name: &format!("multi/{panes}/delta-1/{}", window.label()),
            phase: Phase::Steady,
            bytes: streams.iter().map(|stream| stream.next.bytes.len()).sum(),
        },
        || {
            for stream in &mut streams {
                stream.receive(true);
            }
            shell.draw(&mut terminal)
        },
    );
}
