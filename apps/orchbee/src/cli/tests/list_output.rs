use super::ID;
use crate::cli::{
    OutputFormat, escape,
    results::{ContextRef, Projection, StatusDoc, TabRow, WorkspaceRow},
    success_output,
};
use crate::invocation::QueryCommand;
use crate::localization::Locale;
use serde_json::{Value, json};

#[test]
fn workspace_and_tab_lists_show_localized_names_exact_handles_and_owners() {
    let workspace_id = "abcdef0123456789abcdef0123456789";
    let tab_id = "fedcba9876543210fedcba9876543210";
    let workspace = WorkspaceRow {
        number: 1,
        tab_count: 2,
        panel_count: 3,
        workspace_id: workspace_id.to_owned(),
        handle: workspace_id.to_owned(),
        label: "工作区\n\u{1b}[31m".to_owned(),
        project_id: Some("1234567890abcdef1234567890abcdef".to_owned()),
        project_label: Some("\u{1b}Project that needs clipping".to_owned()),
        primary_directory_id: Some("directory-id-is-not-a-path".to_owned()),
        terminal_context_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
    };
    let ungrouped_workspace = WorkspaceRow {
        number: 2,
        tab_count: 0,
        panel_count: 0,
        workspace_id: "66666666666666666666666666666666".to_owned(),
        handle: "66666666666666666666666666666666".to_owned(),
        label: "Unsorted".to_owned(),
        project_id: None,
        project_label: None,
        primary_directory_id: None,
        terminal_context_id: "77777777777777777777777777777777".to_owned(),
    };
    let tab = TabRow {
        number: 1,
        panel_count: 4,
        tab_id: tab_id.to_owned(),
        handle: tab_id.to_owned(),
        label: "终端 🐝".to_owned(),
        pinned: true,
        group_id: Some("group-id".to_owned()),
        context: ContextRef {
            standalone: false,
            terminal_context_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            workspace_id: Some(workspace_id.to_owned()),
            workspace_label: Some("\u{1b}Workspace label that needs clipping".to_owned()),
        },
    };
    let standalone_tab = TabRow {
        number: 1,
        panel_count: 1,
        tab_id: "11111111111111111111111111111111".to_owned(),
        handle: "11111111111111111111111111111111".to_owned(),
        label: "Console".to_owned(),
        pinned: false,
        group_id: None,
        context: ContextRef {
            standalone: true,
            terminal_context_id: "22222222222222222222222222222222".to_owned(),
            workspace_id: None,
            workspace_label: None,
        },
    };
    let unavailable_owner_tab = TabRow {
        number: 2,
        panel_count: 1,
        tab_id: "33333333333333333333333333333333".to_owned(),
        handle: "33333333333333333333333333333333".to_owned(),
        label: "Unavailable owner".to_owned(),
        pinned: false,
        group_id: None,
        context: ContextRef {
            standalone: false,
            terminal_context_id: "44444444444444444444444444444444".to_owned(),
            workspace_id: Some("55555555555555555555555555555555".to_owned()),
            workspace_label: None,
        },
    };

    for (locale, workspace_headers, tab_headers) in [
        (
            Locale::EnUs,
            ["HANDLE", "NAME", "PROJECT"],
            ["HANDLE", "NAME", "PANELS", "WORKSPACE"],
        ),
        (
            Locale::ZhCn,
            ["句柄", "名称", "项目"],
            ["句柄", "名称", "面板", "工作区"],
        ),
    ] {
        let workspace_output = success_output(
            Projection::WorkspaceList(vec![workspace.clone(), ungrouped_workspace.clone()]),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"9"}),
            &QueryCommand::WorkspaceList { project: None },
            OutputFormat::Text,
            locale,
        );
        let workspace_text = String::from_utf8(workspace_output.stdout).unwrap();
        let workspace_handle = workspace_text
            .lines()
            .nth(1)
            .and_then(|line| line.split_whitespace().next())
            .unwrap();
        assert_eq!(workspace_handle, workspace_id);
        assert!(!workspace_text.contains("@workspace:"));
        assert!(
            crate::invocation::parse_request(
                ["workspace", "get", workspace_handle]
                    .into_iter()
                    .map(std::ffi::OsString::from)
            )
            .is_ok()
        );
        let rejected_workspace_handle = format!("@workspace:{workspace_id}");
        assert!(
            crate::invocation::parse_request(
                ["workspace", "get", rejected_workspace_handle.as_str()]
                    .into_iter()
                    .map(std::ffi::OsString::from)
            )
            .is_err()
        );
        assert!(workspace_text.contains("\"工作区\\n…\""));
        assert!(workspace_text.contains("\"\\u{001b}Pro…\""));
        assert!(workspace_text.contains(&format!("\"{}\"", locale.cli_ungrouped())));
        assert!(!workspace_text.contains(&workspace.terminal_context_id));
        assert!(!workspace_text.contains("directory-id-is-not-a-path"));
        assert!(!workspace_text.contains('\u{1b}'));
        for header in workspace_headers {
            assert!(workspace_text.lines().next().unwrap().contains(header));
        }
        assert!(
            workspace_text
                .lines()
                .all(|line| escape::display_width(line) <= 80)
        );

        let tab_output = success_output(
            Projection::TabList(vec![
                tab.clone(),
                standalone_tab.clone(),
                unavailable_owner_tab.clone(),
            ]),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"9"}),
            &QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            OutputFormat::Text,
            locale,
        );
        let tab_text = String::from_utf8(tab_output.stdout).unwrap();
        let tab_handle = tab_text
            .lines()
            .nth(1)
            .and_then(|line| line.split_whitespace().next())
            .unwrap();
        assert_eq!(tab_handle, tab_id);
        assert!(!tab_text.contains("@tab:"));
        assert!(
            crate::invocation::parse_request(
                ["tab", "get", tab_handle]
                    .into_iter()
                    .map(std::ffi::OsString::from)
            )
            .is_ok()
        );
        let rejected_tab_handle = format!("@tab:{tab_id}");
        assert!(
            crate::invocation::parse_request(
                ["tab", "get", rejected_tab_handle.as_str()]
                    .into_iter()
                    .map(std::ffi::OsString::from)
            )
            .is_err()
        );
        assert!(tab_text.contains("\"终端 🐝\""));
        assert!(tab_text.contains("\"\\u{001b}Wo…\""));
        assert!(tab_text.contains(locale.cli_labels().standalone));
        assert!(tab_text.contains("-"));
        assert!(!tab_text.contains(&tab.context.terminal_context_id));
        assert!(!tab_text.contains(workspace_id));
        assert!(!tab_text.contains('\u{1b}'));
        for header in tab_headers {
            assert!(tab_text.lines().next().unwrap().contains(header));
        }
        assert!(
            tab_text
                .lines()
                .all(|line| escape::display_width(line) <= 80)
        );
    }
}

