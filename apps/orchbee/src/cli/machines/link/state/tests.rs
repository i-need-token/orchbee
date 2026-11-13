//! What the far end sends for each channel when its link ends, whether its input ends, the hub
//! sends `bye`, or the idle limit passes: a channel on which an effect may have started gets no
//! frame that would claim it caused nothing, one on which nothing began may be reset, `bye` comes
//! last, and a routed command that runs is ended. The real channels run in
//! `tests/cli_windows_machine_link.rs`.

use super::{super::channels::deliver, End, Entry, Shared, State, Work};
use orchbee_machine_protocol::{
    ByeReason, Connection, Delivery, Message, PathText, ResetReason, SessionName, Token, link,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Bounds the wait for an ended routed command's worker to settle.
const SETTLE: Duration = Duration::from_secs(10);

/// How the link ends, and the `bye` reason the far end gives for it.
const ENDS: [(Ending, ByeReason); 3] = [
    (Ending::Input, ByeReason::Done),
    (Ending::HubBye, ByeReason::Done),
    (Ending::Idle, ByeReason::Idle),
];

#[derive(Clone, Copy, Debug)]
enum Ending {
    Input,
    HubBye,
    Idle,
}

/// One channel in each state the far end can hold when its link ends.
struct Channels {
    /// A `session` whose Session pipe is not open yet.
    connecting: u32,
    /// A `session` accepted and relaying to its Session.
    streaming: u32,
    /// A `route` whose opener has not sent `end`.
    unsent: u32,
    /// A `route` whose command runs.
    running: u32,
    /// An `ensure-session` whose start runs.
    starting: u32,
    /// The running command's cancel flag, which its process tree watches.
    cancel: Arc<AtomicBool>,
}

/// What reached the hub after the link ended.
#[derive(Debug, Default)]
struct Seen {
    resets: BTreeMap<u32, ResetReason>,
    closed: Vec<u32>,
    bye: Option<ByeReason>,
    after_bye: usize,
}

fn facts() -> link::Facts {
    link::Facts {
        platform: Token::new("windows").unwrap(),
        architecture: Token::new("x86_64").unwrap(),
        executable: PathText::new("C:\\scratch\\orchbee.exe").unwrap(),
        store_root: None,
    }
}

fn session(name: &str) -> link::Channel {
    link::Channel::Session {
        session: SessionName::new(name).unwrap(),
    }
}

/// Opens one channel of each state and ends the link with `ending` while the routed command
/// cannot yet answer. The far end's `session` and `ensure-session` workers are not started: the
/// test puts those channels where their workers would, so no Session is reached.
fn end_link(ending: Ending) -> (Connection<link::Link>, Arc<Shared>, Channels, Seen) {
    let far = Arc::new(Shared::new(State::new(Connection::<link::Link>::acceptor(
        facts(),
    ))));
    let mut hub = Connection::<link::Link>::initiator();
    let mut channels = Channels {
        connecting: hub.open(session("alpha")).unwrap(),
        streaming: hub.open(session("beta")).unwrap(),
        unsent: hub.open(link::Channel::Route).unwrap(),
        running: hub.open(link::Channel::Route).unwrap(),
        starting: hub
            .open(link::Channel::EnsureSession {
                session: SessionName::new("gamma").unwrap(),
            })
            .unwrap(),
        cancel: Arc::default(),
    };
    hub.write(channels.unsent, b"{\"version\":1").unwrap();
    // A packet the helper refuses without running anything, so the worker starts no process.
    hub.write(channels.running, b"{}").unwrap();
    hub.end(channels.running).unwrap();
    if matches!(ending, Ending::HubBye) {
        hub.bye(ByeReason::Done).unwrap();
    }

    // The lock is held from the first frame to the end, so the routed command cannot answer first.
    let mut state = far.lock();
    while let Some(frame) = hub.poll_transmit() {
        let delivery = state.connection.receive(&frame).unwrap().delivery;
        match delivery {
            Some(Delivery::Open {
                channel,
                open: link::Channel::Session { .. },
                ..
            }) => {
                state.channels.insert(channel, Entry::new(Work::Session));
                if channel == channels.streaming {
                    state.connection.accept(channel).unwrap();
                    state.channels.begin(channel);
                }
            }
            Some(Delivery::Open {
                channel,
                open: link::Channel::EnsureSession { .. },
                ..
            }) => {
                state.connection.accept(channel).unwrap();
                state.channels.insert(channel, Entry::new(Work::Start));
                state.channels.begin(channel);
            }
            Some(delivery) => deliver(&far, &mut state, delivery),
            None => {}
        }
        if let Some(entry) = state.channels.entries.get(&channels.running) {
            channels.cancel = entry.cancel.clone();
        }
    }
    assert_eq!(state.channels.commands(), 1, "the routed command runs");
    match ending {
        Ending::Input => state.end(End::InputClosed),
        Ending::Idle => state.end(End::Idle),
        Ending::HubBye => {}
    }
    assert!(!state.running(), "{ending:?} ends the link");
    let seen = drain(&mut hub, &mut state);
    drop(state);
    (hub, far, channels, seen)
}

/// Hands everything the far end queued to the hub.
fn drain(hub: &mut Connection<link::Link>, state: &mut State) -> Seen {
    let mut seen = Seen::default();
    while let Some(frame) = state.connection.poll_transmit() {
        if seen.bye.is_some() {
            seen.after_bye += 1;
        }
        match hub.receive(&frame).unwrap().delivery {
            Some(Delivery::Reset {
                channel, reason, ..
            }) => {
                seen.resets.insert(channel, reason);
            }
            Some(Delivery::Close { channel, .. }) => seen.closed.push(channel),
            Some(Delivery::Message(Message::Bye { reason, .. })) => seen.bye = Some(reason),
            _ => {}
        }
    }
    seen
}

#[test]
fn whatever_ends_the_link_only_channels_on_which_nothing_began_are_reset() {
    for (ending, reason) in ENDS {
        let (_hub, _far, channels, seen) = end_link(ending);
        // Nothing reached a Session or ran on these two, so a reset tells the truth.
        let expected = BTreeMap::from([
            (channels.connecting, ResetReason::Stopping),
            (channels.unsent, ResetReason::Stopping),
        ]);
        assert_eq!(seen.resets, expected, "{ending:?}");
        // The begun ones end without a result as the carrier closes, never with a claim.
        for begun in [channels.streaming, channels.running, channels.starting] {
            assert!(!seen.resets.contains_key(&begun), "{ending:?}: {begun}");
        }
        assert!(seen.closed.is_empty(), "{ending:?}: {:?}", seen.closed);
        assert_eq!(seen.bye, Some(reason), "{ending:?}");
        assert_eq!(seen.after_bye, 0, "{ending:?}: `bye` is the last frame");
    }
}

#[test]
fn a_routed_command_running_when_the_link_ends_is_ended_and_answers_nothing() {
    for (ending, _) in ENDS {
        let (mut hub, far, channels, _) = end_link(ending);
        assert!(
            channels.cancel.load(Ordering::Acquire),
            "{ending:?}: the command is told to end"
        );
        let deadline = Instant::now() + SETTLE;
        let mut state = far.lock();
        while state.channels.commands() > 0 && Instant::now() < deadline {
            state = far.wait_until(state, deadline);
        }
        assert_eq!(
            state.channels.commands(),
            0,
            "{ending:?}: the command ended"
        );
        // Its worker settled without a reply, a `close`, or a reset reaching the hub.
        let seen = drain(&mut hub, &mut state);
        assert!(seen.resets.is_empty(), "{ending:?}: {:?}", seen.resets);
        assert!(seen.closed.is_empty(), "{ending:?}: {:?}", seen.closed);
        assert!(
            state.channels.live(channels.running).is_none(),
            "{ending:?}: the channel is over at the far end"
        );
    }
}
