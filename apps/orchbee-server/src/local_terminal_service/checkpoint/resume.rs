//! Restart planning is complete before the unpublished Site submits any terminal.
use orchbee_agent_observation_protocol::{
    AgentProviderSessionReference, AgentProviderSessionReferenceKind, windows_agent_resume_argv,
};
use orchbee_client_server_protocol::LaunchSelection;
use orchbee_local_site_runtime::windows_encoded_command;
use orchbee_server::{WorkbenchAuthority, WorkbenchRestoreTarget};
use orchbee_workbench_protocol::{PanelAgentSession, WorkerId};
use std::{collections::BTreeSet, ffi::OsString, io, path::Path};

type Plan = (WorkbenchRestoreTarget, Option<LaunchSelection>);

pub(crate) fn resume_policy(value: Option<OsString>) -> io::Result<bool> {
    match value.as_deref().and_then(std::ffi::OsStr::to_str) {
        None if value.is_none() => Ok(true),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ORCHBEE_RESUME_AGENTS_ON_RESTORE must be true or false",
        )),
    }
}

pub(super) fn prepare(
    authority: &mut WorkbenchAuthority,
    worker: &WorkerId,
    enabled: bool,
) -> io::Result<Vec<Plan>> {
    let mut targets = authority
        .restoration_targets(worker)
        .map_err(io::Error::other)?;
    let mut used = BTreeSet::new();
    let mut shell = None;
    let mut plans = Vec::with_capacity(targets.len());
    for target in &mut targets {
        let launch = if enabled {
            if let Some(session) = target.agent_session()
                && let Some(reference) = reference(session)
                && let Some(mut argv) =
                    windows_agent_resume_argv(&session.source, &session.agent, &reference)
            {
                if used.insert(key(session)) {
                    // Current Windows installations use these names; the pinned reference's
                    // pure argv model intentionally retains its historical executable names.
                    argv[0] = match session.agent.as_str() {
                        "cursor" => "agent".to_owned(),
                        "qodercli" => "qoder".to_owned(),
                        _ => argv[0].clone(),
                    };
                    if shell.is_none() {
                        shell = Some(system_powershell()?);
                    }
                    Some(launch(shell.as_deref().expect("resolved shell"), &argv)?)
                } else {
                    authority
                        .suppress_duplicate_restore_reference(target)
                        .map_err(io::Error::other)?;
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        plans.push((target.clone(), launch));
    }
    Ok(plans)
}

fn reference(session: &PanelAgentSession) -> Option<AgentProviderSessionReference> {
    match session.reference.kind {
        AgentProviderSessionReferenceKind::Id => {
            AgentProviderSessionReference::id(&session.reference.value)
        }
        AgentProviderSessionReferenceKind::Path => {
            AgentProviderSessionReference::path(&session.reference.value, |value| {
                Path::new(value).is_absolute()
            })
        }
    }
}

fn key(session: &PanelAgentSession) -> (String, String, bool, String) {
    (
        session.source.clone(),
        session.agent.clone(),
        session.reference.kind == AgentProviderSessionReferenceKind::Path,
        session.reference.value.clone(),
    )
}

fn system_powershell() -> io::Result<String> {
    let root = std::env::var_os("SystemRoot").ok_or(io::ErrorKind::NotFound)?;
    let path = Path::new(&root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    if !path.is_absolute() || !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "system Windows PowerShell unavailable",
        ));
    }
    path.into_os_string()
        .into_string()
        .map_err(|_| io::ErrorKind::InvalidData.into())
}

fn launch(shell: &str, argv: &[String]) -> io::Result<LaunchSelection> {
    let encoded = windows_encoded_command(&argv[0], &argv[1..]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Agent resume command exceeds Windows launch limits",
        )
    })?;
    LaunchSelection::new(
        shell.to_owned(),
        ["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand"]
            .into_iter()
            .map(str::to_owned)
            .chain(std::iter::once(encoded))
            .collect(),
    )
    .map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_defaults_on_and_invalid_values_fail_closed() {
        assert!(resume_policy(None).unwrap());
        assert!(resume_policy(Some("true".into())).unwrap());
        assert!(!resume_policy(Some("false".into())).unwrap());
        for value in ["", "1", "FALSE", " false "] {
            assert!(resume_policy(Some(value.into())).is_err());
        }
    }
    #[test]
    fn complete_reference_bounds_fit_the_typed_launch() {
        let shell = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
        for value in ["'".repeat(4096), "%".repeat(512), "中".repeat(1365)] {
            let selection = launch(shell, &["pi".into(), "--session".into(), value]).unwrap();
            assert_eq!(
                &selection.arguments()[..4],
                &["-NoLogo", "-NoProfile", "-NoExit", "-EncodedCommand"]
            );
            assert!(selection.arguments()[4].len() <= 30_000);
        }
    }
}