#[test]
fn empty_workspace_and_tab_lists_keep_localized_table_headers() {
    for (locale, workspace_headers, tab_headers) in [
        (
            Locale::EnUs,
            ["HANDLE", "NAME", "PROJECT"],
            ["HANDLE", "NAME", "PANELS", "WORKSPACE"],
        ),
        (
            Locale::ZhCn,
            ["句柄", "名称", "项目"],
            ["句柄", "名称", "面板", "工作区"],
        ),
    ] {
        let workspace_output = success_output(
            Projection::WorkspaceList(vec![]),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"9"}),
            &QueryCommand::WorkspaceList { project: None },
            OutputFormat::Text,
            locale,
        );
        let workspace_text = String::from_utf8(workspace_output.stdout).unwrap();
        assert_eq!(workspace_text.lines().count(), 1);
        for header in workspace_headers {
            assert!(workspace_text.contains(header));
        }

        let tab_output = success_output(
            Projection::TabList(vec![]),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"9"}),
            &QueryCommand::TabList {
                workspace: None,
                standalone: false,
            },
            OutputFormat::Text,
            locale,
        );
        let tab_text = String::from_utf8(tab_output.stdout).unwrap();
        assert_eq!(tab_text.lines().count(), 1);
        for header in tab_headers {
            assert!(tab_text.contains(header));
        }
    }
}

