//! Bounded inbox-curl Jobs; root exit alone never releases descendant or pipe ownership.
use super::super::{CatalogUrl, Failure, LIMIT, cancelled};
use process_wrap::std::{ChildWrapper, CommandWrap, CreationFlags, JobObject};
use std::{
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;
struct Job(Box<dyn ChildWrapper>);
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.0.start_kill();
    }
}

pub(in crate::agent_screen::updating) fn text(
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<String, Failure> {
    let program = PathBuf::from(winsafe::GetSystemDirectory().map_err(|_| Failure::Process)?)
        .join("curl.exe");
    let mut result = Err(Failure::Fetch);
    for attempt in 0..3 {
        cancelled(stop, deadline)?;
        result = once(
            &program,
            source,
            stop,
            deadline.min(Instant::now() + Duration::from_secs(15)),
        );
        if matches!(result, Err(Failure::Deadline)) && cancelled(stop, deadline).is_ok() {
            result = Err(Failure::Fetch);
        }
        if !matches!(result, Err(Failure::Fetch)) {
            break;
        }
        if attempt < 2 {
            let retry = deadline.min(Instant::now() + Duration::from_millis(250));
            while Instant::now() < retry {
                cancelled(stop, deadline)?;
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    result
}
fn once(
    program: &std::path::Path,
    source: &CatalogUrl,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<String, Failure> {
    let mut command = Command::new(program);
    command
        .env_clear()
        // Winsock expands its provider DLL path through SystemRoot; derive it from the OS.
        .env(
            "SystemRoot",
            program
                .parent()
                .and_then(std::path::Path::parent)
                .ok_or(Failure::Process)?,
        )
        .args([
            "--disable",
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-redirs",
            "5",
            "--proto",
            "=http,https",
            "--proto-redir",
            "=http,https",
            "--connect-timeout",
            "5",
            "--max-time",
            "15",
            "--max-filesize",
            "262144",
            "--url",
            &source.0,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut wrapped = CommandWrap::from(command);
    wrapped
        .wrap(CreationFlags(CREATE_NO_WINDOW))
        .wrap(JobObject);
    let mut job = Job(wrapped.spawn().map_err(|_| Failure::Process)?);
    // The wrapper owns duplicate parent pipe writers until it is dropped.
    drop(wrapped);
    let stdout = job.0.inner_mut().stdout().take();
    let (send, receive) = mpsc::sync_channel(1);
    let mut failure = None;
    let reader: Option<JoinHandle<()>> = match stdout {
        Some(stdout) => match thread::Builder::new()
            .name("orchbee-rule-fetch-output".into())
            .spawn(move || {
                let mut bytes = Vec::new();
                let result = stdout
                    .take((LIMIT + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| Failure::Fetch)
                    .and({
                        if bytes.len() > LIMIT {
                            Err(Failure::TooLarge)
                        } else {
                            Ok(bytes)
                        }
                    });
                let _ = send.send(result);
            }) {
            Ok(handle) => Some(handle),
            Err(_) => {
                failure = Some(Failure::Process);
                None
            }
        },
        None => {
            failure = Some(Failure::Process);
            None
        }
    };
    let mut bytes = None;
    let mut stopped = false;
    let status = loop {
        if failure.is_none() {
            failure = cancelled(stop, deadline).err();
        }
        match receive.try_recv() {
            Ok(Ok(value)) => bytes = Some(value),
            Ok(Err(error)) => {
                failure.get_or_insert(error);
            }
            Err(mpsc::TryRecvError::Disconnected) if bytes.is_none() && failure.is_none() => {
                failure = Some(Failure::Process);
            }
            Err(_) => {}
        }
        let exited = match job.0.inner_mut().try_wait() {
            Ok(value) => value.is_some(),
            Err(_) => {
                failure.get_or_insert(Failure::Process);
                false
            }
        };
        if !stopped && (failure.is_some() || exited) {
            // This exact Job remains owned if kill/settlement fails; no later update is admitted.
            match job.0.start_kill() {
                Ok(()) => stopped = true,
                Err(_) => {
                    failure.get_or_insert(Failure::Process);
                }
            }
        }
        if stopped {
            match job.0.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(_) => {
                    failure.get_or_insert(Failure::Process);
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    };
    // Confirmed Job exit closes every writer; join only after the reader's completion is observable.
    if let Some(reader) = reader {
        while !reader.is_finished() {
            thread::sleep(Duration::from_millis(1));
        }
        if reader.join().is_err() {
            failure.get_or_insert(Failure::Process);
        }
    }
    if bytes.is_none() {
        match receive.try_recv() {
            Ok(Ok(value)) => bytes = Some(value),
            Ok(Err(error)) => {
                failure.get_or_insert(error);
            }
            Err(_) => {
                failure.get_or_insert(Failure::Process);
            }
        }
    }
    if let Some(error) = failure {
        return Err(error);
    }
    if status.code() == Some(63) {
        return Err(Failure::TooLarge);
    }
    if !status.success() {
        return Err(Failure::Fetch);
    }
    String::from_utf8(bytes.ok_or(Failure::Fetch)?).map_err(|_| Failure::InvalidUtf8)
}
