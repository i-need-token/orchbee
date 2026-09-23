use crate::limits::{
    MAXIMUM_CONTROL_JSON_BYTES, MAXIMUM_RAW_PAYLOAD_BYTES, MAXIMUM_ROW_WINDOW_BYTES,
    MAXIMUM_TERMINAL_FRAME_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayloadEncoding {
    ControlJson,
    RawBytes,
    TerminalFrame,
    RowWindow,
}

impl PayloadEncoding {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::ControlJson => 1,
            Self::RawBytes => 2,
            Self::TerminalFrame => 3,
            Self::RowWindow => 4,
        }
    }

    pub(crate) const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::ControlJson),
            2 => Some(Self::RawBytes),
            3 => Some(Self::TerminalFrame),
            4 => Some(Self::RowWindow),
            _ => None,
        }
    }

    pub(crate) const fn maximum_payload_bytes(self) -> usize {
        match self {
            Self::ControlJson => MAXIMUM_CONTROL_JSON_BYTES,
            Self::RawBytes => MAXIMUM_RAW_PAYLOAD_BYTES,
            Self::TerminalFrame => MAXIMUM_TERMINAL_FRAME_BYTES,
            Self::RowWindow => MAXIMUM_ROW_WINDOW_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    Request,
    Response,
    Event,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestMessageContract {
    pub payload_encoding: PayloadEncoding,
    pub success_kind: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageContract {
    pub event_encoding: Option<PayloadEncoding>,
    pub kind: u16,
    pub name: &'static str,
    pub request: Option<RequestMessageContract>,
    pub response_encoding: Option<PayloadEncoding>,
}

impl MessageContract {
    #[must_use]
    pub const fn payload_encoding(self, role: MessageRole) -> Option<PayloadEncoding> {
        match role {
            MessageRole::Request => match self.request {
                Some(request) => Some(request.payload_encoding),
                None => None,
            },
            MessageRole::Response => self.response_encoding,
            MessageRole::Event => self.event_encoding,
        }
    }
}

const fn request(payload_encoding: PayloadEncoding, success_kind: u16) -> RequestMessageContract {
    RequestMessageContract {
        payload_encoding,
        success_kind,
    }
}

macro_rules! contract {
    ($kind:literal, $name:literal, $event:expr, $request:expr, $response:expr) => {
        MessageContract {
            event_encoding: $event,
            kind: $kind,
            name: $name,
            request: $request,
            response_encoding: $response,
        }
    };
}

pub const MESSAGE_CONTRACTS: &[MessageContract] = &[
    contract!(
        1,
        "Hello",
        None,
        Some(request(PayloadEncoding::ControlJson, 2)),
        None
    ),
    contract!(
        2,
        "HelloAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        3,
        "ProtocolRejected",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        4,
        "Ping",
        None,
        Some(request(PayloadEncoding::ControlJson, 4)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        5,
        "TypedFailure",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        10,
        "ListTerminals",
        None,
        Some(request(PayloadEncoding::ControlJson, 10)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        11,
        "CreateTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 11)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        12,
        "TerminateTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 12)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        13,
        "DisposeTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 13)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        14,
        "TerminalStarted",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        15,
        "TerminalExited",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        16,
        "StartTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 17)),
        None
    ),
    contract!(
        17,
        "TerminalStartAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        18,
        "ForceTerminateTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 18)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        19,
        "TerminalFailed",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        20,
        "AttachTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 22)),
        None
    ),
    contract!(
        21,
        "DetachTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 21)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        22,
        "AttachmentCapabilities",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        23,
        "ResizeLeaseChanged",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        24,
        "UpdateAttachmentInputCapability",
        None,
        Some(request(PayloadEncoding::ControlJson, 22)),
        None
    ),
    contract!(
        25,
        "ClaimResizeLease",
        None,
        Some(request(PayloadEncoding::ControlJson, 22)),
        None
    ),
    contract!(
        30,
        "WriteInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        31,
        "InputAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        32,
        "ResizeTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 33)),
        None
    ),
    contract!(
        33,
        "ResizeAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        34,
        "KeyInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        35,
        "TextInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        36,
        "MouseInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        37,
        "FocusInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        38,
        "PasteInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        39,
        "PasteAutoInput",
        None,
        Some(request(PayloadEncoding::RawBytes, 31)),
        None
    ),
    contract!(
        41,
        "SnapshotFrame",
        Some(PayloadEncoding::TerminalFrame),
        None,
        None
    ),
    contract!(
        42,
        "DeltaFrame",
        Some(PayloadEncoding::TerminalFrame),
        None,
        None
    ),
    contract!(
        43,
        "FrameApplied",
        None,
        Some(request(PayloadEncoding::ControlJson, 43)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        44,
        "NeedSnapshot",
        None,
        Some(request(PayloadEncoding::ControlJson, 44)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        45,
        "ReadRows",
        None,
        Some(request(PayloadEncoding::ControlJson, 46)),
        None
    ),
    contract!(
        46,
        "RowsSnapshot",
        None,
        None,
        Some(PayloadEncoding::RowWindow)
    ),
    contract!(
        47,
        "SetProjectionDemand",
        None,
        Some(request(PayloadEncoding::ControlJson, 48)),
        None
    ),
    contract!(
        48,
        "ProjectionDemandAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        49,
        "ProjectionBlocked",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        50,
        "RowsApplied",
        None,
        Some(request(PayloadEncoding::ControlJson, 50)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        51,
        "SetHistoryDemand",
        None,
        Some(request(PayloadEncoding::ControlJson, 52)),
        None
    ),
    contract!(
        52,
        "HistoryDemandAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        53,
        "HistoryChanged",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        54,
        "HistoryObserved",
        None,
        Some(request(PayloadEncoding::ControlJson, 54)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        55,
        "TerminalBell",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        56,
        "TerminalClipboardWriteRequest",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        57,
        "TerminalMetadataChanged",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        58,
        "TerminalApplicationSignal",
        Some(PayloadEncoding::ControlJson),
        None,
        None
    ),
    contract!(
        60,
        "SelectionGesture",
        None,
        Some(request(PayloadEncoding::ControlJson, 62)),
        None
    ),
    contract!(
        61,
        "SelectionCommand",
        None,
        Some(request(PayloadEncoding::ControlJson, 62)),
        None
    ),
    contract!(
        62,
        "SelectionChanged",
        Some(PayloadEncoding::ControlJson),
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        63,
        "SelectionExport",
        None,
        Some(request(PayloadEncoding::ControlJson, 64)),
        None
    ),
    contract!(
        64,
        "SelectionExportStarted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        65,
        "SelectionExportChunk",
        Some(PayloadEncoding::RawBytes),
        None,
        None
    ),
    contract!(
        66,
        "SelectionExportChunkApplied",
        None,
        Some(request(PayloadEncoding::ControlJson, 66)),
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        67,
        "SelectionExportFinished",
        Some(PayloadEncoding::ControlJson),
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        68,
        "SelectionExportCancel",
        None,
        Some(request(PayloadEncoding::ControlJson, 67)),
        None
    ),
    contract!(
        70,
        "SetTerminalDefaults",
        None,
        Some(request(PayloadEncoding::ControlJson, 71)),
        None
    ),
    contract!(
        71,
        "TerminalDefaultsAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        72,
        "UpdateTerminalConfiguration",
        None,
        Some(request(PayloadEncoding::ControlJson, 73)),
        None
    ),
    contract!(
        73,
        "TerminalConfigurationAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        74,
        "ClearTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 76)),
        None
    ),
    contract!(
        75,
        "ResetTerminal",
        None,
        Some(request(PayloadEncoding::ControlJson, 76)),
        None
    ),
    contract!(
        76,
        "TerminalControlAccepted",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        77,
        "ApplyTerminalTheme",
        None,
        Some(request(PayloadEncoding::ControlJson, 78)),
        None
    ),
    contract!(
        78,
        "TerminalThemeResult",
        None,
        None,
        Some(PayloadEncoding::ControlJson)
    ),
    contract!(
        79,
        "ReadProcessInfo",
        None,
        Some(request(PayloadEncoding::ControlJson, 79)),
        Some(PayloadEncoding::ControlJson)
    ),
];

#[must_use]
pub fn message_contract(kind: u16) -> Option<&'static MessageContract> {
    MESSAGE_CONTRACTS
        .binary_search_by_key(&kind, |contract| contract.kind)
        .ok()
        .and_then(|index| MESSAGE_CONTRACTS.get(index))
}
