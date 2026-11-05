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

#[test]
fn grok_browser_login_blocks_idle_signals_until_the_ready_screen_replaces_it() {
    let engine = ScreenDetector::bundled().unwrap();
    for screen in [
        "Approve in your browser to finish signing in.\n\nWaiting for approval...\nctrl+q  go back",
        "A browser window will open for authentication.\n\nWaiting for login to complete...\nctrl+q  go back",
        "Sign in to Grok\nOpen this URL in your browser to approve:\n\nWaiting for approval…",
        // Narrow chrome may wrap the header; the anchored caption and local return hint remain.
        "Approve in your browser to\nfinish signing in.\nWaiting for approval...\nctrl+q  go back",
    ] {
        for title in ["grok", "session - grok", "⠋ grok"] {
            for progress in ["4;0;0", "4;1;-1", ""] {
                let sample = ScreenInput {
                    screen,
                    title,
                    progress,
                };
                let detected = engine.detect(Some(Provider::Grok), sample).unwrap();
                assert_eq!(detected.state, Blocked, "{sample:?}");
                assert!(detected.visible_blocker && !detected.visible_idle);
                let explanation = engine.explain(Provider::Grok, sample).unwrap();
                assert_eq!(explanation.state, detected.state);
                assert_eq!(
                    explanation.matched_rule.unwrap().id,
                    "browser_login_blocked"
                );
            }
        }
    }
    for (screen, expected) in [
        ("Waiting for approval...", Unknown), // No authentication context.
        ("Waiting for login to complete...", Unknown),
        (
            "Sign in to Grok\nThe transcript says Waiting for approval...",
            Idle,
        ),
        ("Ctrl+.:shortcuts", Idle), // Authentication completed; composer replaces the blocker.
    ] {
        let result = engine
            .detect(
                Some(Provider::Grok),
                ScreenInput {
                    screen,
                    title: "grok",
                    progress: "4;0;0",
                },
            )
            .unwrap();
        assert_eq!(result.state, expected, "{screen}");
        assert!(!result.visible_blocker);
    }
}

