use super::model::{Command, DEADLINE, Delivery, Event, Observation, POLL};
use orchbee_terminal_protocol::TerminalLaunchKind::{self, Cmd, ManagedPowershell, Other};
use orchbee_workbench_protocol::AgentStartFailureReason::{
    self as Fact, Exited, NotFound, PromptUnconfirmed, TerminalTooOld, UnsupportedShell,
};
use std::time::{Duration, Instant};

/// What the terminal shows at `elapsed` from the launch's acceptance, given when the Host typed
/// the line, if it did.
type Script = fn(Duration, Option<Duration>) -> Event;

/// One row of the table: its name, the terminal's script, the answers to its sends, and the run it
/// must come to.
type Case<'a> = (&'a str, Script, &'a [Event], usize, bool, Option<Fact>);

const fn read(kind: Option<TerminalLaunchKind>, idle: bool, provider: bool, prompt: bool) -> Event {
    Event::Read {
        observation: Some(Observation {
            kind,
            idle,
            provider,
            prompt,
        }),
        touched: false,
    }
}

const fn ms(milliseconds: u64) -> Duration {
    Duration::from_millis(milliseconds)
}

/// A shell at its prompt with nothing else running.
const fn at_prompt(kind: TerminalLaunchKind) -> Event {
    read(Some(kind), true, false, true)
}

/// The provider's process runs under the shell.
const fn running(kind: TerminalLaunchKind) -> Event {
    read(Some(kind), false, true, false)
}

#[derive(Debug, Eq, PartialEq)]
struct Run {
    /// Gated sends the candidate asked for.
    sends: usize,
    /// Whether the Host typed the line.
    typed: bool,
    fact: Option<Fact>,
}

/// Drives one candidate as the driver does, on a controlled clock: a read at every instant it
/// asks for, and `answers` in order to its sends.
fn drive(script: Script, answers: &[Event]) -> Run {
    let accepted = Instant::now();
    let mut delivery = Delivery::new(accepted);
    let mut now = accepted;
    let mut typed_at = None;
    let mut sends = 0;
    let mut command = delivery.step(script(Duration::ZERO, None), now);
    for _ in 0..1000 {
        command = match command {
            Command::ReadAt(at) => {
                assert!(at > now, "a read is asked for in the future");
                now = at;
                delivery.step(script(now - accepted, typed_at), now)
            }
            Command::Send => {
                let answer = answers[sends];
                sends += 1;
                if answer == Event::Accepted {
                    typed_at = Some(now - accepted);
                }
                delivery.step(answer, now)
            }
            Command::End(fact) => {
                return Run {
                    sends,
                    typed: delivery.typed(),
                    fact,
                };
            }
        };
    }
    panic!("the candidate never ended");
}

