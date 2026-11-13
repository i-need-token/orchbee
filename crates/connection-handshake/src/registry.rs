//! The Rust mirror of the generation registry `schemas/connection/v1/connections.json`, checked
//! by the registry conformance test, and this build's development fingerprints, which `build.rs`
//! computes from the registry's input directories.

use crate::{CapabilityName, CapabilitySet, ContractName, Fingerprint, GenerationRange};

include!(concat!(env!("OUT_DIR"), "/fingerprints.rs"));

/// The capability both connections negotiate before a send may ask for a confirmed idle
/// prompt. A peer without it gets no gated send and no ungated fallback.
pub const PROMPT_GATED_INPUT: &str = "prompt-gated-input";

/// The `client-server` capability before a Client may ask physical Explain to place the Agent's
/// input area. A Server without it never receives the flag.
pub const AGENT_INPUT_AREA: &str = "agent-input-area";

/// A connection that starts with an `OBHS` offer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConnectionKind {
    /// Client → Server's Client endpoint.
    ClientServer,
    /// A TUI, a CLI command, or Desktop's bundled Client → the remote-machine hub.
    MachineHub,
    /// The remote-machine hub → `orchbee --machine-link-v1` on a remote machine, through `ssh`.
    MachineLink,
    /// Worker → Terminal Host's broker endpoint.
    WorkerHost,
}

impl ConnectionKind {
    /// Every negotiated connection the registry declares.
    pub const ALL: [Self; 4] = [
        Self::ClientServer,
        Self::MachineHub,
        Self::MachineLink,
        Self::WorkerHost,
    ];

    /// The registry and wire name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClientServer => "client-server",
            Self::MachineHub => "machine-hub",
            Self::MachineLink => "machine-link",
            Self::WorkerHost => "worker-host",
        }
    }

    /// The kind `name` denotes, if known.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// The name as an offer's `connection` value.
    #[must_use]
    pub fn contract_name(self) -> ContractName {
        ContractName::new(self.name()).expect("connection kind names follow the name grammar")
    }

    /// The generations this build supports.
    #[must_use]
    pub const fn supported_generations(self) -> GenerationRange {
        match self {
            Self::ClientServer => const { GenerationRange::declared(1, 1) },
            Self::MachineHub => const { GenerationRange::declared(1, 1) },
            Self::MachineLink => const { GenerationRange::declared(1, 1) },
            Self::WorkerHost => const { GenerationRange::declared(1, 1) },
        }
    }

    /// The capability names this build implements.
    #[must_use]
    pub fn capabilities(self) -> CapabilitySet {
        let names: &[&str] = match self {
            Self::ClientServer => &[AGENT_INPUT_AREA, PROMPT_GATED_INPUT],
            Self::WorkerHost => &[PROMPT_GATED_INPUT],
            Self::MachineHub | Self::MachineLink => &[],
        };
        CapabilitySet::new(names.iter().map(|name| {
            CapabilityName::new(*name).expect("registry capability names follow the name grammar")
        }))
        .expect("the registry lists at most 256 capability names")
    }

    /// This build's development protocol fingerprint, which `build.rs` computed from the
    /// registry's input directories for this connection.
    #[must_use]
    pub const fn development_fingerprint(self) -> Fingerprint {
        Fingerprint::from_bytes(match self {
            Self::ClientServer => CLIENT_SERVER_FINGERPRINT,
            Self::MachineHub => MACHINE_HUB_FINGERPRINT,
            Self::MachineLink => MACHINE_LINK_FINGERPRINT,
            Self::WorkerHost => WORKER_HOST_FINGERPRINT,
        })
    }
}

/// A terminal frame format, which carries its generation in its major version byte.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FrameFormat {
    /// Terminal frame, `OBTF`.
    Obtf,
    /// Projection buffer, `OBPB`.
    Obpb,
    /// Row window, `OBRW`.
    Obrw,
    /// Kitty graphics, `OBKG`.
    Obkg,
}

impl FrameFormat {
    /// Every frame format the registry declares.
    pub const ALL: [Self; 4] = [Self::Obtf, Self::Obpb, Self::Obrw, Self::Obkg];

    /// The registry name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Obtf => "obtf",
            Self::Obpb => "obpb",
            Self::Obrw => "obrw",
            Self::Obkg => "obkg",
        }
    }

    /// The generations this build supports.
    #[must_use]
    pub const fn supported_generations(self) -> GenerationRange {
        match self {
            Self::Obtf | Self::Obpb | Self::Obrw | Self::Obkg => {
                const { GenerationRange::declared(1, 1) }
            }
        }
    }
}

/// A delivery whose receiver cannot first announce what it understands.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Delivery {
    /// Agent hook observation to Server.
    AgentHook,
    /// Routed saved-machine command packet and its reply.
    MachineRoute,
}

impl Delivery {
    /// Every delivery the registry declares.
    pub const ALL: [Self; 2] = [Self::AgentHook, Self::MachineRoute];

    /// The registry name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AgentHook => "agent-hook",
            Self::MachineRoute => "machine-route",
        }
    }

    /// The generations this build accepts.
    #[must_use]
    pub const fn supported_generations(self) -> GenerationRange {
        match self {
            Self::AgentHook | Self::MachineRoute => const { GenerationRange::declared(1, 1) },
        }
    }
}
