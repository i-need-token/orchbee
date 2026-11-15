//! Bounded draining of a child's nonblocking standard output and error, shared by each platform's
//! capture loop: each kept output stops growing at its limit and records that it was cut.
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

pub(super) fn drain_tail(
    out: &mut Option<ChildOutputReader>,
    err: &mut Option<ChildOutputReader>,
    result: &mut CommandOutput,
    output_limit: usize,
) -> io::Result<()> {
    for _ in 0..64 {
        drain(
            out,
            &mut result.stdout,
            &mut result.stdout_truncated,
            true,
            output_limit,
        )?;
        drain(
            err,
            &mut result.stderr,
            &mut result.stderr_truncated,
            true,
            output_limit,
        )?;
        if out.is_none() && err.is_none() {
            return Ok(());
        }
    }
    Err(io::ErrorKind::TimedOut.into())
}

pub(super) fn drain(
    reader: &mut Option<ChildOutputReader>,
    kept: &mut Vec<u8>,
    truncated: &mut bool,
    settled: bool,
    output_limit: usize,
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
