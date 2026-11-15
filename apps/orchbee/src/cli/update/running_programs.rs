//! Whether a process runs from a set of files: the process scan, which finds each process whose
//! program file lies inside a folder, and the write probe, which finds a program or library loaded
//! by a process the scan cannot query. Uninstall waits on the scan; store cleanup removes a folder
//! only when both find nothing. Only Windows keeps a release store, so elsewhere nothing can be told.
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(super) use unix::no_process_runs_from;
#[cfg(windows)]
pub(super) use windows::{StoreProcess, inside, no_process_runs_from, processes_inside, same};
