//! Controlled native-ConPTY witness for the outer pointer-admission lifecycle.

use crate::{
    client_view::{KeyCode, Modifiers},
    terminal_device::InheritedTerminal,
    terminal_input::{
        DecodedItem, DecodedKind, LegacyDecoder, MouseAction, MouseButton, MouseEvent,
    },
    terminal_lifecycle::{InheritedTerminalModes, PointerTrackingLevel, TerminalModeGuard},
};
use orchbee_workbench_client::test_support::client_view::KeyEvent;
use orchbee_workbench_client::test_support::terminal_device::InheritedTerminalTestAccess;
use orchbee_workbench_client::test_support::terminal_events::keys::KeyEventTestAccess;
use orchbee_workbench_client::test_support::terminal_input::HostReply as DecodedHostReply;
use orchbee_workbench_client::test_support::terminal_lifecycle::TerminalModeGuardTestAccess;
use orchbee_workbench_client::test_support::terminal_pointer_admission::Capability;
use orchbee_workbench_client::test_support::terminal_pointer_admission::HostReply as PointerHostReply;
use orchbee_workbench_client::test_support::terminal_pointer_admission::ModeReportState;
use orchbee_workbench_client::test_support::terminal_pointer_admission::Probe;
use std::{
    collections::VecDeque,
    error::Error,
    ffi::OsStr,
    io::{self, Write},
    time::{Duration, Instant},
};

const CHILD_ENVIRONMENT: &str = "ORCHBEE_CONPTY_POINTER_WITNESS_CHILD";
const READ_SLICE: Duration = Duration::from_millis(10);
const PHASE_TIMEOUT: Duration = Duration::from_secs(20);

struct DecodedInput {
    decoder: LegacyDecoder,
    pending: VecDeque<DecodedItem>,
}

impl DecodedInput {
    fn new() -> Self {
        Self {
            decoder: LegacyDecoder::default(),
            pending: VecDeque::new(),
        }
    }

    fn next_until(
        &mut self,
        terminal: &InheritedTerminal,
        deadline: Instant,
    ) -> io::Result<Option<DecodedItem>> {
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Ok(Some(item));
            }

            let now = Instant::now();
            if now >= deadline {
                return Ok(None);
            }
            let timeout = deadline.saturating_duration_since(now).min(READ_SLICE);
            if let Some(bytes) = terminal.next_input(timeout)? {
                for byte in bytes {
                    self.pending
                        .extend(self.decoder.push(byte, Instant::now(), false));
                }
            }
            self.pending.extend(self.decoder.tick(Instant::now()));
        }
    }
}

fn witness_error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn next_required(
    input: &mut DecodedInput,
    terminal: &InheritedTerminal,
    deadline: Instant,
    phase: &str,
) -> io::Result<DecodedItem> {
    input
        .next_until(terminal, deadline)?
        .ok_or_else(|| witness_error(format!("timed out waiting for {phase}")))
}

fn expect_key(
    input: &mut DecodedInput,
    terminal: &InheritedTerminal,
    expected: char,
    phase: &str,
) -> io::Result<()> {
    let item = next_required(input, terminal, Instant::now() + PHASE_TIMEOUT, phase)?;
    let expected = DecodedKind::Key(KeyEvent::press(KeyCode::Char(expected), Modifiers::NONE));
    if item.owned || item.kind != expected {
        return Err(witness_error(format!(
            "expected unowned {phase}, decoded {:?}",
            item.kind
        )));
    }
    Ok(())
}

fn expect_no_button_motion(
    input: &mut DecodedInput,
    terminal: &InheritedTerminal,
) -> io::Result<()> {
    let item = next_required(
        input,
        terminal,
        Instant::now() + PHASE_TIMEOUT,
        "typed no-button motion",
    )?;
    let expected = DecodedKind::Mouse(MouseEvent {
        action: MouseAction::Motion,
        button: MouseButton::None,
        mods: Modifiers::NONE,
        column: 0,
        row: 0,
    });
    if item.owned || item.kind != expected {
        return Err(witness_error(format!(
            "expected unowned typed no-button motion, decoded {:?}",
            item.kind
        )));
    }
    Ok(())
}

