//! Retains exact child exit observation and transfers a dropped live child to a background
//! reaper while the parent remains alive. It never terminates that child on drop; after parent
//! exit, orphan reaping belongs to the operating system.

use std::{
    io,
    process::{Child, ExitStatus},
    thread,
};

/// Observes startup exit, then reaps while its parent lives without terminating the child.
/// Parent exit transfers orphan ownership to the operating system.
pub struct RetainedChild(Option<Child>);
impl RetainedChild {
    pub(crate) fn spawn(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
    ) -> io::Result<Self> {
        // The child outlives the activating Client, so it keeps none of that Client's directories
        // busy or reachable, such as a Worktree the person later removes or unmounts.
        let child = std::process::Command::new(executable)
            .args(arguments)
            .current_dir("/")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(Self(Some(child)))
    }
    /// The operating-system process identifier of the held child.
    pub fn id(&self) -> Option<u32> {
        self.0.as_ref().map(Child::id)
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self.0.as_mut() {
            Some(child) => child.try_wait(),
            None => Ok(None),
        }
    }
}
impl Drop for RetainedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            // Failure to allocate a reaper must never kill an already-started service.
            // In that case the OS reaps it when this parent exits.
            let _reaper = thread::Builder::new()
                .name("orchbee-child-reaper".into())
                .spawn(move || {
                    let _status = child.wait();
                });
        }
    }
}
