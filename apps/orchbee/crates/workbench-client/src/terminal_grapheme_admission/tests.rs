use std::time::Instant;

use super::{Probe, Query};
use crate::{
    terminal_events::{DecodedKind, HostReply, LegacyDecoder, ModeReportState},
    terminal_pointer_admission::QUERY_DEADLINE,
};

/// The 2027 report the real decoder produces for the outer terminal's bytes.
fn grapheme_report(bytes: &[u8]) -> Option<ModeReportState> {
    let mut decoder = LegacyDecoder::default();
    let now = Instant::now();
    bytes
        .iter()
        .flat_map(|byte| decoder.push(*byte, now, false))
        .find_map(|item| match item.kind {
            DecodedKind::HostReply(HostReply::Grapheme(state)) => Some(state),
            _ => None,
        })
}

#[test]
fn only_the_first_reset_report_admits_the_enable_once() {
    let cases: [(&[u8], bool); 5] = [
        (&b"\x1b[?2027;2$y"[..], true),
        (&b"\x1b[?2027;0$y"[..], false),
        (&b"\x1b[?2027;1$y"[..], false),
        (&b"\x1b[?2027;3$y"[..], false),
        (&b"\x1b[?2027;4$y"[..], false),
    ];
    for (bytes, admits) in cases {
        let now = Instant::now();
        let mut probe = Probe::default();
        assert_eq!(probe.begin(now), Some(Query));
        assert_eq!(probe.begin(now), None, "the query is issued once");

        let report = grapheme_report(bytes).expect("the decoder yields the 2027 report");
        probe.observe(report, now);
        assert_eq!(probe.take_enable(), admits, "{bytes:?}");
        assert!(!probe.take_enable(), "an admitted enable is taken once");

        // The first report settled the probe, so a later Reset cannot reopen admission.
        probe.observe(ModeReportState::Reset, now);
        assert!(
            !probe.take_enable(),
            "a late report never reopens admission"
        );
    }

    // A Reset report that arrives after the reply window never admits the enable.
    let now = Instant::now();
    let mut probe = Probe::default();
    assert_eq!(probe.begin(now), Some(Query));
    probe.observe(ModeReportState::Reset, now + QUERY_DEADLINE);
    assert!(
        !probe.take_enable(),
        "the window closed before the report arrived"
    );
}
