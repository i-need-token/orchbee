//! Agent launch is admitted on macOS, whose Terminal Host gives zsh and bash the managed startup
//! that confirms a prompt and strips a run prefix; other Unix hosts refuse before any lane, viewer,
//! reservation or input effect.
use super::Unsupported;
/// Admit Agent launch on macOS, and refuse it elsewhere before a caller creates a lane, viewer,
/// reservation or input submission.
///
/// # Errors
/// Returns `Unsupported` off macOS; this adapter performs no observation or mutation.
pub fn admit() -> Result<(), Unsupported> {
    if cfg!(target_os = "macos") {
        Ok(())
    } else {
        Err(Unsupported)
    }
}
