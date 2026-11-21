//! Windows API support functions.

use std::{
	io::{Error, Result},
	ops::ControlFlow,
	time::Duration,
};

#[cfg(feature = "tracing")]
use tracing::{debug, instrument};
use windows::{
	Win32::{
		Foundation::{
			CloseHandle, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, HANDLE, INVALID_HANDLE_VALUE,
		},
		System::{
			Diagnostics::ProcessSnapshotting::{
				HPSS, HPSSWALK, PSS_CAPTURE_THREADS, PSS_THREAD_ENTRY, PSS_WALK_THREADS,
				PssCaptureSnapshot, PssFreeSnapshot, PssWalkMarkerCreate, PssWalkMarkerFree,
				PssWalkSnapshot,
			},
			IO::{CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED},
			JobObjects::{
				AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
				JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
				JOBOBJECT_ASSOCIATE_COMPLETION_PORT, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
				JobObjectAssociateCompletionPortInformation, JobObjectExtendedLimitInformation,
				SetInformationJobObject, TerminateJobObject,
			},
			Threading::{
				CREATE_SUSPENDED, GetCurrentProcess, GetProcessId, GetProcessIdOfThread, INFINITE,
				OpenThread, PROCESS_CREATION_FLAGS, ResumeThread, THREAD_QUERY_LIMITED_INFORMATION,
				THREAD_SUSPEND_RESUME,
			},
		},
	},
	core::HRESULT,
};

#[derive(Debug)]
struct OwnedHandle(HANDLE);

impl OwnedHandle {
	fn into_raw(self) -> HANDLE {
		let handle = self.0;
		std::mem::forget(self);
		handle
	}
}

impl Drop for OwnedHandle {
	fn drop(&mut self) {
		unsafe { CloseHandle(self.0) }.ok();
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct JobCreationFlags {
	pub flags: PROCESS_CREATION_FLAGS,
	pub resume_after_assignment: bool,
}

#[cfg(any(feature = "tokio1", test))]
pub(crate) fn job_creation_flags(user_flags: PROCESS_CREATION_FLAGS) -> JobCreationFlags {
	job_creation_flags_for(user_flags, true)
}

/// The creation flags of a Job's process. Suspension keeps a process from creating children before
/// it is assigned, so that none of them escapes the Job; a Job whose descendants all leave it
/// (`suspend_for_assignment` false) starts the process at once and assigns it right after.
pub(crate) fn job_creation_flags_for(
	user_flags: PROCESS_CREATION_FLAGS,
	suspend_for_assignment: bool,
) -> JobCreationFlags {
	if !suspend_for_assignment {
		return JobCreationFlags {
			flags: user_flags,
			resume_after_assignment: false,
		};
	}
	JobCreationFlags {
		flags: user_flags | CREATE_SUSPENDED,
		resume_after_assignment: !user_flags.contains(CREATE_SUSPENDED),
	}
}

#[cfg(test)]
mod creation_flag_tests {
	use windows::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};

	use super::*;

	#[test]
	fn adds_suspension_without_losing_user_flags() {
		let user_flags = CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;
		let policy = job_creation_flags(user_flags);
		assert_eq!(policy.flags, user_flags | CREATE_SUSPENDED);
		assert!(policy.resume_after_assignment);
	}

	#[test]
	fn preserves_explicit_suspension() {
		let user_flags = CREATE_NO_WINDOW | CREATE_SUSPENDED;
		let policy = job_creation_flags(user_flags);
		assert_eq!(policy.flags, user_flags);
		assert!(!policy.resume_after_assignment);
	}
}

#[derive(Clone, Copy, Debug)]
pub struct JobHandle(pub HANDLE);

unsafe impl Send for JobHandle {}
unsafe impl Sync for JobHandle {}

#[derive(Clone, Copy, Debug)]
pub struct PortHandle(pub HANDLE);

unsafe impl Send for PortHandle {}
unsafe impl Sync for PortHandle {}

/// A JobObject and its associated completion port.
///
/// This struct closes the handles when dropped.
#[derive(Debug)]
pub(crate) struct JobPort {
	pub job: JobHandle,
	pub completion_port: PortHandle,
}

impl Drop for JobPort {
	fn drop(&mut self) {
		unsafe { CloseHandle(self.job.0) }.ok();
		unsafe { CloseHandle(self.completion_port.0) }.ok();
	}
}

/// Create a JobObject and an associated completion port.
///
/// If `kill_on_drop` is true, we opt into the `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` flag, which
/// essentially implements the "reap children" feature of Unix systems directly in Win32.
#[cfg(feature = "tokio1")]
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn make_job_object(process_handle: HANDLE, kill_on_drop: bool) -> Result<JobPort> {
	make_job_object_with_breakaway(process_handle, kill_on_drop, false, false)
}

/// Create a JobObject and an associated completion port, as [`make_job_object`] does.
///
/// If `breakaway` is true, we opt into the `JOB_OBJECT_LIMIT_BREAKAWAY_OK` flag, so an assigned
/// process may create a child outside the job by asking for `CREATE_BREAKAWAY_FROM_JOB`.
///
/// If `silent_breakaway` is true, we opt into the `JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK` flag, so
/// the processes an assigned process creates are not associated with the job.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn make_job_object_with_breakaway(
	process_handle: HANDLE,
	kill_on_drop: bool,
	breakaway: bool,
	silent_breakaway: bool,
) -> Result<JobPort> {
	let job = OwnedHandle(unsafe { CreateJobObjectW(None, None) }.map_err(Error::other)?);
	#[cfg(feature = "tracing")]
	debug!(?job, "done CreateJobObjectW");

	let completion_port =
		OwnedHandle(unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, None, 0, 1) }?);
	#[cfg(feature = "tracing")]
	debug!(?completion_port, "done CreateIoCompletionPort");

	let associate_completion = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
		CompletionKey: job.0.0 as _,
		CompletionPort: completion_port.0,
	};

	unsafe {
		SetInformationJobObject(
			job.0,
			JobObjectAssociateCompletionPortInformation,
			(&associate_completion) as *const _ as _,
			std::mem::size_of_val(&associate_completion)
				.try_into()
				.expect("cannot safely cast to DWORD"),
		)
	}?;
	#[cfg(feature = "tracing")]
	debug!(
		?associate_completion,
		"done SetInformationJobObject(completion)"
	);

	let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();

	if kill_on_drop {
		info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
	}
	if breakaway {
		info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_BREAKAWAY_OK;
	}
	if silent_breakaway {
		info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK;
	}

	unsafe {
		SetInformationJobObject(
			job.0,
			JobObjectExtendedLimitInformation,
			&info as *const _ as _,
			std::mem::size_of_val(&info)
				.try_into()
				.expect("cannot safely cast to DWORD"),
		)
	}?;
	#[cfg(feature = "tracing")]
	debug!(?info, "done SetInformationJobObject(limit)");

	unsafe { AssignProcessToJobObject(job.0, process_handle) }?;
	#[cfg(feature = "tracing")]
	debug!(?job, ?process_handle, "done AssignProcessToJobObject");

	Ok(JobPort {
		job: JobHandle(job.into_raw()),
		completion_port: PortHandle(completion_port.into_raw()),
	})
}

