use super::display::Display;
use super::{
    Action, Connection, ControlInterrupt, Duration, Failure, FailureKind, Gate, Instant,
    LocalClient, Locale, Outcome, Output, STEP, TerminalProjection, Value, ViewerId, classify,
    emit, exchange, history, input, json, query, relay,
};
use crate::{
    client_view::Modifiers,
    invocation::TerminalControlCommand,
    terminal_input::{
        MouseAction, MouseButton, PaneEncode, PaneMouseEvent, encode_cursor_arrows,
        encode_pane_report,
    },
};
use crossterm::event::KeyModifiers;
use orchbee_client_server_protocol::{
    ControlInput, ControllerId, Geometry, TerminalControlAction as ControlAction,
    TerminalControlOutcome as ControlOutcome, TerminalControlRequest,
};
use orchbee_terminal_protocol::{MouseTracking, TerminalTextFrame};
use std::num::NonZeroU64;

pub(crate) fn run(command: &TerminalControlCommand, locale: Locale) -> Output {
    let started = Instant::now();
    let output = |failure: Option<Failure>| Output {
        status: failure.as_ref().map_or(0, |f| f.kind.exit_code()),
        stdout: Vec::new(),
        stderr: failure.map_or_else(String::new, |f| locale.terminal_stream_failure(&f)),
        finalization: None,
    };
    let signal = match ControlInterrupt::arm() {
        Ok(s) => s,
        Err(_) => return output(Some(Failure::new(FailureKind::InternalFailure))),
    };
    let mut relay = match relay::Relay::start() {
        Ok(r) => r,
        Err(_) => return output(Some(Failure::new(FailureKind::InternalFailure))),
    };
    let gate = Gate {
        cancelled: &|| signal.requested(),
        wake: Some(signal.wake()),
        total: command
            .stream
            .timeout_ms
            .map(|ms| started + Duration::from_millis(u64::from(ms))),
    };
    let mut connection = Connection {
        client: None,
        viewer: None,
    };
    let result = control(command, started + STEP, &gate, &mut connection, &mut relay);
    let cleanup = connection.cleanup();
    let (reason, mut failure) = match result {
        Ok(reason) => (reason, None),
        Err(f) => (f.kind.as_str(), Some(f)),
    };
    if signal.requested() {
        failure = Some(Failure::new(FailureKind::Cancelled));
    } else if failure.is_none() && cleanup != "detached" {
        failure = Some(Failure::new(FailureKind::OutcomeUnknown));
    }
    let reason = failure.as_ref().map_or(reason, |f| f.kind.as_str());
    if relay.healthy()
        && let Err(f) = emit(
            &mut relay,
            json!({"kind":"closed","reason":reason,"cleanup":cleanup}),
            Instant::now() + STEP,
            &|| false,
        )
    {
        failure = Some(f);
    }
    drop(relay);
    output(failure)
}

fn control(
    command: &TerminalControlCommand,
    admission: Instant,
    gate: &Gate<'_>,
    connection: &mut Connection,
    relay: &mut relay::Relay,
) -> Result<&'static str, Failure> {
    let deadline = admission.min(gate.step()?);
    let snapshot = query::stream_snapshot(deadline, gate.wake);
    gate.check(deadline)?;
    let (client, snapshot) = snapshot?;
    connection.client = Some(client);
    let client = connection.client.as_mut().expect("admitted");
    let attached = query::attach_viewer_cancellable(
        client,
        deadline,
        &snapshot,
        &command.stream.target,
        gate.wake,
    );
    let (terminal, viewer, attachment, epoch, projection) = match attached {
        Ok(v) => v,
        Err(f) => {
            gate.check(deadline)?;
            return Err(f);
        }
    };
    connection.viewer = Some(viewer.clone());
    gate.check(deadline)?;
    let action = if command.takeover {
        ControlAction::Takeover {
            attachment_id: viewer.clone(),
        }
    } else {
        ControlAction::Acquire {
            attachment_id: viewer.clone(),
        }
    };
    let outcome = submit(client, action, gate)?;
    let ControlOutcome::Acquired { controller_id } = outcome else {
        emit(
            relay,
            json!({"kind":"control","ordinal":"0","operation":"acquire","outcome":outcome}),
            gate.step()?,
            gate.cancelled,
        )?;
        return Err(classify_control(outcome));
    };
    let geometry = Geometry::new(command.stream.columns, command.stream.rows, 0, 0)
        .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
    let mut controller = Controller {
        client,
        viewer: viewer.clone(),
        controller: controller_id.clone(),
        sequence: 0,
        ordinal: 0,
        geometry,
        projection,
        history: history::History::default(),
        gate,
        output: relay,
    };
    controller.resize(geometry)?;
    controller.output.event(
        json!({"kind":"opened","mode":"control","controllerId":controller_id,"source":{
        "sessionId":snapshot.site_id,"revision":snapshot.revision,"terminalId":terminal.terminal_id,"workerId":terminal.worker_id,
        "viewerId":viewer,"attachmentId":attachment.to_text(),"hostEpoch":epoch.to_text()
    },"width":geometry.columns(),"height":geometry.rows()}),
        gate,
    )?;
    let mut input =
        input::Reader::start().map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    controller.drive(&mut input)
}

