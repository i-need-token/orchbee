//! The byte-level contract: every row of the design's encoding table is one
//! expectation, cross-checked against pinned ghostty-vt `mouse_encode.zig`,
//! and the Host's `input_encoding` remap.

use super::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PanePointerRoute, decode_sgr,
    decode_x10, encode_cursor_arrows, encode_pane_report, route_pane_pointer,
};
use crate::client_view::Modifiers;
use orchbee_terminal_protocol::{MouseFormat, MouseTracking};

fn event(
    action: MouseAction,
    button: Option<MouseButton>,
    column: i32,
    row: i32,
) -> PaneMouseEvent {
    PaneMouseEvent {
        action,
        button,
        mods: Modifiers::NONE,
        column,
        row,
    }
}

fn encode(
    event: &PaneMouseEvent,
    tracking: MouseTracking,
    format: MouseFormat,
    last_cell: &mut Option<(u16, u16)>,
) -> PaneEncode {
    encode_pane_report(event, tracking, format, (80, 24), false, last_cell)
}

fn bytes(encoded: PaneEncode) -> Vec<u8> {
    match encoded {
        PaneEncode::Bytes(bytes) => bytes,
        other => panic!("expected bytes, got {other:?}"),
    }
}

#[test]
fn decode_sgr_reports_all_event_shapes() {
    let press = decode_sgr(b"0;35;20M").expect("press");
    assert_eq!(
        press,
        MouseEvent {
            action: MouseAction::Press,
            button: MouseButton::Left,
            mods: Modifiers::NONE,
            column: 34,
            row: 19,
        }
    );
    // Release keeps the button identity (cb 2 = right) on `m`.
    let release = decode_sgr(b"2;5;6m").expect("release");
    assert_eq!(release.action, MouseAction::Release);
    assert_eq!(release.button, MouseButton::Right);
    assert_eq!((release.column, release.row), (4, 5));
    // Motion bit 32 plus button 0; modifier bits 4/8/16.
    let drag = decode_sgr(b"60;10;10M").expect("drag");
    assert_eq!(drag.action, MouseAction::Motion);
    assert_eq!(drag.button, MouseButton::Left);
    assert!(drag.mods.shift && drag.mods.alt && drag.mods.ctrl && !drag.mods.super_);
    // Wheel directions and extended buttons.
    assert_eq!(decode_sgr(b"64;1;1M").unwrap().button, MouseButton::WheelUp);
    assert_eq!(
        decode_sgr(b"65;1;1M").unwrap().button,
        MouseButton::WheelDown
    );
    assert_eq!(
        decode_sgr(b"66;1;1M").unwrap().button,
        MouseButton::WheelLeft
    );
    assert_eq!(
        decode_sgr(b"67;1;1M").unwrap().button,
        MouseButton::WheelRight
    );
    assert_eq!(
        decode_sgr(b"130;1;1M").unwrap().button,
        MouseButton::Extended(2)
    );
    // Hover motion (cb 35 = 32 + 3) has no button.
    let hover = decode_sgr(b"35;2;3M").expect("hover");
    assert_eq!(hover.action, MouseAction::Motion);
    assert_eq!(hover.button, MouseButton::None);
}

#[test]
fn decode_rejects_malformed_reports_atomically() {
    for body in [
        &b""[..],
        b"0;35;20",
        b"0;35;20;1M",
        b"0;35M",
        b"0;0;20M", // coordinates are 1-based
        b"0;35;0M",
        b"256;1;1M",   // cb fits u8 only
        b"0;65536;1M", // coordinate overflow rejects, never wraps
        b"0;99999;1M",
        b"0;1x;1M",
        b"0;;1M",
        b"0;1;1x",
        b"0;1;1Mx",
        b"0;1;1mM",
        b";1;1;1M",
        b"0x0000;1;1M",
    ] {
        assert_eq!(decode_sgr(body), None, "accepted {body:?}");
    }
    assert_eq!(
        decode_sgr(b"0;65535;65535M").map(|e| (e.column, e.row)),
        Some((65534, 65534))
    );
    for body in [&b""[..], b"\x20", b"\x20\x21", b"\x20\x21\x22\x21"] {
        assert_eq!(decode_x10(body), None, "x10 accepted {body:?}");
    }
    // X10 offsets below 32/33 are malformed, not wrapped.
    assert_eq!(decode_x10(b"\x1f\x21\x21"), None);
    assert_eq!(decode_x10(b"\x20\x20\x21"), None);
    let x10 = decode_x10(b"\x20\x23\x26").expect("x10");
    assert_eq!(x10.action, MouseAction::Press);
    assert_eq!(x10.button, MouseButton::Left);
    assert_eq!((x10.column, x10.row), (2, 5));
}

