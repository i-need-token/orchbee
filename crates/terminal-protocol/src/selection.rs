use std::num::NonZeroU64;

use serde::{Deserialize, Serialize};

use crate::{
    AttachmentId, MessageRole, ProtocolError, ProtocolErrorCode,
    control::{
        RequiredNullable, SafeInteger, SafeSignedInteger, decode_wire, preflight_json,
        serialize_wire,
    },
    limits::{
        MAXIMUM_CELL_PIXEL_DIMENSION, MAXIMUM_FRAME_COLUMNS, MAXIMUM_FRAME_ROWS,
        MAXIMUM_SELECTION_PREVIEW_BYTES,
    },
    terminal_fact::{bounded_u16, canonical_u64},
};

const SELECTION_GESTURE_KIND: u16 = 60;
const SELECTION_COMMAND_KIND: u16 = 61;
const SELECTION_CHANGED_KIND: u16 = 62;
const MAXIMUM_GESTURE_POSITION: i32 = 0xffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionViewportPoint {
    pub column: u16,
    pub row: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionPoint {
    pub column: u16,
    pub row: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionGesturePosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionGestureGeometry {
    pub cell_width: u16,
    pub columns: u16,
    pub padding_left: u16,
    pub screen_height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionGestureAction {
    Start {
        click_count: u8,
        point: SelectionViewportPoint,
        position: SelectionGesturePosition,
        select_output: bool,
    },
    Update {
        geometry: SelectionGestureGeometry,
        point: SelectionViewportPoint,
        position: SelectionGesturePosition,
        rectangle: bool,
    },
    End {
        point: SelectionViewportPoint,
    },
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionGesture {
    pub action: SelectionGestureAction,
    pub attachment_id: AttachmentId,
    pub generation: NonZeroU64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionCommandKind {
    Clear,
    SelectAll,
    SetRange {
        end: SelectionPoint,
        rectangle: bool,
        start: SelectionPoint,
    },
    SelectOutput {
        point: SelectionPoint,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionCommand {
    pub attachment_id: AttachmentId,
    pub command: SelectionCommandKind,
    pub generation: NonZeroU64,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionChangedState {
    None,
    Evicted,
    Active {
        end: SelectionPoint,
        preview: Option<String>,
        rectangle: bool,
        start: SelectionPoint,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionChanged {
    pub attachment_id: AttachmentId,
    pub state: SelectionChangedState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionControl {
    Gesture(SelectionGesture),
    Command(SelectionCommand),
    ChangedResponse(SelectionChanged),
    ChangedEvent(SelectionChanged),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedSelectionControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_selection_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<SelectionControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (SELECTION_GESTURE_KIND, MessageRole::Request) => {
            decode_wire::<SelectionGestureWire>(payload)
                .and_then(selection_gesture_from_wire)
                .map(SelectionControl::Gesture)
        }
        (SELECTION_COMMAND_KIND, MessageRole::Request) => {
            decode_wire::<SelectionCommandWire>(payload)
                .and_then(selection_command_from_wire)
                .map(SelectionControl::Command)
        }
        (SELECTION_CHANGED_KIND, MessageRole::Response) => {
            decode_wire::<SelectionChangedWire>(payload)
                .and_then(selection_changed_from_wire)
                .map(SelectionControl::ChangedResponse)
        }
        (SELECTION_CHANGED_KIND, MessageRole::Event) => {
            decode_wire::<SelectionChangedWire>(payload)
                .and_then(selection_changed_from_wire)
                .map(SelectionControl::ChangedEvent)
        }
        _ => Err(invalid_field(
            "message kind and role do not select a selection-control payload",
        )),
    }
}

pub fn encode_selection_control(
    value: &SelectionControl,
) -> Result<EncodedSelectionControl, ProtocolError> {
    let (kind, role, payload) = match value {
        SelectionControl::Gesture(gesture) => (
            SELECTION_GESTURE_KIND,
            MessageRole::Request,
            serialize_wire(&selection_gesture_to_wire(*gesture)?)?,
        ),
        SelectionControl::Command(command) => (
            SELECTION_COMMAND_KIND,
            MessageRole::Request,
            serialize_wire(&selection_command_to_wire(*command)?)?,
        ),
        SelectionControl::ChangedResponse(changed) => (
            SELECTION_CHANGED_KIND,
            MessageRole::Response,
            serialize_wire(&selection_changed_to_wire(changed)?)?,
        ),
        SelectionControl::ChangedEvent(changed) => (
            SELECTION_CHANGED_KIND,
            MessageRole::Event,
            serialize_wire(&selection_changed_to_wire(changed)?)?,
        ),
    };
    Ok(EncodedSelectionControl {
        kind,
        payload,
        role,
    })
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SelectionGestureWire {
    Start(SelectionGestureStartWire),
    Update(SelectionGestureUpdateWire),
    End(SelectionGestureEndWire),
    Cancel(SelectionGestureCancelWire),
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGestureStartWire {
    attachment_id: String,
    generation: String,
    action: SelectionGestureStartActionWire,
    click_count: SafeInteger,
    point: SelectionViewportPointWire,
    position: SelectionGesturePositionWire,
    select_output: bool,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionGestureStartActionWire {
    #[serde(rename = "start")]
    Start,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGestureUpdateWire {
    attachment_id: String,
    generation: String,
    action: SelectionGestureUpdateActionWire,
    geometry: SelectionGestureGeometryWire,
    point: SelectionViewportPointWire,
    position: SelectionGesturePositionWire,
    rectangle: bool,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionGestureUpdateActionWire {
    #[serde(rename = "update")]
    Update,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGestureEndWire {
    attachment_id: String,
    generation: String,
    action: SelectionGestureEndActionWire,
    point: SelectionViewportPointWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionGestureEndActionWire {
    #[serde(rename = "end")]
    End,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGestureCancelWire {
    attachment_id: String,
    generation: String,
    action: SelectionGestureCancelActionWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionGestureCancelActionWire {
    #[serde(rename = "cancel")]
    Cancel,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionViewportPointWire {
    column: SafeInteger,
    row: SafeInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGesturePositionWire {
    x: SafeSignedInteger,
    y: SafeSignedInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionGestureGeometryWire {
    cell_width: SafeInteger,
    columns: SafeInteger,
    padding_left: SafeInteger,
    screen_height: SafeInteger,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SelectionCommandWire {
    Clear(SelectionCommandClearWire),
    SelectAll(SelectionCommandSelectAllWire),
    SetRange(SelectionCommandSetRangeWire),
    SelectOutput(SelectionCommandSelectOutputWire),
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionCommandClearWire {
    attachment_id: String,
    generation: String,
    revision: String,
    command: SelectionCommandClearKindWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionCommandClearKindWire {
    #[serde(rename = "clear")]
    Clear,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionCommandSelectAllWire {
    attachment_id: String,
    generation: String,
    revision: String,
    command: SelectionCommandSelectAllKindWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionCommandSelectAllKindWire {
    #[serde(rename = "select-all")]
    SelectAll,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionCommandSetRangeWire {
    attachment_id: String,
    generation: String,
    revision: String,
    command: SelectionCommandSetRangeKindWire,
    end: SelectionPointWire,
    rectangle: bool,
    start: SelectionPointWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionCommandSetRangeKindWire {
    #[serde(rename = "set-range")]
    SetRange,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionCommandSelectOutputWire {
    attachment_id: String,
    generation: String,
    revision: String,
    command: SelectionCommandSelectOutputKindWire,
    point: SelectionPointWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionCommandSelectOutputKindWire {
    #[serde(rename = "select-output")]
    SelectOutput,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionPointWire {
    column: SafeInteger,
    row: String,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum SelectionChangedWire {
    None(SelectionChangedNoneWire),
    Evicted(SelectionChangedEvictedWire),
    Active(SelectionChangedActiveWire),
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionChangedNoneWire {
    attachment_id: String,
    state: SelectionChangedNoneStateWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionChangedNoneStateWire {
    #[serde(rename = "none")]
    None,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionChangedEvictedWire {
    attachment_id: String,
    state: SelectionChangedEvictedStateWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionChangedEvictedStateWire {
    #[serde(rename = "evicted")]
    Evicted,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SelectionChangedActiveWire {
    attachment_id: String,
    end: SelectionPointWire,
    #[serde(default)]
    preview: RequiredNullable<String>,
    rectangle: bool,
    start: SelectionPointWire,
    state: SelectionChangedActiveStateWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SelectionChangedActiveStateWire {
    #[serde(rename = "active")]
    Active,
}

fn selection_gesture_from_wire(
    wire: SelectionGestureWire,
) -> Result<SelectionGesture, ProtocolError> {
    let (attachment_id, generation, action) = match wire {
        SelectionGestureWire::Start(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            SelectionGestureAction::Start {
                click_count: u8::try_from(bounded_u16(value.click_count, 1, 3)?)
                    .map_err(|_error| invalid_field("selection click count exceeds u8"))?,
                point: viewport_point_from_wire(value.point)?,
                position: gesture_position_from_wire(value.position)?,
                select_output: value.select_output,
            },
        ),
        SelectionGestureWire::Update(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            SelectionGestureAction::Update {
                geometry: gesture_geometry_from_wire(value.geometry)?,
                point: viewport_point_from_wire(value.point)?,
                position: gesture_position_from_wire(value.position)?,
                rectangle: value.rectangle,
            },
        ),
        SelectionGestureWire::End(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            SelectionGestureAction::End {
                point: viewport_point_from_wire(value.point)?,
            },
        ),
        SelectionGestureWire::Cancel(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            SelectionGestureAction::Cancel,
        ),
    };
    Ok(SelectionGesture {
        action,
        attachment_id,
        generation,
    })
}

fn selection_gesture_to_wire(
    value: SelectionGesture,
) -> Result<SelectionGestureWire, ProtocolError> {
    let attachment_id = value.attachment_id.to_text();
    let generation = value.generation.get().to_string();
    match value.action {
        SelectionGestureAction::Start {
            click_count,
            point,
            position,
            select_output,
        } => {
            validate_click_count(click_count)?;
            Ok(SelectionGestureWire::Start(SelectionGestureStartWire {
                attachment_id,
                generation,
                action: SelectionGestureStartActionWire::Start,
                click_count: SafeInteger::new(u64::from(click_count)),
                point: viewport_point_to_wire(point)?,
                position: gesture_position_to_wire(position)?,
                select_output,
            }))
        }
        SelectionGestureAction::Update {
            geometry,
            point,
            position,
            rectangle,
        } => Ok(SelectionGestureWire::Update(SelectionGestureUpdateWire {
            attachment_id,
            generation,
            action: SelectionGestureUpdateActionWire::Update,
            geometry: gesture_geometry_to_wire(geometry)?,
            point: viewport_point_to_wire(point)?,
            position: gesture_position_to_wire(position)?,
            rectangle,
        })),
        SelectionGestureAction::End { point } => {
            Ok(SelectionGestureWire::End(SelectionGestureEndWire {
                attachment_id,
                generation,
                action: SelectionGestureEndActionWire::End,
                point: viewport_point_to_wire(point)?,
            }))
        }
        SelectionGestureAction::Cancel => {
            Ok(SelectionGestureWire::Cancel(SelectionGestureCancelWire {
                attachment_id,
                generation,
                action: SelectionGestureCancelActionWire::Cancel,
            }))
        }
    }
}

fn selection_command_from_wire(
    wire: SelectionCommandWire,
) -> Result<SelectionCommand, ProtocolError> {
    let (attachment_id, generation, revision, command) = match wire {
        SelectionCommandWire::Clear(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            canonical_u64(&value.revision)?,
            SelectionCommandKind::Clear,
        ),
        SelectionCommandWire::SelectAll(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            canonical_u64(&value.revision)?,
            SelectionCommandKind::SelectAll,
        ),
        SelectionCommandWire::SetRange(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            canonical_u64(&value.revision)?,
            SelectionCommandKind::SetRange {
                end: selection_point_from_wire(value.end)?,
                rectangle: value.rectangle,
                start: selection_point_from_wire(value.start)?,
            },
        ),
        SelectionCommandWire::SelectOutput(value) => (
            parse_attachment_id(&value.attachment_id)?,
            positive_canonical_u64(&value.generation)?,
            canonical_u64(&value.revision)?,
            SelectionCommandKind::SelectOutput {
                point: selection_point_from_wire(value.point)?,
            },
        ),
    };
    Ok(SelectionCommand {
        attachment_id,
        command,
        generation,
        revision,
    })
}

fn selection_command_to_wire(
    value: SelectionCommand,
) -> Result<SelectionCommandWire, ProtocolError> {
    let attachment_id = value.attachment_id.to_text();
    let generation = value.generation.get().to_string();
    let revision = value.revision.to_string();
    match value.command {
        SelectionCommandKind::Clear => Ok(SelectionCommandWire::Clear(SelectionCommandClearWire {
            attachment_id,
            generation,
            revision,
            command: SelectionCommandClearKindWire::Clear,
        })),
        SelectionCommandKind::SelectAll => Ok(SelectionCommandWire::SelectAll(
            SelectionCommandSelectAllWire {
                attachment_id,
                generation,
                revision,
                command: SelectionCommandSelectAllKindWire::SelectAll,
            },
        )),
        SelectionCommandKind::SetRange {
            end,
            rectangle,
            start,
        } => Ok(SelectionCommandWire::SetRange(
            SelectionCommandSetRangeWire {
                attachment_id,
                generation,
                revision,
                command: SelectionCommandSetRangeKindWire::SetRange,
                end: selection_point_to_wire(end)?,
                rectangle,
                start: selection_point_to_wire(start)?,
            },
        )),
        SelectionCommandKind::SelectOutput { point } => Ok(SelectionCommandWire::SelectOutput(
            SelectionCommandSelectOutputWire {
                attachment_id,
                generation,
                revision,
                command: SelectionCommandSelectOutputKindWire::SelectOutput,
                point: selection_point_to_wire(point)?,
            },
        )),
    }
}

fn selection_changed_from_wire(
    wire: SelectionChangedWire,
) -> Result<SelectionChanged, ProtocolError> {
    let (attachment_id, state) = match wire {
        SelectionChangedWire::None(value) => (
            parse_attachment_id(&value.attachment_id)?,
            SelectionChangedState::None,
        ),
        SelectionChangedWire::Evicted(value) => (
            parse_attachment_id(&value.attachment_id)?,
            SelectionChangedState::Evicted,
        ),
        SelectionChangedWire::Active(value) => {
            let preview = value.preview.into_option()?;
            validate_preview(preview.as_deref())?;
            (
                parse_attachment_id(&value.attachment_id)?,
                SelectionChangedState::Active {
                    end: selection_point_from_wire(value.end)?,
                    preview,
                    rectangle: value.rectangle,
                    start: selection_point_from_wire(value.start)?,
                },
            )
        }
    };
    Ok(SelectionChanged {
        attachment_id,
        state,
    })
}

fn selection_changed_to_wire(
    value: &SelectionChanged,
) -> Result<SelectionChangedWire, ProtocolError> {
    match &value.state {
        SelectionChangedState::None => Ok(SelectionChangedWire::None(SelectionChangedNoneWire {
            attachment_id: value.attachment_id.to_text(),
            state: SelectionChangedNoneStateWire::None,
        })),
        SelectionChangedState::Evicted => {
            Ok(SelectionChangedWire::Evicted(SelectionChangedEvictedWire {
                attachment_id: value.attachment_id.to_text(),
                state: SelectionChangedEvictedStateWire::Evicted,
            }))
        }
        SelectionChangedState::Active {
            end,
            preview,
            rectangle,
            start,
        } => {
            validate_preview(preview.as_deref())?;
            Ok(SelectionChangedWire::Active(SelectionChangedActiveWire {
                attachment_id: value.attachment_id.to_text(),
                end: selection_point_to_wire(*end)?,
                preview: RequiredNullable::from_option(preview.clone()),
                rectangle: *rectangle,
                start: selection_point_to_wire(*start)?,
                state: SelectionChangedActiveStateWire::Active,
            }))
        }
    }
}

fn viewport_point_from_wire(
    value: SelectionViewportPointWire,
) -> Result<SelectionViewportPoint, ProtocolError> {
    Ok(SelectionViewportPoint {
        column: bounded_u16(value.column, 0, maximum_viewport_column())?,
        row: bounded_u16(value.row, 0, maximum_viewport_row())?,
    })
}

fn viewport_point_to_wire(
    value: SelectionViewportPoint,
) -> Result<SelectionViewportPointWire, ProtocolError> {
    validate_viewport_point(value)?;
    Ok(SelectionViewportPointWire {
        column: SafeInteger::new(u64::from(value.column)),
        row: SafeInteger::new(u64::from(value.row)),
    })
}

fn selection_point_from_wire(value: SelectionPointWire) -> Result<SelectionPoint, ProtocolError> {
    Ok(SelectionPoint {
        column: bounded_u16(value.column, 0, maximum_viewport_column())?,
        row: canonical_u64(&value.row)?,
    })
}

fn selection_point_to_wire(value: SelectionPoint) -> Result<SelectionPointWire, ProtocolError> {
    validate_absolute_point(value)?;
    Ok(SelectionPointWire {
        column: SafeInteger::new(u64::from(value.column)),
        row: value.row.to_string(),
    })
}

fn gesture_position_from_wire(
    value: SelectionGesturePositionWire,
) -> Result<SelectionGesturePosition, ProtocolError> {
    let position = SelectionGesturePosition {
        x: i32::try_from(value.x.get())
            .map_err(|_error| invalid_field("selection position exceeds signed 32-bit range"))?,
        y: i32::try_from(value.y.get())
            .map_err(|_error| invalid_field("selection position exceeds signed 32-bit range"))?,
    };
    validate_gesture_position(position)?;
    Ok(position)
}

fn gesture_position_to_wire(
    value: SelectionGesturePosition,
) -> Result<SelectionGesturePositionWire, ProtocolError> {
    validate_gesture_position(value)?;
    Ok(SelectionGesturePositionWire {
        x: SafeSignedInteger::new(i64::from(value.x)),
        y: SafeSignedInteger::new(i64::from(value.y)),
    })
}

fn gesture_geometry_from_wire(
    value: SelectionGestureGeometryWire,
) -> Result<SelectionGestureGeometry, ProtocolError> {
    Ok(SelectionGestureGeometry {
        cell_width: bounded_u16(value.cell_width, 1, MAXIMUM_CELL_PIXEL_DIMENSION)?,
        columns: bounded_u16(value.columns, 1, MAXIMUM_FRAME_COLUMNS)?,
        padding_left: bounded_u16(value.padding_left, 0, usize::from(u16::MAX))?,
        screen_height: bounded_u16(value.screen_height, 1, usize::from(u16::MAX))?,
    })
}

fn gesture_geometry_to_wire(
    value: SelectionGestureGeometry,
) -> Result<SelectionGestureGeometryWire, ProtocolError> {
    validate_gesture_geometry(value)?;
    Ok(SelectionGestureGeometryWire {
        cell_width: SafeInteger::new(u64::from(value.cell_width)),
        columns: SafeInteger::new(u64::from(value.columns)),
        padding_left: SafeInteger::new(u64::from(value.padding_left)),
        screen_height: SafeInteger::new(u64::from(value.screen_height)),
    })
}

fn validate_viewport_point(value: SelectionViewportPoint) -> Result<(), ProtocolError> {
    if usize::from(value.column) >= MAXIMUM_FRAME_COLUMNS
        || usize::from(value.row) >= MAXIMUM_FRAME_ROWS
    {
        return Err(invalid_field(
            "selection viewport point is outside its field bounds",
        ));
    }
    Ok(())
}

fn validate_absolute_point(value: SelectionPoint) -> Result<(), ProtocolError> {
    if usize::from(value.column) >= MAXIMUM_FRAME_COLUMNS {
        return Err(invalid_field(
            "absolute selection point column is outside its field bounds",
        ));
    }
    Ok(())
}

fn validate_click_count(value: u8) -> Result<(), ProtocolError> {
    if !(1..=3).contains(&value) {
        return Err(invalid_field(
            "selection click count is outside its field bounds",
        ));
    }
    Ok(())
}

fn validate_gesture_position(value: SelectionGesturePosition) -> Result<(), ProtocolError> {
    if !(-MAXIMUM_GESTURE_POSITION..=MAXIMUM_GESTURE_POSITION).contains(&value.x)
        || !(-MAXIMUM_GESTURE_POSITION..=MAXIMUM_GESTURE_POSITION).contains(&value.y)
    {
        return Err(invalid_field(
            "selection gesture position is outside its field bounds",
        ));
    }
    Ok(())
}

fn validate_gesture_geometry(value: SelectionGestureGeometry) -> Result<(), ProtocolError> {
    if value.cell_width == 0
        || usize::from(value.cell_width) > MAXIMUM_CELL_PIXEL_DIMENSION
        || value.columns == 0
        || usize::from(value.columns) > MAXIMUM_FRAME_COLUMNS
        || value.screen_height == 0
    {
        return Err(invalid_field(
            "selection gesture geometry is outside its field bounds",
        ));
    }
    Ok(())
}

fn validate_preview(value: Option<&str>) -> Result<(), ProtocolError> {
    if value.is_some_and(|value| value.len() > MAXIMUM_SELECTION_PREVIEW_BYTES) {
        return Err(invalid_field(
            "selection preview exceeds its UTF-8 byte bound",
        ));
    }
    Ok(())
}

const fn maximum_viewport_column() -> usize {
    MAXIMUM_FRAME_COLUMNS - 1
}

const fn maximum_viewport_row() -> usize {
    MAXIMUM_FRAME_ROWS - 1
}

fn parse_attachment_id(value: &str) -> Result<AttachmentId, ProtocolError> {
    AttachmentId::parse(value).map_err(|_error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidIdentifier,
            "attachment identity is not canonical nonzero 128-bit hexadecimal text",
        )
    })
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?)
        .ok_or_else(|| invalid_field("unsigned 64-bit text must be greater than zero"))
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}
