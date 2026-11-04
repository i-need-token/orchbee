//! The rules a conditional close weighs, in the shapes real Hosts produce. `cli_windows_idle_close`
//! witnesses them end to end on a real PowerShell under both console hosts.
use super::*;
use orchbee_terminal_protocol::{
    AttachmentId, HostEpoch, TerminalCommandObservation, TerminalFrameRoute, TerminalInteraction,
    TerminalObservedCommand, TerminalPixelGeometry, TerminalTextCursor,
};
use std::num::NonZeroU64;

fn frame(cursor: u16, rows: impl Fn(u16) -> (u8, u8)) -> TerminalTextFrame {
    TerminalTextFrame {
        route: TerminalFrameRoute {
            attachment_id: AttachmentId::parse(&format!("{:032x}", 1)).unwrap(),
            host_epoch: HostEpoch::parse(&format!("{:032x}", 2)).unwrap(),
            sequence: NonZeroU64::MIN,
            generation: NonZeroU64::MIN,
            revision: 1,
            base_revision: None,
        },
        geometry: TerminalPixelGeometry {
            cell_pixel_height: 16,
            cell_pixel_width: 8,
            columns: 80,
            rows: 24,
        },
        cursor: TerminalTextCursor {
            column: 0,
            row: cursor,
            visible: true,
        },
        alternate_screen: false,
        interaction: TerminalInteraction::default(),
        foreground: 0,
        background: 0,
        theme_revision: 1,
        scrollback_rows: 0,
        progress: None,
        graphics_present: None,
        command_completion: None,
        command_observation: None,
        title: None,
        current_directory: None,
        rows: (0..24)
            .map(|index| {
                let (prompt_class, flags) = rows(index);
                TerminalTextRow {
                    prompt_class,
                    index,
                    flags,
                    cells: vec![],
                }
            })
            .collect(),
    }
}

/// The reader's own markers, as managed PowerShell produces them at its prompt (`ready`) and between
/// its run markers (`running`).
fn reported(ready: bool, running: bool) -> TerminalTextFrame {
    let one = NonZeroU64::MIN;
    let mut next = frame(0, |_| (0, 0));
    next.command_observation = Some(TerminalCommandObservation {
        output_epoch: one,
        ready,
        command: running.then_some(TerminalObservedCommand {
            id: [1; 16],
            completed: false,
            exit_code: 0,
            start_row: 0,
            start_column: 0,
            end_row: 0,
            end_column: 0,
            generation: one,
            output_epoch: one,
            started_at: None,
        }),
    });
    next
}

#[test]
fn the_shell_reports_where_it_is_by_its_markers_or_the_marks_on_its_rows() {
    assert_eq!(
        ShellReport::of(&reported(true, false)),
        ShellReport::AtPrompt
    );
    // A cmdlet such as `Start-Sleep` starts no process, so only this report shows it running.
    assert_eq!(
        ShellReport::of(&reported(false, true)),
        ShellReport::Running
    );

    // Without the reader's markers, as under the system ConHost, only OSC 133 reaches the Host: it
    // marks the row the prompt ends on.
    let screen = |cursor: u16, marked: &[u16], continuation: &[u16]| {
        ShellReport::of(&frame(cursor, |index| {
            (
                u8::from(marked.contains(&index)),
                if continuation.contains(&index) { 2 } else { 0 },
            )
        }))
    };
    // No mark at all: `cmd`, or a screen a clear erased while PSReadLine redrew its prompt.
    assert_eq!(screen(3, &[], &[]), ShellReport::Silent);
    assert_eq!(screen(1, &[1], &[]), ShellReport::AtPrompt);
    // Input longer than the row stays at the prompt, on rows that carry no mark.
    assert_eq!(screen(3, &[1], &[2, 3]), ShellReport::AtPrompt);
    // A cmdlet or a program below the prompt it left.
    assert_eq!(screen(3, &[1], &[]), ShellReport::Running);
    assert_eq!(screen(3, &[1], &[2]), ShellReport::Running);

    // The terminal also leaves the continuation class on rows no prompt holds, the cursor's own
    // among them.
    let stained = |cursor: u16, marked: &[u16], continuation: &[u16]| {
        ShellReport::of(&frame(cursor, |index| {
            (
                if marked.contains(&index) { 1 } else { 2 },
                if continuation.contains(&index) { 2 } else { 0 },
            )
        }))
    };
    // The cursor on a stained blank row below the prompt a cmdlet left.
    assert_eq!(stained(5, &[3], &[]), ShellReport::Running);
    // Wrapped input keeps its stained rows at the prompt.
    assert_eq!(stained(5, &[3], &[4, 5]), ShellReport::AtPrompt);
    // A clear erased the mark and left the stain.
    assert_eq!(stained(5, &[], &[]), ShellReport::Silent);

    // A full-screen view says nothing about the shell beneath it, nor do a delta frame's rows.
    let mut covered = frame(3, |index| (u8::from(index == 1), 0));
    covered.alternate_screen = true;
    assert_eq!(ShellReport::of(&covered), ShellReport::Silent);
    let mut delta = frame(3, |index| (u8::from(index == 1), 0));
    delta.route.base_revision = Some(0);
    assert_eq!(ShellReport::of(&delta), ShellReport::Silent);
}

