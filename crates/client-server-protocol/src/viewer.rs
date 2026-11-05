//! Defines attachment-scoped viewer requests, semantic bounds, and admissible response classes
//! for projection, history, selection, and read feedback. It carries opaque records without
//! interpreting terminal contents, deciding target exposure, or owning viewer lifetime.

use crate::{
    HostCreationId, HostEpoch, HostTerminalId, ProtocolError, RequestId, SessionId, TerminalId,
    ViewerId, WorkerId,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::num::NonZeroU64;

pub const MAXIMUM_VIEWER_RECORD_BYTES: usize = 64 + 36 * 1024 * 1024;

/// Buffer and viewport reads default to text; render retains styles and graphics resources.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerRowDetail {
    #[default]
    Text,
    Render,
}

impl ViewerRowDetail {
    const fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }
}

/// Projection delivery the Client requests for an attached viewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProjectionDemand {
    Active,
    Paused,
}

/// Retained-history delivery the Client requests for an attached viewer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryDemand {
    FollowingBottom,
    ViewingHistory,
}

/// The row window origin a `read-viewport-rows` action resolves against the
/// Host's retained rows: a distance back from the live tail or a signed delta
/// from the Host-committed viewport anchor, both read from the Host's current
/// view, or an absolute row, read only at the revision the request names.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ViewportRowOrigin {
    Tail {
        rows_before: u32,
    },
    Committed {
        delta: i32,
    },
    Absolute {
        #[serde(with = "crate::number::decimal")]
        row: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerSnapshotReason {
    BaseMismatch,
    DecoderRejected,
    DimensionsMismatch,
    GenerationMismatch,
    IdentityMismatch,
    SequenceGap,
    StoreReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSelectionViewportPoint {
    pub column: u16,
    pub row: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSelectionGesturePosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSelectionGestureGeometry {
    pub cell_width: u16,
    pub columns: u16,
    pub padding_left: u16,
    pub screen_height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ViewerSelectionGestureAction {
    Start {
        click_count: u8,
        point: ViewerSelectionViewportPoint,
        position: ViewerSelectionGesturePosition,
        select_output: bool,
    },
    Update {
        geometry: ViewerSelectionGestureGeometry,
        point: ViewerSelectionViewportPoint,
        position: ViewerSelectionGesturePosition,
        rectangle: bool,
    },
    End {
        point: ViewerSelectionViewportPoint,
    },
    Cancel,
}

/// A flattened internally tagged enum borrows Serde's buffered fields instead
/// of consuming them. The outer action's `deny_unknown_fields` would therefore
/// reject a valid gesture after decoding it. This wire shape is a struct so its
/// named fields are consumed; conversion below then enforces the exact field
/// set for the selected branch.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViewerSelectionGestureWire {
    action: ViewerSelectionGestureDiscriminant,
    #[serde(default)]
    click_count: ViewerSelectionWireField<u8>,
    #[serde(default)]
    geometry: ViewerSelectionWireField<ViewerSelectionGestureGeometry>,
    #[serde(default)]
    point: ViewerSelectionWireField<ViewerSelectionViewportPoint>,
    #[serde(default)]
    position: ViewerSelectionWireField<ViewerSelectionGesturePosition>,
    #[serde(default)]
    rectangle: ViewerSelectionWireField<bool>,
    #[serde(default)]
    select_output: ViewerSelectionWireField<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ViewerSelectionGestureDiscriminant {
    Start,
    Update,
    End,
    Cancel,
}

#[derive(Default)]
enum ViewerSelectionWireField<T> {
    #[default]
    Missing,
    Present(T),
}

impl<'de, T> Deserialize<'de> for ViewerSelectionWireField<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer).map(Self::Present)
    }
}

fn deserialize_viewer_selection_gesture<'de, D>(
    deserializer: D,
) -> Result<ViewerSelectionGestureAction, D::Error>
where
    D: Deserializer<'de>,
{
    use ViewerSelectionGestureDiscriminant as Action;
    use ViewerSelectionWireField::{Missing, Present};

    let wire = ViewerSelectionGestureWire::deserialize(deserializer)?;
    match wire {
        ViewerSelectionGestureWire {
            action: Action::Start,
            click_count: Present(click_count),
            geometry: Missing,
            point: Present(point),
            position: Present(position),
            rectangle: Missing,
            select_output: Present(select_output),
        } => Ok(ViewerSelectionGestureAction::Start {
            click_count,
            point,
            position,
            select_output,
        }),
        ViewerSelectionGestureWire {
            action: Action::Update,
            click_count: Missing,
            geometry: Present(geometry),
            point: Present(point),
            position: Present(position),
            rectangle: Present(rectangle),
            select_output: Missing,
        } => Ok(ViewerSelectionGestureAction::Update {
            geometry,
            point,
            position,
            rectangle,
        }),
        ViewerSelectionGestureWire {
            action: Action::End,
            click_count: Missing,
            geometry: Missing,
            point: Present(point),
            position: Missing,
            rectangle: Missing,
            select_output: Missing,
        } => Ok(ViewerSelectionGestureAction::End { point }),
        ViewerSelectionGestureWire {
            action: Action::Cancel,
            click_count: Missing,
            geometry: Missing,
            point: Missing,
            position: Missing,
            rectangle: Missing,
            select_output: Missing,
        } => Ok(ViewerSelectionGestureAction::Cancel),
        // Any missing or extra discriminant-specific field is an invalid wire shape.
        ViewerSelectionGestureWire {
            action: Action::Start | Action::Update | Action::End | Action::Cancel,
            ..
        } => Err(<D::Error as serde::de::Error>::custom(
            "selection gesture fields do not match its action",
        )),
    }
}

