use std::{
    io,
    process::{Child, ExitStatus},
    thread,
};

/// Observes startup exit, then reaps while its parent lives without terminating the child.
/// Parent exit transfers orphan ownership to the operating system.
pub struct RetainedChild(Option<Child>);
impl RetainedChild {
    pub(super) fn spawn(
        executable: &std::path::Path,
        arguments: &[&std::ffi::OsStr],
    ) -> io::Result<Self> {
        let child = std::process::Command::new(executable)
            .args(arguments)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(Self(Some(child)))
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
