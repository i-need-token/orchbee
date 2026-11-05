//! Generation-scoped live terminal inventory.
//! These sampled facts grant neither process control nor disposal authority.
use std::num::NonZeroU64;

use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::{
    MessageRole, ProtocolError, ProtocolErrorCode, TerminalThemeOperationId,
    control::{RequiredNullable, SafeInteger, decode_wire, preflight_json, serialize_wire},
    limits::{
        MAXIMUM_FRAME_COLUMNS, MAXIMUM_FRAME_ROWS, MAXIMUM_INVENTORY_PAGE_TERMINALS,
        MAXIMUM_RGB_VALUE, TERMINAL_THEME_PALETTE_ENTRIES,
    },
    terminal_configuration::{
        TerminalConfiguration, TerminalConfigurationWire, configuration_from_wire,
        configuration_to_wire, validate_configuration,
    },
    terminal_fact::{
        ExitResult, ExitResultWire, FailurePairWire, ProcessObservation, ProcessObservationWire,
        RuntimeCleanupFailure, RuntimeFailure, TerminalIdentity, TerminalIdentityWire, bounded_u16,
        bounded_u32, canonical_u64, cleanup_failure_from_wire, cleanup_failure_to_wire,
        exit_result_from_wire, exit_result_to_wire, identity_from_wire, identity_to_wire,
        process_observation_from_wire, process_observation_to_wire, runtime_failure_from_wire,
        runtime_failure_to_wire, validate_exit_result, validate_process_observation,
    },
};

