use super::*;
use manifest::Manifest;
use orchbee_agent_observation_protocol::AgentReportProvider as Provider;
use orchbee_workbench_protocol::PanelAgentState::{Blocked, Idle, Unknown, Working};
fn input(screen: &str) -> ScreenInput<'_> {
    ScreenInput {
        screen,
        title: "",
        progress: "",
    }
}
fn one(rules: &str) -> Manifest {
    Manifest::parse(Provider::Codex, &format!("id='codex'\n{rules}")).unwrap()
}
#[test]
fn all_bundled_manifests_compile_and_provider_absence_cannot_infer_identity() {
    let engine = ScreenDetector::bundled().unwrap();
    assert_eq!(engine.0.len(), 22);
    for provider in Provider::ALL {
        // Letta deliberately ends with a catch-all unknown rule, including the empty screen.
        let expected = if provider == Provider::Letta {
            Detection::fallback(false)
        } else {
            Detection::fallback(true)
        };
        assert_eq!(
            engine.detect(Some(provider), input("")).unwrap(),
            expected,
            "{provider:?}"
        );
    }
    assert_eq!(
        engine
            .detect(None, input("Working... Action Required"))
            .unwrap(),
        Detection::fallback(false)
    );
}
#[test]
fn known_provider_signals_transcript_skip_and_separate_osc_precedence() {
    let engine = ScreenDetector::bundled().unwrap();
    for (provider, screen, title, progress, state, skip) in [
        (Provider::Pi, "Working...", "", "", Working, false),
        (
            Provider::Pi,
            "── ⠋ Working ─────────",
            "",
            "",
            Working,
            false,
        ),
        (Provider::Pi, "ready", "", "", Idle, false),
        (
            Provider::Gemini,
            "│ Apply this change",
            "",
            "",
            Blocked,
            false,
        ),
        (Provider::Gemini, "esc to cancel", "", "", Working, false),
        (
            Provider::Codex,
            "arbitrary output",
            "Action Required",
            "",
            Blocked,
            false,
        ),
        (
            Provider::Codex,
            "arbitrary output",
            "⠋ running",
            "",
            Working,
            false,
        ),
        (
            Provider::Codex,
            "arbitrary output",
            "session title",
            "",
            Idle,
            false,
        ),
        (
            Provider::Codex,
            "› question\n↑/↓ to scroll pgup/pgdn to home/end to jump q to quit esc to edit prev",
            "",
            "",
            Unknown,
            true,
        ),
        (
            Provider::Claude,
            "showing detailed transcript ctrl+o to toggle",
            "",
            "",
            Unknown,
            true,
        ),
        (
            Provider::Grok,
            "",
            "Action Required",
            "4;1;-1",
            Blocked,
            false,
        ),
    ] {
        let found = engine
            .detect(
                Some(provider),
                ScreenInput {
                    screen,
                    title,
                    progress,
                },
            )
            .unwrap();
        assert_eq!(
            (found.state, found.skip),
            (state, skip),
            "{provider:?}: {screen:?} {title:?}"
        );
        if state == Blocked {
            assert!(found.visible_blocker);
        }
    }
}
#[test]
fn nested_gates_priority_and_line_patterns_follow_declared_semantics() {
    let m = one(r#"
[[rules]]
id='base'
state='idle'
priority=1
contains=['match']
[[rules]]
id='nested'
state='working'
priority=10
contains=['MATCH']
all=[{any=[{regex=['w[io]n']},{contains=['fallback']}]}]
not=[{contains=['blocked']}]
[[rules]]
id='lines'
state='blocked'
priority=20
line_regex=['^exact line$','^another$']
[[rules]]
id='equal-priority-later'
state='unknown'
priority=20
contains=['exact line']
"#);
    for (screen, expected) in [
        ("match win", Working),
        ("MaTcH fallback", Working),
        ("match WIN", Idle),
        ("match win blocked", Idle),
        ("exact line\nanother", Blocked),
        ("prefix exact line\nanother", Unknown),
    ] {
        assert_eq!(m.detect(input(screen)).state, expected, "{screen}");
    }
}
#[test]
fn generated_gate_truth_table_matches_boolean_oracle() {
    let m = one(r#"
[[rules]]
id='boolean'
state='working'
contains=['alpha']
all=[{contains=['beta']}]
any=[{contains=['gamma']},{contains=['delta']}]
not=[{contains=['epsilon'],not=[{contains=['zeta']}]}]
"#);
    for bits in 0..64 {
        let words = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta"];
        let screen = words
            .iter()
            .enumerate()
            .filter(|(i, _)| bits & (1 << i) != 0)
            .map(|(_, v)| *v)
            .collect::<Vec<_>>()
            .join(" ");
        let yes =
            bits & 1 != 0 && bits & 2 != 0 && bits & 12 != 0 && (bits & 16 == 0 || bits & 32 != 0);
        assert_eq!(
            m.detect(input(&screen)).state,
            if yes { Working } else { Idle },
            "bits={bits}"
        );
    }
}
#[test]
fn invalid_manifest_never_becomes_an_executable_ruleset() {
    for content in [
        "id='codex'",
        "id='different'\n[[rules]]\nid='x'\ncontains=['x']",
        "id='codex'\nunknown=1\n[[rules]]\nid='x'\ncontains=['x']",
        "id='codex'\nmin_engine_version=4\n[[rules]]\nid='x'\ncontains=['x']",
        "id='codex'\n[[rules]]\nid=''\ncontains=['x']",
        "id='codex'\n[[rules]]\nid='x'\nnot=[{contains=['x']}]",
        "id='codex'\n[[rules]]\nid='x'\ncontains=['x']\nregion='bogus'",
        "id='codex'\n[[rules]]\nid='x'\nregex=['[']",
        "id='codex'\n[[rules]]\nid='x'\ncontains=['x']\nstate='stopped'",
        "id='codex'\n[[rules]]\nid='x'\ncontains=['x']\nstate='working'\nskip_state_update=true",
        "id='codex'\n[[rules]]\nid='x'\ncontains=['x']\nstate='unknown'\nskip_state_update=true\nvisible_idle=true",
        "id='codex'\nmin_engine_version=2\n[[rules]]\nid='x'\ncontains=['x']\nregion='top_non_empty_lines(3)'",
    ] {
        assert!(
            Manifest::parse(Provider::Codex, content).is_err(),
            "{content}"
        );
    }
    let long = format!(
        "id='codex'\n[[rules]]\nid='x'\ncontains=['{}']",
        "字".repeat(513)
    );
    assert!(Manifest::parse(Provider::Codex, &long).is_err());
    let mut gate = "{ contains=['x'] }".to_owned();
    for _ in 0..10 {
        gate = format!("{{all=[{gate}]}}");
    }
    assert!(
        Manifest::parse(
            Provider::Codex,
            &format!("id='codex'\n[[rules]]\nid='x'\nall=[{gate}]")
        )
        .is_err()
    );
    let rules = (0..129)
        .map(|i| format!("\n[[rules]]\nid='{i}'\ncontains=['x']"))
        .collect::<String>();
    assert!(Manifest::parse(Provider::Codex, &format!("id='codex'{rules}")).is_err());
}
#[test]
fn input_bounds_reject_whole_sample_and_visibility_matches_state() {
    let engine = ScreenDetector::bundled().unwrap();
    assert!(engine.detect(None, input(&"x".repeat(65536))).is_ok());
    assert!(engine.detect(None, input(&"x".repeat(65537))).is_err());
    assert!(
        engine
            .detect(
                None,
                ScreenInput {
                    screen: "",
                    title: &"x".repeat(4097),
                    progress: ""
                }
            )
            .is_err()
    );
    assert!(
        engine
            .detect(
                None,
                ScreenInput {
                    screen: "",
                    title: "",
                    progress: &"x".repeat(4097)
                }
            )
            .is_err()
    );
    let m = one(
        "[[rules]]\nid='x'\ncontains=['x']\nstate='idle'\nvisible_working=true\nvisible_idle=true\nvisible_blocker=true",
    );
    let d = m.detect(input("x"));
    assert!(d.visible_idle);
    assert!(!d.visible_working && !d.visible_blocker);
}
#[test]
fn live_prompt_regions_exclude_stale_responses_and_preserve_utf8_crlf() {
    use regions::Region;
    for eol in ["\n", "\r\n"] {
        let text = ["你好", "• reply", "› question", "answer"].join(eol);
        let i = input(&text);
        for (name, expected) in [
            ("after_last_prompt_marker", "answer".to_owned()),
            (
                "before_current_prompt_marker",
                format!("你好{eol}• reply{eol}"),
            ),
            ("whole_recent_without_current_prompt_marker", String::new()),
            ("current_prompt_block_marker", "• reply".to_owned()),
            (
                "after_current_prompt_block_marker",
                format!("• reply{eol}› question{eol}answer"),
            ),
            ("bottom_lines(2)", format!("› question{eol}answer")),
            ("top_non_empty_lines(1)", format!("你好{eol}")),
        ] {
            assert_eq!(
                Region::parse(name).unwrap().select(i),
                expected,
                "{name} {eol:?}"
            );
        }
        let stale = format!("{text}{eol}✓ result");
        assert_eq!(Region::BeforePrompt.select(input(&stale)), stale);
        assert_eq!(Region::WithoutPrompt.select(input(&stale)), stale);
    }
}
#[test]
fn prompt_boxes_nonempty_regions_and_invalid_region_counts() {
    use regions::Region;
    let text = "old\nlast signal\n───\n\n输入\n────\nfooter\n";
    for (name, expected) in [
        ("prompt_box_body", "\n输入\n"),
        ("above_prompt_box", "old\nlast signal\n"),
        ("last_non_empty_above_prompt_box", "last signal"),
        ("after_last_horizontal_rule", "footer\n"),
        ("bottom_non_empty_lines(2)", "────\nfooter\n"),
        ("bottom_lines(0)", ""),
    ] {
        assert_eq!(Region::parse(name).unwrap().select(input(text)), expected);
    }
    for name in [
        "bottom_lines(-1)",
        "bottom_lines(65536)",
        "top_non_empty_lines(0)",
        "top_non_empty_lines(01)",
        "top_non_empty_lines(+2)",
        "wat",
    ] {
        assert!(Region::parse(name).is_err(), "{name}");
    }
}
