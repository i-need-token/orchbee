//! Output-boundary and failure-format witnesses: oversized projections are
//! injected at the result layer so every format-sensitive path is covered
//! without an invalid wire-sized snapshot.

mod list_output;
#[cfg(windows)]
mod projection_schemas;

use serde_json::{Value, json};

#[cfg(windows)]
#[test]
fn default_config_uses_the_setting_and_client_action_registries() {
    let output = super::default_config(Locale::EnUs);
    assert_eq!(output.status, 0);
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.ends_with('\n'));
    let parsed: toml::Value = toml::from_str(&text).unwrap();
    let keys = parsed["keys"].as_table().unwrap();
    assert_eq!(keys.len(), 1, "default action rows must remain comments");
    assert_eq!(keys["prefix"].as_str(), Some("ctrl+b"));

    for option in orchbee_local_site_runtime::WINDOWS_USER_CONFIG_OPTIONS {
        let path_line = format!("# {}", option.path);
        assert!(
            text.lines().any(|line| line == path_line.as_str()),
            "{}",
            option.path
        );
        assert!(text.contains(&format!("#   Default: {}", option.default)));
    }
    let actions = crate::client_view::Bindings::default_windows_key_action_bindings();
    let registered_actions = actions.iter().map(|entry| entry.action).collect::<Vec<_>>();
    assert_eq!(
        registered_actions.as_slice(),
        orchbee_local_site_runtime::WindowsKeySettings::registered_actions()
    );
    for action in actions {
        let value = if action.bindings.is_empty() {
            "[]".to_owned()
        } else {
            let values = action
                .bindings
                .iter()
                .map(|binding| toml_edit::Value::from(binding.as_str()).to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        };
        let action_line = format!("# {} = {value}", action.action);
        assert!(
            text.lines().any(|line| line == action_line.as_str()),
            "missing or stale default binding for {}",
            action.action
        );
    }
    assert!(text.contains("# resize_pane_left = []"));
    assert!(text.contains("# focus_agent = []"));
}

#[cfg(any(unix, windows))]
#[test]
fn integration_list_documents_cover_the_entire_provider_registry() {
    use crate::agent_integrations::model::{Availability, ManagementMode};
    use crate::agent_integrations::{CapsuleStatus, IntegrationId, SnapshotItem, State};
    let schema: Value =
        serde_json::from_str(include_str!("schemas/orchbee.agent.integrations.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for outdated_only in [false, true] {
        let items = IntegrationId::ALL
            .into_iter()
            .map(|integration_id| SnapshotItem {
                availability: Availability::Available,
                integration_id,
                issue: None,
                management_mode: ManagementMode::ManagedLocal,
                state: State::UpdateRequired,
                timed_out: false,
            })
            .collect();
        let prepared = super::integration::Prepared::List {
            items,
            capsule: CapsuleStatus {
                ready: false,
                root: "/isolated-capsule".into(),
                source_kind: "absent",
                binding_failure: false,
                unsupported_version: None,
                declared_payload_version: None,
            },
            outdated_only,
        };
        let output = prepared.render(OutputFormat::Json, Locale::EnUs);
        let document: Value = serde_json::from_slice(&output.stdout).unwrap();
        validator
            .validate(&document)
            .unwrap_or_else(|error| panic!("{document}: {error}"));
        assert_eq!(document["version"], if outdated_only { 4 } else { 3 });
        assert_eq!(
            document["result"]["integrations"].as_array().unwrap().len(),
            IntegrationId::ALL.len()
        );
        let mut legacy = document;
        legacy["version"] = json!(if outdated_only { 2 } else { 1 });
        assert!(!validator.is_valid(&legacy));
    }
}

#[test]
fn launch_profile_documents_preserve_platform_paths_and_legacy_constraints() {
    use super::results::{LaunchProfileCatalogDoc, LaunchProfileCatalogSelection};
    use orchbee_workbench_protocol::{
        LaunchProfileCatalogRevision, LaunchProfileEntry, LaunchProfileId, LaunchProfileName,
        LaunchProfileSelector, LaunchSelection,
    };
    for executable in [
        "/bin/zsh",
        r"C:\Program Files\PowerShell\7\pwsh.exe",
        r"\\?\C:\tools\shell.exe",
    ] {
        let entry = LaunchProfileEntry {
            profile_id: LaunchProfileId::parse("shell").unwrap(),
            name: LaunchProfileName::parse("Development shell").unwrap(),
            launch: LaunchSelection::new(executable.to_owned(), vec!["-NoProfile".to_owned()])
                .unwrap(),
        };
        for list in [false, true] {
            let (command, selection) = if list {
                (
                    QueryCommand::LaunchProfileList,
                    LaunchProfileCatalogSelection::List(vec![entry.clone()]),
                )
            } else {
                (
                    QueryCommand::LaunchProfileGet(LaunchProfileSelector::parse("shell").unwrap()),
                    LaunchProfileCatalogSelection::Profile(entry.clone()),
                )
            };
            let projection = Projection::LaunchProfileCatalog(LaunchProfileCatalogDoc {
                catalog_revision: LaunchProfileCatalogRevision::new(7),
                selection,
            });
            let output = super::success_output(
                projection,
                json!({"kind":"launch-profile-catalog","siteId":ID,"catalogRevision":"7"}),
                &command,
                OutputFormat::Json,
                Locale::EnUs,
            );
            let mut document: Value = serde_json::from_slice(&output.stdout).unwrap();
            let schema: Value = serde_json::from_str(if list {
                include_str!("schemas/orchbee.launch-profile.list.json")
            } else {
                include_str!("schemas/orchbee.launch-profile.get.json")
            })
            .unwrap();
            let validator = jsonschema::validator_for(&schema).unwrap();
            validator.validate(&document).unwrap();
            assert_eq!(document["version"], 2);
            document["version"] = json!(1);
            assert_eq!(validator.is_valid(&document), executable.starts_with('/'));
            document["version"] = json!(2);
            let profile = if list {
                &mut document["result"]["profiles"][0]
            } else {
                &mut document["result"]["profile"]
            };
            profile["launch"]["exec"] = json!("relative-shell.exe");
            assert!(!validator.is_valid(&document));
        }
    }
}

use super::results::{
    ContextRef, Projection, TabDoc, TabRow, TerminalListDetails, TerminalListDocument,
    TerminalListState, TerminalRow,
};
use super::{OutputFormat, output};
use crate::invocation::{ParseFailure, ParseFailureKind, QueryCommand, Target};
use crate::localization::Locale;

const ID: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn terminal_output_text_is_copyable_and_json_preserves_original_data() {
    use super::terminal_output::{ContentSpan, SampleRange, TerminalOutputDoc};
    use crate::invocation::{OutputSelection, OutputSource, OutputTarget};

    let content = "C:\\Users\\luna\\中文 \"🐝\"\u{1b}[31m\u{202e}\n";
    let projection = Projection::TerminalOutput(TerminalOutputDoc {
        terminal: TerminalRow {
            terminal_id: ID.into(),
            worker_id: ID.into(),
            presentation_id: ID.into(),
            panel_id: ID.into(),
            tab_id: ID.into(),
            terminal_context_id: ID.into(),
        },
        selection: OutputSelection {
            source: OutputSource::Recent,
            lines: None,
        },
        revision: 1,
        generation: 1,
        alternate: false,
        columns: 80,
        screen_rows: 24,
        range: SampleRange {
            start: 0,
            count: 1,
            total: 24,
            capacity_omitted: 0,
        },
        span: ContentSpan {
            physical_rows: 1,
            first_row: Some(0),
            last_row: Some(0),
            ..ContentSpan::default()
        },
        text: content.into(),
        ansi: false,
    });
    let command = QueryCommand::TerminalOutput {
        target: OutputTarget::Terminal(Target::Id(ID.into())),
        selection: OutputSelection {
            source: OutputSource::Recent,
            lines: None,
        },
        plain: false,
        ansi: false,
    };
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text =
            output::render_text(|out| super::present::render(out, &projection, &command, locale))
                .unwrap();
        assert_eq!(
            String::from_utf8(text).unwrap(),
            "C:\\Users\\luna\\中文 \"🐝\"\\u{001b}[31m\\u{202e}\n"
        );
    }
    assert_eq!(projection.result()["content"]["text"], content);
}

