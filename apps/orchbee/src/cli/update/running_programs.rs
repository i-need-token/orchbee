//! Whether a process runs from a set of files: the process scan, which finds each process whose
//! program file lies inside a folder, and the write probe, which finds a program or library loaded
//! by a process the scan cannot query. Uninstall waits on the scan; store cleanup removes a folder
//! only when both find nothing, which only Windows can tell, so elsewhere cleanup removes nothing,
//! except window versions, which only Orchbee starts programs from and the scan alone covers.
#[cfg(not(windows))]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(super) use unix::{
    StoreProcess, ancestors, inside, no_process_runs_from, no_process_started_from,
    processes_inside, same,
};
#[cfg(windows)]
pub(super) use windows::{
    StoreProcess, inside, no_process_runs_from, no_process_runs_from_except, processes_inside, same,
};
