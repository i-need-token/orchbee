// The log's privacy rule is a grammar, so these tests pin the grammar's refusals for each class of
// text no record may carry, and the date grammar that names, retains and selects daily files.
use super::{
    LogValue, civil_from_days, days_from_civil, encode, file_name, timestamp, well_formed,
    well_formed_lines,
};

/// One sample of each class of text that must never reach a record or a bundle's log entries.
const EXCLUDED: [(&str, &str); 9] = [
    ("terminal content", "\u{1b}[31mbuild failed\u{1b}[0m"),
    ("typed input", "git push --force"),
    ("prompt", "Fix the failing test in main.rs"),
    ("environment value", "OPENAI_API_KEY=sk-live"),
    ("file contents", "[ui]\nwindow_title = 'x'"),
    ("credential", "https://user:secret@example.com/repo"),
    ("command", "powershell.exe -NoLogo"),
    ("argument", "--token secret"),
    ("home path", r"C:\Users\person\project"),
];

#[test]
fn text_outside_the_token_grammar_never_reaches_a_record() {
    for (class, sample) in EXCLUDED {
        let leaked: &'static str = Box::leak(sample.to_owned().into_boxed_str());
        let line = encode(
            0,
            "info",
            "client",
            leaked,
            &[("value", LogValue::Token(leaked))],
        );
        let text = String::from_utf8(line).expect("records are UTF-8");
        assert!(!text.contains(sample), "{class} reached a record: {text}");
        assert!(text.contains("\"event\":\"invalid\""), "{class}");
        assert!(text.contains("\"value\":\"invalid\""), "{class}");
        assert!(well_formed(text.trim_end().as_bytes()), "{class}");
    }
}

#[test]
fn records_keep_at_most_eight_named_fields_on_one_line() {
    let fields: Vec<_> = (0..12)
        .map(|index| {
            let name: &'static str = Box::leak(format!("field{index}").into_boxed_str());
            (name, LogValue::Count(index))
        })
        .chain([("Bad name", LogValue::Flag(true))])
        .collect();
    let line = encode(0, "warning", "server", "worker-lost", &fields);
    let text = String::from_utf8(line).expect("records are UTF-8");
    assert_eq!(text.matches('\n').count(), 1);
    assert!(text.ends_with('\n'));
    let record: serde_json::Value = serde_json::from_str(&text).expect("records are JSON");
    assert_eq!(
        record["fields"].as_object().map(|fields| fields.len()),
        Some(8)
    );
    assert!(!text.contains("Bad name"));
}

#[test]
fn bundles_keep_only_lines_a_writer_produced() {
    let written = encode(1_000, "error", "worker", "host-start-failed", &[]);
    let mut file = written.clone();
    for (_, sample) in EXCLUDED {
        file.extend_from_slice(sample.as_bytes());
        file.push(b'\n');
        let forged = format!(
            "{{\"time\":\"1970-01-01T00:00:01.000Z\",\"level\":\"info\",\"role\":\"client\",\"pid\":1,\"event\":\"started\",\"fields\":{{\"note\":{}}}}}\n",
            serde_json::Value::from(sample)
        );
        file.extend_from_slice(forged.as_bytes());
    }
    file.extend_from_slice(b"{\"time\":\"1970-01-01T00:00:01.000Z\",\"level\":\"info\",\"role\":\"client\",\"pid\":1,\"event\":\"started\",\"fields\":{},\"extra\":1}\n");
    let (kept, omitted) = well_formed_lines(&file);
    assert_eq!(kept, written);
    let raw_lines: usize = EXCLUDED
        .iter()
        .map(|(_, sample)| sample.lines().count())
        .sum();
    assert_eq!(omitted, (raw_lines + EXCLUDED.len() + 1) as u64);
}

#[test]
fn daily_names_follow_the_proleptic_calendar() {
    for (year, month, day) in [(1970, 1, 1), (2000, 2, 29), (2024, 12, 31), (2026, 11, 8)] {
        assert_eq!(
            civil_from_days(days_from_civil(year, month, day)),
            (year, month, day)
        );
    }
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(file_name(days_from_civil(2026, 3, 1)), "2026-03-01.jsonl");
    assert_eq!(timestamp(1_762_570_245_123), "2025-11-08T02:50:45.123Z");
}