pub(super) struct Controller<'a, 'g> {
    client: &'a mut LocalClient,
    viewer: ViewerId,
    controller: ControllerId,
    sequence: u64,
    ordinal: u64,
    geometry: Geometry,
    projection: TerminalProjection,
    history: history::History,
    gate: &'a Gate<'g>,
    output: &'a mut dyn Display,
}
impl<'a, 'g> Controller<'a, 'g> {
    pub(super) fn new(
        client: &'a mut LocalClient,
        viewer: ViewerId,
        controller: ControllerId,
        geometry: Geometry,
        projection: TerminalProjection,
        gate: &'a Gate<'g>,
        output: &'a mut dyn Display,
    ) -> Self {
        Self {
            client,
            viewer,
            controller,
            sequence: 0,
            ordinal: 0,
            geometry,
            projection,
            history: history::History::default(),
            gate,
            output,
        }
    }
    pub(super) fn geometry(&self) -> Geometry {
        self.geometry
    }
    pub(super) fn frame(&self) -> Option<&TerminalTextFrame> {
        self.projection.frame()
    }
    pub(super) fn advance_history(&mut self) -> Result<(), Failure> {
        self.history
            .advance(self.client, &self.viewer, self.gate, Some(self.output))
    }
    pub(super) fn notice(&mut self) -> Result<(), Failure> {
        self.output.event(json!({"kind":"notice"}), self.gate)
    }
    pub(super) fn paste(&mut self, text: String) -> Result<(), Failure> {
        self.history.resume(self.client, &self.viewer, self.gate)?;
        if text.is_empty() {
            return Ok(());
        }
        let content = orchbee_client_server_protocol::PasteContent::new(text.into_bytes())
            .map_err(|_| Failure::new(FailureKind::InvalidUsage))?;
        let sequence = self.sequence()?;
        match submit(
            self.client,
            ControlAction::Paste {
                attachment_id: self.viewer.clone(),
                controller_id: self.controller.clone(),
                sequence,
                content,
            },
            self.gate,
        )? {
            ControlOutcome::Pasted { .. } => Ok(()),
            other => Err(classify_control(other)),
        }
    }

