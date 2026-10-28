//! Admits matching inherited Unix terminal descriptors, duplicates owned input or output
//! handles, and observes their window geometry. It does not change terminal modes or open a
//! replacement terminal device.

use std::{
    io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    sync::Arc,
};

use rustix::{
    fs::fstat,
    termios::{isatty, tcgetwinsize},
};

use super::{TerminalAdmissionError, WindowDimensions};
#[derive(Clone, Debug)]
pub struct InheritedTerminal {
    input: Arc<OwnedFd>,
    output: Arc<OwnedFd>,
}

impl InheritedTerminal {
    pub fn admit() -> Result<Self, TerminalAdmissionError> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        Self::admit_from(stdin.as_fd(), stdout.as_fd())
    }

    pub(super) fn admit_from(
        input: BorrowedFd<'_>,
        output: BorrowedFd<'_>,
    ) -> Result<Self, TerminalAdmissionError> {
        if !isatty(input) || !isatty(output) {
            return Err(TerminalAdmissionError::NotInteractive);
        }

        let stdin_metadata = fstat(input).map_err(io::Error::from);
        let stdout_metadata = fstat(output).map_err(io::Error::from);
        let (stdin_metadata, stdout_metadata) = match (stdin_metadata, stdout_metadata) {
            (Ok(stdin_metadata), Ok(stdout_metadata)) => (stdin_metadata, stdout_metadata),
            (Err(error), _) | (_, Err(error)) => {
                return Err(TerminalAdmissionError::Inspect(error));
            }
        };
        require_same_terminal_identity(
            true,
            true,
            (
                stdin_metadata.st_dev,
                stdin_metadata.st_ino,
                stdin_metadata.st_rdev,
            ),
            (
                stdout_metadata.st_dev,
                stdout_metadata.st_ino,
                stdout_metadata.st_rdev,
            ),
        )?;

        let input = input
            .try_clone_to_owned()
            .map(Arc::new)
            .map_err(TerminalAdmissionError::Inspect)?;
        let output = output
            .try_clone_to_owned()
            .map(Arc::new)
            .map_err(TerminalAdmissionError::Inspect)?;
        let terminal = Self { input, output };
        terminal
            .query_window_dimensions()
            .map_err(TerminalAdmissionError::Inspect)?;
        Ok(terminal)
    }

    pub fn duplicate_input(&self) -> io::Result<OwnedFd> {
        self.input.as_fd().try_clone_to_owned()
    }

    pub fn output(&self) -> io::Result<std::fs::File> {
        self.duplicate_output().map(std::fs::File::from)
    }
    pub(crate) fn duplicate_output(&self) -> io::Result<OwnedFd> {
        self.output.as_fd().try_clone_to_owned()
    }

    pub(crate) const fn input(&self) -> &Arc<OwnedFd> {
        &self.input
    }

    pub fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        let dimensions = self.query_window_dimensions()?;
        if dimensions.columns == 0 || dimensions.rows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the inherited terminal reported zero cell dimensions",
            ));
        }
        Ok(dimensions)
    }

    pub fn observe_window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        let dimensions = self.query_window_dimensions()?;
        if dimensions.columns == 0 || dimensions.rows == 0 {
            return Ok(None);
        }
        Ok(Some(dimensions))
    }

    fn query_window_dimensions(&self) -> io::Result<WindowDimensions> {
        let size = tcgetwinsize(self.output.as_ref()).map_err(io::Error::from)?;
        Ok(WindowDimensions {
            columns: size.ws_col,
            pixel_height: size.ws_ypixel,
            pixel_width: size.ws_xpixel,
            rows: size.ws_row,
        })
    }
}

pub(super) fn require_same_terminal_identity<T: Eq>(
    stdin_is_terminal: bool,
    stdout_is_terminal: bool,
    stdin_identity: T,
    stdout_identity: T,
) -> Result<(), TerminalAdmissionError> {
    if !stdin_is_terminal || !stdout_is_terminal {
        return Err(TerminalAdmissionError::NotInteractive);
    }
    if stdin_identity != stdout_identity {
        return Err(TerminalAdmissionError::DifferentTerminals);
    }
    Ok(())
}

#[cfg(test)]
#[path = "unix/tests.rs"]
mod tests;
