//! Safe ownership and data boundaries for caller-admitted Windows console handles.
//!
//! This crate retains the exact input and output handles supplied by its caller. It does not
//! resolve process-global standard handles, interpret product input semantics, or own terminal
//! lifecycle and restoration policy.

#![deny(unsafe_code)]

#[cfg(windows)]
mod input;
#[cfg(windows)]
mod output;
#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "reviewed Win32 calls and tagged INPUT_RECORD union reads are isolated here"
)]
mod sys;

#[cfg(windows)]
pub use input::{ConsoleInput, InputRecord, InputWait, InputWake, KeyRecord, MouseRecord};
#[cfg(windows)]
pub use output::{
    ConsoleOutput, ConsoleWindowInfo, ConsoleWriteBoundary, ConsoleWriteError,
    ConsoleWriteErrorKind, ConsoleWriteProgress, ConsoleWriter,
};
