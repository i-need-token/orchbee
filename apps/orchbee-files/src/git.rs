//! The File Bar's Git engine: repository discovery, status and its refresh schedule, reads,
//! operations and the credential and editor relay, each run through the Git command line.

mod command;
mod consents;
mod discovery;
mod engine;
mod errors;
mod helper;
mod json;
mod mutations;
mod native_paths;
mod output;
mod porcelain;
pub(crate) mod ports;
mod reads;
mod records;
mod refresh;
mod relay;
mod remote;
mod runner;
mod serve;
mod show;
mod status;
mod workspace;

pub(crate) use self::{helper::helper, serve::Service};

#[cfg(test)]
mod tests;
