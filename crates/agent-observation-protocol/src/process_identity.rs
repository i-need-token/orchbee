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
pub use shell::{ShellRoot, classify_shell_root, idle_shell_only, is_console_host};

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
