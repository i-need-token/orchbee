//! Defines inherited-terminal admission failures and geometry observations and exposes the
//! selected native device adapter. It does not enter raw mode, take an alternate screen, or
//! decide application startup policy.

use std::{fmt, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowDimensions {
    pub(crate) columns: u16,
    pub(crate) pixel_height: u16,
    pub(crate) pixel_width: u16,
    pub(crate) rows: u16,
}

#[derive(Debug)]
pub(crate) enum TerminalAdmissionError {
    #[cfg(unix)]
    DifferentTerminals,
    Inspect(io::Error),
    NotInteractive,
}

impl fmt::Display for TerminalAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(unix)]
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
            #[cfg(unix)]
            Self::DifferentTerminals => None,
            Self::NotInteractive => None,
        }
    }
}

#[cfg(unix)]
#[path = "terminal_device/unix.rs"]
mod native;
#[cfg(windows)]
#[path = "terminal_device/windows.rs"]
mod native;
pub(crate) use native::InheritedTerminal;
#[cfg(all(test, unix))]
use native::require_same_terminal_identity;
#[cfg(all(test, unix))]
mod tests;