#[test]
fn usage_advice_uses_command_scope_and_preserves_usage_exits() {
    use crate::invocation::parse_request;
    use std::ffi::OsString;

    for (words, suggestion) in [
        (vec!["agnet", "list"], "agent"),
        (vec!["agent", "lsit"], "list"),
        (vec!["tab", "create", "--labl", "work"], "--label"),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let failure = parse_request(words.iter().map(OsString::from)).unwrap_err();
            let output = super::parse_failure(failure, locale);
            assert_eq!(output.status, 2);
            assert!(output.stdout.is_empty());
            assert!(
                output.stderr.contains(&format!("\"{suggestion}\"")),
                "{words:?}: {}",
                output.stderr
            );
        }
    }
    for option in ["-x", "-x=value"] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let failure = parse_request([OsString::from(option)].into_iter()).unwrap_err();
            let output = super::parse_failure(failure, locale);
            assert_eq!(output.status, 2);
            assert!(output.stdout.is_empty());
            let expected = match locale {
                Locale::EnUs => format!("Unknown option \"{option}\". Run 'orchbee --help'.\n"),
                Locale::ZhCn => format!("未知选项 \"{option}\"。请运行 ‘orchbee --help’。\n"),
            };
            assert_eq!(output.stderr, expected, "{locale:?} {option}");
        }
    }
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let failure = parse_request([OsString::from("agent")].into_iter()).unwrap_err();
        let output = super::parse_failure(failure, locale);
        assert_eq!(output.status, 2);
        assert!(output.stdout.is_empty());
        for command in ["list", "get", "prompt"] {
            assert!(
                output.stderr.contains(&format!("  {command}  ")),
                "{}",
                output.stderr
            );
        }
    }
}

#[test]
fn unknown_option_advice_uses_root_family_and_leaf_grammar_scopes() {
    use crate::invocation::{UsageContext, UsageReason};

    let advice = |context, option: &str| {
        super::usage::suggestions(&UsageReason::UnknownOption {
            context,
            option: option.to_owned(),
        })
    };

    assert!(
        advice(UsageContext::Root, "--hlep")
            .iter()
            .any(|candidate| candidate == "--help")
    );
    #[cfg(windows)]
    {
        assert!(
            advice(UsageContext::Root, "--sesion")
                .iter()
                .any(|candidate| candidate == "--session")
        );
        assert!(
            advice(UsageContext::Root, "--machne")
                .iter()
                .any(|candidate| candidate == "--machine")
        );
    }
    #[cfg(not(windows))]
    {
        assert!(
            !advice(UsageContext::Root, "--sesion")
                .iter()
                .any(|candidate| candidate == "--session")
        );
        assert!(
            !advice(UsageContext::Root, "--machne")
                .iter()
                .any(|candidate| candidate == "--machine")
        );
    }
    assert!(
        advice(UsageContext::Path("agent"), "--prvider")
            .iter()
            .any(|candidate| candidate == "--provider")
    );
    assert!(
        advice(UsageContext::Path("agent integration"), "--adapter-souce")
            .iter()
            .any(|candidate| candidate == "--adapter-source")
    );
    assert!(
        advice(UsageContext::Path("terminal list"), "--formt")
            .iter()
            .any(|candidate| candidate == "--format")
    );
    assert!(
        advice(UsageContext::Path("terminal list"), "--formt=json")
            .iter()
            .any(|candidate| candidate == "--format")
    );
    for option in ["-x", "-x=value"] {
        assert!(advice(UsageContext::Root, option).is_empty(), "{option}");
        assert!(
            advice(UsageContext::Path("agent"), option).is_empty(),
            "{option}"
        );
    }

    assert!(advice(UsageContext::Root, &format!("--{}", "x".repeat(65))).is_empty());
    assert!(advice(UsageContext::Root, "--hélp").is_empty());
    assert!(advice(UsageContext::Root, "--for").len() <= 3);
}

#[test]
fn terminal_lists_use_localized_headers_and_copyable_snapshot_handles() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let rows = (1..=2)
            .map(|number| TerminalRow {
                terminal_id: format!("{number:032x}"),
                worker_id: "1".repeat(32),
                presentation_id: "2".repeat(32),
                panel_id: "3".repeat(32),
                tab_id: "4".repeat(32),
                terminal_context_id: "5".repeat(32),
            })
            .collect();
        let output = super::success_output(
            Projection::TerminalList(TerminalListDocument::bare(rows)),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
            &QueryCommand::TerminalList { filter: None },
            OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), 3);
        assert!(
            text.lines()
                .all(|line| super::escape::display_width(line) <= 80)
        );
        assert!(text.contains("@terminal:1"), "{text}");
        assert!(text.contains("@terminal:2"), "{text}");
        assert!(!text.contains(ID), "full identities belong to JSON: {text}");
        assert!(text.starts_with(locale.cli_labels().terminal_list_headers[0]));
    }
}

#[test]
fn terminal_handles_expand_to_unique_suffixes_without_truncation() {
    let suffix = "a".repeat(30);
    let rows = [format!("1b{suffix}"), format!("2c{suffix}")].map(|terminal_id| TerminalRow {
        terminal_id,
        worker_id: "1".repeat(32),
        presentation_id: "2".repeat(32),
        panel_id: "3".repeat(32),
        tab_id: "4".repeat(32),
        terminal_context_id: "5".repeat(32),
    });
    let output = super::success_output(
        Projection::TerminalList(TerminalListDocument::bare(rows.into())),
        json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
        &QueryCommand::TerminalList { filter: None },
        OutputFormat::Text,
        Locale::EnUs,
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(&format!("@terminal:b{suffix}")), "{text}");
    assert!(text.contains(&format!("@terminal:c{suffix}")), "{text}");
}

#[test]
fn empty_terminal_lists_keep_the_localized_header() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let output = super::success_output(
            Projection::TerminalList(TerminalListDocument::bare(vec![])),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
            &QueryCommand::TerminalList { filter: None },
            OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text.lines().count(), 1);
        for header in locale.cli_labels().terminal_list_headers {
            assert!(text.contains(header), "missing {header:?} in {text:?}");
        }
    }
}

#[test]
fn terminal_list_agent_states_are_localized() {
    let row = TerminalRow {
        terminal_id: ID.into(),
        worker_id: "1".repeat(32),
        presentation_id: "2".repeat(32),
        panel_id: "3".repeat(32),
        tab_id: "4".repeat(32),
        terminal_context_id: "5".repeat(32),
    };
    let states = [
        TerminalListState::Observed,
        TerminalListState::Agent(orchbee_workbench_protocol::PanelAgentState::Idle),
        TerminalListState::Agent(orchbee_workbench_protocol::PanelAgentState::Unknown),
    ];
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for state in states {
            let output = super::success_output(
                Projection::TerminalListHuman(TerminalListDocument {
                    rows: vec![row.clone()],
                    details: vec![TerminalListDetails {
                        placement: None,
                        state: Some(state),
                        ..TerminalListDetails::default()
                    }],
                }),
                json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
                &QueryCommand::TerminalList { filter: None },
                OutputFormat::Text,
                locale,
            );
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(text.contains(locale.cli_terminal_list_state(state)));
        }
    }
}

