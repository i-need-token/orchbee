//! Kept terminal output in the Session folder: one atomically renamed file per number. The Server
//! names the numbers its committed checkpoint uses and removes every other one.
use super::{SessionStore, private_folder, publish_private, remove_if_present};
use crate::{private_filesystem as permissions, terminal_output as output};
use std::{
    fs,
    io::{self, Read},
    num::NonZeroU64,
    path::PathBuf,
};

impl SessionStore {
    fn output_directory(&self) -> PathBuf {
        self.directory.join(output::DIRECTORY)
    }

    /// Every kept token, ascending. An absent folder keeps none.
    pub fn terminal_output_tokens(&self) -> io::Result<Vec<u64>> {
        let entries = match fs::read_dir(self.output_directory()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut tokens = Vec::new();
        for entry in entries {
            if let Some(token) = entry?.file_name().to_str().and_then(output::entry_token) {
                tokens.push(token.get());
            }
        }
        tokens.sort_unstable();
        Ok(tokens)
    }

    /// The payload kept under `token`, or `None` when no readable file has that token.
    pub fn read_terminal_output(&self, token: u64) -> io::Result<Option<Vec<u8>>> {
        let Some(token) = NonZeroU64::new(token) else {
            return Ok(None);
        };
        let path = self.output_directory().join(output::entry_name(token));
        let mut file = match permissions::open_snapshot_file(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(output::FRAME_LIMIT as u64)
            .read_to_end(&mut bytes)?;
        Ok(output::unframe(token, &bytes))
    }

    /// Writes `payload` under `token` through a flushed staging file and an atomic rename. A
    /// failure before the rename leaves every kept file unchanged.
    pub fn write_terminal_output(&mut self, token: u64, payload: &[u8]) -> io::Result<()> {
        let token = NonZeroU64::new(token).ok_or(io::ErrorKind::InvalidInput)?;
        if payload.len() > output::MAXIMUM_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let directory = self.output_directory();
        let _folder = private_folder(&directory)?;
        publish_private(
            &directory.join(output::PENDING),
            &directory.join(output::entry_name(token)),
            &output::frame(token, payload),
            "terminal output",
        )
    }

    pub fn remove_terminal_output(&mut self, token: u64) -> io::Result<()> {
        match NonZeroU64::new(token) {
            Some(token) => {
                remove_if_present(&self.output_directory().join(output::entry_name(token)))
            }
            None => Ok(()),
        }
    }
}