/// A snapshot of one process that [`resume_threads`] captured; dropping it frees the snapshot.
struct ProcessSnapshot(HPSS);

impl Drop for ProcessSnapshot {
	fn drop(&mut self) {
		// SAFETY: `PssCaptureSnapshot` captured this snapshot in the current process, and only this
		// owner frees it.
		unsafe { PssFreeSnapshot(GetCurrentProcess(), self.0) };
	}
}

/// A walk marker over a [`ProcessSnapshot`]; dropping it frees the marker.
struct SnapshotWalk(HPSSWALK);

impl Drop for SnapshotWalk {
	fn drop(&mut self) {
		// SAFETY: `PssWalkMarkerCreate` created this marker, and only this owner frees it.
		unsafe { PssWalkMarkerFree(self.0) };
	}
}

/// Initialized storage for one `PSS_THREAD_ENTRY`, aligned as that entry, which a snapshot walk
/// fills as bytes.
#[repr(C)]
struct ThreadEntryBytes {
	_alignment: [PSS_THREAD_ENTRY; 0],
	bytes: [u8; size_of::<PSS_THREAD_ENTRY>()],
}

/// The result of a process snapshot function, which returns a Win32 error code.
fn snapshot_result(code: u32) -> Result<()> {
	if code == ERROR_SUCCESS.0 {
		Ok(())
	} else {
		Err(Error::from_raw_os_error(code as i32))
	}
}