#[test]
fn status_text_reports_selected_session_identity_client_version_and_counts() {
    for (
        locale,
        session,
        session_name,
        client_version,
        running,
        counts,
        projects,
        workspaces,
        tabs,
        panels,
        terminals,
    ) in [
        (
            Locale::EnUs,
            "Session",
            "Session name",
            "Client version",
            "running",
            "Counts",
            "projects",
            "workspaces",
            "tabs",
            "panels",
            "bound terminals",
        ),
        (
            Locale::ZhCn,
            "会话",
            "会话名称",
            "客户端版本",
            "运行中",
            "计数",
            "项目",
            "工作区",
            "标签",
            "面板",
            "已绑定终端",
        ),
    ] {
        let output = success_output(
            Projection::Status(StatusDoc {
                site_id: ID.to_owned(),
                revision: "999".to_owned(),
                session_name: Some("work".to_owned()),
                client_version: env!("CARGO_PKG_VERSION").to_owned(),
                projects: 2,
                workspaces: 3,
                tabs: 4,
                panels: 5,
                bound_terminals: 6,
            }),
            json!({"kind":"workbench-snapshot","siteId":ID,"revision":"999"}),
            &QueryCommand::Status,
            OutputFormat::Text,
            locale,
        );
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains(session));
        assert!(text.contains(session_name));
        assert!(text.contains("work"));
        assert!(text.contains(client_version));
        assert!(text.contains(env!("CARGO_PKG_VERSION")));
        assert!(text.contains(running));
        assert!(text.contains(counts));
        assert!(text.contains(projects));
        assert!(text.contains(workspaces));
        assert!(text.contains(tabs));
        assert!(text.contains(panels));
        assert!(text.contains(terminals));
        for count in 2..=6 {
            assert!(text.contains(&count.to_string()));
        }
        assert!(!text.contains(ID));
        assert!(!text.contains("999"));
        assert!(!text.contains("Worker"));
        assert!(!text.contains("unknown"));

        let lines: Vec<_> = text.lines().collect();
        let count_heading = lines.iter().find(|line| line.contains(counts)).unwrap();
        assert!(count_heading.starts_with("  "));
        let count_value_columns: Vec<_> = [projects, workspaces, tabs, panels, terminals]
            .iter()
            .map(|label| {
                let line = lines.iter().find(|line| line.contains(label)).unwrap();
                let value = line
                    .char_indices()
                    .find(|(_, character)| character.is_ascii_digit())
                    .map(|(index, _)| index)
                    .expect("count value");
                crate::cli::escape::display_width(&line[..value])
            })
            .collect();
        assert!(
            count_value_columns
                .iter()
                .all(|column| *column == count_value_columns[0])
        );
    }

    let unnamed_output = success_output(
        Projection::Status(StatusDoc {
            site_id: ID.to_owned(),
            revision: "999".to_owned(),
            session_name: None,
            client_version: env!("CARGO_PKG_VERSION").to_owned(),
            projects: 0,
            workspaces: 0,
            tabs: 0,
            panels: 0,
            bound_terminals: 0,
        }),
        json!({"kind":"workbench-snapshot","siteId":ID,"revision":"999"}),
        &QueryCommand::Status,
        OutputFormat::Text,
        Locale::EnUs,
    );
    let unnamed_text = String::from_utf8(unnamed_output.stdout).unwrap();
    assert!(!unnamed_text.contains("Session name"));
    assert!(!unnamed_text.contains("default"));
}

