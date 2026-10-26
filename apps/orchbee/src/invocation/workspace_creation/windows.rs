//! Resolve admitted Workspace paths/programs in the executing Windows CLI.
use super::{
    CreationDirectory, OutputFormat, ParseFailure, ParsedWorkspaceCreationCommand,
    ParsedWorkspaceCreationDirectory, UsageContext, WorkspaceCreationCommand, path_failure,
    selection_envelope,
};
impl ParsedWorkspaceCreationCommand {
    /// Resolves explicit paths in the final process, before configuration or runtime access.
    ///
    /// # Errors
    /// Returns the existing workspace-create usage failure for invalid execution context.
    pub(crate) fn into_resolved(
        self,
        format: OutputFormat,
        environment: impl FnMut(&str) -> Option<std::ffi::OsString>,
        cwd: impl FnOnce() -> std::io::Result<std::path::PathBuf>,
    ) -> Result<WorkspaceCreationCommand, ParseFailure> {
        let mut options = self.options;
        // Parsed directory intent is the sole path authority, including policy defaults.
        options.cwd = None;
        let directory = match self.directory {
            ParsedWorkspaceCreationDirectory::Default => CreationDirectory::Default,
            ParsedWorkspaceCreationDirectory::Follow => CreationDirectory::Follow,
            ParsedWorkspaceCreationDirectory::Home => CreationDirectory::Home,
            ParsedWorkspaceCreationDirectory::Current => CreationDirectory::Current,
            ParsedWorkspaceCreationDirectory::Workspace => CreationDirectory::Workspace,
            ParsedWorkspaceCreationDirectory::Explicit(path) => {
                options.cwd = Some(
                    crate::invocation::user_path_windows::normalize(
                        path.as_str(),
                        environment,
                        cwd,
                    )
                    .map_err(|failure| {
                        path_failure(path.as_str(), failure, selection_envelope(Some(format)))
                    })?,
                );
                CreationDirectory::Explicit
            }
        };
        Ok(WorkspaceCreationCommand {
            project: self.project,
            source: self.source,
            directory,
            label: self.label,
            geometry: self.geometry,
            launch: crate::invocation::program::resolve(
                self.launch,
                UsageContext::Path("workspace create"),
                selection_envelope(Some(format)),
            )?,
            profile: self.profile,
            options,
            client: self.client,
            focus: self.focus,
            timeout_ms: self.timeout_ms,
        })
    }
}

#[cfg(test)]
mod tests;