/// Resume the threads of a process created suspended, which until then has only its main thread.
///
/// The threads come from a snapshot of that process alone (`PssCaptureSnapshot` with
/// `PSS_CAPTURE_THREADS`). Upstream walked a snapshot of every thread on the system instead, which
/// took tens of milliseconds per process on a busy machine. The standard library keeps the main
/// thread handle that process creation returns, but exposes it only on nightly.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn resume_threads(child_process: HANDLE) -> Result<()> {
	// SAFETY: the caller passes the handle of a process it created and keeps open.
	let child_id = unsafe { GetProcessId(child_process) };
	if child_id == 0 {
		return Err(Error::last_os_error());
	}

	let mut snapshot = HPSS::default();
	// SAFETY: `child_process` is open, as above, and `snapshot` is writable for the call.
	snapshot_result(unsafe {
		PssCaptureSnapshot(child_process, PSS_CAPTURE_THREADS, None, &mut snapshot)
	})?;
	let snapshot = ProcessSnapshot(snapshot);

	let mut walk = HPSSWALK::default();
	// SAFETY: without an allocator the marker uses the process heap; `walk` is writable.
	snapshot_result(unsafe { PssWalkMarkerCreate(None, &mut walk) })?;
	let walk = SnapshotWalk(walk);

	let mut resumed = false;
	loop {
		let mut entry = ThreadEntryBytes {
			_alignment: [],
			bytes: [0; size_of::<PSS_THREAD_ENTRY>()],
		};
		// SAFETY: the snapshot and the marker are live, and `bytes` is writable and holds exactly
		// one entry.
		let status = unsafe {
			PssWalkSnapshot(snapshot.0, PSS_WALK_THREADS, walk.0, Some(&mut entry.bytes))
		};
		if status == ERROR_NO_MORE_ITEMS.0 {
			break;
		}
		snapshot_result(status)?;
		// SAFETY: the walk filled `bytes` with one entry, the storage is aligned for it, and each
		// field of the entry, an integer or a raw pointer, is valid for any bit pattern.
		let entry = unsafe { entry.bytes.as_ptr().cast::<PSS_THREAD_ENTRY>().read() };

		// SAFETY: opening a thread by its ID has no precondition; failure returns an error.
		let thread = OwnedHandle(unsafe {
			OpenThread(
				THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION,
				false,
				entry.ThreadId,
			)
		}?);
		// A thread ID is reused only after its thread ends. The child's process ID stays reserved
		// while the caller keeps its handle open, so a thread that reports it is the child's.
		// SAFETY: `thread` is open with `THREAD_QUERY_LIMITED_INFORMATION`.
		if unsafe { GetProcessIdOfThread(thread.0) } != child_id {
			continue;
		}
		// SAFETY: `thread` is open with `THREAD_SUSPEND_RESUME`.
		let previous_count = unsafe { ResumeThread(thread.0) };
		if previous_count == u32::MAX {
			return Err(Error::last_os_error());
		}
		if previous_count > 0 {
			resumed = true;
		}
	}

	if resumed {
		Ok(())
	} else {
		Err(Error::other("no thread belonging to the child was found"))
	}
}

/// Terminate a job object without waiting for the processes to exit.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn terminate_job(job: JobHandle, exit_code: u32) -> Result<()> {
	unsafe { TerminateJobObject(job.0, exit_code) }.map_err(Error::other)
}

/// Wait for a job to complete.
#[cfg_attr(feature = "tracing", instrument(level = "debug"))]
pub(crate) fn wait_on_job(
	completion_port: PortHandle,
	timeout: Option<Duration>,
) -> Result<ControlFlow<()>> {
	const QUEUED_NOTIFICATION_BOUND: u32 = 64;
	let started = std::time::Instant::now();
	let mut drained_after_timeout = 0;
	loop {
		let mut code: u32 = 0;
		let mut key: usize = 0;
		let mut overlapped = OVERLAPPED::default();
		let mut lp_overlapped = &mut overlapped as *mut OVERLAPPED;

		let result = unsafe {
			GetQueuedCompletionStatus(
				completion_port.0,
				&mut code,
				&mut key,
				&mut lp_overlapped as *mut _,
				timeout.map_or(INFINITE, |d| {
					d.saturating_sub(started.elapsed())
						.as_millis()
						.try_into()
						.unwrap_or(INFINITE)
				}),
			)
		};

		// ignore timing out errors unless the timeout was specified to INFINITE
		// https://docs.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-getqueuedcompletionstatus
		if timeout.is_some()
			&& result
				.as_ref()
				.is_err_and(|error| error.code() == HRESULT::from_win32(258))
			&& lp_overlapped.is_null()
		{
			return Ok(ControlFlow::Continue(()));
		}

		result.map_err(Error::other)?;
		// This private port carries only this Job's notifications. Win32 code 4
		// is JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO; NEW_PROCESS and EXIT_PROCESS
		// notifications cannot prove completion of the whole Job.
		if code == 4 && key != 0 {
			return Ok(ControlFlow::Break(()));
		}
		// Past the timeout, keep reading only notifications already queued: the next read
		// waits zero milliseconds, so an empty port returns at once. The bound keeps a Job
		// that starts processes without pause from holding this call indefinitely.
		if timeout.is_some_and(|limit| started.elapsed() >= limit) {
			drained_after_timeout += 1;
			if drained_after_timeout > QUEUED_NOTIFICATION_BOUND {
				return Ok(ControlFlow::Continue(()));
			}
		}
	}
}
