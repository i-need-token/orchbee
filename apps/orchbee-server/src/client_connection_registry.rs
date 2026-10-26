use std::{collections::BTreeSet, num::NonZeroU64};

pub(crate) const MAXIMUM_ACTIVE_CLIENT_CONNECTIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientConnectionAdmission {
    Accepted { generation: NonZeroU64 },
    CapacityExceeded,
    GenerationExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientConnectionRegistry {
    active_generations: BTreeSet<NonZeroU64>,
    next_generation: Option<NonZeroU64>,
}

impl ClientConnectionRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            active_generations: BTreeSet::new(),
            next_generation: Some(NonZeroU64::MIN),
        }
    }

    pub(crate) fn admit(&mut self) -> ClientConnectionAdmission {
        if self.active_generations.len() >= MAXIMUM_ACTIVE_CLIENT_CONNECTIONS {
            return ClientConnectionAdmission::CapacityExceeded;
        }
        let Some(generation) = self.next_generation else {
            return ClientConnectionAdmission::GenerationExhausted;
        };
        let inserted = self.active_generations.insert(generation);
        debug_assert!(inserted, "Client Connection generation must not repeat");
        self.next_generation = generation.get().checked_add(1).and_then(NonZeroU64::new);
        ClientConnectionAdmission::Accepted { generation }
    }

    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) -> bool {
        self.active_generations.remove(&generation)
    }
}

#[cfg(test)]
mod tests;
