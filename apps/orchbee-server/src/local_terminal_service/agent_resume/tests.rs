//! The delivery rule's decisions: a resume is typed only into an idle interactive shell at its
//! prompt in the retained directory, and never into a busy or foreign terminal; and the handover of
//! a retried reference to the driver at any point of its run. Timing, ordering and wiring are
//! witnessed end to end by `cli_windows_agent_resume`.
use super::{
    Admitted, Candidate, Reason,
    admission::{Readiness, ResumeFacts, readiness, same_directory},
    plan::ResumeShell,
    signals::{Offer, Signals},
};
use crate::local_terminal_service::tab_close::evidence::ShellReport;
use orchbee_terminal_protocol::{
    AttachmentId, MAXIMUM_PROCESS_SAMPLE_ROWS, TerminalId, TerminalProcess, TerminalProcessConsole,
    TerminalProcessInfo,
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

fn sample(root: TerminalProcess, others: Vec<TerminalProcess>) -> TerminalProcessInfo {
    TerminalProcessInfo {
        attachment_id: AttachmentId::from_bytes([1; 16]).unwrap(),
        terminal_id: TerminalId::from_bytes([2; 16]).unwrap(),
        root_pid: 1,
        processes: std::iter::once(root).chain(others).collect(),
    }
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

fn powershell() -> TerminalProcess {
    row(1, Some(PWSH), &format!("{} -NoLogo -NoExit", quoted(PWSH)))
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

fn decide(facts: &ResumeFacts, previous: Option<Readiness>) -> Readiness {
    readiness(
        facts,
        previous,
        &WorkbenchDirectoryPath::parse(RETAINED).unwrap(),
    )
}

#[test]
fn a_shell_at_its_prompt_in_the_retained_directory_receives_the_resume_for_its_kind() {
    for (root, shell) in [
        (managed_powershell(), ResumeShell::ManagedPowerShell),
        (powershell(), ResumeShell::PowerShell),
        (cmd(), ResumeShell::Cmd),
    ] {
        let idle = sample(root, vec![conhost(2)]);
        assert_eq!(
            decide(
                &facts(Some(ShellReport::AtPrompt), Some(idle), Some(RETAINED)),
                None
            ),
            Readiness::Deliver(shell)
        );
    }
}

#[test]
fn a_busy_or_foreign_terminal_never_receives_the_resume() {
    let idle = || sample(managed_powershell(), vec![conhost(2)]);
    let at_prompt = Some(ShellReport::AtPrompt);
    // A running command skips even when its sample shows nothing else, as a cmdlet does.
    assert_eq!(
        decide(
            &facts(Some(ShellReport::Running), Some(idle()), Some(RETAINED)),
            None
        ),
        Readiness::Skip(Reason::NotIdle)
    );
    // Any program beside the shell and its console host, attached or not, whatever the report.
    for report in [at_prompt, Some(ShellReport::Silent), None] {
        let mut helper = row(3, Some("C:\\nodejs\\node.exe"), "node.exe agent.js");
        helper.console = TerminalProcessConsole::Detached;
        let busy = sample(managed_powershell(), vec![conhost(2), helper]);
        assert_eq!(
            decide(&facts(report, Some(busy), Some(RETAINED)), None),
            Readiness::Skip(Reason::NotIdle)
        );
    }
    // A program refutes even a sample that reached its row limit.
    let mut crowded = vec![row(3, Some("C:\\tools\\task.exe"), "task.exe")];
    crowded.extend((4..).take(MAXIMUM_PROCESS_SAMPLE_ROWS).map(conhost));
    assert_eq!(
        decide(
            &facts(
                at_prompt,
                Some(sample(powershell(), crowded)),
                Some(RETAINED)
            ),
            None
        ),
        Readiness::Skip(Reason::NotIdle)
    );
    // Only an interactive cmd, powershell or pwsh root is a shell this resume can be typed into.
    for root in [
        row(
            1,
            Some("C:\\Program Files\\Git\\bin\\bash.exe"),
            "bash.exe -i",
        ),
        row(
            1,
            Some(PWSH),
            &format!("{} -NoExit -Command task", quoted(PWSH)),
        ),
        row(1, Some(CMD), &format!("{} /C task", quoted(CMD))),
        row(1, Some("C:\\nodejs\\node.exe"), "node.exe"),
    ] {
        assert_eq!(
            decide(
                &facts(at_prompt, Some(sample(root, vec![])), Some(RETAINED)),
                None
            ),
            Readiness::Skip(Reason::UnsupportedShell)
        );
    }
}

#[test]
fn a_resume_never_starts_outside_the_retained_directory() {
    let idle = || Some(sample(cmd(), vec![]));
    assert_eq!(
        decide(
            &facts(Some(ShellReport::AtPrompt), idle(), Some("C:\\work\\other")),
            None
        ),
        Readiness::Skip(Reason::DirectoryMismatch)
    );
    // Without directory evidence the read cannot decide.
    assert_eq!(
        decide(&facts(Some(ShellReport::AtPrompt), idle(), None), None),
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
    let mut unreadable_root = cmd();
    unreadable_root.image = None;
    for unknown in [
        // No sample at all.
        None,
        // A truncated sample may hide a program.
        Some(sample(cmd(), full)),
        // A row whose image cannot be read may be any program.
        Some(sample(cmd(), vec![row(2, None, "")])),
        // A root that cannot be read may not be a shell.
        Some(sample(unreadable_root, vec![])),
    ] {
        assert_eq!(
            decide(&facts(at_prompt, unknown, Some(RETAINED)), None),
            Readiness::Wait
        );
    }
    // No frame: nothing says the shell is at its prompt.
    assert_eq!(
        decide(
            &facts(None, Some(sample(cmd(), vec![])), Some(RETAINED)),
            None
        ),
        Readiness::Wait
    );
}

#[test]
fn silence_admits_only_an_unmarked_shell_and_only_on_a_second_consecutive_read() {
    let silent = |root: TerminalProcess| {
        facts(
            Some(ShellReport::Silent),
            Some(sample(root, vec![conhost(2)])),
            Some(RETAINED),
        )
    };
    for (root, shell) in [
        (cmd as fn() -> TerminalProcess, ResumeShell::Cmd),
        (powershell, ResumeShell::PowerShell),
    ] {
        let first = decide(&silent(root()), None);
        assert_eq!(first, Readiness::Quiet(shell));
        assert_eq!(
            decide(&silent(root()), Some(first)),
            Readiness::Deliver(shell)
        );
        // Anything between two silent reads starts the count again.
        assert_eq!(
            decide(&silent(root()), Some(Readiness::Wait)),
            Readiness::Quiet(shell)
        );
    }
    // Managed PowerShell marks its prompt, so its silence is never a prompt.
    let managed = Readiness::Quiet(ResumeShell::ManagedPowerShell);
    assert_eq!(
        decide(&silent(managed_powershell()), Some(managed)),
        Readiness::Wait
    );
    // A silent read of another shell does not count toward this one.
    assert_eq!(
        decide(
            &silent(cmd()),
            Some(Readiness::Quiet(ResumeShell::PowerShell))
        ),
        Readiness::Quiet(ResumeShell::Cmd)
    );
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
