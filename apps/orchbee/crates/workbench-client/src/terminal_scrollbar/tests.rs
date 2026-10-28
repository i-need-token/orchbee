//! Bounds and endpoint evidence over integer terminal-cell geometry.
use super::*;

fn sample(maximum: u32, offset: u32, rows: u16) -> ScrollbarSnapshot {
    ScrollbarSnapshot::new(
        AttachmentId::from_bytes([1; 16]).unwrap(),
        HostEpoch::from_bytes([2; 16]).unwrap(),
        NonZeroU64::new(1).unwrap(),
        (80, rows),
        rows,
        u64::from(maximum) + u64::from(rows),
        u64::from(maximum - offset),
    )
    .unwrap()
}

#[test]
fn scrollbar_integer_bounds_and_endpoints_survive_small_and_large_histories() {
    for maximum in [1, 2, 80, 100_000, u32::MAX] {
        for rows in [1, 2, 24, u16::MAX] {
            for height in 2..80 {
                let track = Rect::new(83, 7, 1, height);
                let bottom = sample(maximum, 0, rows).thumb(track).unwrap();
                let top = sample(maximum, maximum, rows).thumb(track).unwrap();
                assert_eq!(top.y, track.y);
                assert_eq!(bottom.bottom(), track.bottom());
                assert!(top.height > 0 && top.height < height);
                let snapshot = sample(maximum, maximum / 2, rows);
                let thumb = snapshot.thumb(track).unwrap();
                let travel = height - thumb.height;
                if maximum >= u32::from(travel) {
                    for top in 0..=travel {
                        let mut moved = snapshot;
                        moved.offset = snapshot.offset_at(track, track.y + top, 0).unwrap();
                        assert_eq!(
                            moved.thumb(track).unwrap().y,
                            track.y + top,
                            "a representable dragged thumb stays at the grabbed row"
                        );
                    }
                }
                for grab in 0..thumb.height {
                    assert_eq!(snapshot.offset_at(track, 0, grab), Some(maximum));
                    assert_eq!(snapshot.offset_at(track, u16::MAX, grab), Some(0));
                    let offsets: Vec<_> = (0..=height + 7)
                        .map(|y| snapshot.offset_at(track, y, grab).unwrap())
                        .collect();
                    assert!(offsets.windows(2).all(|pair| pair[0] >= pair[1]));
                    assert!(offsets.iter().all(|offset| *offset <= maximum));
                }
            }
        }
    }
}

#[test]
fn scrollbar_thumb_uses_served_rows_when_a_window_is_shorter_than_the_pane() {
    let source = sample(100, 0, 24);
    let short = ScrollbarSnapshot::new(
        source.attachment,
        source.epoch,
        source.generation,
        (80, 24),
        12,
        124,
        112,
    )
    .unwrap();
    let track = Rect::new(80, 0, 1, 24);
    assert_eq!(short.total_rows(), 124);
    assert_eq!(short.thumb(track).unwrap().bottom(), 24);
    assert!(short.thumb(track).unwrap().height < source.thumb(track).unwrap().height);
    assert!(!short.same_source(source));
}

#[test]
fn scrollbar_refuses_unknown_unaddressable_and_degenerate_geometry() {
    let source = sample(100, 0, 24);
    for (total, start) in [(0, 0), (23, 0), (24, 0), (25, 2), (u64::MAX, 0)] {
        assert!(
            ScrollbarSnapshot::new(
                source.attachment,
                source.epoch,
                source.generation,
                (80, 24),
                24,
                total,
                start
            )
            .is_none()
        );
    }
    for track in [
        Rect::new(0, 0, 1, 0),
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 0, 24),
    ] {
        assert_eq!(source.thumb(track), None);
        assert_eq!(source.offset_at(track, 0, 0), None);
    }
}