/// The §9 contract table, row by row.
#[test]
fn contract_rows() {
    let mut last = None;
    // 1. X10 tracking strips modifiers: press left at (0,0) s+a+c.
    let mut e = event(MouseAction::Press, Some(MouseButton::Left), 0, 0);
    e.mods = Modifiers::NONE.with_shift().with_alt().with_ctrl();
    assert_eq!(
        bytes(encode(
            &e,
            MouseTracking::X10,
            MouseFormat::Default,
            &mut last
        )),
        vec![0x1b, b'[', b'M', 32, 33, 33]
    );
    // 2. X10 never reports releases.
    assert_eq!(
        encode(
            &event(MouseAction::Release, Some(MouseButton::Left), 0, 0),
            MouseTracking::X10,
            MouseFormat::Sgr,
            &mut last
        ),
        PaneEncode::Silent
    );
    // 3. Normal tracking drops motion.
    assert_eq!(
        encode(
            &event(MouseAction::Motion, Some(MouseButton::Left), 0, 0),
            MouseTracking::Normal,
            MouseFormat::Sgr,
            &mut last
        ),
        PaneEncode::Silent
    );
    // 4. Button tracking requires a button.
    assert_eq!(
        encode(
            &event(MouseAction::Motion, None, 0, 0),
            MouseTracking::Button,
            MouseFormat::Sgr,
            &mut last
        ),
        PaneEncode::Silent
    );
    // 5. SGR release keeps button identity.
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Release, Some(MouseButton::Right), 4, 5),
            MouseTracking::Any,
            MouseFormat::Sgr,
            &mut last
        )),
        b"\x1b[<2;5;6m"
    );
    // 6. SGR hover motion: null button → 3, +32 motion.
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Motion, None, 1, 2),
            MouseTracking::Any,
            MouseFormat::Sgr,
            &mut last
        )),
        b"\x1b[<35;2;3M"
    );
    // 7. URXVT press with s+a+c: 32 + (0 + 4 + 8 + 16) = 60.
    let mut e = event(MouseAction::Press, Some(MouseButton::Left), 2, 3);
    e.mods = Modifiers::NONE.with_shift().with_alt().with_ctrl();
    assert_eq!(
        bytes(encode(
            &e,
            MouseTracking::Any,
            MouseFormat::Urxvt,
            &mut last
        )),
        b"\x1b[60;3;4M"
    );
    // 8. URXVT release collapses to legacy button 3 (35).
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Release, Some(MouseButton::Right), 2, 3),
            MouseTracking::Any,
            MouseFormat::Urxvt,
            &mut last
        )),
        b"\x1b[35;3;4M"
    );
    // 9. UTF-8 encodes large coordinates as codepoints 33 + cell; the
    //    viewport is wide enough that the cell is in bounds.
    let out = bytes(encode_pane_report(
        &event(MouseAction::Press, Some(MouseButton::Left), 300, 400),
        MouseTracking::Any,
        MouseFormat::Utf8,
        (400, 500),
        false,
        &mut last,
    ));
    assert_eq!(&out[..4], &[0x1b, b'[', b'M', 32]);
    let text = std::str::from_utf8(&out[4..]).expect("utf8 tail");
    let mut chars = text.chars();
    assert_eq!(chars.next().map(|c| c as u32), Some(333));
    assert_eq!(chars.next().map(|c| c as u32), Some(433));
    assert_eq!(chars.next(), None);
    // 10. Legacy byte form rejects cells beyond 222 — drop, never wrap.
    //     The bound applies to the clamped cell, so the viewport itself
    //     must be wider than 222 columns for it to engage.
    assert_eq!(
        encode_pane_report(
            &event(MouseAction::Press, Some(MouseButton::Left), 250, 0),
            MouseTracking::Any,
            MouseFormat::Default,
            (300, 24),
            false,
            &mut last
        ),
        PaneEncode::Silent
    );
    // 11. Wheel directions encode 64-67.
    for (button, code) in [
        (MouseButton::WheelUp, 64),
        (MouseButton::WheelDown, 65),
        (MouseButton::WheelLeft, 66),
        (MouseButton::WheelRight, 67),
    ] {
        assert_eq!(
            bytes(encode(
                &event(MouseAction::Press, Some(button), 0, 0),
                MouseTracking::Any,
                MouseFormat::Sgr,
                &mut last
            )),
            format!("\x1b[<{code};1;1M").into_bytes()
        );
    }
    // 12. Extended buttons ten/eleven net to 130/131 (Host remap effect).
    for (n, code) in [(2, 130), (3, 131)] {
        assert_eq!(
            bytes(encode(
                &event(MouseAction::Press, Some(MouseButton::Extended(n)), 0, 0),
                MouseTracking::Any,
                MouseFormat::Sgr,
                &mut last
            )),
            format!("\x1b[<{code};1;1M").into_bytes()
        );
    }
    // 13. Out-of-viewport motion: dropped without a held button, clamped
    //     to the edge cell with one. The dedup cell resets first — a
    //     motion into the previously reported cell is silent.
    assert_eq!(
        encode(
            &event(MouseAction::Motion, Some(MouseButton::Left), -1, -1),
            MouseTracking::Any,
            MouseFormat::Sgr,
            &mut last
        ),
        PaneEncode::Silent
    );
    last = None;
    assert_eq!(
        bytes(encode_pane_report(
            &event(MouseAction::Motion, Some(MouseButton::Left), -1, -1),
            MouseTracking::Any,
            MouseFormat::Sgr,
            (80, 24),
            true,
            &mut last
        )),
        b"\x1b[<32;1;1M"
    );
    // 14. Same-cell motion dedups on the second report.
    let motion = event(MouseAction::Motion, Some(MouseButton::Left), 5, 6);
    assert!(matches!(
        encode(&motion, MouseTracking::Any, MouseFormat::Sgr, &mut last),
        PaneEncode::Bytes(_)
    ));
    assert_eq!(
        encode(&motion, MouseTracking::Any, MouseFormat::Sgr, &mut last),
        PaneEncode::Silent
    );
    // 15. Legacy release collapses to button 3 → byte 35.
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Release, Some(MouseButton::Right), 0, 0),
            MouseTracking::Any,
            MouseFormat::Default,
            &mut last
        )),
        vec![0x1b, b'[', b'M', 35, 33, 33]
    );
    // 16. Wheel → cursor arrows: three per tick, SS3 vs CSI.
    assert_eq!(encode_cursor_arrows(true, 3, true), b"\x1bOA\x1bOA\x1bOA");
    assert_eq!(encode_cursor_arrows(false, 3, false), b"\x1b[B\x1b[B\x1b[B");
    // 17. Cell input uses the SGR cell fallback even in pixel format.
    assert_eq!(
        encode(
            &event(MouseAction::Press, Some(MouseButton::Left), 0, 0),
            MouseTracking::Any,
            MouseFormat::SgrPixels,
            &mut last
        ),
        PaneEncode::Bytes(b"\x1b[<0;1;1M".to_vec())
    );
}

