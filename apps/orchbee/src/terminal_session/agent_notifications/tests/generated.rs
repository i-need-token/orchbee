//! Replayable semantic inputs and an independent obligation ledger for two Clients.
use super::super::{
    AgentNotificationCommand, AgentNotificationIdentity, AgentNotificationKind,
    AgentNotificationObservation, AgentNotificationPolicy, AgentNotificationScheduler,
    AgentNotificationState, AgentSoundPolicy,
};
use serde_json::Value;
use std::collections::BTreeMap;

const CLIENTS: usize = 2;
const PANES: usize = 2;
const ACTIONS: usize = 14;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Action {
    kind: u8,
    client: u8,
    pane: u8,
    value: u16,
}

impl Action {
    fn new(kind: u8, client: u8, pane: u8, value: u16) -> Self {
        Self {
            kind,
            client,
            pane,
            value,
        }
    }
}

fn serialize(actions: &[Action]) -> String {
    Value::Array(
        actions
            .iter()
            .map(|action| {
                serde_json::json!([action.kind, action.client, action.pane, action.value])
            })
            .collect(),
    )
    .to_string()
}

fn deserialize(encoded: &str) -> Option<Vec<Action>> {
    let value: Value = serde_json::from_str(encoded).ok()?;
    value
        .as_array()?
        .iter()
        .map(|row| {
            let row = row.as_array()?;
            if row.len() != 4 {
                return None;
            }
            let action = Action {
                kind: u8::try_from(row[0].as_u64()?).ok()?,
                client: u8::try_from(row[1].as_u64()?).ok()?,
                pane: u8::try_from(row[2].as_u64()?).ok()?,
                value: u16::try_from(row[3].as_u64()?).ok()?,
            };
            (usize::from(action.kind) < ACTIONS
                && usize::from(action.client) < CLIENTS
                && usize::from(action.pane) < PANES)
                .then_some(action)
        })
        .collect()
}

#[derive(Clone)]
enum Fact {
    Round {
        observations: Vec<AgentNotificationObservation>,
        now: u64,
        draw: u64,
        policy: AgentNotificationPolicy,
    },
    Painted(AgentNotificationIdentity, u64),
    Configuration,
    Gap,
    Retired,
    Delivered(AgentNotificationCommand),
}

struct Obligation {
    command: AgentNotificationCommand,
    due: u64,
    draw: u64,
}

/// Reconstruct obligations from the input transcript, without reading scheduler state
/// or calling its transition/eligibility helpers. A receipt consumes an obligation;
/// configuration, gaps, replacement versions and exact paints revoke it.
fn obligations(facts: &[Fact]) -> Vec<Obligation> {
    let mut preceding: Option<Vec<AgentNotificationObservation>> = None;
    let mut waiting: Vec<Obligation> = Vec::new();
    let mut retired = false;
    for fact in facts {
        if retired {
            continue;
        }
        match fact {
            Fact::Round {
                observations,
                now,
                draw,
                policy,
            } => {
                waiting.retain(|ticket| {
                    observations.iter().any(|current| {
                        current.identity == ticket.command.identity
                            && current.sequence == ticket.command.sequence
                            && !current.seen
                    })
                });
                if let Some(previous) = &preceding {
                    for current in observations {
                        let Some(previous) = previous.iter().find(|previous| {
                            previous.identity == current.identity && previous.agent == current.agent
                        }) else {
                            continue;
                        };
                        if previous.sequence == current.sequence || current.seen {
                            continue;
                        }
                        let kind = match (previous.state, current.state) {
                            (AgentNotificationState::Working, AgentNotificationState::Idle) => {
                                Some(AgentNotificationKind::Done)
                            }
                            (
                                AgentNotificationState::Working | AgentNotificationState::Idle,
                                AgentNotificationState::Blocked,
                            ) => Some(AgentNotificationKind::Request),
                            _ => None,
                        };
                        let (Some(kind), Some(agent)) = (kind, &current.agent) else {
                            continue;
                        };
                        let sound = policy.sound_enabled
                            && policy.agent_sounds.get(agent) != Some(&AgentSoundPolicy::Off);
                        if !policy.popup_enabled && !sound {
                            continue;
                        }
                        waiting.push(Obligation {
                            command: AgentNotificationCommand {
                                identity: current.identity.clone(),
                                sequence: current.sequence,
                                agent: agent.clone(),
                                kind,
                                popup: policy.popup_enabled,
                                sound,
                            },
                            due: now + policy.delay_millis,
                            draw: draw + 1,
                        });
                    }
                }
                preceding = Some(observations.clone());
            }
            Fact::Painted(identity, sequence) => waiting.retain(|ticket| {
                ticket.command.identity != *identity || ticket.command.sequence != *sequence
            }),
            Fact::Configuration => waiting.clear(),
            Fact::Gap => {
                waiting.clear();
                preceding = None;
            }
            Fact::Retired => {
                waiting.clear();
                retired = true;
            }
            Fact::Delivered(command) => waiting.retain(|ticket| ticket.command != *command),
        }
    }
    waiting
}