/// A selection endpoint in the Host's retained rows, as a Host selection event names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewerSelectionPoint {
    pub column: u16,
    #[serde(with = "crate::number::decimal")]
    pub row: u64,
}

/// A selection change that names its result directly instead of following a pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(
    tag = "command",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum ViewerSelectionCommand {
    Clear,
    SelectAll,
    SetRange {
        end: ViewerSelectionPoint,
        rectangle: bool,
        start: ViewerSelectionPoint,
    },
    SelectOutput {
        point: ViewerSelectionPoint,
    },
}

/// Flattened beside the action's own fields for the reason `ViewerSelectionGestureWire` gives.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ViewerSelectionCommandWire {
    command: ViewerSelectionCommandDiscriminant,
    #[serde(default)]
    end: ViewerSelectionWireField<ViewerSelectionPoint>,
    #[serde(default)]
    point: ViewerSelectionWireField<ViewerSelectionPoint>,
    #[serde(default)]
    rectangle: ViewerSelectionWireField<bool>,
    #[serde(default)]
    start: ViewerSelectionWireField<ViewerSelectionPoint>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ViewerSelectionCommandDiscriminant {
    Clear,
    SelectAll,
    SetRange,
    SelectOutput,
}

fn deserialize_viewer_selection_command<'de, D>(
    deserializer: D,
) -> Result<ViewerSelectionCommand, D::Error>
where
    D: Deserializer<'de>,
{
    use ViewerSelectionCommandDiscriminant as Command;
    use ViewerSelectionWireField::{Missing, Present};

    let wire = ViewerSelectionCommandWire::deserialize(deserializer)?;
    match wire {
        ViewerSelectionCommandWire {
            command: Command::Clear,
            end: Missing,
            point: Missing,
            rectangle: Missing,
            start: Missing,
        } => Ok(ViewerSelectionCommand::Clear),
        ViewerSelectionCommandWire {
            command: Command::SelectAll,
            end: Missing,
            point: Missing,
            rectangle: Missing,
            start: Missing,
        } => Ok(ViewerSelectionCommand::SelectAll),
        ViewerSelectionCommandWire {
            command: Command::SetRange,
            end: Present(end),
            point: Missing,
            rectangle: Present(rectangle),
            start: Present(start),
        } => Ok(ViewerSelectionCommand::SetRange {
            end,
            rectangle,
            start,
        }),
        ViewerSelectionCommandWire {
            command: Command::SelectOutput,
            end: Missing,
            point: Present(point),
            rectangle: Missing,
            start: Missing,
        } => Ok(ViewerSelectionCommand::SelectOutput { point }),
        // Any missing or extra discriminant-specific field is an invalid wire shape.
        ViewerSelectionCommandWire {
            command: Command::Clear | Command::SelectAll | Command::SetRange | Command::SelectOutput,
            ..
        } => Err(<D::Error as serde::de::Error>::custom(
            "selection command fields do not match its command",
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerSelectionExportFormat {
    Plain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerSelectionExportCancelReason {
    ClipboardFailed,
    Disposed,
    Replaced,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalViewerAction {
    Attach {
        terminal_create_request_id: RequestId,
    },
    AttachBinding {
        #[serde(rename = "sessionId")]
        session_id: SessionId,
        worker_id: WorkerId,
        terminal_id: TerminalId,
    },
    Poll {
        attachment_id: ViewerId,
    },
    ReadProcessInfo {
        attachment_id: ViewerId,
    },
    FrameApplied {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    NeedSnapshot {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        reason: ViewerSnapshotReason,
    },
    ReadRows {
        attachment_id: ViewerId,
        #[serde(default, skip_serializing_if = "ViewerRowDetail::is_text")]
        detail: ViewerRowDetail,
        #[serde(with = "crate::number::decimal")]
        start_row: u64,
        row_count: u16,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    SetProjectionDemand {
        attachment_id: ViewerId,
        demand: ProjectionDemand,
    },
    SetHistoryDemand {
        attachment_id: ViewerId,
        demand: HistoryDemand,
    },
    // `maximum_bytes` bounds the OBRW payload, not the record. The served
    // window's revision is Host-captured identity, never request equality.
    ReadViewportRows {
        attachment_id: ViewerId,
        #[serde(default, skip_serializing_if = "ViewerRowDetail::is_text")]
        detail: ViewerRowDetail,
        origin: ViewportRowOrigin,
        row_count: u16,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        maximum_bytes: u32,
    },
    RowsApplied {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        request_sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    HistoryObserved {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        sequence: NonZeroU64,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
    },
    SelectionGesture {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(flatten, deserialize_with = "deserialize_viewer_selection_gesture")]
        gesture: ViewerSelectionGestureAction,
    },
    SelectionCommand {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        generation: NonZeroU64,
        #[serde(with = "crate::number::decimal")]
        revision: u64,
        #[serde(flatten, deserialize_with = "deserialize_viewer_selection_command")]
        command: ViewerSelectionCommand,
    },
    SelectionExport {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        export_sequence: NonZeroU64,
        format: ViewerSelectionExportFormat,
    },
    SelectionExportChunkApplied {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        export_sequence: NonZeroU64,
        chunk_sequence: u32,
    },
    SelectionExportCancel {
        attachment_id: ViewerId,
        #[serde(with = "crate::number::positive_decimal")]
        export_sequence: NonZeroU64,
        reason: ViewerSelectionExportCancelReason,
    },
    Detach {
        attachment_id: ViewerId,
    },
}

/// The closed reason vocabulary a `rows-rejected` outcome may carry; each
/// variant maps to exactly one Host row-window failure code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerRowsRejectedReason {
    StaleRevision,
    StaleGeneration,
    HistoryEvicted,
    MissingResource,
    InvalidField,
    InvalidAcknowledgement,
    LimitExceeded,
    ResourceExhausted,
    InvalidPhase,
}

/// `read-rows` bounds: the row count and byte budget the action contract
/// admits. `maximum_bytes` bounds the OBRW payload, not the carrier record.
pub const MAXIMUM_READ_ROWS: u16 = 4096;
pub const MINIMUM_READ_ROWS_BYTES: u32 = 112;
pub const MAXIMUM_READ_ROWS_BYTES: u32 = 8 * 1024 * 1024;

impl TerminalViewerAction {
    /// Semantic bounds the JSON shape cannot express; callers reject the
    /// message when this fails, exactly like a malformed field.
    pub(crate) fn validate(&self) -> Result<(), ProtocolError> {
        let bounds = match self {
            Self::ReadRows {
                row_count,
                maximum_bytes,
                ..
            }
            | Self::ReadViewportRows {
                row_count,
                maximum_bytes,
                ..
            } => Some((*row_count, *maximum_bytes)),
            // Only row reads carry this pair of bounds; other fields validate separately.
            _ => None,
        };
        if bounds.is_some_and(|bounds| {
            bounds.0 == 0
                || bounds.0 > MAXIMUM_READ_ROWS
                || !(MINIMUM_READ_ROWS_BYTES..=MAXIMUM_READ_ROWS_BYTES).contains(&bounds.1)
        }) {
            return Err(ProtocolError::InvalidPayload);
        }
        if let Self::SelectionGesture { gesture, .. } = self {
            validate_selection_gesture(*gesture)?;
        }
        if let Self::SelectionCommand { command, .. } = self {
            validate_selection_command(*command)?;
        }
        Ok(())
    }

    #[must_use]
    pub const fn attachment_id(&self) -> Option<&ViewerId> {
        match self {
            Self::Attach { .. } | Self::AttachBinding { .. } => None,
            Self::Poll { attachment_id }
            | Self::ReadProcessInfo { attachment_id }
            | Self::FrameApplied { attachment_id, .. }
            | Self::NeedSnapshot { attachment_id, .. }
            | Self::ReadRows { attachment_id, .. }
            | Self::SetProjectionDemand { attachment_id, .. }
            | Self::SetHistoryDemand { attachment_id, .. }
            | Self::ReadViewportRows { attachment_id, .. }
            | Self::RowsApplied { attachment_id, .. }
            | Self::HistoryObserved { attachment_id, .. }
            | Self::SelectionGesture { attachment_id, .. }
            | Self::SelectionCommand { attachment_id, .. }
            | Self::SelectionExport { attachment_id, .. }
            | Self::SelectionExportChunkApplied { attachment_id, .. }
            | Self::SelectionExportCancel { attachment_id, .. }
            | Self::Detach { attachment_id } => Some(attachment_id),
        }
    }
}

const MAXIMUM_SELECTION_DIMENSION: u16 = 4096;

fn validate_selection_command(command: ViewerSelectionCommand) -> Result<(), ProtocolError> {
    let valid_point = |point: ViewerSelectionPoint| point.column < MAXIMUM_SELECTION_DIMENSION;
    let valid = match command {
        ViewerSelectionCommand::Clear | ViewerSelectionCommand::SelectAll => true,
        ViewerSelectionCommand::SetRange { end, start, .. } => {
            valid_point(end) && valid_point(start)
        }
        ViewerSelectionCommand::SelectOutput { point } => valid_point(point),
    };
    if valid {
        Ok(())
    } else {
        Err(ProtocolError::InvalidPayload)
    }
}

fn validate_selection_gesture(gesture: ViewerSelectionGestureAction) -> Result<(), ProtocolError> {
    const MAXIMUM_AXIS: i32 = 0xffff;

    let valid_point = |point: ViewerSelectionViewportPoint| {
        point.column < MAXIMUM_SELECTION_DIMENSION && point.row < MAXIMUM_SELECTION_DIMENSION
    };
    let valid_position = |position: ViewerSelectionGesturePosition| {
        (-MAXIMUM_AXIS..=MAXIMUM_AXIS).contains(&position.x)
            && (-MAXIMUM_AXIS..=MAXIMUM_AXIS).contains(&position.y)
    };
    let valid = match gesture {
        ViewerSelectionGestureAction::Start {
            click_count,
            point,
            position,
            ..
        } => (1..=3).contains(&click_count) && valid_point(point) && valid_position(position),
        ViewerSelectionGestureAction::Update {
            geometry,
            point,
            position,
            ..
        } => {
            geometry.cell_width > 0
                && geometry.cell_width <= MAXIMUM_SELECTION_DIMENSION
                && geometry.columns > 0
                && geometry.columns <= MAXIMUM_SELECTION_DIMENSION
                && geometry.screen_height > 0
                && valid_point(point)
                && valid_position(position)
        }
        ViewerSelectionGestureAction::End { point } => valid_point(point),
        ViewerSelectionGestureAction::Cancel => true,
    };
    if valid {
        Ok(())
    } else {
        Err(ProtocolError::InvalidPayload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerRequest {
    pub(crate) request_id: RequestId,
    pub(crate) action: TerminalViewerAction,
}
impl TerminalViewerRequest {
    #[must_use]
    pub const fn new(request_id: RequestId, action: TerminalViewerAction) -> Self {
        Self { request_id, action }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn action(&self) -> &TerminalViewerAction {
        &self.action
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerRejectedReason {
    AlreadyAttached,
    TerminalUnavailable,
    FeedbackMismatch,
    /// Reserved for the four control-acknowledgement actions' explicit Host
    /// typed failures; distinct from the controller-plane reason of the same
    /// name and never applied to older actions.
    HostRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewerUnavailableReason {
    ServerBusy,
    TerminalRuntimeUnavailable,
    WorkbenchUnavailable,
}

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TerminalViewerOutcome {
    Attached {
        attachment_id: ViewerId,
        creation_id: HostCreationId,
        host_epoch: HostEpoch,
        host_terminal_id: HostTerminalId,
    },
    Empty {},
    FeedbackAccepted {},
    Detached {},
    Rejected {
        reason: ViewerRejectedReason,
    },
    RowsRejected {
        reason: ViewerRowsRejectedReason,
    },
    Unavailable {
        reason: ViewerUnavailableReason,
    },
    // Records use OBSR binary framing and have no JSON representation.
    #[serde(skip)]
    Record(Vec<u8>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalViewerResult {
    pub(crate) request_id: RequestId,
    pub(crate) outcome: TerminalViewerOutcome,
}
impl TerminalViewerResult {
    #[must_use]
    pub const fn new(request_id: RequestId, outcome: TerminalViewerOutcome) -> Self {
        Self {
            request_id,
            outcome,
        }
    }
    #[must_use]
    pub const fn request_id(&self) -> &RequestId {
        &self.request_id
    }
    #[must_use]
    pub const fn outcome(&self) -> &TerminalViewerOutcome {
        &self.outcome
    }
    #[must_use]
    pub fn into_outcome(self) -> TerminalViewerOutcome {
        self.outcome
    }
}

impl TerminalViewerAction {
    /// Admits this action's response class; consumers validate record payloads and target echoes.
    #[must_use]
    pub fn admits(&self, outcome: &TerminalViewerOutcome) -> bool {
        let failed = matches!(
            outcome,
            TerminalViewerOutcome::Rejected { .. } | TerminalViewerOutcome::Unavailable { .. }
        );
        match self {
            Self::Attach { .. } => {
                matches!(outcome, TerminalViewerOutcome::Attached { .. }) || failed
            }
            Self::AttachBinding { .. } => {
                matches!(outcome, TerminalViewerOutcome::Attached { .. }) || failed
            }
            Self::Poll { .. } => {
                matches!(
                    outcome,
                    TerminalViewerOutcome::Empty {} | TerminalViewerOutcome::Record(_)
                ) || failed
            }
            Self::FrameApplied { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::NeedSnapshot { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::ReadProcessInfo { .. } => {
                matches!(outcome, TerminalViewerOutcome::Record(_)) || failed
            }
            Self::ReadRows { .. } => {
                matches!(
                    outcome,
                    TerminalViewerOutcome::Record(_) | TerminalViewerOutcome::RowsRejected { .. }
                ) || failed
            }
            Self::ReadViewportRows { .. } => {
                matches!(
                    outcome,
                    TerminalViewerOutcome::Record(_) | TerminalViewerOutcome::RowsRejected { .. }
                ) || failed
            }
            Self::SetProjectionDemand { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::SetHistoryDemand { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::RowsApplied { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::HistoryObserved { .. } => {
                matches!(outcome, TerminalViewerOutcome::FeedbackAccepted {}) || failed
            }
            Self::SelectionGesture { .. } => {
                matches!(outcome, TerminalViewerOutcome::Record(_)) || failed
            }
            Self::SelectionCommand { .. } => {
                matches!(outcome, TerminalViewerOutcome::Record(_)) || failed
            }
            Self::SelectionExport { .. } => {
                matches!(outcome, TerminalViewerOutcome::Record(_)) || failed
            }
            Self::SelectionExportCancel { .. } => {
                matches!(outcome, TerminalViewerOutcome::Record(_)) || failed
            }
            Self::SelectionExportChunkApplied { .. } => {
                matches!(
                    outcome,
                    TerminalViewerOutcome::FeedbackAccepted {} | TerminalViewerOutcome::Record(_)
                ) || failed
            }
            Self::Detach { .. } => matches!(outcome, TerminalViewerOutcome::Detached {}) || failed,
        }
    }
}
