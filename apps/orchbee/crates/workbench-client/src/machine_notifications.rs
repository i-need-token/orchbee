//! Explicit notifications from the Sessions of the connected machines other than the one on
//! screen: `orchbee notification show` there reaches this screen as it reaches a Client attached to
//! that Session. One listener per machine, on the machine's own connection, for the Session its
//! Agents were last read from; the machine on screen hands its feed to its Session's Client.

// module-layer: workflow

use crate::client_view::ClientView;
use crate::machine_discovery::Connection;
use crate::machine_observation::Source;
use crate::machines::Choice;
use crate::notifications::subscription::Subscription;
use orchbee_workbench_protocol::SessionId;
use std::time::Instant;

struct Listener {
    choice: Choice,
    session: SessionId,
    connection: Connection,
    subscription: Subscription,
}

#[derive(Default)]
pub struct RemoteNotifications {
    listeners: Vec<Listener>,
}

impl RemoteNotifications {
    /// Keeps one listener on each of `sources` other than `current` whose Session the machine's
    /// Agents were read from, and shows the first notification one of them delivers while
    /// `accepting`; true when one was shown.
    pub fn tick(
        &mut self,
        sources: &[Source],
        current: &Choice,
        view: &mut ClientView,
        accepting: bool,
        now: Instant,
    ) -> bool {
        let session = |choice: &Choice| view.machines.agent_session(choice);
        // A listener ends with its machine's place among the sources, its carrier, or its Session;
        // a Session not known for a moment keeps it.
        self.listeners.retain(|listener| {
            listener.choice != *current
                && session(&listener.choice).is_none_or(|known| known == listener.session)
                && sources.iter().any(|source| {
                    source.choice == listener.choice
                        && source.connection.same_owner(&listener.connection)
                })
        });
        for source in sources {
            if source.choice == *current
                || self
                    .listeners
                    .iter()
                    .any(|listener| listener.choice == source.choice)
            {
                continue;
            }
            let Some(session) = session(&source.choice) else {
                continue;
            };
            if let Ok(subscription) = Subscription::spawn(
                source.connection.origin().clone(),
                source.connection.clock(),
                session.clone(),
            ) {
                self.listeners.push(Listener {
                    choice: source.choice.clone(),
                    session,
                    connection: source.connection.clone(),
                    subscription,
                });
            }
        }
        // Every listener polls, so one that may no longer accept says so; at most one message lands
        // and the others stay with their Servers.
        let mut message = None;
        for listener in &self.listeners {
            let polled = listener.subscription.poll(accepting && message.is_none());
            message = message.or(polled);
        }
        message.is_some_and(|message| view.notifications.install(message, now))
    }
}
