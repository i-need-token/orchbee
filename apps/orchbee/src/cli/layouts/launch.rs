//! Resolve all local launch intents before any canonical creation.
use super::{apply::error, template};
use crate::{
    cli::{Failure, FailureKind, source::ArmedSignals},
    invocation::user_path_windows,
};
use orchbee_workbench_protocol::LaunchOptions;
use orchbee_workbench_protocol::LaunchSelection;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Instant,
};

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

fn resolve(
    program: &str,
    search: Option<&std::ffi::OsStr>,
    current: &Path,
) -> Result<String, Failure> {
    let failure = || error(FailureKind::CheckFailed, "program-unavailable");
    let path = Path::new(program);
    let candidates = if path.is_absolute() {
        vec![path.to_path_buf()]
    } else if program.contains(['/', '\\']) {
        vec![current.join(path)]
    } else {
        std::env::split_paths(search.ok_or_else(failure)?)
            .take(256)
            .filter(|directory| directory.is_absolute() && directory.as_os_str().len() <= 4096)
            .map(|directory| {
                directory.join(if path.extension().is_none() {
                    format!("{program}.exe")
                } else {
                    program.to_owned()
                })
            })
            .collect::<Vec<PathBuf>>()
    };
    for candidate in candidates {
        if candidate.is_file() {
            // Consistent Windows executable spelling also avoids cmd.exe treating '/' in argv0 as switches.
            let value = candidate.to_str().ok_or_else(failure)?.replace('/', "\\");
            return Ok(value);
        }
    }
    Err(failure())
}
