//! Decoded pointer-report data; one-shot host capability admission remains outside the parser.

use super::ModeReportState;

/// Pointer-admission replies recognized and removed by the host decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReply {
    Mode1003(ModeReportState),
    PrimaryDeviceAttributes,
}
