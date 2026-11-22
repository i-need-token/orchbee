//! Notifications for the Agents on the machines other than the one on screen, which the Session on
//! screen cannot observe. They follow the local policy, delay and transitions, read from every
//! connected machine's Agent rows, whose seen state this Client's attention projection keeps; a
//! machine that comes into view is a fresh baseline, so a switch raises nothing. Opening one chooses
//! that Agent as its sidebar row does.

use super::{Choice, Picker, Selection, SelectionOrigin};
use crate::agent_notifications::{
    AgentNotificationIdentity, AgentNotificationKind, AgentNotificationObservation,
    AgentNotificationPolicy, AgentNotificationScheduler, AgentNotificationState,
};
use crate::localization::Locale;
use crate::machine_agents::{Target, checkpoint::Observation};
use crate::startup_directory::ClientStartupTarget;
use orchbee_local_runtime::{NotificationDelivery, NotificationSettings};
use orchbee_workbench_protocol::{Notification, NotificationSound};
use std::time::Instant;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RemoteNotifications {
    scheduler: AgentNotificationScheduler,
    delivery: NotificationDelivery,
    /// The scheduler's logical time zero: the first observation.
    origin: Option<Instant>,
    /// The Agent rows and the machine on screen the scheduler last observed.
    observed: Option<(u64, Choice)>,
}

/// One Agent's identity among every machine's: the machine, then its Presentation.
fn qualified(choice: &Choice, member: &str) -> String {
    match choice {
        Choice::Local => format!("local:{member}"),
        Choice::Remote(id) => format!("{}:{member}", id.as_str()),
    }
}

impl RemoteNotifications {
    fn millis(&self, now: Instant) -> Option<u64> {
        now.checked_duration_since(self.origin?)?
            .as_millis()
            .try_into()
            .ok()
    }
}

impl Picker {
    /// Every Agent row of the connected machines other than the one on screen.
    fn remote_agent_rows(
        &self,
    ) -> impl Iterator<Item = (&Choice, &crate::machine_agents::checkpoint::Row)> {
        self.agent_observations
            .iter()
            .filter(|(choice, _)| *choice != self.current)
            .filter_map(|(choice, observation)| match observation {
                Observation::Ready(round) => Some((choice, round)),
                _ => None,
            })
            .flat_map(|(choice, round)| round.rows.iter().map(move |row| (choice, row)))
    }

    /// The Session a connected machine's Agents were last read from.
    pub fn agent_session(&self, choice: &Choice) -> Option<orchbee_workbench_protocol::SessionId> {
        self.agent_observations
            .iter()
            .find(|(id, _)| id == choice)
            .and_then(|(_, observation)| match observation {
                Observation::Ready(round) => Some(round.session_id.clone()),
                _ => None,
            })
    }

    /// Takes the notification profile and the other machines' Agent rows as they stand at `now`;
    /// true when what may notify changed.
    pub fn observe_remote_notifications(
        &mut self,
        settings: &NotificationSettings,
        now: Instant,
    ) -> bool {
        self.remote_notifications.origin.get_or_insert(now);
        let Some(now_millis) = self.remote_notifications.millis(now) else {
            return false;
        };
        self.remote_notifications.delivery = settings.agent_delivery;
        let changed = self
            .remote_notifications
            .scheduler
            .configure(0, AgentNotificationPolicy::of(settings));
        let observed = Some((self.agent_revision, self.current.clone()));
        if self.remote_notifications.observed == observed {
            return changed;
        }
        self.remote_notifications.observed = observed;
        let observations = self
            .remote_agent_rows()
            .map(|(choice, row)| AgentNotificationObservation {
                identity: AgentNotificationIdentity {
                    presentation_id: qualified(choice, row.target.member.as_str()),
                    generation: row.target.physical.generation.as_str().to_owned(),
                    host_epoch: row.target.physical.host_epoch.as_str().to_owned(),
                    host_terminal_id: row.target.physical.host_terminal_id.as_str().to_owned(),
                },
                sequence: row.sequence.value(),
                agent: Some(row.target.agent.clone()),
                state: AgentNotificationState::of(row.state),
                seen: row.seen,
            })
            .collect();
        let scheduler = &mut self.remote_notifications.scheduler;
        // A round the scheduler refuses is a gap: the next one is a fresh baseline.
        let observed = scheduler
            .observe(now_millis, observations)
            .unwrap_or_else(|()| scheduler.unavailable());
        // No frame acknowledges another machine's Agent, so its notifications wait only for the
        // delay, never for a frame.
        scheduler.after_draw();
        changed | observed
    }

    /// The next due notification about an Agent on another machine, worded as a local one, with
    /// the machine's name beside its Workspace as the sidebar writes it.
    pub(crate) fn take_remote_notification(
        &mut self,
        locale: Locale,
        sink_available: bool,
        now: Instant,
    ) -> Option<(NotificationDelivery, Notification, Option<Target>)> {
        let now_millis = self.remote_notifications.millis(now)?;
        let command = self
            .remote_notifications
            .scheduler
            .take_ready(now_millis, sink_available)?;
        let (choice, row) = self.remote_agent_rows().find(|(choice, row)| {
            qualified(choice, row.target.member.as_str()) == command.identity.presentation_id
        })?;
        let workspace = match choice {
            Choice::Local => row.workspace.clone(),
            Choice::Remote(id) => format!("{} @{}", row.workspace, self.machine_label(id.as_str())),
        };
        let title = locale.agent_notification_title(
            &command.agent,
            matches!(command.kind, AgentNotificationKind::Request),
        );
        let body = locale.agent_notification_context(
            Some(&workspace),
            None,
            row.tab_label.as_deref(),
            &row.label,
        );
        let sound = match (command.sound, command.kind) {
            (false, _) => NotificationSound::None,
            (true, AgentNotificationKind::Done) => NotificationSound::Done,
            (true, AgentNotificationKind::Request) => NotificationSound::Request,
        };
        let message = Notification::normalize(&title, Some(&body), None, sound).ok()?;
        Some((
            self.remote_notifications.delivery,
            message,
            Some(row.target.clone()),
        ))
    }

    /// Chooses the Agent a notification announced when it is on another machine, as its sidebar
    /// row does: `Some(true)` once chosen, `Some(false)` while another choice is under way, and
    /// `None` when the Agent is on the machine on screen or no longer listed, which the Session on
    /// screen answers.
    pub(crate) fn open_remote_agent(&mut self, target: &Target) -> Option<bool> {
        let (choice, _) = self.remote_agent_rows().find(|(choice, row)| {
            row.target == *target
                && self
                    .rows
                    .iter()
                    .any(|machine| machine.enabled && machine.choice == **choice)
        })?;
        let selection = Selection {
            machine: choice.clone(),
            context: Some(ClientStartupTarget::Agent(Box::new(target.clone()))),
        };
        if !self.begin_selection(SelectionOrigin::Sidebar) {
            return Some(false);
        }
        self.request = Some(selection.clone());
        self.note_choice(&selection);
        self.error = None;
        Some(true)
    }
}
