//! Validated scrollbar observations and the integer cell geometry every Client scrollbar shares.
//! This projection owns no terminal history: all positions come from a frame or served window.

use std::num::NonZeroU64;

use orchbee_terminal_protocol::{AttachmentId, HostEpoch};
use ratatui::layout::Rect;

/// One scrolled range as a scrollbar shows it: `visible` units of `visible + maximum`, starting
/// `start` units from the top. A Panel counts terminal rows and the sidebar its entries; both place
/// their thumb and map a drag through this one geometry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollbarRange {
    visible: u32,
    maximum: u32,
    start: u32,
}

impl ScrollbarRange {
    /// `None` for an empty view or content that fits whole, which shows no scrollbar.
    pub(crate) fn new(visible: u32, maximum: u32, start: u32) -> Option<Self> {
        (visible > 0 && maximum > 0 && start <= maximum).then_some(Self {
            visible,
            maximum,
            start,
        })
    }

    /// The range's position may move during a drag; its extent may not.
    pub(crate) fn same_extent(self, other: Self) -> bool {
        (self.visible, self.maximum) == (other.visible, other.maximum)
    }

    /// The thumb, with at least one cell of travel on a usable track.
    pub(crate) fn thumb(self, track: Rect) -> Option<Rect> {
        if track.width != 1 || track.height < 2 || track.y.checked_add(track.height).is_none() {
            return None;
        }
        let total = u64::from(self.maximum) + u64::from(self.visible);
        let length = (u64::from(self.visible) * u64::from(track.height) / total)
            .clamp(1, u64::from(track.height - 1));
        let length = u16::try_from(length).ok()?;
        let travel = track.height - length;
        let top = (u64::from(self.start) * u64::from(travel) + u64::from(self.maximum) / 2)
            / u64::from(self.maximum);
        Some(Rect::new(
            track.x,
            track.y + u16::try_from(top).ok()?,
            1,
            length,
        ))
    }

    /// The start that puts the thumb, grabbed `grab` cells below its top, at `row`; rows outside
    /// the track clamp to its ends.
    pub(crate) fn start_at(self, track: Rect, row: u16, grab: u16) -> Option<u32> {
        let thumb = self.thumb(track)?;
        let travel = track.height - thumb.height;
        let top = row.saturating_sub(track.y).saturating_sub(grab).min(travel);
        let start =
            (u64::from(top) * u64::from(self.maximum) + u64::from(travel) / 2) / u64::from(travel);
        u32::try_from(start).ok()
    }
}

/// A known, addressable history range from one verified terminal surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScrollbarSnapshot {
    pub(crate) attachment: AttachmentId,
    pub(crate) epoch: HostEpoch,
    pub(crate) generation: NonZeroU64,
    pub(crate) columns: u16,
    pub(crate) rows: u16,
    visible_rows: u16,
    maximum: u32,
    offset: u32,
}

impl ScrollbarSnapshot {
    /// Rejects absent, inconsistent, empty or unaddressable scroll ranges.
    pub(crate) fn new(
        attachment: AttachmentId,
        epoch: HostEpoch,
        generation: NonZeroU64,
        geometry: (u16, u16),
        visible_rows: u16,
        total: u64,
        start: u64,
    ) -> Option<Self> {
        let (columns, rows) = geometry;
        let maximum = u32::try_from(total.checked_sub(u64::from(visible_rows))?).ok()?;
        if columns == 0
            || visible_rows == 0
            || visible_rows > rows
            || maximum == 0
            || start > u64::from(maximum)
        {
            return None;
        }
        Some(Self {
            attachment,
            epoch,
            generation,
            columns,
            rows,
            visible_rows,
            maximum,
            offset: maximum - u32::try_from(start).ok()?,
        })
    }

    /// Row positions may move during a drag; its source and total range may not.
    pub(crate) fn same_source(self, other: Self) -> bool {
        self.attachment == other.attachment
            && self.epoch == other.epoch
            && self.generation == other.generation
            && (self.columns, self.rows) == (other.columns, other.rows)
            && self.maximum == other.maximum
            && self.visible_rows == other.visible_rows
    }

    /// Exact total recorded at press time; changes cancel further drag intent.
    pub(crate) fn total_rows(self) -> u64 {
        u64::from(self.maximum) + u64::from(self.visible_rows)
    }

    /// Served rows, which can be fewer than the physical pane after read halving.
    pub(crate) fn visible_rows(self) -> u16 {
        self.visible_rows
    }

    /// Bounds an absolute tail request to the observed retained range.
    pub(crate) fn clamp(self, offset: u32) -> u32 {
        offset.min(self.maximum)
    }

    /// The observed range from the top: served rows of the retained history.
    pub(crate) fn range(self) -> ScrollbarRange {
        ScrollbarRange {
            visible: u32::from(self.visible_rows),
            maximum: self.maximum,
            start: self.maximum - self.offset,
        }
    }

    /// The observed thumb, with at least one cell of travel on a usable track.
    pub(crate) fn thumb(self, track: Rect) -> Option<Rect> {
        self.range().thumb(track)
    }

    /// Maps a dragged thumb to the frozen range, clamping outside-track motion.
    pub(crate) fn offset_at(self, track: Rect, row: u16, grab: u16) -> Option<u32> {
        Some(self.maximum - self.range().start_at(track, row, grab)?)
    }
}

#[cfg(test)]
mod tests;
