//! A POSIX Host's samples, in the shapes macOS reports, name the Agent a Windows sample of the
//! same launch would. Only POSIX builds decode the POSIX Host's quoted command lines.
use super::*;
use orchbee_terminal_protocol::{AttachmentId, TerminalId, TerminalProcess};

fn process(pid: u32, parent: u32, image: &str, command_line: Option<&str>) -> TerminalProcess {
    TerminalProcess {
        pid,
        parent_pid: Some(parent),
        creation_time: u64::from(pid) * 10,
        console: TerminalProcessConsole::Attached,
        image: Some(image.into()),
        command_line: command_line.map(Into::into),
        current_directory: None,
    }
}

fn identified(members: Vec<TerminalProcess>) -> Option<(u32, AgentReportProvider)> {
    let shell = process(10, 1, "/bin/zsh", Some("-zsh"));
    let sample = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        launch_kind: None,
        root_pid: 10,
        processes: std::iter::once(shell).chain(members).collect(),
    };
    identify(&sample).map(|selected| (selected.pid(), selected.provider))
}

#[test]
fn macos_samples_name_the_agent_its_launch_shape_names() {
    use AgentReportProvider as P;
    const NODE: &str = "/opt/homebrew/Cellar/node/24.1.0/bin/node";
    let mcp = |parent| {
        process(
            30,
            parent,
            NODE,
            Some("node '/Users/u/Library/Application Support/mcp/server.js' --stdio"),
        )
    };
    for (case, members, want) in [
        (
            "retitled npm Claude with an MCP child",
            vec![process(20, 10, NODE, Some("claude")), mcp(20)],
            Some((20, P::Claude)),
        ),
        (
            "native Claude release",
            vec![process(
                20,
                10,
                "/Users/u/.local/share/claude/versions/2.0.14",
                Some("claude --model 'opus 4'"),
            )],
            Some((20, P::Claude)),
        ),
        (
            "an Agent another user runs withholds its arguments",
            vec![process(
                20,
                10,
                "/Users/u/.local/share/claude/versions/2.0.14",
                None,
            )],
            Some((20, P::Claude)),
        ),
        (
            "npm Codex wrapper and its native child",
            vec![
                process(20, 10, NODE, Some("node /opt/homebrew/bin/codex")),
                process(
                    30,
                    20,
                    "/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/codex/codex",
                    Some(
                        "/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/codex/codex",
                    ),
                ),
            ],
            Some((20, P::Codex)),
        ),
        (
            "sh wrapper re-executed as bash",
            vec![process(
                20,
                10,
                "/bin/bash",
                Some("/bin/sh /usr/local/bin/opencode '/Users/u/my project'"),
            )],
            Some((20, P::OpenCode)),
        ),
        (
            "an ordinary program",
            vec![process(20, 10, "/usr/bin/vim", Some("vim notes.md"))],
            None,
        ),
        // A line the Host's quoting cannot produce is unreadable, never guessed at.
        (
            "an unreadable runtime",
            vec![process(20, 10, NODE, Some("node \"$HOME/cli.js\""))],
            None,
        ),
    ] {
        assert_eq!(identified(members), want, "{case}");
    }
}