#[test]
fn list_and_status_machine_documents_keep_their_exact_json_bytes() {
    fn bytes(value: Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(&value).unwrap();
        bytes.push(b'\n');
        bytes
    }

    let source = json!({"kind":"workbench-snapshot","siteId":ID,"revision":"9"});
    let workspace_id = "abcdef0123456789abcdef0123456789";
    let workspace = WorkspaceRow {
        number: 1,
        tab_count: 2,
        panel_count: 3,
        workspace_id: workspace_id.to_owned(),
        handle: workspace_id.to_owned(),
        label: "Work".to_owned(),
        project_id: Some("1234567890abcdef1234567890abcdef".to_owned()),
        project_label: Some("Project".to_owned()),
        primary_directory_id: Some("99999999999999999999999999999999".to_owned()),
        terminal_context_id: "88888888888888888888888888888888".to_owned(),
    };
    let workspace_command = QueryCommand::WorkspaceList { project: None };
    let workspace_output = success_output(
        Projection::WorkspaceList(vec![workspace.clone()]),
        source.clone(),
        &workspace_command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(
        workspace_output.stdout,
        bytes(json!({
            "schema":"orchbee.workspace.list",
            "version":8,
            "source":source,
            "result":{"workspaces":[{
                "workspaceId":workspace.workspace_id,
                "number":workspace.number,
                "tabCount":workspace.tab_count,
                "panelCount":workspace.panel_count,
                "label":workspace.label,
                "projectId":workspace.project_id,
                "primaryDirectoryId":workspace.primary_directory_id,
                "terminalContextId":workspace.terminal_context_id,
            }]},
        }))
    );

    let tab = TabRow {
        number: 1,
        panel_count: 4,
        tab_id: "fedcba9876543210fedcba9876543210".to_owned(),
        handle: "fedcba9876543210fedcba9876543210".to_owned(),
        label: "Terminal".to_owned(),
        pinned: true,
        group_id: Some("group".to_owned()),
        context: ContextRef {
            standalone: false,
            terminal_context_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
            workspace_id: Some(workspace_id.to_owned()),
            workspace_label: Some("Work".to_owned()),
        },
    };
    let tab_command = QueryCommand::TabList {
        workspace: None,
        standalone: false,
    };
    let tab_output = success_output(
        Projection::TabList(vec![tab.clone()]),
        source.clone(),
        &tab_command,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(
        tab_output.stdout,
        bytes(json!({
            "schema":"orchbee.tab.list",
            "version":5,
            "source":source,
            "result":{"tabs":[{
                "tabId":tab.tab_id,
                "number":tab.number,
                "panelCount":tab.panel_count,
                "label":tab.label,
                "pinned":tab.pinned,
                "groupId":tab.group_id,
                "context":{
                    "kind":"workspace",
                    "terminalContextId":tab.context.terminal_context_id,
                    "workspaceId":workspace_id,
                },
            }]},
        }))
    );

    let status = StatusDoc {
        site_id: ID.to_owned(),
        revision: "9".to_owned(),
        session_name: Some("work".to_owned()),
        client_version: env!("CARGO_PKG_VERSION").to_owned(),
        projects: 1,
        workspaces: 2,
        tabs: 3,
        panels: 4,
        bound_terminals: 5,
    };
    let status_output = success_output(
        Projection::Status(status.clone()),
        source.clone(),
        &QueryCommand::Status,
        OutputFormat::Json,
        Locale::EnUs,
    );
    assert_eq!(
        status_output.stdout,
        bytes(json!({
            "schema":"orchbee.status",
            "version":1,
            "source":source,
            "result":{
                "site":{"reachable":true,"siteId":status.site_id,"revision":status.revision},
                "counts":{
                    "projects":status.projects,
                    "workspaces":status.workspaces,
                    "tabs":status.tabs,
                    "panels":status.panels,
                    "boundTerminals":status.bound_terminals,
                },
                "worker":{"status":"unknown","reason":"not-in-workbench-snapshot"},
            },
        }))
    );
}