const LIST_TERMINALS_KIND: u16 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorScheme {
    Dark,
    Light,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalTheme {
    pub color_scheme: ColorScheme,
    pub default_background_rgb: u32,
    pub default_cursor_rgb: Option<u32>,
    pub default_foreground_rgb: u32,
    pub palette: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalThemeState {
    pub operation_id: Option<TerminalThemeOperationId>,
    pub revision: u64,
    pub terminal: TerminalTheme,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSummaryCore {
    pub configuration: TerminalConfiguration,
    pub configuration_revision: u64,
    pub identity: TerminalIdentity,
    pub revision: u64,
    pub theme: TerminalThemeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalGeometry {
    pub columns: u16,
    pub rows: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FailureContainment {
    Stopping {
        process: Option<ProcessObservation>,
    },
    Blocked {
        cleanup_failure: RuntimeCleanupFailure,
        process: Option<ProcessObservation>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalSummary {
    AwaitingStart {
        core: TerminalSummaryCore,
    },
    Running {
        core: TerminalSummaryCore,
        geometry: TerminalGeometry,
        terminate_requested: bool,
    },
    Finishing {
        core: TerminalSummaryCore,
        exit: Option<ExitResult>,
        geometry: TerminalGeometry,
        terminate_requested: bool,
    },
    ContainingFailure {
        core: TerminalSummaryCore,
        failure: RuntimeFailure,
        failure_containment: FailureContainment,
        geometry: TerminalGeometry,
    },
    Exited {
        core: TerminalSummaryCore,
        exit: ExitResult,
        geometry: TerminalGeometry,
        terminate_requested: bool,
    },
    Failed {
        core: TerminalSummaryCore,
        failure: RuntimeFailure,
        geometry: TerminalGeometry,
        process: ProcessObservation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalInventoryCursor {
    pub inventory_generation: NonZeroU64,
    pub offset: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListTerminals {
    pub cursor: Option<TerminalInventoryCursor>,
    pub maximum_terminals: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalList {
    pub inventory_generation: NonZeroU64,
    pub next_cursor: Option<TerminalInventoryCursor>,
    pub terminals: Vec<TerminalSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InventoryControl {
    ListTerminals(ListTerminals),
    TerminalList(TerminalList),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedInventoryControl {
    pub kind: u16,
    pub payload: Vec<u8>,
    pub role: MessageRole,
}

pub fn decode_inventory_control(
    kind: u16,
    role: MessageRole,
    payload: &[u8],
) -> Result<InventoryControl, ProtocolError> {
    preflight_json(payload)?;
    match (kind, role) {
        (LIST_TERMINALS_KIND, MessageRole::Request) => {
            decode_wire::<ListTerminalsWire>(payload).and_then(list_terminals_from_wire)
        }
        (LIST_TERMINALS_KIND, MessageRole::Response) => {
            decode_wire::<TerminalListWire>(payload).and_then(terminal_list_from_wire)
        }
        // This codec accepts only its declared message family and request/response roles.
        _ => Err(invalid_field(
            "message kind and role do not select a terminal-inventory payload",
        )),
    }
}

pub fn encode_inventory_control(
    value: &InventoryControl,
) -> Result<EncodedInventoryControl, ProtocolError> {
    let (role, payload) = match value {
        InventoryControl::ListTerminals(request) => {
            validate_list_terminals(request)?;
            let wire = ListTerminalsEncodeWire {
                cursor: RequiredNullable::from_option(request.cursor.map(cursor_to_wire)),
                maximum_terminals: SafeInteger::new(u64::from(request.maximum_terminals)),
            };
            (MessageRole::Request, serialize_wire(&wire)?)
        }
        InventoryControl::TerminalList(response) => {
            validate_terminal_list(response)?;
            let terminals = response
                .terminals
                .iter()
                .map(TerminalSummaryEncodeWire)
                .collect();
            let wire = TerminalListEncodeWire {
                inventory_generation: response.inventory_generation.get().to_string(),
                next_cursor: RequiredNullable::from_option(
                    response.next_cursor.map(cursor_to_wire),
                ),
                terminals,
            };
            (MessageRole::Response, serialize_wire(&wire)?)
        }
    };
    Ok(EncodedInventoryControl {
        kind: LIST_TERMINALS_KIND,
        payload,
        role,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ListTerminalsWire {
    #[serde(default)]
    cursor: RequiredNullable<TerminalInventoryCursorWire>,
    maximum_terminals: SafeInteger,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListTerminalsEncodeWire {
    cursor: RequiredNullable<TerminalInventoryCursorWire>,
    maximum_terminals: SafeInteger,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalListWire {
    inventory_generation: String,
    #[serde(default)]
    next_cursor: RequiredNullable<TerminalInventoryCursorWire>,
    terminals: Vec<TerminalSummaryWire>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalListEncodeWire<'value> {
    inventory_generation: String,
    next_cursor: RequiredNullable<TerminalInventoryCursorWire>,
    terminals: Vec<TerminalSummaryEncodeWire<'value>>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalInventoryCursorWire {
    inventory_generation: String,
    offset: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TerminalSummaryWire {
    configuration: TerminalConfigurationWire,
    configuration_revision: String,
    identity: TerminalIdentityWire,
    revision: String,
    theme: TerminalThemeStateWire,
    #[serde(default)]
    columns: RequiredNullable<SafeInteger>,
    #[serde(default)]
    exit: RequiredNullable<ExitResultWire>,
    #[serde(default)]
    failure: RequiredNullable<FailurePairWire>,
    #[serde(default)]
    failure_containment: RequiredNullable<FailureContainmentWire>,
    phase: TerminalPhaseWire,
    #[serde(default)]
    process: RequiredNullable<ProcessObservationWire>,
    #[serde(default)]
    rows: RequiredNullable<SafeInteger>,
    terminate_requested: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalThemeStateWire {
    #[serde(default)]
    operation_id: RequiredNullable<String>,
    revision: String,
    terminal: TerminalThemeWire,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TerminalThemeWire {
    color_scheme: ColorSchemeWire,
    default_background_rgb: SafeInteger,
    #[serde(default)]
    default_cursor_rgb: RequiredNullable<SafeInteger>,
    default_foreground_rgb: SafeInteger,
    palette: Vec<SafeInteger>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ColorSchemeWire {
    Dark,
    Light,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum TerminalPhaseWire {
    AwaitingStart,
    ContainingFailure,
    Exited,
    Failed,
    Finishing,
    Running,
}

#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum FailureContainmentWire {
    Stopping(StoppingContainmentWire),
    Blocked(BlockedContainmentWire),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoppingContainmentWire {
    kind: StoppingContainmentKindWire,
    #[serde(default)]
    process: RequiredNullable<ProcessObservationWire>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum StoppingContainmentKindWire {
    #[serde(rename = "stopping")]
    Stopping,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BlockedContainmentWire {
    cleanup_failure: FailurePairWire,
    kind: BlockedContainmentKindWire,
    #[serde(default)]
    process: RequiredNullable<ProcessObservationWire>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum BlockedContainmentKindWire {
    #[serde(rename = "blocked")]
    Blocked,
}

struct TerminalSummaryEncodeWire<'value>(&'value TerminalSummary);

impl Serialize for TerminalSummaryEncodeWire<'_> {
    fn serialize<SerializerType>(
        &self,
        serializer: SerializerType,
    ) -> Result<SerializerType::Ok, SerializerType::Error>
    where
        SerializerType: Serializer,
    {
        let mut state = serializer.serialize_struct("TerminalSummary", 13)?;
        let null = Option::<u8>::None;
        match self.0 {
            TerminalSummary::AwaitingStart { core } => {
                serialize_summary_core(&mut state, core)?;
                state.serialize_field("columns", &null)?;
                state.serialize_field("exit", &null)?;
                state.serialize_field("failure", &null)?;
                state.serialize_field("failureContainment", &null)?;
                state.serialize_field("phase", "awaiting-start")?;
                state.serialize_field("process", &null)?;
                state.serialize_field("rows", &null)?;
                state.serialize_field("terminateRequested", &false)?;
            }
            TerminalSummary::Running {
                core,
                geometry,
                terminate_requested,
            } => {
                serialize_summary_core(&mut state, core)?;
                serialize_geometry(&mut state, *geometry)?;
                state.serialize_field("failure", &null)?;
                state.serialize_field("failureContainment", &null)?;
                state.serialize_field("process", &null)?;
                state.serialize_field("exit", &null)?;
                state.serialize_field("phase", "running")?;
                state.serialize_field("terminateRequested", terminate_requested)?;
            }
            TerminalSummary::Finishing {
                core,
                exit,
                geometry,
                terminate_requested,
            } => {
                serialize_summary_core(&mut state, core)?;
                serialize_geometry(&mut state, *geometry)?;
                state.serialize_field("failure", &null)?;
                state.serialize_field("failureContainment", &null)?;
                state.serialize_field("process", &null)?;
                state.serialize_field("exit", &exit.map(exit_result_to_wire))?;
                state.serialize_field("phase", "finishing")?;
                state.serialize_field("terminateRequested", terminate_requested)?;
            }
            TerminalSummary::ContainingFailure {
                core,
                failure,
                failure_containment,
                geometry,
            } => {
                serialize_summary_core(&mut state, core)?;
                serialize_geometry(&mut state, *geometry)?;
                state.serialize_field("exit", &null)?;
                state.serialize_field("failure", &runtime_failure_to_wire(*failure))?;
                state.serialize_field(
                    "failureContainment",
                    &failure_containment_to_wire(failure_containment),
                )?;
                state.serialize_field("phase", "containing-failure")?;
                state.serialize_field("process", &null)?;
                state.serialize_field("terminateRequested", &true)?;
            }
            TerminalSummary::Exited {
                core,
                exit,
                geometry,
                terminate_requested,
            } => {
                serialize_summary_core(&mut state, core)?;
                serialize_geometry(&mut state, *geometry)?;
                state.serialize_field("failure", &null)?;
                state.serialize_field("failureContainment", &null)?;
                state.serialize_field("process", &null)?;
                state.serialize_field("exit", &exit_result_to_wire(*exit))?;
                state.serialize_field("phase", "exited")?;
                state.serialize_field("terminateRequested", terminate_requested)?;
            }
            TerminalSummary::Failed {
                core,
                failure,
                geometry,
                process,
            } => {
                serialize_summary_core(&mut state, core)?;
                serialize_geometry(&mut state, *geometry)?;
                state.serialize_field("exit", &null)?;
                state.serialize_field("failure", &runtime_failure_to_wire(*failure))?;
                state.serialize_field("failureContainment", &null)?;
                state.serialize_field("phase", "failed")?;
                state.serialize_field("process", &process_observation_to_wire(*process))?;
                state.serialize_field("terminateRequested", &true)?;
            }
        }
        state.end()
    }
}

fn serialize_summary_core<State>(
    state: &mut State,
    core: &TerminalSummaryCore,
) -> Result<(), State::Error>
where
    State: SerializeStruct,
{
    state.serialize_field("configuration", &configuration_to_wire(&core.configuration))?;
    state.serialize_field(
        "configurationRevision",
        &core.configuration_revision.to_string(),
    )?;
    state.serialize_field("identity", &identity_to_wire(core.identity))?;
    state.serialize_field("revision", &core.revision.to_string())?;
    state.serialize_field("theme", &theme_state_to_wire(&core.theme))
}

fn serialize_geometry<State>(
    state: &mut State,
    geometry: TerminalGeometry,
) -> Result<(), State::Error>
where
    State: SerializeStruct,
{
    state.serialize_field("columns", &SafeInteger::new(u64::from(geometry.columns)))?;
    state.serialize_field("rows", &SafeInteger::new(u64::from(geometry.rows)))
}

fn list_terminals_from_wire(wire: ListTerminalsWire) -> Result<InventoryControl, ProtocolError> {
    let maximum_terminals =
        bounded_u16(wire.maximum_terminals, 1, MAXIMUM_INVENTORY_PAGE_TERMINALS)?;
    let cursor = wire
        .cursor
        .into_option()?
        .map(cursor_from_wire)
        .transpose()?;
    Ok(InventoryControl::ListTerminals(ListTerminals {
        cursor,
        maximum_terminals,
    }))
}

fn terminal_list_from_wire(wire: TerminalListWire) -> Result<InventoryControl, ProtocolError> {
    if wire.terminals.len() > MAXIMUM_INVENTORY_PAGE_TERMINALS {
        return Err(invalid_field(
            "terminal inventory page exceeds its terminal-count bound",
        ));
    }
    let inventory_generation = positive_canonical_u64(&wire.inventory_generation)?;
    let next_cursor = wire
        .next_cursor
        .into_option()?
        .map(cursor_from_wire)
        .transpose()?;
    if next_cursor.is_some_and(|cursor| cursor.inventory_generation != inventory_generation) {
        return Err(invalid_field(
            "terminal inventory cursor names a different generation",
        ));
    }
    let terminals = wire
        .terminals
        .into_iter()
        .map(terminal_summary_from_wire)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InventoryControl::TerminalList(TerminalList {
        inventory_generation,
        next_cursor,
        terminals,
    }))
}

fn terminal_summary_from_wire(wire: TerminalSummaryWire) -> Result<TerminalSummary, ProtocolError> {
    let core = TerminalSummaryCore {
        configuration: configuration_from_wire(wire.configuration)?,
        configuration_revision: canonical_u64(&wire.configuration_revision)?,
        identity: identity_from_wire(wire.identity)?,
        revision: canonical_u64(&wire.revision)?,
        theme: theme_state_from_wire(wire.theme)?,
    };
    let columns = wire
        .columns
        .into_option()?
        .map(geometry_dimension)
        .transpose()?;
    let rows = wire
        .rows
        .into_option()?
        .map(geometry_row_dimension)
        .transpose()?;
    let exit = wire
        .exit
        .into_option()?
        .map(exit_result_from_wire)
        .transpose()?;
    let failure = wire
        .failure
        .into_option()?
        .map(runtime_failure_from_wire)
        .transpose()?;
    let failure_containment = wire
        .failure_containment
        .into_option()?
        .map(failure_containment_from_wire)
        .transpose()?;
    let process = wire
        .process
        .into_option()?
        .map(process_observation_from_wire)
        .transpose()?;

    match (
        wire.phase,
        columns,
        rows,
        exit,
        failure,
        failure_containment,
        process,
        wire.terminate_requested,
    ) {
        (TerminalPhaseWire::AwaitingStart, None, None, None, None, None, None, false) => {
            Ok(TerminalSummary::AwaitingStart { core })
        }
        (
            TerminalPhaseWire::Running,
            Some(columns),
            Some(rows),
            None,
            None,
            None,
            None,
            terminate_requested,
        ) => Ok(TerminalSummary::Running {
            core,
            geometry: TerminalGeometry { columns, rows },
            terminate_requested,
        }),
        (
            TerminalPhaseWire::Finishing,
            Some(columns),
            Some(rows),
            exit,
            None,
            None,
            None,
            terminate_requested,
        ) => Ok(TerminalSummary::Finishing {
            core,
            exit,
            geometry: TerminalGeometry { columns, rows },
            terminate_requested,
        }),
        (
            TerminalPhaseWire::ContainingFailure,
            Some(columns),
            Some(rows),
            None,
            Some(failure),
            Some(failure_containment),
            None,
            true,
        ) => Ok(TerminalSummary::ContainingFailure {
            core,
            failure,
            failure_containment,
            geometry: TerminalGeometry { columns, rows },
        }),
        (
            TerminalPhaseWire::Exited,
            Some(columns),
            Some(rows),
            Some(exit),
            None,
            None,
            None,
            terminate_requested,
        ) => Ok(TerminalSummary::Exited {
            core,
            exit,
            geometry: TerminalGeometry { columns, rows },
            terminate_requested,
        }),
        (
            TerminalPhaseWire::Failed,
            Some(columns),
            Some(rows),
            None,
            Some(failure),
            None,
            Some(process),
            true,
        ) => Ok(TerminalSummary::Failed {
            core,
            failure,
            geometry: TerminalGeometry { columns, rows },
            process,
        }),
        (
            TerminalPhaseWire::AwaitingStart
            | TerminalPhaseWire::ContainingFailure
            | TerminalPhaseWire::Exited
            | TerminalPhaseWire::Failed
            | TerminalPhaseWire::Finishing
            | TerminalPhaseWire::Running,
            _,
            _,
            _,
            _,
            _,
            _,
            _,
        ) => Err(invalid_field(
            "terminal summary fields do not match its lifecycle phase",
        )),
    }
}

pub(crate) fn theme_state_from_wire(
    wire: TerminalThemeStateWire,
) -> Result<TerminalThemeState, ProtocolError> {
    let operation_id = wire
        .operation_id
        .into_option()?
        .map(|value| TerminalThemeOperationId::parse(&value).map_err(|_error| invalid_identity()))
        .transpose()?;
    let value = TerminalThemeState {
        operation_id,
        revision: canonical_u64(&wire.revision)?,
        terminal: theme_from_wire(wire.terminal)?,
    };
    validate_theme_state(&value)?;
    Ok(value)
}

pub(crate) fn theme_state_to_wire(value: &TerminalThemeState) -> TerminalThemeStateWire {
    TerminalThemeStateWire {
        operation_id: RequiredNullable::from_option(
            value.operation_id.map(TerminalThemeOperationId::to_text),
        ),
        revision: value.revision.to_string(),
        terminal: theme_to_wire(&value.terminal),
    }
}

pub(crate) fn theme_from_wire(wire: TerminalThemeWire) -> Result<TerminalTheme, ProtocolError> {
    if wire.palette.len() != TERMINAL_THEME_PALETTE_ENTRIES {
        return Err(invalid_field(
            "terminal theme palette does not contain exactly 256 entries",
        ));
    }
    let palette = wire
        .palette
        .into_iter()
        .map(rgb_from_wire)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(TerminalTheme {
        color_scheme: match wire.color_scheme {
            ColorSchemeWire::Dark => ColorScheme::Dark,
            ColorSchemeWire::Light => ColorScheme::Light,
        },
        default_background_rgb: rgb_from_wire(wire.default_background_rgb)?,
        default_cursor_rgb: wire
            .default_cursor_rgb
            .into_option()?
            .map(rgb_from_wire)
            .transpose()?,
        default_foreground_rgb: rgb_from_wire(wire.default_foreground_rgb)?,
        palette,
    })
}

pub(crate) fn theme_to_wire(value: &TerminalTheme) -> TerminalThemeWire {
    TerminalThemeWire {
        color_scheme: match value.color_scheme {
            ColorScheme::Dark => ColorSchemeWire::Dark,
            ColorScheme::Light => ColorSchemeWire::Light,
        },
        default_background_rgb: SafeInteger::new(u64::from(value.default_background_rgb)),
        default_cursor_rgb: RequiredNullable::from_option(
            value
                .default_cursor_rgb
                .map(|rgb| SafeInteger::new(u64::from(rgb))),
        ),
        default_foreground_rgb: SafeInteger::new(u64::from(value.default_foreground_rgb)),
        palette: value
            .palette
            .iter()
            .map(|rgb| SafeInteger::new(u64::from(*rgb)))
            .collect(),
    }
}

fn cursor_from_wire(
    wire: TerminalInventoryCursorWire,
) -> Result<TerminalInventoryCursor, ProtocolError> {
    Ok(TerminalInventoryCursor {
        inventory_generation: positive_canonical_u64(&wire.inventory_generation)?,
        offset: canonical_u64(&wire.offset)?,
    })
}

fn cursor_to_wire(value: TerminalInventoryCursor) -> TerminalInventoryCursorWire {
    TerminalInventoryCursorWire {
        inventory_generation: value.inventory_generation.get().to_string(),
        offset: value.offset.to_string(),
    }
}

fn failure_containment_from_wire(
    wire: FailureContainmentWire,
) -> Result<FailureContainment, ProtocolError> {
    match wire {
        FailureContainmentWire::Stopping(value) => Ok(FailureContainment::Stopping {
            process: value
                .process
                .into_option()?
                .map(process_observation_from_wire)
                .transpose()?,
        }),
        FailureContainmentWire::Blocked(value) => Ok(FailureContainment::Blocked {
            cleanup_failure: cleanup_failure_from_wire(value.cleanup_failure)?,
            process: value
                .process
                .into_option()?
                .map(process_observation_from_wire)
                .transpose()?,
        }),
    }
}

fn failure_containment_to_wire(value: &FailureContainment) -> FailureContainmentWire {
    match value {
        FailureContainment::Stopping { process } => {
            FailureContainmentWire::Stopping(StoppingContainmentWire {
                kind: StoppingContainmentKindWire::Stopping,
                process: RequiredNullable::from_option(process.map(process_observation_to_wire)),
            })
        }
        FailureContainment::Blocked {
            cleanup_failure,
            process,
        } => FailureContainmentWire::Blocked(BlockedContainmentWire {
            cleanup_failure: cleanup_failure_to_wire(*cleanup_failure),
            kind: BlockedContainmentKindWire::Blocked,
            process: RequiredNullable::from_option(process.map(process_observation_to_wire)),
        }),
    }
}

fn validate_list_terminals(value: &ListTerminals) -> Result<(), ProtocolError> {
    if value.maximum_terminals == 0
        || usize::from(value.maximum_terminals) > MAXIMUM_INVENTORY_PAGE_TERMINALS
    {
        return Err(invalid_field(
            "requested terminal inventory page size is outside its protocol bounds",
        ));
    }
    Ok(())
}

fn validate_terminal_list(value: &TerminalList) -> Result<(), ProtocolError> {
    if value.terminals.len() > MAXIMUM_INVENTORY_PAGE_TERMINALS {
        return Err(invalid_field(
            "terminal inventory page exceeds its terminal-count bound",
        ));
    }
    if value
        .next_cursor
        .is_some_and(|cursor| cursor.inventory_generation != value.inventory_generation)
    {
        return Err(invalid_field(
            "terminal inventory cursor names a different generation",
        ));
    }
    for terminal in &value.terminals {
        validate_terminal_summary(terminal)?;
    }
    Ok(())
}

fn validate_terminal_summary(value: &TerminalSummary) -> Result<(), ProtocolError> {
    let (core, geometry, exits, containment, process) = match value {
        TerminalSummary::AwaitingStart { core } => (core, None, None, None, None),
        TerminalSummary::Running { core, geometry, .. } => {
            (core, Some(*geometry), None, None, None)
        }
        TerminalSummary::Finishing {
            core,
            exit,
            geometry,
            ..
        } => (core, Some(*geometry), exit.as_ref(), None, None),
        TerminalSummary::ContainingFailure {
            core,
            failure_containment,
            geometry,
            ..
        } => (core, Some(*geometry), None, Some(failure_containment), None),
        TerminalSummary::Exited {
            core,
            exit,
            geometry,
            ..
        } => (core, Some(*geometry), Some(exit), None, None),
        TerminalSummary::Failed {
            core,
            geometry,
            process,
            ..
        } => (core, Some(*geometry), None, None, Some(process)),
    };
    validate_summary_core(core)?;
    if let Some(geometry) = geometry {
        validate_geometry(geometry)?;
    }
    if let Some(exit) = exits {
        validate_exit_result(*exit)?;
    }
    if let Some(containment) = containment {
        validate_failure_containment(containment)?;
    }
    if let Some(process) = process {
        validate_process_observation(*process)?;
    }
    Ok(())
}

fn validate_summary_core(value: &TerminalSummaryCore) -> Result<(), ProtocolError> {
    validate_configuration(&value.configuration)?;
    validate_theme_state(&value.theme)
}

fn validate_theme_state(value: &TerminalThemeState) -> Result<(), ProtocolError> {
    if (value.revision == 0) != value.operation_id.is_none() {
        return Err(invalid_field(
            "theme revision zero is the only state without an operation identity",
        ));
    }
    validate_theme(&value.terminal)
}

pub(crate) fn validate_theme(value: &TerminalTheme) -> Result<(), ProtocolError> {
    if value.default_background_rgb > MAXIMUM_RGB_VALUE
        || value
            .default_cursor_rgb
            .is_some_and(|rgb| rgb > MAXIMUM_RGB_VALUE)
        || value.default_foreground_rgb > MAXIMUM_RGB_VALUE
        || value.palette.len() != TERMINAL_THEME_PALETTE_ENTRIES
        || value.palette.iter().any(|rgb| *rgb > MAXIMUM_RGB_VALUE)
    {
        return Err(invalid_field(
            "terminal theme exceeds its RGB or palette bounds",
        ));
    }
    Ok(())
}

fn validate_geometry(value: TerminalGeometry) -> Result<(), ProtocolError> {
    if value.columns == 0
        || usize::from(value.columns) > MAXIMUM_FRAME_COLUMNS
        || value.rows == 0
        || usize::from(value.rows) > MAXIMUM_FRAME_ROWS
    {
        return Err(invalid_field(
            "terminal summary geometry is outside its protocol bounds",
        ));
    }
    Ok(())
}

fn validate_failure_containment(value: &FailureContainment) -> Result<(), ProtocolError> {
    let process = match value {
        FailureContainment::Stopping { process } | FailureContainment::Blocked { process, .. } => {
            process
        }
    };
    if let Some(process) = process {
        validate_process_observation(*process)?;
    }
    Ok(())
}

fn positive_canonical_u64(value: &str) -> Result<NonZeroU64, ProtocolError> {
    NonZeroU64::new(canonical_u64(value)?).ok_or_else(|| {
        invalid_field("positive canonical unsigned 64-bit text must be greater than zero")
    })
}

fn geometry_dimension(value: SafeInteger) -> Result<u16, ProtocolError> {
    bounded_u16(value, 1, MAXIMUM_FRAME_COLUMNS)
}

fn geometry_row_dimension(value: SafeInteger) -> Result<u16, ProtocolError> {
    bounded_u16(value, 1, MAXIMUM_FRAME_ROWS)
}

fn rgb_from_wire(value: SafeInteger) -> Result<u32, ProtocolError> {
    bounded_u32(value, 0, MAXIMUM_RGB_VALUE)
}

const fn invalid_field(detail: &'static str) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidField, detail)
}

const fn invalid_identity() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidIdentifier,
        "inventory identity is not canonical nonzero 128-bit hexadecimal text",
    )
}
