//! Owns the bounded set of admitted Client connection generations and retires only the exact
//! disconnected generation, with what each connection's Client stated in its offer. It allocates
//! connection identity without authenticating peers or representing product Sessions.

use orchbee_connection_handshake::{CapabilitySet, GenerationRange, Offer, ProductVersion};
use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

pub(crate) const MAXIMUM_ACTIVE_CLIENT_CONNECTIONS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClientConnectionAdmission {
    Accepted { generation: NonZeroU64 },
    CapacityExceeded,
    GenerationExhausted,
}

/// What an admitted Client stated in its offer: its version, its `client-server` generations and
/// every capability it implements, not only those both sides share. A `query` reply lists them, so
/// a restart's requester can tell whether every connected Client can follow the Session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientOffer {
    pub(crate) product_version: ProductVersion,
    pub(crate) generations: GenerationRange,
    pub(crate) capabilities: CapabilitySet,
}

impl ClientOffer {
    pub(crate) fn of(offer: &Offer) -> Self {
        Self {
            product_version: offer.product_version.clone(),
            generations: offer.generations,
            capabilities: offer.capabilities.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClientConnectionRegistry {
    active_generations: BTreeSet<NonZeroU64>,
    next_generation: Option<NonZeroU64>,
    offers: BTreeMap<NonZeroU64, ClientOffer>,
}

impl ClientConnectionRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            active_generations: BTreeSet::new(),
            next_generation: Some(NonZeroU64::MIN),
            offers: BTreeMap::new(),
        }
    }

    /// Whether another connection fits; `admit` still decides, since a concurrent admission may
    /// take the last slot first.
    pub(crate) fn has_capacity(&self) -> bool {
        self.active_generations.len() < MAXIMUM_ACTIVE_CLIENT_CONNECTIONS
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

    /// Keeps what the Client of the admitted connection `generation` offered, until it disconnects.
    pub(crate) fn note_offer(&mut self, generation: NonZeroU64, offer: ClientOffer) {
        if self.active_generations.contains(&generation) {
            self.offers.insert(generation, offer);
        }
    }

    /// What every connected Client offered, in admission order.
    pub(crate) fn offers(&self) -> Vec<ClientOffer> {
        self.offers.values().cloned().collect()
    }

    pub(crate) fn disconnect(&mut self, generation: NonZeroU64) -> bool {
        self.offers.remove(&generation);
        self.active_generations.remove(&generation)
    }
}

#[cfg(test)]
mod tests;
