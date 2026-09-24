//! Bounded cell geometry observed by one Client; no canonical topology or view state is mutated.
use super::json_fields::deserialize_nullable;
use crate::{
    ContractError, MAXIMUM_PANEL_NODES_PER_CONTEXT, PanelOrientation, WorkbenchPanelId,
    WorkbenchPanelSlotId, WorkbenchRevision, WorkbenchSplitId, WorkbenchTabId,
    WorkbenchTerminalContextId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeSet;

/// Maximum encoded request or response containing one complete Client layout.
pub const MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES: usize = 1024 * 1024;

/// A finite split proportion. Equality is safe because NaN and signed zero are inadmissible.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientSplitRatio(f64);
impl Eq for ClientSplitRatio {}
impl ClientSplitRatio {
    /// The default equal split, before integer cell rounding.
    pub const HALF: Self = Self(0.5);
    /// Admit the supported finite split range without silently clamping input.
    pub fn new(value: f64) -> Result<Self, ContractError> {
        if value.is_finite() && (0.1..=0.9).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Return the admitted numeric proportion.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}
impl Serialize for ClientSplitRatio {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}
impl<'de> Deserialize<'de> for ClientSplitRatio {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// An integer cell rectangle; sums are validated in widened arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientCellRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}
impl ClientCellRect {
    /// Whether this rectangle contains at least one cell.
    #[must_use]
    pub const fn nonempty(self) -> bool {
        self.width > 0 && self.height > 0
    }
    /// Whether both extents fit the u16 terminal coordinate domain.
    #[must_use]
    pub fn valid(self) -> bool {
        u32::from(self.x) + u32::from(self.width) <= u32::from(u16::MAX)
            && u32::from(self.y) + u32::from(self.height) <= u32::from(u16::MAX)
    }
    fn right(self) -> u32 {
        u32::from(self.x) + u32::from(self.width)
    }
    fn bottom(self) -> u32 {
        u32::from(self.y) + u32::from(self.height)
    }

    /// Containment includes empty rectangles at the containing boundary.
    #[must_use]
    pub fn contains(self, other: Self) -> bool {
        self.valid()
            && other.valid()
            && other.x >= self.x
            && other.y >= self.y
            && u32::from(other.x) + u32::from(other.width)
                <= u32::from(self.x) + u32::from(self.width)
            && u32::from(other.y) + u32::from(other.height)
                <= u32::from(self.y) + u32::from(self.height)
    }
}

/// One canonical leaf, with null geometry for layers that do not place it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPanelArea {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub unzoomed: Option<ClientCellRect>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub terminal: Option<ClientCellRect>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub displayed: Option<ClientCellRect>,
}

/// One canonical split and the geometry the renderer actually resolved for it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientSplitArea {
    pub split_id: WorkbenchSplitId,
    pub first_slot_id: WorkbenchPanelSlotId,
    pub second_slot_id: WorkbenchPanelSlotId,
    pub orientation: PanelOrientation,
    pub ratio: ClientSplitRatio,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub area: Option<ClientCellRect>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub divider: Option<ClientCellRect>,
}

/// Cardinal navigation in terminal cell space.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClientPanelDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Four neighbors of the source; absent neighbors imply edges only for an available source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPanelNeighbors {
    #[serde(deserialize_with = "deserialize_nullable")]
    pub left: Option<WorkbenchPanelId>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub right: Option<WorkbenchPanelId>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub up: Option<WorkbenchPanelId>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub down: Option<WorkbenchPanelId>,
}

impl ClientPanelNeighbors {
    /// Resolve neighbors using outer cells and canonical input order, independent of zoom.
    #[must_use]
    pub fn from_panels(source: &WorkbenchPanelId, panels: &[ClientPanelArea]) -> Self {
        Self {
            left: neighbor(source, panels, ClientPanelDirection::Left),
            right: neighbor(source, panels, ClientPanelDirection::Right),
            up: neighbor(source, panels, ClientPanelDirection::Up),
            down: neighbor(source, panels, ClientPanelDirection::Down),
        }
    }

    /// Read the neighbor for one requested direction.
    #[must_use]
    pub const fn get(&self, direction: ClientPanelDirection) -> Option<&WorkbenchPanelId> {
        match direction {
            ClientPanelDirection::Left => self.left.as_ref(),
            ClientPanelDirection::Right => self.right.as_ref(),
            ClientPanelDirection::Up => self.up.as_ref(),
            ClientPanelDirection::Down => self.down.as_ref(),
        }
    }
}

fn neighbor(
    source: &WorkbenchPanelId,
    panels: &[ClientPanelArea],
    direction: ClientPanelDirection,
) -> Option<WorkbenchPanelId> {
    let from = panels
        .iter()
        .find(|panel| &panel.panel_id == source)?
        .unzoomed?;
    if !from.nonempty() {
        return None;
    }
    panels
        .iter()
        .enumerate()
        .filter_map(|(index, panel)| {
            if &panel.panel_id == source {
                return None;
            }
            let to = panel.unzoomed?;
            if !to.nonempty() {
                return None;
            }
            let (f_start, f_end, t_start, t_end, distance) = match direction {
                ClientPanelDirection::Left if to.right() <= u32::from(from.x) => (
                    u32::from(from.y),
                    from.bottom(),
                    u32::from(to.y),
                    to.bottom(),
                    u32::from(from.x) - to.right(),
                ),
                ClientPanelDirection::Right if u32::from(to.x) >= from.right() => (
                    u32::from(from.y),
                    from.bottom(),
                    u32::from(to.y),
                    to.bottom(),
                    u32::from(to.x) - from.right(),
                ),
                ClientPanelDirection::Up if to.bottom() <= u32::from(from.y) => (
                    u32::from(from.x),
                    from.right(),
                    u32::from(to.x),
                    to.right(),
                    u32::from(from.y) - to.bottom(),
                ),
                ClientPanelDirection::Down if u32::from(to.y) >= from.bottom() => (
                    u32::from(from.x),
                    from.right(),
                    u32::from(to.x),
                    to.right(),
                    u32::from(to.y) - from.bottom(),
                ),
                _ => return None,
            };
            let overlap = f_end.min(t_end).saturating_sub(f_start.max(t_start));
            if overlap == 0 {
                return None;
            }
            let center = (f_start + f_end).abs_diff(t_start + t_end);
            Some((
                (distance, std::cmp::Reverse(overlap), center, index),
                &panel.panel_id,
            ))
        })
        .min_by_key(|(score, _)| *score)
        .map(|(_, panel)| panel.clone())
}

