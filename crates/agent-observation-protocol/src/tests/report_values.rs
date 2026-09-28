//! Report decisions cover source ownership, literal resume vectors and lossless bounds.
use crate::*;

fn absolute(value: &str) -> bool {
    value.starts_with("C:\\") || value.starts_with("\\\\")
}

const OFFICIAL: [(&str, &str, AgentReportSourcePolicy); 18] = {
    use AgentReportSourcePolicy::{
        FullLifecycle as Full, IdentityOnly as Identity, NativeState as Native,
    };
    [
        ("herdr:claude", "claude", Native),
        ("herdr:codex", "codex", Native),
        ("herdr:copilot", "copilot", Native),
        ("herdr:devin", "devin", Native),
        ("herdr:droid", "droid", Native),
        ("herdr:kimi", "kimi", Full),
        ("herdr:omp", "omp", Full),
        ("herdr:mastracode", "mastracode", Full),
        ("herdr:pi", "pi", Full),
        ("herdr:hermes", "hermes", Identity),
        ("herdr:opencode", "opencode", Full),
        ("herdr:qodercli", "qodercli", Native),
        ("herdr:qwen", "qwen", Identity),
        ("herdr:kilo", "kilo", Full),
        ("herdr:cursor", "cursor", Native),
        ("herdr:antigravity_cli", "agy", Identity),
        ("herdr:grok", "grok", Native),
        ("herdr:letta", "letta", Identity),
    ]
};

#[test]
fn report_pairs_do_not_confer_resumability_to_custom_or_foreign_sources() {
    for (source, agent, policy) in OFFICIAL {
        assert_eq!(agent_report_source_policy(source, agent), policy);
        assert!(
            AgentProviderSessionReference::from_report(source, agent, Some("s"), None, |_| panic!(
                "ID-only report must not inspect paths"
            ))
            .is_some()
        );
        for other in AgentReportProvider::ALL {
            let label = other.report_label();
            if label != agent {
                assert_eq!(
                    agent_report_source_policy(source, label),
                    AgentReportSourcePolicy::Custom
                );
                assert!(
                    AgentProviderSessionReference::from_report(
                        source,
                        label,
                        Some("s"),
                        Some(r"C:\a"),
                        |_| panic!("foreign source must not inspect paths")
                    )
                    .is_none()
                );
            }
        }
        for foreign in [
            "".to_owned(),
            source.to_uppercase(),
            format!(" {source}"),
            source.replace("herdr:", "custom:"),
        ] {
            assert_eq!(
                agent_report_source_policy(&foreign, agent),
                AgentReportSourcePolicy::Custom
            );
        }
    }
    assert_eq!(
        agent_report_source_policy("herdr:claude", "claude-code"),
        AgentReportSourcePolicy::Custom
    );
    assert_eq!(
        agent_report_source_policy("herdr:agy", "agy"),
        AgentReportSourcePolicy::Custom
    );
}

#[test]
fn provider_reference_admission_preserves_text_and_uses_byte_bounds() {
    for value in [
        " ",
        " leading trailing ",
        "路径 session",
        "--literal",
        "a;$(echo x)",
    ] {
        let id = AgentProviderSessionReference::id(value).unwrap();
        assert_eq!(id.value(), value);
        assert_eq!(id.kind(), AgentProviderSessionReferenceKind::Id);
        assert_eq!(
            serde_json::to_value(id).unwrap(),
            serde_json::json!({"kind":"id","value":value})
        );
    }
    for value in [
        "".to_owned(),
        "x\n".to_owned(),
        "x\0y".to_owned(),
        "x\u{85}y".to_owned(),
        "a".repeat(513),
        "界".repeat(171),
    ] {
        assert!(AgentProviderSessionReference::id(&value).is_none());
    }
    for value in ["a".repeat(512), format!("{}aa", "界".repeat(170))] {
        assert_eq!(
            AgentProviderSessionReference::id(&value).unwrap().value(),
            value
        );
    }
    let boundary = format!("C:\\{}", "a".repeat(4093));
    assert_eq!(
        AgentProviderSessionReference::path(&boundary, absolute)
            .unwrap()
            .value(),
        boundary
    );
    assert!(
        AgentProviderSessionReference::path(&format!("{boundary}x"), |_| panic!(
            "bound checked before capability"
        ))
        .is_none()
    );
    assert!(AgentProviderSessionReference::path("relative", |_| false).is_none());
    assert!(
        AgentProviderSessionReference::path("bad\t", |_| panic!(
            "controls checked before capability"
        ))
        .is_none()
    );
}

#[test]
fn session_report_path_precedence_falls_back_without_discarding_valid_id() {
    for (source, agent, _) in OFFICIAL {
        let reference = AgentProviderSessionReference::from_report(
            source,
            agent,
            Some("id"),
            Some(r"C:\session"),
            absolute,
        )
        .unwrap();
        let paths_allowed = matches!(agent, "pi" | "omp");
        assert_eq!(
            reference.value(),
            if paths_allowed { r"C:\session" } else { "id" }
        );
        let fallback = AgentProviderSessionReference::from_report(
            source,
            agent,
            Some("id"),
            Some("relative"),
            absolute,
        )
        .unwrap();
        assert_eq!(fallback.kind(), AgentProviderSessionReferenceKind::Id);
        assert!(
            AgentProviderSessionReference::from_report(
                source,
                agent,
                None,
                Some("relative"),
                absolute
            )
            .is_none()
        );
    }
    assert!(
        AgentProviderSessionReference::from_report(
            "herdr:pi",
            "pi",
            Some(""),
            Some(r"C:\session"),
            absolute
        )
        .is_some()
    );
    let observed = std::cell::Cell::new(false);
    assert!(
        AgentProviderSessionReference::from_report(
            "herdr:codex",
            "codex",
            Some("id"),
            Some(r"C:\session"),
            |_| {
                observed.set(true);
                true
            }
        )
        .is_some()
    );
    assert!(
        !observed.get(),
        "providers without path refs do not invoke path capabilities"
    );
}

