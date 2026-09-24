//! Bounded Client-local layout adjustments and deterministic resize-border selection.
use super::client_layout::{
    ClientPanelDirection, ClientPanelLayout, ClientSplitArea, ClientSplitRatio,
};
use crate::{
    ContractError, MAXIMUM_PANEL_NODE_DEPTH, WorkbenchPanelSlotId, WorkbenchRevision,
    WorkbenchSplitId, WorkbenchTabId, WorkbenchTerminalContextId,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A normalized finite resize magnitude in 0..=0.5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientResizeAmount(f64);
impl Eq for ClientResizeAmount {}
impl ClientResizeAmount {
    /// Admit a normalized amount; callers normalize user-facing values before serialization.
    pub fn new(value: f64) -> Result<Self, ContractError> {
        if value.is_finite() && (0.0..=0.5).contains(&value) {
            Ok(Self(if value == 0.0 { 0.0 } else { value }))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Apply one signed, clamped adjustment to an admitted ratio.
    #[must_use]
    pub fn apply(
        self,
        ratio: ClientSplitRatio,
        direction: ClientPanelDirection,
    ) -> ClientSplitRatio {
        let sign = match direction {
            ClientPanelDirection::Right | ClientPanelDirection::Down => 1.0,
            ClientPanelDirection::Left | ClientPanelDirection::Up => -1.0,
        };
        ClientSplitRatio::new((ratio.value() + sign * self.0).clamp(0.1, 0.9))
            .expect("finite bounded ratio adjustment")
    }
    /// Return the normalized magnitude.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}
impl Serialize for ClientResizeAmount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f64(self.0)
    }
}
impl<'de> Deserialize<'de> for ClientResizeAmount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(f64::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// A root-relative sequence of binary child indices, bounded by canonical tree depth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ClientSplitPath(Vec<u8>);
impl ClientSplitPath {
    /// Reject nonbinary indices and paths deeper than admitted Workbench topology.
    pub fn new(indices: Vec<u8>) -> Result<Self, ContractError> {
        if indices.len() <= MAXIMUM_PANEL_NODE_DEPTH && indices.iter().all(|index| *index <= 1) {
            Ok(Self(indices))
        } else {
            Err(ContractError::InvalidDocument)
        }
    }
    /// Borrow the validated path; an empty path addresses the root split.
    #[must_use]
    pub fn indices(&self) -> &[u8] {
        &self.0
    }
}
impl<'de> Deserialize<'de> for ClientSplitPath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(Vec::<u8>::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// One resolved split and its before/after nominal proportions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientSplitChange {
    pub split_id: WorkbenchSplitId,
    pub first_slot_id: WorkbenchPanelSlotId,
    pub second_slot_id: WorkbenchPanelSlotId,
    pub previous_ratio: ClientSplitRatio,
    pub ratio: ClientSplitRatio,
}
impl ClientSplitChange {
    /// Whether the nominal ratio changed, independently of cell rounding.
    #[must_use]
    pub fn changed(&self) -> bool {
        self.previous_ratio != self.ratio
    }
}

/// An eligible split or the absence of a resize border, never a fabricated split.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientLayoutChange {
    NoSplit {},
    Split { split: ClientSplitChange },
}
impl ClientLayoutChange {
    /// Only a different nominal ratio commits Client-local state.
    #[must_use]
    pub fn changed(&self) -> bool {
        matches!(self, Self::Split { split } if split.changed())
    }
}

/// The retained observation of a single adjustment; selection and topology are untouched.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientLayoutAdjustment {
    pub terminal_context_id: WorkbenchTerminalContextId,
    pub tab_id: WorkbenchTabId,
    pub workbench_revision: WorkbenchRevision,
    pub selected: bool,
    pub change: ClientLayoutChange,
}

impl ClientLayoutAdjustment {
    /// Validate relationships not already guaranteed by the field types.
    pub fn validate(&self) -> Result<(), ContractError> {
        if matches!(&self.change, ClientLayoutChange::Split { split } if split.first_slot_id == split.second_slot_id)
        {
            return Err(ContractError::InvalidDocument);
        }
        Ok(())
    }
}

impl ClientPanelLayout {
    /// Choose a visible resize border using requested-edge priority and opposite-edge fallback.
    /// Hidden sources are rejected; None means a visible source has no eligible split.
    pub fn resize_split(
        &self,
        direction: ClientPanelDirection,
    ) -> Result<Option<&ClientSplitArea>, ContractError> {
        let source = self
            .panels
            .iter()
            .find(|panel| panel.panel_id == self.source_panel_id)
            .and_then(|panel| panel.unzoomed)
            .filter(|rect| rect.nonempty())
            .ok_or(ContractError::InvalidDocument)?;
        let horizontal = matches!(
            direction,
            ClientPanelDirection::Left | ClientPanelDirection::Right
        );
        let nearest = |edge: ClientPanelDirection| {
            self.splits
                .iter()
                .enumerate()
                .filter_map(|(index, split)| {
                    let area = split.area?;
                    let divider = split.divider?;
                    if matches!(split.orientation, crate::PanelOrientation::Horizontal)
                        != horizontal
                    {
                        return None;
                    }
                    let (start, end, from, to, pos, target) = if horizontal {
                        (
                            u32::from(area.y),
                            u32::from(area.y) + u32::from(area.height),
                            u32::from(source.y),
                            u32::from(source.y) + u32::from(source.height),
                            u32::from(divider.x),
                            u32::from(source.x)
                                + if edge == ClientPanelDirection::Right {
                                    u32::from(source.width)
                                } else {
                                    0
                                },
                        )
                    } else {
                        (
                            u32::from(area.x),
                            u32::from(area.x) + u32::from(area.width),
                            u32::from(source.x),
                            u32::from(source.x) + u32::from(source.width),
                            u32::from(divider.y),
                            u32::from(source.y)
                                + if edge == ClientPanelDirection::Down {
                                    u32::from(source.height)
                                } else {
                                    0
                                },
                        )
                    };
                    let distance = pos.abs_diff(target);
                    (end.min(to) > start.max(from) && distance <= 1)
                        .then_some((distance, index, split))
                })
                .min_by_key(|(distance, index, _)| (*distance, *index))
                .map(|(_, _, split)| split)
        };
        let opposite = match direction {
            ClientPanelDirection::Left => ClientPanelDirection::Right,
            ClientPanelDirection::Right => ClientPanelDirection::Left,
            ClientPanelDirection::Up => ClientPanelDirection::Down,
            ClientPanelDirection::Down => ClientPanelDirection::Up,
        };
        Ok(nearest(direction).or_else(|| nearest(opposite)))
    }
}
