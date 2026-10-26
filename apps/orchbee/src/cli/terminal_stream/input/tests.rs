use super::*;
#[cfg(windows)]
mod windows;

#[test]
fn commands_are_closed_bounded_and_preserve_opaque_input() {
    assert_eq!(
        parse(br#"{"type":"terminal.input","bytes":"AP8bDQ=="}"#),
        Ok(Command::Input(vec![0, 255, 27, 13]))
    );
    assert_eq!(
        parse(br#"{"type":"terminal.input","text":""}"#),
        Ok(Command::Input(Vec::new()))
    );
    assert!(matches!(
        parse(br#"{"type":"terminal.resize","cols":120,"rows":40}"#),
        Ok(Command::Resize(_))
    ));
    assert!(matches!(parse(br#"{"type":"terminal.scroll","direction":"up","lines":65535,"column":2,"modifiers":63}"#),Ok(Command::Scroll(_))));
    for bad in [
        r#"{"type":"terminal.release","extra":1}"#,
        r#"{"type":"terminal.release","type":"terminal.release"}"#,
        r#"{"type":"terminal.input"}"#,
        r#"{"type":"terminal.input","text":null}"#,
        r#"{"type":"terminal.input","text":"a","bytes":"YQ=="}"#,
        r#"{"type":"terminal.input","bytes":"!"}"#,
        r#"{"type":"terminal.resize","cols":4096,"rows":4096}"#,
        r#"{"type":"terminal.resize","cols":80,"rows":24,"cell_width_px":8}"#,
        r#"{"type":"terminal.scroll","direction":"down","lines":0}"#,
        r#"{"type":"terminal.scroll","direction":"down","lines":1,"modifiers":64}"#,
        r#"{"type":"terminal.scroll","direction":"down","lines":1,"source":"unknown"}"#,
    ] {
        assert!(parse(bad.as_bytes()).is_err(), "{bad}");
    }
    for count in [MAX_INPUT, MAX_INPUT + 1] {
        let line = serde_json::to_vec(
            &serde_json::json!({"type":"terminal.input","text":"x".repeat(count)}),
        )
        .unwrap();
        assert_eq!(parse(&line).is_ok(), count == MAX_INPUT);
    }
    assert!(parse(&vec![b' '; MAX_LINE + 1]).is_err());
    assert!(parse(b"\xff").is_err());
}
#[test]
fn input_schema_and_parser_agree_on_public_records_and_reject_unknown_shape() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../schemas/orchbee.terminal.control.input.json"
    ))
    .unwrap();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for (record, accepted) in [
        (
            serde_json::json!({"type":"terminal.input","text":"hello\n世界"}),
            true,
        ),
        (
            serde_json::json!({"type":"terminal.input","bytes":"AP8bDQ=="}),
            true,
        ),
        (
            serde_json::json!({"type":"terminal.input","text":"a","bytes":"YQ=="}),
            false,
        ),
        (
            serde_json::json!({"type":"terminal.resize","cols":80,"rows":24,"cell_width_px":8,"cell_height_px":16}),
            true,
        ),
        (
            serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":63}),
            true,
        ),
        (
            serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":0.5}),
            false,
        ),
        (
            serde_json::json!({"type":"terminal.scroll","direction":"down","lines":3,"modifiers":"1"}),
            false,
        ),
        (serde_json::json!({"type":"terminal.release"}), true),
        (
            serde_json::json!({"type":"terminal.release","extra":null}),
            false,
        ),
    ] {
        assert_eq!(validator.is_valid(&record), accepted, "schema {record}");
        assert_eq!(
            parse(&serde_json::to_vec(&record).unwrap()).is_ok(),
            accepted,
            "parser {record}"
        );
    }
}
