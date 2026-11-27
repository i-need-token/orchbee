//! Bounded draining of a child's nonblocking standard output and error, shared by each platform's
//! capture loop: each kept output stops growing at its limit and records that it was cut. A
//! caller that follows standard error as it arrives is handed every read, past the limit too.
use crate::{CommandFailure, CommandOutput};
use orchbee_local_ipc::ChildOutputReader;
use std::io::{self, Read};

impl CommandOutput {
    pub(super) fn empty() -> Self {
        Self {
            exit_code: None,
            failure: None,
            stdout: vec![],
            stderr: vec![],
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }
    pub(super) fn failed(mut self, reason: CommandFailure) -> Self {
        self.failure = Some(reason);
        self
    }
}

/// Receives every read of standard error as it arrives; [`ignore_output`] for a caller that only
/// keeps it.
pub(super) type ErrorOutput<'a> = &'a mut dyn FnMut(&[u8]);

/// An [`ErrorOutput`] that follows nothing.
pub(super) fn ignore_output(_bytes: &[u8]) {}

pub(super) fn drain_tail(
    out: &mut Option<ChildOutputReader>,
    err: &mut Option<ChildOutputReader>,
    result: &mut CommandOutput,
    output_limit: usize,
    error_output: ErrorOutput<'_>,
) -> io::Result<()> {
    for _ in 0..64 {
        drain(
            out,
            &mut result.stdout,
            &mut result.stdout_truncated,
            true,
            output_limit,
            &mut ignore_output,
        )?;
        drain(
            err,
            &mut result.stderr,
            &mut result.stderr_truncated,
            true,
            output_limit,
            &mut *error_output,
        )?;
        if out.is_none() && err.is_none() {
            return Ok(());
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}

/// Reads what `reader` holds now, at most eight reads, keeping up to `output_limit` bytes in
/// `kept` and handing every read to `observe`.
pub(super) fn drain(
    reader: &mut Option<ChildOutputReader>,
    kept: &mut Vec<u8>,
    truncated: &mut bool,
    settled: bool,
    output_limit: usize,
    observe: ErrorOutput<'_>,
) -> io::Result<()> {
    let Some(pipe) = reader.as_mut() else {
        return Ok(());
    };
    let mut bytes = [0; 4096];
    for _ in 0..8 {
        match pipe.read(&mut bytes) {
            Ok(0) => {
                *reader = None;
                return Ok(());
            }
            Ok(count) => {
                observe(&bytes[..count]);
                let keep = count.min(output_limit.saturating_sub(kept.len()));
                kept.extend_from_slice(&bytes[..keep]);
                *truncated |= keep < count;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if settled {
                    *reader = None;
                }
                return Ok(());
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
