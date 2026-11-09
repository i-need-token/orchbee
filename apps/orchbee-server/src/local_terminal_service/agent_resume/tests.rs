//! The delivery rule's decisions: a gated resume is tried only in an idle shell the Host launched
//! with prompt marks, at its prompt in the retained directory, and never in a busy, foreign or
//! too-old terminal; and the handover of a retried reference to the driver at any point of its run.
//! Timing, ordering, the Host's gate and wiring are witnessed end to end by
//! `cli_windows_agent_resume`.
use super::{
    Admitted, Candidate, Reason,
    admission::{Readiness, ResumeFacts, readiness, same_directory},
    plan::ResumeShell,
    signals::{Offer, Signals},
};
use crate::local_terminal_service::tab_close::evidence::ShellReport;
use orchbee_terminal_protocol::{
    AttachmentId, MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalId, TerminalLaunchKind, TerminalProcess,
    TerminalProcessConsole, TerminalProcessInfo,
};
use orchbee_workbench_protocol::{TerminalPresentationId, WorkbenchDirectoryPath};

const RETAINED: &str = "C:\\work\\project";

fn row(pid: u32, image: Option<&str>, command: &str) -> TerminalProcess {
    TerminalProcess {
        pid,
        parent_pid: (pid != 1).then_some(1),
        creation_time: 1,
        console: TerminalProcessConsole::Attached,
        image: image.map(Into::into),
        command_line: Some(command.into()),
        current_directory: None,
    }
}

/// A sample of a terminal the Host launched as `root` with `launch_kind`.
fn launched(
    launch_kind: Option<TerminalLaunchKind>,
    root: TerminalProcess,
    others: Vec<TerminalProcess>,
) -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 1,
        processes: std::iter::once(root).chain(others).collect(),
        launch_kind,
    }
}

/// A sample of a marked cmd or managed PowerShell, as its root's image says.
fn sample(root: TerminalProcess, others: Vec<TerminalProcess>) -> TerminalProcessInfo {
    let kind = if root.image.as_deref() == Some(PWSH) {
        TerminalLaunchKind::ManagedPowershell
    } else {
        TerminalLaunchKind::Cmd
    };
    launched(Some(kind), root, others)
}

/// The Windows rule for one quoted command-line argument.
fn quoted(value: &str) -> String {
    let mut quoted = String::from("\"");
    let mut slashes = 0;
    for character in value.chars() {
        if character == '\\' {
            slashes += 1;
        } else {
            let escapes = if character == '"' {
                slashes * 2 + 1
            } else {
                slashes
            };
            quoted.extend(std::iter::repeat_n('\\', escapes));
            quoted.push(character);
            slashes = 0;
        }
    }
    quoted.extend(std::iter::repeat_n('\\', slashes * 2));
    quoted.push('"');
    quoted
}

const PWSH: &str = "C:\\Program Files\\PowerShell\\7\\pwsh.exe";
const CMD: &str = "C:\\Windows\\System32\\cmd.exe";

fn managed_powershell() -> TerminalProcess {
    let initialization = orchbee_local_runtime::windows_interactive_powershell_initialization();
    row(
        1,
        Some(PWSH),
        &format!(
            "{} -NoLogo -NoProfile -NoExit -Command {}",
            quoted(PWSH),
            quoted(&initialization)
        ),
    )
}

fn cmd() -> TerminalProcess {
    row(1, Some(CMD), &format!("{} /D /Q", quoted(CMD)))
}

fn conhost(pid: u32) -> TerminalProcess {
    row(
        pid,
        Some("C:\\Windows\\System32\\conhost.exe"),
        "conhost.exe 0x4",
    )
}

fn facts(
    report: Option<ShellReport>,
    sample: Option<TerminalProcessInfo>,
    directory: Option<&str>,
) -> ResumeFacts {
    ResumeFacts {
        report,
        directory: directory.map(|path| WorkbenchDirectoryPath::parse(path).unwrap()),
        sample,
    }
}

fn decide(facts: &ResumeFacts) -> Readiness {
    readiness(facts, &WorkbenchDirectoryPath::parse(RETAINED).unwrap())
}

#[test]
fn a_shell_at_its_prompt_in_the_retained_directory_receives_the_resume_for_its_kind() {
    for (root, shell) in [
        (managed_powershell(), ResumeShell::ManagedPowerShell),
        (cmd(), ResumeShell::Cmd),
    ] {
        let idle = sample(root, vec![conhost(2)]);
        assert_eq!(
            decide(&facts(
                Some(ShellReport::AtPrompt),
                Some(idle),
                Some(RETAINED)
            )),
            Readiness::Try(shell)
        );
    }
}

