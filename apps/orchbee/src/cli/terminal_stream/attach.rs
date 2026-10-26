//! Direct Windows interface over the same exact controller and viewport owners.
use super::{
    Connection, ControlInterrupt, Duration, Failure, FailureKind, Gate, Instant, Locale, Output,
    STEP, Value, frame, query,
};
use super::{
    control::{Controller, classify_control, submit},
    display::Display,
};
use crate::{
    invocation::{AttachTarget, TerminalAttachCommand},
    terminal_device::{InheritedTerminal, TerminalAdmissionError},
    terminal_input::{
        MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, encode_pane_report,
    },
    terminal_lifecycle::{InheritedTerminalModes, TerminalModeGuard},
};
use orchbee_client_server_protocol::{Geometry, TerminalControlAction, TerminalControlOutcome};
use orchbee_terminal_protocol::{MouseTracking, TerminalTextFrame};
use std::io::{self, Write};
mod input;
#[cfg(test)]
mod tests;

pub(crate) fn run(command: &TerminalAttachCommand, locale: Locale) -> Output {
    let finish = |failure: Option<Failure>| Output {
        status: failure.as_ref().map_or(0, |f| f.kind.exit_code()),
        stdout: Vec::new(),
        stderr: failure.map_or_else(String::new, |f| locale.terminal_stream_failure(&f)),
        finalization: None,
    };
    let inherited = match InheritedTerminal::admit() {
        Ok(terminal) => terminal,
        Err(TerminalAdmissionError::NotInteractive) => {
            return Output {
                status: 2,
                stdout: Vec::new(),
                stderr: locale.attach_requires_terminal().to_owned(),
                finalization: None,
            };
        }
        Err(_) => return finish(Some(Failure::new(FailureKind::InternalFailure))),
    };
    let started = Instant::now();
    let signal = match ControlInterrupt::arm() {
        Ok(signal) => signal,
        Err(_) => return finish(Some(Failure::new(FailureKind::InternalFailure))),
    };
    let gate = Gate {
        cancelled: &|| signal.requested(),
        wake: Some(signal.wake()),
        total: command
            .timeout_ms
            .map(|ms| started + Duration::from_millis(u64::from(ms))),
    };
    let mut connection = Connection {
        client: None,
        viewer: None,
    };
    let mut modes = None;
    let result = connect(
        command,
        locale,
        &inherited,
        &gate,
        started + STEP,
        &mut connection,
        &mut modes,
    );
    let cleanup = connection.cleanup();
    let restored = modes.as_mut().is_none_or(|mode| mode.restore().is_ok());
    drop(modes);
    let mut failure = result.err();
    if !restored {
        failure = Some(Failure::new(FailureKind::InternalFailure));
    } else if signal.requested() {
        failure = Some(Failure::new(FailureKind::Cancelled));
    } else if failure.is_none() && cleanup != "detached" {
        failure = Some(Failure::new(FailureKind::OutcomeUnknown));
    }
    finish(failure)
}

fn connect(
    command: &TerminalAttachCommand,
    locale: Locale,
    inherited: &InheritedTerminal,
    gate: &Gate<'_>,
    admission: Instant,
    connection: &mut Connection,
    modes: &mut Option<TerminalModeGuard<InheritedTerminalModes>>,
) -> Result<(), Failure> {
    let geometry = geometry(inherited)?;
    let deadline = admission.min(gate.step()?);
    let snapshot = query::stream_snapshot(deadline, gate.wake);
    gate.check(deadline)?;
    let (client, snapshot) = snapshot?;
    connection.client = Some(client);
    let client = connection.client.as_mut().expect("admitted");
    let target = match &command.target {
        AttachTarget::Terminal(target) => target.clone(),
        AttachTarget::Agent(target) => {
            query::named_terminal_target(client, &snapshot, target, deadline, gate.wake, false)?
        }
    };
    gate.check(deadline)?;
    let attached =
        query::attach_viewer_cancellable(client, deadline, &snapshot, &target, gate.wake);
    let (_, viewer, _, _, projection) = match attached {
        Ok(attached) => attached,
        Err(failure) => {
            gate.check(deadline)?;
            return Err(failure);
        }
    };
    connection.viewer = Some(viewer.clone());
    gate.check(deadline)?;
    let action = if command.takeover {
        TerminalControlAction::Takeover {
            attachment_id: viewer.clone(),
        }
    } else {
        TerminalControlAction::Acquire {
            attachment_id: viewer.clone(),
        }
    };
    let grant = submit(client, action, gate)?;
    let TerminalControlOutcome::Acquired { controller_id } = grant else {
        return Err(classify_control(grant));
    };
    let writer = inherited.output().map_err(io_failure)?;
    let mut display = ConsoleDisplay {
        writer,
        locale,
        columns: geometry.columns(),
    };
    let mut controller = Controller::new(
        client,
        viewer,
        controller_id,
        geometry,
        projection,
        gate,
        &mut display,
    );
    controller.resize(geometry)?;
    *modes = Some(
        TerminalModeGuard::enter(InheritedTerminalModes::new(inherited.clone()))
            .map_err(io_failure)?,
    );
    let modes = modes.as_mut().expect("owned modes");
    modes
        .set_pointer_capture(command.mouse)
        .map_err(io_failure)?;
    modes.set_bracketed_paste(true).map_err(io_failure)?;
    modes.set_focus_reporting(true).map_err(io_failure)?;
    run_console(&mut controller, inherited, gate, command.mouse)
}
fn geometry(terminal: &InheritedTerminal) -> Result<Geometry, Failure> {
    let size = terminal.window_dimensions().map_err(io_failure)?;
    Geometry::new(size.columns, size.rows, 0, 0)
        .map_err(|_| Failure::new(FailureKind::InvalidUsage))
}
fn run_console(
    controller: &mut Controller<'_, '_>,
    inherited: &InheritedTerminal,
    gate: &Gate<'_>,
    mouse: bool,
) -> Result<(), Failure> {
    let mut input = input::Input::new(mouse);
    let mut pointer = Pointer::default();
    loop {
        gate.step()?;
        if let Some(size) = inherited.observe_window_dimensions().map_err(io_failure)? {
            let size = Geometry::new(size.columns, size.rows, 0, 0)
                .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
            if size != controller.geometry() {
                controller.resize(size)?;
                pointer.last_cell = None;
            }
        }
        let size = controller.geometry();
        controller.poll()?;
        controller.advance_history()?;
        let application_cursor = controller
            .frame()
            .is_some_and(|frame| frame.interaction.application_cursor_keys());
        let actions = match inherited
            .next_input(Duration::from_millis(10))
            .map_err(io_failure)?
        {
            Some(bytes) => input.read(&bytes, Instant::now(), size.rows(), application_cursor),
            None => input.tick(Instant::now(), size.rows(), application_cursor),
        };
        for action in actions {
            match action {
                input::Action::Detach => return Ok(()),
                input::Action::Input(bytes) => {
                    controller.input(&bytes).map_err(|(failure, _)| failure)?;
                }
                input::Action::Paste(text) => controller.paste(text)?,
                input::Action::Scroll(scroll) => {
                    if controller
                        .scroll(scroll)
                        .map_err(|(failure, _)| failure)?
                        .is_some()
                    {
                        controller.notice()?;
                    }
                }
                input::Action::Mouse(mouse) => {
                    match pointer.encode(mouse, controller.frame(), size) {
                        PaneEncode::Bytes(bytes) => {
                            controller.input(&bytes).map_err(|(failure, _)| failure)?;
                        }
                        PaneEncode::Silent => {}
                    }
                }
                input::Action::Focus(focused) => controller.focus(focused)?,
                input::Action::Notice => controller.notice()?,
            }
        }
    }
}