/// Design 2.5: the launch kind and what reads and sends show decide what is typed and which
/// fact, if any, ends the candidate.
#[test]
fn launch_kind_and_reads_decide_the_line_and_the_fact() {
    let refused = Event::Refused { settling: false };
    let settling = Event::Refused { settling: true };
    // One gated send every poll from the launch's acceptance until the deadline.
    let refusals_until_deadline = usize::try_from(DEADLINE.as_millis() / POLL.as_millis()).unwrap();
    let cases: [Case<'_>; 18] = [
        (
            "PowerShell runs the Agent past its watch",
            |t, _| {
                if t < ms(1500) {
                    at_prompt(ManagedPowershell)
                } else {
                    running(ManagedPowershell)
                }
            },
            &[],
            0,
            false,
            None,
        ),
        (
            "PowerShell stays idle with no Agent seen",
            |_, _| at_prompt(ManagedPowershell),
            &[],
            0,
            false,
            Some(NotFound),
        ),
        (
            "PowerShell's Agent exits inside the watch",
            |t, _| {
                if t > ms(1000) && t < ms(4000) {
                    running(ManagedPowershell)
                } else {
                    at_prompt(ManagedPowershell)
                }
            },
            &[],
            0,
            false,
            Some(Exited),
        ),
        (
            "PowerShell busy with something else to the end",
            |t, _| {
                if t.is_zero() {
                    at_prompt(ManagedPowershell)
                } else {
                    read(Some(ManagedPowershell), false, false, false)
                }
            },
            &[],
            0,
            false,
            None,
        ),
        (
            "PowerShell reads fail after the first",
            |t, _| {
                if t.is_zero() {
                    at_prompt(ManagedPowershell)
                } else {
                    Event::Read {
                        observation: None,
                        touched: false,
                    }
                }
            },
            &[],
            0,
            false,
            None,
        ),
        (
            "cmd types once and its Agent runs",
            |t, typed| match typed {
                Some(typed) if t > typed => running(Cmd),
                _ => at_prompt(Cmd),
            },
            &[Event::Accepted],
            1,
            true,
            None,
        ),
        (
            "cmd types once and the command is not found",
            |_, _| at_prompt(Cmd),
            &[Event::Accepted],
            1,
            true,
            Some(NotFound),
        ),
        (
            "cmd types once and the Agent exits",
            |t, typed| match typed {
                Some(typed) if t > typed && t < typed + ms(1000) => running(Cmd),
                _ => at_prompt(Cmd),
            },
            &[Event::Accepted],
            1,
            true,
            Some(Exited),
        ),
        (
            "cmd waits for its prompt, then types",
            |t, typed| match typed {
                Some(typed) if t > typed => running(Cmd),
                _ if t < ms(2000) => read(Some(Cmd), true, false, false),
                _ => at_prompt(Cmd),
            },
            &[Event::Accepted],
            1,
            true,
            None,
        ),
        (
            "cmd's prompt is never confirmed",
            |_, _| read(Some(Cmd), true, false, false),
            &[],
            0,
            false,
            Some(PromptUnconfirmed),
        ),
        (
            "cmd retries refusals that typed nothing, then types once",
            |t, typed| match typed {
                Some(typed) if t > typed => running(Cmd),
                _ => at_prompt(Cmd),
            },
            &[settling, refused, Event::Accepted],
            3,
            true,
            None,
        ),
        (
            "cmd is refused until the deadline",
            |_, _| at_prompt(Cmd),
            &[refused; 200],
            refusals_until_deadline,
            false,
            Some(PromptUnconfirmed),
        ),
        (
            "cmd's Host has no prompt gate",
            |_, _| at_prompt(Cmd),
            &[Event::GateUnsupported],
            1,
            false,
            Some(TerminalTooOld),
        ),
        (
            "cmd's send fails and is never replayed",
            |_, _| at_prompt(Cmd),
            &[Event::Failed],
            1,
            false,
            None,
        ),
        (
            "another shell takes no start command",
            |_, _| at_prompt(Other),
            &[],
            0,
            false,
            Some(UnsupportedShell),
        ),
        (
            "a Host without launch kinds is too old",
            |_, _| read(None, true, false, true),
            &[],
            0,
            false,
            Some(TerminalTooOld),
        ),
        (
            "a terminal never sampled",
            |_, _| Event::Read {
                observation: None,
                touched: false,
            },
            &[],
            0,
            false,
            None,
        ),
        (
            "the Presentation leaves",
            |t, _| {
                if t < ms(1000) {
                    read(Some(Cmd), true, false, false)
                } else {
                    Event::Refuted
                }
            },
            &[],
            0,
            false,
            None,
        ),
    ];
    for (name, script, answers, sends, typed, fact) in cases {
        assert_eq!(drive(script, answers), Run { sends, typed, fact }, "{name}");
    }
}

/// Client input or a shell command that reaches a marked cmd before its line is sent ends the
/// candidate with nothing typed, whether a read or the send's own admission sees it; after the
/// line was typed, or in managed PowerShell, input is the person using the Agent.
#[test]
fn client_input_before_the_send_types_nothing() {
    const fn touched(prompt: bool) -> Event {
        Event::Read {
            observation: Some(Observation {
                kind: Some(Cmd),
                idle: true,
                provider: false,
                prompt,
            }),
            touched: true,
        }
    }
    assert_eq!(
        drive(
            |t, _| if t < ms(500) {
                read(Some(Cmd), true, false, false)
            } else {
                touched(true)
            },
            &[]
        ),
        Run {
            sends: 0,
            typed: false,
            fact: None
        },
        "a read shows the input"
    );
    assert_eq!(
        drive(|_, _| at_prompt(Cmd), &[Event::Refuted]),
        Run {
            sends: 1,
            typed: false,
            fact: None
        },
        "the send's admission sees the input"
    );
    assert_eq!(
        drive(
            |t, typed| match typed {
                Some(typed) if t > typed => Event::Read {
                    observation: Some(Observation {
                        kind: Some(Cmd),
                        idle: false,
                        provider: true,
                        prompt: false,
                    }),
                    touched: true,
                },
                _ => at_prompt(Cmd),
            },
            &[Event::Accepted]
        ),
        Run {
            sends: 1,
            typed: true,
            fact: None
        },
        "input after the line"
    );
    assert_eq!(
        drive(
            |t, _| Event::Read {
                observation: Some(Observation {
                    kind: Some(ManagedPowershell),
                    idle: t.is_zero(),
                    provider: !t.is_zero(),
                    prompt: false,
                }),
                touched: !t.is_zero(),
            },
            &[]
        ),
        Run {
            sends: 0,
            typed: false,
            fact: None
        },
        "input to PowerShell's Agent"
    );
}
