//! Probe ownership, real reply framing and literal input preservation witnesses.
use super::*;

fn dimensions(columns: u16, rows: u16) -> WindowDimensions {
    WindowDimensions {
        columns,
        rows,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[test]
fn current_host_measurements_never_advertise_an_unused_pixel_renderer() {
    let now = Instant::now();
    let mut probe = Probe::default();
    assert!(probe.query((80, 24), now).is_none());
    probe.activate(31);
    let query = probe.query((80, 24), now).expect("startup query");
    assert_eq!(
        query.sequence(),
        "\x1b[16t\x1b_Ga=q,t=d,f=32,s=1,v=1,i=31;AAAAAA==\x1b\\"
    );
    assert!(probe.query((80, 24), now).is_none());
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 32,
            success: true,
        },
        now,
    );
    probe.reply(
        Reply::Kitty {
            image_id: 31,
            placement_id: Some(1),
            success: true,
        },
        now,
    );
    assert!(
        !probe.facts().kitty,
        "placement acknowledgement cannot answer the probe"
    );
    assert!(!probe.facts().kitty);
    probe.reply(Reply::Cell(Some((8, 16))), now + Duration::from_millis(10));
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 31,
            success: true,
        },
        now + Duration::from_millis(20),
    );
    let facts = probe.facts();
    assert!(facts.kitty);
    let caps = facts.cell_capabilities();
    assert_eq!(caps.transport, PaneGraphicsTransport::CellRaster);
    assert_eq!(
        caps.cell_pixels,
        Some(PaneGraphicsCellPixels {
            width: 8,
            height: 16
        })
    );
    assert!(!caps.pixel_mouse);
    let measured = facts
        .measurement
        .expect("real pixels")
        .apply(dimensions(80, 24));
    assert_eq!((measured.pixel_width, measured.pixel_height), (640, 384));
    assert_eq!(
        facts.measurement.unwrap().apply(dimensions(81, 24)),
        dimensions(81, 24)
    );
    assert_eq!(
        probe.query((81, 24), now + Duration::from_millis(30)),
        Some(Query {
            cell: true,
            image_id: None
        })
    );
    assert!(probe.facts().measurement.is_none());
    probe.reply(
        Reply::Cell(Some((u16::MAX, 16))),
        now + Duration::from_millis(40),
    );
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((81, 24), now + Duration::from_millis(41))
            .is_none()
    );
}

#[test]
fn resize_and_timeout_keep_the_uncorrelated_reply_debt_until_its_late_response() {
    let now = Instant::now();
    let mut probe = Probe::default();
    probe.activate(77);
    assert!(probe.query((80, 24), now).is_some());
    assert!(
        probe
            .query((100, 30), now + Duration::from_millis(10))
            .is_none()
    );
    probe.reply(Reply::Cell(Some((8, 16))), now + Duration::from_millis(20));
    assert!(probe.facts().measurement.is_none());
    assert_eq!(
        probe.query((100, 30), now + Duration::from_millis(21)),
        Some(Query {
            cell: true,
            image_id: None
        })
    );
    probe.reply(Reply::Cell(Some((9, 18))), now + Duration::from_millis(22));
    assert_eq!(
        probe
            .facts()
            .measurement
            .unwrap()
            .apply(dimensions(100, 30))
            .pixel_width,
        900
    );
    probe.reply(
        Reply::Kitty {
            placement_id: None,
            image_id: 77,
            success: true,
        },
        now + QUERY_DEADLINE,
    );
    assert!(!probe.facts().kitty);
    assert!(
        probe
            .query((101, 30), now + Duration::from_secs(1))
            .is_some()
    );
    assert!(
        probe
            .query((102, 30), now + Duration::from_secs(5))
            .is_none()
    );
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(500))
            .is_none()
    );
    probe.reply(Reply::Cell(Some((10, 20))), now + Duration::from_secs(501));
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(502))
            .is_some()
    );
    probe.reply(Reply::Cell(None), now + Duration::from_secs(502));
    assert!(probe.facts().measurement.is_none());
    assert!(
        probe
            .query((103, 30), now + Duration::from_secs(502))
            .is_none()
    );
}
