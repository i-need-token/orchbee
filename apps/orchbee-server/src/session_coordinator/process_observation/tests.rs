//! A POSIX Host's samples, in the shapes macOS reports, name the Agent a Windows sample of the
//! same launch would. Only POSIX builds decode the POSIX Host's quoted command lines.
use super::script_operands::{ANSWER_WAIT, OperandResolver, file_real_path, resolve_with};
use super::*;
use orchbee_terminal_protocol::{AttachmentId, TerminalId, TerminalProcess};
use std::time::Duration;

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

/// A shared alias's bin link names its provider only through the package entry it resolves to,
/// read against the process's working folder. A link elsewhere, or a relative operand without a
/// working folder, leaves the sample as the Host reported it.
#[test]
fn bin_links_of_shared_aliases_resolve_through_the_working_folder() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    const NODE: &str = "/opt/homebrew/Cellar/node/24.1.0/bin/node";
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let scratch = Scratch(std::env::temp_dir().canonicalize().unwrap().join(format!(
        "orchbee-script-operands-{:032x}",
        u128::from_le_bytes(nonce)
    )));
    let package = scratch.0.join("lib/node_modules/@github/copilot");
    for folder in [&package, &scratch.0.join("bin"), &scratch.0.join("other")] {
        std::fs::create_dir_all(folder).unwrap();
    }
    std::fs::write(package.join("index.js"), "").unwrap();
    std::fs::write(scratch.0.join("other/index.js"), "").unwrap();
    let link = |target: &str, name: &str| {
        std::os::unix::fs::symlink(target, scratch.0.join("bin").join(name)).unwrap();
    };
    link("../lib/node_modules/@github/copilot/index.js", "copilot");
    link("../other/index.js", "cbc");
    let root = scratch.0.to_str().unwrap();
    let line = |argv: &[&str]| {
        host_conventions::line(&argv.iter().map(|&a| a.to_owned()).collect::<Vec<_>>()).unwrap()
    };
    let sample = |argv: &[&str], directory: Option<&str>| TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        launch_kind: None,
        root_pid: 10,
        processes: vec![
            process(10, 1, "/bin/zsh", Some("-zsh")),
            TerminalProcess {
                current_directory: directory.map(Into::into),
                ..process(20, 10, NODE, Some(&line(argv)))
            },
        ],
    };
    let resolver = OperandResolver::spawn(file_real_path).unwrap();
    let resolve_script_operands = |sample: &TerminalProcessInfo| resolve_with(&resolver, sample);
    let unresolved = sample(&["node", "bin/copilot"], Some(root));
    assert_eq!(identify(&unresolved), None, "the name alone proves nothing");
    let resolved = resolve_script_operands(&unresolved).unwrap();
    let entry = package.join("index.js");
    assert_eq!(
        decoded_argv(resolved.processes[1].command_line.as_deref()),
        Some(vec!["node".to_owned(), entry.to_str().unwrap().to_owned()])
    );
    assert_eq!(
        identify(&resolved).map(|selected| (selected.pid(), selected.provider)),
        Some((20, AgentReportProvider::Copilot))
    );
    let absolute = format!("{root}/bin/copilot");
    assert!(resolve_script_operands(&sample(&["node", &absolute], None)).is_some());
    let outside = format!("{root}/bin/cbc");
    for (argv, directory) in [
        (["node", "bin/copilot"], None),
        (["node", "bin/cbc"], Some(root)),
        (["node", outside.as_str()], None),
    ] {
        assert_eq!(
            resolve_script_operands(&sample(&argv, directory)),
            None,
            "{argv:?} {directory:?}"
        );
    }
}

/// A real-path lookup that never returns, as on an unresponsive network mount, leaves the row as
/// the Host reported it within the resolver's wait, and later samples skip it at once.
#[test]
fn an_unanswered_real_path_lookup_leaves_the_row_unchanged_within_the_wait() {
    let (release, held) = std::sync::mpsc::channel::<()>();
    let resolver = OperandResolver::spawn(move |_| {
        let _released = held.recv();
        None
    })
    .unwrap();
    let sample = TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        launch_kind: None,
        root_pid: 10,
        processes: vec![
            process(10, 1, "/bin/zsh", Some("-zsh")),
            process(
                20,
                10,
                "/opt/homebrew/Cellar/node/24.1.0/bin/node",
                Some("node /Volumes/share/bin/copilot"),
            ),
        ],
    };
    let started = Instant::now();
    assert_eq!(resolve_with(&resolver, &sample), None);
    assert!(started.elapsed() < Duration::from_secs(1));
    let started = Instant::now();
    assert_eq!(resolve_with(&resolver, &sample), None);
    assert!(
        started.elapsed() < ANSWER_WAIT,
        "an unanswered lookup is skipped, not awaited again"
    );
    drop(release);
}