#[test]
fn terminal_list_keeps_full_paths_and_escapes_terminal_controls() {
    let row = TerminalRow {
        terminal_id: ID.into(),
        worker_id: "1".repeat(32),
        presentation_id: "2".repeat(32),
        panel_id: "3".repeat(32),
        tab_id: "4".repeat(32),
        terminal_context_id: "5".repeat(32),
    };
    let output = super::success_output(
        Projection::TerminalListHuman(TerminalListDocument {
            rows: vec![row],
            details: vec![TerminalListDetails {
                placement: None,
                title: Some("pwsh\n\u{001b}[31m".into()),
                agent: Some("grok".into()),
                state: Some(TerminalListState::Agent(
                    orchbee_workbench_protocol::PanelAgentState::Idle,
                )),
                cwd: Some(format!("C:\\work\\{}\\\u{001b}", "nested\\".repeat(32))),
            }],
        }),
        json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
        &QueryCommand::TerminalList { filter: None },
        OutputFormat::Text,
        Locale::EnUs,
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text.lines().count(),
        2,
        "controls must not add lines: {text:?}"
    );
    assert!(text.contains("@terminal:f"));
    assert!(text.contains("pwsh"));
    assert!(text.contains("grok"));
    // A path is copyable as a whole: never clipped, controls still escaped.
    assert!(text.contains(&"nested\\".repeat(32)));
    assert!(text.contains("\\u{001b}"));
    assert!(text.contains("\\n"));
    assert!(!text.contains('\u{001b}'));
}

