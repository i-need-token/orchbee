use super::LaunchCapture;
use crate::terminal_coordinator::{TerminalCoordinator, WorkerPlacement};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences,
    LeaseExpiresAtUnixMilliseconds, TerminalLaunchPreferences, WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    LaunchSelection, RequestId, StartMode, TerminalCreateRequest,
};
use orchbee_workbench_protocol::PresentationLaunchProgram;
use std::num::NonZeroU64;

fn available(program: Option<DefaultLaunchProgram>) -> LaunchPreferences {
    LaunchPreferences::Available(TerminalLaunchPreferences::new(1_000, None, program).unwrap())
}

/// The preferences the planned launch dispatches with and the program its binding publishes.
fn planned(
    capture: &LaunchCapture,
    selection: Option<LaunchSelection>,
) -> (LaunchPreferences, Option<String>) {
    let mut coordinator = TerminalCoordinator::new();
    let request = TerminalCreateRequest::new(
        RequestId::parse(&format!("{:032x}", 1)).unwrap(),
        None,
        None,
        StartMode::Deferred,
        selection,
    );
    let worker = WorkerPlacement {
        worker_id: WorkerId::from_bytes([9; 16]).unwrap(),
        generation: WorkerGeneration::new(NonZeroU64::MIN),
    };
    let expiry = LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(10_000).unwrap());
    let plan = coordinator
        .plan_create(request, Some(worker), Some(expiry), capture)
        .unwrap();
    let CapabilityNodeMessage::AuthorizedTerminalLaunch(launch) = plan.command() else {
        panic!("a planned creation dispatches a launch")
    };
    (
        launch.preferences().clone(),
        plan.launch_program()
            .map(|program| program.as_str().to_owned()),
    )
}

#[test]
fn the_published_program_is_the_one_the_dispatched_launch_starts() {
    let shell = PresentationLaunchProgram::parse("pwsh.exe").ok();
    let custom = DefaultLaunchProgram::Custom(
        DefaultCustomProgram::new("C:\\tools\\nu.exe".into(), vec!["-l".into()]).unwrap(),
    );
    let system = DefaultLaunchProgram::SystemShell { login: false };
    let explicit = || LaunchSelection::new("/usr/bin/fish".into(), Vec::new()).unwrap();

    // An explicit selection names its own executable and never carries the default program.
    let capture = LaunchCapture::new(available(None), available(Some(custom.clone())))
        .with_system_shell(shell.clone());
    assert_eq!(
        planned(&capture, Some(explicit())),
        (available(None), Some("fish".into()))
    );
    // A default launch names the program the captured default profile dispatches.
    assert_eq!(
        planned(&capture, None),
        (available(Some(custom)), Some("nu.exe".into()))
    );
    let capture = LaunchCapture::new(available(None), available(Some(system.clone())))
        .with_system_shell(shell.clone());
    assert_eq!(
        planned(&capture, None),
        (available(Some(system.clone())), Some("pwsh.exe".into()))
    );
    // A generation that reported no System shell leaves the Client's own fallback in charge.
    let capture = LaunchCapture::new(available(None), available(Some(system.clone())));
    assert_eq!(planned(&capture, None), (available(Some(system)), None));
    // Device defaults start the System shell.
    let capture = LaunchCapture::absent().with_system_shell(shell.clone());
    assert_eq!(
        planned(&capture, None),
        (LaunchPreferences::Absent, Some("pwsh.exe".into()))
    );
    // The Worker refuses an unavailable capture; only an explicit selection still names the program
    // it requested.
    let capture = LaunchCapture::unavailable().with_system_shell(shell);
    assert_eq!(
        planned(&capture, None),
        (LaunchPreferences::Unavailable, None)
    );
    assert_eq!(
        planned(&capture, Some(explicit())),
        (LaunchPreferences::Unavailable, Some("fish".into()))
    );
}
