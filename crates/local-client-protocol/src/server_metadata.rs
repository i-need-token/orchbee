//! Bounded diagnostic data; feature bits never grant application capabilities.
use crate::{ClientHello, ProtocolError};
use std::num::{NonZeroU8, NonZeroU32};

pub const SERVER_FEATURE_RENDER_ROWS: u32 = 1;
pub const SERVER_FEATURE_CLIENT_VIEW: u32 = 2;
pub const SERVER_FEATURE_AGENT_OBSERVATION: u32 = 4;
pub const SERVER_FEATURE_LIVE_HANDOFF: u32 = 8;
pub const SERVER_KNOWN_FEATURES: u32 = SERVER_FEATURE_RENDER_ROWS
    | SERVER_FEATURE_CLIENT_VIEW
    | SERVER_FEATURE_AGENT_OBSERVATION
    | SERVER_FEATURE_LIVE_HANDOFF;
pub const MAXIMUM_SERVER_VERSION_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerMode {
    Unmanaged,
    ManagedReady,
    ManagedStopping,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerMetadata {
    version: Box<str>,
    cohort: ClientHello,
    process_id: NonZeroU32,
    terminal_protocol: NonZeroU8,
    mode: ServerMode,
    features: u32,
}

impl ServerMetadata {
    pub fn new(
        version: &str,
        cohort: ClientHello,
        process_id: NonZeroU32,
        terminal_protocol: NonZeroU8,
        mode: ServerMode,
        features: u32,
    ) -> Result<Self, ProtocolError> {
        if version.is_empty()
            || version.len() > MAXIMUM_SERVER_VERSION_BYTES
            || !version.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ProtocolError::InvalidPayload);
        }
        Ok(Self {
            version: version.into(),
            cohort,
            process_id,
            terminal_protocol,
            mode,
            features,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn cohort(&self) -> &str {
        self.cohort.cohort()
    }
    pub fn process_id(&self) -> NonZeroU32 {
        self.process_id
    }
    pub fn terminal_protocol(&self) -> NonZeroU8 {
        self.terminal_protocol
    }
    pub fn mode(&self) -> ServerMode {
        self.mode
    }
    /// Unknown bits are retained as diagnostic data for newer Server implementations.
    pub fn features(&self) -> u32 {
        self.features
    }
}

pub(super) fn encode(metadata: &ServerMetadata) -> Vec<u8> {
    let mut bytes = vec![7, metadata.version.len() as u8];
    bytes.extend_from_slice(metadata.version.as_bytes());
    bytes.push(metadata.cohort().len() as u8);
    bytes.extend_from_slice(metadata.cohort().as_bytes());
    bytes.extend_from_slice(&metadata.process_id.get().to_be_bytes());
    bytes.push(metadata.terminal_protocol.get());
    bytes.push(match metadata.mode {
        ServerMode::Unmanaged => 0,
        ServerMode::ManagedReady => 1,
        ServerMode::ManagedStopping => 2,
    });
    bytes.extend_from_slice(&metadata.features.to_be_bytes());
    bytes
}

pub(super) fn decode(bytes: &[u8]) -> Result<ServerMetadata, ProtocolError> {
    let mut remaining = bytes;
    if take(&mut remaining, 1)? != [7] {
        return Err(ProtocolError::InvalidPayload);
    }
    let version = token(&mut remaining)?;
    let cohort = ClientHello::new(token(&mut remaining)?)?;
    let process_id = NonZeroU32::new(u32::from_be_bytes(
        take(&mut remaining, 4)?
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    ))
    .ok_or(ProtocolError::InvalidPayload)?;
    let terminal_protocol =
        NonZeroU8::new(take(&mut remaining, 1)?[0]).ok_or(ProtocolError::InvalidPayload)?;
    let mode = match take(&mut remaining, 1)?[0] {
        0 => ServerMode::Unmanaged,
        1 => ServerMode::ManagedReady,
        2 => ServerMode::ManagedStopping,
        _ => return Err(ProtocolError::InvalidPayload),
    };
    let features = u32::from_be_bytes(
        take(&mut remaining, 4)?
            .try_into()
            .map_err(|_| ProtocolError::InvalidPayload)?,
    );
    if !remaining.is_empty() {
        return Err(ProtocolError::InvalidPayload);
    }
    ServerMetadata::new(
        version,
        cohort,
        process_id,
        terminal_protocol,
        mode,
        features,
    )
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], ProtocolError> {
    let (head, tail) = bytes
        .split_at_checked(count)
        .ok_or(ProtocolError::InvalidPayload)?;
    *bytes = tail;
    Ok(head)
}

fn token<'a>(bytes: &mut &'a [u8]) -> Result<&'a str, ProtocolError> {
    let count = usize::from(take(bytes, 1)?[0]);
    std::str::from_utf8(take(bytes, count)?).map_err(|_| ProtocolError::InvalidPayload)
}

#[cfg(test)]
mod tests;
