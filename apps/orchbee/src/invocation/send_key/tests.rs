use super::*;

#[test]
fn preserves_semantic_identity_without_encoding_terminal_bytes() {
    for (input, key, flags) in [
        ("f1", SendStrokeKey::Function(1), 0),
        ("f25", SendStrokeKey::Function(25), 0),
        ("space", SendStrokeKey::Character(' '), 0),
        ("plus", SendStrokeKey::Character('+'), 0),
        ("super+plus", SendStrokeKey::Character('+'), 8),
        ("super+C", SendStrokeKey::Character('c'), 12),
        (
            "ctrl+alt+shift+super+中",
            SendStrokeKey::Character('中'),
            15,
        ),
        ("😀", SendStrokeKey::Character('😀'), 0),
        ("shift+semicolon", SendStrokeKey::Character(';'), 4),
        ("ctrl+c", SendStrokeKey::Character('c'), 1),
    ] {
        assert_eq!(parse(input), SendKeyStroke::new(key, flags).ok(), "{input}");
    }
    for input in [
        "",
        "ctrl",
        "ctrl+control+a",
        "ctrl++",
        "a+b",
        "f0",
        "f26",
        "a\0",
        "\u{0085}",
        "雪人",
        "super+ctrl-c",
        "ctrl-c",
        "ctrl-d",
        "C-c",
        "cmd+c",
        "control+c",
        "F25",
        "esc",
        " ctrl+c",
    ] {
        assert_eq!(parse(input), None, "{input:?}");
    }
}