#[derive(Debug)]
struct Failure {
    class: &'static str,
    client: usize,
    detail: String,
}

#[derive(Default)]
struct Coverage {
    actions: [bool; ACTIONS],
    deliveries: [[bool; 2]; CLIENTS],
}

struct World {
    schedulers: [AgentNotificationScheduler; CLIENTS],
    sources: [AgentNotificationObservation; PANES],
    seen: [[bool; PANES]; CLIENTS],
    generations: [u64; PANES],
    facts: [Vec<Fact>; CLIENTS],
    policy: [AgentNotificationPolicy; CLIENTS],
    revision: [u64; CLIENTS],
    draws: [u64; CLIENTS],
    retired: [bool; CLIENTS],
    receipts: [Vec<AgentNotificationCommand>; CLIENTS],
    now: u64,
    coverage: Coverage,
}

impl World {
    fn new() -> Self {
        Self {
            schedulers: std::array::from_fn(|_| AgentNotificationScheduler::default()),
            sources: std::array::from_fn(|pane| AgentNotificationObservation {
                identity: AgentNotificationIdentity {
                    presentation_id: ["z-old", "a-new"][pane].to_owned(),
                    generation: "binding-0".to_owned(),
                    host_epoch: "epoch-0".to_owned(),
                    host_terminal_id: format!("terminal-{pane}-0"),
                },
                sequence: 1,
                agent: Some("codex".to_owned()),
                state: AgentNotificationState::Unknown,
                seen: false,
            }),
            seen: [[false; PANES]; CLIENTS],
            generations: [0; PANES],
            facts: std::array::from_fn(|_| Vec::new()),
            policy: std::array::from_fn(|_| AgentNotificationPolicy::default()),
            revision: [0; CLIENTS],
            draws: [0; CLIENTS],
            retired: [false; CLIENTS],
            receipts: std::array::from_fn(|_| Vec::new()),
            now: 0,
            coverage: Coverage::default(),
        }
    }

    fn round(&mut self, client: usize) -> Result<(), Failure> {
        let observations: Vec<_> = self
            .sources
            .iter()
            .enumerate()
            .map(|(pane, source)| {
                let mut observation = source.clone();
                observation.seen = self.seen[client][pane];
                observation
            })
            .collect();
        self.schedulers[client]
            .observe(self.now, observations.clone())
            .map_err(|()| Failure {
                class: "valid-round",
                client,
                detail: format!("rejected {observations:?}"),
            })?;
        self.facts[client].push(Fact::Round {
            observations,
            now: self.now,
            draw: self.draws[client],
            policy: self.policy[client].clone(),
        });
        Ok(())
    }

    fn take(&mut self, client: usize, available: bool) -> Result<(), Failure> {
        let waiting = obligations(&self.facts[client]);
        let expected = available
            .then(|| {
                waiting
                    .iter()
                    .filter(|ticket| ticket.due <= self.now && ticket.draw <= self.draws[client])
                    .min_by_key(|ticket| {
                        (
                            ticket.due,
                            ticket.draw,
                            &ticket.command.identity.presentation_id,
                        )
                    })
                    .map(|ticket| ticket.command.clone())
            })
            .flatten();
        let actual = self.schedulers[client].take_ready(self.now, available);
        if actual != expected {
            let class = match (&expected, &actual) {
                (None, Some(_)) => "unexpected-delivery",
                (Some(_), None) => "missing-delivery",
                (Some(expected), Some(actual))
                    if expected.identity != actual.identity
                        || expected.sequence != actual.sequence =>
                {
                    "wrong-version"
                }
                (Some(expected), Some(actual)) if expected.kind != actual.kind => "wrong-kind",
                _ => "wrong-policy",
            };
            return Err(Failure {
                class,
                client,
                detail: format!(
                    "at {} draw {} expected {expected:?}, got {actual:?}",
                    self.now, self.draws[client]
                ),
            });
        }
        if let Some(command) = actual {
            if self.receipts[client].iter().any(|previous| {
                previous.identity == command.identity
                    && previous.sequence == command.sequence
                    && previous.kind == command.kind
            }) {
                return Err(Failure {
                    class: "duplicate",
                    client,
                    detail: format!("{command:?}"),
                });
            }
            let kind = match command.kind {
                AgentNotificationKind::Done => 0,
                AgentNotificationKind::Request => 1,
            };
            self.coverage.deliveries[client][kind] = true;
            self.receipts[client].push(command.clone());
            self.facts[client].push(Fact::Delivered(command));
        }
        Ok(())
    }

