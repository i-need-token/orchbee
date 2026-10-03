//! Bounded plain-text assembly; no partial export can become clipboard input.
use orchbee_terminal_protocol::{
    AttachmentId, MAXIMUM_SELECTION_EXPORT_BYTES, MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES,
    SelectionExportChunk, SelectionExportFinished, SelectionExportStarted, SelectionExportStatus,
};
use std::num::NonZeroU64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Failure {
    Invalid,
    Memory,
}

pub(super) struct Export {
    attachment: AttachmentId,
    sequence: NonZeroU64,
    announced: Option<usize>,
    early: Option<SelectionExportChunk>,
    bytes: Vec<u8>,
    next_chunk: u32,
    awaiting_ack: Option<u32>,
    final_seen: bool,
    finish_seen: bool,
    retired: bool,
}

impl Export {
    pub(super) const fn new(attachment: AttachmentId, sequence: NonZeroU64) -> Self {
        Self {
            attachment,
            sequence,
            announced: None,
            early: None,
            bytes: Vec::new(),
            next_chunk: 0,
            awaiting_ack: None,
            final_seen: false,
            finish_seen: false,
            retired: false,
        }
    }

    pub(super) fn started(
        &mut self,
        value: SelectionExportStarted,
    ) -> Result<Option<u32>, Failure> {
        if self.retired
            || self.announced.is_some()
            || value.attachment_id != self.attachment
            || value.export_sequence != self.sequence
            || value.byte_length as usize > MAXIMUM_SELECTION_EXPORT_BYTES
        {
            return Err(Failure::Invalid);
        }
        self.announced = Some(value.byte_length as usize);
        if let Some(early) = self.early.take() {
            return self.chunk(early);
        }
        Ok(None)
    }

    pub(super) fn chunk(&mut self, value: SelectionExportChunk) -> Result<Option<u32>, Failure> {
        if self.retired
            || value.attachment_id != self.attachment
            || value.export_sequence != self.sequence
            || value.chunk_sequence != self.next_chunk
            || self.awaiting_ack.is_some()
            || self.final_seen
            || value.bytes.len() > MAXIMUM_SELECTION_EXPORT_CHUNK_BYTES
        {
            return Err(Failure::Invalid);
        }
        let Some(total) = self.announced else {
            if self.early.is_some() {
                return Err(Failure::Invalid);
            }
            self.early = Some(value);
            return Ok(None);
        };
        let length = self
            .bytes
            .len()
            .checked_add(value.bytes.len())
            .ok_or(Failure::Invalid)?;
        if length > total || value.final_chunk != (length == total) {
            return Err(Failure::Invalid);
        }
        self.bytes
            .try_reserve(value.bytes.len())
            .map_err(|_| Failure::Memory)?;
        self.bytes.extend_from_slice(&value.bytes);
        self.final_seen = value.final_chunk;
        self.awaiting_ack = Some(value.chunk_sequence);
        Ok(Some(value.chunk_sequence))
    }

    pub(super) fn acknowledged(&mut self, chunk_sequence: u32) -> Result<(), Failure> {
        if self.retired || self.awaiting_ack != Some(chunk_sequence) {
            return Err(Failure::Invalid);
        }
        self.next_chunk = self.next_chunk.checked_add(1).ok_or(Failure::Invalid)?;
        self.awaiting_ack = None;
        Ok(())
    }

    pub(super) fn finished(&mut self, value: SelectionExportFinished) -> Result<(), Failure> {
        if self.retired
            || value.attachment_id != self.attachment
            || value.export_sequence != self.sequence
        {
            return Err(Failure::Invalid);
        }
        if value.status == SelectionExportStatus::Cancelled {
            self.retired = true;
            self.bytes.clear();
            self.early = None;
            return Ok(());
        }
        if self.finish_seen
            || self.announced != Some(value.byte_length as usize)
            || self.bytes.len() != value.byte_length as usize
            || !self.final_seen
        {
            return Err(Failure::Invalid);
        }
        self.finish_seen = true;
        Ok(())
    }

    pub(super) fn take_text(&mut self) -> Result<Option<String>, Failure> {
        if self.retired || !self.finish_seen || self.awaiting_ack.is_some() {
            return Ok(None);
        }
        self.retired = true;
        String::from_utf8(std::mem::take(&mut self.bytes))
            .map(Some)
            .map_err(|_| Failure::Invalid)
    }
}

#[cfg(test)]
mod tests;
