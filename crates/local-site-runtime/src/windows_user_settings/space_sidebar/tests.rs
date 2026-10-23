use super::*;
fn parse(text: &str) -> Result<WindowsSpaceSidebarSettings, WindowsSettingsError> {
    WindowsSpaceSidebarSettings::parse(&toml::from_str::<toml::Value>(text).unwrap())
}
#[test]
fn defaults_empty_layouts_custom_names_and_git_demand_remain_distinct() {
    let default = parse("").unwrap();
    assert_eq!(default, WindowsSpaceSidebarSettings::default());
    assert!(default.needs_git());
    assert_eq!(default.rows().len(), 2);
    let empty = parse("rows=[]\nrow_gap=65535").unwrap();
    assert!(empty.rows().is_empty());
    assert!(!empty.needs_git());
    assert_eq!(empty.row_gap(), u16::MAX);
    let all = parse("rows=[['state_icon','state_text','workspace','branch','location','git_status','$branch','$git_status']]").unwrap();
    assert_eq!(
        all.rows()[0][3].kind(),
        WindowsSpaceSidebarTokenKind::Branch
    );
    assert_eq!(
        all.rows()[0][4].kind(),
        WindowsSpaceSidebarTokenKind::Location
    );
    assert_eq!(
        all.rows()[0][6].kind(),
        WindowsSpaceSidebarTokenKind::Custom("branch")
    );
    assert!(parse("rows=[['location']]").unwrap().needs_git());
    let custom = parse("rows=[['$branch','$git_status']]").unwrap();
    assert!(!custom.needs_git());
    for settings in [default, empty, all, custom] {
        assert_eq!(
            WindowsSpaceSidebarSettings::from_profile(&settings.profile()).unwrap(),
            settings
        );
    }
}
#[test]
fn text_rules_match_raw_values_and_family_specific_non_text_tokens_reject_rules() {
    let configured = parse("rows=[[{token='branch',fg='#abc',bold=true,dim=true,rules=[{starts_with='worktree/',fg='#102030',bold=false,dim=false},{contains='/',hide=true}]},{token='$x',rules=[{gt=2,hide=true}]}]]").unwrap();
    let token = &configured.rows()[0][0];
    let style = token.style_for_value(Some("worktree/测试")).unwrap();
    assert_eq!(style.fg, Some([16, 32, 48]));
    assert_eq!(style.bold, Some(false));
    assert_eq!(style.dim, Some(false));
    assert!(token.style_for_value(Some("other/branch")).is_none());
    assert!(configured.rows()[0][1].style_for_value(Some("3")).is_none());
    for text in [
        "rows=[['terminal_title']]",
        "rows=[['agent']]",
        "rows=[['$bad.name']]",
        "rows=[[{token='state_icon',rules=[{equals='x'}]}]]",
        "rows=[[{token='git_status',rules=[{gt=0}]}]]",
        "rows=[[{token='branch',fg='red'}]]",
        "rows=[[{token='branch',rules=[{gt=nan}]}]]",
        "rows_by_agent={codex=[]}",
        "row_gap=65536",
    ] {
        assert_eq!(
            parse(text).unwrap_err().field,
            "ui.sidebar.spaces",
            "{text}"
        );
    }
}
#[test]
fn rows_rules_and_canonical_publication_have_closed_bounded_admission() {
    for (count, accepted) in [(16, true), (17, false)] {
        let rows = std::iter::repeat_n("['workspace']", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse(&format!("rows=[{rows}]")).is_ok(), accepted);
        let tokens = std::iter::repeat_n("'workspace'", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(parse(&format!("rows=[[{tokens}]]")).is_ok(), accepted);
        let rules = std::iter::repeat_n("{equals='x'}", count)
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(
            parse(&format!("rows=[[{{token='workspace',rules=[{rules}]}}]]")).is_ok(),
            accepted
        );
    }
    let profile = parse("rows=[['workspace']]").unwrap().profile();
    for invalid in [
        "{}".into(),
        profile.replace("workspace", "agent"),
        profile.replace("\"rules\":[]", "\"rules\":[],\"extra\":1"),
        "x".repeat(MAXIMUM_SPACE_SIDEBAR_PROFILE_BYTES + 1),
    ] {
        assert!(WindowsSpaceSidebarSettings::from_profile(&invalid).is_err());
    }
}
