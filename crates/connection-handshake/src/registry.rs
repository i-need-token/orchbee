//! The Rust mirror of the generation registry `schemas/connection/v1/connections.json`, checked
//! by the registry conformance test, and this build's development fingerprints, which `build.rs`
//! computes from the registry's input directories.

use crate::{CapabilityName, CapabilitySet, ContractName, Fingerprint, GenerationRange};

include!(concat!(env!("OUT_DIR"), "/fingerprints.rs"));

/// The capability both connections negotiate before a send may ask for a confirmed idle
/// prompt. A peer without it gets no gated send and no ungated fallback.
pub const PROMPT_GATED_INPUT: &str = "prompt-gated-input";

/// The `worker-host` capability before a Worker may send a `ScreenGatedKeyInput`. A Host without
/// it gets no gated key and no ungated fallback.
pub const SCREEN_GATED_INPUT: &str = "screen-gated-input";

/// The `client-server` capability before a Client may ask physical Explain to place the Agent's
/// input area. A Server without it never receives the flag.
pub const AGENT_INPUT_AREA: &str = "agent-input-area";

/// The `client-server` capability that marks a Session of the first automatic-update release or
/// later, which keeps the compatibility window, so a newer Client may join it. Every build from
/// that release lists it in [`ConnectionKind::capabilities`], and the registry records that
/// release as its `addedIn`.
pub const COMPATIBILITY_WINDOW: &str = "compatibility-window";

/// The `client-server` capability before a Client may read `workbench.terminal-runtime`, the
/// Session's Serving, Reconnecting, or Given-up state while the Server replaces a lost Worker. A
/// Client whose Server lacks it never sends the read.
pub const TERMINAL_RUNTIME: &str = "terminal-runtime";
/// The `client-server` capability before a Client may send `workbench.update-check`, which asks the
/// Session Server to run its automatic-update job now. A Client shows Check for updates as
/// unavailable on a Session without it.
pub const UPDATE_CHECK: &str = "update-check";

/// The `client-server` capability before a Server may close one request stream with the OBWP
/// `OutcomeUnknown` reason, for an intent that failed after it ran or whose result it could not
/// deliver, instead of ending the connection. A Client without it keeps the connection-ending
/// answer it understands.
pub const STREAM_OUTCOME_UNKNOWN: &str = "stream-outcome-unknown";

/// The `client-server` capability before a Server may refuse a configured command with
/// `source-exited`, when its source Panel's terminal has exited or failed. A Client without it
/// reads `unavailable` for that refusal, as its own release did.
pub const CONFIGURED_COMMAND_SOURCE_EXITED: &str = "configured-command-source-exited";

/// The `client-server` capability before a Server may refuse a terminal create with
/// `terminal-limit-reached`, when the Session already retains its bound of terminals. A Client
/// without it reads `server-busy` for that refusal, as its own release did.
pub const TERMINAL_LIMIT_REACHED: &str = "terminal-limit-reached";

/// The `worker-host` capability before a Worker may ask a Host to attach a Keeper and hold its
/// terminals' handles there. No build offers it before the Worker uses the Keeper.
pub const KEEPER: &str = "keeper";

/// The `worker-host` capability before a Worker may ask a Host to quiesce, export, import, or
/// claim a terminal for a Host move. No build offers it before the Worker moves terminals.
pub const HANDOVER: &str = "handover";

/// A connection that starts with an `OBHS` offer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ConnectionKind {
    /// Client → Server's Client endpoint.
    ClientServer,
    /// A Terminal Host or the Worker → the Keeper's endpoint, before `OBKP` frames.
    Keeper,
    /// A TUI, a CLI command, or Desktop's bundled Client → the remote-machine hub.
    MachineHub,
    /// The remote-machine hub → `orchbee --machine-link-v1` on a remote machine, through `ssh`.
    MachineLink,
    /// Worker → Terminal Host's broker endpoint.
    WorkerHost,
}

