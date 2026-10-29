//! Worker launch facts use the shared host shell policy, resolved once per Worker generation.
pub(crate) use orchbee_local_runtime::default_launch_environment;
use orchbee_local_runtime::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, resolve_default_interactive_launch,
};

/// The System shell this Worker generation resolved before it connected to the Server. Every
/// default launch of the generation uses this one resolution, so a shell installed or removed
/// while the Session runs changes default launches only in the next Session.
#[derive(Debug)]
pub(crate) struct SystemShell(Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError>);

impl SystemShell {
    pub(crate) fn resolve() -> Self {
        Self(resolve_default_interactive_launch())
    }

    #[cfg(all(test, windows))]
    pub(crate) const fn pinned(launch: DefaultInteractiveLaunch) -> Self {
        Self(Ok(launch))
    }

    /// The resolved shell's file name, which this generation's `WorkerHello` reports.
    pub(crate) fn file_name(&self) -> Option<&str> {
        let executable = &self.0.as_ref().ok()?.executable;
        executable.rsplit(['/', '\\']).next()
    }

    /// # Errors
    /// The generation's resolution failure, which refuses every System-shell launch alike.
    pub(crate) fn launch(&self) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
        self.0.clone()
    }
}
