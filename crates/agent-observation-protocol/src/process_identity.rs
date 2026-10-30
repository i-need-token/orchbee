//! Pure native process identity rules shared by diagnostic and trusted Host adapters:
//! the provider registry, the invocation mode classifier, per-process recognition,
//! the per-sample anchor verdict, and the identity result vocabulary. Nothing here performs I/O.
mod executable;
mod mode;
mod registry;
mod result;
mod sample;
pub use executable::{AgentProcessRecognition, AgentUnresolvedProcess, classify_agent_process};
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