#[test]
fn windows_resume_vectors_follow_provider_syntax_without_shell_interpretation() {
    let vectors = [
        ("claude", vec!["claude", "--resume", "session"]),
        ("codex", vec!["codex", "resume", "session"]),
        ("copilot", vec!["copilot", "--resume=session"]),
        ("devin", vec!["devin", "--resume", "session"]),
        ("droid", vec!["droid", "--resume", "session"]),
        ("kimi", vec!["kimi", "--session", "session"]),
        ("omp", vec!["omp", "--resume=session"]),
        ("mastracode", vec!["mastracode", "--thread", "session"]),
        ("pi", vec!["pi", "--session", "session"]),
        ("hermes", vec!["hermes", "--resume", "session"]),
        ("opencode", vec!["opencode", "--session", "session"]),
        ("qodercli", vec!["qodercli", "--resume", "session"]),
        ("qwen", vec!["qwen", "--resume", "session"]),
        ("kilo", vec!["kilo", "--session", "session"]),
        ("cursor", vec!["cursor-agent.cmd", "--resume", "session"]),
        ("agy", vec!["agy", "--conversation", "session"]),
        ("grok", vec!["grok", "--resume", "session"]),
        ("letta", vec!["letta", "--conversation", "session"]),
    ];
    for ((source, agent, _), (expected_agent, expected)) in OFFICIAL.into_iter().zip(vectors) {
        assert_eq!(agent, expected_agent);
        let id = AgentProviderSessionReference::id("session").unwrap();
        assert_eq!(
            windows_agent_resume_argv(source, agent, &id).unwrap(),
            expected
        );
        assert!(windows_agent_resume_argv("custom", agent, &id).is_none());
        let path = AgentProviderSessionReference::path(r"C:\session file; $(x)", absolute).unwrap();
        let plan = windows_agent_resume_argv(source, agent, &path);
        if matches!(agent, "pi" | "omp") {
            let plan = plan.unwrap();
            assert!(plan.last().unwrap().ends_with(path.value()));
        } else {
            assert!(plan.is_none());
        }
    }
    let id = AgentProviderSessionReference::id("default:agent with spaces;$(x)").unwrap();
    assert_eq!(
        windows_agent_resume_argv("herdr:letta", "letta", &id).unwrap(),
        [
            "letta",
            "--conversation",
            "default",
            "--agent",
            "agent with spaces;$(x)"
        ]
    );
    assert!(
        windows_agent_resume_argv(
            "herdr:letta",
            "letta",
            &AgentProviderSessionReference::id("default:").unwrap()
        )
        .is_none()
    );
}

#[test]
fn report_sequences_retain_full_u64_and_start_reasons_have_no_fallback_guess() {
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
    for source in [
        "startup", "resume", "clear", "compact", "branch", "new", "fork", "select",
    ] {
        let parsed = AgentSessionStartSource::normalize(&format!(" {source}\t")).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), source);
    }
    for source in ["", "STARTUP", "unknown", "new-session", " session "] {
        assert!(AgentSessionStartSource::normalize(source).is_none());
    }
}

#[cfg(windows)]
#[test]
fn reference_paths_use_real_windows_absoluteness_without_filesystem_access() {
    for (path, expected) in [
        (r"C:\sessions\one.json", true),
        (r"\\server\share\one", true),
        (r"\\?\C:\sessions\one", true),
        ("C:relative", false),
        ("/relative-to-drive", false),
        ("plain", false),
    ] {
        let reference = AgentProviderSessionReference::path(path, |value| {
            std::path::Path::new(value).is_absolute()
        });
        assert_eq!(reference.is_some(), expected, "{path}");
    }
}

#[test]
fn source_reservation_and_start_eligibility_are_independent_guards() {
    assert_eq!(
        agent_report_source_policy("herdr:qwen", "qwen"),
        AgentReportSourcePolicy::IdentityOnly
    );
    assert!(agent_report_reserves_native_state("herdr:qwen", "qwen"));
    assert!(!agent_report_reserves_native_state(
        "herdr:hermes",
        "hermes"
    ));
    assert!(agent_report_reserves_native_state("herdr:claude", "claude"));
    assert!(!agent_report_reserves_native_state("foreign", "claude"));
    use AgentSessionStartSource::{Branch, Clear, Compact, Fork, New, Resume, Select, Startup};
    for reason in [Clear, Compact, Fork, New, Resume, Select, Startup] {
        assert!(reason.is_full_lifecycle_start());
    }
    assert!(!Branch.is_full_lifecycle_start());
    for (source, agent, reason, allowed) in [
        ("herdr:claude", "claude", Some(Startup), false),
        ("herdr:codex", "codex", Some(Startup), true),
        ("herdr:opencode", "opencode", Some(New), false),
        ("herdr:opencode", "opencode", Some(Resume), false),
        ("herdr:opencode", "opencode", Some(Select), true),
        ("herdr:qwen", "qwen", Some(Branch), true),
        ("herdr:antigravity_cli", "agy", None, true),
        ("herdr:antigravity_cli", "agy", Some(Startup), false),
        ("herdr:letta", "letta", Some(Startup), false),
        ("custom", "codex", Some(Startup), false),
    ] {
        assert_eq!(
            agent_report_allows_session_replacement(source, agent, reason),
            allowed
        );
    }
}
