//! Frame decoding and application: the OBTF decoder alone, and one attachment's
//! `TerminalProjection` receiving complete transport records (envelope, route, decode, delta
//! merge, row validation, and feedback). Initialization is a fresh projection's first snapshot;
//! steady state is a live projection receiving the next frame of its stream.

use crate::{
    harness::{Case, Harness, Phase},
    obtf::{Record, Size, attachment, epoch},
};
use orchbee_client_server_protocol::TerminalViewerAction;
use orchbee_terminal_protocol::decode_terminal_text_frame;
use orchbee_workbench_client::application::terminal_projection::TerminalProjection;

/// A classic terminal, the release-baseline PTY size, and a large full-screen pane.
pub const SIZES: [Size; 3] = [
    Size {
        columns: 80,
        rows: 24,
    },
    Size {
        columns: 120,
        rows: 32,
    },
    Size {
        columns: 240,
        rows: 64,
    },
];

pub fn register(harness: &mut Harness) {
    for size in SIZES {
        decode(harness, size);
        apply(harness, size);
    }
}

/// Dirty-row shapes of a live stream: one row (typing, a spinner, a status line), a quarter of
/// the screen (a redrawn region), and every row (scrolling output).
fn deltas(size: Size) -> [(&'static str, u16); 3] {
    [
        ("delta-1", 1),
        ("delta-quarter", size.rows.div_ceil(4)),
        ("delta-all", size.rows),
    ]
}

fn decode(harness: &mut Harness, size: Size) {
    let snapshot = Record::snapshot(size, 1, 1, 0);
    harness.run(
        &Case {
            name: &format!("decode/snapshot/{}", size.label()),
            phase: Phase::Init,
            bytes: snapshot.payload().len(),
        },
        || decode_terminal_text_frame(snapshot.payload()).expect("benchmark snapshot decodes"),
    );
    for (label, dirty) in deltas(size) {
        let delta = Record::delta(size, dirty, 2, 1, 1);
        harness.run(
            &Case {
                name: &format!("decode/{label}/{}", size.label()),
                phase: Phase::Steady,
                bytes: delta.payload().len(),
            },
            || decode_terminal_text_frame(delta.payload()).expect("benchmark delta decodes"),
        );
    }
}

fn apply(harness: &mut Harness, size: Size) {
    let geometry = size.geometry();
    let snapshot = Record::snapshot(size, 1, 1, 0);
    harness.run_with_input(
        &Case {
            name: &format!("apply/init/{}", size.label()),
            phase: Phase::Init,
            bytes: snapshot.bytes.len(),
        },
        || TerminalProjection::new(attachment(), epoch()),
        |mut projection| {
            applied(projection.receive_for_geometry(&snapshot.bytes, Some(geometry)));
            projection
        },
    );

    let mut stream = Stream::new(size, Record::snapshot(size, 2, 2, 1));
    harness.run(
        &Case {
            name: &format!("apply/full/{}", size.label()),
            phase: Phase::Steady,
            bytes: stream.next.bytes.len(),
        },
        || stream.receive(false),
    );
    for (label, dirty) in deltas(size) {
        let mut stream = Stream::new(size, Record::delta(size, dirty, 2, 1, 1));
        harness.run(
            &Case {
                name: &format!("apply/{label}/{}", size.label()),
                phase: Phase::Steady,
                bytes: stream.next.bytes.len(),
            },
            || stream.receive(true),
        );
    }
}

/// A live projection and the record it receives next, re-addressed after every application so
/// sequence and revision keep advancing exactly as one Host stream would.
pub struct Stream {
    projection: TerminalProjection,
    geometry: orchbee_client_server_protocol::Geometry,
    pub next: Record,
    sequence: u64,
    revision: u64,
}

impl Stream {
    pub fn new(size: Size, next: Record) -> Self {
        let geometry = size.geometry();
        let mut projection = TerminalProjection::new(attachment(), epoch());
        applied(
            projection.receive_for_geometry(&Record::snapshot(size, 1, 1, 0).bytes, Some(geometry)),
        );
        Self {
            projection,
            geometry,
            next,
            sequence: 1,
            revision: 1,
        }
    }

    pub fn receive(&mut self, delta: bool) {
        let base = delta.then_some(self.revision);
        self.sequence += 1;
        self.revision += 1;
        self.next.advance(self.sequence, base, self.revision);
        applied(
            self.projection
                .receive_for_geometry(&self.next.bytes, Some(self.geometry)),
        );
    }
}

pub fn applied<E: std::fmt::Debug>(feedback: Result<Option<TerminalViewerAction>, E>) {
    assert!(
        matches!(
            feedback,
            Ok(Some(TerminalViewerAction::FrameApplied { .. }))
        ),
        "benchmark frame was not applied: {feedback:?}"
    );
}
