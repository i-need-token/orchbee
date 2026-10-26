//! Defines checked terminal incarnation, exit, failure, cleanup, and process-observation facts
//! and their shared JSON refinements. These facts carry reported state without observing a
//! process or inferring lifecycle transitions.

use serde::{Deserialize, Serialize};

use crate::{
    CreationId, HostEpoch, ProtocolError, ProtocolErrorCode, TerminalId, control::SafeInteger,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalIdentity {
    pub creation_id: CreationId,
    pub host_epoch: HostEpoch,
    pub terminal_id: TerminalId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitResult {
    ExitCode(u32),
    Signal(u32),
    Unknown(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFailure {
    AdmitTerminalReply,
    ApplyResize,
    DispatchTermination,
    ObserveProcess,
    ReadOutput,
    SynchronizeWorker,
    WriteInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCleanupFailure {
    ContainProcess,
    ReleaseOutput,
    ReleaseProcessIdentity,
    ReleaseEditorDocument,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessObservation {
    Observed(ExitResult),
    Unavailable,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalIdentityWire {
    creation_id: String,
    host_epoch: String,
    terminal_id: String,
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub(crate) struct ExitResultWire(ExitResultWireValue);

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ExitResultWireValue {
    ExitCode(ExitCodeWire),
    Signal(SignalExitWire),
    Unknown(UnknownExitWire),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExitCodeWire {
    code: SafeInteger,
    kind: ExitCodeKindWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum ExitCodeKindWire {
    #[serde(rename = "exit-code")]
    ExitCode,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SignalExitWire {
    kind: SignalExitKindWire,
    signal: SafeInteger,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum SignalExitKindWire {
    #[serde(rename = "signal")]
    Signal,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UnknownExitWire {
    kind: UnknownExitKindWire,
    value: SafeInteger,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum UnknownExitKindWire {
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FailurePairWire {
    kind: RuntimeFailureKindWire,
    stage: RuntimeFailureStageWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RuntimeFailureKindWire {
    EditorDocument,
    ProcessControl,
    ProcessObservation,
    PtyRead,
    PtyWrite,
    Resize,
    TerminalReply,
    WorkerIntegrity,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RuntimeFailureStageWire {
    AdmitTerminalReply,
    ApplyResize,
    ContainProcess,
    DispatchTermination,
    ObserveProcess,
    ReadOutput,
    ReleaseOutput,
    ReleaseProcessIdentity,
    ReleaseEditorDocument,
    SynchronizeWorker,
    WriteInput,
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
pub(crate) struct ProcessObservationWire(ProcessObservationWireValue);

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum ProcessObservationWireValue {
    Observed(ObservedProcessWire),
    Unavailable(UnavailableProcessWire),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObservedProcessWire {
    kind: ObservedProcessKindWire,
    result: ExitResultWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum ObservedProcessKindWire {
    #[serde(rename = "observed")]
    Observed,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct UnavailableProcessWire {
    failure: FailurePairWire,
    kind: UnavailableProcessKindWire,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum UnavailableProcessKindWire {
    #[serde(rename = "unavailable")]
    Unavailable,
}

pub(crate) fn identity_from_wire(
    wire: TerminalIdentityWire,
) -> Result<TerminalIdentity, ProtocolError> {
    Ok(TerminalIdentity {
        creation_id: CreationId::parse(&wire.creation_id).map_err(|_error| invalid_identity())?,
        host_epoch: HostEpoch::parse(&wire.host_epoch).map_err(|_error| invalid_identity())?,
        terminal_id: TerminalId::parse(&wire.terminal_id).map_err(|_error| invalid_identity())?,
    })
}

pub(crate) fn identity_to_wire(value: TerminalIdentity) -> TerminalIdentityWire {
    TerminalIdentityWire {
        creation_id: value.creation_id.to_text(),
        host_epoch: value.host_epoch.to_text(),
        terminal_id: value.terminal_id.to_text(),
    }
}

pub(crate) fn exit_result_from_wire(wire: ExitResultWire) -> Result<ExitResult, ProtocolError> {
    match wire.0 {
        ExitResultWireValue::ExitCode(value) => Ok(ExitResult::ExitCode(unsigned_u32(value.code)?)),
        ExitResultWireValue::Signal(value) => {
            let signal = unsigned_u32(value.signal)?;
            if signal == 0 {
                return Err(invalid_field("process signal must be greater than zero"));
            }
            Ok(ExitResult::Signal(signal))
        }
        ExitResultWireValue::Unknown(value) => Ok(ExitResult::Unknown(unsigned_u32(value.value)?)),
    }
}

pub(crate) fn exit_result_to_wire(value: ExitResult) -> ExitResultWire {
    ExitResultWire(match value {
        ExitResult::ExitCode(code) => ExitResultWireValue::ExitCode(ExitCodeWire {
            code: SafeInteger::new(u64::from(code)),
            kind: ExitCodeKindWire::ExitCode,
        }),
        ExitResult::Signal(signal) => ExitResultWireValue::Signal(SignalExitWire {
            kind: SignalExitKindWire::Signal,
            signal: SafeInteger::new(u64::from(signal)),
        }),
        ExitResult::Unknown(value) => ExitResultWireValue::Unknown(UnknownExitWire {
            kind: UnknownExitKindWire::Unknown,
            value: SafeInteger::new(u64::from(value)),
        }),
    })
}

pub(crate) fn runtime_failure_from_wire(
    wire: FailurePairWire,
) -> Result<RuntimeFailure, ProtocolError> {
    match (wire.kind, wire.stage) {
        (RuntimeFailureKindWire::TerminalReply, RuntimeFailureStageWire::AdmitTerminalReply) => {
            Ok(RuntimeFailure::AdmitTerminalReply)
        }
        (RuntimeFailureKindWire::Resize, RuntimeFailureStageWire::ApplyResize) => {
            Ok(RuntimeFailure::ApplyResize)
        }
        (RuntimeFailureKindWire::ProcessControl, RuntimeFailureStageWire::DispatchTermination) => {
            Ok(RuntimeFailure::DispatchTermination)
        }
        (RuntimeFailureKindWire::ProcessObservation, RuntimeFailureStageWire::ObserveProcess) => {
            Ok(RuntimeFailure::ObserveProcess)
        }
        (RuntimeFailureKindWire::PtyRead, RuntimeFailureStageWire::ReadOutput) => {
            Ok(RuntimeFailure::ReadOutput)
        }
        (RuntimeFailureKindWire::WorkerIntegrity, RuntimeFailureStageWire::SynchronizeWorker) => {
            Ok(RuntimeFailure::SynchronizeWorker)
        }
        (RuntimeFailureKindWire::PtyWrite, RuntimeFailureStageWire::WriteInput) => {
            Ok(RuntimeFailure::WriteInput)
        }
        (
            RuntimeFailureKindWire::EditorDocument
            | RuntimeFailureKindWire::ProcessControl
            | RuntimeFailureKindWire::ProcessObservation
            | RuntimeFailureKindWire::PtyRead
            | RuntimeFailureKindWire::PtyWrite
            | RuntimeFailureKindWire::Resize
            | RuntimeFailureKindWire::TerminalReply
            | RuntimeFailureKindWire::WorkerIntegrity,
            RuntimeFailureStageWire::AdmitTerminalReply
            | RuntimeFailureStageWire::ApplyResize
            | RuntimeFailureStageWire::ContainProcess
            | RuntimeFailureStageWire::DispatchTermination
            | RuntimeFailureStageWire::ObserveProcess
            | RuntimeFailureStageWire::ReadOutput
            | RuntimeFailureStageWire::ReleaseOutput
            | RuntimeFailureStageWire::ReleaseProcessIdentity
            | RuntimeFailureStageWire::ReleaseEditorDocument
            | RuntimeFailureStageWire::SynchronizeWorker
            | RuntimeFailureStageWire::WriteInput,
        ) => Err(invalid_field("runtime failure kind and stage do not match")),
    }
}

pub(crate) fn runtime_failure_to_wire(value: RuntimeFailure) -> FailurePairWire {
    let (kind, stage) = match value {
        RuntimeFailure::AdmitTerminalReply => (
            RuntimeFailureKindWire::TerminalReply,
            RuntimeFailureStageWire::AdmitTerminalReply,
        ),
        RuntimeFailure::ApplyResize => (
            RuntimeFailureKindWire::Resize,
            RuntimeFailureStageWire::ApplyResize,
        ),
        RuntimeFailure::DispatchTermination => (
            RuntimeFailureKindWire::ProcessControl,
            RuntimeFailureStageWire::DispatchTermination,
        ),
        RuntimeFailure::ObserveProcess => (
            RuntimeFailureKindWire::ProcessObservation,
            RuntimeFailureStageWire::ObserveProcess,
        ),
        RuntimeFailure::ReadOutput => (
            RuntimeFailureKindWire::PtyRead,
            RuntimeFailureStageWire::ReadOutput,
        ),
        RuntimeFailure::SynchronizeWorker => (
            RuntimeFailureKindWire::WorkerIntegrity,
            RuntimeFailureStageWire::SynchronizeWorker,
        ),
        RuntimeFailure::WriteInput => (
            RuntimeFailureKindWire::PtyWrite,
            RuntimeFailureStageWire::WriteInput,
        ),
    };
    FailurePairWire { kind, stage }
}

pub(crate) fn cleanup_failure_from_wire(
    wire: FailurePairWire,
) -> Result<RuntimeCleanupFailure, ProtocolError> {
    match (wire.kind, wire.stage) {
        (RuntimeFailureKindWire::ProcessControl, RuntimeFailureStageWire::ContainProcess) => {
            Ok(RuntimeCleanupFailure::ContainProcess)
        }
        (RuntimeFailureKindWire::PtyRead, RuntimeFailureStageWire::ReleaseOutput) => {
            Ok(RuntimeCleanupFailure::ReleaseOutput)
        }
        (
            RuntimeFailureKindWire::ProcessObservation,
            RuntimeFailureStageWire::ReleaseProcessIdentity,
        ) => Ok(RuntimeCleanupFailure::ReleaseProcessIdentity),
        (
            RuntimeFailureKindWire::EditorDocument,
            RuntimeFailureStageWire::ReleaseEditorDocument,
        ) => Ok(RuntimeCleanupFailure::ReleaseEditorDocument),
        (
            RuntimeFailureKindWire::EditorDocument
            | RuntimeFailureKindWire::ProcessControl
            | RuntimeFailureKindWire::ProcessObservation
            | RuntimeFailureKindWire::PtyRead
            | RuntimeFailureKindWire::PtyWrite
            | RuntimeFailureKindWire::Resize
            | RuntimeFailureKindWire::TerminalReply
            | RuntimeFailureKindWire::WorkerIntegrity,
            RuntimeFailureStageWire::AdmitTerminalReply
            | RuntimeFailureStageWire::ApplyResize
            | RuntimeFailureStageWire::ContainProcess
            | RuntimeFailureStageWire::DispatchTermination
            | RuntimeFailureStageWire::ObserveProcess
            | RuntimeFailureStageWire::ReadOutput
            | RuntimeFailureStageWire::ReleaseOutput
            | RuntimeFailureStageWire::ReleaseProcessIdentity
            | RuntimeFailureStageWire::ReleaseEditorDocument
            | RuntimeFailureStageWire::SynchronizeWorker
            | RuntimeFailureStageWire::WriteInput,
        ) => Err(invalid_field(
            "runtime cleanup failure kind and stage do not match",
        )),
    }
}

pub(crate) fn cleanup_failure_to_wire(value: RuntimeCleanupFailure) -> FailurePairWire {
    let (kind, stage) = match value {
        RuntimeCleanupFailure::ContainProcess => (
            RuntimeFailureKindWire::ProcessControl,
            RuntimeFailureStageWire::ContainProcess,
        ),
        RuntimeCleanupFailure::ReleaseOutput => (
            RuntimeFailureKindWire::PtyRead,
            RuntimeFailureStageWire::ReleaseOutput,
        ),
        RuntimeCleanupFailure::ReleaseProcessIdentity => (
            RuntimeFailureKindWire::ProcessObservation,
            RuntimeFailureStageWire::ReleaseProcessIdentity,
        ),
        RuntimeCleanupFailure::ReleaseEditorDocument => (
            RuntimeFailureKindWire::EditorDocument,
            RuntimeFailureStageWire::ReleaseEditorDocument,
        ),
    };
    FailurePairWire { kind, stage }
}

pub(crate) fn process_observation_from_wire(
    wire: ProcessObservationWire,
) -> Result<ProcessObservation, ProtocolError> {
    match wire.0 {
        ProcessObservationWireValue::Observed(value) => Ok(ProcessObservation::Observed(
            exit_result_from_wire(value.result)?,
        )),
        ProcessObservationWireValue::Unavailable(value) => {
            if runtime_failure_from_wire(value.failure)? != RuntimeFailure::ObserveProcess {
                return Err(invalid_field(
                    "unavailable process observation has the wrong failure",
                ));
            }
            Ok(ProcessObservation::Unavailable)
        }
    }
}

pub(crate) fn process_observation_to_wire(value: ProcessObservation) -> ProcessObservationWire {
    ProcessObservationWire(match value {
        ProcessObservation::Observed(result) => {
            ProcessObservationWireValue::Observed(ObservedProcessWire {
                kind: ObservedProcessKindWire::Observed,
                result: exit_result_to_wire(result),
            })
        }
        ProcessObservation::Unavailable => {
            ProcessObservationWireValue::Unavailable(UnavailableProcessWire {
                failure: runtime_failure_to_wire(RuntimeFailure::ObserveProcess),
                kind: UnavailableProcessKindWire::Unavailable,
            })
        }
    })
}

pub(crate) fn validate_exit_result(value: ExitResult) -> Result<(), ProtocolError> {
    if matches!(value, ExitResult::Signal(0)) {
        return Err(invalid_field("process signal must be greater than zero"));
    }
    Ok(())
}

pub(crate) fn validate_process_observation(value: ProcessObservation) -> Result<(), ProtocolError> {
    if let ProcessObservation::Observed(exit) = value {
        validate_exit_result(exit)?;
    }
    Ok(())
}

pub(crate) fn canonical_u64(value: &str) -> Result<u64, ProtocolError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid_field(
            "unsigned 64-bit text is not in canonical decimal form",
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_error| invalid_field("canonical decimal text exceeds the unsigned 64-bit range"))
}

pub(crate) fn bounded_u16(
    value: SafeInteger,
    minimum: u16,
    maximum: usize,
) -> Result<u16, ProtocolError> {
    let value = u16::try_from(value.get())
        .map_err(|_error| invalid_field("JSON integer exceeds its field bound"))?;
    if value < minimum || usize::from(value) > maximum {
        return Err(invalid_field("JSON integer is outside its field bounds"));
    }
    Ok(value)
}

pub(crate) fn bounded_u32(
    value: SafeInteger,
    minimum: u32,
    maximum: u32,
) -> Result<u32, ProtocolError> {
    let value = u32::try_from(value.get())
        .map_err(|_error| invalid_field("JSON integer exceeds its field bound"))?;
    if value < minimum || value > maximum {
        return Err(invalid_field("JSON integer is outside its field bounds"));
    }
    Ok(value)
}

fn unsigned_u32(value: SafeInteger) -> Result<u32, ProtocolError> {
    bounded_u32(value, 0, u32::MAX)
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}

const fn invalid_identity() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidIdentifier,
        "terminal identity is not canonical nonzero 128-bit hexadecimal text",
    )
}
