//! Defines inherited-terminal admission failures and geometry observations and exposes the
//! selected native device adapter. It does not enter raw mode, take an alternate screen, or
//! decide application startup policy.

use std::{fmt, io};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WindowDimensions {
    pub columns: u16,
    pub pixel_height: u16,
    pub pixel_width: u16,
    pub rows: u16,
}

#[derive(Debug)]
pub enum TerminalAdmissionError {
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
pub use native::InheritedTerminal;
#[cfg(all(test, unix))]
use native::require_same_terminal_identity;
#[cfg(all(test, unix))]
mod tests;

/// Crate-internal `InheritedTerminal` operations the application's tests drive.
#[cfg(all(windows, any(test, feature = "test-support")))]
pub trait InheritedTerminalTestAccess {
    fn input_mode(&self) -> io::Result<u32>;
    fn output_mode(&self) -> io::Result<u32>;
}

#[cfg(all(windows, any(test, feature = "test-support")))]
impl InheritedTerminalTestAccess for InheritedTerminal {
    fn input_mode(&self) -> io::Result<u32> {
        InheritedTerminal::input_mode(self)
    }
    fn output_mode(&self) -> io::Result<u32> {
        InheritedTerminal::output_mode(self)
    }
}
