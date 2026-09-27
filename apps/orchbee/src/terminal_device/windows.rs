use super::{TerminalAdmissionError, WindowDimensions};
use crossterm_winapi::{ConsoleMode, Handle, HandleType};
use std::{
    io::{self, IsTerminal},
    os::windows::io::{AsHandle, OwnedHandle},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Debug)]
pub(crate) struct InheritedTerminal {
    input: Arc<OwnedHandle>,
    _output: Arc<OwnedHandle>,
    pub(crate) input_mode: ConsoleMode,
    pub(crate) output_mode: ConsoleMode,
    input_reader: Arc<Mutex<crate::windows_console_input::Reader>>,
}
impl InheritedTerminal {
    pub(crate) fn admit() -> Result<Self, TerminalAdmissionError> {
        let stdin = io::stdin();
        let stdout = io::stdout();
        if !stdin.is_terminal() || !stdout.is_terminal() {
            return Err(TerminalAdmissionError::NotInteractive);
        }
        let inspect = || -> io::Result<Self> {
            let terminal = Self {
                input: Arc::new(stdin.as_handle().try_clone_to_owned()?),
                _output: Arc::new(stdout.as_handle().try_clone_to_owned()?),
                input_mode: ConsoleMode::from(Handle::new(HandleType::InputHandle)?),
                output_mode: ConsoleMode::from(Handle::new(HandleType::OutputHandle)?),
                input_reader: Arc::new(Mutex::new(crate::windows_console_input::Reader::new()?)),
            };
            terminal.input_mode.mode()?;
            terminal.output_mode.mode()?;
            terminal.observe_window_dimensions()?;
            Ok(terminal)
        };
        inspect().map_err(TerminalAdmissionError::Inspect)
    }
    pub(crate) fn duplicate_input(&self) -> io::Result<OwnedHandle> {
        self.input.as_handle().try_clone_to_owned()
    }
    /// Startup wait, the TUI and direct attach share this admitted handle's
    /// reader state, so a phase transition cannot lose a surrogate or repeat.
    pub(crate) fn next_input(&self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.input_reader
            .lock()
            .map_err(|_| io::Error::other("console input reader poisoned"))?
            .next_input(timeout)
    }
    // std::io::Stdout writes Unicode through WriteConsoleW on a Windows
    // console; File::write would interpret UTF-8 bytes using the user's OEM CP.
    pub(crate) fn output(&self) -> io::Result<io::Stdout> {
        Ok(io::stdout())
    }
    pub(crate) fn window_dimensions(&self) -> io::Result<WindowDimensions> {
        self.observe_window_dimensions()?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "console reported zero cell dimensions",
            )
        })
    }
    pub(crate) fn observe_window_dimensions(&self) -> io::Result<Option<WindowDimensions>> {
        let (columns, rows) = crossterm::terminal::size()?;
        Ok((columns != 0 && rows != 0).then_some(WindowDimensions {
            columns,
            rows,
            pixel_width: 0,
            pixel_height: 0,
        }))
    }
}
