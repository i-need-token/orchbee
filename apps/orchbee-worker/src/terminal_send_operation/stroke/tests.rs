use super::*;

#[test]
fn preserves_logical_text_and_modifiers_for_host_encoding() {
    for (value, flags, code, text) in [
        (' ', 0, "Space", " "),
        ('a', 4, "KeyA", "A"),
        (';', 4, "Semicolon", ":"),
        ('+', 8, "Equal", "+"),
        ('中', 0, "Unidentified", "中"),
        ('😀', 10, "Unidentified", "😀"),
    ] {
        let key = input(
            SendKeyStroke::new(SendStrokeKey::Character(value), flags).unwrap(),
            AttachmentId::from_bytes([1; 16]).unwrap(),
            NonZeroU64::new(1).unwrap(),
        )
        .unwrap();
        assert_eq!(key.key.as_str(), code);
        assert_eq!(key.text, text);
        assert_eq!(key.unshifted_codepoint, u32::from(value));
        assert_eq!(key.modifiers.meta, flags & 8 != 0);
        assert_eq!(key.modifiers.alt, flags & 2 != 0);
        assert_eq!(key.modifiers.shift, flags & 4 != 0);
    }
}
