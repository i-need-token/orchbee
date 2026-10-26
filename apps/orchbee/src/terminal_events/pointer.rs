//! Decoded pointer-report data; one-shot host capability admission remains outside the parser.

/// The five values defined by DECRPM for one DEC private mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModeReportState {
    Unsupported,
    Set,
    Reset,
    PermanentlySet,
    PermanentlyReset,
}

/// Pointer-admission replies recognized and removed by the host decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostReply {
    Mode1003(ModeReportState),
    PrimaryDeviceAttributes,
}
