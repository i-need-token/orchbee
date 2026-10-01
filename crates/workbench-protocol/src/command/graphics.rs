//! Complete Panel image contracts and bounded revision-pinned reads; no decoding or rendering effects.
use crate::{
    ContractError, ProtocolVersion, SiteId, TerminalPresentationId, WorkbenchClientId,
    WorkbenchPanelId, WorkbenchRequestId, WorkbenchRevision,
};
use serde::{Deserialize, Serialize};
mod validation;

pub const MAXIMUM_GRAPHICS_SET_BYTES: usize = 512 * 1024;
pub const MAXIMUM_GRAPHICS_STREAM_BYTES: usize = 16 * 1024 * 1024;
pub const MAXIMUM_GRAPHICS_DECODED_BYTES: usize = 32 * 1024 * 1024;
pub const MAXIMUM_GRAPHICS_RESIDENT_BYTES: usize = 64 * 1024 * 1024;
pub const MAXIMUM_GRAPHICS_CHUNK_BYTES: usize = 256 * 1024;
pub const MAXIMUM_GRAPHICS_REQUEST_BYTES: usize = 24 * 1024 * 1024;
pub const MAXIMUM_GRAPHICS_RESULT_BYTES: usize = 512 * 1024;
pub const MAXIMUM_GRAPHICS_LAYERS_PER_PANE: usize = 16;
pub const MAXIMUM_GRAPHICS_LAYERS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsTarget {
    pub panel_id: WorkbenchPanelId,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub presentation_id: Option<TerminalPresentationId>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneGraphicsFormat {
    Png,
    Rgb,
    Rgba,
    Bgra,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsPlacement {
    pub viewport_column: i32,
    pub viewport_row: i32,
    pub grid_columns: u32,
    pub grid_rows: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsFrame {
    pub format: PaneGraphicsFormat,
    pub width: u32,
    pub height: u32,
    pub data_base64: String,
    pub placement: PaneGraphicsPlacement,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsDamage {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PaneGraphicsStreamImage {
    Inline {
        image: PaneGraphicsFrame,
    },
    File {
        format: PaneGraphicsFormat,
        width: u32,
        height: u32,
        path: String,
        placement: PaneGraphicsPlacement,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneGraphicsTransport {
    CellRaster,
    Kitty,
    Sixel,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsCellPixels {
    pub width: u16,
    pub height: u16,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsCapabilities {
    pub transport: PaneGraphicsTransport,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub cell_pixels: Option<PaneGraphicsCellPixels>,
    pub pixel_mouse: bool,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsClientPane {
    pub target: PaneGraphicsTarget,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub binding: Option<crate::PresentationBinding>,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub drawn_revision: Option<WorkbenchRevision>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsClientObservation {
    pub workbench_revision: WorkbenchRevision,
    pub capabilities: PaneGraphicsCapabilities,
    pub complete: bool,
    pub panes: Vec<PaneGraphicsClientPane>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsLayer {
    pub layer_id: String,
    pub z_index: i32,
    pub width: u32,
    pub height: u32,
    pub placement: PaneGraphicsPlacement,
    pub resource_revision: WorkbenchRevision,
    #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
    pub stream_id: Option<WorkbenchRequestId>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PaneGraphicsAction {
    Observe {
        client_id: WorkbenchClientId,
        heartbeat: crate::ClientViewSequence,
        observation: Box<PaneGraphicsClientObservation>,
    },
    Info {
        target: PaneGraphicsTarget,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        client_id: Option<WorkbenchClientId>,
    },
    Set {
        target: PaneGraphicsTarget,
        layer_id: String,
        z_index: i32,
        image: PaneGraphicsFrame,
    },
    Clear {
        target: PaneGraphicsTarget,
        layer_id: String,
    },
    Catalog {
        target: PaneGraphicsTarget,
    },
    Pixels {
        target: PaneGraphicsTarget,
        layer_id: String,
        resource_revision: WorkbenchRevision,
        offset: u32,
        maximum_bytes: u32,
    },
    StreamOpen {
        stream_id: WorkbenchRequestId,
        target: PaneGraphicsTarget,
        layer_id: String,
        z_index: i32,
    },
    StreamFrame {
        stream_id: WorkbenchRequestId,
        sequence: WorkbenchRevision,
        revision: WorkbenchRevision,
        image: PaneGraphicsStreamImage,
        damage: Vec<PaneGraphicsDamage>,
    },
    StreamClose {
        stream_id: WorkbenchRequestId,
    },
}
impl PaneGraphicsAction {
    #[must_use]
    pub const fn writes(&self) -> bool {
        !matches!(
            self,
            Self::Info { .. } | Self::Catalog { .. } | Self::Pixels { .. } | Self::Observe { .. }
        )
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsRequest {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub action: PaneGraphicsAction,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneGraphicsFailure {
    Denied,
    InvalidIntent,
    SiteMismatch,
    Unavailable,
    Busy,
    NotFound,
    Conflict,
    SourceChanged,
    SourceUnavailable,
    InvalidImage,
    ImageTooLarge,
    LayerLimit,
    MemoryLimit,
    StaleResource,
    InvalidSequence,
    StreamClosed,
    PhysicalUnknown,
    RevisionExhausted,
    InvalidFile,
    CellSizeUnavailable,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum PaneGraphicsOutcome {
    Observed {
        client_id: WorkbenchClientId,
        heartbeat: crate::ClientViewSequence,
    },
    Info {
        target: PaneGraphicsTarget,
        revision: WorkbenchRevision,
        layer_count: u16,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        client_id: Option<WorkbenchClientId>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        capabilities: Option<PaneGraphicsCapabilities>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        pane_visible: Option<bool>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        drawn_revision: Option<WorkbenchRevision>,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        file_frame_directory: Option<String>,
    },
    Catalog {
        target: PaneGraphicsTarget,
        revision: WorkbenchRevision,
        layers: Vec<PaneGraphicsLayer>,
    },
    Pixels {
        target: PaneGraphicsTarget,
        layer_id: String,
        resource_revision: WorkbenchRevision,
        width: u32,
        height: u32,
        offset: u32,
        total_bytes: u32,
        data_base64: String,
        complete: bool,
    },
    Applied {
        target: PaneGraphicsTarget,
        layer_id: String,
        revision: WorkbenchRevision,
        changed: bool,
    },
    StreamOpened {
        stream_id: WorkbenchRequestId,
        target: PaneGraphicsTarget,
        layer_id: String,
        z_index: i32,
        revision: WorkbenchRevision,
        #[serde(deserialize_with = "super::json_fields::deserialize_nullable")]
        file_frame_directory: Option<String>,
    },
    FrameApplied {
        stream_id: WorkbenchRequestId,
        target: PaneGraphicsTarget,
        layer_id: String,
        sequence: WorkbenchRevision,
        frame_revision: WorkbenchRevision,
        revision: WorkbenchRevision,
        changed: bool,
    },
    Closed {
        stream_id: WorkbenchRequestId,
        revision: WorkbenchRevision,
        removed: bool,
    },
    Failed {
        reason: PaneGraphicsFailure,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaneGraphicsResult {
    pub protocol_version: ProtocolVersion,
    pub request_id: WorkbenchRequestId,
    pub site_id: SiteId,
    pub outcome: PaneGraphicsOutcome,
}
impl PaneGraphicsRequest {
    pub fn validate(&self) -> Result<(), ContractError> {
        validation::request(self)
    }
}
impl PaneGraphicsResult {
    pub fn validate(&self) -> Result<(), ContractError> {
        validation::result(self)
    }
    #[must_use]
    pub fn answers(&self, request: &PaneGraphicsRequest) -> bool {
        validation::answers(self, request)
    }
}
impl PaneGraphicsFrame {
    pub fn validate(&self, maximum_input_bytes: usize) -> Result<(), ContractError> {
        validation::frame(self, maximum_input_bytes)
    }
}
/// Canonical layer names never contain path separators or control characters.
#[must_use]
pub fn graphics_layer_id(value: &str) -> bool {
    (1..=80).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
/// Checked canonical RGBA length; dimension validity is independent of the caller's image encoding.
pub fn graphics_rgba_bytes(width: u32, height: u32) -> Result<usize, ContractError> {
    if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
        return Err(ContractError::InvalidDocument);
    }
    let length = u64::from(width) * u64::from(height) * 4;
    if length > MAXIMUM_GRAPHICS_DECODED_BYTES as u64 {
        return Err(ContractError::InvalidDocument);
    }
    usize::try_from(length).map_err(|_| ContractError::InvalidDocument)
}
/// Validates padded base64, including unused pad bits, without decoding or allocating.
pub fn graphics_base64_bytes(value: &str) -> Result<usize, ContractError> {
    validation::base64_bytes(value)
}
