use super::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, UsableShellCandidates, default_launch,
    select_default_interactive_launch,
};
use orchbee_local_site_runtime::{WindowsShellMode, WindowsShellSettings};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[path = "windows/pe.rs"]
mod pe;

fn usable_shell_candidates(
    env: &impl Fn(&str) -> Option<OsString>,
    machine: u16,
) -> UsableShellCandidates {
    let search = env("PATH");
    let inherited = search
        .as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join("pwsh.exe"))
        .find_map(|path| usable_automatic_shell(&path, machine));
    let fallback = env("SystemRoot").and_then(|root| {
        usable(&Path::new(&root).join("System32/WindowsPowerShell/v1.0/powershell.exe"))
    });
    UsableShellCandidates {
        inherited,
        account: None,
        fallback,
    }
}

fn usable_automatic_shell(path: &Path, machine: u16) -> Option<String> {
    let executable = usable(path)?;
    let resolved = path.canonicalize().ok()?;
    if resolved
        .ancestors()
        .skip(1)
        .any(|directory| directory.join("AppxManifest.xml").is_file())
    {
        return None;
    }
    pe::compatible_image(path, machine).then_some(executable)
}

pub(super) fn resolve(
    shell: &WindowsShellSettings,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
    let mut launch = if shell.executable().is_empty() {
        select_default_interactive_launch(usable_shell_candidates(&env, native_machine()))?
    } else {
        let executable = resolve_configured(shell.executable(), env("PATH"))
            .ok_or(DefaultInteractiveLaunchError::NoUsableShell)?;
        default_launch(&executable)
    };
    if shell.mode() == WindowsShellMode::Login && login_supported(&launch.executable) {
        launch.arguments.push("-l".to_owned());
    }
    Ok(launch)
}

fn native_machine() -> u16 {
    // This safe wrapper reports the emulated architecture on ARM64 for x86/x64 Workers.
    let architecture = winsafe::GetNativeSystemInfo().wProcessorArchitecture;
    if architecture == winsafe::co::PROCESSOR_ARCHITECTURE::ARM64 {
        0xaa64
    } else if architecture == winsafe::co::PROCESSOR_ARCHITECTURE::INTEL {
        0x014c
    } else {
        0x8664
    }
}

fn resolve_configured(executable: &str, path: Option<OsString>) -> Option<String> {
    let selected = Path::new(executable);
    if selected.is_absolute() {
        return usable(selected);
    }
    let names: Vec<PathBuf> = if selected.extension().is_none() {
        vec![selected.to_owned(), selected.with_extension("exe")]
    } else {
        vec![selected.to_owned()]
    };
    path.as_deref()
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|directory| directory.is_absolute())
        .find_map(|directory| names.iter().find_map(|name| usable(&directory.join(name))))
}

fn login_supported(executable: &str) -> bool {
    let name = executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(executable)
        .to_ascii_lowercase();
    matches!(
        name.strip_suffix(".exe").unwrap_or(&name),
        "sh" | "bash" | "zsh" | "ksh" | "dash" | "ash" | "mksh" | "fish" | "csh" | "tcsh"
    )
}

fn usable(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    (path.is_absolute() && path.is_file() && text.len() <= 4096 && !text.contains('\0'))
        .then(|| text.to_owned())
}

#[cfg(test)]
#[path = "windows/tests.rs"]
mod tests;
