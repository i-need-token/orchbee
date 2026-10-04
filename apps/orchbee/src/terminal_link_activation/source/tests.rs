use std::num::NonZeroU64;

use super::PaintedRows;

#[test]
fn point_maps_live_rows_and_excludes_history_chrome() {
    let mut rows = PaintedRows {
        attachment: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
        host_epoch: orchbee_terminal_protocol::HostEpoch::from_bytes([2; 16]).unwrap(),
        sequence: NonZeroU64::MIN,
        generation: NonZeroU64::MIN,
        revision: 3,
        columns: 80,
        total_rows: 120,
        start_row: 96,
        row_count: 24,
        history: false,
    };
    assert_eq!(rows.point(7, 2), Some((7, 98)));
    assert_eq!(rows.point(80, 2), None);
    rows.history = true;
    assert_eq!(rows.point(7, 0), None);
    assert_eq!(rows.point(7, 23), None);
    assert_eq!(rows.point(7, 2), Some((7, 98)));
}
