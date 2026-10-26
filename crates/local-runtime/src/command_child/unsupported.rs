//! This host has no registered command Job implementation.
use super::CommandSpawnFailure;
use std::{
    io,
    process::{Command, ExitStatus},
};
pub enum CommandChild {}
impl CommandChild {
    pub fn spawn(_command: Command) -> Result<Self, CommandSpawnFailure> {
        Err(CommandSpawnFailure {
            error: io::ErrorKind::Unsupported.into(),
            created: false,
        })
    }
    pub fn id(&self) -> u32 {
        match *self {}
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Err(io::ErrorKind::Unsupported.into())
    }
    pub fn settle(&mut self) -> io::Result<ExitStatus> {
        Err(io::ErrorKind::Unsupported.into())
    }
}