#[test]
fn devin_thinking_footer_is_working_and_missing_evidence_is_unknown() {
    let engine = ScreenDetector::bundled().unwrap();
    for (screen, expected) in [
        (
            ": Thinking · 1s (esc twice to interrupt)\nGuide Devin while it works",
            Working,
        ),
        (": Thinking · 1s (esc twice to interrupt)", Working),
        ("  Thinking · 1.5s (esc to interrupt)", Working),
        (
            "This transcript discusses Thinking · 1s (esc twice to interrupt)",
            Unknown,
        ),
        ("Thinking", Unknown),
        ("No current footer", Unknown),
        (
            "❭ Ask Devin to build features, fix bugs, or explain your code",
            Idle,
        ),
        // Sampled rows lose trailing spaces, so a choice that ends its row must still match.
        (
            " ✱ Do you trust the authors of this directory?
   For security, devin should not be run in directories with untrusted content.
 ~/src/app
 ❭ 1 Yes, trust
   2 No, exit
 ↑↓ select · ↵ confirm · esc cancel",
            Blocked,
        ),
        // A first-run choice must be answered before Devin proceeds; the question alone is not one.
        (
            "3    }
Detected light theme but configured for dark. Switch?
❭ 1 Yes, switch theme
  2 No, keep current
  3 Yes, always auto-detect
  4 No, don't ask again
↑↓ select · ↵ confirm",
            Blocked,
        ),
        (
            "The log says: Detected light theme but configured for dark. Switch?",
            Unknown,
        ),
    ] {
        let explanation = engine.explain(Provider::Devin, input(screen)).unwrap();
        explanation.validate().unwrap();
        assert_eq!(explanation.state, expected, "{screen}");
        assert_eq!(
            engine
                .detect(Some(Provider::Devin), input(screen))
                .unwrap()
                .state,
            expected
        );
    }
    for state in ["done", "error"] {
        assert!(
            Manifest::parse(
                Provider::Devin,
                &format!(
                    "id='devin'\n[[rules]]\nid='outcome'\nstate='{state}'\ncontains=['finished']"
                )
            )
            .is_err(),
            "screen text cannot claim a reliable {state} outcome"
        );
    }
}
/// Redacted live screens from opencode 2.2.5, pi 1.5.0 and omp 18.16.0. An idle report needs
/// positive composer chrome; busy or question chrome outranks it, and stray words stay unknown.
#[test]
fn opencode_pi_and_omp_report_idle_only_from_live_composer_chrome() {
    const OC_PROMPT: &str = "  ┃\n  ┃  Build · Model Default / OpenCode\n  ╹▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀\n";
    const PI_EDITOR: &str = "──────────────────────\n\n──────────────────────\n~/src/app\n";
    const OMP_COMPOSER: &str = "──────────────────────\n❯\n──────────────────────\n";
    let oc_idle = format!("{OC_PROMPT}  ~/src/app        9.2K (5%)  ctrl+p commands    ~/src/app");
    let oc_home = format!(
        "  ┃  Ask anything… \"Fix broken tests\"\n{OC_PROMPT}  ~/src/app    shift+tab agents  ctrl+p commands\n                    2.2.5"
    );
    let oc_busy = format!("{OC_PROMPT}   ■■■⬝⬝⬝⬝⬝ esc interrupt       8.4K (4%)  ctrl+p commands");
    let oc_busy_hint = format!("{OC_PROMPT}   esc interrupt       8.4K (4%)  ctrl+p commands");
    let oc_question = "  ┃  Questions\n  ┃  Pick one\n  ┃  ↑↓ select  enter submit  esc dismiss\n  ~/src/app   ctrl+p commands";
    let pi_idle = format!("{PI_EDITOR}0.0%/0 (auto)                              no-model");
    let pi_model =
        format!("{PI_EDITOR}↑1.2k ↓340 $0.004 4.5%/200k (auto)     model • thinking off");
    let pi_busy = format!("── ⠋ Working ─────────\n\n{PI_EDITOR}?/200k (auto)    model");
    let pi_shell = format!(
        "$ ping localhost\n ⠙ Running... (escape/ctrl+c to cancel)\n{PI_EDITOR}0.0%/0 (auto)    no-model"
    );
    let omp_idle = format!("{OMP_COMPOSER} π · ◒ Model · 🗑 src\\app · ◫ 4.2%/200K ⟲");
    let omp_busy =
        format!("  ⎋ Working…\n{OMP_COMPOSER} ⠴ 0s · ◒ Model · 🗑 src\\app · ◫ 4.1%/200K ⟲");
    let omp_shell = format!(
        "$ ping localhost\n─── ⣾ Running… (⎋ to cancel) ─────\n{OMP_COMPOSER} π · ◒ Model · 🗑 src\\app"
    );
    let engine = ScreenDetector::bundled().unwrap();
    for (provider, screen, title, expected) in [
        (Provider::OpenCode, oc_idle.as_str(), "OC | session", Idle),
        (Provider::OpenCode, &oc_home, "OpenCode", Idle),
        (Provider::OpenCode, &oc_busy, "OC | session", Working),
        (Provider::OpenCode, &oc_busy_hint, "OC | session", Working),
        (Provider::OpenCode, oc_question, "OC | session", Blocked),
        (
            Provider::OpenCode,
            "the docs say ctrl+p commands",
            "",
            Unknown,
        ),
        (Provider::OpenCode, OC_PROMPT, "", Unknown), // Footer clipped away.
        (Provider::Pi, &pi_idle, "π - app", Idle),
        (Provider::Pi, &pi_model, "π - app", Idle),
        (Provider::Pi, &pi_busy, "π - app", Working),
        (Provider::Pi, &pi_shell, "π - app", Working),
        (Provider::Pi, "context 0.0%/0 (auto)", "π - app", Unknown),
        (Provider::Omp, "", "π > app", Idle),
        (Provider::Omp, "", "π ⠋ app", Working),
        (Provider::Omp, "", "π : app", Working),
        (Provider::Omp, &omp_idle, "π ! app", Blocked),
        (Provider::Omp, "", "π: app", Unknown), // Title animation disabled.
        (Provider::Omp, &omp_idle, "", Idle),
        (Provider::Omp, &omp_idle, "π ⠙ app", Working),
        (Provider::Omp, &omp_busy, "π > app", Working), // Title lags the screen.
        (Provider::Omp, &omp_shell, "π > app", Working),
        (Provider::Omp, "a π · in the transcript", "", Unknown),
    ] {
        let sample = ScreenInput {
            screen,
            title,
            progress: "4;0;0",
        };
        let explanation = engine.explain(provider, sample).unwrap();
        assert_eq!(
            explanation.state, expected,
            "{provider:?} {title:?}\n{screen}"
        );
        assert_eq!(
            engine.detect(Some(provider), sample).unwrap().state,
            expected
        );
    }
}
fn one(rules: &str) -> Manifest {
    Manifest::parse(Provider::Codex, &format!("id='codex'\n{rules}")).unwrap()
}