#[test]
fn out_of_viewport_rules_match_the_host_gate() {
    let mut last = None;
    // Release outside the viewport still reports, clamped to the edge.
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Release, Some(MouseButton::Left), 200, 50),
            MouseTracking::Any,
            MouseFormat::Sgr,
            &mut last
        )),
        b"\x1b[<0;80;24m"
    );
    // Press outside under non-motion tracking drops.
    assert_eq!(
        encode(
            &event(MouseAction::Press, Some(MouseButton::Left), 200, 0),
            MouseTracking::Normal,
            MouseFormat::Sgr,
            &mut last
        ),
        PaneEncode::Silent
    );
    // Press outside under motion tracking reports only with a button held —
    // the press itself counts as held.
    assert_eq!(
        bytes(encode_pane_report(
            &event(MouseAction::Press, Some(MouseButton::Left), 200, 0),
            MouseTracking::Button,
            MouseFormat::Sgr,
            (80, 24),
            true,
            &mut last
        )),
        b"\x1b[<0;80;1M"
    );
    // A zero-sized viewport has no in-bounds cell, but the Host gate
    // still reports a held-button event clamped to cell (0,0) — the same
    // clamp the pixel pipeline performs.
    assert_eq!(
        bytes(encode_pane_report(
            &event(MouseAction::Press, Some(MouseButton::Left), 0, 0),
            MouseTracking::Any,
            MouseFormat::Sgr,
            (0, 0),
            true,
            &mut last
        )),
        b"\x1b[<0;1;1M"
    );
}

