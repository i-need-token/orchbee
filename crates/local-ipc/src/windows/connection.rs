//! One bounded kernel wait followed by same-user admission under the caller's whole deadline.
use super::{ConnectWaitMode, LocalStream, Pipe, pipe_name};
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

/// Preserves whether a candidate pipe existed; admission failure never authorizes another dial.
#[derive(Debug)]
pub enum ConnectAttemptError {
    Dial(io::Error),
    Admission(io::Error),
}
impl std::fmt::Display for ConnectAttemptError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dial(error) | Self::Admission(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ConnectAttemptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Dial(error) | Self::Admission(error) => Some(error),
        }
    }
}

impl LocalStream {
    /// `None` means only a kernel busy/wait timeout before connection. The short wait limit
    /// never becomes an authentication deadline; an opened pipe is admitted or failed once.
    pub fn connect_before_with_wait_limit(
        path: &Path,
        deadline: Instant,
        wait_limit: Duration,
    ) -> Result<Option<Self>, ConnectAttemptError> {
        attempt(
            deadline,
            wait_limit,
            Instant::now,
            |wait| {
                Pipe::connect_by_path_with_wait_mode(
                    pipe_name(path)?,
                    ConnectWaitMode::Timeout(wait),
                )
            },
            Self::admit,
        )
    }
}

fn attempt<Candidate, Admitted>(
    deadline: Instant,
    wait_limit: Duration,
    mut now: impl FnMut() -> Instant,
    dial: impl FnOnce(Duration) -> io::Result<Candidate>,
    authenticate: impl FnOnce(Candidate) -> io::Result<Admitted>,
) -> Result<Option<Admitted>, ConnectAttemptError> {
    if wait_limit.is_zero() {
        return Err(ConnectAttemptError::Dial(
            io::ErrorKind::InvalidInput.into(),
        ));
    }
    let remaining = deadline
        .checked_duration_since(now())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| ConnectAttemptError::Dial(io::ErrorKind::TimedOut.into()))?;
    let candidate = match dial(remaining.min(wait_limit)) {
        Ok(candidate) => candidate,
        // interprocess can also synthesize a raw-code-free timeout before opening the pipe.
        Err(error)
            if matches!(error.raw_os_error(), Some(121 | 231))
                || error.kind() == io::ErrorKind::TimedOut =>
        {
            return Ok(None);
        }
        Err(error) => return Err(ConnectAttemptError::Dial(error)),
    };
    let admitted = authenticate(candidate).map_err(ConnectAttemptError::Admission)?;
    if now() >= deadline {
        return Err(ConnectAttemptError::Admission(
            io::ErrorKind::TimedOut.into(),
        ));
    }
    Ok(Some(admitted))
}

#[cfg(test)]
mod tests;
