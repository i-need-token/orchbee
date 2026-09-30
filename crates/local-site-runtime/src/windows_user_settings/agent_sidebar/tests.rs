use super::*;

fn parse(text: &str) -> Result<WindowsAgentSidebarSettings, WindowsSettingsError> {
    WindowsAgentSidebarSettings::parse(&toml::from_str::<toml::Value>(text).unwrap())
}

#[test]
fn explicit_empty_defaults_overrides_and_profiles_keep_distinct_meanings() {
    let default = parse("").unwrap();
    assert!(default.rows_for_agent("codex", None).is_none());
    let configured = parse(
        "rows=[['state_icon','state_text','machine','workspace','tab','pane','agent','$summary']]\nrow_gap=65535\n[rows_by_agent]\nclaude=[]\n'claude-code'=[['$model']]\n",
    )
    .unwrap();
    assert_eq!(configured.row_gap(), u16::MAX);
    assert_eq!(
        configured.rows_for_agent("codex", None).unwrap()[0].len(),
        8
    );
    assert!(
        configured
            .rows_for_agent("claude", None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        configured
            .rows_for_agent("claude-code", Some("claude"))
            .unwrap()[0][0]
            .name(),
        "$model"
    );
    assert!(
        configured
            .rows_for_agent("other-claude", Some("claude"))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        WindowsAgentSidebarSettings::from_profile(&configured.profile()).unwrap(),
        configured
    );
    assert_eq!(
        WindowsAgentSidebarSettings::from_profile(&default.profile()).unwrap(),
        default
    );
}

#[test]
fn malformed_and_unsupported_layouts_are_rejected_instead_of_silently_ignored() {
    for text in [
        "rows='agent'",
        "rows=[['typo']]",
        "rows=[['$']]",
        "rows=[['$a.b']]",
        "rows=[['terminal_title']]",
        "rows=[['terminal_title_stripped']]",
        "row_gap=-1",
        "row_gap=65536",
        "row_gap=1.5",
        "other=1",
        "[rows_by_agent]\n'UPPER'=[['agent']]",
        "[rows_by_agent]\n''=[]",
        "rows=[[{token='agent',fg='#xxff00'}]]",
        "rows=[[{token='agent',fg='red'}]]",
        "rows=[[{token='agent',bold='true'}]]",
        "rows=[[{token='agent',extra=1}]]",
        "rows=[[{token='state_icon',rules=[{equals='x'}]}]]",
        "rows=[[{token='agent',rules=[{}]}]]",
        "rows=[[{token='agent',rules=[{equals='x',contains='x'}]}]]",
        "rows=[[{token='agent',rules=[{gt=1,ignore_case=false}]}]]",
        "rows=[[{token='agent',rules=[{gt=nan}]}]]",
        "rows=[[{token='agent',rules=[{lt=inf}]}]]",
        "rows=[[{token='agent',rules=[{equals='x',extra=1}]}]]",
    ] {
        assert!(parse(text).is_err(), "admitted {text}");
    }
}

#[test]
fn bounds_apply_to_rows_tokens_rules_providers_and_canonical_profiles() {
    for (count, expected) in [(16, true), (17, false)] {
        let rows = std::iter::repeat_n("['agent']", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse(&format!("rows=[{rows}]")).is_ok(), expected);
        let tokens = std::iter::repeat_n("'agent'", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse(&format!("rows=[[{tokens}]]")).is_ok(), expected);
        let rules = std::iter::repeat_n("{equals='x'}", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse(&format!("rows=[[{{token='agent',rules=[{rules}]}}]]")).is_ok(),
            expected
        );
    }
    for (count, expected) in [(64, true), (65, false)] {
        let entries = (0..count).map(|i| format!("p{i}=[]\n")).collect::<String>();
        assert_eq!(
            parse(&format!("[rows_by_agent]\n{entries}")).is_ok(),
            expected
        );
    }
    assert!(parse(&format!("rows=[['${}']]", "a".repeat(33))).is_err());
    let large = format!(
        "rows=[[{{token='agent',rules=[{}]}}]]",
        std::iter::repeat_n(format!("{{equals='{}'}}", "x".repeat(4096)), 16)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(
        parse(&large).is_err(),
        "canonical budget is enforced at file admission"
    );
    let profile = parse("rows=[['$x']]").unwrap().profile();
    for invalid in [
        profile.replace("$x", "$bad.name"),
        profile.replace("\"rules\":[]", "\"rules\":[],\"extra\":1"),
        "{}".into(),
        "x".repeat(MAXIMUM_AGENT_SIDEBAR_PROFILE_BYTES + 1),
    ] {
        assert!(WindowsAgentSidebarSettings::from_profile(&invalid).is_err());
    }
}

#[test]
fn first_rule_wins_hide_and_false_modifiers_override_base_style() {
    let settings = parse("rows=[[{token='$model',fg='#abc',bold=true,dim=true,rules=[{contains='FAST',ignore_case=true,fg='#102030',bold=false,dim=false},{equals='fast',hide=true}]}]]").unwrap();
    let token = &settings.rows_for_agent("codex", None).unwrap()[0][0];
    let style = token.style_for_value(Some("fast")).unwrap();
    assert_eq!(style.fg, Some([0x10, 0x20, 0x30]));
    assert_eq!(style.bold, Some(false));
    assert_eq!(style.dim, Some(false));
    assert_eq!(
        token.style_for_value(Some("slow")).unwrap().fg,
        Some([0xaa, 0xbb, 0xcc])
    );
    let settings =
        parse("rows=[[{token='$x',rules=[{equals='hidden',hide=true},{contains='h',bold=true}]}]]")
            .unwrap();
    let token = &settings.rows_for_agent("codex", None).unwrap()[0][0];
    assert!(token.style_for_value(Some("hidden")).is_none());
    assert_eq!(
        token.style_for_value(Some("hello")).unwrap().bold,
        Some(true)
    );
}

#[test]
fn rules_use_raw_values_ascii_case_and_strict_finite_numeric_comparison() {
    for (condition, value, yes, no) in [
        ("equals", "MiXeD", "mixed", "mixed tail"),
        ("contains", "MiXeD", "a mixed b", "else"),
        ("starts_with", "MiXeD", "mixed tail", "a mixed"),
        ("equals", "é", "é", "É"),
        ("contains", "", "", "unreachable"),
    ] {
        let settings = parse(&format!(
            "rows=[[{{token='$x',rules=[{{{condition}='{value}',ignore_case=true,hide=true}}]}}]]"
        ))
        .unwrap();
        let token = &settings.rows_for_agent("codex", None).unwrap()[0][0];
        assert!(token.style_for_value(Some(yes)).is_none());
        if !value.is_empty() {
            assert!(token.style_for_value(Some(no)).is_some());
        }
    }
    let settings = parse("rows=[[{token='$x',rules=[{gt=2,hide=true}]}]]").unwrap();
    let token = &settings.rows_for_agent("codex", None).unwrap()[0][0];
    for value in ["3", "3e0", "+3"] {
        assert!(token.style_for_value(Some(value)).is_none());
    }
    for value in ["2", " 3", "3 ", "NaN", "inf", "3ms", ""] {
        assert!(token.style_for_value(Some(value)).is_some(), "{value}");
    }
    let settings =
        parse("rows=[[{token='$x',rules=[{equals=\"raw\\nvalue\",hide=true}]}]]").unwrap();
    assert!(
        settings.rows_for_agent("codex", None).unwrap()[0][0]
            .style_for_value(Some("raw\nvalue"))
            .is_none()
    );
}