fn admit_any_motion(
    input: &mut DecodedInput,
    terminal: &InheritedTerminal,
    guard: &TerminalModeGuard<InheritedTerminalModes>,
) -> io::Result<()> {
    let mut probe = Probe::default();
    let query = probe
        .begin(Instant::now())
        .ok_or_else(|| witness_error("fresh pointer probe refused its first query"))?;
    guard.query_pointer_admission(query)?;

    let sentinel_deadline = Instant::now() + PHASE_TIMEOUT;
    let mut reset_observed = false;
    loop {
        if !reset_observed && let Some(capability) = probe.tick(Instant::now()) {
            return Err(witness_error(format!(
                "pointer admission settled before a mode-1003 reset: {capability:?}"
            )));
        }

        let read_deadline = (Instant::now() + READ_SLICE).min(sentinel_deadline);
        let Some(item) = input.next_until(terminal, read_deadline)? else {
            if Instant::now() >= sentinel_deadline {
                return Err(witness_error(
                    "timed out waiting for ordered mode-1003 reply and DA1 sentinel",
                ));
            }
            continue;
        };
        if item.owned {
            return Err(witness_error(
                "pointer admission metadata unexpectedly belonged to captured input",
            ));
        }
        match item.kind {
            DecodedKind::HostReply(DecodedHostReply::Pointer(PointerHostReply::Mode1003(
                ModeReportState::Reset,
            ))) if !reset_observed => {
                let settled = probe.observe(
                    PointerHostReply::Mode1003(ModeReportState::Reset),
                    Instant::now(),
                );
                if settled != Some(Capability::Admitted) {
                    return Err(witness_error(format!(
                        "mode-1003 reset did not admit any-motion tracking: {settled:?}"
                    )));
                }
                reset_observed = true;
            }
            DecodedKind::HostReply(DecodedHostReply::Pointer(
                PointerHostReply::PrimaryDeviceAttributes,
            )) if reset_observed => {
                if probe
                    .observe(PointerHostReply::PrimaryDeviceAttributes, Instant::now())
                    .is_some()
                {
                    return Err(witness_error(
                        "DA1 sentinel changed an already settled pointer probe",
                    ));
                }
                break;
            }
            other => {
                return Err(witness_error(format!(
                    "unexpected decoded item in pointer admission order: {other:?}"
                )));
            }
        }
    }

    if probe.capability() != Some(Capability::Admitted) || !probe.any_motion_admitted() {
        return Err(witness_error(
            "settled pointer probe did not retain admitted capability",
        ));
    }
    if probe.begin(Instant::now()).is_some() {
        return Err(witness_error(
            "settled pointer probe incorrectly allowed a second query",
        ));
    }
    Ok(())
}

fn commit_phase(
    terminal: &InheritedTerminal,
    guard: &TerminalModeGuard<InheritedTerminalModes>,
    marker: &str,
) -> io::Result<()> {
    // Text identifies the test phase; the lifecycle-owned bell is only the
    // host synchronization fence after preceding mode writes have flushed.
    let mut output = terminal.output()?;
    output.write_all(marker.as_bytes())?;
    output.flush()?;
    guard.ring_bell()
}

fn commit_restored(terminal: &InheritedTerminal) -> io::Result<()> {
    // The restored guard correctly refuses further control writes. This
    // test-only bell lets the host observe that the preceding restore and
    // printable marker reached the primary screen; it is no capability proof.
    let mut output = terminal.output()?;
    output.write_all(b"OBPW:RESTORED|\x07")?;
    output.flush()
}

#[test]
#[ignore = "run by the native Windows x64 ConPTY pointer witness"]
fn inherited_terminal_pointer_admission() -> Result<(), Box<dyn Error>> {
    if std::env::var_os(CHILD_ENVIRONMENT).as_deref() != Some(OsStr::new("1")) {
        return Ok(());
    }

    let terminal = InheritedTerminal::admit()?;
    let original_input_mode = terminal.input_mode()?;
    let original_output_mode = terminal.output_mode()?;
    let mut guard = TerminalModeGuard::enter(InheritedTerminalModes::new(terminal.clone()))?;
    let mut input = DecodedInput::new();

    guard.set_pointer_capture_level(Some(PointerTrackingLevel::ButtonOnly))?;
    commit_phase(&terminal, &guard, "OBPW:BUTTON|")?;
    expect_key(
        &mut input,
        &terminal,
        'b',
        "button-only phase acknowledgement",
    )?;

    // A capturing menu can request motion without a useful DECRQM reply, retaining button
    // tracking for a host that ignores mode 1003. This phase uses no admission reply.
    guard.set_pointer_tracking(PointerTrackingLevel::MenuMotion)?;
    commit_phase(&terminal, &guard, "OBPW:MENU|")?;
    expect_no_button_motion(&mut input, &terminal)?;
    guard.set_pointer_tracking(PointerTrackingLevel::ButtonOnly)?;
    commit_phase(&terminal, &guard, "OBPW:BUTTON-RETURN|")?;
    expect_key(&mut input, &terminal, 'r', "menu return acknowledgement")?;

    admit_any_motion(&mut input, &terminal, &guard)?;
    guard.set_pointer_capture_level(Some(PointerTrackingLevel::AnyMotion))?;
    commit_phase(&terminal, &guard, "OBPW:ANY|")?;
    expect_no_button_motion(&mut input, &terminal)?;

    guard.set_pointer_capture_level(None)?;
    commit_phase(&terminal, &guard, "OBPW:RELEASED|")?;
    expect_key(&mut input, &terminal, 'q', "released phase acknowledgement")?;

    guard.restore()?;
    let restored_input_mode = terminal.input_mode()?;
    let restored_output_mode = terminal.output_mode()?;
    if restored_input_mode != original_input_mode || restored_output_mode != original_output_mode {
        return Err(witness_error(format!(
            "inherited console modes were not restored: input {original_input_mode:#010x} -> {restored_input_mode:#010x}, output {original_output_mode:#010x} -> {restored_output_mode:#010x}"
        ))
        .into());
    }

    commit_restored(&terminal)?;
    Ok(())
}
