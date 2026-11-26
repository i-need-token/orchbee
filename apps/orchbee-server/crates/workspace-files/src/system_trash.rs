//! Moving untracked files a discard names to the system trash, where they can be restored. The
//! move runs on a thread of its own, so on Windows COM starts on a thread nothing else has set up,
//! and a trash that stalls holds the request no longer than its deadline.
use orchbee_workbench_protocol::WorkspaceFilesFailure as Failure;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

/// Moves `files` to the trash: each is a file or a symbolic link, which goes as the link itself.
/// A failure may come after the trash took some of them; the rest are where they were.
pub(crate) fn move_to_trash(files: Vec<PathBuf>, deadline: Instant) -> Result<(), Failure> {
    let (sender, receiver) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("workspace-files-trash".to_owned())
        .spawn(move || {
            let moved = context().delete_all(&files).is_ok();
            // Nobody waits any more once the deadline passed.
            let _late = sender.send(moved);
        });
    if spawned.is_err() {
        return Err(Failure::TrashUnavailable);
    }
    match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(true) => Ok(()),
        // The thread ending without an answer means the trash panicked.
        Ok(false) | Err(RecvTimeoutError::Disconnected) => Err(Failure::TrashUnavailable),
        Err(RecvTimeoutError::Timeout) => Err(Failure::Timeout),
    }
}

/// The trash as the platform's file manager uses it. On macOS the file manager object moves files
/// itself, rather than asking Finder by script, which would ask the person for permission to
/// control Finder and play Finder's sound.
fn context() -> trash::TrashContext {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos as _};
        let mut context = trash::TrashContext::default();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context
    }
    #[cfg(not(target_os = "macos"))]
    trash::TrashContext::default()
}
