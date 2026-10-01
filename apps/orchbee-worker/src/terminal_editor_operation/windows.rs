//! Windows preference and native argv adapter; no editor process or document is created here.
use orchbee_capability_node_protocol::{
    LaunchSelection, MAXIMUM_LAUNCH_SELECTION_ARGUMENTS, MAXIMUM_LAUNCH_SELECTION_BYTES,
    TerminalEditorCommandRefusal as Refusal, TerminalEditorCommandSource as Source,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(super) fn resolve() -> Result<(Source, LaunchSelection), Refusal> {
    resolve_with(
        |name| std::env::var_os(name),
        || {
            winsafe::GetSystemDirectory()
                .map(PathBuf::from)
                .map_err(|_| Refusal::ProgramUnavailable)
        },
    )
}

fn resolve_with(
    env: impl Fn(&str) -> Option<OsString>,
    system_directory: impl FnOnce() -> Result<PathBuf, Refusal>,
) -> Result<(Source, LaunchSelection), Refusal> {
    for (name, source) in [("VISUAL", Source::Visual), ("EDITOR", Source::Editor)] {
        let Some(value) = env(name) else { continue };
        let command = value.into_string().map_err(|_| Refusal::InvalidCommand)?;
        if command.trim().is_empty() {
            continue;
        }
        if command.len() > MAXIMUM_LAUNCH_SELECTION_BYTES || command.contains('\0') {
            return Err(Refusal::InvalidCommand);
        }
        let mut argv = winsafe::CommandLineToArgv(&command).map_err(|_| Refusal::InvalidCommand)?;
        if argv.is_empty() || argv.len() - 1 > MAXIMUM_LAUNCH_SELECTION_ARGUMENTS {
            return Err(Refusal::InvalidCommand);
        }
        let program = argv.remove(0);
        let executable = resolve_program(&program, || env("PATH"))?;
        let selection =
            LaunchSelection::new(executable, argv).map_err(|_| Refusal::InvalidCommand)?;
        return Ok((source, selection));
    }
    let executable =
        usable(&system_directory()?.join("notepad.exe")).ok_or(Refusal::ProgramUnavailable)?;
    let selection =
        LaunchSelection::new(executable, Vec::new()).map_err(|_| Refusal::InvalidCommand)?;
    Ok((Source::Default, selection))
}

fn resolve_program(
    program: &str,
    path: impl FnOnce() -> Option<OsString>,
) -> Result<String, Refusal> {
    if program.is_empty() {
        return Err(Refusal::InvalidCommand);
    }
    let selected = Path::new(program);
    if selected.is_absolute() {
        return usable(selected).ok_or(Refusal::ProgramUnavailable);
    }
    // Drive-relative, rooted and relative-directory forms never inherit a cwd.
    if program.contains(['/', '\\', ':']) || program == "." || program == ".." {
        return Err(Refusal::InvalidCommand);
    }
    let names = if selected.extension().is_none() {
        vec![selected.to_path_buf(), selected.with_extension("exe")]
    } else {
        vec![selected.to_path_buf()]
    };
    path()
        .as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| names.iter().find_map(|name| usable(&directory.join(name))))
        .ok_or(Refusal::ProgramUnavailable)
}

fn usable(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (path.is_absolute() && path.is_file() && !text.contains('\0')).then(|| text.to_owned())
}

#[cfg(test)]
mod tests;
