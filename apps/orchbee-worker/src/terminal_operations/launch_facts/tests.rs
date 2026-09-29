use super::*;
use crate::{
    terminal_authorization::{AuthorizedHostOperationKernel, HostOperationAdmission},
    terminal_host_launch_adapter::map_launch_dispatch,
};
use orchbee_capability_node_protocol::*;
use orchbee_local_site_runtime::WindowsShellSettings;
use std::num::NonZeroU64;

#[test]
fn configured_and_explicit_shells_reach_real_host_launch_preparation() -> Result<(), Box<dyn Error>>
{
    let powershell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let cwd = AbsoluteCwd::new(std::env::temp_dir().to_str().unwrap())?;
    let configured = WindowsShellSettings::new(powershell.to_str().unwrap(), "login")?;
    for explicit in [false, true] {
        let selection = explicit.then(|| {
            LaunchSelection::new(
                "C:/explicit/program.exe".into(),
                vec!["literal & value".into()],
            )
            .unwrap()
        });
        let snapshot = if explicit {
            WindowsShellSettings::new("definitely-missing-shell.exe", "login")?
        } else {
            configured.clone()
        };
        let environment =
            LaunchEnvironment::new(vec![("USER_MARKER".into(), "literal $ value".into())])?;
        let facts = resolve(Some(&cwd), selection.as_ref(), &environment, &snapshot)?;
        let n = NonZeroU64::MIN;
        let authorization = TerminalOperationAuthorization::new(
            TerminalId::from_bytes([1; 16])?,
            OperationId::from_bytes([2; 16])?,
            WorkerId::from_bytes([3; 16])?,
            WorkerGeneration::new(n),
            TerminalFence::new(n),
            TerminalOperationLease::new(
                LeaseId::from_bytes([4; 16])?,
                LeaseExpiresAtUnixMilliseconds::new(NonZeroU64::new(200).unwrap()),
            ),
            TERMINAL_LAUNCH_CAPABILITY_VERSION,
        );
        let mut kernel = AuthorizedHostOperationKernel::new(
            orchbee_local_site_protocol::WorkerId::from_bytes([3; 16]),
            n,
        )?;
        let request = AuthorizedTerminalLaunch::new(
            authorization,
            Some(cwd.clone()),
            TerminalLaunchMode::Deferred,
            selection,
        )
        .with_environment(environment);
        let HostOperationAdmission::Dispatch(dispatch) = kernel.admit_launch(request, 100)? else {
            panic!("dispatch")
        };
        let host = map_launch_dispatch(&dispatch, facts, None)?;
        assert_eq!(host.launch.current_directory.as_deref(), Some(cwd.as_str()));
        assert!(
            host.launch
                .environment
                .iter()
                .any(|entry| entry.name == "USER_MARKER"
                    && entry.value.as_deref() == Some("literal $ value"))
        );
        if explicit {
            assert_eq!(host.launch.executable, "C:/explicit/program.exe");
            assert_eq!(host.launch.arguments, ["literal & value"]);
        } else {
            assert_eq!(std::path::Path::new(&host.launch.executable), powershell);
            assert!(
                host.launch
                    .arguments
                    .iter()
                    .any(|argument| argument == "-Command")
            );
            assert!(
                !host
                    .launch
                    .arguments
                    .iter()
                    .any(|argument| argument == "-l")
            );
        }
    }
    Ok(())
}
