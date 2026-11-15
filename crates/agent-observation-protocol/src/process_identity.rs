//! Pure native process identity rules shared by diagnostic and trusted Host adapters:
//! the provider registry, the invocation mode classifier, per-process recognition,
//! the per-sample anchor verdict, the identity result vocabulary, and interactive shell
//! recognition. Nothing here performs I/O.
mod executable;
mod mode;
mod registry;
mod result;
mod sample;
mod shell;
pub use executable::{
    AgentProcessRecognition, AgentUnresolvedProcess, classify_agent_process,
    interactive_agent_arguments,
};
pub use mode::{AgentInvocationMode, AgentModeGrammar};
pub use registry::{
    AGENT_PROVIDER_REGISTRY, AGENT_PROVIDER_REGISTRY_VERSION, AgentAlias, AgentAliasClass,
    AgentBundledRuntime, AgentInstallRoot, AgentInstallShape, AgentProviderEntry,
};
pub use result::{
    AgentIdentityGrade, AgentIdentityUnknownReason, AgentIdentityUnsupportedReason,
    AgentProcessAnchor, AgentProcessIdentity,
};
pub use sample::{
    AgentAnchor, AgentAnchorEvidence, AgentProcessRow, AgentSampleVerdict, descends_from,
    evaluate_agent_sample,
};
pub use shell::{
    InteractiveShells, ShellRoot, classify_shell_root, idle_shell_only, is_console_host,
};

/// The POSIX shells identity reads, both as a terminal's interactive root and as the runtime of a
/// script wrapper.
const POSIX_SHELLS: [&str; 4] = ["sh", "bash", "zsh", "fish"];

/// Whether `name`, a basename from [native_agent_basename], is a POSIX shell's argv[0]. A login
/// shell's argv[0] starts with `-`, which is not part of the shell's name.
fn names_posix_shell(name: &str) -> bool {
    POSIX_SHELLS.contains(&name.strip_prefix('-').unwrap_or(name))
}

/// Extracts the native executable basename and one recognized suffix.
pub fn native_agent_basename(path: &str) -> String {
    let value = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    for suffix in [
        ".exe", ".com", ".cmd", ".bat", ".ps1", ".js", ".mjs", ".cjs", ".py",
    ] {
        if let Some(stem) = value.strip_suffix(suffix) {
            return stem.to_owned();
        }
    }
    value
}

#[cfg(test)]
mod tests;