#[test]
fn fresh_routing_matches_tracking_matrix_without_adopting_gesture_tails() {
    let input = |action, button| MouseEvent {
        action,
        button,
        mods: Modifiers::NONE,
        column: 0,
        row: 0,
    };
    let events = [
        input(MouseAction::Press, MouseButton::Left),
        input(MouseAction::Press, MouseButton::WheelUp),
        input(MouseAction::Motion, MouseButton::Left),
        input(MouseAction::Motion, MouseButton::None),
        input(MouseAction::Release, MouseButton::Left),
        input(MouseAction::Release, MouseButton::None),
    ];
    let matrix = [
        (
            MouseTracking::None,
            [
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
            ],
        ),
        (
            MouseTracking::X10,
            [
                PanePointerRoute::ProgramReport,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
            ],
        ),
        (
            MouseTracking::Normal,
            [
                PanePointerRoute::ProgramReport,
                PanePointerRoute::ProgramReport,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
            ],
        ),
        (
            MouseTracking::Button,
            [
                PanePointerRoute::ProgramReport,
                PanePointerRoute::ProgramReport,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
            ],
        ),
        (
            MouseTracking::Any,
            [
                PanePointerRoute::ProgramReport,
                PanePointerRoute::ProgramReport,
                PanePointerRoute::Client,
                PanePointerRoute::ProgramReport,
                PanePointerRoute::Client,
                PanePointerRoute::Client,
            ],
        ),
    ];
    for (tracking, expected) in matrix {
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                route_pane_pointer(tracking, event, false),
                expected[index],
                "{tracking:?} event {index}"
            );
        }
    }

    let vertical_wheel = events[1];
    assert_eq!(
        route_pane_pointer(MouseTracking::None, &vertical_wheel, true),
        PanePointerRoute::AlternateScroll
    );
    assert_eq!(
        route_pane_pointer(MouseTracking::X10, &vertical_wheel, true),
        PanePointerRoute::Client,
        "DEC 1007 cannot override an active tracking mode"
    );
    let horizontal_wheel = input(MouseAction::Press, MouseButton::WheelLeft);
    assert_eq!(
        route_pane_pointer(MouseTracking::None, &horizontal_wheel, true),
        PanePointerRoute::Client
    );
    let no_button_press = input(MouseAction::Press, MouseButton::None);
    assert_eq!(
        route_pane_pointer(MouseTracking::Any, &no_button_press, false),
        PanePointerRoute::Client
    );

    for tracking in [
        MouseTracking::None,
        MouseTracking::X10,
        MouseTracking::Normal,
        MouseTracking::Button,
        MouseTracking::Any,
    ] {
        let shifted = MouseEvent {
            mods: Modifiers::NONE.with_shift(),
            ..events[0]
        };
        assert_eq!(
            route_pane_pointer(tracking, &shifted, true),
            PanePointerRoute::Host
        );
    }
}

#[test]
fn pixel_request_preserves_cell_fallback_tracking_release_and_dedup() {
    // Cell-coordinate oracle: cell (4,6) produces (5,7),
    // not its separately tested exact pixel position (48,139).
    let mut last = None;
    let hover = event(MouseAction::Motion, None, 4, 6);
    assert_eq!(
        bytes(encode(
            &hover,
            MouseTracking::Any,
            MouseFormat::SgrPixels,
            &mut last
        )),
        b"\x1b[<35;5;7M"
    );
    assert_eq!(
        encode(
            &hover,
            MouseTracking::Any,
            MouseFormat::SgrPixels,
            &mut last
        ),
        PaneEncode::Silent
    );
    let mut release = event(MouseAction::Release, Some(MouseButton::Right), 4, 6);
    release.mods = Modifiers::NONE.with_ctrl().with_alt();
    assert_eq!(
        bytes(encode(
            &release,
            MouseTracking::Normal,
            MouseFormat::SgrPixels,
            &mut last
        )),
        b"\x1b[<26;5;7m"
    );
    for tracking in [
        MouseTracking::None,
        MouseTracking::X10,
        MouseTracking::Normal,
        MouseTracking::Button,
    ] {
        assert_eq!(
            encode(&hover, tracking, MouseFormat::SgrPixels, &mut None),
            PaneEncode::Silent
        );
    }
    assert_eq!(
        bytes(encode(
            &event(MouseAction::Release, Some(MouseButton::Left), 200, -1),
            MouseTracking::Any,
            MouseFormat::SgrPixels,
            &mut None
        )),
        b"\x1b[<0;80;1m"
    );
}
