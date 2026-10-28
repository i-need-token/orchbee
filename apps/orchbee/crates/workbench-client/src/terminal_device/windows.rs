//! Admits inherited Windows console input and output as the Client's device boundary and
//! exposes bounded reads, mode access, and geometry observations. It does not choose mode
//! lifetime or create a replacement console.

use super::{TerminalAdmissionError, WindowDimensions};
use orchbee_windows_console_host::{ConsoleInput, ConsoleOutput, ConsoleWriter};
use std::{
    fmt,
    io::{self, IsTerminal},
    os::windows::io::AsHandle,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone)]
pub struct InheritedTerminal {
    input: Arc<ConsoleInput>,
    output: Arc<ConsoleOutput>,
    input_reader: Arc<Mutex<crate::windows_console_input::Reader>>,
}

impl fmt::Debug for InheritedTerminal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InheritedTerminal")
            .finish_non_exhaustive()
    }
}

impl InheritedTerminal {
    pub fn admit() -> Result<Self, TerminalAdmissionError> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        if !stdin.is_terminal() || !stdout.is_terminal() {
            return Err(TerminalAdmissionError::NotInteractive);
        }
        let inspect = || -> io::Result<Self> {
            let input = Arc::new(ConsoleInput::admit(
                stdin.as_handle().try_clone_to_owned()?,
            )?);
            let output = Arc::new(ConsoleOutput::admit(
                stdout.as_handle().try_clone_to_owned()?,
            )?);
            let terminal = Self {
                input_reader: Arc::new(Mutex::new(crate::windows_console_input::Reader::new(
                    Arc::clone(&input),
                    Arc::clone(&output),
                ))),
                input,
                output,
            };
            terminal.input_mode()?;
            terminal.output_mode()?;
            terminal.observe_window_dimensions()?;
            Ok(terminal)
        };
        inspect().map_err(TerminalAdmissionError::Inspect)
    }

    pub(crate) fn input_mode(&self) -> io::Result<u32> {
        self.input.mode()
    }

    pub(crate) fn set_input_mode(&self, mode: u32) -> io::Result<()> {
        self.input.set_mode(mode)
    }

    pub(crate) fn output_mode(&self) -> io::Result<u32> {
        self.output.mode()
    }

    pub(crate) fn set_output_mode(&self, mode: u32) -> io::Result<()> {
        self.output.set_mode(mode)
    }

    /// Startup wait, the TUI and direct attach share this admitted handle's
    /// reader state, so a phase transition cannot lose a surrogate or repeat.
    pub fn next_input(&self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.input_reader
            .lock()
            .map_err(|_| io::Error::other("console input reader poisoned"))?
            .next_input(timeout)
    }
    /// Wait on the admitted reader while permitting a worker notification to
    /// interrupt idle input. Buffered input retains its original reader and order.
    pub fn next_input_until(
        &self,
        timeout: Duration,
        interrupted: impl FnMut() -> bool,
    ) -> io::Result<Option<Vec<u8>>> {
        self.input_reader
            .lock()
            .map_err(|_| io::Error::other("console input reader poisoned"))?
            .next_input_until(timeout, interrupted)
    }
    pub fn output(&self) -> io::Result<ConsoleWriter> {
        Ok(self.output.writer())
    }
    pub fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        self.observe_window_dimensions()?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "console reported zero cell dimensions",
            )
        })
    }
    pub fn observe_window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        let window = self.output.window_info()?;
        Ok(
            (window.columns != 0 && window.rows != 0).then_some(WindowDimensions {
                columns: window.columns,
                rows: window.rows,
                pixel_width: 0,
                pixel_height: 0,
            }),
        )
    }
}
