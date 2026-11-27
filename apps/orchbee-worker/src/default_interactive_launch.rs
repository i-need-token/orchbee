//! Worker launch facts use the shared host shell policy, resolved once per Worker generation.
pub(crate) use orchbee_local_runtime::default_launch_environment;
use orchbee_local_runtime::{
    DefaultInteractiveLaunch, DefaultInteractiveLaunchError, resolve_default_interactive_launch,
};

/// The System shell this Worker generation resolved before it connected to the Server. Every
/// default launch of the generation uses this one resolution, so a shell installed or removed
/// while the Session runs changes default launches only in the next Session.
#[derive(Debug)]
pub(crate) struct SystemShell {
    launch: Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError>,
    /// The `PATH` an explicit program starts with, read at the generation's first explicit launch.
    #[cfg(target_os = "macos")]
    launch_path: std::sync::OnceLock<Option<String>>,
}

impl SystemShell {
    pub(crate) fn resolve() -> Self {
        Self::new(resolve_default_interactive_launch())
    }

    #[cfg(all(test, windows))]
    pub(crate) const fn pinned(launch: DefaultInteractiveLaunch) -> Self {
        Self::new(Ok(launch))
    }

    const fn new(launch: Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError>) -> Self {
        Self {
            launch,
            #[cfg(target_os = "macos")]
            launch_path: std::sync::OnceLock::new(),
        }
    }

    /// The resolved shell's file name, which this generation's `WorkerHello` reports.
    pub(crate) fn file_name(&self) -> Option<&str> {
        let executable = &self.launch.as_ref().ok()?.executable;
        executable.rsplit(['/', '\\']).next()
    }

    /// # Errors
    /// The generation's resolution failure, which refuses every System-shell launch alike.
    pub(crate) fn launch(&self) -> Result<DefaultInteractiveLaunch, DefaultInteractiveLaunchError> {
        self.launch.clone()
    }

    /// The `PATH` a program started without a shell gets on macOS: a new terminal's login shell's
    /// `PATH`, then this Worker's own folders. The generation's first explicit launch runs that
    /// shell once, within six seconds, and later ones reuse the answer, so a profile change
    /// reaches them in the next Session. `None` keeps this Worker's own `PATH`.
    #[cfg(target_os = "macos")]
    pub(crate) fn launch_path(&self) -> Option<&str> {
        self.launch_path
            .get_or_init(|| {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
                let login = crate::login_path::first(deadline, || false)?;
                crate::login_path::joined(&login, std::env::var_os("PATH").as_deref())
            })
            .as_deref()
    }
}