    fn apply(&mut self, action: Action) -> Result<(), Failure> {
        let client = usize::from(action.client);
        let pane = usize::from(action.pane);
        self.coverage.actions[usize::from(action.kind)] = true;
        match action.kind {
            0 => {
                self.sources[pane].state = [
                    AgentNotificationState::Unknown,
                    AgentNotificationState::Working,
                    AgentNotificationState::Idle,
                    AgentNotificationState::Blocked,
                ][usize::from(action.value % 4)];
                self.sources[pane].sequence += 1;
                for seen in &mut self.seen {
                    seen[pane] = false;
                }
                self.round(client)?;
            }
            1 => {
                self.generations[pane] += 1;
                let generation = self.generations[pane];
                self.sources[pane].identity.generation = format!("binding-{generation}");
                self.sources[pane].identity.host_epoch = format!("epoch-{generation}");
                self.sources[pane].identity.host_terminal_id =
                    format!("terminal-{pane}-{generation}");
                self.sources[pane].sequence = 1;
                for seen in &mut self.seen {
                    seen[pane] = false;
                }
                self.round(client)?;
            }
            2 => {
                self.schedulers[client].unavailable();
                self.facts[client].push(Fact::Gap);
            }
            3 => self.round(client)?,
            4 | 5 | 6 => {
                let identity = self.sources[pane].identity.clone();
                let sequence = self.sources[pane].sequence;
                if action.kind == 6 {
                    self.schedulers[client].suppress_painted(&identity, sequence - 1);
                    self.facts[client].push(Fact::Painted(identity, sequence - 1));
                } else {
                    if action.kind == 5 {
                        self.schedulers[client].acknowledge(&identity, sequence);
                        self.seen[client][pane] = true;
                    } else {
                        self.schedulers[client].suppress_painted(&identity, sequence);
                    }
                    self.facts[client].push(Fact::Painted(identity, sequence));
                }
            }
            7 => self.now += u64::from(action.value),
            8 => {
                self.schedulers[client].after_draw();
                self.draws[client] += 1;
            }
            9 => {
                self.revision[client] += 1;
                self.policy[client] = AgentNotificationPolicy {
                    delay_millis: [0, 1, 999, 1000, 1001][usize::from(action.value % 5)],
                    popup_enabled: action.value & 8 != 0,
                    sound_enabled: action.value & 16 != 0,
                    agent_sounds: BTreeMap::from([(
                        "codex".to_owned(),
                        if action.value & 32 != 0 {
                            AgentSoundPolicy::Off
                        } else if action.value & 64 != 0 {
                            AgentSoundPolicy::On
                        } else {
                            AgentSoundPolicy::Default
                        },
                    )]),
                };
                self.schedulers[client]
                    .configure(self.revision[client], self.policy[client].clone());
                self.facts[client].push(Fact::Configuration);
            }
            10 => self.take(client, action.value % 2 != 0)?,
            11 => {
                let before = self.schedulers[client].clone();
                let duplicate = self.sources[pane].clone();
                let result =
                    self.schedulers[client].observe(self.now, vec![duplicate.clone(), duplicate]);
                if (!self.retired[client] && result.is_ok()) || self.schedulers[client] != before {
                    return Err(Failure {
                        class: "atomic-invalid",
                        client,
                        detail: "duplicate complete round changed state or was accepted".to_owned(),
                    });
                }
            }
            12 => {
                self.schedulers[client].retire();
                self.retired[client] = true;
                self.facts[client].push(Fact::Retired);
            }
            13 => {
                // A stale complete round is meaningful only after a live baseline.
                let previous = self.facts[client]
                    .iter()
                    .rev()
                    .find_map(|fact| match fact {
                        Fact::Round { observations, .. } => Some(Some(observations.clone())),
                        Fact::Gap | Fact::Retired => Some(None),
                        _ => None,
                    })
                    .flatten();
                if let Some(mut previous) = previous {
                    previous[pane].sequence -= 1;
                    let before = self.schedulers[client].clone();
                    let result = self.schedulers[client].observe(self.now, previous);
                    if result.is_ok() || self.schedulers[client] != before {
                        return Err(Failure {
                            class: "atomic-stale",
                            client,
                            detail: "stale round changed state or was accepted".to_owned(),
                        });
                    }
                }
            }
            _ => unreachable!("validated trace action"),
        }
        Ok(())
    }
}

fn replay(actions: &[Action]) -> Result<Coverage, Failure> {
    let mut world = World::new();
    for action in actions {
        world.apply(*action)?;
    }
    Ok(world.coverage)
}

