use super::*;

#[test]
fn report_sequences_retain_full_u64_without_numeric_coercion() {
    for value in [
        0,
        1,
        MAXIMUM_SAFE_INTEGER,
        MAXIMUM_SAFE_INTEGER + 1,
        u64::MAX,
    ] {
        let sequence = AgentReportSequence::new(value);
        let json = serde_json::to_string(&sequence).unwrap();
        assert_eq!(json, format!("\"{value}\""));
        assert_eq!(
            serde_json::from_str::<AgentReportSequence>(&json)
                .unwrap()
                .get(),
            value
        );
    }
    for json in [
        "0",
        "1",
        "null",
        "true",
        "\"\"",
        "\"00\"",
        "\"01\"",
        "\"+1\"",
        "\"-1\"",
        "\"1.0\"",
        "\"1e3\"",
        "\" 1\"",
        "\"１\"",
        "\"18446744073709551616\"",
    ] {
        assert!(
            serde_json::from_str::<AgentReportSequence>(json).is_err(),
            "{json}"
        );
    }
}
