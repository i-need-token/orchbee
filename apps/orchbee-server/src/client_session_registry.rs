use std::{collections::BTreeSet, num::NonZeroU64};

pub(crate) const MAXIMUM_ACTIVE_CLIENT_SESSIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientSessionAdmission {
    Accepted { generation: NonZeroU64 },
    CapacityExceeded,
    GenerationExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientSessionRegistry {
    active_generations: BTreeSet<NonZeroU64>,
    next_generation: Option<NonZeroU64>,
}

impl ClientSessionRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            active_generations: BTreeSet::new(),
            next_generation: Some(NonZeroU64::MIN),
        }
    }

    pub(crate) fn admit(&mut self) -> ClientSessionAdmission {
        if self.active_generations.len() >= MAXIMUM_ACTIVE_CLIENT_SESSIONS {
            return ClientSessionAdmission::CapacityExceeded;
        }
        let Some(generation) = self.next_generation else {
            return ClientSessionAdmission::GenerationExhausted;
        };
        let inserted = self.active_generations.insert(generation);
        debug_assert!(inserted, "Client Session generation must not repeat");
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        ClientSessionAdmission::Accepted { generation }
    }

    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) -> bool {
        self.active_generations.remove(&generation)
    }
}

#[cfg(test)]
mod tests;
