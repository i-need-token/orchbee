//! Owns the Client Connection service's controlled diagnostic exit for internal failures that have
//! no caller left to receive them. A report is one bounded, locale-neutral standard-error line built
//! only from static identifiers, a numeric Connection generation, and an I/O error kind. It never
//! formats record bytes, terminal content, paths, credentials, environment values, or arbitrary
//! error text, and it never changes the failure's external classification or the peer's outcome.

use std::{
    error::Error,
    fmt::Write as _,
    io::{self, Write as _},
    num::NonZeroU64,
};

/// One internal failure: the operation that was running, the stage that failed, and its cause.
pub(super) struct Diagnostic {
    operation: &'static str,
    stage: &'static str,
    cause: Cause,
    connection: Option<NonZeroU64>,
}

impl Diagnostic {
    pub(super) const fn new(operation: &'static str, stage: &'static str) -> Self {
        Self {
            operation,
            stage,
            cause: Cause::None,
            connection: None,
        }
    }

    /// Keeps only the I/O error kind found in the error's source chain; any other cause is a
    /// protocol or contract rejection whose stage already names the protocol.
    pub(super) fn caused_by(mut self, error: &(dyn Error + 'static)) -> Self {
        self.cause = io_kind(error).map_or(Cause::Protocol, Cause::Io);
        self
    }

    pub(super) const fn connection(mut self, generation: NonZeroU64) -> Self {
        self.connection = Some(generation);
        self
    }

    fn line(&self) -> String {
        let mut line = format!(
            "orchbee-server diagnostic component=local-client-service operation={} stage={}",
            self.operation, self.stage
        );
        // Writing into a String cannot fail.
        let _infallible = match self.cause {
            Cause::None => Ok(()),
            Cause::Protocol => write!(line, " cause=protocol"),
            Cause::Io(kind) => write!(line, " cause=io:{kind:?}"),
        };
        if let Some(generation) = self.connection {
            let _infallible = write!(line, " connection={generation}");
        }
        line
    }
}

enum Cause {
    None,
    Protocol,
    Io(io::ErrorKind),
}

/// The first I/O error kind in the chain, including the error itself.
pub(super) fn io_kind(error: &(dyn Error + 'static)) -> Option<io::ErrorKind> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(io) = error.downcast_ref::<io::Error>() {
            return Some(io.kind());
        }
        current = error.source();
    }
    None
}

/// Writes one diagnostic line to standard error.
///
/// Managed launch binds the Server's standard error to the null device, so reports reach a reader
/// only when the Server runs under a development or test harness that owns its standard error.
pub(super) fn report(diagnostic: &Diagnostic) {
    let line = diagnostic.line();
    // A failed diagnostic write has no further owner; the failure it describes already settled
    // its own external outcome.
    let _unobserved = writeln!(io::stderr().lock(), "{line}");
}

#[cfg(test)]
mod tests;
