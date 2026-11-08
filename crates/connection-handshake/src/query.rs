//! The `query-result` reply: the answering process first, then for a Server its Worker and
//! connected Hosts. One validation owns the cross-member rules for decoding and construction.

use std::num::NonZeroU32;

use crate::{
    Build, CapabilitySet, ContractName, EnvironmentName, Extensions, GenerationRange, InvalidValue,
    ProcessRole, ProcessState, ProductVersion,
};

/// The most processes one reply lists: a Server, its Worker, and 256 Hosts.
pub const MAXIMUM_QUERY_PROCESSES: usize = 258;
/// The most connection entries one process lists.
pub const MAXIMUM_PROCESS_CONNECTIONS: usize = 64;

/// One connection or delivery kind a process takes part in.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionEntry {
    /// The connection or delivery name, possibly one unknown here.
    pub kind: ContractName,
    /// The generations the process supports for it.
    pub generations: GenerationRange,
    /// The capability names, present exactly in the answering process's entries.
    pub capabilities: Option<CapabilitySet>,
}

/// One process a query reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessEntry {
    /// The process role.
    pub role: ProcessRole,
    /// The process's product version.
    pub product_version: ProductVersion,
    /// The process's build identity.
    pub build: Build,
    /// The process's environment.
    pub environment: EnvironmentName,
    /// The operating-system process identifier.
    pub pid: NonZeroU32,
    /// The lifecycle state.
    pub state: ProcessState,
    /// The number of terminals the process holds.
    pub terminals: u32,
    /// At most [`MAXIMUM_PROCESS_CONNECTIONS`] entries in strictly ascending bytewise `kind`
    /// order.
    pub connections: Vec<ConnectionEntry>,
}

/// The reply to `query`; it changes nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    processes: Vec<ProcessEntry>,
    restart_keeps_terminals: Option<bool>,
    extensions: Extensions,
}

impl QueryResult {
    /// A reply listing `processes`, the answering process first.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidValue`] unless 1 to [`MAXIMUM_QUERY_PROCESSES`] processes are listed;
    /// `restart_keeps_terminals` is present exactly when the first process is a Server; only the
    /// first process's connection entries carry capabilities; and each process lists at most
    /// [`MAXIMUM_PROCESS_CONNECTIONS`] connections in strictly ascending `kind` order.
    pub fn new(
        processes: Vec<ProcessEntry>,
        restart_keeps_terminals: Option<bool>,
        extensions: Extensions,
    ) -> Result<Self, InvalidValue> {
        let Some(answering) = processes.first() else {
            return Err(InvalidValue);
        };
        let valid = processes.len() <= MAXIMUM_QUERY_PROCESSES
            && restart_keeps_terminals.is_some() == (answering.role == ProcessRole::Server)
            && processes.iter().enumerate().all(|(index, process)| {
                process.connections.len() <= MAXIMUM_PROCESS_CONNECTIONS
                    && process.connections.windows(2).all(|pair| match pair {
                        [earlier, later] => earlier.kind < later.kind,
                        _ => false,
                    })
                    && process
                        .connections
                        .iter()
                        .all(|connection| connection.capabilities.is_some() == (index == 0))
            });
        if valid {
            Ok(Self {
                processes,
                restart_keeps_terminals,
                extensions,
            })
        } else {
            Err(InvalidValue)
        }
    }

    /// The processes, the answering process first.
    #[must_use]
    pub fn processes(&self) -> &[ProcessEntry] {
        &self.processes
    }

    /// Whether Restart Session keeps terminals; reported only by a Server.
    #[must_use]
    pub fn restart_keeps_terminals(&self) -> Option<bool> {
        self.restart_keeps_terminals
    }

    /// Extension members.
    #[must_use]
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }
}