    fn sequence(&mut self) -> Result<NonZeroU64, Failure> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?;
        Ok(NonZeroU64::new(self.sequence).expect("incremented"))
    }
    pub(super) fn resize(&mut self, geometry: Geometry) -> Result<(), Failure> {
        let action = ControlAction::Resize {
            attachment_id: self.viewer.clone(),
            controller_id: self.controller.clone(),
            sequence: self.sequence()?,
            geometry,
        };
        match submit(self.client, action, self.gate)? {
            ControlOutcome::Resized { .. } => {
                self.geometry = geometry;
                self.history.resize(geometry.columns(), geometry.rows())
            }
            other => Err(classify_control(other)),
        }
    }
    pub(super) fn input(&mut self, bytes: &[u8]) -> Result<usize, (Failure, usize)> {
        self.history
            .resume(self.client, &self.viewer, self.gate)
            .map_err(|f| (f, 0))?;
        self.write_input(bytes)
    }
    pub(super) fn focus(&mut self, focused: bool) -> Result<(), Failure> {
        if self
            .frame()
            .is_some_and(|frame| frame.interaction.focus_reporting())
        {
            self.write_input(if focused { b"\x1b[I" } else { b"\x1b[O" })
                .map_err(|(failure, _)| failure)?;
        }
        Ok(())
    }
    fn write_input(&mut self, bytes: &[u8]) -> Result<usize, (Failure, usize)> {
        let mut accepted = 0;
        for chunk in bytes.chunks(4096) {
            let sequence = self.sequence().map_err(|f| (f, accepted))?;
            let bytes = ControlInput::new(chunk.to_vec())
                .map_err(|_| (Failure::new(FailureKind::InvalidUsage), accepted))?;
            let action = ControlAction::Input {
                attachment_id: self.viewer.clone(),
                controller_id: self.controller.clone(),
                sequence,
                bytes,
            };
            let outcome = submit(self.client, action, self.gate).map_err(|f| (f, accepted))?;
            if !matches!(outcome, ControlOutcome::InputAccepted {}) {
                return Err((classify_control(outcome), accepted));
            }
            accepted += chunk.len();
        }
        Ok(accepted)
    }
    fn drive(&mut self, input: &mut input::Reader) -> Result<&'static str, Failure> {
        loop {
            self.gate.step()?;
            self.poll()?;
            self.history
                .advance(self.client, &self.viewer, self.gate, Some(self.output))?;
            match input.poll().map_err(|e| {
                Failure::new(if e.kind() == std::io::ErrorKind::InvalidData {
                    FailureKind::InvalidUsage
                } else {
                    FailureKind::InternalFailure
                })
            })? {
                input::Poll::Pending => std::thread::sleep(Duration::from_millis(10)),
                input::Poll::Eof => return Ok("stdin-eof"),
                input::Poll::Line(line) => {
                    if line.iter().all(u8::is_ascii_whitespace) {
                        continue;
                    }
                    self.ordinal = self
                        .ordinal
                        .checked_add(1)
                        .ok_or_else(|| Failure::new(FailureKind::InvalidUsage))?;
                    let command = match input::parse(&line) {
                        Ok(c) => c,
                        Err(reason) => {
                            self.receipt(json!({"kind":"rejected","reason":reason}), "parse")?;
                            continue;
                        }
                    };
                    match command {
                        input::Command::Release => return Ok("released"),
                        input::Command::Input(bytes) => match self.input(&bytes) {
                            Ok(count) => self.receipt(
                                json!({"kind":"input-accepted","acceptedBytes":count}),
                                "input",
                            )?,
                            Err((failure, count)) => {
                                self.receipt(json!({"kind":"failed","reason":failure.kind.as_str(),"acceptedBytes":count,"deliveryMayBeUnknown":true}),"input")?;
                                return Err(failure);
                            }
                        },
                        input::Command::Resize(geometry) => {
                            self.resize(geometry)?;
                            self.receipt(json!({"kind":"resized","geometry":geometry}), "resize")?;
                        }
                        input::Command::Scroll(scroll) => match self.scroll(scroll) {
                            Ok(Some(reason)) => {
                                self.receipt(json!({"kind":"rejected","reason":reason}), "scroll")?
                            }
                            Ok(None) => {
                                self.receipt(json!({"kind":"scroll-accepted"}), "scroll")?
                            }
                            Err((failure, count)) => {
                                self.receipt(json!({"kind":"failed","reason":failure.kind.as_str(),"acceptedBytes":count,"deliveryMayBeUnknown":true}), "scroll")?;
                                return Err(failure);
                            }
                        },
                    }
                }
            }
        }
    }
    fn receipt(&mut self, outcome: Value, operation: &str) -> Result<(), Failure> {
        self.output.event(json!({"kind":"control","ordinal":self.ordinal.to_string(),"operation":operation,"outcome":outcome}), self.gate)
    }

    pub(super) fn poll(&mut self) -> Result<(), Failure> {
        match exchange(
            self.client,
            Action::Poll {
                attachment_id: self.viewer.clone(),
            },
            self.gate,
        )? {
            Outcome::Empty {} => Ok(()),
            Outcome::Record(record) => {
                if self
                    .projection
                    .receive_lifecycle(&record)
                    .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?
                {
                    return Ok(());
                }
                self.history.event(&record, &self.projection)?;
                let feedback = self
                    .projection
                    .receive_for_geometry(&record, None)
                    .map_err(|_| Failure::new(FailureKind::ProtocolInvalid))?;
                let Some(feedback) = feedback else {
                    return self.output.event(
                        json!({"kind":"reset","reason":"projection-blocked"}),
                        self.gate,
                    );
                };
                if matches!(feedback, Action::FrameApplied { .. }) {
                    self.history.frame()?;
                    if self.history.active() {
                        self.history.present(self.output, self.gate)?;
                    } else {
                        self.output.live(
                            self.projection
                                .frame()
                                .ok_or_else(|| Failure::new(FailureKind::ProtocolInvalid))?,
                            self.geometry.columns(),
                            self.geometry.rows(),
                            self.gate,
                        )?;
                    }
                } else if matches!(feedback, Action::NeedSnapshot { .. }) {
                    self.output
                        .event(json!({"kind":"reset","reason":"resynchronize"}), self.gate)?;
                }
                match exchange(self.client, feedback, self.gate)? {
                    Outcome::FeedbackAccepted {} => Ok(()),
                    other => Err(classify(other)),
                }
            }
            other => Err(classify(other)),
        }
    }
    pub(super) fn scroll(
        &mut self,
        scroll: input::Scroll,
    ) -> Result<Option<&'static str>, (Failure, usize)> {
        let Some(frame) = self.projection.frame() else {
            return Ok(Some("frame-unavailable"));
        };
        // The resize receipt can precede its projection. Do not start history
        // or encode mouse coordinates against the superseded dimensions.
        if frame.geometry.columns != self.geometry.columns()
            || frame.geometry.rows != self.geometry.rows()
        {
            return Ok(Some("frame-resizing"));
        }
        match scroll_effect(frame, scroll, self.history.active()) {
            ScrollEffect::History(lines) => {
                self.history
                    .scroll(lines, frame, self.client, &self.viewer, self.gate)
                    .map_err(|f| (f, 0))?;
            }
            ScrollEffect::Input(bytes) => {
                self.input(&bytes)?;
            }
            ScrollEffect::Rejected(reason) => return Ok(Some(reason)),
        }
        Ok(None)
    }
}
#[derive(Debug, Eq, PartialEq)]
enum ScrollEffect {
    History(i64),
    Input(Vec<u8>),
    Rejected(&'static str),
}
fn scroll_effect(
    frame: &TerminalTextFrame,
    scroll: input::Scroll,
    history_active: bool,
) -> ScrollEffect {
    let up = scroll.direction == input::Direction::Up;
    let page = scroll.source == input::ScrollSource::PageKey;
    let untracked = frame.interaction.mouse_tracking() == MouseTracking::None;
    let host_page = !frame.alternate_screen
        && untracked
        && (!frame.interaction.application_cursor_keys() || frame.interaction.bracketed_paste());
    if history_active
        || (if page {
            host_page
        } else {
            untracked && (!frame.alternate_screen || !frame.interaction.alternate_scroll())
        })
    {
        return ScrollEffect::History(if up {
            i64::from(scroll.lines)
        } else {
            -i64::from(scroll.lines)
        });
    }
    if page {
        return ScrollEffect::Input(if up {
            b"\x1b[5~".to_vec()
        } else {
            b"\x1b[6~".to_vec()
        });
    }
    if untracked {
        // Only an alternate screen with DEC 1007 enabled reaches this branch.
        return ScrollEffect::Input(encode_cursor_arrows(
            up,
            1,
            frame.interaction.application_cursor_keys(),
        ));
    }
    let mods = KeyModifiers::from_bits(scroll.modifiers).expect("validated scroll flags");
    let event = PaneMouseEvent {
        action: MouseAction::Press,
        button: Some(if up {
            MouseButton::WheelUp
        } else {
            MouseButton::WheelDown
        }),
        mods: Modifiers {
            shift: mods.contains(KeyModifiers::SHIFT),
            alt: mods.contains(KeyModifiers::ALT),
            ctrl: mods.contains(KeyModifiers::CONTROL),
            super_: mods.contains(KeyModifiers::SUPER),
        },
        column: i32::from(scroll.column.unwrap_or(0)),
        row: i32::from(scroll.row.unwrap_or(0)),
    };
    match encode_pane_report(
        &event,
        frame.interaction.mouse_tracking(),
        frame.interaction.mouse_format(),
        (frame.geometry.columns, frame.geometry.rows),
        false,
        &mut None,
    ) {
        PaneEncode::Bytes(bytes) => ScrollEffect::Input(bytes),
        PaneEncode::Silent => ScrollEffect::Rejected("mouse-report-unavailable"),
    }
}

pub(super) fn submit(
    client: &mut LocalClient,
    action: ControlAction,
    gate: &Gate<'_>,
) -> Result<ControlOutcome, Failure> {
    let deadline = gate.step()?;
    let result = client.submit_terminal_control_cancellable(
        TerminalControlRequest::new(query::obcs_request_id()?, action),
        Some(deadline),
        gate.wake,
    );
    gate.check(deadline)?;
    result
        .map(|r| r.into_outcome())
        .map_err(query::classify_viewer)
}
pub(super) fn classify_control(outcome: ControlOutcome) -> Failure {
    if let ControlOutcome::Rejected { reason } = outcome {
        return Failure::new(FailureKind::Rejected).details(serde_json::Map::from_iter([
            ("stage".into(), json!("terminal-control")),
            ("reason".into(), json!(reason)),
        ]));
    }
    Failure::new(match outcome {
        ControlOutcome::Unavailable { .. } => FailureKind::ServiceUnavailable,
        ControlOutcome::OutcomeUnknown {} => FailureKind::OutcomeUnknown,
        _ => FailureKind::ProtocolInvalid,
    })
}

#[cfg(test)]
mod tests;