/// Removing whole semantic inputs preserves valid traces: absent baselines and
/// retired Clients are intentional no-ops, and source versions are reconstructed.
fn shrink(mut actions: Vec<Action>, failure: &Failure) -> Vec<Action> {
    let same_failure = |candidate: &[Action]| {
        replay(candidate)
            .is_err_and(|other| other.class == failure.class && other.client == failure.client)
    };
    let mut width = actions.len() / 2;
    while width > 0 {
        let mut start = 0;
        while start + width <= actions.len() {
            let mut candidate = actions.clone();
            candidate.drain(start..start + width);
            if same_failure(&candidate) {
                actions = candidate;
            } else {
                start += 1;
            }
        }
        width /= 2;
    }
    for index in 0..actions.len() {
        for value in [0, 1, 999, 1000] {
            if value >= actions[index].value {
                continue;
            }
            let mut candidate = actions.clone();
            candidate[index].value = value;
            if same_failure(&candidate) {
                actions = candidate;
            }
        }
    }
    actions
}

fn generated(seed: u64) -> Vec<Action> {
    // Both Clients receive the same physical transitions. A paint in Client 0
    // suppresses only its effect; Client 1 must still deliver that exact version.
    let mut actions = vec![
        Action::new(3, 0, 0, 0),
        Action::new(3, 1, 0, 0),
        Action::new(0, 0, 0, 1),
        Action::new(3, 1, 0, 0),
        Action::new(0, 0, 0, 2),
        Action::new(3, 1, 0, 0),
        Action::new(8, 0, 0, 0),
        Action::new(8, 1, 0, 0),
        Action::new(7, 0, 0, 1000),
        Action::new(4, 0, 0, 0),
        Action::new(3, 0, 0, 0),
        Action::new(10, 0, 0, 1),
        Action::new(10, 1, 0, 1),
        Action::new(0, 0, 1, 1),
        Action::new(3, 1, 0, 0),
        Action::new(0, 0, 1, 3),
        Action::new(3, 1, 0, 0),
        Action::new(8, 0, 0, 0),
        Action::new(8, 1, 0, 0),
        Action::new(10, 0, 0, 0),
        Action::new(7, 0, 0, 999),
        Action::new(10, 0, 0, 1),
        Action::new(7, 0, 0, 1),
        Action::new(10, 0, 0, 1),
        Action::new(10, 1, 0, 1),
        Action::new(6, 0, 1, 0),
        Action::new(9, 0, 0, 24),
        Action::new(0, 0, 0, 1),
        Action::new(0, 0, 0, 2),
        Action::new(8, 0, 0, 0),
        Action::new(5, 0, 0, 0),
        Action::new(3, 0, 0, 0),
        Action::new(2, 0, 0, 0),
        Action::new(3, 0, 0, 0),
        Action::new(1, 0, 0, 0),
        Action::new(11, 0, 0, 0),
        Action::new(13, 0, 0, 0),
    ];
    let mut random = seed;
    for _ in 0..96 {
        random = random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let kind = ((random >> 32) % 13) as u8;
        let kind = if kind == 12 { 13 } else { kind };
        let client = ((random >> 48) % 2) as u8;
        let pane = ((random >> 50) % 2) as u8;
        let value = if kind == 7 {
            [0, 1, 999, 1000, 1001][((random >> 16) % 5) as usize]
        } else {
            ((random >> 16) % 128) as u16
        };
        actions.push(Action::new(kind, client, pane, value));
    }
    actions.push(Action::new(7, 0, 0, 1001));
    for client in 0..2 {
        actions.push(Action::new(8, client, 0, 0));
        for _ in 0..3 {
            actions.push(Action::new(10, client, 0, 1));
        }
        actions.push(Action::new(12, client, 0, 0));
        actions.push(Action::new(0, client, 0, 3));
        actions.push(Action::new(10, client, 0, 1));
    }
    actions
}

#[test]
fn generated_inputs_replay_cover_client_isolation_and_shrink_delivery_failures() {
    for seed in 1..=64 {
        let actions = generated(seed);
        let encoded = serialize(&actions);
        assert_eq!(
            deserialize(&encoded),
            Some(actions.clone()),
            "unstable trace seed={seed}"
        );
        match replay(&actions) {
            Ok(coverage) => {
                assert!(
                    coverage.actions.into_iter().all(|covered| covered),
                    "action coverage seed={seed}"
                );
                assert!(
                    coverage.deliveries[0][1]
                        && coverage.deliveries[1][0]
                        && coverage.deliveries[1][1],
                    "delivery coverage seed={seed}"
                );
            }
            Err(failure) => {
                let minimized = shrink(actions, &failure);
                let replayed = replay(&minimized).err().expect("shrinker lost the failure");
                assert_eq!(
                    (replayed.class, replayed.client),
                    (failure.class, failure.client)
                );
                panic!(
                    "notification-trace seed={seed} class={} client={} detail={} replay={}",
                    replayed.class,
                    replayed.client,
                    replayed.detail,
                    serialize(&minimized)
                );
            }
        }
    }
}
