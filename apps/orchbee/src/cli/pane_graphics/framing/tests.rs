//! Reference raw-frame grammar, closed header bounds and complete-file compatibility.
use super::*;
#[test]
fn fragmented_binary_frames_pin_full_sequences_and_reject_truncation_without_partial_image() {
    let mut parser = Parser::new();
    let mut bytes = b"{\"format\":\"rgba\",\"width\":1,\"height\":1,\"dataLength\":4,\"sequence\":\"18446744073709551614\",\"revision\":\"18446744073709551615\",\"placement\":{\"gridColumns\":4294967295,\"gridRows\":4294967295}}\r\n".to_vec();
    bytes.extend([0, 10, 255, 128]);
    let mut frames = Vec::new();
    for byte in bytes {
        parser.feed(&[byte]).unwrap();
        if let Some(frame) = parser.next().unwrap() {
            parser.accept(frame.sequence);
            frames.push(frame);
        }
    }
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].sequence.value(), u64::MAX - 1);
    assert_eq!(frames[0].revision.value(), u64::MAX);
    let PaneGraphicsStreamImage::Inline { image } = &frames[0].image else {
        panic!("inline image");
    };
    assert_eq!(image.placement.grid_columns, u32::MAX);
    assert_eq!(image.placement.grid_rows, u32::MAX);
    parser.finish().unwrap();
    parser
        .feed(b"{\"format\":\"rgba\",\"width\":1,\"height\":1,\"dataLength\":4}\n\0")
        .unwrap();
    assert!(parser.next().unwrap().is_none());
    assert_eq!(parser.finish(), Err("truncated-frame"));
}
#[test]
fn header_bounds_and_sizes_are_checked_before_body_allocation() {
    for raw in [
        r#"{"format":"rgba","width":1,"height":1,"dataLength":5}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"format":"bgra"}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"unknown":0}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"sequence":"0"}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"sequence":9007199254740992}"#,
        r#"{"format":"png","width":4096,"height":4096,"dataLength":16777217}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"damage":[{"x":1,"y":0,"width":1,"height":1}]}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"damage":{"x":1,"y":0,"width":1,"height":1}}"#,
        r#"{"format":"rgba","width":1,"height":1,"dataLength":4,"transport":"pretend-kitty"}"#,
    ] {
        let mut parser = Parser::new();
        parser.feed(format!("{raw}\n").as_bytes()).unwrap();
        assert!(parser.next().is_err(), "{raw}");
        assert!(parser.body.is_none());
    }
    let mut parser = Parser::new();
    for chunk in vec![b'x'; MAXIMUM_HEADER].chunks(8192) {
        parser.feed(chunk).unwrap();
        assert!(parser.next().unwrap().is_none());
    }
    parser.feed(b"x\n").unwrap();
    assert_eq!(parser.next().err(), Some("header-too-large"));
}
#[test]
fn reference_browser_file_header_normalizes_damage_without_manufacturing_a_transport() {
    let mut parser = Parser::new();
    parser.feed(br#"{"format":"rgba","width":2,"height":3,"sequence":7,"revision":8,"file":{"path":"C:\\private\\frame"},"damage":{"x":1,"y":1,"width":1,"height":1},"transport":"direct-kitty","placement":{"gridColumns":2,"gridRows":3}}
"#).unwrap();
    let frame = parser.next().unwrap().unwrap();
    assert_eq!(frame.sequence.value(), 7);
    assert_eq!(frame.revision.value(), 8);
    assert_eq!(
        frame.damage,
        [PaneGraphicsDamage {
            x: 1,
            y: 1,
            width: 1,
            height: 1
        }]
    );
    assert!(
        matches!(frame.image, PaneGraphicsStreamImage::File { width: 2, height: 3, placement, .. } if placement.grid_columns == 2 && placement.grid_rows == 3)
    );
    assert!(parser.next().unwrap().is_none());
    parser.finish().unwrap();
}
