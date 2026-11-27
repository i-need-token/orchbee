//! The File Bar's Git engine: repository discovery, status and its refresh schedule, reads,
//! operations and the credential and editor relay, each run through the Git command line.

mod command;
mod consents;
mod discovery;
mod errors;
mod json;
mod native_paths;
mod output;
mod porcelain;
pub(crate) mod ports;
mod refresh;
mod runner;
mod status;
mod workspace;

#[cfg(test)]
mod tests;