#[test]
fn explanations_evaluate_all_rules_with_original_evidence_and_first_highest_priority() {
    let manifest = one(r#"
version = "2026.09.28.1"
[[rules]]
id = "first"
priority = 10
state = "working"
contains = ["Working"]
all = [{ contains = ["中文"] }]
[[rules]]
id = "equal"
priority = 10
state = "blocked"
regex = ["Working"]
[[rules]]
id = "lower"
priority = 1
state = "idle"
line_regex = ["^Working"]
[[rules]]
id = "absent"
priority = 100
state = "idle"
region = "osc_title"
contains = ["finished"]
"#);
    let screen = format!("Working 中文 {}", "界".repeat(300));
    let explanation = manifest.explain(input(&screen));
    explanation.validate().unwrap();
    assert_eq!(
        explanation.manifest_version.as_deref(),
        Some("2026.09.28.1")
    );
    assert_eq!(explanation.matched_rule.as_ref().unwrap().id, "first");
    assert_eq!(
        explanation
            .evaluated_rules
            .iter()
            .map(|r| r.matched)
            .collect::<Vec<_>>(),
        [true, true, true, false]
    );
    let evidence = &explanation.evaluated_rules[0].evidence;
    assert_eq!(evidence.contains, ["Working"]);
    assert_eq!(evidence.all_count, 1);
    assert_eq!(evidence.region_bytes as usize, screen.len());
    assert_eq!(evidence.region_preview.chars().count(), 243);
    assert!(evidence.region_preview.ends_with("..."));
    assert_eq!(explanation.evaluated_rules[3].evidence.region_bytes, 0);
    let mut invalid = explanation.clone();
    invalid.matched_rule = Some(invalid.evaluated_rules[1].rule.clone());
    assert!(
        invalid.validate().is_err(),
        "equal-priority later rule cannot claim victory"
    );
}

#[test]
fn explain_and_fallback_detection_agree_for_every_provider_and_bounded_input() {
    let engine = ScreenDetector::bundled().unwrap();
    for provider in Provider::ALL {
        for screen in [
            "",
            "Working...",
            "Action Required",
            "Thinking",
            "> prompt",
            "中文 ✦ 完成",
            "Press Enter",
            "tool execution",
        ] {
            for title in ["", "Working...", "⠋"] {
                let input = ScreenInput {
                    screen,
                    title,
                    progress: "4;3;0",
                };
                let result = engine.detect(Some(provider), input).unwrap();
                let explanation = engine.explain(provider, input).unwrap();
                assert_eq!(
                    (
                        explanation.state,
                        explanation.skip_state_update,
                        explanation.visible_idle,
                        explanation.visible_blocker,
                        explanation.visible_working
                    ),
                    (
                        result.state,
                        result.skip,
                        result.visible_idle,
                        result.visible_blocker,
                        result.visible_working
                    ),
                    "{provider:?}"
                );
            }
        }
    }
    assert!(
        engine
            .explain(Provider::Pi, input(&"x".repeat(64 * 1024 + 1)))
            .is_err()
    );
}
#[test]
fn all_bundled_manifests_compile_and_provider_absence_cannot_infer_identity() {
    let engine = ScreenDetector::bundled().unwrap();
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
        (Provider::Pi, "ready", "", "", Unknown, false),
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
        (Provider::Codex, "arbitrary output", "", "", Unknown, false),
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
            if yes { Working } else { Unknown },
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