#[test]
fn terminal_list_shows_location_handles_and_program_names() {
    let row = TerminalRow {
        terminal_id: ID.into(),
        worker_id: "1".repeat(32),
        presentation_id: "2".repeat(32),
        panel_id: "3".repeat(32),
        tab_id: "4".repeat(32),
        terminal_context_id: "5".repeat(32),
    };
    let placement = |workspace: Option<&str>| TerminalListDetails {
        placement: Some(super::results::TerminalPlacement {
            handle: "@terminal:f".into(),
            workspace: workspace.map(str::to_owned),
            panel: "@panel:3".into(),
        }),
        title: Some(r"C:\Program Files\PowerShell\7\pwsh.exe".into()),
        ..TerminalListDetails::default()
    };
    for (locale, workspace) in [(Locale::EnUs, Some("@workspace:9")), (Locale::ZhCn, None)] {
        let output = super::success_output(
            Projection::TerminalListHuman(TerminalListDocument {
                rows: vec![row.clone()],
                details: vec![placement(workspace)],
            }),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"}),
            &QueryCommand::TerminalList { filter: None },
            OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let line = text.lines().nth(1).unwrap();
        let cells: Vec<_> = line.split_whitespace().collect();
        assert_eq!(cells[0], "@terminal:f", "{text}");
        assert_eq!(cells[1], "pwsh", "{text}");
        assert!(line.contains("@panel:3"), "{text}");
        assert!(
            line.contains(workspace.unwrap_or(locale.cli_labels().standalone)),
            "{text}"
        );
    }
}

#[test]
fn terminal_list_human_enrichment_does_not_change_json_bytes() {
    let rows = vec![TerminalRow {
        terminal_id: ID.into(),
        worker_id: "1".repeat(32),
        presentation_id: "2".repeat(32),
        panel_id: "3".repeat(32),
        tab_id: "4".repeat(32),
        terminal_context_id: "5".repeat(32),
    }];
    let source = json!({"kind":"workbench-snapshot","siteId":ID,"revision":"1"});
    let command = QueryCommand::TerminalList { filter: None };
    let plain = super::success_output(
        Projection::TerminalList(TerminalListDocument::bare(rows.clone())),
        source.clone(),
        &command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    let enriched = super::success_output(
        Projection::TerminalListHuman(TerminalListDocument {
            rows,
            details: vec![TerminalListDetails {
                placement: None,
                title: Some("pwsh".into()),
                agent: Some("grok".into()),
                state: Some(TerminalListState::Observed),
                cwd: Some("C:\\work".into()),
            }],
        }),
        source,
        &command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(plain.stdout, enriched.stdout);
}

fn oversized_terminals() -> Projection {
    // ~42k rows × six full identity fields exceeds the 8 MiB JSON preparation
    // bound without any Site or wire involvement.
    let row = || TerminalRow {
        terminal_id: ID.to_owned(),
        worker_id: "6f7263686265652d6c6f63616c2d7631".to_owned(),
        presentation_id: ID.to_owned(),
        panel_id: ID.to_owned(),
        tab_id: ID.to_owned(),
        terminal_context_id: ID.to_owned(),
    };
    Projection::TerminalList(TerminalListDocument::bare(
        (0..42_000).map(|_| row()).collect(),
    ))
}

fn oversized_snapshot() -> Projection {
    // A syntactically valid machine payload beyond the preparation bound.
    Projection::ApiSnapshot(json!({"blob": "x".repeat(output::LIMIT)}))
}

fn envelope(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("failure envelope is JSON")
}

#[test]
fn terminal_get_preserves_launch_arguments_and_omits_unknown_selection_in_all_locales() {
    use orchbee_client_server_protocol::{
        LaunchSelection, TerminalLaunchFact, TerminalLaunchState,
    };
    let command = QueryCommand::TerminalGet(Target::Id(ID.to_owned()));
    for explicit in [false, true] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for format in [OutputFormat::Json, OutputFormat::Text] {
                let selection = explicit.then(|| {
                    LaunchSelection::new(
                        "C:\\Program Files\\shell.exe".into(),
                        vec!["".into(), "quoted \" text".into(), "中文".into()],
                    )
                    .unwrap()
                });
                let expected = json!(selection);
                let projection = Projection::TerminalGet(super::results::TerminalGetDoc {
                    terminal: TerminalRow {
                        terminal_id: ID.into(),
                        worker_id: ID.into(),
                        presentation_id: ID.into(),
                        panel_id: ID.into(),
                        tab_id: ID.into(),
                        terminal_context_id: ID.into(),
                    },
                    launch: TerminalLaunchFact::new(
                        TerminalLaunchState::Creating,
                        std::num::NonZeroU64::new(1),
                        None,
                        None,
                    )
                    .unwrap(),
                    launch_profile: None,
                    launch_selection: selection,
                });
                let result = super::success_output(
                    projection,
                    super::results::launch_read_source(ID, 1),
                    &command,
                    format,
                    locale,
                );
                assert_eq!(result.status, 0);
                assert!(result.stderr.is_empty());
                match format {
                    OutputFormat::Json => {
                        let doc = envelope(&result.stdout);
                        assert_eq!(doc["version"], 4);
                        let terminal = &doc["result"]["terminal"];
                        if explicit {
                            assert_eq!(terminal["launchSelection"], expected);
                        } else {
                            assert!(terminal.get("launchSelection").is_none());
                        }
                        assert!(terminal.get("launchProfile").is_none());
                    }
                    OutputFormat::Text => {
                        let text = String::from_utf8(result.stdout).unwrap();
                        assert_eq!(text.contains(locale.cli_labels().launch_program), explicit);
                        if explicit {
                            assert!(text.contains("[\"\",\"quoted \\\" text\",\"中文\"]"));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn oversized_json_result_emits_the_selected_envelope() {
    let output = super::success_output(
        oversized_terminals(),
        json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"}),
        &QueryCommand::TerminalList { filter: None },
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    let failure = envelope(&output.stdout);
    assert_eq!(failure["schema"], "orchbee.cli.failure");
    assert_eq!(failure["failure"]["kind"], "output-too-large");
    assert_eq!(
        failure["failure"]["details"]["limitBytes"],
        output::LIMIT as u64
    );
    assert_eq!(failure["source"]["kind"], "workbench-snapshot");
    assert!(!output.stderr.is_empty());
}

#[test]
fn api_snapshot_overflow_obeys_the_caller_format() {
    // `api snapshot` success is always a machine document, but its
    // preparation overflow answers in the caller's selected format.
    let source = json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"});
    let text = super::success_output(
        oversized_snapshot(),
        source.clone(),
        &QueryCommand::ApiSnapshot,
        OutputFormat::Text,
        Locale::EnUs,
    );
    assert_eq!(text.status, 1);
    assert!(
        text.stdout.is_empty(),
        "text caller gets no failure document"
    );
    assert!(!text.stderr.is_empty());

    let json_output = super::success_output(
        oversized_snapshot(),
        source,
        &QueryCommand::ApiSnapshot,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(json_output.status, 1);
    assert_eq!(
        envelope(&json_output.stdout)["failure"]["kind"],
        "output-too-large"
    );
}

#[test]
fn api_schema_failures_follow_the_caller_format() {
    // Text/default: the unknown schema id keeps stdout empty.
    let text = super::schema::answer(Some("orchbee.nope"), OutputFormat::Text, Locale::EnUs);
    assert_eq!(text.status, 3);
    assert!(text.stdout.is_empty());
    assert!(!text.stderr.is_empty());

    // Explicit json: the same failure is a typed envelope with embedded source.
    let json_output = super::schema::answer(Some("orchbee.nope"), OutputFormat::Json, Locale::EnUs);
    assert_eq!(json_output.status, 3);
    let failure = envelope(&json_output.stdout);
    assert_eq!(failure["failure"]["kind"], "not-found");
    assert_eq!(failure["failure"]["details"]["resource"], "schema");
    assert_eq!(failure["failure"]["details"]["target"], "orchbee.nope");
    assert_eq!(failure["source"]["kind"], "embedded");

    // Success is a machine document in both formats.
    for format in [OutputFormat::Text, OutputFormat::Json] {
        let output = super::schema::answer(Some("orchbee.status"), format, Locale::EnUs);
        assert_eq!(output.status, 0);
        let document = envelope(&output.stdout);
        assert_eq!(document["schema"], "orchbee.api.schema");
        assert_eq!(document["source"]["kind"], "embedded");
        assert_eq!(document["result"]["schema"]["schemaId"], "orchbee.status");
    }
}

#[test]
fn parse_failure_envelopes_follow_the_recorded_selection() {
    let usage_failure = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
        usage: None,
    };
    let output = super::parse_failure(usage_failure, Locale::EnUs);
    assert_eq!(output.status, 2);
    assert_eq!(envelope(&output.stdout)["failure"]["kind"], "invalid-usage");

    let silent = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Silent,
        usage: None,
    };
    let output = super::parse_failure(silent, Locale::EnUs);
    assert_eq!(output.status, 2);
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn renderer_path_lands_exactly_on_and_just_over_the_bound() {
    // The real `tab get` renderer streams into the bounded sink: an exact
    // 8 MiB document succeeds and one byte over is a preparation failure
    // with no result prefix on stdout.
    let doc = |blob: usize| {
        Projection::TabGet(TabDoc {
            row: TabRow {
                number: 1,
                panel_count: 1,
                tab_id: ID.to_owned(),
                handle: ID.to_owned(),
                label: "t".to_owned(),
                pinned: false,
                group_id: None,
                context: ContextRef {
                    standalone: true,
                    terminal_context_id: ID.to_owned(),
                    workspace_id: None,
                    workspace_label: None,
                },
            },
            root: json!({"blob": "x".repeat(blob)}),
        })
    };
    let command = QueryCommand::TabGet(Target::Id(ID.to_owned()));
    let baseline =
        output::render_text(|out| super::present::render(out, &doc(0), &command, Locale::EnUs))
            .expect("small render");
    // Growing `blob` grows the rendered body by exactly that many bytes.
    let fill = output::LIMIT - baseline.len();
    let rendered =
        output::render_text(|out| super::present::render(out, &doc(fill), &command, Locale::EnUs))
            .expect("render exactly at the bound");
    assert_eq!(rendered.len(), output::LIMIT);

    let output = super::success_output(
        doc(fill + 1),
        json!({"kind": "workbench-snapshot", "siteId": ID, "revision": "7"}),
        &command,
        OutputFormat::Text,
        Locale::EnUs,
    );
    assert_eq!(output.status, 1);
    assert!(output.stdout.is_empty(), "no result prefix over the bound");
    assert!(!output.stderr.is_empty());
}

#[test]
fn fixed_outputs_are_far_below_the_bound() {
    use crate::invocation::{
        CommandPath, CompletionShell, Family, HelpTopic, Request, parse_request,
    };
    use std::ffi::OsString;
    let mut bodies: Vec<_> = [
        CompletionShell::Bash,
        CompletionShell::Zsh,
        CompletionShell::Fish,
        CompletionShell::PowerShell,
        CompletionShell::Elvish,
    ]
    .into_iter()
    .map(|shell| {
        (
            format!("completion {shell:?}"),
            super::completion::render(shell),
        )
    })
    .collect();
    #[cfg(windows)]
    bodies.push((
        "default config".to_owned(),
        String::from_utf8(super::default_config(Locale::EnUs).stdout).unwrap(),
    ));
    for family in [
        Family::Agent,
        Family::AgentIntegration,
        #[cfg(windows)]
        Family::Command,
        #[cfg(windows)]
        Family::Events,
        #[cfg(windows)]
        Family::Plugin,
        #[cfg(windows)]
        Family::PluginAction,
        #[cfg(windows)]
        Family::PluginLog,
        #[cfg(windows)]
        Family::PluginPane,
        #[cfg(windows)]
        Family::Popup,
        #[cfg(windows)]
        Family::Session,
        Family::TerminalTitle,
        Family::TerminalSession,
        Family::Notification,
        Family::Client,
        Family::Api,
        Family::Completion,
        Family::Config,
        #[cfg(windows)]
        Family::Channel,
        #[cfg(windows)]
        Family::Machine,
        Family::LaunchProfile,
        Family::Panel,
        Family::Layout,
        Family::Project,
        Family::Server,
        Family::Tab,
        Family::Terminal,
        Family::Workspace,
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            bodies.push((
                format!("{family:?} help {locale:?}"),
                super::text::help(HelpTopic::Family(family), locale).to_owned(),
            ));
        }
    }
    // Every `CommandPath` variant, in both locales — the bound must cover the
    // mutation helps as well as the queries.
    let commands = [
        #[cfg(windows)]
        CommandPath::SessionList,
        #[cfg(windows)]
        CommandPath::SessionAttach,
        #[cfg(windows)]
        CommandPath::SessionStop,
        #[cfg(windows)]
        CommandPath::SessionDelete,
        CommandPath::StatusServer,
        CommandPath::StatusClient,
        CommandPath::PanelInput,
        CommandPath::TerminalAttach,
        CommandPath::AgentAttach,
        CommandPath::TerminalObserve,
        CommandPath::TerminalControl,
        CommandPath::TerminalRun,
        #[cfg(windows)]
        CommandPath::TerminalSelectionRead,
        #[cfg(windows)]
        CommandPath::TerminalCopyMotion,
        #[cfg(windows)]
        CommandPath::TerminalCopySearch,
        #[cfg(windows)]
        CommandPath::TerminalLinkResolve,
        #[cfg(windows)]
        CommandPath::TerminalLinkActivate,
        #[cfg(windows)]
        CommandPath::TerminalClear,
        #[cfg(windows)]
        CommandPath::TerminalEditScrollback,
        #[cfg(windows)]
        CommandPath::PanelCreateTerminal,
        #[cfg(windows)]
        CommandPath::PanelGraphicsSet,
        #[cfg(windows)]
        CommandPath::PanelGraphicsClear,
        #[cfg(windows)]
        CommandPath::PanelGraphicsInfo,
        #[cfg(windows)]
        CommandPath::PanelGraphicsStream,
        #[cfg(windows)]
        CommandPath::AgentViewGet,
        #[cfg(windows)]
        CommandPath::AgentViewSet,
        #[cfg(windows)]
        CommandPath::AgentViewClear,
        #[cfg(windows)]
        CommandPath::PluginPaneOpen,
        #[cfg(windows)]
        CommandPath::PluginPaneFocus,
        #[cfg(windows)]
        CommandPath::PluginPaneClose,
        #[cfg(windows)]
        CommandPath::PopupClose,
        CommandPath::ClientList,
        CommandPath::ClientGet,
        CommandPath::ClientControlStatus,
        CommandPath::WorkspaceFocus,
        CommandPath::TabFocus,
        CommandPath::PanelFocus,
        CommandPath::PanelZoom,
        CommandPath::PanelLayout,
        CommandPath::PanelNeighbor,
        CommandPath::PanelEdges,
        CommandPath::PanelResize,
        CommandPath::LayoutSetSplitRatio,
        #[cfg(windows)]
        CommandPath::LayoutExport,
        #[cfg(windows)]
        CommandPath::LayoutApply,
        CommandPath::AgentDoctor,
        CommandPath::AgentExplain,
        CommandPath::AgentFocus,
        CommandPath::AgentAttention,
        CommandPath::AgentGet,
        CommandPath::AgentRead,
        CommandPath::AgentPrompt,
        CommandPath::AgentStart,
        CommandPath::AgentSendKeys,
        CommandPath::AgentList,
        CommandPath::AgentChanges,
        CommandPath::AgentNames,
        CommandPath::AgentRename,
        CommandPath::AgentResolve,
        CommandPath::AgentIntegrationDisable,
        CommandPath::AgentIntegrationEnable,
        CommandPath::AgentIntegrationExplain,
        CommandPath::AgentIntegrationList,
        CommandPath::AgentWait,
        CommandPath::ApiSchema,
        CommandPath::ApiSnapshot,
        #[cfg(windows)]
        CommandPath::ChannelShow,
        #[cfg(windows)]
        CommandPath::MachineList,
        #[cfg(windows)]
        CommandPath::MachineAdd,
        #[cfg(windows)]
        CommandPath::MachineRename,
        #[cfg(windows)]
        CommandPath::MachineRemove,
        #[cfg(windows)]
        CommandPath::MachineEnable,
        #[cfg(windows)]
        CommandPath::MachineDisable,
        #[cfg(windows)]
        CommandPath::ChannelSet,
        #[cfg(windows)]
        CommandPath::Update,
        CommandPath::ConfigCheck,
        CommandPath::ConfigResetKeys,
        #[cfg(windows)]
        CommandPath::ConfigImportHerdr,
        CommandPath::LaunchProfileCreate,
        CommandPath::LaunchProfileDelete,
        CommandPath::LaunchProfileGet,
        CommandPath::LaunchProfileList,
        CommandPath::PanelClose,
        CommandPath::PanelCurrent,
        CommandPath::PanelProcessInfo,
        CommandPath::PanelGet,
        CommandPath::PanelList,
        CommandPath::PanelMove,
        CommandPath::PanelSplit,
        CommandPath::ProjectCreate,
        CommandPath::ProjectDissolve,
        CommandPath::ProjectCloseAll,
        CommandPath::ProjectCloseStatus,
        CommandPath::ProjectCloseResume,
        CommandPath::WorkspaceSetProject,
        CommandPath::WorkspaceCreateProject,
        CommandPath::WorkspaceUngroup,
        CommandPath::ProjectGet,
        CommandPath::ProjectList,
        CommandPath::ProjectRename,
        CommandPath::SiteAgentManifests,
        CommandPath::SiteConfig,
        CommandPath::SiteReloadConfig,
        CommandPath::NotificationShow,
        #[cfg(windows)]
        CommandPath::CommandList,
        #[cfg(windows)]
        CommandPath::EventsSubscribe,
        #[cfg(windows)]
        CommandPath::EventsWait,
        #[cfg(windows)]
        CommandPath::CommandInvoke,
        #[cfg(windows)]
        CommandPath::PluginLink,
        #[cfg(windows)]
        CommandPath::PluginUnlink,
        #[cfg(windows)]
        CommandPath::PluginEnable,
        #[cfg(windows)]
        CommandPath::PluginDisable,
        #[cfg(windows)]
        CommandPath::PluginList,
        #[cfg(windows)]
        CommandPath::PluginConfigDir,
        #[cfg(windows)]
        CommandPath::PluginActionList,
        #[cfg(windows)]
        CommandPath::PluginActionInvoke,
        #[cfg(windows)]
        CommandPath::PluginLogList,
        #[cfg(windows)]
        CommandPath::PluginInstall,
        #[cfg(windows)]
        CommandPath::PluginUninstall,
        CommandPath::TerminalTitleSet,
        CommandPath::TerminalTitleClear,
        CommandPath::SiteReloadAgentManifests,
        CommandPath::SiteUpdateAgentManifests,
        CommandPath::SiteAgentManifestsStatus,
        CommandPath::SiteDiagnose,
        CommandPath::SiteStart,
        CommandPath::SiteStop,
        CommandPath::SiteStopStatus,
        CommandPath::Status,
        CommandPath::TabClose,
        CommandPath::TabCreate,
        CommandPath::TabGet,
        CommandPath::TabList,
        CommandPath::TabMove,
        CommandPath::TabRename,
        CommandPath::PanelRename,
        CommandPath::PanelSwap,
        CommandPath::TerminalGet,
        CommandPath::TerminalList,
        CommandPath::TerminalRead,
        CommandPath::TerminalSend,
        CommandPath::TerminalWaitOutput,
        CommandPath::Tui,
        CommandPath::WorkspaceAcceptDirectory,
        CommandPath::WorkspaceClose,
        CommandPath::WorkspaceCloseResume,
        CommandPath::WorkspaceCloseStatus,
        CommandPath::WorkspaceCreate,
        CommandPath::WorkspaceGet,
        CommandPath::WorkspaceList,
        CommandPath::WorkspaceRename,
        CommandPath::WorkspaceReportMetadata,
        CommandPath::PanelReportMetadata,
        CommandPath::PanelReportAgent,
        CommandPath::PanelReportAgentSession,
        CommandPath::PanelClearAgentAuthority,
        CommandPath::PanelReleaseAgent,
        CommandPath::WorkspaceMove,
        CommandPath::WorkspaceMoveBlock,
        CommandPath::WorktreeList,
        CommandPath::WorktreeCreate,
        CommandPath::WorktreeOpen,
        CommandPath::WorktreeRemove,
        CommandPath::WorktreeStatus,
        CommandPath::WorktreeResume,
    ];
    for command in &commands {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            bodies.push((
                format!("{command:?} help {locale:?}"),
                super::text::help(HelpTopic::Command(*command), locale).to_owned(),
            ));
        }
    }
    // Coverage contract — manual vocabulary, verified both directions:
    // `roots`/`verbs` below are the *current* routing words maintained by
    // hand. The walk proves the pinned `commands` table and the grammar agree
    // on every command reachable through these surfaces; it cannot discover a
    // root or verb that the grammar later admits but nobody lists here. When
    // the grammar grows a new root or verb, extend these lists in the same
    // change — the two assertions then keep the pin table honest for
    // everything the walk does enumerate. Both legal surfaces are walked:
    // trailing `--help` and the `help` prefix.
    let roots = [
        #[cfg(windows)]
        "command",
        #[cfg(windows)]
        "events",
        #[cfg(windows)]
        "popup",
        #[cfg(windows)]
        "machine",
        #[cfg(windows)]
        "channel",
        #[cfg(windows)]
        "update",
        "plugin",
        "notification",
        "session",
        "layout",
        "client",
        "agent",
        "api",
        "completion",
        "config",
        "help",
        "launch-profile",
        "panel",
        "project",
        "server",
        "status",
        "tab",
        "terminal",
        "tui",
        "workspace",
        "worktree",
    ];
    let verbs = [
        "set-project",
        "create-project",
        "ungroup",
        "dissolve",
        "close-all",
        "invoke",
        "subscribe",
        "selection-read",
        "copy-motion",
        "copy-search",
        "link-resolve",
        "link-activate",
        "edit-scrollback",
        "create-terminal",
        "add",
        "show",
        "install",
        "uninstall",
        "link",
        "unlink",
        "enable",
        "disable",
        "config-dir",
        "title",
        "set",
        "clear",
        "show",
        "config",
        "reload-config",
        "agent-manifests",
        "reload-agent-manifests",
        "update-agent-manifests",
        "agent-manifests-status",
        "apply",
        "export",
        "move-block",
        "server",
        "client",
        "input",
        "attach",
        "observe",
        "control",
        "run",
        "changes",
        "swap",
        "resize",
        "set-split-ratio",
        "control-status",
        "focus",
        "attention",
        "zoom",
        "layout",
        "neighbor",
        "edges",
        "accept-directory",
        "check",
        "reset-keys",
        #[cfg(windows)]
        "import-herdr",
        "close",
        "close-resume",
        "close-status",
        "create",
        "current",
        "delete",
        "diagnose",
        "doctor",
        "explain",
        "get",
        "integration",
        "list",
        "move",
        "names",
        "process-info",
        "open",
        "remove",
        "resume",
        "status",
        "read",
        "prompt",
        "rename",
        "report-metadata",
        "report-agent",
        "report-agent-session",
        "clear-agent-authority",
        "release-agent",
        "resolve",
        "schema",
        "send",
        "send-keys",
        "snapshot",
        "split",
        "start",
        "stop",
        "stop-status",
        "wait",
        "wait-output",
    ];
    // `agent integration` nests one level deeper — the pin table must cover
    // its commands through both legal surfaces as well.
    let integration_verbs = ["disable", "enable", "explain", "list"];
    let mut topics: Vec<CommandPath> = Vec::new();
    let mut collect = |arguments: &[&str]| {
        if let Ok(Request::Help(HelpTopic::Command(command))) =
            parse_request(arguments.iter().map(|word| OsString::from(*word)))
            && !topics.contains(&command)
        {
            topics.push(command);
        }
    };
    for root in roots {
        collect(&[root, "--help"]);
        collect(&["help", root]);
        for verb in verbs {
            collect(&[root, verb, "--help"]);
            collect(&["help", root, verb]);
        }
    }
    for verb in integration_verbs {
        collect(&["agent", "integration", verb, "--help"]);
        collect(&["help", "agent", "integration", verb]);
    }
    for verb in ["set", "clear"] {
        collect(&["terminal", "title", verb, "--help"]);
        collect(&["help", "terminal", "title", verb]);
    }
    collect(&["plugin", "action", "list", "--help"]);
    collect(&["help", "plugin", "action", "list"]);
    collect(&["plugin", "action", "invoke", "--help"]);
    collect(&["help", "plugin", "action", "invoke"]);
    collect(&["plugin", "log", "list", "--help"]);
    collect(&["help", "plugin", "log", "list"]);
    for (root, family, verbs) in [
        ("terminal", "session", &["observe", "control"][..]),
        ("plugin", "pane", &["open", "focus", "close"][..]),
        ("agent", "view", &["get", "set", "clear"][..]),
        ("panel", "graphics", &["set", "clear", "info", "stream"][..]),
    ] {
        for verb in verbs {
            collect(&[root, family, verb, "--help"]);
            collect(&["help", root, family, verb]);
        }
    }
    for command in &topics {
        assert!(
            commands.contains(command),
            "command path {command:?} resolved by the grammar is not pinned in the bound test"
        );
    }
    for command in &commands {
        assert!(
            topics.contains(command),
            "pinned command {command:?} is not reachable through the enumerated help paths"
        );
    }
    assert_eq!(
        topics.len(),
        commands.len(),
        "the pinned table must cover every command path the grammar admits"
    );
    // Negative control on the mechanism over the current vocabulary: the walk
    // must actually resolve known mutation topics, and a pin table missing
    // one would trip both assertions above — proving the check is not vacuous.
    // This says nothing about roots/verbs admitted later but never listed.
    assert!(
        topics.contains(&CommandPath::PanelSplit) && topics.contains(&CommandPath::WorkspaceCreate),
        "the grammar walk must resolve real mutation help topics"
    );
    for (name, body) in &bodies {
        // Heuristic guardrail, not a protocol contract: the real emission
        // bound is `output::LIMIT` (8 MiB). Keep fixed bodies at least 16x
        // under it. The current finite command/flag grammar emits at most
        // 284566 bytes (Fish); PowerShell and Elvish stay below 68 KiB.
        // This reminder detects abnormal growth;
        // the separate exact-limit tests enforce the runtime contract.
        assert!(
            body.len() < output::LIMIT / 16,
            "{name}: fixed output {} bytes must stay at least 16x under the bound",
            body.len()
        );
    }
    // History syntax belongs to the read command; the terminal directory links it.
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let body = super::text::help(HelpTopic::Command(CommandPath::TerminalRead), locale);
        for flag in ["--history", "--rows", "--unwrap"] {
            assert!(body.contains(flag), "{locale:?} terminal read lost {flag}");
        }
        assert!(
            body.contains("1000"),
            "history row bound must remain discoverable"
        );
    }
    // The geometry-bearing command help must state the all-or-none group
    // and the exact default so a partial-set refusal reads as intended.
    for command in [CommandPath::TabCreate, CommandPath::PanelSplit] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let body = super::text::help(HelpTopic::Command(command), locale)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                body.contains("[--cols <n> --rows <n> --cell-width <px> --cell-height <px>]"),
                "{locale:?} {command:?} help lost the optional geometry group"
            );
            for needle in match locale {
                Locale::EnUs => ["all-or-none", "80x24"],
                Locale::ZhCn => ["全有或全无", "80x24"],
            } {
                assert!(
                    body.contains(needle),
                    "{locale:?} {command:?} help lost '{needle}'"
                );
            }
        }
    }
}

#[test]
fn failure_documents_stay_well_below_the_bound() {
    // A maximally populated failure envelope must never overflow preparation.
    let failure =
        super::Failure::new(super::FailureKind::Ambiguous).details(serde_json::Map::from_iter([
            ("resource".to_owned(), json!("tab")),
            ("target".to_owned(), json!("x".repeat(512))),
            ("candidates".to_owned(), json!(vec!["y".repeat(32); 8])),
            ("totalCount".to_owned(), json!(u64::MAX)),
        ]));
    let output = super::failure_output(failure, Some(OutputFormat::Json), Locale::ZhCn);
    assert!(!output.stdout.is_empty(), "json failure envelope emitted");
    assert!(output.stdout.len() < 64 * 1024);
}

/// The usage reason is a stderr-only diagnostic: the JSON failure document is
/// byte-identical whether or not a reason rides along.
#[test]
fn usage_reason_never_reaches_the_machine_envelope() {
    let reasoned = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
        usage: Some(crate::invocation::UsageReason::UnknownOption {
            context: crate::invocation::UsageContext::Root,
            option: "--bogus".to_owned(),
        }),
    };
    let plain = ParseFailure {
        kind: ParseFailureKind::InvalidUsage,
        envelope: crate::invocation::FailureEnvelope::Json,
        usage: None,
    };
    let with_reason = super::parse_failure(reasoned, Locale::EnUs);
    let without = super::parse_failure(plain, Locale::EnUs);
    assert_eq!(with_reason.status, 2);
    assert_eq!(with_reason.status, without.status);
    assert_eq!(with_reason.stdout, without.stdout);
    assert_eq!(
        envelope(&with_reason.stdout),
        json!({
            "schema": "orchbee.cli.failure",
            "version": 1,
            "failure": { "kind": "invalid-usage" }
        })
    );
    assert!(with_reason.stdout.iter().all(|byte| byte.is_ascii()));
    assert_eq!(
        with_reason.stderr,
        "Unknown option \"--bogus\". Did you mean \"--focus\"? Run 'orchbee --help'.\n"
    );
    assert_ne!(with_reason.stderr, without.stderr);
}

/// `terminal wait-output` reserves two failure kinds: timeout exits 7 and a
/// signal-cancelled wait exits 130, each serializing its own `kind` string.
/// The whole matrix stays pinned so no existing kind can drift.
#[test]
fn wait_failure_kinds_map_to_their_exit_codes() {
    use super::FailureKind;
    let cases: &[(FailureKind, &str, u8)] = &[
        (FailureKind::Ambiguous, "ambiguous", 3),
        (FailureKind::Cancelled, "cancelled", 130),
        (FailureKind::CheckFailed, "check-failed", 1),
        (FailureKind::Denied, "denied", 3),
        (FailureKind::Incompatible, "incompatible", 6),
        (FailureKind::InternalFailure, "internal-failure", 1),
        (FailureKind::InvalidIdentifier, "invalid-identifier", 2),
        (FailureKind::InvalidUsage, "invalid-usage", 2),
        (FailureKind::NotFound, "not-found", 3),
        (FailureKind::OutcomeUnknown, "outcome-unknown", 8),
        (FailureKind::OutputTooLarge, "output-too-large", 1),
        (FailureKind::ProtocolInvalid, "protocol-invalid", 5),
        (FailureKind::Rejected, "rejected", 3),
        (FailureKind::ServiceUnavailable, "service-unavailable", 4),
        (FailureKind::SessionCapacity, "session-capacity", 4),
        (FailureKind::Timeout, "timeout", 7),
        (FailureKind::TransportLoss, "transport-loss", 5),
    ];
    for (kind, name, code) in cases {
        assert_eq!(kind.as_str(), *name, "{kind:?}");
        assert_eq!(kind.exit_code(), *code, "{kind:?}");
    }
    // Both new kinds emit their typed JSON envelope and localized stderr.
    for (kind, name, code) in [
        (FailureKind::Timeout, "timeout", 7_u8),
        (FailureKind::Cancelled, "cancelled", 130_u8),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let output =
                super::failure_output(super::Failure::new(kind), Some(OutputFormat::Json), locale);
            assert_eq!(output.status, code, "{kind:?} {locale:?}");
            assert_eq!(envelope(&output.stdout)["failure"]["kind"], name);
            assert!(!output.stderr.is_empty(), "localized diagnostic");
        }
    }
    // The wait result schema is registered under its published id.
    let document = super::schema::answer(
        Some("orchbee.terminal.wait-output"),
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(document.status, 0);
    let body = envelope(&document.stdout);
    assert_eq!(
        body["result"]["schema"]["schemaId"],
        "orchbee.terminal.wait-output"
    );
    assert_eq!(
        body["result"]["schema"]["document"]["$id"],
        "orchbee.terminal.wait-output"
    );
}

/// A `terminal send` source-stage timeout or cancellation diagnoses the
/// input read — nothing was awaited and nothing was sent. The wait stage
/// keeps its own wording. Both locales and both routing paths are pinned.
#[cfg(target_os = "linux")]
#[test]
fn source_stage_failures_diagnose_the_input_read() {
    use super::{Failure, FailureKind, source_failure_output};
    let input_details = || {
        serde_json::Map::from_iter([
            ("stage".to_owned(), json!("input")),
            ("inputSource".to_owned(), json!("stdin")),
        ])
    };
    for kind in [FailureKind::Timeout, FailureKind::Cancelled] {
        for (locale, expected, forbidden) in [
            (
                Locale::EnUs,
                match kind {
                    FailureKind::Timeout => "could not be read within the time limit",
                    _ => "Reading the declared input was cancelled",
                },
                match kind {
                    FailureKind::Timeout => "wait deadline elapsed",
                    _ => "wait was cancelled",
                },
            ),
            (
                Locale::ZhCn,
                match kind {
                    FailureKind::Timeout => "未能在时限内读取",
                    _ => "读取已被取消",
                },
                match kind {
                    FailureKind::Timeout => "未出现目标文本",
                    _ => "未报告任何匹配",
                },
            ),
        ] {
            let failure = Failure::new(kind).details(input_details());
            let stderr = locale.cli_failure(&failure);
            assert!(
                stderr.contains(expected),
                "{kind:?} {locale:?} must diagnose the input read, got {stderr:?}"
            );
            assert!(
                !stderr.contains(forbidden),
                "{kind:?} {locale:?} must not claim a wait ran, got {stderr:?}"
            );
        }
    }
    // Both routing paths carry the same source-stage wording.
    for (format, expect_stdout) in [(OutputFormat::Text, false), (OutputFormat::Json, true)] {
        let failure = Failure::new(FailureKind::Timeout).details(input_details());
        let output = source_failure_output(failure, 7, format, Locale::EnUs);
        assert_eq!(output.stdout.is_empty(), !expect_stdout);
        assert!(
            output.stderr.contains("could not be read"),
            "stderr diagnoses the input read on the {format:?} path"
        );
        if expect_stdout {
            let body = envelope(&output.stdout);
            assert_eq!(body["schema"], "orchbee.cli.failure.source");
            assert_eq!(body["failure"]["kind"], "timeout");
        }
    }
}

/// The wait stage and the detail-free fallback keep the established wait
/// wording — the source-stage carve-out never leaks into other producers.
#[test]
fn wait_stage_failures_keep_the_wait_wording() {
    use super::{Failure, FailureKind};
    for kind in [FailureKind::Timeout, FailureKind::Cancelled] {
        for details in [
            Some(serde_json::Map::from_iter([(
                "stage".to_owned(),
                json!("wait"),
            )])),
            None,
        ] {
            let mut failure = Failure::new(kind);
            if let Some(details) = details {
                failure = failure.details(details);
            }
            let expected = match (kind, &failure.details) {
                (FailureKind::Timeout, _) => "wait deadline elapsed",
                _ => "wait was cancelled",
            };
            assert!(
                Locale::EnUs.cli_failure(&failure).contains(expected),
                "{kind:?} without stage:\"input\" keeps the wait wording"
            );
        }
    }
}

/// A captured signal on an Agent read answers `orchbee.cli.failure.agent`
/// with the cancelled kind at 130 and the noncloneable finalization riding
/// the common emit path — a cancelled read carries no correlated source.
#[cfg(target_os = "linux")]
#[test]
fn agent_cancellation_emits_agent_envelope_with_finalization() {
    use super::{Failure, FailureKind, SourceFinalization, query};
    let command = QueryCommand::AgentList {
        client: None,
        include_ended: false,
        limit: 50,
        cursor: None,
    };
    let mut details = serde_json::Map::new();
    details.insert("stage".to_owned(), json!("agent-read"));
    details.insert("operation".to_owned(), json!("list"));
    let output = super::agent_output(
        Err(query::AgentQueryError::Cancelled(Box::new(
            query::AgentCancelled {
                failure: Failure::new(FailureKind::Cancelled).details(details),
                finalization: SourceFinalization::failing(),
            },
        ))),
        &command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(output.status, 130);
    assert!(
        output.finalization.is_some(),
        "the owed finalization reaches the common emit"
    );
    let body = envelope(&output.stdout);
    assert_eq!(body["schema"], "orchbee.cli.failure.agent");
    assert_eq!(body["failure"]["kind"], "cancelled");
    assert_eq!(body["failure"]["details"]["stage"], "agent-read");
    assert_eq!(body["failure"]["details"]["operation"], "list");
    assert!(
        body.get("source").is_none() || body["source"].is_null(),
        "a cancelled read publishes no correlated source"
    );
}

/// A failed termination-signal registration refuses an integration
/// invocation before any provider context, inspection, or effect — the
/// armed seam makes `install` fail once, and the refusal carries the
/// existing `internal-failure` kind at exit 1. A second install succeeds,
/// proving the refusal came from the injected fault, not ambient state.
#[cfg(unix)]
#[test]
fn integration_signal_install_failure_refuses_before_effects() {
    use super::{FailureKind, integration};
    use crate::agent_integrations::IntegrationId;
    use crate::invocation::AgentIntegrationCommand;

    crate::termination_signal::arm_install_fault();
    let command = AgentIntegrationCommand::Enable {
        provider: IntegrationId::ClaudeCode,
        adapter_source: None,
    };
    let failure = match integration::run_local(&command) {
        Err(failure) => failure,
        Ok(_) => panic!("armed install fault must refuse the invocation"),
    };
    assert_eq!(failure.kind, FailureKind::InternalFailure);
    assert_eq!(failure.kind.exit_code(), 1);
    assert!(
        crate::termination_signal::TerminationSignal::install().is_ok(),
        "the armed fault is one-shot; install itself stays healthy"
    );
}

/// A cancelled integration mutation projects `outcome:"cancelled"` — the
/// schema must accept the real emitted document and the exit stays 130.
/// The report is an entry-gate cancel: no attempted step, no observation.
#[cfg(unix)]
#[test]
fn integration_cancelled_mutation_document_validates_against_schema() {
    use super::integration::Prepared;
    use crate::agent_integrations::model::Provenance;
    use crate::agent_integrations::{Direction, IntegrationId, MutationReport, Verdict};

    let report = MutationReport {
        integration_id: IntegrationId::ClaudeCode,
        direction: Direction::Enable,
        attempted: vec![],
        residuals: vec![],
        observed: None,
        provenance: Provenance::PreEffect,
        effect_begun: false,
        verdict: Verdict::Cancelled,
    };
    let output = Prepared::Mutation { report }.render(OutputFormat::Json, Locale::EnUs);
    assert_eq!(output.status, 130, "cancelled keeps exit 130");
    let document: Value = serde_json::from_slice(&output.stdout).expect("mutation JSON");
    assert_eq!(document["result"]["outcome"], "cancelled");

    let schema: Value = serde_json::from_str(include_str!(
        "schemas/orchbee.agent.integration.mutation.json"
    ))
    .expect("mutation schema parses");
    let validator = jsonschema::draft202012::new(&schema).expect("schema compiles");
    assert!(
        validator.is_valid(&document),
        "real cancelled projection must validate: {document}"
    );

    // The vocabulary stays closed — an unknown outcome word is rejected.
    let mut bad = document.clone();
    bad["result"]["outcome"] = json!("aborted");
    assert!(
        !validator.is_valid(&bad),
        "a non-vocabulary outcome must fail the schema"
    );
}

#[test]
fn changes_text_localizes_reset_and_keeps_recovery_cursor() {
    use orchbee_agent_observation_protocol::{
        AgentChangesCursor, AgentChangesOutcome, AgentSafeInteger,
    };
    let token = format!("v1.{}.{}", "A".repeat(59), "A".repeat(43));
    for (locale, expected) in [(Locale::EnUs, "History lost"), (Locale::ZhCn, "历史已丢失")] {
        let projection = Projection::AgentChanges(AgentChangesOutcome::ResetRequired {
            cursor: AgentChangesCursor::parse(&token).unwrap(),
            revision: AgentSafeInteger::new(77),
        });
        let output = super::success_output(
            projection,
            json!({"kind":"agent-read","requestId":ID}),
            &QueryCommand::AgentChanges {
                limit: 50,
                cursor: None,
            },
            OutputFormat::Text,
            locale,
        );
        assert_eq!(output.status, 0);
        let rendered = String::from_utf8(output.stdout).unwrap();
        assert!(rendered.contains(expected));
        assert!(rendered.contains("77"));
        assert!(rendered.contains(&token));
    }
}

/// Detail views open with one sentence and keep worker, context and
/// presentation identities, and raw launch structures, out of text.
#[test]
fn terminal_get_text_is_a_sentence_and_aligned_labels() {
    use orchbee_client_server_protocol::{TerminalLaunchFact, TerminalLaunchState};
    let worker = "9".repeat(32);
    let context = "8".repeat(32);
    let panel = "7".repeat(32);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let projection = Projection::TerminalGet(super::results::TerminalGetDoc {
            terminal: TerminalRow {
                terminal_id: ID.into(),
                worker_id: worker.clone(),
                presentation_id: "6".repeat(32),
                panel_id: panel.clone(),
                tab_id: "5".repeat(32),
                terminal_context_id: context.clone(),
            },
            launch: TerminalLaunchFact::new(
                TerminalLaunchState::Creating,
                std::num::NonZeroU64::new(1),
                None,
                None,
            )
            .unwrap(),
            launch_profile: None,
            launch_selection: None,
        });
        let output = super::success_output(
            projection,
            super::results::launch_read_source(ID, 1),
            &QueryCommand::TerminalGet(Target::Id(ID.to_owned())),
            OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.starts_with(&locale.cli_terminal_summary(&panel)),
            "{text}"
        );
        assert!(text.contains(locale.cli_launch_state("creating")), "{text}");
        for internal in [&worker, &context, &"6".repeat(32)] {
            assert!(!text.contains(internal.as_str()), "{text}");
        }
        assert!(!text.contains("createOutcome"), "{text}");
    }
}

#[test]
fn usage_advice_offers_synonyms_and_the_label_form() {
    use crate::invocation::{UsageContext, UsageReason};
    let unknown = |context, word: &str| {
        super::usage::suggestions(&UsageReason::UnknownCommand {
            context,
            word: word.to_owned(),
        })
    };
    assert_eq!(
        unknown(UsageContext::Path("workspace"), "delete")[0],
        "close"
    );
    assert_eq!(unknown(UsageContext::Path("tab"), "rm")[0], "close");
    let stray = |context, value: &str| {
        super::usage::suggestions(&UsageReason::UnexpectedArgument {
            context,
            value: value.to_owned(),
        })
    };
    assert_eq!(
        stray(UsageContext::Path("tab rename"), "main"),
        ["--label main"]
    );
    assert_eq!(
        stray(UsageContext::Path("project create"), "demo"),
        ["--label demo"]
    );
    assert!(stray(UsageContext::Path("terminal list"), "x").is_empty());
}
