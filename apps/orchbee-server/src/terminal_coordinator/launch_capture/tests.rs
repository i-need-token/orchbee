use super::LaunchCapture;
use crate::terminal_coordinator::{TerminalCoordinator, WorkerPlacement};
use orchbee_capability_node_protocol::{
    CapabilityNodeMessage, DefaultCustomProgram, DefaultLaunchProgram, LaunchPreferences,
    LeaseExpiresAtUnixMilliseconds, TerminalLaunchPreferences, TerminalLaunchThemes,
    WorkerGeneration, WorkerId,
};
use orchbee_client_server_protocol::{
    LaunchSelection, RequestId, StartMode, TerminalCreateRequest,
};
use orchbee_workbench_protocol::PresentationLaunchProgram;
use std::num::NonZeroU64;

fn available(program: Option<DefaultLaunchProgram>) -> LaunchPreferences {
    let themes = TerminalLaunchThemes::new("l".into(), "d".into()).unwrap();
    LaunchPreferences::Available(
        TerminalLaunchPreferences::new(1_000, None, themes, program).unwrap(),
    )
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

#[test]
fn an_env_launch_names_the_program_it_starts() {
    let named = |executable: &str, arguments: &[&str]| {
        super::explicit_program(
            executable,
            &arguments.iter().map(|&a| a.to_owned()).collect::<Vec<_>>(),
        )
        .map(|program| program.as_str().to_owned())
    };
    let env = "/usr/bin/env";
    assert_eq!(
        named(env, &["ORCHBEE_A=1", "/opt/bin/tool", "-x"]),
        Some("tool".into())
    );
    // A custom command's login shell names the program its command line starts.
    let shell = |command| named(env, &["ORCHBEE_A=/w x", "/bin/zsh", "-l", "-c", command]);
    assert_eq!(shell("npm test"), Some("npm".into()));
    assert_eq!(shell("  ./build.sh --all"), Some("build.sh".into()));
    assert_eq!(
        named(env, &["/bin/tcsh", "-c", "make"]),
        Some("make".into())
    );
    // Assignments, expansions, quotes, compound lines and builtins name the shell.
    for command in [
        "FOO=1 make",
        "$EDITOR x",
        "'npm' test",
        "cd src && make",
        "(ls)",
        "exec ls",
    ] {
        assert_eq!(shell(command), Some("zsh".into()), "{command}");
    }
    // An option to env, or no program, names env; only env is unwrapped.
    assert_eq!(named(env, &["-i", "/bin/sh"]), Some("env".into()));
    assert_eq!(named(env, &["A=1"]), Some("env".into()));
    assert_eq!(
        named("/bin/zsh", &["-l", "-c", "npm test"]),
        Some("zsh".into())
    );
}