/// A point-in-time target projection. Selection and focus describe the observing Client only.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientPanelLayout {
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub source_panel_id: WorkbenchPanelId,
    pub workbench_revision: WorkbenchRevision,
    pub viewport: ClientCellRect,
    pub area: ClientCellRect,
    pub selected: bool,
    pub zoomed: bool,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub focused_panel_id: Option<WorkbenchPanelId>,
    pub source_available: bool,
    pub neighbors: ClientPanelNeighbors,
    pub panels: Vec<ClientPanelArea>,
    pub splits: Vec<ClientSplitArea>,
}
impl ClientPanelLayout {
    /// Read a physical terminal-area edge; hidden source geometry makes no edge claim.
    #[must_use]
    pub fn at_edge(&self, direction: ClientPanelDirection) -> Option<bool> {
        let source = self
            .panels
            .iter()
            .find(|panel| panel.panel_id == self.source_panel_id)?
            .unzoomed?;
        if !self.source_available || !source.nonempty() {
            return None;
        }
        Some(match direction {
            ClientPanelDirection::Left => source.x <= self.area.x,
            ClientPanelDirection::Right => source.right() >= self.area.right(),
            ClientPanelDirection::Up => source.y <= self.area.y,
            ClientPanelDirection::Down => source.bottom() >= self.area.bottom(),
        })
    }

    /// Validate bounds, geometry containment and all intra-observation identity references.
    pub fn validate(&self) -> Result<(), ContractError> {
        if !self.viewport.nonempty()
            || !self.viewport.contains(self.area)
            || self.panels.is_empty()
            || self.panels.len().saturating_add(self.splits.len()) > MAXIMUM_PANEL_NODES_PER_CONTEXT
            || self.splits.len().saturating_add(1) != self.panels.len()
        {
            return Err(ContractError::InvalidDocument);
        }
        let mut panels = BTreeSet::new();
        for panel in &self.panels {
            if !panels.insert(&panel.panel_id)
                || [panel.unzoomed, panel.terminal, panel.displayed]
                    .into_iter()
                    .flatten()
                    .any(|rect| !rect.nonempty() || !self.area.contains(rect))
                || panel
                    .terminal
                    .is_some_and(|rect| panel.unzoomed.is_none_or(|outer| !outer.contains(rect)))
                || (self.zoomed
                    && panel.displayed.is_some()
                    && self.focused_panel_id.as_ref() != Some(&panel.panel_id))
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        let Some(source) = self
            .panels
            .iter()
            .find(|panel| panel.panel_id == self.source_panel_id)
        else {
            return Err(ContractError::InvalidDocument);
        };
        if self.source_available != source.unzoomed.is_some()
            || self
                .focused_panel_id
                .as_ref()
                .is_some_and(|id| !panels.contains(id))
            || (self.selected && self.focused_panel_id.is_none())
        {
            return Err(ContractError::InvalidDocument);
        }
        if self.neighbors != ClientPanelNeighbors::from_panels(&self.source_panel_id, &self.panels)
        {
            return Err(ContractError::InvalidDocument);
        }
        let neighbors = [
            &self.neighbors.left,
            &self.neighbors.right,
            &self.neighbors.up,
            &self.neighbors.down,
        ];
        for neighbor in neighbors.into_iter().flatten() {
            if !self.source_available
                || neighbor == &self.source_panel_id
                || !self
                    .panels
                    .iter()
                    .any(|panel| &panel.panel_id == neighbor && panel.unzoomed.is_some())
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        let mut splits = BTreeSet::new();
        let mut slots = BTreeSet::new();
        for split in &self.splits {
            if !splits.insert(&split.split_id)
                || !slots.insert(&split.first_slot_id)
                || !slots.insert(&split.second_slot_id)
                || split
                    .area
                    .is_some_and(|rect| !rect.nonempty() || !self.area.contains(rect))
                || split.divider.is_some_and(|rect| {
                    !rect.nonempty()
                        || split.area.is_none_or(|area| !area.contains(rect))
                        || match split.orientation {
                            PanelOrientation::Horizontal => rect.width != 1,
                            PanelOrientation::Vertical => rect.height != 1,
                        }
                })
            {
                return Err(ContractError::InvalidDocument);
            }
        }
        let encoded = serde_json::to_vec(self).map_err(|_| ContractError::InvalidDocument)?;
        // Reserve space for the bounded control, view and response correlation envelopes.
        if encoded.len() > MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES - 4096 {
            return Err(ContractError::EncodedValueTooLarge {
                actual: encoded.len(),
                maximum: MAXIMUM_ENCODED_CLIENT_LAYOUT_BYTES - 4096,
            });
        }
        Ok(())
    }
}