#[test]
fn a_busy_foreign_or_too_old_terminal_never_receives_the_resume() {
    let idle = || sample(managed_powershell(), vec![conhost(2)]);
    let at_prompt = Some(ShellReport::AtPrompt);
    // A running command skips even when its sample shows nothing else, as a cmdlet does.
    assert_eq!(
        decide(&facts(
            Some(ShellReport::Running),
            Some(idle()),
            Some(RETAINED)
        )),
        Readiness::Skip(Reason::NotIdle)
    );
    // Any program beside the shell and its console host, attached or not, whatever the report.
    for report in [at_prompt, Some(ShellReport::Silent), None] {
        let mut helper = row(3, Some("C:\\nodejs\\node.exe"), "node.exe agent.js");
        helper.console = TerminalProcessConsole::Detached;
        let busy = sample(managed_powershell(), vec![conhost(2), helper]);
        assert_eq!(
            decide(&facts(report, Some(busy), Some(RETAINED))),
            Readiness::Skip(Reason::NotIdle)
        );
    }
    // A program refutes even a sample that reached its row limit.
    let mut crowded = vec![row(3, Some("C:\\tools\\task.exe"), "task.exe")];
    crowded.extend((4..).take(MAXIMUM_PROCESS_SAMPLE_ROWS).map(conhost));
    assert_eq!(
        decide(&facts(
            at_prompt,
            Some(sample(managed_powershell(), crowded)),
            Some(RETAINED)
        )),
        Readiness::Skip(Reason::NotIdle)
    );
    // Only a shell the Host launched with prompt marks can have its prompt confirmed: its own
    // launch record decides, whatever the root's command line says.
    assert_eq!(
        decide(&facts(
            at_prompt,
            Some(launched(Some(TerminalLaunchKind::Other), cmd(), vec![])),
            Some(RETAINED)
        )),
        Readiness::Skip(Reason::UnsupportedShell)
    );
    // A Host that records no launch kind predates the prompt gate.
    assert_eq!(
        decide(&facts(
            at_prompt,
            Some(launched(None, managed_powershell(), vec![])),
            Some(RETAINED)
        )),
        Readiness::Skip(Reason::TerminalTooOld)
    );
}

#[test]
fn a_resume_never_starts_outside_the_retained_directory() {
    let idle = || Some(sample(cmd(), vec![]));
    assert_eq!(
        decide(&facts(
            Some(ShellReport::AtPrompt),
            idle(),
            Some("C:\\work\\other")
        )),
        Readiness::Skip(Reason::DirectoryMismatch)
    );
    // Without directory evidence the read cannot decide.
    assert_eq!(
        decide(&facts(Some(ShellReport::AtPrompt), idle(), None)),
        Readiness::Wait
    );
}

#[test]
fn evidence_that_cannot_decide_waits() {
    let at_prompt = Some(ShellReport::AtPrompt);
    let full: Vec<_> = (2..)
        .take(MAXIMUM_PROCESS_SAMPLE_ROWS)
        .map(conhost)
        .collect();
    for unknown in [
        // No sample at all.
        None,
        // A truncated sample may hide a program.
        Some(sample(cmd(), full)),
        // A row whose image cannot be read may be any program.
        Some(sample(cmd(), vec![row(2, None, "")])),
    ] {
        assert_eq!(
            decide(&facts(at_prompt, unknown, Some(RETAINED))),
            Readiness::Wait
        );
    }
    // No frame, or a silent shell, which may be waiting for input: nothing says the shell is at
    // its prompt, whatever the shell.
    for root in [cmd(), managed_powershell()] {
        for report in [None, Some(ShellReport::Silent)] {
            assert_eq!(
                decide(&facts(
                    report,
                    Some(sample(root.clone(), vec![])),
                    Some(RETAINED)
                )),
                Readiness::Wait
            );
        }
    }
}

#[test]
fn directories_compare_case_insensitively_with_one_trailing_separator() {
    assert!(same_directory("C:\\Work\\Project", "c:\\work\\project\\"));
    assert!(same_directory("C:\\work\\project/", "C:\\work\\project"));
    assert!(same_directory("C:\\", "c:\\"));
    // A drive root keeps its separator: `C:` alone is the drive's current directory.
    assert!(!same_directory("C:\\", "C:"));
    assert!(!same_directory(
        "C:\\work\\project\\\\",
        "C:\\work\\project"
    ));
    assert!(!same_directory("C:\\work\\project", "C:\\work\\projects"));
    assert!(same_directory("\\\\server\\share\\", "\\\\SERVER\\share"));
}

fn retried(seed: u8) -> Admitted {
    Admitted {
        candidate: Candidate {
            presentation: TerminalPresentationId::parse(&format!("{seed:032x}")).unwrap(),
            directory: WorkbenchDirectoryPath::parse(RETAINED).unwrap(),
        },
        committed: None,
        retried: Some(std::time::Instant::now()),
    }
}

fn presentation(admitted: Option<Admitted>) -> Option<TerminalPresentationId> {
    admitted.map(|admitted| admitted.candidate.presentation)
}

#[test]
fn a_retried_reference_is_attempted_whether_or_not_a_driver_already_ran() {
    let signals = Signals::default();
    // The startup driver runs: a Retry joins it, and it takes the reference before it ends.
    assert!(signals.activate());
    assert!(matches!(signals.offer(retried(1)), Offer::Queued));
    assert_eq!(
        presentation(signals.next_retried()),
        Some(retried(1).candidate.presentation)
    );
    assert!(signals.next_retried().is_none());
    // That driver ended: the next Retry starts a run of its own instead of waiting for a restart.
    let Offer::Start(started) = signals.offer(retried(2)) else {
        panic!("a Retry after the driver ended must start a run");
    };
    assert_eq!(
        started.candidate.presentation,
        retried(2).candidate.presentation
    );
    assert!(matches!(signals.offer(retried(3)), Offer::Queued));
    assert_eq!(
        presentation(signals.next_retried()),
        Some(retried(3).candidate.presentation)
    );
    assert!(signals.next_retried().is_none());
}

#[test]
fn a_stopping_server_attempts_no_retried_reference_and_keeps_it_pending() {
    let signals = Signals::default();
    assert!(signals.activate());
    assert!(matches!(signals.offer(retried(1)), Offer::Queued));
    signals.stop();
    // The driver returns early: the waiting reference is left to the final checkpoint.
    assert_eq!(signals.finish(), 1);
    assert!(matches!(signals.offer(retried(2)), Offer::Refused));
}
