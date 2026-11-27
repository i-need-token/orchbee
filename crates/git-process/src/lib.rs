//! Git child processes for Orchbee's native owners: the Git program a command starts, the
//! repository paths a command may name, and one run per command under a process-tree owner with
//! bounded output. Each consumer keeps its own environment policy, deadlines and error vocabulary.

mod path;
mod run;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

pub use path::{CanonicalPath, CanonicalPathError, canonical_path};
pub use platform::{GIT_PROGRAM, checkout_spelling, is_admissible_path, read_program};
pub use run::{Output, OutputBounds, Run, RunError};
