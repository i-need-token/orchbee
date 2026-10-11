use super::*;

#[test]
fn stroke_wire_preserves_scalar_and_all_modifier_classes() {
    for flags in 0..=15 {
        for (key, kind, value) in [
            (SendStrokeKey::Named(SendKey::Tab), 1, 3_u32),
            (SendStrokeKey::Insert, 2, 0),
            (SendStrokeKey::Function(25), 3, 25),
            (SendStrokeKey::Character(' '), 4, 32),
            (SendStrokeKey::Character('😀'), 4, 0x1_f600),
        ] {
            let stroke = SendKeyStroke::new(key, flags).unwrap();
            let [a, b, c, d] = value.to_be_bytes();
            let wire = [flags, kind, a, b, c, d];
            assert_eq!(stroke.to_wire(), wire);
            assert_eq!(SendKeyStroke::from_wire(wire).unwrap(), stroke);
        }
    }
    for (flags, kind, value) in [
        (16, 4, 32_u32),
        (0, 0, 0),
        (0, 2, 1),
        (0, 3, 0),
        (0, 3, 26),
        (0, 1, 14),
        (0, 1, 256),
        (0, 4, 0),
        (0, 4, 0x85),
        (0, 4, 0xd800),
        (0, 4, 0x11_0000),
    ] {
        let [a, b, c, d] = value.to_be_bytes();
        assert!(SendKeyStroke::from_wire([flags, kind, a, b, c, d]).is_err());
    }
}
