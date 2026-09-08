use std::{
    fmt, io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
    sync::Arc,
};

use rustix::{
    fs::fstat,
    termios::{isatty, tcgetwinsize},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowDimensions {
    pub(crate) columns: u16,
    pub(crate) pixel_height: u16,
    pub(crate) pixel_width: u16,
    pub(crate) rows: u16,
}

#[derive(Debug)]
pub(crate) enum TerminalAdmissionError {
    DifferentTerminals,
    Inspect(io::Error),
    NotInteractive,
}

impl fmt::Display for TerminalAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DifferentTerminals => {
                formatter.write_str("standard input and output identify different terminals")
            }
            Self::Inspect(_) => {
                formatter.write_str("the inherited terminal could not be inspected")
            }
            Self::NotInteractive => {
                formatter.write_str("standard input and output must both be terminals")
            }
        }
    }
}

impl std::error::Error for TerminalAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inspect(error) => Some(error),
            Self::DifferentTerminals | Self::NotInteractive => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InheritedTerminal {
    input: Arc<OwnedFd>,
    output: Arc<OwnedFd>,
}

impl InheritedTerminal {
    pub(crate) fn admit() -> Result<Self, TerminalAdmissionError> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        Self::admit_from(stdin.as_fd(), stdout.as_fd())
    }

    fn admit_from(
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
            .window_dimensions()
            .map_err(TerminalAdmissionError::Inspect)?;
        Ok(terminal)
    }

    pub(crate) fn duplicate_input(&self) -> io::Result<OwnedFd> {
        self.input.as_fd().try_clone_to_owned()
    }

    pub(crate) fn duplicate_output(&self) -> io::Result<OwnedFd> {
        self.output.as_fd().try_clone_to_owned()
    }

    pub(crate) const fn input(&self) -> &Arc<OwnedFd> {
        &self.input
    }

    pub(crate) fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        let size = tcgetwinsize(self.output.as_ref()).map_err(io::Error::from)?;
        if size.ws_col == 0 || size.ws_row == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the inherited terminal reported zero cell dimensions",
            ));
        }
        Ok(WindowDimensions {
            columns: size.ws_col,
            pixel_height: size.ws_ypixel,
            pixel_width: size.ws_xpixel,
            rows: size.ws_row,
        })
    }
}

fn require_same_terminal_identity<T: Eq>(
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
mod tests;
