//! How this process ends: the exit code of each outcome, its one `exited` record, and the panic
//! policy that makes every thread one failure domain. The Server counts every end it did not ask
//! for as a crash, so only its drain exits successfully.

use std::{io, process::ExitCode};

use orchbee_files_protocol::DrainReason;
use orchbee_local_runtime::{LogLevel, LogValue, log_event, log_panicked, panic_thread_token};

/// Where a failed process stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stage {
    /// The arguments were not the one invocation the Worker uses.
    Invocation,
    /// Standard input carried no token line.
    Token,
    /// The dial or the same-user proof failed.
    Connect,
    /// The hello or the Server's answer failed.
    Handshake,
    /// The Server refused this build.
    Refused,
    /// The signal handlers could not be installed.
    #[cfg_attr(
        not(unix),
        expect(dead_code, reason = "only macOS and Linux catch termination signals")
    )]
    Signals,
    /// A serving thread could not start.
    Threads,
    /// The link ended without a drain.
    ServerGone,
    /// A frame from the Server did not decode, or reading failed.
    Read,
    /// Writing to the Server failed.
    Write,
}

impl Stage {
    const fn token(self) -> &'static str {
        match self {
            Self::Invocation => "invocation",
            Self::Token => "token",
            Self::Connect => "connect",
            Self::Handshake => "handshake",
            Self::Refused => "refused",
            Self::Signals => "signals",
            Self::Threads => "threads",
            Self::ServerGone => "server-gone",
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// How the process ended.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    /// The Server drained the process.
    Drained(DrainReason),
    /// A termination signal ended the process after its children.
    Signalled(i32),
    /// The process failed at `stage`, with the I/O error kind when one caused it.
    Failed {
        stage: Stage,
        error: Option<io::ErrorKind>,
    },
}

impl Outcome {
    /// The failure at `stage`.
    pub(crate) const fn failed(stage: Stage, error: Option<io::ErrorKind>) -> Self {
        Self::Failed { stage, error }
    }

    /// Success only for a drain; `128 + signal` for a signal, as a shell reports one.
    pub(crate) fn exit_code(&self) -> ExitCode {
        match self {
            Self::Drained(_) => ExitCode::SUCCESS,
            Self::Signalled(signal) => {
                ExitCode::from(u8::try_from(128 + signal).unwrap_or(u8::MAX))
            }
            Self::Failed { .. } => ExitCode::FAILURE,
        }
    }

    /// Writes the process's one `exited` record.
    pub(crate) fn record(&self) {
        match self {
            Self::Drained(reason) => log_event(
                LogLevel::Info,
                "exited",
                &[
                    ("outcome", LogValue::Token("drained")),
                    (
                        "reason",
                        LogValue::Token(match reason {
                            DrainReason::Idle => "idle",
                            DrainReason::Session => "session",
                        }),
                    ),
                ],
            ),
            Self::Signalled(signal) => log_event(
                LogLevel::Info,
                "exited",
                &[
                    ("outcome", LogValue::Token("signalled")),
                    (
                        "signal",
                        LogValue::Count(u64::try_from(*signal).unwrap_or_default()),
                    ),
                ],
            ),
            Self::Failed { stage, error } => {
                let mut fields = vec![
                    ("outcome", LogValue::Token("failed")),
                    ("stage", LogValue::Token(stage.token())),
                ];
                if let Some(error) = error {
                    fields.push(("error", LogValue::from(*error)));
                }
                log_event(LogLevel::Error, "exited", &fields);
            }
        }
    }
}

/// Records that the Server admitted this process.
pub(crate) fn started() {
    log_event(LogLevel::Info, "started", &[]);
}

/// Makes a panic on any thread write one `panicked` record and end the process after the usual
/// report, as a kill would. Unwinding one thread would leave the rest serving wrongly: a reader's
/// panic stops every call's frames while the link stays open, and an operation's leaves its call
/// open for ever. The Server restarts the process on the next demand.
pub(crate) fn end_on_panic() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        let current = std::thread::current();
        log_panicked(
            Some(panic_thread_token(current.name())),
            panic.location(),
            None,
        );
        report(panic);
        std::process::abort();
    }));
}