impl ConnectionKind {
    /// Every negotiated connection the registry declares.
    pub const ALL: [Self; 5] = [
        Self::ClientServer,
        Self::Keeper,
        Self::MachineHub,
        Self::MachineLink,
        Self::WorkerHost,
    ];

    /// The registry and wire name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClientServer => "client-server",
            Self::Keeper => "keeper",
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
            Self::Keeper => const { GenerationRange::declared(1, 1) },
            Self::MachineHub => const { GenerationRange::declared(1, 1) },
            Self::MachineLink => const { GenerationRange::declared(1, 1) },
            Self::WorkerHost => const { GenerationRange::declared(1, 1) },
        }
    }

    /// The capability names this build implements and offers, each one the registry declares.
    #[must_use]
    pub fn capabilities(self) -> CapabilitySet {
        let names: &[&str] = match self {
            Self::ClientServer => &[
                AGENT_INPUT_AREA,
                COMPATIBILITY_WINDOW,
                CONFIGURED_COMMAND_SOURCE_EXITED,
                PROMPT_GATED_INPUT,
                STREAM_OUTCOME_UNKNOWN,
                TERMINAL_LIMIT_REACHED,
                TERMINAL_RUNTIME,
                UPDATE_CHECK,
            ],
            Self::WorkerHost => &[PROMPT_GATED_INPUT, SCREEN_GATED_INPUT],
            Self::Keeper | Self::MachineHub | Self::MachineLink => &[],
        };
        capability_set(names)
    }

    /// Every capability name the registry declares for this connection, including names this
    /// build does not offer yet.
    #[must_use]
    pub fn declared_capabilities(self) -> CapabilitySet {
        match self {
            Self::WorkerHost => {
                capability_set(&[HANDOVER, KEEPER, PROMPT_GATED_INPUT, SCREEN_GATED_INPUT])
            }
            Self::ClientServer | Self::Keeper | Self::MachineHub | Self::MachineLink => {
                self.capabilities()
            }
        }
    }

    /// This build's development protocol fingerprint, which `build.rs` computed from the
    /// registry's input directories for this connection.
    #[must_use]
    pub const fn development_fingerprint(self) -> Fingerprint {
        Fingerprint::from_bytes(match self {
            Self::ClientServer => CLIENT_SERVER_FINGERPRINT,
            Self::Keeper => KEEPER_FINGERPRINT,
            Self::MachineHub => MACHINE_HUB_FINGERPRINT,
            Self::MachineLink => MACHINE_LINK_FINGERPRINT,
            Self::WorkerHost => WORKER_HOST_FINGERPRINT,
        })
    }
}

fn capability_set(names: &[&str]) -> CapabilitySet {
    CapabilitySet::new(names.iter().map(|name| {
        CapabilityName::new(*name).expect("registry capability names follow the name grammar")
    }))
    .expect("the registry lists at most 256 capability names")
}

/// A frame format, which carries its generation in its header.
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
    /// A Windows terminal's handover record, `OBHR`, which a Terminal Host writes and imports.
    Obhr,
}

impl FrameFormat {
    /// Every frame format the registry declares.
    pub const ALL: [Self; 5] = [Self::Obtf, Self::Obpb, Self::Obrw, Self::Obkg, Self::Obhr];

    /// The formats of terminal streams, which every Session process carries.
    pub const STREAMED: [Self; 4] = [Self::Obtf, Self::Obpb, Self::Obrw, Self::Obkg];

    /// The registry name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Obtf => "obtf",
            Self::Obpb => "obpb",
            Self::Obrw => "obrw",
            Self::Obkg => "obkg",
            Self::Obhr => "obhr",
        }
    }

    /// The generations this build supports.
    #[must_use]
    pub const fn supported_generations(self) -> GenerationRange {
        match self {
            Self::Obtf | Self::Obpb | Self::Obrw | Self::Obkg | Self::Obhr => {
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