#[test]
fn two_facts_decide_and_a_missing_fact_only_ever_asks() {
    use CloseSample::{Program, ShellOnly, Unknown};
    use ShellReport::{AtPrompt, Running, Silent};
    for (report, sample, closes) in [
        // A silent shell is judged by its sample alone.
        (Some(Silent), Some(ShellOnly), true),
        (Some(Silent), Some(Program), false),
        (Some(Silent), Some(Unknown), false),
        (Some(Silent), None, false),
        (None, Some(ShellOnly), true),
        (None, None, false),
        // A shell at its prompt closes when its sample is refused or late, never past a program.
        (Some(AtPrompt), Some(ShellOnly), true),
        (Some(AtPrompt), Some(Unknown), true),
        (Some(AtPrompt), None, true),
        (Some(AtPrompt), Some(Program), false),
        // A command the shell reports running asks whatever the sample says.
        (Some(Running), Some(ShellOnly), false),
        (Some(Running), None, false),
    ] {
        assert_eq!(
            CloseFacts { report, sample }.runs_nothing(),
            closes,
            "{report:?} {sample:?}"
        );
    }
}

/// The close rule follows the Host's: only a member holding the Terminal console is a program,
/// unlike the strict Agent start rule that refuses a detached helper.
#[cfg(windows)]
#[test]
fn a_sample_counts_only_what_holds_the_console() {
    use orchbee_terminal_protocol::{
        TerminalProcess, TerminalProcessConsole as Console, TerminalProcessInfo,
    };
    let row = |pid, image: Option<&str>, command: &str, console| TerminalProcess {
        pid,
        parent_pid: None,
        creation_time: 1,
        console,
        image: image.map(Into::into),
        command_line: Some(command.into()),
        current_directory: None,
    };
    let shell = row(
        1,
        Some("C:\\Program Files\\PowerShell\\7\\pwsh.exe"),
        "\"C:\\Program Files\\PowerShell\\7\\pwsh.exe\" -NoExit",
        Console::Attached,
    );
    let sample = |rows: Vec<TerminalProcess>| TerminalProcessInfo {
        attachment_id: orchbee_terminal_protocol::AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: orchbee_terminal_protocol::TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 1,
        processes: std::iter::once(shell.clone()).chain(rows).collect(),
    };
    let with = |image: Option<&str>, console| sample(vec![row(2, image, "helper", console)]);

    assert_eq!(CloseSample::of(&sample(vec![])), CloseSample::ShellOnly);
    for (image, console, expected) in [
        (
            Some("conhost.exe"),
            Console::Attached,
            CloseSample::ShellOnly,
        ),
        (
            Some("OpenConsole.exe"),
            Console::Unknown,
            CloseSample::ShellOnly,
        ),
        (Some("PING.EXE"), Console::Attached, CloseSample::Program),
        (Some("agent.exe"), Console::Detached, CloseSample::ShellOnly),
        (Some("agent.exe"), Console::None, CloseSample::ShellOnly),
        (Some("agent.exe"), Console::Unknown, CloseSample::Unknown),
        (None, Console::Unknown, CloseSample::Unknown),
    ] {
        assert_eq!(
            CloseSample::of(&with(image, console)),
            expected,
            "{image:?}"
        );
    }

    let mut root = sample(vec![]);
    root.processes[0].image = None;
    assert_eq!(CloseSample::of(&root), CloseSample::Unknown);
    let mut root = sample(vec![]);
    root.processes[0].image = Some("claude.exe".into());
    assert_eq!(CloseSample::of(&root), CloseSample::Program);
    let mut root = sample(vec![]);
    root.processes[0].command_line = Some("pwsh.exe -File build.ps1".into());
    root.processes[0].image = Some("pwsh.exe".into());
    assert_eq!(CloseSample::of(&root), CloseSample::Program);
    let mut root = sample(vec![]);
    root.root_pid = 9;
    assert_eq!(CloseSample::of(&root), CloseSample::Unknown);
}