#[derive(Default)]
struct Pointer {
    pressed: u8,
    last_cell: Option<(u16, u16)>,
    contract: Option<(MouseTracking, orchbee_terminal_protocol::MouseFormat)>,
}
impl Pointer {
    fn encode(
        &mut self,
        mouse: MouseEvent,
        frame: Option<&TerminalTextFrame>,
        geometry: Geometry,
    ) -> PaneEncode {
        let bit = match mouse.button {
            MouseButton::Left => 1,
            MouseButton::Middle => 2,
            MouseButton::Right => 4,
            _ => 0,
        };
        if mouse.action == MouseAction::Press {
            self.pressed |= bit;
        }
        let held = self.pressed != 0;
        if mouse.action == MouseAction::Release {
            self.pressed &= !bit;
        }
        let Some(frame) = frame else {
            return PaneEncode::Silent;
        };
        if frame.geometry.columns != geometry.columns() || frame.geometry.rows != geometry.rows() {
            return PaneEncode::Silent;
        }
        let contract = (
            frame.interaction.mouse_tracking(),
            frame.interaction.mouse_format(),
        );
        if contract.0 == MouseTracking::None {
            self.contract = None;
            self.last_cell = None;
            return PaneEncode::Silent;
        }
        if self.contract != Some(contract) {
            self.last_cell = None;
            self.contract = Some(contract);
        }
        encode_pane_report(
            &PaneMouseEvent {
                action: mouse.action,
                button: Some(mouse.button),
                mods: mouse.mods,
                column: i32::from(mouse.column),
                row: i32::from(mouse.row),
            },
            contract.0,
            contract.1,
            (geometry.columns(), geometry.rows()),
            held,
            &mut self.last_cell,
        )
    }
}

struct ConsoleDisplay<W> {
    writer: W,
    locale: Locale,
    columns: u16,
}
impl<W: Write> ConsoleDisplay<W> {
    fn write(&mut self, bytes: &[u8], gate: &Gate<'_>) -> Result<(), Failure> {
        let deadline = gate.step()?;
        self.writer
            .write_all(bytes)
            .and_then(|()| self.writer.flush())
            .map_err(io_failure)?;
        gate.check(deadline)
    }
}
impl<W: Write> Display for ConsoleDisplay<W> {
    fn event(&mut self, record: Value, gate: &Gate<'_>) -> Result<(), Failure> {
        if record["kind"] == "reset" {
            return self.write(b"\x1b[0m\x1b[?25l\x1b[2J\x1b[H", gate);
        }
        if record["kind"] == "notice" {
            let mut text = String::from("\x1b7\x1b[0m\x1b[H\x1b[2K");
            let mut columns = 0;
            for ch in self.locale.attach_input_unavailable().chars() {
                columns += ratatui::text::Span::raw(ch.to_string()).width();
                if columns >= usize::from(self.columns) {
                    break;
                }
                text.push(ch);
            }
            text.push_str("\x1b8");
            return self.write(text.as_bytes(), gate);
        }
        Err(Failure::new(FailureKind::ProtocolInvalid))
    }
    fn live(
        &mut self,
        frame: &TerminalTextFrame,
        columns: u16,
        rows: u16,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        self.columns = columns;
        self.write(&frame::render(frame, columns, rows)?, gate)
    }
    fn history(
        &mut self,
        window: &crate::workbench_client::terminal_viewport::ViewportWindow,
        columns: u16,
        rows: u16,
        gate: &Gate<'_>,
    ) -> Result<(), Failure> {
        self.columns = columns;
        self.write(&frame::render_history(window, columns, rows)?, gate)
    }
}
fn io_failure(_: io::Error) -> Failure {
    Failure::new(FailureKind::InternalFailure)
}
