//! Resolve all local launch intents before any canonical creation.
use super::{apply::error, template};
use crate::{
    cli::{
        Failure, FailureKind,
        console_interrupt::{ArmedSignals, SignalWatch},
    },
    invocation::user_path_windows,
};
use orchbee_workbench_protocol::LaunchOptions;
use orchbee_workbench_protocol::LaunchSelection;
use std::{ffi::OsString, time::Instant};

#[cfg(windows)]
#[path = "launch/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "launch/unix.rs"]
mod platform;
use platform::resolve;

pub(super) struct Prepared {
    pub(super) launch: Option<LaunchSelection>,
    pub(super) options: LaunchOptions,
}

pub(super) fn prepare(
    plan: &template::Plan,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Vec<Vec<Option<Prepared>>>, Failure> {
    if plan
        .leaves
        .iter()
        .flat_map(|leaf| leaf.members())
        .all(|member| !member.start)
    {
        signals.check_before(deadline)?;
        return Ok(plan
            .leaves
            .iter()
            .map(|leaf| leaf.members().map(|_| None).collect())
            .collect());
    }
    let current = std::env::current_dir()
        .map_err(|_| error(FailureKind::CheckFailed, "input-unavailable"))?;
    let ambient = std::env::var_os("PATH");
    let mut prepared = Vec::new();
    for panel in &plan.leaves {
        let mut members = Vec::new();
        for leaf in panel.members() {
            if !leaf.start {
                members.push(None);
                continue;
            }
            signals.check_before(deadline)?;
            let cwd = leaf
                .cwd
                .as_ref()
                .map(|value| {
                    user_path_windows::normalize(
                        value,
                        |key| std::env::var_os(key),
                        || Ok(current.clone()),
                    )
                    .map_err(|_| error(FailureKind::InvalidUsage, "invalid-layout"))
                })
                .transpose()?;
            let path = leaf
                .environment
                .entries()
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
                .map(|(_, value)| OsString::from(value))
                .or_else(|| ambient.clone());
            let launch = leaf
                .command
                .as_ref()
                .map(|argv| {
                    let executable = resolve(&argv[0], path.as_deref(), &current)?;
                    LaunchSelection::new(executable, argv[1..].to_vec())
                        .map_err(|_| error(FailureKind::InvalidUsage, "invalid-layout"))
                })
                .transpose()?;
            signals.check_before(deadline)?;
            members.push(Some(Prepared {
                launch,
                options: LaunchOptions {
                    plugin_pane: None,
                    plugin_context: None,
                    cwd,
                    environment: leaf.environment.clone(),
                },
            }));
        }
        prepared.push(members);
    }
    Ok(prepared)
}
